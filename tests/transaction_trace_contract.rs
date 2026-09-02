//! Contract tests for the transaction flight recorder executor.
//!
//! Validates that `waitFor` and `batch` execution produces deterministic
//! per-command receipts with before/after snapshots, poll observations,
//! elapsed timings, and actionable failure suggestions.

use anyhow::Result;
use script_kit_gpui::protocol::transaction_executor::{
    execute_batch, execute_wait_for, TransactionStateProvider,
};
use script_kit_gpui::protocol::transaction_trace::{
    append_transaction_trace, read_latest_transaction_trace,
};
use script_kit_gpui::protocol::{
    BatchCommand, TransactionErrorCode, TransactionTrace, TransactionTraceMode,
    TransactionTraceStatus, UiStateSnapshot, WaitCondition, WaitNamedCondition,
};

// ── Fake provider ──────────────────────────────────────────────────────────

/// A deterministic in-memory provider for testing.
#[derive(Debug, Clone, Default)]
struct FakeProvider {
    snapshot: UiStateSnapshot,
}

impl TransactionStateProvider for FakeProvider {
    fn snapshot(&self) -> UiStateSnapshot {
        self.snapshot.clone()
    }

    fn set_input(&mut self, text: &str) -> Result<()> {
        self.snapshot.window_visible = true;
        self.snapshot.window_focused = true;
        self.snapshot.input_value = Some(text.to_string());
        self.snapshot.focused_semantic_id = Some("input:filter".to_string());
        self.snapshot.visible_semantic_ids = vec![
            "window:main".to_string(),
            "input:filter".to_string(),
            "list:choices".to_string(),
        ];

        // Simulate: "apple" produces one choice, everything else produces zero
        if text == "apple" {
            self.snapshot.choice_count = 1;
            self.snapshot
                .visible_semantic_ids
                .push("choice:0:apple".to_string());
        } else {
            self.snapshot.choice_count = 0;
        }

        Ok(())
    }

    fn select_by_value(&mut self, value: &str, submit: bool) -> Result<Option<String>> {
        let semantic_id = format!("choice:0:{value}");
        if !self
            .snapshot
            .visible_semantic_ids
            .iter()
            .any(|id| id == &semantic_id)
        {
            return Ok(None);
        }

        self.snapshot.selected_value = Some(value.to_string());
        self.snapshot.focused_semantic_id = Some(semantic_id);
        if submit {
            self.snapshot.input_value = Some(value.to_string());
        }

        Ok(Some(value.to_string()))
    }

    fn select_by_semantic_id(&mut self, semantic_id: &str, submit: bool) -> Result<Option<String>> {
        if !self
            .snapshot
            .visible_semantic_ids
            .iter()
            .any(|id| id == semantic_id)
        {
            return Ok(None);
        }

        let value = semantic_id
            .split(':')
            .last()
            .unwrap_or(semantic_id)
            .to_string();

        self.snapshot.selected_value = Some(value.clone());
        self.snapshot.focused_semantic_id = Some(semantic_id.to_string());
        if submit {
            self.snapshot.input_value = Some(value.clone());
        }

        Ok(Some(value))
    }
}

// ── waitFor tests ──────────────────────────────────────────────────────────

#[test]
fn wait_for_timeout_returns_actionable_error() {
    let mut provider = FakeProvider {
        snapshot: UiStateSnapshot {
            window_visible: true,
            window_focused: true,
            input_value: Some("apple".to_string()),
            focused_semantic_id: Some("input:filter".to_string()),
            visible_semantic_ids: vec![
                "window:main".to_string(),
                "input:filter".to_string(),
                "list:choices".to_string(),
            ],
            choice_count: 0,
            ..Default::default()
        },
    };

    let result = execute_wait_for(
        &mut provider,
        "wait-1".to_string(),
        &WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
        Some(30),
        Some(10),
        TransactionTraceMode::Off,
    )
    .expect("waitFor should complete without internal error");

    assert!(!result.success, "should have timed out");
    let error = result.error.expect("should have error");
    assert_eq!(error.code, TransactionErrorCode::WaitConditionTimeout);
    assert!(
        error
            .suggestion
            .as_deref()
            .expect("should have suggestion")
            .contains("No choices were visible"),
        "suggestion should explain the timeout cause"
    );
    assert!(
        result.trace.is_none(),
        "trace should be absent when mode is off"
    );
}

#[test]
fn wait_for_success_with_satisfied_condition() {
    let mut provider = FakeProvider {
        snapshot: UiStateSnapshot {
            window_visible: true,
            window_focused: true,
            choice_count: 3,
            ..Default::default()
        },
    };

    let result = execute_wait_for(
        &mut provider,
        "wait-ok".to_string(),
        &WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
        Some(100),
        Some(10),
        TransactionTraceMode::Off,
    )
    .expect("waitFor should complete");

    assert!(result.success, "condition was already satisfied");
    assert!(result.error.is_none());
}

#[test]
fn wait_for_on_failure_trace_included_on_timeout() {
    let mut provider = FakeProvider {
        snapshot: UiStateSnapshot {
            window_visible: true,
            window_focused: false,
            ..Default::default()
        },
    };

    let result = execute_wait_for(
        &mut provider,
        "wait-trace".to_string(),
        &WaitCondition::Named(WaitNamedCondition::WindowFocused),
        Some(30),
        Some(10),
        TransactionTraceMode::OnFailure,
    )
    .expect("waitFor should complete");

    assert!(!result.success);
    let trace = result
        .trace
        .expect("trace should be present on failure with onFailure mode");
    assert_eq!(trace.status, TransactionTraceStatus::Timeout);
    assert_eq!(trace.failed_at, Some(0));
    assert!(!trace.commands.is_empty());
    assert!(
        !trace.commands[0].polls.is_empty(),
        "should have poll observations"
    );
}

#[test]
fn wait_for_on_failure_trace_absent_on_success() {
    let mut provider = FakeProvider {
        snapshot: UiStateSnapshot {
            window_visible: true,
            ..Default::default()
        },
    };

    let result = execute_wait_for(
        &mut provider,
        "wait-ok-no-trace".to_string(),
        &WaitCondition::Named(WaitNamedCondition::WindowVisible),
        Some(100),
        Some(10),
        TransactionTraceMode::OnFailure,
    )
    .expect("waitFor should complete");

    assert!(result.success);
    assert!(
        result.trace.is_none(),
        "trace should be absent on success with onFailure mode"
    );
}

// ── batch tests ────────────────────────────────────────────────────────────

#[test]
fn batch_success_returns_selected_value() {
    let mut provider = FakeProvider::default();

    let commands = vec![
        BatchCommand::SetInput {
            text: "apple".to_string(),
        },
        BatchCommand::WaitFor {
            condition: WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
            timeout: Some(50),
            poll_interval: Some(10),
        },
        BatchCommand::SelectByValue {
            value: "apple".to_string(),
            submit: true,
        },
    ];

    let result = execute_batch(
        &mut provider,
        "txn-1".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("batch should complete");

    assert!(result.success, "all commands should succeed");
    assert_eq!(result.results.len(), 3);
    assert_eq!(result.results[0].command, "setInput");
    assert!(result.results[0].success);
    assert_eq!(result.results[1].command, "waitFor");
    assert!(result.results[1].success);
    assert_eq!(result.results[2].command, "selectByValue");
    assert!(result.results[2].success);
    assert_eq!(
        result.results[2].value.as_deref(),
        Some("apple"),
        "selectByValue should return matched value"
    );
    assert!(result.failed_at.is_none());
}

#[test]
fn batch_stop_on_error_halts_at_failure() {
    let mut provider = FakeProvider::default();

    // setInput "banana" → 0 choices → waitFor choicesRendered times out
    let commands = vec![
        BatchCommand::SetInput {
            text: "banana".to_string(),
        },
        BatchCommand::WaitFor {
            condition: WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
            timeout: Some(30),
            poll_interval: Some(10),
        },
        BatchCommand::SelectByValue {
            value: "banana".to_string(),
            submit: true,
        },
    ];

    let result = execute_batch(
        &mut provider,
        "txn-fail".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("batch should complete");

    assert!(!result.success);
    assert_eq!(result.failed_at, Some(1), "should fail at waitFor");
    assert_eq!(
        result.results.len(),
        2,
        "stop_on_error should halt, only 2 results"
    );
    assert!(result.results[0].success);
    assert!(!result.results[1].success);
    assert_eq!(
        result.results[1].error.as_ref().expect("error").code,
        TransactionErrorCode::WaitConditionTimeout
    );
}

#[test]
fn batch_selection_not_found_error() {
    let mut provider = FakeProvider::default();

    // setInput "apple" → 1 choice ("apple") → try to select "grape" → not found
    let commands = vec![
        BatchCommand::SetInput {
            text: "apple".to_string(),
        },
        BatchCommand::SelectByValue {
            value: "grape".to_string(),
            submit: true,
        },
    ];

    let result = execute_batch(
        &mut provider,
        "txn-sel-fail".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("batch should complete");

    assert!(!result.success);
    assert_eq!(result.failed_at, Some(1));
    let error = result.results[1].error.as_ref().expect("error");
    assert_eq!(error.code, TransactionErrorCode::SelectionNotFound);
    assert!(
        error.message.contains("grape"),
        "error should mention the missing value"
    );
}

#[test]
fn batch_with_trace_on_produces_trace() {
    let mut provider = FakeProvider::default();

    let commands = vec![BatchCommand::SetInput {
        text: "apple".to_string(),
    }];

    let result = execute_batch(
        &mut provider,
        "txn-traced".to_string(),
        &commands,
        None,
        TransactionTraceMode::On,
    )
    .expect("batch should complete");

    assert!(result.success);
    let trace = result
        .trace
        .expect("trace should be present when mode is On");
    assert_eq!(trace.status, TransactionTraceStatus::Ok);
    assert_eq!(trace.commands.len(), 1);
    assert_eq!(trace.commands[0].command, "setInput");
    // Verify before/after snapshots capture state change
    assert!(
        trace.commands[0].after.window_visible,
        "after snapshot should reflect state after setInput"
    );
}

// ── JSONL persistence tests ────────────────────────────────────────────────

#[test]
fn trace_log_round_trips_through_jsonl() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("transactions.jsonl");

    let trace = TransactionTrace {
        schema_version: 1,
        request_id: "txn-rt".to_string(),
        command_fingerprint: "fp-rt".to_string(),
        status: TransactionTraceStatus::Ok,
        started_at_ms: 1000,
        total_elapsed_ms: 42,
        failed_at: None,
        commands: Vec::new(),
    };

    let written_path = append_transaction_trace(Some(&path), &trace).expect("should append trace");
    assert_eq!(written_path, path);

    let loaded = read_latest_transaction_trace(Some(&path), Some("txn-rt"))
        .expect("should read trace")
        .expect("trace should exist");

    assert_eq!(loaded.request_id, "txn-rt");
    assert_eq!(loaded.status, TransactionTraceStatus::Ok);
    assert_eq!(loaded.total_elapsed_ms, 42);
}

#[test]
fn trace_log_returns_latest_when_multiple_entries() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("transactions.jsonl");

    for i in 0..3 {
        let trace = TransactionTrace {
            schema_version: 1,
            request_id: format!("txn-{i}"),
            command_fingerprint: format!("fp-{i}"),
            status: TransactionTraceStatus::Ok,
            started_at_ms: i * 100,
            total_elapsed_ms: i * 10,
            failed_at: None,
            commands: Vec::new(),
        };
        append_transaction_trace(Some(&path), &trace).expect("append");
    }

    // Without filter → returns the latest entry
    let latest = read_latest_transaction_trace(Some(&path), None)
        .expect("read")
        .expect("should find entry");
    assert_eq!(latest.request_id, "txn-2");

    // With filter → returns specific entry
    let specific = read_latest_transaction_trace(Some(&path), Some("txn-0"))
        .expect("read")
        .expect("should find entry");
    assert_eq!(specific.request_id, "txn-0");
}

#[test]
fn trace_log_returns_none_for_nonexistent_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("does_not_exist.jsonl");

    let result = read_latest_transaction_trace(Some(&path), None).expect("should not error");
    assert!(result.is_none());
}

#[test]
fn trace_log_returns_none_for_missing_request_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("transactions.jsonl");

    let trace = TransactionTrace {
        schema_version: 1,
        request_id: "txn-exists".to_string(),
        command_fingerprint: "fp-exists".to_string(),
        status: TransactionTraceStatus::Ok,
        started_at_ms: 0,
        total_elapsed_ms: 0,
        failed_at: None,
        commands: Vec::new(),
    };
    append_transaction_trace(Some(&path), &trace).expect("append");

    let result =
        read_latest_transaction_trace(Some(&path), Some("txn-missing")).expect("should not error");
    assert!(result.is_none());
}

// ── selectBySemanticId tests ──────────────────────────────────────────────

#[test]
fn batch_select_by_semantic_id_success() {
    let mut provider = FakeProvider::default();

    let commands = vec![
        BatchCommand::SetInput {
            text: "apple".to_string(),
        },
        BatchCommand::WaitFor {
            condition: WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
            timeout: Some(50),
            poll_interval: Some(10),
        },
        BatchCommand::SelectBySemanticId {
            semantic_id: "choice:0:apple".to_string(),
            submit: true,
        },
    ];

    let result = execute_batch(
        &mut provider,
        "txn-sem-ok".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("batch should complete");

    assert!(result.success, "all commands should succeed");
    assert_eq!(result.results.len(), 3);
    assert_eq!(result.results[2].command, "selectBySemanticId");
    assert!(result.results[2].success);
    assert_eq!(
        result.results[2].value.as_deref(),
        Some("apple"),
        "selectBySemanticId should return extracted value"
    );
    assert!(result.failed_at.is_none());
}

#[test]
fn batch_select_by_semantic_id_not_found() {
    let mut provider = FakeProvider::default();

    let commands = vec![
        BatchCommand::SetInput {
            text: "apple".to_string(),
        },
        BatchCommand::SelectBySemanticId {
            semantic_id: "choice:0:grape".to_string(),
            submit: false,
        },
    ];

    let result = execute_batch(
        &mut provider,
        "txn-sem-miss".to_string(),
        &commands,
        None,
        TransactionTraceMode::Off,
    )
    .expect("batch should complete");

    assert!(!result.success);
    assert_eq!(result.failed_at, Some(1));
    let error = result.results[1].error.as_ref().expect("error");
    assert_eq!(error.code, TransactionErrorCode::SelectionNotFound);
    assert!(
        error.message.contains("choice:0:grape"),
        "error should mention the missing semantic ID"
    );
}

#[test]
fn batch_wait_uses_remaining_deadline_not_its_own_long_poll_interval() {
    let mut provider = FakeProvider::default();
    let options = script_kit_gpui::protocol::BatchOptions {
        stop_on_error: true,
        rollback_on_error: false,
        timeout: 10,
    };
    let result = execute_batch(
        &mut provider,
        "bounded-batch-wait".into(),
        &[
            BatchCommand::WaitFor {
                condition: WaitCondition::Named(WaitNamedCondition::WindowVisible),
                timeout: Some(10_000),
                poll_interval: Some(500),
            },
            BatchCommand::SetInput {
                text: "must not execute".into(),
            },
        ],
        Some(&options),
        TransactionTraceMode::Off,
    )
    .unwrap();
    assert!(!result.success);
    assert_eq!(result.failed_at, Some(0));
    assert_eq!(result.results.len(), 1);
    assert!(
        result.total_elapsed < 400,
        "one poll must not escape the batch deadline"
    );
    assert_eq!(provider.snapshot.input_value, None);
}

#[test]
fn invalid_batch_budget_and_rollback_refuse_before_any_provider_mutation() {
    let mut provider = FakeProvider::default();
    let commands = [BatchCommand::SetInput {
        text: "must not execute".into(),
    }];
    for (timeout, rollback_on_error) in [(0, false), (5_000, true)] {
        let options = script_kit_gpui::protocol::BatchOptions {
            stop_on_error: true,
            rollback_on_error,
            timeout,
        };
        assert!(execute_batch(
            &mut provider,
            format!("invalid-budget-{timeout}"),
            &commands,
            Some(&options),
            TransactionTraceMode::Off
        )
        .is_err());
        assert_eq!(provider.snapshot.input_value, None);
    }
}
