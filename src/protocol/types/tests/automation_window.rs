use super::*;

// ============================================================
// AutomationWindowTarget serde round-trips
// ============================================================

#[test]
fn automation_window_target_round_trip_main() {
    let target = AutomationWindowTarget::Main;
    let json = serde_json::to_string(&target).expect("serialize");
    let back: AutomationWindowTarget = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, target);
    assert!(json.contains(r#""type":"main"#));
}

#[test]
fn automation_window_target_round_trip_focused() {
    let target = AutomationWindowTarget::Focused;
    let json = serde_json::to_string(&target).expect("serialize");
    let back: AutomationWindowTarget = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, target);
}

#[test]
fn automation_window_target_round_trip_id() {
    let target = AutomationWindowTarget::Id {
        id: "notes:primary".into(),
    };
    let json = serde_json::to_string(&target).expect("serialize");
    let back: AutomationWindowTarget = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, target);
}

#[test]
fn automation_window_target_round_trip_instance() {
    let target = AutomationWindowTarget::Instance {
        id: "dictation-microphone-popup".into(),
        generation: 42,
    };
    let json = serde_json::to_string(&target).expect("serialize");
    assert_eq!(
        json,
        r#"{"type":"instance","id":"dictation-microphone-popup","generation":42}"#
    );
    let back: AutomationWindowTarget = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, target);
}

#[test]
fn automation_window_target_instance_requires_generation() {
    let error = serde_json::from_str::<AutomationWindowTarget>(
        r#"{"type":"instance","id":"dictation-microphone-popup"}"#,
    )
    .expect_err("instance target without generation must fail closed");
    assert!(error.to_string().contains("generation"));
}

#[test]
fn automation_window_target_round_trip_kind() {
    let json = r#"{"type":"kind","kind":"agentChatDetached","index":0}"#;
    let parsed: AutomationWindowTarget =
        serde_json::from_str(json).expect("target should deserialize");
    match &parsed {
        AutomationWindowTarget::Kind { kind, index } => {
            assert_eq!(*kind, AutomationWindowKind::AgentChatDetached);
            assert_eq!(*index, Some(0));
        }
        other => panic!("Expected Kind, got: {:?}", other),
    }
    let out = serde_json::to_string(&parsed).expect("target should serialize");
    let back: AutomationWindowTarget = serde_json::from_str(&out).expect("roundtrip");
    assert_eq!(back, parsed);
}

#[test]
fn automation_window_target_kind_without_index() {
    let json = r#"{"type":"kind","kind":"notes"}"#;
    let parsed: AutomationWindowTarget = serde_json::from_str(json).expect("parse");
    match &parsed {
        AutomationWindowTarget::Kind { kind, index } => {
            assert_eq!(*kind, AutomationWindowKind::Notes);
            assert_eq!(*index, None);
        }
        other => panic!("Expected Kind, got: {:?}", other),
    }
}

#[test]
fn automation_window_target_title_contains() {
    let target = AutomationWindowTarget::TitleContains {
        text: "Script Kit".into(),
    };
    let json = serde_json::to_string(&target).expect("serialize");
    let back: AutomationWindowTarget = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, target);
}

// ============================================================
// AutomationWindowKind exhaustive serde
// ============================================================

#[test]
fn automation_window_kind_all_variants_round_trip() {
    let kinds = vec![
        (AutomationWindowKind::Main, "main"),
        (AutomationWindowKind::Notes, "notes"),
        (AutomationWindowKind::AgentChatDetached, "agentChatDetached"),
        (AutomationWindowKind::Dictation, "dictation"),
        (AutomationWindowKind::ActionsDialog, "actionsDialog"),
        (AutomationWindowKind::PromptPopup, "promptPopup"),
        (AutomationWindowKind::Hud, "hud"),
    ];
    for (kind, expected_str) in kinds {
        let json = serde_json::to_string(&kind).expect("serialize kind");
        assert_eq!(json, format!("\"{}\"", expected_str), "kind {:?}", kind);
        let back: AutomationWindowKind = serde_json::from_str(&json).expect("deserialize kind");
        assert_eq!(back, kind);
    }
}

// ============================================================
// AutomationWindowInfo serde
// ============================================================

#[test]
fn automation_window_info_round_trip() {
    let info = AutomationWindowInfo {
        id: "notes:primary".into(),
        kind: AutomationWindowKind::Notes,
        title: Some("Script Kit Notes".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("notes".into()),
        bounds: None,
        parent_window_id: None,
        parent_kind: None,
        pid: Some(1234),
        generation: None,
    };
    let json = serde_json::to_string(&info).expect("serialize");
    let back: AutomationWindowInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, info);
}

#[test]
fn automation_window_generation_round_trips_serde() {
    let info = AutomationWindowInfo {
        id: "agent_chat-history-popup".into(),
        kind: AutomationWindowKind::PromptPopup,
        title: Some("History".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("promptPopup".into()),
        bounds: None,
        parent_window_id: Some("main".into()),
        parent_kind: Some(AutomationWindowKind::Main),
        pid: Some(1234),
        generation: Some(17),
    };
    let json = serde_json::to_string(&info).expect("serialize");
    assert!(json.contains(r#""generation":17"#));
    let back: AutomationWindowInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, info);
}

#[test]
fn automation_window_info_minimal_fields() {
    let json = r#"{"id":"main","kind":"main","focused":false,"visible":true}"#;
    let info: AutomationWindowInfo = serde_json::from_str(json).expect("parse minimal");
    assert_eq!(info.id, "main");
    assert_eq!(info.kind, AutomationWindowKind::Main);
    assert!(!info.focused);
    assert!(info.visible);
    assert!(info.title.is_none());
    assert!(info.semantic_surface.is_none());
    assert!(info.bounds.is_none());
}

#[test]
fn automation_window_info_with_bounds() {
    let info = AutomationWindowInfo {
        id: "main".into(),
        kind: AutomationWindowKind::Main,
        title: Some("Script Kit".into()),
        focused: true,
        visible: true,
        semantic_surface: None,
        bounds: Some(AutomationWindowBounds {
            x: 100.0,
            y: 200.0,
            width: 800.0,
            height: 600.0,
        }),
        parent_window_id: None,
        parent_kind: None,
        pid: Some(1234),
        generation: None,
    };
    let json = serde_json::to_string(&info).expect("serialize");
    assert!(json.contains("\"bounds\""));
    let back: AutomationWindowInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.bounds.as_ref().expect("bounds").width, 800.0);
}

// ============================================================
// SimulatedGpuiEvent serde
// ============================================================

#[test]
fn simulate_gpui_event_key_down_round_trip() {
    let event = SimulatedGpuiEvent::KeyDown {
        key: "k".into(),
        modifiers: vec![crate::stdin_commands::KeyModifier::Cmd],
        text: None,
    };
    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains(r#""type":"keyDown"#));
    assert!(json.contains(r#""key":"k"#));
    let back: SimulatedGpuiEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, event);
}

#[test]
fn simulate_gpui_event_mouse_move_round_trip() {
    let event = SimulatedGpuiEvent::MouseMove { x: 10.5, y: 20.0 };
    let json = serde_json::to_string(&event).expect("serialize");
    let back: SimulatedGpuiEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, event);
}

#[test]
fn simulate_gpui_event_mouse_down_round_trip() {
    let event = SimulatedGpuiEvent::MouseDown {
        x: 50.0,
        y: 100.0,
        button: Some("right".into()),
    };
    let json = serde_json::to_string(&event).expect("serialize");
    let back: SimulatedGpuiEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, event);
}

#[test]
fn simulate_gpui_event_mouse_click_round_trip() {
    let event = SimulatedGpuiEvent::MouseClick {
        x: 50.0,
        y: 100.0,
        button: Some("left".into()),
    };
    let json = serde_json::to_string(&event).expect("serialize");
    assert!(json.contains(r#""type":"mouseClick"#));
    let back: SimulatedGpuiEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, event);
}

#[test]
fn simulated_touch_phase_all_variants_round_trip() {
    for (phase, expected) in [
        (SimulatedTouchPhase::Started, "started"),
        (SimulatedTouchPhase::Moved, "moved"),
        (SimulatedTouchPhase::Ended, "ended"),
    ] {
        let json = serde_json::to_string(&phase).expect("serialize touch phase");
        assert_eq!(json, format!("\"{expected}\""));
        let back: SimulatedTouchPhase =
            serde_json::from_str(&json).expect("deserialize touch phase");
        assert_eq!(back, phase);
    }
}

#[test]
fn simulate_gpui_event_scroll_wheel_round_trip_uses_pixel_schema() {
    let event = SimulatedGpuiEvent::ScrollWheel {
        x: 120.5,
        y: 240.25,
        delta_x: -3.0,
        delta_y: 18.5,
        phase: SimulatedTouchPhase::Moved,
        direct_phase: None,
        momentum_phase: None,
        timestamp_seconds: None,
    };
    let value = serde_json::to_value(&event).expect("serialize scroll wheel");
    assert_eq!(
        value,
        serde_json::json!({
            "type": "scrollWheel",
            "x": 120.5,
            "y": 240.25,
            "deltaX": -3.0,
            "deltaY": 18.5,
            "phase": "moved"
        })
    );
    let back: SimulatedGpuiEvent = serde_json::from_value(value).expect("deserialize scroll wheel");
    assert_eq!(back, event);
}

#[test]
fn simulate_gpui_event_scroll_wheel_preserves_native_lifecycle_and_timestamp() {
    use crate::protocol::SimulatedScrollPhase;

    let value = serde_json::json!({
        "type": "scrollWheel",
        "x": 10.0,
        "y": 20.0,
        "deltaX": 0.0,
        "deltaY": 4.0,
        "phase": "moved",
        "directPhase": "none",
        "momentumPhase": "changed",
        "timestampSeconds": 42.125
    });
    let event: SimulatedGpuiEvent =
        serde_json::from_value(value.clone()).expect("deserialize rich scroll lifecycle");
    assert_eq!(
        event,
        SimulatedGpuiEvent::ScrollWheel {
            x: 10.0,
            y: 20.0,
            delta_x: 0.0,
            delta_y: 4.0,
            phase: SimulatedTouchPhase::Moved,
            direct_phase: Some(SimulatedScrollPhase::None),
            momentum_phase: Some(SimulatedScrollPhase::Changed),
            timestamp_seconds: Some(42.125),
        }
    );
    assert_eq!(
        serde_json::to_value(event).expect("serialize rich lifecycle"),
        value
    );
}

#[test]
fn simulate_gpui_event_scroll_wheel_request_round_trip() {
    let json = r#"{
        "type": "simulateGpuiEvent",
        "requestId": "gpui-scroll-1",
        "target": {"type": "main"},
        "event": {
            "type": "scrollWheel",
            "x": 160.0,
            "y": 220.0,
            "deltaX": 0.0,
            "deltaY": -24.0,
            "phase": "started"
        }
    }"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("parse scroll-wheel simulateGpuiEvent");
    match msg {
        crate::protocol::Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
        } => {
            assert_eq!(request_id, "gpui-scroll-1");
            assert_eq!(target, Some(AutomationWindowTarget::Main));
            assert_eq!(
                event,
                SimulatedGpuiEvent::ScrollWheel {
                    x: 160.0,
                    y: 220.0,
                    delta_x: 0.0,
                    delta_y: -24.0,
                    phase: SimulatedTouchPhase::Started,
                    direct_phase: None,
                    momentum_phase: None,
                    timestamp_seconds: None,
                }
            );
        }
        other => panic!("Expected SimulateGpuiEvent, got: {other:?}"),
    }
}

#[test]
fn simulate_gpui_event_request_round_trip() {
    let json = r#"{
        "type": "simulateGpuiEvent",
        "requestId": "gpui-1",
        "target": {"type": "kind", "kind": "agentChatDetached"},
        "event": {"type": "keyDown", "key": "k", "modifiers": ["cmd"]}
    }"#;
    let msg: crate::protocol::Message =
        serde_json::from_str(json).expect("parse simulateGpuiEvent");
    match msg {
        crate::protocol::Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
        } => {
            assert_eq!(request_id, "gpui-1");
            assert!(target.is_some());
            match event {
                SimulatedGpuiEvent::KeyDown { key, modifiers, .. } => {
                    assert_eq!(key, "k");
                    assert_eq!(modifiers.len(), 1);
                }
                other => panic!("Expected KeyDown, got: {:?}", other),
            }
        }
        other => panic!("Expected SimulateGpuiEvent, got: {:?}", other),
    }
}

// ============================================================
// ListAutomationWindows serde
// ============================================================

#[test]
fn list_automation_windows_request_round_trip() {
    let json = r#"{"type":"listAutomationWindows","requestId":"wins-1"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::ListAutomationWindows { request_id } => {
            assert_eq!(request_id, "wins-1");
        }
        other => panic!("Expected ListAutomationWindows, got: {:?}", other),
    }
}

#[test]
fn automation_window_list_result_round_trip() {
    let msg = crate::protocol::Message::automation_window_list_result(
        "wins-1".into(),
        vec![
            AutomationWindowInfo {
                id: "main".into(),
                kind: AutomationWindowKind::Main,
                title: Some("Script Kit".into()),
                focused: false,
                visible: true,
                semantic_surface: Some("scriptList".into()),
                bounds: None,
                parent_window_id: None,
                parent_kind: None,
                pid: Some(1001),
                generation: None,
            },
            AutomationWindowInfo {
                id: "agentChatDetached:thread-1".into(),
                kind: AutomationWindowKind::AgentChatDetached,
                title: Some("Script Kit Agent Chat".into()),
                focused: true,
                visible: true,
                semantic_surface: Some("agentChatChat".into()),
                bounds: None,
                parent_window_id: None,
                parent_kind: None,
                pid: Some(1001),
                generation: None,
            },
        ],
        Some("agentChatDetached:thread-1".into()),
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("automationWindowListResult"));
    assert!(json.contains("focusedWindowId"));
    let back: crate::protocol::Message = serde_json::from_str(&json).expect("deserialize");
    match back {
        crate::protocol::Message::AutomationWindowListResult {
            windows,
            focused_window_id,
            ..
        } => {
            assert_eq!(windows.len(), 2);
            assert_eq!(
                focused_window_id.as_deref(),
                Some("agentChatDetached:thread-1")
            );
        }
        other => panic!("Expected AutomationWindowListResult, got: {:?}", other),
    }
}

// ============================================================
// Backward compatibility: target omitted
// ============================================================

#[test]
fn legacy_get_elements_request_still_parses_without_target() {
    let json = r#"{"type":"getElements","requestId":"elm-1"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::GetElements {
            request_id,
            limit,
            target,
            ..
        } => {
            assert_eq!(request_id, "elm-1");
            assert_eq!(limit, None);
            assert!(target.is_none(), "target should default to None");
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn legacy_get_state_request_still_parses_without_target() {
    let json = r#"{"type":"getState","requestId":"gs-1"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::GetState {
            request_id, target, ..
        } => {
            assert_eq!(request_id, "gs-1");
            assert!(target.is_none());
        }
        other => panic!("Expected GetState, got: {:?}", other),
    }
}

#[test]
fn legacy_capture_screenshot_still_parses_without_target() {
    let json = r#"{"type":"captureScreenshot","requestId":"shot-1"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::CaptureScreenshot { target, .. } => {
            assert!(target.is_none());
        }
        other => panic!("Expected CaptureScreenshot, got: {:?}", other),
    }
}

#[test]
fn legacy_wait_for_still_parses_without_target() {
    let json = r#"{"type":"waitFor","requestId":"wf-1","condition":"windowVisible"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::WaitFor { target, .. } => {
            assert!(target.is_none());
        }
        other => panic!("Expected WaitFor, got: {:?}", other),
    }
}

#[test]
fn legacy_batch_still_parses_without_target() {
    let json = r#"{"type":"batch","requestId":"b-1","commands":[]}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::Batch { target, .. } => {
            assert!(target.is_none());
        }
        other => panic!("Expected Batch, got: {:?}", other),
    }
}

#[test]
fn legacy_get_agent_chat_state_still_parses_without_target() {
    let json = r#"{"type":"getAgentChatState","requestId":"agent_chat-1"}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::GetAgentChatState { target, .. } => {
            assert!(target.is_none());
        }
        other => panic!("Expected GetAgentChatState, got: {:?}", other),
    }
}

#[test]
fn legacy_simulate_click_still_parses_without_target() {
    let json = r#"{"type":"simulateClick","requestId":"sc-1","x":10,"y":20}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::SimulateClick { target, .. } => {
            assert!(target.is_none());
        }
        other => panic!("Expected SimulateClick, got: {:?}", other),
    }
}

// ============================================================
// Targeted requests: target included
// ============================================================

#[test]
fn get_elements_with_target_parses() {
    let json = r#"{"type":"getElements","requestId":"elm-notes","target":{"type":"kind","kind":"notes"},"limit":50}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::GetElements {
            request_id,
            limit,
            target,
            ..
        } => {
            assert_eq!(request_id, "elm-notes");
            assert_eq!(limit, Some(50));
            let t = target.expect("target should be present");
            match t {
                AutomationWindowTarget::Kind { kind, index } => {
                    assert_eq!(kind, AutomationWindowKind::Notes);
                    assert_eq!(index, None);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn capture_screenshot_with_target_parses() {
    let json = r#"{"type":"captureScreenshot","requestId":"shot-agent_chat","target":{"type":"kind","kind":"agentChatDetached","index":0},"hiDpi":true}"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    match msg {
        crate::protocol::Message::CaptureScreenshot {
            request_id,
            hi_dpi,
            target,
        } => {
            assert_eq!(request_id, "shot-agent_chat");
            assert_eq!(hi_dpi, Some(true));
            assert!(target.is_some());
        }
        other => panic!("Expected CaptureScreenshot, got: {:?}", other),
    }
}

#[test]
fn inspect_automation_window_request_round_trip_and_request_id() {
    let json = r#"{
        "type":"inspectAutomationWindow",
        "requestId":"inspect-1",
        "target":{"type":"kind","kind":"agentChatDetached","index":0},
        "hiDpi":true,
        "probes":[{"x":24,"y":24},{"x":320,"y":180}]
    }"#;
    let msg: crate::protocol::Message = serde_json::from_str(json).expect("parse");
    assert_eq!(msg.request_id(), Some("inspect-1"));
    match msg {
        crate::protocol::Message::InspectAutomationWindow {
            request_id,
            target,
            hi_dpi,
            probes,
        } => {
            assert_eq!(request_id, "inspect-1");
            assert_eq!(hi_dpi, Some(true));
            assert_eq!(probes.len(), 2);
            match target.expect("target should be present") {
                AutomationWindowTarget::Kind { kind, index } => {
                    assert_eq!(kind, AutomationWindowKind::AgentChatDetached);
                    assert_eq!(index, Some(0));
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected InspectAutomationWindow, got: {:?}", other),
    }
}

#[test]
fn automation_inspect_result_round_trip_and_request_id() {
    let msg = crate::protocol::Message::automation_inspect_result(
        "inspect-1".into(),
        crate::protocol::AutomationInspectSnapshot {
            schema_version: crate::protocol::AUTOMATION_INSPECT_SCHEMA_VERSION,
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
            pixel_probes: vec![crate::protocol::PixelProbeResult {
                x: 24,
                y: 24,
                r: 28,
                g: 28,
                b: 30,
                a: 255,
            }],
            os_window_id: Some(99),
            semantic_quality: Some(crate::protocol::SemanticQuality::PanelOnly),
            warnings: vec!["panel_only_agent_chat_detached".into()],
            pid: Some(1234),
        },
    );
    assert_eq!(msg.request_id(), Some("inspect-1"));
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("automationInspectResult"));
    let back: crate::protocol::Message = serde_json::from_str(&json).expect("deserialize");
    match back {
        crate::protocol::Message::AutomationInspectResult {
            request_id,
            snapshot,
        } => {
            assert_eq!(request_id, "inspect-1");
            assert_eq!(snapshot.window_id, "agentChatDetached:thread-1");
            assert_eq!(snapshot.screenshot_width, Some(1440));
            assert_eq!(snapshot.pixel_probes.len(), 1);
        }
        other => panic!("Expected AutomationInspectResult, got: {:?}", other),
    }
}

// ============================================================
// Constructor tests
// ============================================================

#[test]
fn list_automation_windows_constructor() {
    let msg = crate::protocol::Message::list_automation_windows("test-1".into());
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("listAutomationWindows"));
    assert!(json.contains("test-1"));
}

#[test]
fn simulate_gpui_event_constructor() {
    let msg = crate::protocol::Message::simulate_gpui_event(
        "evt-1".into(),
        SimulatedGpuiEvent::KeyDown {
            key: "enter".into(),
            modifiers: vec![],
            text: None,
        },
        Some(AutomationWindowTarget::Main),
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("simulateGpuiEvent"));
    assert!(json.contains("enter"));
}

#[test]
fn simulate_gpui_event_result_success_constructor() {
    let msg = crate::protocol::Message::simulate_gpui_event_result_success(
        "evt-1".into(),
        Some("exact_handle".into()),
        Some("win-42".into()),
        true,
        false,
        Some("not_observed".into()),
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains(r#""success":true"#));
    assert!(json.contains(r#""dispatchPath":"exact_handle""#));
    assert!(json.contains(r#""resolvedWindowId":"win-42""#));
    assert!(json.contains(r#""dispatchCompleted":true"#));
    assert!(json.contains(r#""dispatchScheduled":false"#));
    assert!(json.contains(r#""activationProof":"not_observed""#));
}

#[test]
fn simulate_gpui_event_result_success_omits_none_fields() {
    let msg = crate::protocol::Message::simulate_gpui_event_result_success(
        "evt-2".into(),
        None,
        None,
        true,
        false,
        Some("not_observed".into()),
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains(r#""success":true"#));
    assert!(!json.contains("dispatchPath"));
    assert!(!json.contains("resolvedWindowId"));
}

#[test]
fn simulate_gpui_event_result_error_constructor() {
    let msg = crate::protocol::Message::simulate_gpui_event_result_error(
        "evt-1".into(),
        "target_not_found".into(),
        "Window not found".into(),
        None,
        None,
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains(r#""success":false"#));
    assert!(json.contains(r#""errorCode":"target_not_found""#));
    assert!(json.contains("Window not found"));
}
