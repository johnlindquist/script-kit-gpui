//! Notes window targeting regression tests.
//!
//! Proves that the automation registry resolves Notes as a distinct
//! window, not the main window, and that Notes-specific metadata
//! is preserved across register/resolve/unregister cycles.

use script_kit_gpui::protocol::{
    AutomationWindowInfo, AutomationWindowKind, AutomationWindowTarget, Message,
};
use std::sync::atomic::{AtomicU32, Ordering};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(20_000);
fn prefix() -> String {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("!nt{n}")
}

fn cleanup(prefix: &str, ids: &[&str]) {
    for id in ids {
        script_kit_gpui::windows::remove_automation_window(&format!("{prefix}:{id}"));
    }
}

#[test]
fn notes_window_targeting_flow() {
    let p = prefix();

    // Register main window as focused
    let main = AutomationWindowInfo {
        id: format!("{p}:main"),
        kind: AutomationWindowKind::Main,
        title: Some("Script Kit".into()),
        focused: true,
        visible: true,
        semantic_surface: Some("scriptList".into()),
        bounds: None,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(main);

    // Register Notes window
    let notes = AutomationWindowInfo {
        id: format!("{p}:notes"),
        kind: AutomationWindowKind::Notes,
        title: Some("Script Kit Notes".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("notes".into()),
        bounds: None,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(notes);

    // Target by kind → Notes, not Main
    let resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        }))
        .expect("should resolve Notes");
    assert_eq!(resolved.kind, AutomationWindowKind::Notes);
    assert_eq!(resolved.semantic_surface.as_deref(), Some("notes"));
    assert_eq!(resolved.title.as_deref(), Some("Script Kit Notes"));
    assert_ne!(
        resolved.id,
        format!("{p}:main"),
        "must not fall back to main"
    );

    // Target by title → Notes
    let resolved_title = script_kit_gpui::windows::resolve_automation_window(Some(
        &AutomationWindowTarget::TitleContains {
            text: "Notes".into(),
        },
    ))
    .expect("should resolve by title");
    assert_eq!(resolved_title.kind, AutomationWindowKind::Notes);

    // No target (None) → focused window (Main)
    let focused =
        script_kit_gpui::windows::resolve_automation_window(None).expect("should resolve focused");
    assert_eq!(focused.kind, AutomationWindowKind::Main);

    // Close Notes → should disappear from registry
    let removed = script_kit_gpui::windows::remove_automation_window(&format!("{p}:notes"));
    assert!(removed.is_some());

    // Notes targeting now fails
    let err =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        }));
    assert!(err.is_err(), "Notes should no longer be resolvable");

    cleanup(&p, &["main"]);
}

#[test]
fn notes_window_info_serde_round_trip() {
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
        pid: None,
        generation: None,
    };
    let json = serde_json::to_string(&info).expect("serialize");
    let back: AutomationWindowInfo = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, info);
    assert!(json.contains(r#""kind":"notes"#));
}

#[test]
fn notes_focus_transfer_from_main() {
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
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(main);

    let notes = AutomationWindowInfo {
        id: format!("{p}:notes"),
        kind: AutomationWindowKind::Notes,
        title: Some("Script Kit Notes".into()),
        focused: false,
        visible: true,
        semantic_surface: Some("notes".into()),
        bounds: None,
        parent_window_id: None,
        parent_kind: None,
        pid: None,
        generation: None,
    };
    script_kit_gpui::windows::upsert_automation_window(notes);

    // Transfer focus to Notes
    assert!(script_kit_gpui::windows::set_automation_focus(&format!(
        "{p}:notes"
    )));

    // Focused resolution now returns Notes
    let focused =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Focused))
            .expect("resolve focused");
    assert_eq!(focused.kind, AutomationWindowKind::Notes);

    // Main should be unfocused
    let main_resolved =
        script_kit_gpui::windows::resolve_automation_window(Some(&AutomationWindowTarget::Main))
            .expect("resolve main");
    assert!(!main_resolved.focused);

    cleanup(&p, &["main", "notes"]);
}

#[test]
fn get_state_notes_target_round_trip() {
    // getState with a Notes target should parse and round-trip correctly.
    let json = serde_json::json!({
        "type": "getState",
        "requestId": "gs-notes-1",
        "target": { "type": "kind", "kind": "notes" }
    });
    let msg: Message = serde_json::from_value(json).expect("parse getState with notes target");
    match msg {
        Message::GetState {
            request_id, target, ..
        } => {
            assert_eq!(request_id, "gs-notes-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::Notes);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected GetState, got: {:?}", other),
    }
}

#[test]
fn get_layout_info_notes_target_round_trip() {
    // getLayoutInfo with a Notes target should parse and round-trip correctly.
    let msg = Message::get_layout_info_targeted(
        "li-notes-1".into(),
        AutomationWindowTarget::Kind {
            kind: AutomationWindowKind::Notes,
            index: None,
        },
    );
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: Message = serde_json::from_str(&json).expect("deserialize");
    match back {
        Message::GetLayoutInfo { request_id, target } => {
            assert_eq!(request_id, "li-notes-1");
            let target = target.expect("target should be present");
            match target {
                AutomationWindowTarget::Kind { kind, .. } => {
                    assert_eq!(kind, AutomationWindowKind::Notes);
                }
                other => panic!("Expected Kind target, got: {:?}", other),
            }
        }
        other => panic!("Expected GetLayoutInfo, got: {:?}", other),
    }
}

#[test]
// ============================================================
// Cross-window transaction contract: Notes not rejected as main-only
// ============================================================

fn notes_get_elements_not_rejected_as_main_only() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("collect_surface_snapshot"),
        "getElements must route non-main targets through surface collector"
    );
    // Parse check
    let json = serde_json::json!({
        "type": "getElements",
        "requestId": "elm-notes-det",
        "target": {"type": "kind", "kind": "notes"},
        "limit": 10
    });
    let msg: Message =
        serde_json::from_value(json).expect("getElements with notes target should parse");
    match msg {
        Message::GetElements { target, .. } => assert!(target.is_some()),
        other => panic!("Expected GetElements, got: {:?}", other),
    }
}

#[test]
fn notes_batch_not_rejected_as_main_only() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("resolve_automation_read_target(\n                        &rid,\n                        \"batch\"")
            || source.contains("resolve_automation_read_target(&rid, \"batch\""),
        "batch handler must use resolve_automation_read_target (accepts Notes)"
    );
    assert!(
        source.contains("notes_batch_target"),
        "batch handler must have a Notes routing path"
    );
    notes_get_elements_not_rejected_as_main_only();
}

#[test]
fn notes_command_bar_uses_notes_as_actions_parent() {
    let source = include_str!("../../src/actions/command_bar.rs");
    let open_start = source
        .find("pub fn open_at_position")
        .expect("CommandBar::open_at_position should exist");
    let open_body = &source[open_start..];
    let open_end = open_body
        .find("/// Close the command bar")
        .expect("open_at_position should end before close docs");
    let open_body = &open_body[..open_end];

    assert!(
        open_body.contains("crate::notes::is_notes_window(window)"),
        "Notes CommandBar must identify the Notes host directly instead of trusting stale focused automation state"
    );
    assert!(
        open_body.contains("Some(\"notes\".to_string())"),
        "Notes CommandBar must pass the stable notes automation id to open_actions_window"
    );
    assert!(
        open_body.contains("crate::windows::focused_automation_window_id()"),
        "non-Notes CommandBar hosts should retain the existing focused-window fallback"
    );
}

#[test]
fn notes_wait_for_not_rejected_as_main_only() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("resolve_automation_read_target(\n                            &rid,\n                            \"waitFor\"")
            || source.contains("resolve_automation_read_target(&rid, \"waitFor\""),
        "waitFor handler must use resolve_automation_read_target (accepts Notes)"
    );
    assert!(
        source.contains("notes_wait_condition_satisfied"),
        "waitFor must delegate Notes conditions to notes_wait_condition_satisfied"
    );
    assert!(
        !source
            .contains("waitFor currently supports only the main automation window; resolved notes"),
        "waitFor must not reject Notes with main-only error"
    );
}

#[test]
fn notes_condition_checker_supports_generic_conditions() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    // notes_wait_condition_satisfied must handle the standard generic conditions
    assert!(
        source.contains("WaitDetailedCondition::ElementExists"),
        "Notes condition checker must handle ElementExists"
    );
    assert!(
        source.contains("WaitNamedCondition::WindowVisible"),
        "Notes condition checker must handle WindowVisible"
    );
}

#[test]
fn notes_batch_emits_transaction_logs() {
    let source = include_str!("../../src/prompt_handler/mod.rs");
    assert!(
        source.contains("transaction_notes_set_input"),
        "Notes batch setInput must emit structured log"
    );
    assert!(
        source.contains("transaction_notes_wait_complete"),
        "Notes batch waitFor must emit structured log"
    );
    assert!(
        source.contains("automation.batch.notes.completed"),
        "Notes batch must emit completion log"
    );
}

#[test]
fn get_layout_info_without_target_backward_compatible() {
    // Legacy getLayoutInfo requests (no target field) should still parse.
    let json = r#"{"type":"getLayoutInfo","requestId":"li-legacy"}"#;
    let msg: Message = serde_json::from_str(json).expect("parse");
    match msg {
        Message::GetLayoutInfo { request_id, target } => {
            assert_eq!(request_id, "li-legacy");
            assert!(target.is_none(), "target should default to None for legacy");
        }
        other => panic!("Expected GetLayoutInfo, got: {:?}", other),
    }
}
