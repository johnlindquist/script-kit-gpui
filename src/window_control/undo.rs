//! Compensating rollback and transaction-scoped undo/redo.
//!
//! - Strict rollback walks successfully mutated windows in REVERSE apply
//!   order, restoring each through a verified engine operation; individual
//!   rollback failures do not stop the walk and are reported per window.
//! - One multi-window transaction = one undo record. Depth: 50. A new
//!   ordinary transaction clears redo. Undo/redo execute verified plans
//!   through the same engine with `history_mode: Skip` (no recursive
//!   records). Bounds and minimized state are undoable; focus and close are
//!   never recorded.
//! - A failed undo/redo leaves its record on its source stack. A rollback
//!   failure that leaves net changes pushes a RECOVERY record so the user
//!   can still attempt to restore the original state.

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};

use super::diagnostics::OperationSource;
use super::executor::{submit, WorkerCommand};
use super::plan::{
    ExpectedWindowIdentity, FocusPolicy, PlannedWindowMutation, RequestedMutation, RollbackPolicy,
    VerificationPolicy, WindowMutationPlan,
};
use super::transaction::{
    execute_plan_with_deadline, MutationStatus, OperationReceipt, RollbackReceipt,
    TransactionReceipt,
};
use super::types::{Bounds, WindowHandle};
use super::verification::ObservedState;

pub(super) const UNDO_DEPTH: usize = 50;

/// The reversible portion of a window's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestorableWindowState {
    pub bounds: Bounds,
    pub minimized: bool,
}

impl RestorableWindowState {
    fn from_observed(state: &ObservedState) -> Self {
        Self {
            bounds: state.bounds,
            minimized: state.minimized,
        }
    }
}

#[derive(Debug, Clone)]
struct UndoEntry {
    nonce: u64,
    pid: i32,
    before: RestorableWindowState,
    after: RestorableWindowState,
}

#[derive(Debug, Clone)]
struct UndoRecord {
    transaction_id: String,
    entries: Vec<UndoEntry>,
}

#[derive(Default)]
struct UndoState {
    undo: VecDeque<UndoRecord>,
    redo: VecDeque<UndoRecord>,
}

static UNDO: std::sync::LazyLock<parking_lot::Mutex<UndoState>> =
    std::sync::LazyLock::new(Default::default);

fn reversible(request: &RequestedMutation) -> bool {
    matches!(
        request,
        RequestedMutation::SetPosition { .. }
            | RequestedMutation::SetSize { .. }
            | RequestedMutation::SetBounds(_)
            | RequestedMutation::SetMinimized(_)
    )
}

/// Roll back successfully applied operations in reverse order.
///
/// `succeeded` holds operation indices in APPLY order; `before_states` is
/// indexed by operation index.
pub(super) fn roll_back_operations(
    plan: &WindowMutationPlan,
    succeeded: &[usize],
    before_states: &[Option<ObservedState>],
    cancelled: &Arc<AtomicBool>,
    deadline: Duration,
) -> RollbackReceipt {
    let mut operations: Vec<OperationReceipt> = Vec::new();
    let mut fully_restored = true;
    let mut recovery_entries: Vec<UndoEntry> = Vec::new();

    for &index in succeeded.iter().rev() {
        let operation = &plan.operations[index];
        if !reversible(&operation.request) {
            continue;
        }
        let Some(before) = before_states.get(index).copied().flatten() else {
            fully_restored = false;
            continue;
        };
        let restore = PlannedWindowMutation {
            target: operation.target,
            expected_identity: operation.expected_identity.clone(),
            request: RequestedMutation::SetBounds(before.bounds),
            semantic_target: None,
            destination_display: None,
        };
        let receipt = run_restore(&restore, cancelled, deadline);
        let restored_bounds = receipt.status == MutationStatus::Succeeded;

        // Restore minimized state when it changed.
        let mut restored_minimized = true;
        if let Some(actual) = receipt.actual {
            if actual.minimized != before.minimized {
                let minimize = PlannedWindowMutation {
                    request: RequestedMutation::SetMinimized(before.minimized),
                    ..restore.clone()
                };
                let minimize_receipt = run_restore(&minimize, cancelled, deadline);
                restored_minimized = minimize_receipt.status == MutationStatus::Succeeded;
            }
        }

        if !(restored_bounds && restored_minimized) {
            fully_restored = false;
            if let Some(actual) = receipt.actual {
                recovery_entries.push(UndoEntry {
                    nonce: operation.target.nonce,
                    pid: operation.target.pid,
                    before: RestorableWindowState::from_observed(&before),
                    after: RestorableWindowState::from_observed(&actual),
                });
            }
        }
        operations.push(receipt);
    }

    // Net changes remain: leave the user a recovery record.
    if !recovery_entries.is_empty() {
        let mut state = UNDO.lock();
        push_undo_record(
            &mut state,
            UndoRecord {
                transaction_id: format!("{}-recovery", plan.plan_id),
                entries: recovery_entries,
            },
        );
    }

    RollbackReceipt {
        operations,
        fully_restored,
    }
}

fn run_restore(
    operation: &PlannedWindowMutation,
    cancelled: &Arc<AtomicBool>,
    deadline: Duration,
) -> OperationReceipt {
    let receiver = match submit(
        operation.target.pid,
        WorkerCommand::Restore {
            operation: operation.clone(),
        },
        Arc::clone(cancelled),
    ) {
        Ok(receiver) => receiver,
        Err(error) => {
            return OperationReceipt {
                target: operation.target,
                status: MutationStatus::Failed,
                before: None,
                request: operation.request.clone(),
                actual: None,
                attempts: Vec::new(),
                error: Some(error.to_string()),
            }
        }
    };
    match receiver.recv_timeout(deadline) {
        Ok(reply) => OperationReceipt {
            target: operation.target,
            status: if reply.error.is_none() {
                MutationStatus::Succeeded
            } else {
                MutationStatus::Failed
            },
            before: reply.before,
            request: operation.request.clone(),
            actual: reply.after,
            attempts: reply.attempts,
            error: reply.error,
        },
        Err(_) => OperationReceipt {
            target: operation.target,
            status: MutationStatus::TimedOut,
            before: None,
            request: operation.request.clone(),
            actual: None,
            attempts: Vec::new(),
            error: Some("window_engine:rollback_timeout".into()),
        },
    }
}

fn push_undo_record(state: &mut UndoState, record: UndoRecord) {
    if state.undo.len() == UNDO_DEPTH {
        state.undo.pop_front();
    }
    state.undo.push_back(record);
}

/// Record one undo record for a successful undoable transaction.
pub(super) fn record_transaction_undo(plan: &WindowMutationPlan, receipt: &TransactionReceipt) {
    let entries: Vec<UndoEntry> = receipt
        .operations
        .iter()
        .filter(|operation| reversible(&operation.request))
        .filter_map(|operation| {
            let before = operation.before?;
            let after = operation.actual?;
            Some(UndoEntry {
                nonce: operation.target.nonce,
                pid: operation.target.pid,
                before: RestorableWindowState::from_observed(&before),
                after: RestorableWindowState::from_observed(&after),
            })
        })
        .collect();
    if entries.is_empty() {
        return;
    }
    let mut state = UNDO.lock();
    push_undo_record(
        &mut state,
        UndoRecord {
            transaction_id: plan.plan_id.clone(),
            entries,
        },
    );
    // A new ordinary transaction clears redo.
    state.redo.clear();
}

/// Resolve a recorded nonce to its CURRENT handle.
fn current_handle_for(nonce: u64, pid: i32) -> Result<(WindowHandle, ExpectedWindowIdentity)> {
    let observation =
        super::registry::resolve_nonce(nonce).context("undo target no longer exists")?;
    anyhow::ensure!(
        observation.handle.pid == pid,
        "undo target changed process identity"
    );
    Ok((
        observation.handle,
        ExpectedWindowIdentity::from_observation(&observation),
    ))
}

fn build_restore_plan(
    record: &UndoRecord,
    to_before: bool,
    source: OperationSource,
) -> Result<WindowMutationPlan> {
    // Refresh so recorded nonces resolve at the current generation.
    let snapshot = super::registry::refresh_window_registry()?;
    let mut operations = Vec::with_capacity(record.entries.len() * 2);
    for entry in &record.entries {
        let (handle, identity) = current_handle_for(entry.nonce, entry.pid)?;
        let target_state = if to_before { entry.before } else { entry.after };
        operations.push(PlannedWindowMutation {
            target: handle,
            expected_identity: identity.clone(),
            request: RequestedMutation::SetBounds(target_state.bounds),
            semantic_target: None,
            destination_display: None,
        });
        operations.push(PlannedWindowMutation {
            target: handle,
            expected_identity: identity,
            request: RequestedMutation::SetMinimized(target_state.minimized),
            semantic_target: None,
            destination_display: None,
        });
    }
    Ok(WindowMutationPlan {
        plan_id: super::plan::next_plan_id(),
        source,
        snapshot_generation: snapshot.generation,
        topology_generation: super::display_topology::topology_generation(),
        requires_topology_generation: false,
        operations,
        focus_policy: FocusPolicy::PreserveCurrentFocus,
        rollback_policy: RollbackPolicy::BestEffort,
        verification: VerificationPolicy::Required,
        // history_mode Skip: undo/redo never records recursively.
        record_undo: false,
    })
}

/// Undo the most recent window transaction.
pub fn undo_last_window_transaction() -> Result<TransactionReceipt> {
    undo_redo(true)
}

/// Redo the most recently undone window transaction.
pub fn redo_last_window_transaction() -> Result<TransactionReceipt> {
    undo_redo(false)
}

fn undo_redo(undo: bool) -> Result<TransactionReceipt> {
    // Peek without popping: a failed undo/redo stays on its source stack.
    let record = {
        let state = UNDO.lock();
        let source = if undo { &state.undo } else { &state.redo };
        source.back().cloned().context(if undo {
            "nothing to undo"
        } else {
            "nothing to redo"
        })?
    };
    let source = if undo {
        OperationSource::Undo
    } else {
        OperationSource::Redo
    };
    let plan = build_restore_plan(&record, undo, source)?;
    let receipt =
        execute_plan_with_deadline(&plan, super::mutation::TRANSACTION_RESPONSE_DEADLINE)?;
    if receipt.status == MutationStatus::Succeeded {
        let mut state = UNDO.lock();
        if undo {
            if let Some(record) = state.undo.pop_back() {
                state.redo.push_back(record);
            }
        } else if let Some(record) = state.redo.pop_back() {
            if state.undo.len() == UNDO_DEPTH {
                state.undo.pop_front();
            }
            state.undo.push_back(record);
        }
    }
    Ok(receipt)
}

/// Clear all undo/redo history (process-local).
pub fn clear_window_undo_history() {
    let mut state = UNDO.lock();
    state.undo.clear();
    state.redo.clear();
}

/// Current undo/redo depths (for tests and diagnostics).
#[doc(hidden)]
pub fn window_undo_depths() -> (usize, usize) {
    let state = UNDO.lock();
    (state.undo.len(), state.redo.len())
}
