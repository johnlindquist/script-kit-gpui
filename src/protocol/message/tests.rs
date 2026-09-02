use super::*;

/// Test: Chat message with useBuiltinAi flag should be parsed correctly
///
/// When SDK sends chat with `useBuiltinAi: true`, the app should use
/// its built-in AI providers instead of relying on SDK callbacks.
#[test]
fn test_chat_message_with_use_builtin_ai() {
    let json = r#"{
            "type": "chat",
            "id": "chat-1",
            "placeholder": "Ask a question...",
            "messages": [{"role": "user", "content": "Hello"}],
            "hint": "Agent Chat",
            "useBuiltinAi": true
        }"#;

    let msg: Message = serde_json::from_str(json).expect("Should parse chat message");

    match msg {
        Message::Chat {
            id,
            placeholder,
            use_builtin_ai,
            ..
        } => {
            assert_eq!(id, "chat-1");
            assert_eq!(placeholder, Some("Ask a question...".to_string()));
            assert!(use_builtin_ai, "useBuiltinAi should be true");
        }
        _ => panic!("Expected Chat message"),
    }
}

/// Test: Chat message without useBuiltinAi should default to false
#[test]
fn test_chat_message_without_use_builtin_ai_defaults_to_false() {
    let json = r#"{
            "type": "chat",
            "id": "chat-2",
            "messages": []
        }"#;

    let msg: Message = serde_json::from_str(json).expect("Should parse chat message");

    match msg {
        Message::Chat { use_builtin_ai, .. } => {
            assert!(!use_builtin_ai, "useBuiltinAi should default to false");
        }
        _ => panic!("Expected Chat message"),
    }
}

#[test]
fn query_and_terminal_messages_preserve_request_identity_through_wire_roundtrip() {
    use serde_json::json;
    let messages = [
        json!({"type":"windowVisibilityAck","windowVisible":false}),
        json!({"type":"displayList"}),
        json!({"type":"displayListResult","displays":[]}),
        json!({"type":"frontmostWindow"}),
        json!({"type":"frontmostWindowResult"}),
        json!({"type":"getAgentChatState"}),
        json!({"type":"getAiReliabilityState"}),
        json!({"type":"setAiReliabilityTestFixture","fixtureId":"fixture"}),
        json!({"type":"inspectContextPreparation","fixtureId":"fixture"}),
        json!({"type":"resetAgentChatTestProbe"}),
        json!({"type":"getAgentChatTestProbe"}),
        json!({"type":"performAgentChatSetupAction","action":"retry"}),
        json!({"type":"waitFor","condition":"inputEmpty"}),
        json!({"type":"waitForResult","success":true,"elapsed":0}),
        json!({"type":"batch","commands":[]}),
        json!({"type":"batchResult","success":true,"results":[],"totalElapsed":0}),
        json!({"type":"listAutomationWindows"}),
        json!({"type":"automationWindowListResult","windows":[]}),
        json!({"type":"simulateGpuiEvent","event":{"type":"keyDown","key":"down"},"deadlineUnixMs":123}),
        json!({"type":"cancelGpuiEvent"}),
        json!({"type":"simulateGpuiEventResult","success":false,"errorCode":"dispatch_cancelled","wasDeferred":true}),
        json!({"type":"getLogs"}),
        json!({"type":"logsResult","entries":[],"matched":0,"capacity":500}),
    ];
    for (index, mut wire) in messages.into_iter().enumerate() {
        let id = format!("correlated-{index}");
        wire["requestId"] = json!(id);
        let message: Message = serde_json::from_value(wire.clone())
            .unwrap_or_else(|error| panic!("{}: {error}", wire["type"]));
        assert_eq!(message.request_id(), Some(id.as_str()), "{}", wire["type"]);
        assert_eq!(message.id(), Some(id.as_str()));
        let serialized = crate::protocol::serialize_message(&message).unwrap();
        let response: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(response["requestId"], id);
        assert_eq!(response["protocolVersion"], 2);
        let roundtripped: Message = serde_json::from_str(&serialized).unwrap();
        assert_eq!(roundtripped.request_id(), Some(id.as_str()));
        assert_eq!(roundtripped.id(), Some(id.as_str()));
    }
}

#[test]
fn wire_serializer_owns_version_even_when_update_payload_supplies_one() {
    let message = Message::Update {
        id: "update-version-owner".into(),
        data: serde_json::json!({"protocolVersion": 999, "text": "unchanged"}),
    };
    let serialized = crate::protocol::serialize_message(&message).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({
            "type": "update",
            "id": "update-version-owner",
            "protocolVersion": crate::protocol::version::CURRENT_PROTOCOL_VERSION,
            "text": "unchanged"
        })
    );
    assert_eq!(
        serialized.matches("\"protocolVersion\"").count(),
        1,
        "transport metadata must not be duplicated by flattened payloads"
    );
}

#[test]
fn scoped_batch_fingerprint_changes_for_target_parent_and_session_lifetimes() {
    let target: AutomationWindowInfo = serde_json::from_value(serde_json::json!({
        "id":"main","kind":"main","generation":4,"visible":false,"focused":false,
    }))
    .unwrap();
    let commands = [BatchCommand::SetInput {
        text: "owned input".into(),
    }];
    let fingerprint = |target: &AutomationWindowInfo, session: &str| {
        crate::protocol::transaction_executor::scoped_transaction_fingerprint(
            &commands, None, target, session,
        )
        .unwrap()
    };
    let original = fingerprint(&target, "session-a");
    assert_eq!(original, fingerprint(&target, "session-a"));
    assert_ne!(original, fingerprint(&target, "session-b"));
    let mut changed = target.clone();
    changed.generation = Some(5);
    assert_ne!(original, fingerprint(&changed, "session-a"));
    changed = target.clone();
    changed.id = "another-main".into();
    assert_ne!(original, fingerprint(&changed, "session-a"));
    changed = target;
    changed.parent_window_id = Some("parent".into());
    changed.parent_window_generation = Some(6);
    assert_ne!(original, fingerprint(&changed, "session-a"));
}
