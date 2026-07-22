//! Contract tests verifying that Notes is wired into the batch, waitFor,
//! and getElements transaction paths as a first-class automation target.
//! These are source-level audits that confirm the wiring exists without
//! requiring a live GPUI window.

// ============================================================
// getElements: Notes surface collector is registered
// ============================================================

#[test]
fn get_elements_has_notes_collector() {
    let source = include_str!("../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("AutomationWindowKind::Notes => collect_notes_snapshot"),
        "getElements must route Notes targets to collect_notes_snapshot"
    );
}

#[test]
fn notes_collector_exposes_stable_semantic_ids() {
    let source = include_str!("../src/windows/automation_surface_collector.rs");
    assert!(
        source.contains("\"panel:notes-window\""),
        "Notes collector must expose panel:notes-window semantic ID"
    );
    assert!(
        source.contains("\"input:notes-editor\""),
        "Notes collector must expose input:notes-editor semantic ID"
    );
}

// ============================================================
// Target resolution: Notes is accepted alongside Main and AgentChatDetached
// ============================================================

#[test]
fn automation_read_target_enum_has_notes_variant() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("AutomationReadTarget::Notes {"),
        "AutomationReadTarget enum must have a Notes variant"
    );
}

#[test]
fn resolve_automation_read_target_accepts_notes() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("AutomationWindowKind::Notes =>"),
        "resolve_automation_read_target must handle Notes kind"
    );
    assert!(
        source.contains("automation.target.notes_resolved"),
        "Notes resolution must emit a structured log"
    );
}

// ============================================================
// waitFor: Notes targets are no longer rejected
// ============================================================

#[test]
fn wait_for_uses_automation_read_target_for_non_agent_chat_conditions() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    let wait_for_start = source
        .find("let resolved_target: AutomationReadTarget = if target.is_some()")
        .expect("waitFor target-resolution block must exist");
    let wait_for_block = &source[wait_for_start..(wait_for_start + 1800).min(source.len())];
    assert!(
        wait_for_block.contains("resolve_automation_read_target(")
            && wait_for_block.contains("&rid,")
            && wait_for_block.contains("\"waitFor\","),
        "waitFor should use resolve_automation_read_target for non-Agent Chat conditions"
    );
}

#[test]
fn wait_for_checks_notes_condition_on_notes_target() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("notes_wait_condition_satisfied"),
        "waitFor must delegate to notes_wait_condition_satisfied for Notes targets"
    );
}

// ============================================================
// batch: Notes targets are accepted and routed
// ============================================================

#[test]
fn batch_uses_automation_read_target() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    let batch_start = source
        .find("let batch_target: AutomationReadTarget = if target.is_some()")
        .expect("batch target-resolution block must exist");
    let batch_block = &source[batch_start..(batch_start + 900).min(source.len())];
    assert!(
        batch_block.contains("resolve_automation_read_target(")
            && batch_block.contains("&rid,")
            && batch_block.contains("\"batch\","),
        "batch handler should use resolve_automation_read_target for target resolution"
    );
}

#[test]
fn batch_has_notes_path() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("notes_batch_target"),
        "batch handler should have a notes_batch_target variable for Notes routing"
    );
    assert!(
        source.contains("automation.batch.notes.completed"),
        "batch handler should emit Notes completion log"
    );
}

#[test]
fn notes_batch_emits_set_input_log() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("transaction_notes_set_input"),
        "Notes batch setInput should emit transaction_notes_set_input log"
    );
}

#[test]
fn notes_batch_emits_wait_complete_log() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("transaction_notes_wait_complete"),
        "Notes batch waitFor should emit transaction_notes_wait_complete log"
    );
}

// ============================================================
// Notes editor_state accessibility for transaction operations
// ============================================================

#[test]
fn notes_editor_state_is_crate_visible() {
    let source = include_str!("../src/notes/window.rs");
    assert!(
        source.contains("pub(crate) editor_state"),
        "editor_state must be pub(crate) for transaction provider access"
    );
}

#[test]
fn notes_entity_and_handle_helper_exists() {
    let source = include_str!("../src/notes/window/window_ops.rs");
    assert!(
        source.contains("pub fn get_notes_app_entity_and_handle"),
        "get_notes_app_entity_and_handle helper must be exported"
    );
}

// ============================================================
// Notes condition checker covers generic conditions
// ============================================================

#[test]
fn notes_wait_condition_handles_element_exists() {
    let source = include_str!("../src/prompt_handler/mod.rs");
    assert!(
        source.contains("WaitDetailedCondition::ElementExists"),
        "notes_wait_condition_satisfied must handle ElementExists"
    );
    assert!(
        source.contains("WaitDetailedCondition::ElementFocused"),
        "notes_wait_condition_satisfied must handle ElementFocused"
    );
}

// ============================================================
// Protocol parsing: Notes target parses correctly
// ============================================================

#[test]
fn batch_with_notes_target_parses() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "batch",
        "requestId": "b-notes",
        "target": {"type": "kind", "kind": "notes"},
        "commands": [
            {"type": "setInput", "text": "Hello from automation"}
        ],
        "options": {"stopOnError": true}
    });

    let msg: Message = serde_json::from_value(json).expect("batch with notes target should parse");
    match msg {
        Message::Batch {
            request_id,
            commands,
            target,
            ..
        } => {
            assert_eq!(request_id, "b-notes");
            assert_eq!(commands.len(), 1);
            assert!(target.is_some(), "target should be present");
        }
        other => panic!("Expected Batch, got: {other:?}"),
    }
}

#[test]
fn wait_for_with_notes_target_parses() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "waitFor",
        "requestId": "w-notes",
        "target": {"type": "kind", "kind": "notes"},
        "condition": {"type": "elementExists", "semanticId": "input:notes-editor"},
        "timeout": 3000,
        "pollInterval": 25
    });

    let msg: Message =
        serde_json::from_value(json).expect("waitFor with notes target should parse");
    match msg {
        Message::WaitFor {
            request_id, target, ..
        } => {
            assert_eq!(request_id, "w-notes");
            assert!(target.is_some(), "target should be present");
        }
        other => panic!("Expected WaitFor, got: {other:?}"),
    }
}

#[test]
fn get_elements_with_notes_target_parses() {
    use script_kit_gpui::protocol::Message;

    let json = serde_json::json!({
        "type": "getElements",
        "requestId": "elm-notes",
        "target": {"type": "kind", "kind": "notes"},
        "limit": 10
    });

    let msg: Message =
        serde_json::from_value(json).expect("getElements with notes target should parse");
    match msg {
        Message::GetElements {
            request_id, target, ..
        } => {
            assert_eq!(request_id, "elm-notes");
            assert!(target.is_some(), "target should be present");
        }
        other => panic!("Expected GetElements, got: {other:?}"),
    }
}

// ============================================================
// matches_state_spec is publicly accessible for non-main providers
// ============================================================

#[test]
fn matches_state_spec_is_exported() {
    let source = include_str!("../src/protocol/transaction_executor.rs");
    assert!(
        source.contains("pub fn matches_state_spec"),
        "matches_state_spec must be pub for use by non-main condition checkers"
    );
}

// ============================================================
// Notes skill documents automation-only surface and SDK has no
// public Notes globals
// ============================================================

#[test]
fn notes_skill_documents_automation_only_surface_and_sdk_has_no_public_notes_globals() {
    let skill = include_str!("../kit-init/skills/manage-notes/SKILL.md");
    assert!(
        skill.contains(
            "The current public Notes script surface is the automation target (`kind: notes`)"
        ),
        "Notes skill must document the automation-only surface"
    );

    let sdk = include_str!("../scripts/kit-sdk.ts");
    for forbidden in [
        "globalThis.notesOpen",
        "globalThis.notesCreate",
        "globalThis.notesSearch",
    ] {
        assert!(
            !sdk.contains(forbidden),
            "scripts/kit-sdk.ts must not expose invented Notes global: {forbidden}"
        );
    }
}

// ============================================================
// Notes ↔ Agent Chat handoff labels are consistent
// ============================================================

#[test]
fn notes_send_to_agent_chat_label_is_consistent_across_builder_and_panel() {
    let builder = include_str!("../src/actions/builders/notes.rs");
    let panel = include_str!("../src/notes/actions_panel.rs");

    assert!(
        builder.contains("\"Send to Agent Chat\""),
        "Notes builder must use 'Send to Agent Chat' label"
    );
    assert!(
        panel.contains("\"Send to Agent Chat\""),
        "Notes actions panel must use 'Send to Agent Chat' display name"
    );
}

#[test]
fn notes_embedded_agent_chat_switch_emits_structured_logs() {
    let panels = include_str!("../src/notes/window/panels.rs");
    assert!(
        panels.contains("notes_cart_open_embedded_agent_chat_requested"),
        "Notes cart handler must emit notes_cart_open_embedded_agent_chat_requested structured log"
    );
    assert!(
        panels.contains("notes_cart_handoff_skipped"),
        "Notes cart handler must emit notes_cart_handoff_skipped for empty notes"
    );
    assert!(
        panels.contains("open_or_focus_embedded_agent_chat")
            || panels.contains("relaunch_embedded_agent_chat"),
        "Notes must route through the Notes-owned embedded Agent Chat helpers"
    );
    assert!(
        !panels.contains("request_explicit_agent_chat_handoff_from_secondary_window"),
        "Notes must not use the detached secondary-window Agent Chat handoff path"
    );

    // Detached Agent Chat and shared helpers must still exist for non-Notes paths.
    let agent_chat_mod = include_str!("../src/ai/agent_chat/ui/mod.rs");
    assert!(
        agent_chat_mod.contains("pub(crate) fn open_or_focus_chat_with_input("),
        "Agent Chat staging helper must exist for non-Notes secondary-window handoffs"
    );

    let handler = include_str!("../src/app_actions/handle_action/mod.rs");
    assert!(
        handler.contains("agent_chat_save_as_note"),
        "Agent Chat handler must emit agent_chat_save_as_note structured log"
    );
    assert!(
        handler.contains("self.close_agent_chat_to_script_list(false, cx);"),
        "Embedded Agent Chat save-as-note should close Agent Chat back to ScriptList on success"
    );

    let detached = include_str!("../src/ai/agent_chat/ui/chat_window.rs");
    assert!(
        detached.contains("\"agent_chat_save_as_note\""),
        "Detached Agent Chat handler must implement agent_chat_save_as_note"
    );
    assert!(
        detached.contains("close_chat_window(cx);"),
        "Detached Agent Chat save-as-note should close the detached Agent Chat window on success"
    );

    let builder = include_str!("../src/actions/builders/script_context.rs");
    assert!(
        builder.contains("| \"agent_chat_save_as_note\""),
        "Detached Agent Chat host filter should allow agent_chat_save_as_note"
    );
}

#[test]
fn notes_agent_chat_handoff_documented_in_skill_and_guide() {
    let skill = include_str!("../kit-init/skills/manage-notes/SKILL.md");
    let guide = include_str!("../kit-init/GUIDE.md");

    for doc in [skill, guide] {
        assert!(
            doc.contains("## Agent Chat Handoffs"),
            "Notes docs must have Agent Chat Handoffs section"
        );
        assert!(
            doc.contains("**Send to Agent Chat**"),
            "Notes docs must document Send to Agent Chat handoff"
        );
        assert!(
            doc.contains("**Save as Note**"),
            "Notes docs must document Save as Note handoff"
        );
    }
}
