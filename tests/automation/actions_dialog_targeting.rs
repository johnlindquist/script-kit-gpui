//! Actions dialog targeting regression tests.
//!
//! The actions dialog is an attached popup (no independent window handle).
//! These tests verify that targeting it resolves through the registry and
//! that the protocol correctly handles the attached-popup pattern.

use script_kit_gpui::protocol::{
    AutomationInspectSnapshot, AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget,
    BatchCommand, Message, SemanticQuality, SimulatedGpuiEvent, AUTOMATION_INSPECT_SCHEMA_VERSION,
};
use std::sync::atomic::{AtomicU32, Ordering};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(40_000);
fn prefix() -> String {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("!ad{n}")
}

fn cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

#[test]
fn actions_dialog_registered_as_popup() {
    let p = "!0_actions_dialog_registered_as_popup".to_string();

    // Register main window
    let main = AutomationWindowInfo {
        id: format!("{p}:main"),
        kind: AutomationWindowKind::Main,
        title: Some("Script Kit".into()),
        focused: true,
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

    // Register actions dialog as ActionsDialog kind
    let actions = AutomationWindowInfo {
        id: format!("{p}:actions"),
        kind: AutomationWindowKind::ActionsDialog,
        title: Some("Actions".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("actionsDialog".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(actions);

    // Resolve actions dialog by kind
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::ActionsDialog,
            index: None,
        }))
        .expect("resolve actions dialog");
    assert_eq!(resolved.kind, AutomationWindowKind::ActionsDialog);
    assert_eq!(
        resolved.semantic_surface.as_deref(),
        Some("actionsDialog"),
        "Resolved window did not match expected. Resolved: {:?}",
        resolved
    );

    // Actions dialog is distinct from main
    let main_resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Main))
            .expect("resolve main");
    assert_ne!(resolved.id, main_resolved.id);

    cleanup(&p, &["main", "actions"]);
}

#[test]
fn actions_dialog_simulate_event_targets_correctly() {
    // Verify a simulateGpuiEvent targeting ActionsDialog parses correctly
    let json = r#"{
        "type": "simulateGpuiEvent",
        "requestId": "actions-sim-1",
        "target": {"type": "kind", "kind": "actionsDialog"},
        "event": {"type": "keyDown", "key": "escape", "modifiers": []}
    }"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
            ..
        } => {
            assert_eq!(request_id, "actions-sim-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::ActionsDialog);
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
fn actions_dialog_close_removes_from_listing() {
    let p = prefix();

    let actions = AutomationWindowInfo {
        id: format!("{p}:actions-close"),
        kind: AutomationWindowKind::ActionsDialog,
        title: Some("Actions".into()),
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
    script_kit_gpui::windows::upsert_automation_window(actions);

    // Should be in the list
    let all = script_kit_gpui::windows::list_automation_windows();
    assert!(all.iter().any(|w| w.id == format!("{p}:actions-close")));

    // Remove
    script_kit_gpui::windows::remove_automation_window(&format!("{p}:actions-close"));

    // No longer in the list
    let all = script_kit_gpui::windows::list_automation_windows();
    assert!(!all.iter().any(|w| w.id == format!("{p}:actions-close")));
}

#[test]
fn prompt_popup_kind_resolves_independently() {
    let p = prefix();

    let popup = AutomationWindowInfo {
        id: format!("{p}:popup-confirm"),
        kind: AutomationWindowKind::PromptPopup,
        title: Some("Confirm".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("confirmDialog".into()),
        bounds: None,
        parent_window_id: None,
        parent_window_generation: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(popup);

    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::PromptPopup,
            index: None,
        }))
        .expect("resolve prompt popup");
    assert_eq!(resolved.kind, AutomationWindowKind::PromptPopup);
    assert_eq!(resolved.semantic_surface.as_deref(), Some("confirmDialog"));

    cleanup(&p, &["popup-confirm"]);
}

#[test]
fn backward_compat_get_elements_without_target() {
    // Legacy getElements requests (no target field) should still parse
    let json = r#"{"type":"getElements","requestId":"legacy-1"}"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::GetElements { target, .. } => {
            assert!(target.is_none(), "target should default to None for legacy");
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn backward_compat_get_agent_chat_state_without_target() {
    let json = r#"{"type":"getAgentChatState","requestId":"legacy-agent_chat-1"}"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::GetAgentChatState { target, .. } => {
            assert!(target.is_none());
        }
        other => panic!("Expected GetAgentChatState, got: {:?}", other),
    }
}

#[test]
fn get_state_actions_dialog_target_round_trip() {
    // getState with an ActionsDialog target should parse correctly.
    let json = serde_json::json!({
        "type": "getState",
        "requestId": "gs-actions-1",
        "target": { "type": "kind", "kind": "actionsDialog" }
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::GetState {
            request_id, target, ..
        } => {
            assert_eq!(request_id, "gs-actions-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::ActionsDialog);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected GetState, got: {:?}", other),
    }
}

#[test]
fn get_layout_info_actions_dialog_target_round_trip() {
    // getLayoutInfo with an ActionsDialog target should parse correctly.
    let json = serde_json::json!({
        "type": "getLayoutInfo",
        "requestId": "li-actions-1",
        "target": { "type": "kind", "kind": "actionsDialog" }
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::GetLayoutInfo { request_id, target } => {
            assert_eq!(request_id, "li-actions-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::ActionsDialog);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected GetLayoutInfo, got: {:?}", other),
    }
}

#[test]
fn get_layout_info_actions_dialog_routes_to_runtime_layout_model() {
    let prompt_handler = include_str!("../../src/prompt_handler/mod.rs");
    let actions_dialog = include_str!("../../src/actions/dialog.rs");

    assert!(
        prompt_handler.contains("AutomationWindowKind::ActionsDialog")
            && prompt_handler.contains("automation_layout_info(&resolved)"),
        "getLayoutInfo must route ActionsDialog targets to the live runtime layout model"
    );
    for needle in [
        "pub(crate) fn automation_layout_info",
        "ActionsDialog",
        "ActionsSearchInput",
        "ActionsList",
        "ActionsRow[",
        "ActionsShortcut[",
        "runtime ActionsDialog row geometry",
    ] {
        assert!(
            actions_dialog.contains(needle),
            "ActionsDialog layout model must expose {needle}"
        );
    }
}

#[test]
fn get_elements_with_actions_dialog_target_round_trip() {
    // getElements with an ActionsDialog target should parse correctly.
    let json = serde_json::json!({
        "type": "getElements",
        "requestId": "ge-actions-1",
        "limit": 20,
        "target": { "type": "kind", "kind": "actionsDialog" }
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::GetElements {
            request_id,
            limit,
            target,
            ..
        } => {
            assert_eq!(request_id, "ge-actions-1");
            assert_eq!(limit, Some(20));
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::ActionsDialog);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn capture_screenshot_with_actions_dialog_target_round_trip() {
    // captureScreenshot with an ActionsDialog target should parse correctly.
    let json = serde_json::json!({
        "type": "captureScreenshot",
        "requestId": "ss-actions-1",
        "target": { "type": "kind", "kind": "actionsDialog" }
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::CaptureScreenshot {
            request_id, target, ..
        } => {
            assert_eq!(request_id, "ss-actions-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::ActionsDialog);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected CaptureScreenshot, got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Actions dialog semantic element contract tests
// ---------------------------------------------------------------------------

#[test]
fn actions_dialog_collector_has_search_input_element() {
    // Verify the collector source defines the search input semantic ID
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"input:actions-search\""),
        "Actions dialog collector must define input:actions-search element"
    );
}

#[test]
fn actions_dialog_collector_has_list_element() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"list:actions\""),
        "Actions dialog collector must define list:actions element"
    );
}

#[test]
fn actions_dialog_collector_emits_choice_elements() {
    // The collector must use choice:N:id format for individual action rows
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("ElementType::Choice"),
        "Actions dialog collector must emit Choice-typed elements for actions"
    );
}

#[test]
fn actions_dialog_fallback_preserves_panel_only_warning() {
    // When entity is unavailable, must still emit the panel_only warning
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel_only_actions_dialog\""),
        "Actions dialog must preserve panel_only_actions_dialog fallback warning"
    );
}

#[test]
fn prompt_popup_collector_tries_known_popup_types() {
    // The PromptPopup collector must try history, and confirm.
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("collect_history_popup_snapshot"),
        "PromptPopup collector must try history popup"
    );
    assert!(
        source.contains("collect_confirm_popup_snapshot"),
        "PromptPopup collector must try confirm popup"
    );
}

#[test]
fn prompt_popup_fallback_preserves_panel_only_warning() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel_only_prompt_popup\""),
        "PromptPopup must preserve panel_only_prompt_popup fallback warning"
    );
}

#[test]
fn confirm_popup_collector_has_button_elements() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"button:0:confirm\""),
        "Confirm popup must define button:0:confirm"
    );
    assert!(
        source.contains("\"button:1:cancel\""),
        "Confirm popup must define button:1:cancel"
    );
}

// Mention picker / composer picker is no longer used in prompt popup collector.

// ---------------------------------------------------------------------------
// Actions dialog batch mutation contract tests
// ---------------------------------------------------------------------------

#[test]
fn batch_with_actions_dialog_target_parses_correctly() {
    let json = serde_json::json!({
        "type": "batch",
        "requestId": "batch-ad-1",
        "target": {"type": "kind", "kind": "actionsDialog"},
        "commands": [
            {"type": "setInput", "text": "edit"},
            {"type": "selectByValue", "value": "edit-script", "submit": false}
        ],
        "options": {"stopOnError": true}
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::Batch {
            request_id,
            commands,
            target,
            ..
        } => {
            assert_eq!(request_id, "batch-ad-1");
            assert_eq!(commands.len(), 2);
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::ActionsDialog);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected Batch, got: {:?}", other),
    }
}

#[test]
fn batch_actions_dialog_select_by_semantic_id_parses() {
    let json = serde_json::json!({
        "type": "batch",
        "requestId": "batch-ad-sem",
        "target": {"type": "kind", "kind": "actionsDialog"},
        "commands": [
            {"type": "selectBySemanticId", "semanticId": "choice:0:edit-script", "submit": true}
        ]
    });
    let msg: Message = serde_json::from_value(json).expect("parse");
    match msg {
        Message::Batch { commands, .. } => {
            assert_eq!(commands.len(), 1);
            match &commands[0] {
                BatchCommand::SelectBySemanticId {
                    semantic_id,
                    submit,
                } => {
                    assert_eq!(semantic_id, "choice:0:edit-script");
                    assert!(*submit);
                }
                other => panic!("Expected SelectBySemanticId, got: {:?}", other),
            }
        }
        other => panic!("Expected Batch, got: {:?}", other),
    }
}

#[test]
fn actions_dialog_batch_handler_has_direct_mutation_path() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    // Must have direct entity mutation, not raw key injection
    assert!(
        source.contains("dialog.set_search_text("),
        "ActionsDialog batch must use direct set_search_text mutation, not key injection"
    );
    assert!(
        source.contains("dialog.select_action_by_id("),
        "ActionsDialog batch must use direct select_action_by_id mutation"
    );
    assert!(
        source.contains("dialog.select_action_by_semantic_id("),
        "ActionsDialog batch must use direct select_action_by_semantic_id mutation"
    );
}

#[test]
fn actions_dialog_batch_unsupported_commands_fail_closed_with_structured_error() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("UnsupportedCommand"),
        "ActionsDialog batch must use UnsupportedCommand error code for rejected commands"
    );
    assert!(
        source.contains("ActionsDialog batch supports:"),
        "ActionsDialog batch error must list supported commands"
    );
}

#[test]
fn prompt_popup_batch_target_fails_closed() {
    // PromptPopup should fail at resolve_automation_read_target (unsupported kind).
    // This proves it does NOT silently fall back to raw key injection.
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains(
            "supports Main, Ai, AgentChatDetached, Notes, ActionsDialog, and PromptPopup targets"
        ),
        "unsupported kind error message must list all supported targets including PromptPopup"
    );
}

// ---------------------------------------------------------------------------
// Attached popup parent identity contract tests
// ---------------------------------------------------------------------------

#[test]
fn actions_dialog_records_parent_identity_from_main() {
    let p = prefix();
    let main = AutomationWindowInfo {
        id: format!("{p}:main"),
        kind: AutomationWindowKind::Main,
        title: Some("Script Kit".into()),
        focused: true,
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
    script_kit_gpui::windows::register_attached_popup(
        format!("{p}:actions-parent"),
        AutomationWindowKind::ActionsDialog,
        Some("Actions".into()),
        Some("actionsDialog".into()),
        None,
        Some(&format!("{p}:main")),
    )
    .expect("should register with main parent");
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:actions-parent"),
        }))
        .expect("should resolve");
    assert_eq!(
        resolved.parent_window_id.as_deref(),
        Some(format!("{p}:main").as_str()),
    );
    assert_eq!(resolved.parent_kind, Some(AutomationWindowKind::Main));
    cleanup(&p, &["main", "actions-parent"]);
}

#[test]
fn actions_dialog_records_parent_identity_from_non_main_host() {
    let p = prefix();
    let agent_chat = AutomationWindowInfo {
        id: format!("{p}:agent_chat"),
        kind: AutomationWindowKind::AgentChatDetached,
        title: Some("Agent Chat".into()),
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
    script_kit_gpui::windows::register_attached_popup(
        format!("{p}:agent_chat-actions"),
        AutomationWindowKind::ActionsDialog,
        Some("Actions".into()),
        Some("actionsDialog".into()),
        None,
        Some(&format!("{p}:agent_chat")),
    )
    .expect("should register with Agent Chat parent");
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:agent_chat-actions"),
        }))
        .expect("should resolve");
    assert_eq!(
        resolved.parent_window_id.as_deref(),
        Some(format!("{p}:agent_chat").as_str()),
    );
    assert_eq!(
        resolved.parent_kind,
        Some(AutomationWindowKind::AgentChatDetached)
    );
    cleanup(&p, &["agent_chat", "agent_chat-actions"]);
}

#[test]
fn confirm_popup_records_parent_identity() {
    let p = prefix();
    let main = AutomationWindowInfo {
        id: format!("{p}:main"),
        kind: AutomationWindowKind::Main,
        title: Some("Script Kit".into()),
        focused: true,
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
    script_kit_gpui::windows::register_attached_popup(
        format!("{p}:confirm"),
        AutomationWindowKind::PromptPopup,
        Some("Confirm".into()),
        Some("confirmDialog".into()),
        None,
        Some(&format!("{p}:main")),
    )
    .expect("should register confirm with main parent");
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:confirm"),
        }))
        .expect("should resolve");
    assert_eq!(resolved.kind, AutomationWindowKind::PromptPopup);
    assert_eq!(
        resolved.parent_window_id.as_deref(),
        Some(format!("{p}:main").as_str())
    );
    assert_eq!(resolved.parent_kind, Some(AutomationWindowKind::Main));
    cleanup(&p, &["main", "confirm"]);
}

#[test]
fn attached_popup_parent_fails_closed_on_unknown_parent() {
    let p = prefix();
    let result = script_kit_gpui::windows::register_attached_popup(
        format!("{p}:orphan"),
        AutomationWindowKind::ActionsDialog,
        Some("Actions".into()),
        None,
        None,
        Some("nonexistent-parent-window"),
    );
    assert!(
        result.is_err(),
        "must fail closed when parent is not in registry"
    );
    assert!(
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Id {
            id: format!("{p}:orphan"),
        }))
        .is_err(),
        "orphan popup must not be in the registry"
    );
}

#[test]
fn parent_identity_round_trips_through_serde() {
    let info = AutomationWindowInfo {
        id: "test-popup".into(),
        kind: AutomationWindowKind::ActionsDialog,
        title: Some("Actions".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("actionsDialog".into()),
        bounds: None,
        parent_window_id: Some("main".into()),
        parent_window_generation: None,
        parent_kind: Some(AutomationWindowKind::Main),
        pid: None,
        generation: None,
    };
    let json = serde_json::to_string(&info).expect("serialize");
    assert!(json.contains("\"parentWindowId\":\"main\""));
    assert!(json.contains("\"parentKind\":\"main\""));
    let deserialized: AutomationWindowInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.parent_window_id.as_deref(), Some("main"));
    assert_eq!(deserialized.parent_kind, Some(AutomationWindowKind::Main));
}

#[test]
fn parent_identity_omitted_when_none_in_serde() {
    let info = AutomationWindowInfo {
        id: "test-no-parent".into(),
        kind: AutomationWindowKind::Main,
        title: None,
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
    let json = serde_json::to_string(&info).expect("serialize");
    assert!(!json.contains("parentWindowId"));
    assert!(!json.contains("parentKind"));
}

#[test]
fn open_actions_window_registers_in_automation_registry() {
    let source = include_str!("../../src/actions/window.rs");
    assert!(source.contains("register_attached_popup("));
    assert!(source.contains("\"actions-dialog\""));
}

#[test]
fn close_actions_window_unregisters_from_automation_registry() {
    let source = include_str!("../../src/actions/window.rs");
    assert!(source.contains("remove_automation_window(\"actions-dialog\")"));
}

#[test]
fn open_confirm_popup_registers_in_automation_registry() {
    let source = include_str!("../../src/confirm/window.rs");
    assert!(source.contains("register_attached_popup("));
    assert!(source.contains("\"confirm-popup\""));
}

#[test]
fn close_confirm_popup_unregisters_from_automation_registry() {
    let source = include_str!("../../src/confirm/window.rs");
    assert!(source.contains("CONFIRM_POPUP_AUTOMATION_ID"));
    assert!(source.contains("remove_automation_window(CONFIRM_POPUP_AUTOMATION_ID)"));
}

// ---------------------------------------------------------------------------
// Non-main semantic proof: popup inspect receipts
// ---------------------------------------------------------------------------

#[test]
fn actions_dialog_collector_never_returns_non_main_pending_warning() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel_only_actions_dialog\""),
        "ActionsDialog must use panel_only_actions_dialog fallback"
    );
}

#[test]
fn prompt_popup_collector_never_returns_non_main_pending_warning() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel_only_prompt_popup\""),
        "PromptPopup must use panel_only_prompt_popup fallback"
    );
}

#[test]
fn notes_collector_never_returns_non_main_pending_warning() {
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel_only_notes\""),
        "Notes must use panel_only_notes fallback"
    );
    // Must not use ? which returns None → triggers non_main_pending
    assert!(
        !source.contains("collect_notes_snapshot(resolved, cx)?"),
        "Notes collector must not use ? operator"
    );
}

#[test]
fn supported_non_main_kinds_all_have_panel_only_fallbacks() {
    // Every supported non-main kind must have a panel_only fallback, not None.
    let source = include_str!("../../src/windows/automation_surface_collector.rs");
    for (kind, expected_warning) in [
        ("Notes", "panel_only_notes"),
        ("AgentChatDetached", "panel_only_agent_chat_detached"),
        ("ActionsDialog", "panel_only_actions_dialog"),
        ("PromptPopup", "panel_only_prompt_popup"),
    ] {
        assert!(
            source.contains(&format!("\"{expected_warning}\"")),
            "{kind} must define {expected_warning} fallback warning"
        );
    }
}

#[test]
fn actions_dialog_inspect_result_with_panel_only_quality() {
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: "actionsDialog:0".into(),
        window_kind: "ActionsDialog".into(),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: Some("Actions".into()),
        resolved_bounds: None,
        target_bounds_in_screenshot: None,
        surface_hit_point: None,
        suggested_hit_points: Vec::new(),
        elements: Vec::new(),
        total_count: 0,
        focused_semantic_id: None,
        selected_semantic_id: None,
        screenshot_width: Some(800),
        screenshot_height: Some(600),
        pixel_probes: Vec::new(),
        os_window_id: None,
        semantic_quality: Some(SemanticQuality::PanelOnly),
        warnings: vec!["panel_only_actions_dialog".into()],
        pid: None,
    };
    let json = serde_json::to_value(&snapshot).expect("serialize");
    assert_eq!(json["semanticQuality"], "panel_only");
    assert_eq!(json["windowKind"], "ActionsDialog");
    assert_eq!(json["schemaVersion"], 4);
}

#[test]
fn prompt_popup_inspect_result_with_full_quality() {
    let snapshot = AutomationInspectSnapshot {
        schema_version: AUTOMATION_INSPECT_SCHEMA_VERSION,
        window_id: "promptPopup:0".into(),
        window_kind: "PromptPopup".into(),
        surface_kind: None,
        app_view_variant: None,
        native_footer_surface: None,
        target_generation: Some(1),
        surface_generation: Some(1),
        data_generation: Some(1),
        title: Some("Confirm".into()),
        resolved_bounds: None,
        target_bounds_in_screenshot: None,
        surface_hit_point: None,
        suggested_hit_points: Vec::new(),
        elements: vec![script_kit_gpui::protocol::ElementInfo {
            element_type: script_kit_gpui::protocol::ElementType::Button,
            text: Some("Confirm".into()),
            value: Some("confirm".into()),
            selected: None,
            focused: Some(true),
            index: Some(0),
            role: None,
            kind: None,
            source: None,
            source_name: None,
            selectable: None,
            status_kind: None,
            action_disabled: None,
            style: None,
            semantic_id: "button:0:confirm".into(),
        }],
        total_count: 1,
        focused_semantic_id: Some("button:0:confirm".into()),
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
    assert_eq!(json["focusedSemanticId"], "button:0:confirm");
}

#[test]
fn backward_compat_inspect_receipt_without_semantic_quality() {
    // v2 callers (no semanticQuality) must still parse correctly
    let json = r#"{
        "schemaVersion": 2,
        "windowId": "actionsDialog:0",
        "windowKind": "ActionsDialog",
        "totalCount": 0
    }"#;
    let parsed: AutomationInspectSnapshot =
        serde_json::from_str(json).expect("should parse without semanticQuality");
    assert_eq!(parsed.semantic_quality, None);
    assert_eq!(parsed.window_id, "actionsDialog:0");
}

#[test]
fn attached_popup_parent_resolved_log_is_emitted() {
    let source = include_str!("../../src/windows/automation_registry.rs");
    assert!(source.contains("\"automation.attached_popup_parent_resolved\""));
    assert!(source.contains("popup_window_id"));
    assert!(source.contains("popup_kind"));
    assert!(source.contains("parent_window_id"));
    assert!(source.contains("parent_kind"));
}
