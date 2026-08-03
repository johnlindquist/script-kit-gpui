//! Cross-window targeting regression tests.
//!
//! These tests validate that the automation window registry and resolver
//! produce stable, deterministic results for all window kinds — without
//! requiring a live GPUI event loop.

use script_kit_gpui::protocol::{
    AutomationWindowBounds, AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget,
    Message, SimulatedGpuiEvent,
};
use script_kit_gpui::stdin_commands::KeyModifier;
use std::sync::atomic::{AtomicU32, Ordering};

// Unique prefix per test to avoid global registry collisions.
static TEST_COUNTER: AtomicU32 = AtomicU32::new(10_000);
fn prefix() -> String {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("!wt{n}")
}

fn make_info(prefix: &str, id: &str, kind: AutomationWindowKind) -> AutomationWindowInfo {
    AutomationWindowInfo {
        id: format!("{prefix}:{id}"),
        kind,
        title: Some(format!("Window {id}")),
        focused: false,
        visible: true,
        semantic_surface: None,
        bounds: None,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    }
}

fn cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

// ============================================================
// Target round-trip serde
// ============================================================

#[test]
fn automation_window_target_round_trip_kind() {
    let json = r#"{"type":"kind","kind":"agentChatDetached","index":0}"#;
    let parsed: AutomationWindowTarget =
        serde_json::from_str(json).expect("target should deserialize");
    let out = serde_json::to_string(&parsed).expect("target should serialize");
    let back: AutomationWindowTarget = serde_json::from_str(&out).expect("roundtrip");
    assert_eq!(back, parsed);
}

#[test]
fn automation_window_target_round_trip_all_kinds() {
    let kinds = [
        AutomationWindowKind::Main,
        AutomationWindowKind::Notes,
        AutomationWindowKind::AgentChatDetached,
        AutomationWindowKind::ActionsDialog,
        AutomationWindowKind::PromptPopup,
    ];
    for kind in &kinds {
        let target = AutomationWindowTarget::Kind {
            kind: *kind,
            index: None,
        };
        let json = serde_json::to_string(&target).expect("serialize");
        let back: AutomationWindowTarget = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, target, "round-trip failed for kind {:?}", kind);
    }
}

// ============================================================
// SimulateGpuiEvent protocol serde
// ============================================================

#[test]
fn simulate_gpui_event_dispatches_to_target_window() {
    // Verify the full request message parses with a target
    let json = r#"{
        "type": "simulateGpuiEvent",
        "requestId": "gpui-target-1",
        "target": {"type": "kind", "kind": "notes"},
        "event": {"type": "keyDown", "key": "escape", "modifiers": []}
    }"#;
    let msg: Message = serde_json::from_str(json).expect("parse simulateGpuiEvent");
    match msg {
        Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
        } => {
            assert_eq!(request_id, "gpui-target-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::Notes);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
            match event {
                SimulatedGpuiEvent::KeyDown { key, .. } => {
                    assert_eq!(key, "escape");
                }
                other => panic!("Expected KeyDown, got: {:?}", other),
            }
        }
        other => panic!("Expected SimulateGpuiEvent, got: {:?}", other),
    }
}

#[test]
fn simulate_gpui_event_mouse_events_parse() {
    let json = r#"{
        "type": "simulateGpuiEvent",
        "requestId": "mouse-1",
        "target": {"type": "main"},
        "event": {"type": "mouseDown", "x": 100.5, "y": 200.0, "button": "left"}
    }"#;
    let msg: Message = serde_json::from_str(json).expect("parse mouse event");
    match msg {
        Message::SimulateGpuiEvent { event, .. } => match event {
            SimulatedGpuiEvent::MouseDown { x, y, button } => {
                assert!((x - 100.5).abs() < f64::EPSILON);
                assert!((y - 200.0).abs() < f64::EPSILON);
                assert_eq!(button.as_deref(), Some("left"));
            }
            other => panic!("Expected MouseDown, got: {:?}", other),
        },
        other => panic!("Expected SimulateGpuiEvent, got: {:?}", other),
    }
}

// ============================================================
// Registry targeting — multi-window resolution
// ============================================================

#[test]
fn registry_resolves_each_kind_independently() {
    let p = "!0_registry_resolves_each_kind_independently".to_string();

    let mut main = make_info(&p, "main", AutomationWindowKind::Main);
    main.focused = true;
    main.semantic_surface = Some("scriptList".into());
    script_kit_gpui::windows::upsert_automation_window(main);

    let mut notes = make_info(&p, "notes", AutomationWindowKind::Notes);
    notes.semantic_surface = Some("notes".into());
    script_kit_gpui::windows::upsert_automation_window(notes);

    let mut agent_chat = make_info(&p, "agent_chat", AutomationWindowKind::AgentChatDetached);
    agent_chat.semantic_surface = Some("agentChatChat".into());
    script_kit_gpui::windows::upsert_automation_window(agent_chat);

    // Resolve each by kind
    let resolved_main =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Main,
            index: None,
        }))
        .expect("resolve main");
    assert_eq!(
        resolved_main.semantic_surface.as_deref(),
        Some("scriptList")
    );

    let resolved_notes =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        }))
        .expect("resolve notes");
    assert_eq!(resolved_notes.semantic_surface.as_deref(), Some("notes"));

    let resolved_agent_chat =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::AgentChatDetached,
            index: None,
        }))
        .expect("resolve agent_chat");
    assert_eq!(
        resolved_agent_chat.semantic_surface.as_deref(),
        Some("agentChatChat")
    );

    // All three have distinct IDs
    assert_ne!(resolved_main.id, resolved_notes.id);
    assert_ne!(resolved_main.id, resolved_agent_chat.id);
    assert_ne!(resolved_notes.id, resolved_agent_chat.id);

    cleanup(&p, &["main", "notes", "agent_chat"]);
}

#[test]
fn focused_resolution_tracks_across_windows() {
    let _lock = crate::acquire_automation_test_lock();
    let p = prefix();

    let mut main = make_info(&p, "main", AutomationWindowKind::Main);
    main.focused = true;
    script_kit_gpui::windows::upsert_automation_window(main);

    let notes = make_info(&p, "notes", AutomationWindowKind::Notes);
    script_kit_gpui::windows::upsert_automation_window(notes);

    // Initially main is focused
    let focused =
        script_kit_gpui::windows::resolve_automation_window(None).expect("resolve focused");
    assert_eq!(focused.kind, AutomationWindowKind::Main);

    // Shift focus to notes
    script_kit_gpui::windows::set_automation_focus(&format!("{p}:notes"));

    let focused = script_kit_gpui::windows::resolve_automation_window(None)
        .expect("resolve focused after shift");
    assert_eq!(focused.kind, AutomationWindowKind::Notes);

    cleanup(&p, &["main", "notes"]);
}

// ============================================================
// Result message serde
// ============================================================

#[test]
fn simulate_gpui_event_result_round_trip() {
    let success = Message::simulate_gpui_event_result_success(
        "res-1".into(),
        None,
        None,
        true,
        false,
        Some("not_observed".into()),
    );
    let json = serde_json::to_string(&success).expect("serialize");
    assert!(json.contains(r#""success":true"#));

    let error = Message::simulate_gpui_event_result_error(
        "res-2".into(),
        "target_not_found".into(),
        "Window not found".into(),
        None,
        None,
    );
    let json = serde_json::to_string(&error).expect("serialize");
    assert!(json.contains(r#""success":false"#));
    assert!(json.contains(r#""errorCode":"target_not_found""#));
    assert!(json.contains("Window not found"));
}

// ============================================================
// Legacy simulateKey backward compat
// ============================================================

#[test]
fn legacy_simulate_key_still_parses() {
    let json = r#"{"type":"simulateKey","key":"enter","modifiers":["cmd"]}"#;
    let cmd: script_kit_gpui::stdin_commands::ExternalCommand =
        serde_json::from_str(json).expect("parse legacy simulateKey");
    assert_eq!(cmd.command_type(), "simulateKey");
}

// ============================================================
// ListAutomationWindows
// ============================================================

#[test]
fn list_automation_windows_returns_all_registered() {
    let p = prefix();

    script_kit_gpui::windows::upsert_automation_window(make_info(
        &p,
        "main",
        AutomationWindowKind::Main,
    ));
    script_kit_gpui::windows::upsert_automation_window(make_info(
        &p,
        "notes",
        AutomationWindowKind::Notes,
    ));
    script_kit_gpui::windows::upsert_automation_window(make_info(
        &p,
        "agent_chat",
        AutomationWindowKind::AgentChatDetached,
    ));

    let all = script_kit_gpui::windows::list_automation_windows();
    let ours: Vec<_> = all.iter().filter(|w| w.id.starts_with(&p)).collect();
    assert_eq!(ours.len(), 3);

    // Verify distinct kinds
    let kinds: std::collections::HashSet<_> = ours.iter().map(|w| w.kind).collect();
    assert!(kinds.contains(&AutomationWindowKind::Main));
    assert!(kinds.contains(&AutomationWindowKind::Notes));
    assert!(kinds.contains(&AutomationWindowKind::AgentChatDetached));

    cleanup(&p, &["main", "notes", "agent_chat"]);
}

// ============================================================
// Constructor: simulate_gpui_event with target
// ============================================================

#[test]
fn simulate_gpui_event_constructor_includes_target() {
    let msg = Message::simulate_gpui_event(
        "evt-target-1".into(),
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
    assert!(json.contains("simulateGpuiEvent"));
    assert!(json.contains("agentChatDetached"));
    assert!(json.contains("evt-target-1"));
}

// ============================================================
// Window bounds round-trip through registry
// ============================================================

#[test]
fn window_bounds_survive_registry_round_trip() {
    let _lock = crate::acquire_automation_test_lock();
    let p = prefix();

    let info = AutomationWindowInfo {
        id: format!("{p}:bounded"),
        kind: AutomationWindowKind::Main,
        title: Some("Bounded".into()),
        focused: true,
        visible: true,
        semantic_surface: None,
        bounds: Some(AutomationWindowBounds {
            x: 50.0,
            y: 100.0,
            width: 1200.0,
            height: 800.0,
        }),
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(info);

    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:bounded"),
        }))
        .expect("resolve bounded");
    let bounds = resolved.bounds.expect("bounds should be present");
    assert!((bounds.width - 1200.0).abs() < f64::EPSILON);
    assert!((bounds.height - 800.0).abs() < f64::EPSILON);

    script_kit_gpui::windows::remove_automation_window(&format!("{p}:bounded"));
}

// ============================================================
// Machine-readable error codes — distinguishability
// ============================================================

#[test]
fn error_codes_are_distinct_and_machine_parseable() {
    // target_not_found
    let not_found = Message::simulate_gpui_event_result_error(
        "err-1".into(),
        "target_not_found".into(),
        "No focused automation window".into(),
        None,
        None,
    );
    let json = serde_json::to_value(&not_found).expect("serialize");
    assert_eq!(json["errorCode"], "target_not_found");
    assert_eq!(json["success"], false);

    // target_ambiguous
    let ambiguous = Message::simulate_gpui_event_result_error(
        "err-2".into(),
        "target_ambiguous".into(),
        "2 visible windows share this kind".into(),
        None,
        None,
    );
    let json = serde_json::to_value(&ambiguous).expect("serialize");
    assert_eq!(json["errorCode"], "target_ambiguous");

    // handle_unavailable
    let no_handle = Message::simulate_gpui_event_result_error(
        "err-3".into(),
        "handle_unavailable".into(),
        "Window handle not available for role Main".into(),
        None,
        None,
    );
    let json = serde_json::to_value(&no_handle).expect("serialize");
    assert_eq!(json["errorCode"], "handle_unavailable");

    // dispatch_failed
    let dispatch = Message::simulate_gpui_event_result_error(
        "err-4".into(),
        "dispatch_failed".into(),
        "GPUI dispatch failed: window closed".into(),
        None,
        None,
    );
    let json = serde_json::to_value(&dispatch).expect("serialize");
    assert_eq!(json["errorCode"], "dispatch_failed");

    // Success has no errorCode
    let success = Message::simulate_gpui_event_result_success(
        "ok-1".into(),
        None,
        None,
        true,
        false,
        Some("not_observed".into()),
    );
    let json = serde_json::to_value(&success).expect("serialize");
    assert!(json.get("errorCode").is_none());
    assert_eq!(json["dispatchCompleted"], true);
    assert_eq!(json["dispatchScheduled"], false);
    assert_eq!(json["activationProof"], "not_observed");
}

#[test]
fn error_result_round_trips_with_error_code() {
    let msg = Message::simulate_gpui_event_result_error(
        "rt-1".into(),
        "target_ambiguous".into(),
        "2 visible windows share kind AgentChatDetached".into(),
        None,
        Some("agent_chat-thread-1".into()),
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    match back {
        Message::SimulateGpuiEventResult {
            request_id,
            success,
            error_code,
            error,
            resolved_window_id,
            ..
        } => {
            assert_eq!(request_id, "rt-1");
            assert!(!success);
            assert_eq!(error_code.as_deref(), Some("target_ambiguous"));
            assert!(error.unwrap().contains("2 visible windows"));
            assert_eq!(resolved_window_id.as_deref(), Some("agent_chat-thread-1"));
        }
        other => panic!("Expected SimulateGpuiEventResult, got: {:?}", other),
    }
}
