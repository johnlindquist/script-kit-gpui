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

#[cfg(test)]
mod tests {
    use super::super::diagnostics::OperationSource;
    use super::super::registry;
    use super::super::test_support::test_env::EnvGuard;
    use super::super::transaction::execute_plan;
    use super::*;

    fn refreshed() {
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");
    }

    /// Build a strict multi-window SetBounds plan against provider windows.
    fn multi_bounds_plan(targets: &[(u32, Bounds)]) -> WindowMutationPlan {
        let operations = targets
            .iter()
            .map(|(legacy_id, bounds)| {
                let handle = registry::resolve_legacy_window_id(*legacy_id).expect("resolve");
                let observation = registry::resolve_handle(handle).expect("observation");
                PlannedWindowMutation {
                    target: handle,
                    expected_identity: ExpectedWindowIdentity::from_observation(&observation),
                    request: RequestedMutation::SetBounds(*bounds),
                    semantic_target: None,
                    destination_display: None,
                }
            })
            .collect();
        WindowMutationPlan {
            plan_id: super::super::plan::next_plan_id(),
            source: OperationSource::Test,
            snapshot_generation: registry::registry_generation(),
            topology_generation: super::super::display_topology::topology_generation(),
            requires_topology_generation: false,
            operations,
            focus_policy: FocusPolicy::PreserveCurrentFocus,
            rollback_policy: RollbackPolicy::Strict,
            verification: VerificationPolicy::Required,
            record_undo: true,
        }
    }

    #[test]
    fn strict_second_operation_failure_restores_the_first() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Good","pid":9,
                 "bounds":{"x":0,"y":0,"width":800,"height":600}},
                {"id":2,"app":"A","title":"Bad","pid":9,
                 "mutation":{"positionDeltaX":50,"positionDeltaY":50}}
            ]}"#,
        );
        refreshed();
        clear_window_undo_history();
        let plan = multi_bounds_plan(&[
            (1, Bounds::new(100, 100, 700, 500)),
            (2, Bounds::new(200, 200, 700, 500)),
        ]);
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::RolledBack);
        let rollback = receipt.rollback.expect("rollback receipt");
        assert!(rollback.fully_restored);
        // Window 1 is back at its original frame.
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds, Bounds::new(0, 0, 800, 600));
        // A fully restored strict failure records NO undo entry.
        assert_eq!(window_undo_depths().0, 0);
        clear_window_undo_history();
    }

    #[test]
    fn best_effort_returns_partial_and_does_not_roll_back() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Good","pid":9},
                {"id":2,"app":"A","title":"Bad","pid":9,
                 "mutation":{"positionDeltaX":50}}
            ]}"#,
        );
        refreshed();
        clear_window_undo_history();
        let mut plan = multi_bounds_plan(&[
            (1, Bounds::new(100, 100, 700, 500)),
            (2, Bounds::new(200, 200, 700, 500)),
        ]);
        plan.rollback_policy = RollbackPolicy::BestEffort;
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::Partial);
        assert!(receipt.rollback.is_none());
        // The successful window keeps its new frame.
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds, Bounds::new(100, 100, 700, 500));
        clear_window_undo_history();
    }

    #[test]
    fn rollback_failure_reports_separately_and_leaves_a_recovery_record() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Fragile","pid":9,
                 "bounds":{"x":0,"y":0,"width":800,"height":600},
                 "mutation":{"destroyOnAttempt":2}},
                {"id":2,"app":"A","title":"Bad","pid":9,
                 "mutation":{"positionDeltaX":50}}
            ]}"#,
        );
        refreshed();
        clear_window_undo_history();
        let plan = multi_bounds_plan(&[
            (1, Bounds::new(100, 100, 700, 500)),
            (2, Bounds::new(200, 200, 700, 500)),
        ]);
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::RollbackFailed);
        let rollback = receipt.rollback.expect("rollback receipt");
        assert!(!rollback.fully_restored);
        assert!(rollback
            .operations
            .iter()
            .any(|operation| operation.status != MutationStatus::Succeeded));
        // Net changes remained -> a recovery record exists.
        assert_eq!(window_undo_depths().0, 1);
        clear_window_undo_history();
    }

    #[test]
    fn one_multi_window_plan_produces_one_undo_record_and_round_trips() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"One","pid":9,
                 "bounds":{"x":0,"y":0,"width":800,"height":600}},
                {"id":2,"app":"A","title":"Two","pid":9,
                 "bounds":{"x":900,"y":0,"width":800,"height":600}}
            ]}"#,
        );
        refreshed();
        clear_window_undo_history();
        let plan = multi_bounds_plan(&[
            (1, Bounds::new(50, 50, 640, 480)),
            (2, Bounds::new(950, 50, 640, 480)),
        ]);
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::Succeeded);
        assert_eq!(
            window_undo_depths(),
            (1, 0),
            "one transaction = one undo record"
        );

        // Undo restores BOTH windows (transaction boundary preserved).
        let undo_receipt = undo_last_window_transaction().expect("undo");
        assert_eq!(undo_receipt.status, MutationStatus::Succeeded);
        assert_eq!(
            super::super::test_support::window_state(1).unwrap().bounds,
            Bounds::new(0, 0, 800, 600)
        );
        assert_eq!(
            super::super::test_support::window_state(2).unwrap().bounds,
            Bounds::new(900, 0, 800, 600)
        );
        assert_eq!(window_undo_depths(), (0, 1));

        // Redo re-applies BOTH windows.
        let redo_receipt = redo_last_window_transaction().expect("redo");
        assert_eq!(redo_receipt.status, MutationStatus::Succeeded);
        assert_eq!(
            super::super::test_support::window_state(1).unwrap().bounds,
            Bounds::new(50, 50, 640, 480)
        );
        assert_eq!(window_undo_depths(), (1, 0));
        clear_window_undo_history();
    }

    #[test]
    fn a_new_ordinary_transaction_clears_redo() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"One","pid":9}
            ]}"#,
        );
        refreshed();
        clear_window_undo_history();
        let plan = multi_bounds_plan(&[(1, Bounds::new(10, 10, 700, 500))]);
        execute_plan(&plan).expect("execute");
        undo_last_window_transaction().expect("undo");
        assert_eq!(window_undo_depths(), (0, 1));

        // A fresh ordinary transaction clears the redo stack.
        let plan = multi_bounds_plan(&[(1, Bounds::new(30, 30, 700, 500))]);
        execute_plan(&plan).expect("execute");
        assert_eq!(window_undo_depths(), (1, 0));
        clear_window_undo_history();
    }

    #[test]
    fn undo_depth_is_exactly_fifty() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        clear_window_undo_history();
        let entry = UndoEntry {
            nonce: 1,
            pid: 9,
            before: RestorableWindowState {
                bounds: Bounds::new(0, 0, 100, 100),
                minimized: false,
            },
            after: RestorableWindowState {
                bounds: Bounds::new(1, 1, 100, 100),
                minimized: false,
            },
        };
        {
            let mut state = UNDO.lock();
            for index in 0..55 {
                push_undo_record(
                    &mut state,
                    UndoRecord {
                        transaction_id: format!("t{index}"),
                        entries: vec![entry.clone()],
                    },
                );
            }
        }
        assert_eq!(window_undo_depths().0, UNDO_DEPTH);
        {
            let state = UNDO.lock();
            assert_eq!(
                state.undo.front().unwrap().transaction_id,
                "t5",
                "oldest records evict first"
            );
        }
        clear_window_undo_history();
    }

    #[test]
    fn failed_undo_remains_available() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = EnvGuard::set(
            r#"{"windows":[
                {"id":1,"app":"A","title":"Doomed","pid":9,
                 "mutation":{"destroyOnAttempt":2}}
            ]}"#,
        );
        refreshed();
        clear_window_undo_history();
        let plan = multi_bounds_plan(&[(1, Bounds::new(10, 10, 700, 500))]);
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::Succeeded);
        assert_eq!(window_undo_depths(), (1, 0));

        // Destroy the window; undo must fail but keep its record.
        super::super::test_support::apply_mutation(1, None, |_| {}).ok();
        registry::refresh_from_test_provider().expect("refresh");
        let result = undo_last_window_transaction();
        assert!(result.is_err() || result.unwrap().status != MutationStatus::Succeeded);
        assert_eq!(
            window_undo_depths().0,
            1,
            "failed undo must remain on the undo stack"
        );
        clear_window_undo_history();
    }
}
