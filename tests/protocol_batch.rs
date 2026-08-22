//! Integration tests for the `batch` / `batchResult` protocol messages.
//!
//! Validates wire-level compatibility: parsing batch requests with multiple
//! commands, stop-on-error behavior, and `selectByValue` against choices.

use script_kit_gpui::protocol::Message;

// ---------- batch request parsing ----------

#[test]
fn batch_full_transaction_parses_from_raw_json() {
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-1",
        "commands": [
            {"type": "setInput", "text": "apple"},
            {"type": "waitFor", "condition": "choicesRendered", "timeout": 1000},
            {"type": "selectByValue", "value": "apple", "submit": true}
        ]
    });
    let msg: Message = serde_json::from_value(raw).expect("should parse batch");
    let reserialized = serde_json::to_value(&msg).expect("should reserialize");

    assert_eq!(reserialized["type"], "batch");
    assert_eq!(reserialized["requestId"], "txn-1");

    let commands = reserialized["commands"].as_array().expect("commands array");
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0]["type"], "setInput");
    assert_eq!(commands[0]["text"], "apple");
    assert_eq!(commands[1]["type"], "waitFor");
    assert_eq!(commands[1]["condition"], "choicesRendered");
    assert_eq!(commands[2]["type"], "selectByValue");
    assert_eq!(commands[2]["value"], "apple");
    assert_eq!(commands[2]["submit"], true);
}

#[test]
fn batch_with_options_parses() {
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-opts",
        "commands": [
            {"type": "typeAndSubmit", "text": "hello"}
        ],
        "options": {
            "stopOnError": false,
            "timeout": 10000
        }
    });
    let msg: Message = serde_json::from_value(raw).expect("should parse batch with options");
    let reserialized = serde_json::to_value(&msg).expect("should reserialize");

    assert_eq!(reserialized["options"]["stopOnError"], false);
    assert_eq!(reserialized["options"]["timeout"], 10000);
}

#[test]
fn batch_without_options_omits_field() {
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-no-opts",
        "commands": [
            {"type": "setInput", "text": "test"}
        ]
    });
    let msg: Message = serde_json::from_value(raw).expect("should parse");
    let reserialized = serde_json::to_value(&msg).expect("should reserialize");

    assert!(
        reserialized.get("options").is_none(),
        "options should be omitted when absent"
    );
}

// ---------- batchResult response shapes ----------

#[test]
fn batch_result_success_shape_with_apple_selection() {
    let msg = Message::batch_result(
        "txn-1".to_string(),
        true,
        vec![
            script_kit_gpui::protocol::BatchResultEntry {
                index: 0,
                success: true,
                command: "setInput".to_string(),
                elapsed: Some(1),
                value: None,
                error: None,
            },
            script_kit_gpui::protocol::BatchResultEntry {
                index: 1,
                success: true,
                command: "waitFor".to_string(),
                elapsed: Some(17),
                value: None,
                error: None,
            },
            script_kit_gpui::protocol::BatchResultEntry {
                index: 2,
                success: true,
                command: "selectByValue".to_string(),
                elapsed: Some(2),
                value: Some("apple".to_string()),
                error: None,
            },
        ],
        None,
        24,
    );
    let json = serde_json::to_value(&msg).expect("serialize batchResult");

    assert_eq!(json["type"], "batchResult");
    assert_eq!(json["requestId"], "txn-1");
    assert_eq!(json["success"], true);
    assert_eq!(json["totalElapsed"], 24);
    assert!(
        json.get("failedAt").is_none(),
        "failedAt should be absent on full success"
    );

    let results = json["results"].as_array().expect("results array");
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["command"], "setInput");
    assert_eq!(results[1]["command"], "waitFor");
    assert_eq!(results[1]["elapsed"], 17);
    assert_eq!(results[2]["command"], "selectByValue");
    assert_eq!(results[2]["value"], "apple");
}

// ---------- stop-on-error behavior ----------

#[test]
fn batch_result_stop_on_error_sets_failed_at() {
    let msg = Message::batch_result(
        "txn-fail".to_string(),
        false,
        vec![
            script_kit_gpui::protocol::BatchResultEntry {
                index: 0,
                success: true,
                command: "setInput".to_string(),
                elapsed: Some(1),
                value: None,
                error: None,
            },
            script_kit_gpui::protocol::BatchResultEntry {
                index: 1,
                success: false,
                command: "selectByValue".to_string(),
                elapsed: Some(3),
                value: None,
                error: Some(
                    script_kit_gpui::protocol::TransactionError::selection_not_found(
                        "No visible choice matched value 'grape'",
                    ),
                ),
            },
        ],
        Some(1),
        4,
    );
    let json = serde_json::to_value(&msg).expect("serialize failed batch");

    assert_eq!(json["type"], "batchResult");
    assert_eq!(json["success"], false);
    assert_eq!(json["failedAt"], 1);
    assert_eq!(json["totalElapsed"], 4);

    let results = json["results"].as_array().expect("results array");
    assert_eq!(
        results.len(),
        2,
        "stop-on-error should include results up to and including failure"
    );
    assert_eq!(results[0]["success"], true);
    assert_eq!(results[1]["success"], false);
    assert_eq!(results[1]["error"]["code"], "selection_not_found");
    assert_eq!(
        results[1]["error"]["message"],
        "No visible choice matched value 'grape'"
    );
}

#[test]
fn batch_result_stop_on_error_preserves_successful_prefix() {
    // When stop_on_error is true and command 2 fails, commands 0 and 1 should
    // still appear in results with success: true.
    let msg = Message::batch_result(
        "txn-prefix".to_string(),
        false,
        vec![
            script_kit_gpui::protocol::BatchResultEntry {
                index: 0,
                success: true,
                command: "setInput".to_string(),
                elapsed: Some(1),
                value: None,
                error: None,
            },
            script_kit_gpui::protocol::BatchResultEntry {
                index: 1,
                success: true,
                command: "waitFor".to_string(),
                elapsed: Some(50),
                value: None,
                error: None,
            },
            script_kit_gpui::protocol::BatchResultEntry {
                index: 2,
                success: false,
                command: "selectByValue".to_string(),
                elapsed: Some(2),
                value: None,
                error: Some(
                    script_kit_gpui::protocol::TransactionError::selection_not_found(
                        "No visible choice matched value 'mango'",
                    ),
                ),
            },
        ],
        Some(2),
        53,
    );
    let json = serde_json::to_value(&msg).expect("serialize");

    assert_eq!(json["failedAt"], 2);
    let results = json["results"].as_array().expect("results");
    assert_eq!(results[0]["success"], true);
    assert_eq!(results[1]["success"], true);
    assert_eq!(results[2]["success"], false);
}

// ---------- batchResult round-trip ----------

#[test]
fn batch_result_round_trips_through_serde() {
    let msg = Message::batch_result(
        "txn-rt".to_string(),
        true,
        vec![script_kit_gpui::protocol::BatchResultEntry {
            index: 0,
            success: true,
            command: "setInput".to_string(),
            elapsed: Some(1),
            value: None,
            error: None,
        }],
        None,
        1,
    );
    let serialized = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&serialized).expect("deserialize");
    let re = serde_json::to_value(&back).expect("re-serialize");

    assert_eq!(re["type"], "batchResult");
    assert_eq!(re["success"], true);
    assert_eq!(re["totalElapsed"], 1);
}

// ---------- selectByValue in batch context ----------

#[test]
fn batch_select_by_value_against_filtered_choices_shape() {
    // Verifies the wire shape for a transaction that filters, waits, and selects
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-filter-select",
        "commands": [
            {"type": "setInput", "text": "app"},
            {"type": "waitFor", "condition": "choicesRendered", "timeout": 2000},
            {"type": "selectByValue", "value": "apple", "submit": true}
        ],
        "options": {"stopOnError": true, "timeout": 5000}
    });

    let msg: Message = serde_json::from_value(raw).expect("parse filter+select batch");
    let reserialized = serde_json::to_value(&msg).expect("reserialize");

    // Verify the full transaction structure
    let cmds = reserialized["commands"].as_array().expect("commands");
    assert_eq!(cmds.len(), 3);

    // Step 1: set filter text
    assert_eq!(cmds[0]["type"], "setInput");
    assert_eq!(cmds[0]["text"], "app");

    // Step 2: wait for choices to appear
    assert_eq!(cmds[1]["type"], "waitFor");
    assert_eq!(cmds[1]["condition"], "choicesRendered");

    // Step 3: select the matching choice
    assert_eq!(cmds[2]["type"], "selectByValue");
    assert_eq!(cmds[2]["value"], "apple");
    assert_eq!(cmds[2]["submit"], true);

    // Options
    assert_eq!(reserialized["options"]["stopOnError"], true);
}

// ---------- filterAndSelect batch command ----------

#[test]
fn batch_filter_and_select_round_trips() {
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-fas",
        "commands": [
            {"type": "filterAndSelect", "filter": "app", "selectFirst": true, "submit": true}
        ]
    });

    let msg: Message = serde_json::from_value(raw).expect("parse filterAndSelect batch");
    let reserialized = serde_json::to_value(&msg).expect("reserialize");

    let cmd = &reserialized["commands"][0];
    assert_eq!(cmd["type"], "filterAndSelect");
    assert_eq!(cmd["filter"], "app");
    assert_eq!(cmd["selectFirst"], true);
    assert_eq!(cmd["submit"], true);
}

// ---------- batch with trace mode ----------

#[test]
fn batch_with_trace_mode_parses_and_preserves() {
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-trace",
        "commands": [
            {"type": "setInput", "text": "apple"},
            {"type": "waitFor", "condition": "choicesRendered"}
        ],
        "trace": "onFailure"
    });
    let msg: Message = serde_json::from_value(raw).expect("parse batch with trace");
    let reserialized = serde_json::to_value(&msg).expect("reserialize");

    assert_eq!(reserialized["trace"], "onFailure");
    assert_eq!(reserialized["commands"].as_array().expect("cmds").len(), 2);
}

#[test]
fn batch_without_trace_omits_field() {
    let raw = serde_json::json!({
        "type": "batch",
        "requestId": "txn-no-trace",
        "commands": [
            {"type": "setInput", "text": "test"}
        ]
    });
    let msg: Message = serde_json::from_value(raw).expect("parse");
    let reserialized = serde_json::to_value(&msg).expect("reserialize");

    assert!(
        reserialized.get("trace").is_none(),
        "trace field should be omitted when off (default)"
    );
}

// ---------- batchResult with trace receipt ----------

#[test]
fn batch_result_failure_with_trace_receipt() {
    use script_kit_gpui::protocol::{
        TransactionCommandTrace, TransactionError, TransactionErrorCode, TransactionTrace,
        TransactionTraceStatus, UiStateSnapshot,
    };

    let trace = TransactionTrace {
        schema_version: 1,
        request_id: "txn-fail-trace".to_string(),
        command_fingerprint: "fp-txn-fail-trace".to_string(),
        status: TransactionTraceStatus::Failed,
        started_at_ms: 100,
        total_elapsed_ms: 1003,
        failed_at: Some(1),
        commands: vec![
            TransactionCommandTrace {
                index: 0,
                command: "setInput".to_string(),
                command_payload: None,
                started_at_ms: 100,
                elapsed_ms: 2,
                before: UiStateSnapshot::default(),
                after: UiStateSnapshot {
                    window_visible: true,
                    window_focused: true,
                    input_value: Some("apple".to_string()),
                    ..Default::default()
                },
                polls: vec![],
                error: None,
            },
            TransactionCommandTrace {
                index: 1,
                command: "waitFor".to_string(),
                command_payload: None,
                started_at_ms: 102,
                elapsed_ms: 1001,
                before: UiStateSnapshot {
                    window_visible: true,
                    window_focused: true,
                    input_value: Some("apple".to_string()),
                    ..Default::default()
                },
                after: UiStateSnapshot {
                    window_visible: true,
                    window_focused: true,
                    input_value: Some("apple".to_string()),
                    choice_count: 0,
                    ..Default::default()
                },
                polls: vec![],
                error: Some(TransactionError {
                    code: TransactionErrorCode::WaitConditionTimeout,
                    message: "Timeout after 1000ms waiting for choicesRendered".to_string(),
                    suggestion: Some("No choices were visible at timeout.".to_string()),
                }),
            },
        ],
    };

    let msg = Message::batch_result_with_trace(
        "txn-fail-trace".to_string(),
        false,
        vec![
            script_kit_gpui::protocol::BatchResultEntry {
                index: 0,
                success: true,
                command: "setInput".to_string(),
                elapsed: Some(2),
                value: None,
                error: None,
            },
            script_kit_gpui::protocol::BatchResultEntry {
                index: 1,
                success: false,
                command: "waitFor".to_string(),
                elapsed: Some(1001),
                value: None,
                error: Some(TransactionError {
                    code: TransactionErrorCode::WaitConditionTimeout,
                    message: "Timeout after 1000ms waiting for choicesRendered".to_string(),
                    suggestion: Some("No choices were visible at timeout.".to_string()),
                }),
            },
        ],
        Some(1),
        1003,
        Some(trace),
    );
    let json = serde_json::to_value(&msg).expect("serialize batch with trace");

    assert_eq!(json["type"], "batchResult");
    assert_eq!(json["success"], false);
    assert_eq!(json["failedAt"], 1);
    assert_eq!(json["totalElapsed"], 1003);

    // Per-command structured errors
    let results = json["results"].as_array().expect("results");
    assert_eq!(results[0]["success"], true);
    assert!(results[0].get("error").is_none());
    assert_eq!(results[1]["success"], false);
    assert_eq!(results[1]["error"]["code"], "wait_condition_timeout");
    assert!(results[1]["error"]["suggestion"].is_string());

    // Trace receipt
    let trace_json = &json["trace"];
    assert_eq!(trace_json["requestId"], "txn-fail-trace");
    assert_eq!(trace_json["status"], "failed");
    assert_eq!(trace_json["failedAt"], 1);
    assert_eq!(trace_json["commands"].as_array().expect("cmds").len(), 2);
    assert_eq!(trace_json["commands"][0]["command"], "setInput");
    assert_eq!(trace_json["commands"][1]["command"], "waitFor");
    assert!(trace_json["commands"][1]["error"]["suggestion"].is_string());

    // Round-trip
    let back: Message = serde_json::from_value(json).expect("round-trip batch with trace");
    let re = serde_json::to_value(&back).expect("re-serialize");
    assert_eq!(re["trace"]["status"], "failed");
}

#[test]
fn batch_result_success_with_trace_omits_when_absent() {
    let msg = Message::batch_result(
        "txn-ok".to_string(),
        true,
        vec![script_kit_gpui::protocol::BatchResultEntry {
            index: 0,
            success: true,
            command: "setInput".to_string(),
            elapsed: Some(1),
            value: None,
            error: None,
        }],
        None,
        1,
    );
    let json = serde_json::to_value(&msg).expect("serialize");

    assert!(
        json.get("trace").is_none(),
        "trace should be omitted when not provided"
    );
}

#[test]
fn batch_trace_helper_preserves_shape_without_exposing_private_payloads() {
    use script_kit_gpui::protocol::transaction_trace::build_batch_trace_from_results;
    use script_kit_gpui::protocol::{
        BatchCommand, BatchResultEntry, TransactionError, TransactionErrorCode,
        TransactionTraceStatus, WaitCondition, WaitNamedCondition,
    };

    let commands = vec![
        BatchCommand::SetInput {
            text: "apple".to_string(),
        },
        BatchCommand::WaitFor {
            condition: WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
            timeout: Some(1000),
            poll_interval: Some(25),
        },
    ];
    let timeout_error = TransactionError {
        code: TransactionErrorCode::WaitConditionTimeout,
        message: "timeout".to_string(),
        suggestion: None,
    };
    let results = vec![
        BatchResultEntry {
            index: 0,
            success: true,
            command: "setInput".to_string(),
            elapsed: Some(2),
            value: None,
            error: None,
        },
        BatchResultEntry {
            index: 1,
            success: false,
            command: "waitFor".to_string(),
            elapsed: Some(1000),
            value: None,
            error: Some(timeout_error.clone()),
        },
    ];

    let trace = build_batch_trace_from_results(
        "txn-shared-trace".to_string(),
        "fp-shared-trace".to_string(),
        12345,
        1002,
        false,
        Some(1),
        &commands,
        &results,
    );

    assert_eq!(trace.status, TransactionTraceStatus::Failed);
    assert_eq!(trace.started_at_ms, 12345);
    assert_eq!(trace.total_elapsed_ms, 1002);
    assert_eq!(trace.failed_at, Some(1));
    assert_eq!(trace.commands.len(), 2);
    let Some(BatchCommand::SetInput { text }) = &trace.commands[0].command_payload else {
        panic!("setInput trace must preserve its command shape");
    };
    assert!(text.starts_with("[REDACTED sha256:"));
    assert!(!text.contains("apple"));
    assert_eq!(trace.commands[1].command_payload, Some(commands[1].clone()));
    let error = trace.commands[1]
        .error
        .as_ref()
        .expect("failure must preserve its typed error");
    assert_eq!(error.code, timeout_error.code);
    assert_eq!(
        error.message,
        "The requested UI condition was not met before the timeout."
    );
    assert!(trace.command_fingerprint.starts_with("sha256:"));
}

#[test]
fn prompt_handler_batch_trace_finalization_is_shared_and_nonzero() {
    let source = include_str!("../src/prompt_handler/mod.rs");

    assert!(
        source.contains("maybe_persist_batch_trace_from_results"),
        "batch paths must finalize traces through the shared helper"
    );
    assert!(
        !source.contains("let started_at_ms = 0u64"),
        "batch trace receipts must not use a zero started_at_ms sentinel"
    );
    assert!(
        !source.contains("commands: results.iter().map(|r|"),
        "prompt_handler must not hand-roll batch trace command mapping"
    );
    assert!(
        !source.contains("append_transaction_trace(None, trace)"),
        "prompt_handler must not persist traces through a target-specific side path"
    );
}

#[test]
fn included_batch_trace_persistence_error_is_returned_to_caller() {
    use script_kit_gpui::protocol::transaction_trace::maybe_persist_batch_trace_from_results;
    use script_kit_gpui::protocol::{BatchCommand, BatchResultEntry, TransactionTraceMode};

    let temp = tempfile::tempdir().expect("tempdir");
    let bad_log_path = temp.path();
    let commands = vec![BatchCommand::SetInput {
        text: "x".to_string(),
    }];
    let results = vec![BatchResultEntry {
        index: 0,
        success: true,
        command: "setInput".to_string(),
        elapsed: Some(1),
        value: None,
        error: None,
    }];

    let err = maybe_persist_batch_trace_from_results(
        TransactionTraceMode::On,
        "txn-persist-fails".to_string(),
        "fp".to_string(),
        123,
        1,
        true,
        None,
        &commands,
        &results,
        Some(bad_log_path),
    )
    .expect_err("trace persistence should fail closed");
    let message = err.to_string();
    assert!(
        message.contains("failed to open") || message.contains("Is a directory"),
        "unexpected persistence error: {message}"
    );
}
