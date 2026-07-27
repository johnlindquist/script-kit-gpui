use super::*;

// ============================================================
// WaitCondition serde — named conditions
// ============================================================

#[test]
fn wait_named_condition_choices_rendered_round_trips() {
    let cond = WaitCondition::Named(WaitNamedCondition::ChoicesRendered);
    let json = serde_json::to_value(&cond).expect("serialize named condition");
    assert_eq!(json, serde_json::json!("choicesRendered"));

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize named condition");
    assert_eq!(back, cond);
}

#[test]
fn wait_named_condition_input_empty_round_trips() {
    let cond = WaitCondition::Named(WaitNamedCondition::InputEmpty);
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json, serde_json::json!("inputEmpty"));

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_named_condition_window_visible_round_trips() {
    let cond = WaitCondition::Named(WaitNamedCondition::WindowVisible);
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json, serde_json::json!("windowVisible"));

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_named_condition_window_focused_round_trips() {
    let cond = WaitCondition::Named(WaitNamedCondition::WindowFocused);
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json, serde_json::json!("windowFocused"));

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

// ============================================================
// WaitCondition serde — detailed conditions
// ============================================================

#[test]
fn wait_detailed_element_exists_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::ElementExists {
        semantic_id: "choice:0:apple".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize detailed condition");
    assert_eq!(json["type"], "elementExists");
    assert_eq!(json["semanticId"], "choice:0:apple");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize detailed condition");
    assert_eq!(back, cond);
}

#[test]
fn wait_detailed_element_visible_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::ElementVisible {
        semantic_id: "input:filter".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "elementVisible");
    assert_eq!(json["semanticId"], "input:filter");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_detailed_element_focused_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::ElementFocused {
        semantic_id: "input:filter".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "elementFocused");
    assert_eq!(json["semanticId"], "input:filter");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_detailed_state_match_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
        state: StateMatchSpec {
            prompt_type: Some("arg".to_string()),
            input_value: Some("apple".to_string()),
            selected_value: None,
            window_visible: Some(true),
        },
    });
    let json = serde_json::to_value(&cond).expect("serialize state match");
    assert_eq!(json["type"], "stateMatch");
    assert_eq!(json["state"]["promptType"], "arg");
    assert_eq!(json["state"]["inputValue"], "apple");
    assert!(json["state"].get("selectedValue").is_none());
    assert_eq!(json["state"]["windowVisible"], true);

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize state match");
    assert_eq!(back, cond);
}

#[test]
fn state_match_spec_omits_none_fields() {
    let spec = StateMatchSpec {
        prompt_type: None,
        input_value: None,
        selected_value: None,
        window_visible: None,
    };
    let json = serde_json::to_value(&spec).expect("serialize empty spec");
    let obj = json.as_object().expect("should be object");
    assert!(
        obj.is_empty(),
        "All-None spec should serialize to empty object"
    );
}

// ============================================================
// waitFor message — request parsing
// ============================================================

#[test]
fn wait_for_request_parses_with_named_condition() {
    let json = r#"{"type":"waitFor","requestId":"wait-1","condition":"choicesRendered","timeout":1000,"pollInterval":25}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse waitFor");

    match msg {
        crate::protocol::Message::WaitFor {
            request_id,
            condition,
            timeout,
            poll_interval,
            ..
        } => {
            assert_eq!(request_id, "wait-1");
            assert_eq!(
                condition,
                WaitCondition::Named(WaitNamedCondition::ChoicesRendered)
            );
            assert_eq!(timeout, Some(1000));
            assert_eq!(poll_interval, Some(25));
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_request_parses_with_detailed_condition() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "wait-2",
        "condition": {
            "type": "elementExists",
            "semanticId": "choice:0:apple"
        }
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor detailed");

    match msg {
        crate::protocol::Message::WaitFor {
            request_id,
            condition,
            timeout,
            poll_interval,
            ..
        } => {
            assert_eq!(request_id, "wait-2");
            assert_eq!(
                condition,
                WaitCondition::Detailed(WaitDetailedCondition::ElementExists {
                    semantic_id: "choice:0:apple".to_string(),
                })
            );
            assert_eq!(timeout, None);
            assert_eq!(poll_interval, None);
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_request_defaults_omitted_timeout_and_poll_interval() {
    let json = r#"{"type":"waitFor","requestId":"wait-3","condition":"inputEmpty"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse waitFor minimal");

    match msg {
        crate::protocol::Message::WaitFor {
            timeout,
            poll_interval,
            ..
        } => {
            assert_eq!(timeout, None);
            assert_eq!(poll_interval, None);
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

// ============================================================
// waitForResult message — response serialization
// ============================================================

#[test]
fn wait_for_result_success_serializes_correctly() {
    let msg = crate::protocol::Message::wait_for_result("wait-1".to_string(), true, 17, None);
    let json = serde_json::to_value(&msg).expect("serialize waitForResult");

    assert_eq!(json["type"], "waitForResult");
    assert_eq!(json["requestId"], "wait-1");
    assert_eq!(json["success"], true);
    assert_eq!(json["elapsed"], 17);
    assert!(
        json.get("error").is_none(),
        "error should be omitted on success"
    );
    assert!(
        json.get("trace").is_none(),
        "trace should be omitted when absent"
    );
}

#[test]
fn wait_for_result_timeout_serializes_correctly() {
    let msg = crate::protocol::Message::wait_for_result(
        "wait-1".to_string(),
        false,
        5000,
        Some(crate::protocol::TransactionError::wait_timeout(
            "Timeout after 5000ms",
        )),
    );
    let json = serde_json::to_value(&msg).expect("serialize waitForResult timeout");

    assert_eq!(json["type"], "waitForResult");
    assert_eq!(json["requestId"], "wait-1");
    assert_eq!(json["success"], false);
    assert_eq!(json["elapsed"], 5000);
    assert_eq!(json["error"]["code"], "wait_condition_timeout");
    assert_eq!(json["error"]["message"], "Timeout after 5000ms");
}

#[test]
fn wait_for_result_round_trips() {
    let msg = crate::protocol::Message::wait_for_result("wait-rt".to_string(), true, 42, None);
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: crate::protocol::Message = serde_json::from_str(&json).expect("deserialize");

    match back {
        crate::protocol::Message::WaitForResult {
            request_id,
            success,
            elapsed,
            error,
            trace,
        } => {
            assert_eq!(request_id, "wait-rt");
            assert!(success);
            assert_eq!(elapsed, 42);
            assert_eq!(error, None);
            assert!(trace.is_none());
        }
        other => panic!("Expected WaitForResult, got: {:?}", other),
    }
}

// ============================================================
// Agent Chat-specific wait conditions — serde roundtrip
// ============================================================

#[test]
fn wait_agent_chat_ready_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatReady);
    let json = serde_json::to_value(&cond).expect("serialize agent_chatReady");
    assert_eq!(json["type"], "agent_chatReady");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize agent_chatReady");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_picker_open_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatPickerOpen);
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatPickerOpen");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_picker_closed_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatPickerClosed);
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatPickerClosed");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_item_accepted_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatItemAccepted);
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatItemAccepted");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_cursor_at_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatCursorAt { index: 15 });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatCursorAt");
    assert_eq!(json["index"], 15);

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_status_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatStatus {
        status: "streaming".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatStatus");
    assert_eq!(json["status"], "streaming");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_input_match_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputMatch {
        text: "@context ".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatInputMatch");
    assert_eq!(json["text"], "@context ");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_input_contains_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputContains {
        substring: "hello".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatInputContains");
    assert_eq!(json["substring"], "hello");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

// ============================================================
// Agent Chat wait conditions — full protocol message roundtrip
// ============================================================

#[test]
fn wait_for_request_parses_with_agent_chat_ready_condition() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "agent_chat-1",
        "condition": { "type": "agent_chatReady" },
        "timeout": 3000,
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor agent_chatReady");

    match msg {
        crate::protocol::Message::WaitFor {
            request_id,
            condition,
            timeout,
            ..
        } => {
            assert_eq!(request_id, "agent_chat-1");
            assert_eq!(
                condition,
                WaitCondition::Detailed(WaitDetailedCondition::AgentChatReady)
            );
            assert_eq!(timeout, Some(3000));
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_request_parses_with_agent_chat_cursor_at_condition() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "agent_chat-2",
        "condition": { "type": "agent_chatCursorAt", "index": 42 },
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor agent_chatCursorAt");

    match msg {
        crate::protocol::Message::WaitFor { condition, .. } => {
            assert_eq!(
                condition,
                WaitCondition::Detailed(WaitDetailedCondition::AgentChatCursorAt { index: 42 })
            );
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

// ============================================================
// getAgentChatState / agent_chatStateResult — protocol message roundtrip
// ============================================================

#[test]
fn get_agent_chat_state_request_parses() {
    let json = r#"{"type":"getAgentChatState","requestId":"agent_chat-state-1"}"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("parse getAgentChatState");

    match msg {
        crate::protocol::Message::GetAgentChatState { request_id, .. } => {
            assert_eq!(request_id, "agent_chat-state-1");
        }
        other => panic!("Expected GetAgentChatState, got: {:?}", other),
    }
}

#[test]
fn agent_chat_state_result_round_trips() {
    let snapshot = crate::protocol::AgentChatStateSnapshot {
        context_parts: Vec::new(),
        schema_version: crate::protocol::AGENT_CHAT_STATE_SCHEMA_VERSION,
        resolved_target: None,
        status: "idle".to_string(),
        ui_variant: "standard".to_string(),
        input_text: "test".to_string(),
        cursor_index: 4,
        has_selection: false,
        selection_range: None,
        message_count: 2,
        composer_fingerprint: Some("composer-fingerprint".to_string()),
        transcript_fingerprint: Some("transcript-fingerprint".to_string()),
        reliability: Some(crate::protocol::AiReliabilityStateSnapshot::ready(
            "agentChat",
        )),
        retained_thread_count: 0,
        fork_point_count: 0,
        awaiting_first_assistant_text: false,
        picker: None,
        spine: None,
        last_accepted_item: None,
        context_chip_count: 1,
        context_summary: None,
        dictation_phase: None,
        context_ready: true,
        has_pending_permission: false,
        input_layout: None,
        transcript_scroll: None,
        composer_scroll: None,
        focused_text: None,
        setup: None,
        warnings: Vec::new(),
    };
    let msg =
        crate::protocol::Message::agent_chat_state_result("agent_chat-rt".to_string(), snapshot);
    let json = serde_json::to_value(&msg).expect("serialize agent_chatStateResult");

    assert_eq!(json["type"], "agent_chatStateResult");
    assert_eq!(json["requestId"], "agent_chat-rt");
    assert_eq!(
        json["schemaVersion"],
        crate::protocol::AGENT_CHAT_STATE_SCHEMA_VERSION
    );
    assert_eq!(json["status"], "idle");
    assert_eq!(json["inputText"], "test");
    assert_eq!(json["cursorIndex"], 4);
    assert_eq!(json["messageCount"], 2);
    assert_eq!(json["contextChipCount"], 1);
    assert!(json["contextReady"].as_bool().unwrap_or(false));
}

// ============================================================
// Agent Chat proof wait conditions — serde roundtrip
// ============================================================

#[test]
fn wait_agent_chat_accepted_via_key_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedViaKey {
        key: "tab".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatAcceptedViaKey");
    assert_eq!(json["key"], "tab");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_accepted_label_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedLabel {
        label: "What I\u{2019}m Looking At".to_string(),
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatAcceptedLabel");
    assert_eq!(json["label"], "What I\u{2019}m Looking At");

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_accepted_cursor_at_round_trips() {
    let cond =
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedCursorAt { index: 17 });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatAcceptedCursorAt");
    assert_eq!(json["index"], 17);

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

#[test]
fn wait_agent_chat_input_layout_match_round_trips() {
    let cond = WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputLayoutMatch {
        visible_start: 0,
        visible_end: 27,
        cursor_in_window: 17,
    });
    let json = serde_json::to_value(&cond).expect("serialize");
    assert_eq!(json["type"], "agent_chatInputLayoutMatch");
    assert_eq!(json["visibleStart"], 0);
    assert_eq!(json["visibleEnd"], 27);
    assert_eq!(json["cursorInWindow"], 17);

    let back: WaitCondition = serde_json::from_value(json).expect("deserialize");
    assert_eq!(back, cond);
}

// ============================================================
// Agent Chat proof wait conditions — full protocol message roundtrip
// ============================================================

#[test]
fn wait_for_request_parses_with_agent_chat_accepted_via_key() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "wait-tab",
        "condition": { "type": "agent_chatAcceptedViaKey", "key": "tab" },
        "timeout": 3000,
        "pollInterval": 25,
        "trace": "onFailure"
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor agent_chatAcceptedViaKey");

    match msg {
        crate::protocol::Message::WaitFor {
            request_id,
            condition,
            timeout,
            ..
        } => {
            assert_eq!(request_id, "wait-tab");
            assert_eq!(
                condition,
                WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedViaKey {
                    key: "tab".to_string(),
                })
            );
            assert_eq!(timeout, Some(3000));
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_request_parses_with_agent_chat_input_layout_match() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "wait-layout",
        "condition": {
            "type": "agent_chatInputLayoutMatch",
            "visibleStart": 0,
            "visibleEnd": 27,
            "cursorInWindow": 17
        },
        "timeout": 1500,
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor agent_chatInputLayoutMatch");

    match msg {
        crate::protocol::Message::WaitFor { condition, .. } => {
            assert_eq!(
                condition,
                WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputLayoutMatch {
                    visible_start: 0,
                    visible_end: 27,
                    cursor_in_window: 17,
                })
            );
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

// ============================================================
// waitFor trace propagation — request parsing and result embedding
// ============================================================

#[test]
fn wait_for_request_parses_trace_on_failure() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "wf-trace",
        "condition": { "type": "agent_chatAcceptedViaKey", "key": "enter" },
        "timeout": 100,
        "pollInterval": 25,
        "trace": "onFailure"
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor with trace");

    match msg {
        crate::protocol::Message::WaitFor {
            request_id, trace, ..
        } => {
            assert_eq!(request_id, "wf-trace");
            assert_eq!(trace, crate::protocol::TransactionTraceMode::OnFailure);
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_request_parses_trace_on() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "wf-trace-on",
        "condition": "windowVisible",
        "trace": "on"
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor with trace on");

    match msg {
        crate::protocol::Message::WaitFor { trace, .. } => {
            assert_eq!(trace, crate::protocol::TransactionTraceMode::On);
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_request_defaults_trace_off() {
    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "wf-no-trace",
        "condition": "windowVisible"
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse waitFor without trace");

    match msg {
        crate::protocol::Message::WaitFor { trace, .. } => {
            assert_eq!(trace, crate::protocol::TransactionTraceMode::Off);
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn wait_for_result_with_trace_serializes_correctly() {
    let trace = crate::protocol::TransactionTrace {
        schema_version: 1,
        request_id: "wf-trace".to_string(),
        command_fingerprint: "fp-wf-trace".to_string(),
        status: crate::protocol::TransactionTraceStatus::Timeout,
        started_at_ms: 1000,
        total_elapsed_ms: 100,
        failed_at: Some(0),
        commands: vec![crate::protocol::TransactionCommandTrace {
            index: 0,
            command: "waitFor".to_string(),
            command_payload: None,
            started_at_ms: 1000,
            elapsed_ms: 100,
            before: crate::protocol::UiStateSnapshot::default(),
            after: crate::protocol::UiStateSnapshot::default(),
            polls: Vec::new(),
            error: Some(crate::protocol::TransactionError::wait_timeout(
                "Timeout after 100ms waiting for AgentChatAcceptedViaKey { key: \"enter\" }",
            )),
        }],
    };
    let msg = crate::protocol::Message::wait_for_result_with_trace(
        "wf-trace".to_string(),
        false,
        100,
        Some(crate::protocol::TransactionError::wait_timeout(
            "Timeout after 100ms",
        )),
        Some(trace),
    );
    let json = serde_json::to_value(&msg).expect("serialize waitForResult with trace");

    assert_eq!(json["type"], "waitForResult");
    assert_eq!(json["requestId"], "wf-trace");
    assert_eq!(json["success"], false);
    assert_eq!(json["elapsed"], 100);
    assert!(json["error"].is_object(), "error should be present");
    assert!(json["trace"].is_object(), "trace should be present");
    assert_eq!(json["trace"]["requestId"], "wf-trace");
    assert_eq!(json["trace"]["status"], "timeout");
    assert_eq!(json["trace"]["failedAt"], 0);
    assert_eq!(json["trace"]["commands"][0]["command"], "waitFor");
}

#[test]
fn wait_for_result_without_trace_omits_trace_field() {
    let msg = crate::protocol::Message::wait_for_result_with_trace(
        "wf-no-trace".to_string(),
        true,
        50,
        None::<crate::protocol::TransactionError>,
        None,
    );
    let json = serde_json::to_value(&msg).expect("serialize");

    assert_eq!(json["type"], "waitForResult");
    assert_eq!(json["success"], true);
    assert!(
        json.get("trace").is_none(),
        "trace should be omitted when None"
    );
}

// ============================================================
// batch trace propagation — request parsing and result embedding
// ============================================================

#[test]
fn batch_request_parses_trace_on_failure() {
    let json = serde_json::json!({
        "type": "batch",
        "requestId": "b-trace",
        "commands": [
            { "type": "waitFor", "condition": "windowVisible", "timeout": 100 }
        ],
        "trace": "onFailure"
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse batch with trace");

    match msg {
        crate::protocol::Message::Batch {
            request_id, trace, ..
        } => {
            assert_eq!(request_id, "b-trace");
            assert_eq!(trace, crate::protocol::TransactionTraceMode::OnFailure);
        }
        other => panic!("Expected Batch, got: {:?}", other),
    }
}

#[test]
fn batch_result_with_trace_serializes_correctly() {
    let trace = crate::protocol::TransactionTrace {
        schema_version: 1,
        request_id: "b-trace".to_string(),
        command_fingerprint: "fp-b-trace".to_string(),
        status: crate::protocol::TransactionTraceStatus::Failed,
        started_at_ms: 2000,
        total_elapsed_ms: 150,
        failed_at: Some(0),
        commands: vec![crate::protocol::TransactionCommandTrace {
            index: 0,
            command: "waitFor".to_string(),
            command_payload: None,
            started_at_ms: 2000,
            elapsed_ms: 150,
            before: crate::protocol::UiStateSnapshot::default(),
            after: crate::protocol::UiStateSnapshot::default(),
            polls: Vec::new(),
            error: Some(crate::protocol::TransactionError::wait_timeout("Timeout")),
        }],
    };
    let msg = crate::protocol::Message::batch_result_with_trace(
        "b-trace".to_string(),
        false,
        vec![crate::protocol::BatchResultEntry {
            index: 0,
            success: false,
            command: "waitFor".to_string(),
            elapsed: Some(150),
            value: None,
            error: Some(crate::protocol::TransactionError::wait_timeout("Timeout")),
        }],
        Some(0),
        150,
        Some(trace),
    );
    let json = serde_json::to_value(&msg).expect("serialize batchResult with trace");

    assert_eq!(json["type"], "batchResult");
    assert_eq!(json["requestId"], "b-trace");
    assert_eq!(json["success"], false);
    assert!(json["trace"].is_object(), "trace should be present");
    assert_eq!(json["trace"]["status"], "failed");
    assert_eq!(json["trace"]["failedAt"], 0);
}

// ============================================================
// resetAgentChatTestProbe / getAgentChatTestProbe / agent_chatTestProbeResult
// ============================================================

#[test]
fn reset_agent_chat_test_probe_request_parses() {
    let json = r#"{"type":"resetAgentChatTestProbe","requestId":"probe-reset-1"}"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("parse resetAgentChatTestProbe");

    match msg {
        crate::protocol::Message::ResetAgentChatTestProbe { request_id, target } => {
            assert_eq!(request_id, "probe-reset-1");
            assert!(target.is_none(), "legacy request should have no target");
        }
        other => panic!("Expected ResetAgentChatTestProbe, got: {:?}", other),
    }
}

#[test]
fn reset_agent_chat_test_probe_with_target_round_trips() {
    let json = serde_json::json!({
        "type": "resetAgentChatTestProbe",
        "requestId": "probe-reset-targeted",
        "target": {
            "type": "kind",
            "kind": "agentChatDetached",
            "index": 0
        }
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse resetAgentChatTestProbe with target");

    match &msg {
        crate::protocol::Message::ResetAgentChatTestProbe { request_id, target } => {
            assert_eq!(request_id, "probe-reset-targeted");
            let t = target.as_ref().expect("should have target");
            match t {
                crate::protocol::AutomationWindowTarget::Kind { kind, index } => {
                    assert_eq!(
                        *kind,
                        crate::protocol::AutomationWindowKind::AgentChatDetached
                    );
                    assert_eq!(*index, Some(0));
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected ResetAgentChatTestProbe, got: {:?}", other),
    }

    // Round-trip
    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(
        re_serialized["type"].as_str(),
        Some("resetAgentChatTestProbe")
    );
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("probe-reset-targeted")
    );
    assert_eq!(
        re_serialized["target"]["kind"].as_str(),
        Some("agentChatDetached")
    );
}

#[test]
fn get_agent_chat_test_probe_request_parses() {
    let json = serde_json::json!({
        "type": "getAgentChatTestProbe",
        "requestId": "probe-get-1",
        "tail": 20,
    });
    let msg: crate::protocol::Message =
        serde_json::from_value(json).expect("parse getAgentChatTestProbe");

    match msg {
        crate::protocol::Message::GetAgentChatTestProbe {
            request_id, tail, ..
        } => {
            assert_eq!(request_id, "probe-get-1");
            assert_eq!(tail, Some(20));
        }
        other => panic!("Expected GetAgentChatTestProbe, got: {:?}", other),
    }
}

#[test]
fn get_agent_chat_test_probe_request_parses_without_tail() {
    let json = r#"{"type":"getAgentChatTestProbe","requestId":"probe-get-2"}"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("parse getAgentChatTestProbe without tail");

    match msg {
        crate::protocol::Message::GetAgentChatTestProbe {
            request_id, tail, ..
        } => {
            assert_eq!(request_id, "probe-get-2");
            assert_eq!(tail, None);
        }
        other => panic!("Expected GetAgentChatTestProbe, got: {:?}", other),
    }
}

#[test]
fn agent_chat_test_probe_result_round_trips() {
    let probe = crate::protocol::AgentChatTestProbeSnapshot {
        schema_version: crate::protocol::AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION,
        event_seq: 5,
        key_routes: vec![crate::protocol::AgentChatKeyRouteTelemetry {
            key: "tab".to_string(),
            route: crate::protocol::AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 17,
            caused_submit: false,
            consumed: true,
        }],
        accepted_items: vec![crate::protocol::AgentChatPickerItemAcceptedTelemetry {
            trigger: "@".to_string(),
            item_label: "What I\u{2019}m Looking At".to_string(),
            item_id: "built_in:context".to_string(),
            accepted_via_key: "tab".to_string(),
            cursor_after: 17,
            caused_submit: false,
        }],
        input_layout: Some(crate::protocol::AgentChatInputLayoutTelemetry {
            char_count: 27,
            visible_start: 0,
            visible_end: 27,
            cursor_in_window: 17,
        }),
        last_interaction_trace: None,
        state: crate::protocol::AgentChatStateSnapshot {
            status: "idle".to_string(),
            cursor_index: 17,
            ..Default::default()
        },
        warnings: Vec::new(),
    };
    let msg = crate::protocol::Message::agent_chat_test_probe_result("probe-rt".to_string(), probe);
    let json = serde_json::to_value(&msg).expect("serialize agent_chatTestProbeResult");

    assert_eq!(json["type"], "agent_chatTestProbeResult");
    assert_eq!(json["requestId"], "probe-rt");
    assert_eq!(
        json["schemaVersion"],
        crate::protocol::AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION
    );
    assert_eq!(json["eventSeq"], 5);
    assert_eq!(json["acceptedItems"][0]["acceptedViaKey"], "tab");
    assert_eq!(json["acceptedItems"][0]["cursorAfter"], 17);
    assert_eq!(json["keyRoutes"][0]["key"], "tab");
    assert_eq!(json["inputLayout"]["cursorInWindow"], 17);

    // Round-trip back through deserialization
    let json_str = serde_json::to_string(&msg).expect("serialize to string");
    let back: crate::protocol::Message = serde_json::from_str(&json_str).expect("deserialize");
    match back {
        crate::protocol::Message::AgentChatTestProbeResult { request_id, probe } => {
            assert_eq!(request_id, "probe-rt");
            assert_eq!(probe.event_seq, 5);
            assert_eq!(probe.accepted_items.len(), 1);
            assert_eq!(probe.key_routes.len(), 1);
        }
        other => panic!("Expected AgentChatTestProbeResult, got: {:?}", other),
    }
}
