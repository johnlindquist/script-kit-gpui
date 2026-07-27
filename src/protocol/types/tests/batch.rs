use super::*;

// ============================================================
// BatchCommand serde
// ============================================================

#[test]
fn batch_command_set_input_round_trips() {
    let cmd = BatchCommand::SetInput {
        text: "apple".to_string(),
    };
    let json = serde_json::to_value(&cmd).expect("serialize setInput");
    assert_eq!(json["type"], "setInput");
    assert_eq!(json["text"], "apple");

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize setInput");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_open_actions_round_trips() {
    let cmd = BatchCommand::OpenActions;
    let json = serde_json::to_value(&cmd).expect("serialize openActions");
    assert_eq!(json["type"], "openActions");

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize openActions");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_select_by_value_round_trips() {
    let cmd = BatchCommand::SelectByValue {
        value: "apple".to_string(),
        submit: true,
    };
    let json = serde_json::to_value(&cmd).expect("serialize selectByValue");
    assert_eq!(json["type"], "selectByValue");
    assert_eq!(json["value"], "apple");
    assert_eq!(json["submit"], true);

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize selectByValue");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_select_by_value_defaults_submit_false() {
    let json = serde_json::json!({"type": "selectByValue", "value": "banana"});
    let cmd: BatchCommand = serde_json::from_value(json).expect("deserialize");
    match cmd {
        BatchCommand::SelectByValue { value, submit } => {
            assert_eq!(value, "banana");
            assert!(!submit, "submit should default to false");
        }
        other => panic!("Expected SelectByValue, got: {:?}", other),
    }
}

#[test]
fn batch_command_select_by_semantic_id_round_trips() {
    let cmd = BatchCommand::SelectBySemanticId {
        semantic_id: "choice:0:apple".to_string(),
        submit: true,
    };
    let json = serde_json::to_value(&cmd).expect("serialize selectBySemanticId");
    assert_eq!(json["type"], "selectBySemanticId");
    assert_eq!(json["semanticId"], "choice:0:apple");
    assert_eq!(json["submit"], true);

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize selectBySemanticId");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_select_by_semantic_id_defaults_submit_false() {
    let json = serde_json::json!({"type": "selectBySemanticId", "semanticId": "choice:2:banana"});
    let cmd: BatchCommand = serde_json::from_value(json).expect("deserialize");
    match cmd {
        BatchCommand::SelectBySemanticId {
            semantic_id,
            submit,
        } => {
            assert_eq!(semantic_id, "choice:2:banana");
            assert!(!submit, "submit should default to false");
        }
        other => panic!("Expected SelectBySemanticId, got: {:?}", other),
    }
}

#[test]
fn batch_command_set_theme_control_round_trips() {
    let cmd = BatchCommand::SetThemeControl {
        control: "surface-opacity".to_string(),
        value: "0.5".to_string(),
    };
    let json = serde_json::to_value(&cmd).expect("serialize setThemeControl");
    assert_eq!(json["type"], "setThemeControl");
    assert_eq!(json["control"], "surface-opacity");
    assert_eq!(json["value"], "0.5");

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize setThemeControl");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_save_current_style_settings_round_trips() {
    let cmd = BatchCommand::SaveCurrentStyleSettings;
    let json = serde_json::to_value(&cmd).expect("serialize saveCurrentStyleSettings");
    assert_eq!(json["type"], "saveCurrentStyleSettings");

    let back: BatchCommand =
        serde_json::from_value(json).expect("deserialize saveCurrentStyleSettings");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_dev_style_history_controls_round_trip() {
    for (cmd, wire_type) in [
        (BatchCommand::UndoStyleChange, "undoStyleChange"),
        (BatchCommand::RedoStyleChange, "redoStyleChange"),
        (BatchCommand::ResetStyleControls, "resetStyleControls"),
    ] {
        let json = serde_json::to_value(&cmd).expect("serialize dev style history command");
        assert_eq!(json["type"], wire_type);
        let back: BatchCommand =
            serde_json::from_value(json).expect("deserialize dev style history command");
        assert_eq!(back, cmd);
    }
}

#[test]
fn batch_command_filter_and_select_round_trips() {
    let cmd = BatchCommand::FilterAndSelect {
        filter: "app".to_string(),
        select_first: true,
        submit: false,
    };
    let json = serde_json::to_value(&cmd).expect("serialize filterAndSelect");
    assert_eq!(json["type"], "filterAndSelect");
    assert_eq!(json["filter"], "app");
    assert_eq!(json["selectFirst"], true);
    assert_eq!(json["submit"], false);

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize filterAndSelect");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_type_and_submit_round_trips() {
    let cmd = BatchCommand::TypeAndSubmit {
        text: "hello world".to_string(),
    };
    let json = serde_json::to_value(&cmd).expect("serialize typeAndSubmit");
    assert_eq!(json["type"], "typeAndSubmit");
    assert_eq!(json["text"], "hello world");

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize typeAndSubmit");
    assert_eq!(back, cmd);
}

#[test]
fn batch_command_wait_for_round_trips() {
    let cmd = BatchCommand::WaitFor {
        condition: WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
        timeout: Some(2000),
        poll_interval: Some(50),
    };
    let json = serde_json::to_value(&cmd).expect("serialize waitFor in batch");
    assert_eq!(json["type"], "waitFor");
    assert_eq!(json["condition"], "choicesRendered");
    assert_eq!(json["timeout"], 2000);
    assert_eq!(json["pollInterval"], 50);

    let back: BatchCommand = serde_json::from_value(json).expect("deserialize waitFor in batch");
    assert_eq!(back, cmd);
}

// ============================================================
// BatchOptions serde
// ============================================================

#[test]
fn batch_options_defaults_applied() {
    let json = serde_json::json!({});
    let opts: BatchOptions = serde_json::from_value(json).expect("deserialize empty options");
    assert!(opts.stop_on_error, "stop_on_error should default to true");
    assert!(
        !opts.rollback_on_error,
        "rollback_on_error should default to false"
    );
    assert_eq!(opts.timeout, 5000, "timeout should default to 5000");
}

#[test]
fn batch_options_round_trips_custom_values() {
    let opts = BatchOptions {
        stop_on_error: false,
        rollback_on_error: false,
        timeout: 10_000,
    };
    let json = serde_json::to_value(&opts).expect("serialize");
    assert_eq!(json["stopOnError"], false);
    assert_eq!(json["timeout"], 10_000);

    let back: BatchOptions = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, opts);
}

// ============================================================
// BatchResultEntry serde
// ============================================================

#[test]
fn batch_result_entry_success_round_trips() {
    let entry = BatchResultEntry {
        index: 0,
        success: true,
        command: "setInput".to_string(),
        elapsed: Some(2),
        value: None,
        error: None,
    };
    let json = serde_json::to_value(&entry).expect("serialize result entry");
    assert_eq!(json["index"], 0);
    assert_eq!(json["success"], true);
    assert_eq!(json["command"], "setInput");
    assert_eq!(json["elapsed"], 2);
    assert!(
        json.get("value").is_none(),
        "value should be omitted when None"
    );
    assert!(
        json.get("error").is_none(),
        "error should be omitted when None"
    );

    let back: BatchResultEntry = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, entry);
}

#[test]
fn batch_result_entry_failure_includes_error() {
    let entry = BatchResultEntry {
        index: 2,
        success: false,
        command: "selectByValue".to_string(),
        elapsed: Some(5),
        value: None,
        error: Some(crate::protocol::TransactionError::selection_not_found(
            "No visible choice matched value 'grape'",
        )),
    };
    let json = serde_json::to_value(&entry).expect("serialize failed entry");
    assert_eq!(
        json["error"]["message"],
        "No visible choice matched value 'grape'"
    );
    assert_eq!(json["success"], false);
}

// ============================================================
// batch message — request parsing
// ============================================================

#[test]
fn batch_request_parses_full_transaction() {
    let json = serde_json::json!({
        "type": "batch",
        "requestId": "txn-1",
        "commands": [
            {"type": "setInput", "text": "apple"},
            {"type": "waitFor", "condition": "choicesRendered", "timeout": 1000},
            {"type": "selectByValue", "value": "apple", "submit": true}
        ]
    });
    let msg: crate::protocol::Message = serde_json::from_value(json).expect("parse batch");

    match msg {
        crate::protocol::Message::Batch {
            request_id,
            commands,
            options,
            ..
        } => {
            assert_eq!(request_id, "txn-1");
            assert_eq!(commands.len(), 3);
            assert!(options.is_none());

            // Verify command types
            assert!(matches!(&commands[0], BatchCommand::SetInput { text } if text == "apple"));
            assert!(matches!(
                &commands[1],
                BatchCommand::WaitFor {
                    condition: WaitCondition::Named(WaitNamedCondition::ChoicesRendered),
                    ..
                }
            ));
            assert!(
                matches!(&commands[2], BatchCommand::SelectByValue { value, submit: true } if value == "apple")
            );
        }
        other => panic!("Expected Batch, got: {:?}", other),
    }
}

#[test]
fn batch_request_parses_with_options() {
    let json = serde_json::json!({
        "type": "batch",
        "requestId": "txn-2",
        "commands": [
            {"type": "typeAndSubmit", "text": "hello"}
        ],
        "options": {
            "stopOnError": false,
            "timeout": 10000
        }
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse batch with options");

    match msg {
        crate::protocol::Message::Batch { options, .. } => {
            let opts = options.expect("options should be present");
            assert!(!opts.stop_on_error);
            assert_eq!(opts.timeout, 10_000);
        }
        other => panic!("Expected Batch, got: {:?}", other),
    }
}

// ============================================================
// batchResult message — response serialization
// ============================================================

#[test]
fn batch_result_success_serializes_correctly() {
    let msg = crate::protocol::Message::batch_result(
        "txn-1".to_string(),
        true,
        vec![
            BatchResultEntry {
                index: 0,
                success: true,
                command: "setInput".to_string(),
                elapsed: Some(1),
                value: None,
                error: None,
            },
            BatchResultEntry {
                index: 1,
                success: true,
                command: "waitFor".to_string(),
                elapsed: Some(17),
                value: None,
                error: None,
            },
            BatchResultEntry {
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
        "failedAt should be omitted on success"
    );

    let results = json["results"].as_array().expect("results array");
    assert_eq!(results.len(), 3);
    assert_eq!(results[2]["value"], "apple");
}

#[test]
fn batch_result_stop_on_error_serializes_failed_at() {
    let msg = crate::protocol::Message::batch_result(
        "txn-err".to_string(),
        false,
        vec![
            BatchResultEntry {
                index: 0,
                success: true,
                command: "setInput".to_string(),
                elapsed: Some(1),
                value: None,
                error: None,
            },
            BatchResultEntry {
                index: 1,
                success: false,
                command: "selectByValue".to_string(),
                elapsed: Some(3),
                value: None,
                error: Some(crate::protocol::TransactionError::selection_not_found(
                    "No visible choice matched value 'grape'",
                )),
            },
        ],
        Some(1),
        4,
    );
    let json = serde_json::to_value(&msg).expect("serialize batchResult with failure");

    assert_eq!(json["success"], false);
    assert_eq!(json["failedAt"], 1);
    assert_eq!(json["results"][1]["error"]["code"], "selection_not_found");
    assert_eq!(
        json["results"][1]["error"]["message"],
        "No visible choice matched value 'grape'"
    );
}

#[test]
fn batch_result_round_trips() {
    let msg = crate::protocol::Message::batch_result(
        "txn-rt".to_string(),
        true,
        vec![BatchResultEntry {
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
    let back: crate::protocol::Message = serde_json::from_str(&serialized).expect("deserialize");

    match back {
        crate::protocol::Message::BatchResult {
            request_id,
            success,
            results,
            failed_at,
            total_elapsed,
            trace,
        } => {
            assert_eq!(request_id, "txn-rt");
            assert!(success);
            assert_eq!(results.len(), 1);
            assert_eq!(failed_at, None);
            assert_eq!(total_elapsed, 1);
            assert!(trace.is_none());
        }
        other => panic!("Expected BatchResult, got: {:?}", other),
    }
}
