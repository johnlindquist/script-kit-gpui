//! Transaction execution: preflight -> ordered apply -> readback -> receipt.
//!
//! Algorithm (locked): reject empty plans; check registry generation; check
//! topology generation when required; preflight EVERY operation (strict plans
//! abort without mutation on any preflight failure); sort by (pid, nonce);
//! apply; strict failures hand off to rollback (S11); record diagnostics;
//! return a truthful receipt. A caller deadline sets the cancellation flag —
//! a timed-out command can never mutate later.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

use super::diagnostics::{
    next_correlation_id, record_diagnostics, CacheStatus, OperationBackend, OperationStatus,
    WindowOperationDiagnostics,
};
use super::executor::{submit, MutationAttempt, WorkerCommand};
use super::mutation::TRANSACTION_RESPONSE_DEADLINE;
use super::plan::{
    FocusPolicy, PlannedWindowMutation, RequestedMutation, RollbackPolicy, WindowMutationPlan,
};
use super::types::WindowHandle;
use super::verification::ObservedState;

/// Terminal status of an operation or transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationStatus {
    Succeeded,
    Partial,
    Failed,
    TimedOut,
    RolledBack,
    RollbackFailed,
    Cancelled,
}

impl MutationStatus {
    fn to_diagnostics(self) -> OperationStatus {
        match self {
            Self::Succeeded => OperationStatus::Succeeded,
            Self::Partial => OperationStatus::Partial,
            Self::Failed => OperationStatus::Failed,
            Self::TimedOut => OperationStatus::TimedOut,
            Self::RolledBack => OperationStatus::RolledBack,
            Self::RollbackFailed => OperationStatus::RollbackFailed,
            Self::Cancelled => OperationStatus::Cancelled,
        }
    }
}

/// Receipt for one operation.
#[derive(Debug, Clone)]
pub struct OperationReceipt {
    pub target: WindowHandle,
    pub status: MutationStatus,
    pub before: Option<ObservedState>,
    pub request: RequestedMutation,
    pub actual: Option<ObservedState>,
    pub attempts: Vec<MutationAttempt>,
    pub error: Option<String>,
}

/// Receipt for a rollback pass (populated by S11).
#[derive(Debug, Clone)]
pub struct RollbackReceipt {
    pub operations: Vec<OperationReceipt>,
    pub fully_restored: bool,
}

/// Receipt for a whole transaction.
#[derive(Debug, Clone)]
pub struct TransactionReceipt {
    pub plan_id: String,
    pub status: MutationStatus,
    pub operations: Vec<OperationReceipt>,
    pub rollback: Option<RollbackReceipt>,
}

impl TransactionReceipt {
    /// Legacy adapter: Ok(()) ONLY for `Succeeded`; everything else is a
    /// `window_engine:<status>` error preserving the first operation error.
    pub fn into_legacy_result(self) -> Result<()> {
        if self.status == MutationStatus::Succeeded {
            return Ok(());
        }
        let detail = self
            .operations
            .iter()
            .find_map(|operation| operation.error.clone())
            .unwrap_or_else(|| format!("{:?}", self.status));
        bail!("window_engine:{:?}:{detail}", self.status)
    }
}

/// Execute with the default caller deadline.
pub fn execute_plan(plan: &WindowMutationPlan) -> Result<TransactionReceipt> {
    execute_plan_with_deadline(plan, TRANSACTION_RESPONSE_DEADLINE)
}

/// Execute a plan with an explicit per-operation reply deadline.
pub fn execute_plan_with_deadline(
    plan: &WindowMutationPlan,
    deadline: Duration,
) -> Result<TransactionReceipt> {
    let started = Instant::now();
    anyhow::ensure!(!plan.operations.is_empty(), "empty window mutation plan");

    // Multi-operation strict plans must be reversible: close/focus are not.
    if plan.rollback_policy == RollbackPolicy::Strict && plan.operations.len() > 1 {
        let irreversible = plan.operations.iter().any(|operation| {
            matches!(
                operation.request,
                RequestedMutation::Close | RequestedMutation::Focus
            )
        });
        anyhow::ensure!(
            !irreversible,
            "strict multi-window plans cannot contain close/focus operations"
        );
    }

    // Generation gates BEFORE any mutation.
    let current_generation = super::registry::registry_generation();
    if current_generation != plan.snapshot_generation {
        bail!(
            "stale plan: registry generation {} != current {}",
            plan.snapshot_generation,
            current_generation
        );
    }
    if plan.requires_topology_generation {
        let current_topology = super::display_topology::topology_generation();
        if current_topology != plan.topology_generation {
            bail!(
                "stale plan: topology generation {} != current {}",
                plan.topology_generation,
                current_topology
            );
        }
    }

    let cancelled = Arc::new(AtomicBool::new(false));
    let preflight_started = Instant::now();

    // Preflight every operation first.
    let mut before_states: Vec<Option<ObservedState>> = Vec::with_capacity(plan.operations.len());
    let mut preflight_error: Option<(usize, String)> = None;
    for (index, operation) in plan.operations.iter().enumerate() {
        let receiver = submit(
            operation.target.pid,
            WorkerCommand::Preflight(operation.clone()),
            Arc::clone(&cancelled),
        );
        let reply = match receiver {
            Ok(receiver) => match receiver.recv_timeout(deadline) {
                Ok(reply) => reply,
                Err(_) => {
                    cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
                    preflight_error = Some((index, "window_engine:preflight_timeout".into()));
                    break;
                }
            },
            Err(error) => {
                preflight_error = Some((index, error.to_string()));
                break;
            }
        };
        if let Some(error) = reply.error {
            preflight_error = Some((index, error));
            break;
        }
        before_states.push(reply.before);
    }

    if let Some((index, error)) = preflight_error {
        // Strict and best-effort plans both abort BEFORE mutation when
        // preflight fails: nothing has been written yet.
        let receipts = plan
            .operations
            .iter()
            .enumerate()
            .map(|(op_index, operation)| OperationReceipt {
                target: operation.target,
                status: if op_index == index {
                    MutationStatus::Failed
                } else {
                    MutationStatus::Cancelled
                },
                before: before_states.get(op_index).copied().flatten(),
                request: operation.request.clone(),
                actual: None,
                attempts: Vec::new(),
                error: (op_index == index).then_some(error.clone()),
            })
            .collect();
        let receipt = TransactionReceipt {
            plan_id: plan.plan_id.clone(),
            status: MutationStatus::Failed,
            operations: receipts,
            rollback: None,
        };
        record_transaction_diagnostics(plan, &receipt, started, preflight_started);
        return Ok(receipt);
    }

    // Deterministic apply order: (pid, nonce).
    let mut order: Vec<usize> = (0..plan.operations.len()).collect();
    order.sort_by_key(|&index| {
        let operation = &plan.operations[index];
        (operation.target.pid, operation.target.nonce)
    });

    let mut receipts: Vec<Option<OperationReceipt>> = vec![None; plan.operations.len()];
    let mut first_failure: Option<usize> = None;
    let mut succeeded_in_order: Vec<usize> = Vec::new();

    for &index in &order {
        let operation = &plan.operations[index];
        if first_failure.is_some() && plan.rollback_policy != RollbackPolicy::BestEffort {
            receipts[index] = Some(OperationReceipt {
                target: operation.target,
                status: MutationStatus::Cancelled,
                before: before_states.get(index).copied().flatten(),
                request: operation.request.clone(),
                actual: None,
                attempts: Vec::new(),
                error: Some("skipped after strict failure".into()),
            });
            continue;
        }
        let receipt = apply_one(
            operation,
            before_states.get(index).copied().flatten(),
            &cancelled,
            deadline,
        );
        let failed = receipt.status != MutationStatus::Succeeded;
        if failed && first_failure.is_none() {
            first_failure = Some(index);
        }
        if !failed {
            succeeded_in_order.push(index);
        }
        receipts[index] = Some(receipt);
    }

    let mut rollback: Option<RollbackReceipt> = None;
    let status = if let Some(failure_index) = first_failure {
        // Propagate the failing operation's status (TimedOut/Cancelled stay
        // visible) when no other window was touched.
        let failure_status = receipts[failure_index]
            .as_ref()
            .map(|receipt| receipt.status)
            .unwrap_or(MutationStatus::Failed);
        match plan.rollback_policy {
            RollbackPolicy::BestEffort => {
                if succeeded_in_order.is_empty() {
                    failure_status
                } else {
                    MutationStatus::Partial
                }
            }
            RollbackPolicy::Strict if !succeeded_in_order.is_empty() => {
                let result = super::undo::roll_back_operations(
                    plan,
                    &succeeded_in_order,
                    &before_states,
                    &cancelled,
                    deadline,
                );
                let fully_restored = result.fully_restored;
                rollback = Some(result);
                if fully_restored {
                    MutationStatus::RolledBack
                } else {
                    MutationStatus::RollbackFailed
                }
            }
            _ => failure_status,
        }
    } else {
        MutationStatus::Succeeded
    };

    // Focus policy: focus the target at the very end, after all geometry.
    if status == MutationStatus::Succeeded && plan.focus_policy == FocusPolicy::FocusTargetAtEnd {
        // The focus operation itself carries RequestedMutation::Focus; plans
        // with a separate focus policy but no focus op focus their last target.
        let has_focus_op = plan
            .operations
            .iter()
            .any(|operation| matches!(operation.request, RequestedMutation::Focus));
        if !has_focus_op {
            if let Some(operation) = plan.operations.last() {
                let _ = apply_one(
                    &PlannedWindowMutation {
                        request: RequestedMutation::Focus,
                        ..operation.clone()
                    },
                    None,
                    &cancelled,
                    deadline,
                );
            }
        }
    }

    let operations: Vec<OperationReceipt> = receipts.into_iter().flatten().collect();
    let receipt = TransactionReceipt {
        plan_id: plan.plan_id.clone(),
        status,
        operations,
        rollback,
    };

    // Undo recording: one record per successful undoable transaction (S11).
    if plan.record_undo && receipt.status == MutationStatus::Succeeded {
        super::undo::record_transaction_undo(plan, &receipt);
    }

    record_transaction_diagnostics(plan, &receipt, started, preflight_started);
    Ok(receipt)
}

fn apply_one(
    operation: &PlannedWindowMutation,
    before: Option<ObservedState>,
    cancelled: &Arc<AtomicBool>,
    deadline: Duration,
) -> OperationReceipt {
    let receiver = match submit(
        operation.target.pid,
        WorkerCommand::Apply(operation.clone()),
        Arc::clone(cancelled),
    ) {
        Ok(receiver) => receiver,
        Err(error) => {
            return OperationReceipt {
                target: operation.target,
                status: MutationStatus::Failed,
                before,
                request: operation.request.clone(),
                actual: None,
                attempts: Vec::new(),
                error: Some(error.to_string()),
            }
        }
    };
    match receiver.recv_timeout(deadline) {
        Ok(reply) => {
            let status = match &reply.error {
                None => MutationStatus::Succeeded,
                Some(error) if error.contains("cancelled") => MutationStatus::Cancelled,
                Some(_) => MutationStatus::Failed,
            };
            OperationReceipt {
                target: operation.target,
                status,
                before: reply.before.or(before),
                request: operation.request.clone(),
                actual: reply.after,
                attempts: reply.attempts,
                error: reply.error,
            }
        }
        Err(_) => {
            // Deadline expired: cancel so the queued/working command can
            // never mutate later.
            cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            OperationReceipt {
                target: operation.target,
                status: MutationStatus::TimedOut,
                before,
                request: operation.request.clone(),
                actual: None,
                attempts: Vec::new(),
                error: Some("window_engine:operation_timeout".into()),
            }
        }
    }
}

fn record_transaction_diagnostics(
    plan: &WindowMutationPlan,
    receipt: &TransactionReceipt,
    started: Instant,
    preflight_started: Instant,
) {
    let first = receipt.operations.first();
    let attempts = receipt
        .operations
        .iter()
        .map(|operation| operation.attempts.len() as u8)
        .max()
        .unwrap_or(0);
    let readback_count = receipt
        .operations
        .iter()
        .flat_map(|operation| operation.attempts.iter())
        .filter(|attempt| attempt.observed_bounds.is_some())
        .count() as u8;
    record_diagnostics(WindowOperationDiagnostics {
        schema_version: 1,
        correlation_id: next_correlation_id(),
        plan_id: Some(plan.plan_id.clone()),
        source: plan.source,
        backend: if super::test_support::is_active() {
            OperationBackend::TestProvider
        } else {
            OperationBackend::Ax
        },
        status: receipt.status.to_diagnostics(),
        legacy_window_id: None,
        handle_nonce: first.map(|operation| operation.target.nonce),
        registry_generation: Some(plan.snapshot_generation),
        topology_generation: plan
            .requires_topology_generation
            .then_some(plan.topology_generation),
        observation_us: 0,
        planning_us: 0,
        queue_wait_us: preflight_started.duration_since(started).as_micros() as u64,
        first_mutation_us: Some(preflight_started.duration_since(started).as_micros() as u64),
        mutation_us: started.elapsed().as_micros() as u64,
        verification_us: 0,
        rollback_us: 0,
        total_us: started.elapsed().as_micros() as u64,
        attempts,
        readback_count,
        cache_status: CacheStatus::Hit,
        ax_timeout_ms: None,
        warning_codes: Vec::new(),
        error_code: receipt
            .operations
            .iter()
            .find_map(|operation| operation.error.clone()),
    });
}

#[cfg(test)]
mod tests {
    use super::super::legacy::{compile_legacy_window_action, LegacyWindowAction};
    use super::super::registry;
    use super::super::test_support::test_env::EnvGuard;
    use super::super::types::Bounds;
    use super::*;

    fn fixture(windows_json: &str) -> EnvGuard {
        EnvGuard::set(&format!(r#"{{"windows":{windows_json}}}"#))
    }

    fn refreshed() {
        registry::reset_registry_for_tests();
        registry::refresh_from_test_provider().expect("refresh");
    }

    #[test]
    fn a_verified_move_succeeds_and_reads_back() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(r#"[{"id":1,"app":"A","title":"Doc","pid":9}]"#);
        refreshed();
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 40,
            y: 30,
        })
        .expect("plan");
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::Succeeded);
        let actual = receipt.operations[0].actual.expect("readback state");
        assert_eq!(actual.bounds.x, 40);
        assert_eq!(actual.bounds.y, 30);
        assert!(receipt.operations[0].attempts.iter().any(|a| a.verified));
    }

    #[test]
    fn clamped_mismatch_is_failure_not_success() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"Clamped","pid":9,
                 "mutation":{"minWidth":500,"minHeight":400}}]"#,
        );
        refreshed();
        let plan = compile_legacy_window_action(LegacyWindowAction::Resize {
            window_id: 1,
            width: 300,
            height: 200,
        })
        .expect("plan");
        let receipt = execute_plan(&plan).expect("execute");
        assert_ne!(
            receipt.status,
            MutationStatus::Succeeded,
            "clamped result must never be reported as success"
        );
        assert!(receipt.operations[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("rejected")));
    }

    #[test]
    fn close_with_save_prompt_simulation_is_failure() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"Prompt","pid":9,
                 "mutation":{"closeLeavesWindow":true}}]"#,
        );
        refreshed();
        let plan =
            compile_legacy_window_action(LegacyWindowAction::Close { window_id: 1 }).expect("plan");
        let receipt = execute_plan(&plan).expect("execute");
        assert_ne!(receipt.status, MutationStatus::Succeeded);
    }

    #[test]
    fn stale_registry_generation_fails_before_any_mutation() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"One","pid":9},
                {"id":2,"app":"A","title":"Two","pid":9}]"#,
        );
        refreshed();
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 5,
            y: 5,
        })
        .expect("plan");
        // Force a membership change AFTER planning: destroy window 2.
        super::super::test_support::apply_mutation(2, None, |window| {
            window.destroyed = true;
        })
        .expect("destroy");
        let mutations_before = super::super::test_support::mutation_count().expect("count");
        registry::refresh_from_test_provider().expect("refresh");
        let error = execute_plan(&plan).expect_err("stale plan must fail");
        assert!(error.to_string().contains("stale plan"));
        assert_eq!(
            super::super::test_support::mutation_count().expect("count"),
            mutations_before,
            "no mutation may happen after a stale-generation rejection"
        );
    }

    #[test]
    fn non_settable_field_fails_preflight_without_mutation() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"Frozen","pid":9,
                 "positionSettable":false}]"#,
        );
        refreshed();
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 5,
            y: 5,
        })
        .expect("plan");
        let before = super::super::test_support::mutation_count().expect("count");
        let receipt = execute_plan(&plan).expect("execute");
        assert_eq!(receipt.status, MutationStatus::Failed);
        assert!(receipt.operations[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("preflight")));
        assert_eq!(
            super::super::test_support::mutation_count().expect("count"),
            before,
            "preflight failure must not mutate"
        );
    }

    #[test]
    fn caller_timeout_cancels_a_slow_command_before_mutation() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"Slow","pid":9,
                 "mutation":{"delayMs":400}}]"#,
        );
        refreshed();
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 77,
            y: 88,
        })
        .expect("plan");
        let receipt =
            execute_plan_with_deadline(&plan, Duration::from_millis(80)).expect("execute");
        assert_eq!(receipt.status, MutationStatus::TimedOut);
        // Give the provider time to observe cancellation, then prove the
        // slow mutation never landed.
        std::thread::sleep(Duration::from_millis(600));
        let state = super::super::test_support::window_state(1).expect("state");
        assert_eq!(state.bounds.x, 0, "timed-out command must never mutate");
    }

    #[test]
    fn same_pid_serializes_and_different_pids_overlap() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"S1","pid":9,"mutation":{"delayMs":120}},
                {"id":2,"app":"A","title":"S2","pid":9,"mutation":{"delayMs":120}},
                {"id":3,"app":"B","title":"P1","pid":10,"mutation":{"delayMs":120}}]"#,
        );
        refreshed();

        let plan_for = |window_id: u32| {
            compile_legacy_window_action(LegacyWindowAction::Move {
                window_id,
                x: 5,
                y: 5,
            })
            .expect("plan")
        };

        // Same PID: two commands serialize on one worker.
        let started = Instant::now();
        let plan_a = plan_for(1);
        let plan_b = plan_for(2);
        let handle_a = std::thread::spawn(move || execute_plan(&plan_a).expect("a"));
        let handle_b = std::thread::spawn(move || execute_plan(&plan_b).expect("b"));
        let receipt_a = handle_a.join().expect("join");
        let receipt_b = handle_b.join().expect("join");
        let same_pid_elapsed = started.elapsed();
        assert_eq!(receipt_a.status, MutationStatus::Succeeded);
        assert_eq!(receipt_b.status, MutationStatus::Succeeded);
        assert!(
            same_pid_elapsed >= Duration::from_millis(230),
            "same-PID work must serialize (elapsed {same_pid_elapsed:?})"
        );

        // Different PIDs: run concurrently, well under 2x the delay.
        let started = Instant::now();
        let plan_c = plan_for(1);
        let plan_d = plan_for(3);
        let handle_c = std::thread::spawn(move || execute_plan(&plan_c).expect("c"));
        let handle_d = std::thread::spawn(move || execute_plan(&plan_d).expect("d"));
        handle_c.join().expect("join");
        handle_d.join().expect("join");
        let cross_pid_elapsed = started.elapsed();
        assert!(
            cross_pid_elapsed < Duration::from_millis(230),
            "cross-PID work must overlap (elapsed {cross_pid_elapsed:?})"
        );
    }

    #[test]
    fn diagnostics_record_the_transaction() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(r#"[{"id":1,"app":"A","title":"Doc","pid":9}]"#);
        refreshed();
        super::super::diagnostics::clear_diagnostics_for_tests();
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 15,
            y: 25,
        })
        .expect("plan");
        execute_plan(&plan).expect("execute");
        let snapshot = super::super::diagnostics::window_operation_diagnostics_snapshot();
        let entry = snapshot
            .iter()
            .find(|entry| entry.plan_id.as_deref() == Some(plan.plan_id.as_str()))
            .expect("diagnostics entry");
        assert!(entry.readback_count >= 1);
        assert!(entry.attempts >= 1);
        super::super::diagnostics::clear_diagnostics_for_tests();
    }

    #[test]
    fn no_successful_receipt_carries_a_failed_verification() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"Offset","pid":9,
                 "mutation":{"positionDeltaX":30,"positionDeltaY":30}}]"#,
        );
        refreshed();
        let plan = compile_legacy_window_action(LegacyWindowAction::Move {
            window_id: 1,
            x: 100,
            y: 100,
        })
        .expect("plan");
        let receipt = execute_plan(&plan).expect("execute");
        // 30-point offset exceeds the 2-point tolerance: must not be success.
        assert_ne!(receipt.status, MutationStatus::Succeeded);
        for operation in &receipt.operations {
            if operation.status == MutationStatus::Succeeded {
                assert!(operation.attempts.iter().any(|attempt| attempt.verified));
            }
        }
    }

    #[test]
    fn one_hundred_rapid_alternating_placements_hit_only_their_targets() {
        let _lock = registry::REGISTRY_TEST_LOCK.lock();
        let _env = fixture(
            r#"[{"id":1,"app":"A","title":"Left","pid":9,
                 "bounds":{"x":0,"y":0,"width":800,"height":600}},
                {"id":2,"app":"B","title":"Right","pid":10,
                 "bounds":{"x":900,"y":0,"width":800,"height":600}}]"#,
        );
        refreshed();
        for cycle in 0..100u32 {
            let (target, x) = if cycle % 2 == 0 { (1, 10) } else { (2, 910) };
            let plan = compile_legacy_window_action(LegacyWindowAction::Move {
                window_id: target,
                x: x + (cycle as i32 % 7),
                y: 20,
            })
            .expect("plan");
            let receipt = execute_plan(&plan).expect("execute");
            assert_eq!(
                receipt.status,
                MutationStatus::Succeeded,
                "cycle {cycle} failed"
            );
        }
        // Zero wrong-window mutations: each window only ever landed in its
        // own lane.
        let one = super::super::test_support::window_state(1).expect("state");
        let two = super::super::test_support::window_state(2).expect("state");
        assert!(
            (10..=16).contains(&one.bounds.x),
            "window 1 stayed in its lane"
        );
        assert!(
            (910..=916).contains(&two.bounds.x),
            "window 2 stayed in its lane"
        );
        super::super::undo::clear_window_undo_history();
    }
}
