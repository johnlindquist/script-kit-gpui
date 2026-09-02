//! Integration tests for targeted Agent Chat state and test-probe reads.
//!
//! Verifies that:
//! - `getAgentChatState` and `getAgentChatTestProbe` with an `AgentChatDetached` target resolve
//!   through the automation registry (registry-level, not runtime-level, since
//!   we cannot open real GPUI windows in unit tests).
//! - Non-Agent Chat secondary targets produce structured `target_unsupported` warnings.
//! - Protocol messages round-trip correctly with target fields.
//! - Main-window (no target) messages still parse and serialize correctly.

use script_kit_gpui::protocol::{
    AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget,
};
use std::sync::atomic::{AtomicU32, Ordering};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(50_000);
fn prefix() -> String {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("agent_chatread{n}")
}

fn cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

// ── Registry-level targeting ────────────────────────────────────────

#[test]
fn agent_chat_detached_target_resolves_to_correct_kind() {
    let p = prefix();

    script_kit_gpui::windows::upsert_automation_window(AutomationWindowInfo {
        id: format!("{p}:main"),
        kind: AutomationWindowKind::Main,
        title: Some("Script Kit".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("scriptList".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    });
    script_kit_gpui::windows::upsert_automation_window(AutomationWindowInfo {
        id: format!("{p}:agent_chat-1"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Script Kit Agent Chat".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("agentChatChat".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    });

    // Target by ID for deterministic resolution (avoids global registry index collisions)
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-1"),
        }))
        .expect("should resolve AgentChatDetached target by ID");

    assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);
    assert_eq!(resolved.id, format!("{p}:agent_chat-1"));

    let main =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:main"),
        }))
        .expect("should resolve main by ID");
    assert_eq!(main.kind, AutomationWindowKind::Main);

    cleanup(&p, &["main", "agent_chat-1"]);
}

#[test]
fn non_agent_chat_secondary_target_does_not_resolve_as_agent_chat() {
    let p = prefix();

    script_kit_gpui::windows::upsert_automation_window(AutomationWindowInfo {
        id: format!("{p}:notes"),
        kind: AutomationWindowKind::Notes,
        title: Some("Script Kit Notes".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("notes".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    });

    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        }))
        .expect("should resolve notes");
    assert_eq!(resolved.kind, AutomationWindowKind::Notes);
    assert_ne!(resolved.kind, AutomationWindowKind::AgentChatDetached);

    cleanup(&p, &["notes"]);
}

// ── Protocol message round-trip ─────────────────────────────────────

#[test]
fn get_agent_chat_state_with_detached_target_round_trips() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "getAgentChatState",
        "requestId": "agent_chat-state-detached-1",
        "target": {
            "type": "kind",
            "kind": "agentChatDetached",
            "index": 0
        }
    });

    let msg: Message = serde_json::from_value(json.clone())
        .expect("should parse getAgentChatState with AgentChatDetached target");

    // Round-trip: serialize back and compare key fields
    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(re_serialized["type"].as_str(), Some("getAgentChatState"));
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("agent_chat-state-detached-1")
    );
    assert_eq!(
        re_serialized["target"]["kind"].as_str(),
        Some("agentChatDetached")
    );
}

#[test]
fn get_agent_chat_test_probe_with_detached_target_round_trips() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "getAgentChatTestProbe",
        "requestId": "agent_chat-probe-detached-1",
        "tail": 10,
        "target": {
            "type": "kind",
            "kind": "agentChatDetached",
            "index": 0
        }
    });

    let msg: Message = serde_json::from_value(json)
        .expect("should parse getAgentChatTestProbe with AgentChatDetached target");

    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(
        re_serialized["type"].as_str(),
        Some("getAgentChatTestProbe")
    );
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("agent_chat-probe-detached-1")
    );
    assert_eq!(
        re_serialized["target"]["kind"].as_str(),
        Some("agentChatDetached")
    );
    assert_eq!(re_serialized["tail"].as_u64(), Some(10));
}

#[test]
fn get_agent_chat_state_without_target_round_trips() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "getAgentChatState",
        "requestId": "agent_chat-state-main-1"
    });

    let msg: Message =
        serde_json::from_value(json).expect("should parse getAgentChatState without target");

    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(re_serialized["type"].as_str(), Some("getAgentChatState"));
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("agent_chat-state-main-1")
    );
}

#[test]
fn get_agent_chat_test_probe_without_target_round_trips() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "getAgentChatTestProbe",
        "requestId": "agent_chat-probe-main-1"
    });

    let msg: Message =
        serde_json::from_value(json).expect("should parse getAgentChatTestProbe without target");

    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(
        re_serialized["type"].as_str(),
        Some("getAgentChatTestProbe")
    );
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("agent_chat-probe-main-1")
    );
}

// ── Agent Chat state/probe result with warnings round-trip ──────────────────

#[test]
fn agent_chat_state_result_with_target_unsupported_warning_round_trips() {
    use script_kit_gpui::protocol::{AgentChatStateSnapshot, Message};

    let mut state = AgentChatStateSnapshot::default();
    state.warnings = vec![
        "target_unsupported: getAgentChatState supports only Main and AgentChatDetached targets; resolved notes:1 (Notes)".to_string(),
    ];

    let response = Message::agent_chat_state_result("agent_chat-state-notes-1".to_string(), state);
    let json = serde_json::to_value(&response).expect("serialize");

    assert_eq!(json["type"].as_str(), Some("agent_chatStateResult"));
    let warnings = json["warnings"].as_array().expect("warnings array");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]
        .as_str()
        .expect("warning str")
        .contains("target_unsupported"));
}

#[test]
fn agent_chat_test_probe_result_with_target_unsupported_warning_round_trips() {
    use script_kit_gpui::protocol::{AgentChatTestProbeSnapshot, Message};

    let mut probe = AgentChatTestProbeSnapshot::default();
    probe.warnings = vec![
        "target_unsupported: getAgentChatTestProbe supports only Main and AgentChatDetached targets; resolved notes:1 (Notes)".to_string(),
    ];

    let response =
        Message::agent_chat_test_probe_result("agent_chat-probe-notes-1".to_string(), probe);
    let json = serde_json::to_value(&response).expect("serialize");

    assert_eq!(json["type"].as_str(), Some("agent_chatTestProbeResult"));
    let warnings = json["warnings"].as_array().expect("warnings array");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]
        .as_str()
        .expect("warning str")
        .contains("target_unsupported"));
}

// ── Target-by-ID for detached Agent Chat ───────────────────────────────────

#[test]
fn agent_chat_detached_target_by_id_resolves_correctly() {
    let p = prefix();

    script_kit_gpui::windows::upsert_automation_window(AutomationWindowInfo {
        id: format!("{p}:agent_chat-thread-42"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Script Kit Agent Chat".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("agentChatChat".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    });

    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-thread-42"),
        }))
        .expect("should resolve by ID");

    assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);
    assert_eq!(resolved.id, format!("{p}:agent_chat-thread-42"));

    cleanup(&p, &["agent_chat-thread-42"]);
}

// ── Multiple Agent Chat windows with index targeting ────────────────────────

#[test]
fn multiple_agent_chat_detached_windows_indexed_targeting() {
    let p = prefix();

    for i in 0..3 {
        script_kit_gpui::windows::upsert_automation_window(AutomationWindowInfo {
            id: format!("{p}:agent_chat-{i}"),
            kind: AutomationWindowKind::AgentChatDetached,
            title: Some(format!("Agent Chat Thread {i}")),
            focused: i == 1,
            visible: true,
            semantic_surface: Some("agentChatChat".into()),
            bounds: None,
            parent_window_id: None,
            parent_window_generation: None,
            parent_kind: None,
            pid: None,
            generation: None,
        });
    }

    let r0 =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(0),
        }))
        .expect("index 0");
    assert_eq!(r0.kind, AutomationWindowKind::AgentChatDetached);

    let r2 =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(2),
        }))
        .expect("index 2");
    assert_eq!(r2.kind, AutomationWindowKind::AgentChatDetached);
    assert_ne!(r0.id, r2.id);

    let err =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(99),
        }));
    assert!(err.is_err());

    cleanup(&p, &["agent_chat-0", "agent_chat-1", "agent_chat-2"]);
}

// ── resetAgentChatTestProbe with target ──────────────────────────────────

#[test]
fn reset_agent_chat_test_probe_with_detached_target_round_trips() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "resetAgentChatTestProbe",
        "requestId": "probe-reset-detached-1",
        "target": {
            "type": "kind",
            "kind": "agentChatDetached",
            "index": 0
        }
    });

    let msg: Message = serde_json::from_value(json)
        .expect("should parse resetAgentChatTestProbe with AgentChatDetached target");

    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(
        re_serialized["type"].as_str(),
        Some("resetAgentChatTestProbe")
    );
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("probe-reset-detached-1")
    );
    assert_eq!(
        re_serialized["target"]["kind"].as_str(),
        Some("agentChatDetached")
    );
}

#[test]
fn reset_agent_chat_test_probe_without_target_backward_compatible() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "resetAgentChatTestProbe",
        "requestId": "probe-reset-legacy"
    });

    let msg: Message = serde_json::from_value(json)
        .expect("should parse legacy resetAgentChatTestProbe without target");

    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(
        re_serialized["type"].as_str(),
        Some("resetAgentChatTestProbe")
    );
    assert_eq!(
        re_serialized["requestId"].as_str(),
        Some("probe-reset-legacy")
    );
    // target should not be present in serialized output
    assert!(
        re_serialized.get("target").is_none() || re_serialized["target"].is_null(),
        "target should be omitted for legacy requests"
    );
}

// ── AgentChatResolvedTarget in state responses ──────────────────────────────

#[test]
fn agent_chat_state_result_with_resolved_target_round_trips() {
    use script_kit_gpui::protocol::{AgentChatResolvedTarget, AgentChatStateSnapshot, Message};

    let mut state = AgentChatStateSnapshot::default();
    state.resolved_target = Some(AgentChatResolvedTarget {
        window_id: "agentChatDetached:thread-1".to_string(),
        window_kind: "agentChatDetached".to_string(),
        title: Some("Script Kit Agent Chat".to_string()),
    });

    let response =
        Message::agent_chat_state_result("agent_chat-state-resolved-1".to_string(), state);
    let json = serde_json::to_value(&response).expect("serialize");

    assert_eq!(json["type"].as_str(), Some("agent_chatStateResult"));
    assert_eq!(
        json["resolvedTarget"]["windowId"].as_str(),
        Some("agentChatDetached:thread-1")
    );
    assert_eq!(
        json["resolvedTarget"]["windowKind"].as_str(),
        Some("agentChatDetached")
    );
    assert_eq!(
        json["resolvedTarget"]["title"].as_str(),
        Some("Script Kit Agent Chat")
    );

    // Schema version should be 2 with resolved_target support
    assert_eq!(
        json["schemaVersion"].as_u64(),
        Some(script_kit_gpui::protocol::AGENT_CHAT_STATE_SCHEMA_VERSION as u64)
    );
}

#[test]
fn agent_chat_state_result_without_resolved_target_omits_field() {
    use script_kit_gpui::protocol::{AgentChatStateSnapshot, Message};

    let state = AgentChatStateSnapshot::default();
    let response =
        Message::agent_chat_state_result("agent_chat-state-main-compat".to_string(), state);
    let json = serde_json::to_value(&response).expect("serialize");

    assert!(
        json.get("resolvedTarget").is_none() || json["resolvedTarget"].is_null(),
        "resolvedTarget should be absent when None"
    );
}

// ── waitFor with Agent Chat detached target round-trips ─────────────────────

#[test]
fn wait_for_with_agent_chat_detached_target_round_trips() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "agent_chat-wait-detached-1",
        "condition": {
            "type": "agent_chatAcceptedViaKey",
            "key": "enter"
        },
        "target": {
            "type": "kind",
            "kind": "agentChatDetached",
            "index": 0
        },
        "timeout": 3000,
        "pollInterval": 25,
        "trace": "onFailure"
    });

    let msg: Message =
        serde_json::from_value(json).expect("should parse waitFor with AgentChatDetached target");

    let re_serialized = serde_json::to_value(&msg).expect("re-serialize");
    assert_eq!(re_serialized["type"].as_str(), Some("waitFor"));
    assert_eq!(
        re_serialized["target"]["kind"].as_str(),
        Some("agentChatDetached")
    );
    assert_eq!(re_serialized["timeout"].as_u64(), Some(3000));
}
