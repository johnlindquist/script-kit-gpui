//! Detached Agent Chat window targeting regression tests.
//!
//! Proves that the automation registry can track one or more detached
//! Agent Chat chat windows, resolve them by kind + index, and distinguish them
//! from the main window for screenshot and element targeting.

use script_kit_gpui::protocol::{
    AgentChatResolvedTarget, AgentChatSetupActionKind, AgentChatStateSnapshot,
    AgentChatTestProbeSnapshot, AutomationInspectSnapshot, AutomationWindowInfo,
    AutomationWindowKind, AutomationWindowTarget, Message, SemanticQuality, SimulatedGpuiEvent,
    AUTOMATION_INSPECT_SCHEMA_VERSION,
};
use script_kit_gpui::stdin_commands::KeyModifier;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

static TEST_COUNTER: AtomicU32 = AtomicU32::new(30_000);

/// Serializes tests that resolve by KIND + INDEX (or register/remove windows
/// of the same kind). The automation registry is process-global, so
/// kind-indexed queries scan every test thread's windows — unique ID prefixes
/// cannot isolate them and parallel scheduling makes index/title assertions
/// racy (observed once under full-suite load: index 0 resolved another test's
/// "Closing Agent Chat" window).
static REGISTRY_ORDER_LOCK: Mutex<()> = Mutex::new(());
fn prefix() -> String {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("!agent_chat{n}")
}

fn cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

#[test]
fn detached_agent_chat_targeting_flow() {
    let _registry_order = REGISTRY_ORDER_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let p = prefix();

    // Register main
    let main = AutomationWindowInfo {
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
    };
    script_kit_gpui::windows::upsert_automation_window(main);

    // Register detached Agent Chat
    let agent_chat = AutomationWindowInfo {
        id: format!("{p}:agent_chat-thread-1"),
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
    };
    script_kit_gpui::windows::upsert_automation_window(agent_chat);

    // Target by kind → Agent Chat
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(0),
        }))
        .expect("should resolve Agent Chat");
    assert_eq!(resolved.kind, AutomationWindowKind::AgentChatDetached);
    assert_eq!(resolved.semantic_surface.as_deref(), Some("agentChatChat"));
    assert_eq!(resolved.title.as_deref(), Some("Script Kit Agent Chat"));

    // Target by ID → Agent Chat
    let resolved_id =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-thread-1"),
        }))
        .expect("resolve by ID");
    assert_eq!(resolved_id.kind, AutomationWindowKind::AgentChatDetached);

    // Screenshot routing: Agent Chat title differs from main
    assert_ne!(
        resolved.title.as_deref(),
        Some("Script Kit"),
        "must not screenshot the main window"
    );

    // Focused → Agent Chat (since Agent Chat has focused=true)
    let focused =
        script_kit_gpui::windows::resolve_automation_window(None).expect("resolve focused");
    assert_eq!(focused.kind, AutomationWindowKind::AgentChatDetached);

    cleanup(&p, &["main", "agent_chat-thread-1"]);
}

#[test]
fn multiple_detached_agent_chat_windows_indexed() {
    let _registry_order = REGISTRY_ORDER_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let p = prefix();

    let agent_chat0 = AutomationWindowInfo {
        id: format!("{p}:agent_chat-0"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Agent Chat Thread 0".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("agentChatChat".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(agent_chat0);

    let agent_chat1 = AutomationWindowInfo {
        id: format!("{p}:agent_chat-1"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Agent Chat Thread 1".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("agentChatChat".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(agent_chat1);

    // Index 0 → first registered
    let first =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(0),
        }))
        .expect("resolve index 0");

    // Index 1 → second registered
    let second =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(1),
        }))
        .expect("resolve index 1");

    assert_ne!(first.id, second.id, "index 0 and 1 must differ");

    // Index 2 → error
    let err =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(2),
        }));
    assert!(err.is_err());

    cleanup(&p, &["agent_chat-0", "agent_chat-1"]);
}

#[test]
fn agent_chat_simulate_gpui_event_request_round_trip() {
    // Verify a full Agent Chat-targeted simulateGpuiEvent request parses and
    // serializes with all fields intact.
    let msg = Message::simulate_gpui_event(
        "agent_chat-sim-1".into(),
        SimulatedGpuiEvent::KeyDown {
            key: "k".into(),
            modifiers: vec![KeyModifier::Cmd],
            text: None,
        },
        Some(AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(0),
        }),
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    match back {
        Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
            ..
        } => {
            assert_eq!(request_id, "agent_chat-sim-1");
            assert!(target.is_some());
            match event {
                SimulatedGpuiEvent::KeyDown { key, modifiers, .. } => {
                    assert_eq!(key, "k");
                    assert!(modifiers.contains(&KeyModifier::Cmd));
                }
                other => panic!("Expected KeyDown, got: {:?}", other),
            }
        }
        other => panic!("Expected SimulateGpuiEvent, got: {:?}", other),
    }
}

#[test]
fn agent_chat_window_close_removes_from_registry() {
    let _registry_order = REGISTRY_ORDER_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let p = prefix();

    let agent_chat = AutomationWindowInfo {
        id: format!("{p}:agent_chat-close"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Closing Agent Chat".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("agentChatChat".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(agent_chat);

    // Verify it resolves
    assert!(script_kit_gpui::windows::resolve_automation_window(Some(
        &AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-close"),
        },
    ))
    .is_ok());

    // Close
    script_kit_gpui::windows::remove_automation_window(&format!("{p}:agent_chat-close"));

    // No longer resolvable
    assert!(script_kit_gpui::windows::resolve_automation_window(Some(
        &AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-close"),
        },
    ))
    .is_err());
}

#[test]
fn agent_chat_visibility_toggle() {
    let _registry_order = REGISTRY_ORDER_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let p = prefix();

    let agent_chat = AutomationWindowInfo {
        id: format!("{p}:agent_chat-vis"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Visibility Agent Chat".into()),
        focused: false,
        visible: true,
        semantic_surface: None,
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(agent_chat);

    // Hide
    script_kit_gpui::windows::set_automation_visibility(&format!("{p}:agent_chat-vis"), false);
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-vis"),
        }))
        .expect("resolve");
    assert!(!resolved.visible);

    // Show
    script_kit_gpui::windows::set_automation_visibility(&format!("{p}:agent_chat-vis"), true);
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-vis"),
        }))
        .expect("resolve");
    assert!(resolved.visible);

    cleanup(&p, &["agent_chat-vis"]);
}

// ============================================================
// Protocol-level targeted Agent Chat regression tests
// ============================================================

#[test]
fn reset_agent_chat_test_probe_targeted_round_trip() {
    // Verify resetAgentChatTestProbe with a target field serializes and deserializes correctly.
    let msg = Message::reset_agent_chat_test_probe_targeted(
        "probe-reset-det-1".into(),
        AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: Some(0),
        },
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    match back {
        Message::ResetAgentChatTestProbe { request_id, target } => {
            assert_eq!(request_id, "probe-reset-det-1");
            let target = target.expect("target must be present");
            match target {
                AutomationWindowTarget::Kind { kind, index } => {
                    assert_eq!(kind, AutomationWindowKind::AgentChatDetached);
                    assert_eq!(index, Some(0));
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected ResetAgentChatTestProbe, got: {:?}", other),
    }
}

#[test]
fn reset_agent_chat_test_probe_without_target_backward_compatible() {
    // Legacy global reset (no target) must still parse.
    let json = r#"{"type":"resetAgentChatTestProbe","requestId":"probe-reset-legacy"}"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::ResetAgentChatTestProbe { request_id, target } => {
            assert_eq!(request_id, "probe-reset-legacy");
            assert!(target.is_none(), "legacy reset must have no target");
        }
        other => panic!("Expected ResetAgentChatTestProbe, got: {:?}", other),
    }
}

#[test]
fn agent_chat_test_probe_result_carries_resolved_target() {
    // Build a probe result with resolvedTarget and verify it round-trips.
    let mut probe = AgentChatTestProbeSnapshot::default();
    probe.state.resolved_target = Some(AgentChatResolvedTarget {
        window_id: "agentChatDetached:thread-1".to_string(),
        window_kind: "agentChatDetached".to_string(),
        title: Some("Script Kit Agent Chat".to_string()),
    });
    let msg = Message::agent_chat_test_probe_result("probe-read-det-1".into(), probe);
    let json = serde_json::to_value(&msg).expect("serialize");

    // Verify resolvedTarget is nested inside state
    let state = json.get("state").expect("state field");
    let rt = state
        .get("resolvedTarget")
        .expect("resolvedTarget in state");
    assert_eq!(rt["windowKind"], "agentChatDetached");
    assert_eq!(rt["windowId"], "agentChatDetached:thread-1");
    assert_eq!(rt["title"], "Script Kit Agent Chat");

    // Round-trip
    let back: Message = serde_json::from_value(json).expect("deserialize");
    match back {
        Message::AgentChatTestProbeResult { request_id, probe } => {
            assert_eq!(request_id, "probe-read-det-1");
            let rt = probe.state.resolved_target.expect("resolvedTarget");
            assert_eq!(rt.window_kind, "agentChatDetached");
        }
        other => panic!("Expected AgentChatTestProbeResult, got: {:?}", other),
    }
}

#[test]
fn agent_chat_state_result_carries_resolved_target() {
    // Build an Agent Chat state result with resolvedTarget and verify round-trip.
    let mut state = AgentChatStateSnapshot::default();
    state.resolved_target = Some(AgentChatResolvedTarget {
        window_id: "agentChatDetached:thread-1".to_string(),
        window_kind: "agentChatDetached".to_string(),
        title: Some("Script Kit Agent Chat".to_string()),
    });
    let msg = Message::agent_chat_state_result("agent_chat-state-det-1".into(), state);
    let json = serde_json::to_value(&msg).expect("serialize");

    // resolvedTarget is flattened into the top-level AgentChatStateResult
    let rt = json.get("resolvedTarget").expect("resolvedTarget");
    assert_eq!(rt["windowKind"], "agentChatDetached");

    let back: Message = serde_json::from_value(json).expect("deserialize");
    match back {
        Message::AgentChatStateResult { request_id, state } => {
            assert_eq!(request_id, "agent_chat-state-det-1");
            let rt = state.resolved_target.expect("resolvedTarget");
            assert_eq!(rt.window_kind, "agentChatDetached");
            assert_eq!(rt.window_id, "agentChatDetached:thread-1");
        }
        other => panic!("Expected AgentChatStateResult, got: {:?}", other),
    }
}

#[test]
fn perform_agent_chat_setup_action_targeted_round_trip() {
    // Verify performAgentChatSetupAction with detached target parses correctly.
    let json = serde_json::json!({
        "type": "performAgentChatSetupAction",
        "requestId": "setup-det-1",
        "action": "openAgentPicker",
        "target": {
            "type": "kind",
            "kind": "agentChatDetached",
            "index": 0
        }
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::PerformAgentChatSetupAction {
            request_id,
            action,
            agent_id,
            target,
        } => {
            assert_eq!(request_id, "setup-det-1");
            assert_eq!(action, AgentChatSetupActionKind::OpenAgentPicker);
            assert!(agent_id.is_none());
            let target = target.expect("target must be present");
            match target {
                AutomationWindowTarget::Kind { kind, index } => {
                    assert_eq!(kind, AutomationWindowKind::AgentChatDetached);
                    assert_eq!(index, Some(0));
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected PerformAgentChatSetupAction, got: {:?}", other),
    }
}

#[test]
fn agent_chat_setup_action_result_with_resolved_target_round_trips() {
    // Verify agent_chatSetupActionResult carries resolvedTarget through state.
    let mut state = AgentChatStateSnapshot::default();
    state.status = "setup".to_string();
    state.resolved_target = Some(AgentChatResolvedTarget {
        window_id: "agentChatDetached:thread-1".to_string(),
        window_kind: "agentChatDetached".to_string(),
        title: Some("Script Kit Agent Chat".to_string()),
    });
    let msg = Message::AgentChatSetupActionResult {
        request_id: "setup-det-result".into(),
        success: true,
        error: None,
        state: Some(state),
    };
    let json = serde_json::to_value(&msg).expect("serialize");
    assert_eq!(json["success"], true);

    let state_json = json.get("state").expect("state");
    let rt = state_json.get("resolvedTarget").expect("resolvedTarget");
    assert_eq!(rt["windowKind"], "agentChatDetached");

    let back: Message = serde_json::from_value(json).expect("deserialize");
    match back {
        Message::AgentChatSetupActionResult {
            request_id,
            success,
            state,
            ..
        } => {
            assert_eq!(request_id, "setup-det-result");
            assert!(success);
            let state = state.expect("state");
            let rt = state.resolved_target.expect("resolvedTarget");
            assert_eq!(rt.window_kind, "agentChatDetached");
        }
        other => panic!("Expected AgentChatSetupActionResult, got: {:?}", other),
    }
}

#[test]
fn get_agent_chat_state_targeted_from_json() {
    // Verify getAgentChatState with detached target parses.
    let json = r#"{"type":"getAgentChatState","requestId":"agent_chat-s-1","target":{"type":"kind","kind":"agentChatDetached","index":0}}"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::GetAgentChatState { request_id, target } => {
            assert_eq!(request_id, "agent_chat-s-1");
            let target = target.expect("target");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::AgentChatDetached);
                }
                other => panic!("Expected Kind, got: {:?}", other),
            }
        }
        other => panic!("Expected GetAgentChatState, got: {:?}", other),
    }
}

#[test]
// ============================================================
// Cross-window transaction contract: Agent Chat not rejected as main-only
// ============================================================

fn get_elements_with_agent_chat_detached_target_parses() {
    let json = serde_json::json!({
        "type": "getElements",
        "requestId": "elm-agent_chat-det",
        "target": {"type": "kind", "kind": "agentChatDetached"},
        "limit": 20
    });
    let msg: Message = serde_json::from_value(json)
        .expect("getElements with agentChatDetached target should parse");
    match msg {
        Message::GetElements {
            request_id,
            target,
            limit,
            ..
        } => {
            assert_eq!(request_id, "elm-agent_chat-det");
            assert!(target.is_some());
            assert_eq!(limit, Some(20));
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn agent_chat_detached_get_elements_not_rejected_as_main_only() {
    // Verify that getElements routes Agent Chat targets through surface collector,
    // not the main-only reject path.
    let source = include_str!("../../src/prompt_handler/mod.rs");
    // getElements must use resolve_automation_window (not resolve_main_only_target)
    assert!(
        source.contains("collect_surface_snapshot"),
        "getElements must delegate non-main targets to surface collector"
    );
    // getElements parse test
    get_elements_with_agent_chat_detached_target_parses();
}

#[test]
fn agent_chat_detached_batch_not_rejected_as_main_only() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("resolve_automation_read_target(\n                        &rid,\n                        \"batch\"")
            || source.contains("resolve_automation_read_target(&rid, \"batch\""),
        "batch handler must use resolve_automation_read_target (accepts AgentChatDetached)"
    );
    assert!(
        !source.contains("batch currently supports only the main automation window"),
        "batch must not reject AgentChatDetached with main-only error"
    );
}

#[test]
fn agent_chat_detached_wait_for_not_rejected_as_main_only() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("resolve_agent_chat_read_target(\n                            &rid,\n                            \"waitFor\"")
            || source.contains("resolve_agent_chat_read_target(&request_id, \"waitFor\"")
            || source.contains("resolve_agent_chat_read_target(&rid, \"waitFor\""),
        "waitFor handler must use resolve_agent_chat_read_target (accepts AgentChatDetached)"
    );
    assert!(
        !source.contains(
            "waitFor currently supports only the main automation window; resolved agentChatDetached"
        ),
        "waitFor must not reject AgentChatDetached with main-only error"
    );
}

#[test]
fn agent_chat_detached_transaction_provider_builds_snapshot() {
    let source = include_str!("../../src/windows/automation_transaction_provider.rs");
    assert!(
        source.contains("fn snapshot(&self) -> UiStateSnapshot"),
        "provider must build UiStateSnapshot from detached Agent Chat state"
    );
    assert!(
        source.contains("collect_agent_chat_state_snapshot"),
        "provider snapshot must use live Agent Chat state"
    );
    assert!(
        source.contains("collect_agent_chat_detached_elements"),
        "provider snapshot must include semantic IDs from shared collector"
    );
}

#[test]
fn get_agent_chat_test_probe_targeted_from_json() {
    // Verify getAgentChatTestProbe with detached target parses.
    let json = r#"{"type":"getAgentChatTestProbe","requestId":"agent_chat-p-1","tail":16,"target":{"type":"kind","kind":"agentChatDetached","index":0}}"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::GetAgentChatTestProbe {
            request_id,
            tail,
            target,
        } => {
            assert_eq!(request_id, "agent_chat-p-1");
            assert_eq!(tail, Some(16));
            let target = target.expect("target");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::AgentChatDetached);
                }
                other => panic!("Expected Kind, got: {:?}", other),
            }
        }
        other => panic!("Expected GetAgentChatTestProbe, got: {:?}", other),
    }
}

// ============================================================
// Non-main semantic proof: AgentChatDetached inspect receipts
// ============================================================

#[test]
fn agent_chat_detached_collector_never_returns_non_main_pending_warning() {
    // The collector source must NOT emit semantic_elements_non_main_pending for AgentChatDetached.
    // Instead it must fall back to panel_only_agent_chat_detached.
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel_only_agent_chat_detached\""),
        "AgentChatDetached must use panel_only_agent_chat_detached fallback, not semantic_elements_non_main_pending"
    );
    // The match arm for AgentChatDetached must use unwrap_or_else, not ?.
    assert!(
        !source.contains("collect_agent_chat_detached_snapshot(resolved, cx)?"),
        "AgentChatDetached collector must not use ? (which returns None → triggers non_main_pending)"
    );
}

#[test]
fn agent_chat_detached_inspect_receipt_has_semantic_quality_field() {
    // The handler must set semantic_quality on all inspect receipts
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("semantic_quality: Some(semantic_quality)"),
        "inspect handler must set semantic_quality on the snapshot"
    );
}

#[test]
fn inspect_receipt_schema_version_is_v4() {
    assert_eq!(
        AUTOMATION_INSPECT_SCHEMA_VERSION, 4,
        "schema version must be 4 after schema bump"
    );
}

#[test]
fn semantic_quality_serde_contract() {
    // Full
    let json = serde_json::to_string(&SemanticQuality::Full).expect("serialize");
    assert_eq!(json, "\"full\"");
    // PanelOnly
    let json = serde_json::to_string(&SemanticQuality::PanelOnly).expect("serialize");
    assert_eq!(json, "\"panel_only\"");
    // Unavailable
    let json = serde_json::to_string(&SemanticQuality::Unavailable).expect("serialize");
    assert_eq!(json, "\"unavailable\"");
}

#[test]
fn agent_chat_detached_inspect_result_carries_semantic_quality() {
    // Build an inspect result with semantic_quality and verify it round-trips.
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: "agentChatDetached:thread-1".into(),
        window_kind: "AgentChatDetached".into(),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: Some("Script Kit Agent Chat".into()),
        resolved_bounds: None,
        target_bounds_in_screenshot: None,
        surface_hit_point: None,
        suggested_hit_points: Vec::new(),
        elements: Vec::new(),
        total_count: 0,
        focused_semantic_id: None,
        selected_semantic_id: None,
        screenshot_width: Some(1440),
        screenshot_height: Some(900),
        pixel_probes: Vec::new(),
        os_window_id: Some(99),
        semantic_quality: Some(SemanticQuality::PanelOnly),
        warnings: vec!["panel_only_agent_chat_detached".into()],
        pid: None,
    };
    let msg = Message::automation_inspect_result("inspect-agent_chat-1".into(), snapshot);
    let json = serde_json::to_value(&msg).expect("serialize");

    // Snapshot fields are flattened into top-level JSON (serde flatten)
    assert_eq!(json["semanticQuality"], "panel_only");
    assert_eq!(json["schemaVersion"], 4);

    // Round-trip
    let back: Message = serde_json::from_value(json).expect("deserialize");
    match back {
        Message::AutomationInspectResult { snapshot, .. } => {
            assert_eq!(snapshot.semantic_quality, Some(SemanticQuality::PanelOnly));
        }
        other => panic!("Expected AutomationInspectResult, got: {:?}", other),
    }
}

#[test]
fn agent_chat_detached_full_quality_inspect_receipt() {
    // Full quality when entity is available
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: "agentChatDetached:thread-2".into(),
        window_kind: "AgentChatDetached".into(),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: Some("Script Kit Agent Chat".into()),
        resolved_bounds: None,
        target_bounds_in_screenshot: None,
        surface_hit_point: None,
        suggested_hit_points: Vec::new(),
        elements: vec![script_kit_gpui::protocol::ElementInfo {
            semantic_id: "input:agent_chat-composer".into(),
            element_type: script_kit_gpui::protocol::ElementType::Input,
            text: None,
            value: Some("hello".into()),
            selected: None,
            focused: Some(true),
            index: None,
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
        }],
        total_count: 1,
        focused_semantic_id: Some("input:agent_chat-composer".into()),
        selected_semantic_id: None,
        screenshot_width: Some(800),
        screenshot_height: Some(600),
        pixel_probes: Vec::new(),
        os_window_id: None,
        semantic_quality: Some(SemanticQuality::Full),
        warnings: Vec::new(),
        pid: None,
    };
    let json = serde_json::to_value(&snapshot).expect("serialize");
    assert_eq!(json["semanticQuality"], "full");
    assert!(json.get("warnings").is_none()); // empty vec skipped
    assert_eq!(json["elements"].as_array().expect("elements").len(), 1);
}
