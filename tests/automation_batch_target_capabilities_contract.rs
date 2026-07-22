//! Source-level contract for targeted automation batch capabilities.
//!
//! Target-specific batch command support should live behind named capability
//! helpers before any future extraction of the async execution loops.

const PROMPT_HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");

fn source_between<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing source start: {start_pat}"));
    let end_rel = source[start..]
        .find(end_pat)
        .unwrap_or_else(|| panic!("missing source end: {end_pat}"));
    &source[start..start + end_rel]
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[test]
fn prompt_handler_declares_batch_target_capability_owner() {
    for required in [
        "enum AutomationBatchTargetKind",
        "struct BatchTargetCapabilities",
        "fn batch_target_kind_for_resolved_target(",
        "fn supported_batch_commands_for_target(",
        "fn unsupported_batch_command_error(",
        "fn is_agent_chat_wait_condition(",
    ] {
        assert!(
            PROMPT_HANDLER.contains(required),
            "prompt_handler must declare {required}"
        );
    }
}

#[test]
fn supported_command_lists_are_named_by_target_kind() {
    let body = source_between(
        PROMPT_HANDLER,
        "impl BatchTargetCapabilities",
        "\nfn batch_target_kind_for_resolved_target(",
    );

    for (kind, commands) in [
        (
            "AutomationBatchTargetKind::Main",
            &[
                "\"setInput\"",
                "\"forceSubmit\"",
                "\"waitFor\"",
                "\"openActions\"",
                "\"selectByValue\"",
                "\"selectBySemanticId\"",
                "\"filterAndSelect\"",
                "\"typeAndSubmit\"",
            ][..],
        ),
        (
            "AutomationBatchTargetKind::AgentChatDetached",
            &[
                "\"setInput\"",
                "\"waitFor\"",
                "\"selectByValue\"",
                "\"selectBySemanticId\"",
            ][..],
        ),
        (
            "AutomationBatchTargetKind::Notes",
            &[
                "\"setInput\"",
                "\"openActions\"",
                "\"togglePreview\"",
                "\"openNotesAgentChat\"",
                "\"waitFor\"",
            ][..],
        ),
        (
            "AutomationBatchTargetKind::ActionsDialog",
            &[
                "\"setInput\"",
                "\"selectByValue\"",
                "\"selectBySemanticId\"",
                "\"waitFor\"",
            ][..],
        ),
        (
            "AutomationBatchTargetKind::PromptPopup",
            &["\"selectByValue\"", "\"selectBySemanticId\"", "\"waitFor\""][..],
        ),
    ] {
        let arm = source_between(body, kind, "concise_unsupported_message:");
        for command in commands {
            assert!(
                arm.contains(command),
                "{kind} capability list must include {command}"
            );
        }
    }
}

#[test]
fn main_batch_open_actions_uses_shared_actions_dispatcher() {
    let main_batch_body = source_between(
        PROMPT_HANDLER,
        "// ── Main-window batch path (existing)",
        "let success = !failed;",
    );

    for required in [
        "protocol::BatchCommand::OpenActions =>",
        "crate::get_main_window_handle()",
        "this.dispatch_actions_toggle_for_current_view(",
        "\"devtools_batch_open_actions\"",
        "command: \"openActions\".to_string()",
        "Current main view does not expose actions",
    ] {
        assert!(
            main_batch_body.contains(required),
            "Main batch openActions must route through the shared actions dispatcher: {required}"
        );
    }

    let open_actions_arm = source_between(
        main_batch_body,
        "protocol::BatchCommand::OpenActions =>",
        "protocol::BatchCommand::TogglePreview =>",
    );

    assert!(
        !open_actions_arm.contains("simulateKey"),
        "Main batch openActions must not synthesize Cmd+K"
    );
}

#[test]
fn unsupported_command_errors_are_generated_by_the_helper() {
    let helper_body = source_between(
        PROMPT_HANDLER,
        "fn unsupported_batch_command_error(",
        "\nfn is_agent_chat_wait_condition(",
    );

    assert!(helper_body.contains("protocol::TransactionErrorCode::UnsupportedCommand"));
    assert!(helper_body.contains("supported_batch_commands_for_target(kind).join(\", \")"));
    assert!(helper_body.contains("ActionsDialog batch supports:"));
    assert!(helper_body.contains("PromptPopup batch supports:"));
    assert_eq!(
        count_occurrences(PROMPT_HANDLER, "TransactionErrorCode::UnsupportedCommand"),
        1,
        "target batch branches must not copy unsupported-command construction"
    );

    for required in [
        "unsupported_batch_command_error(\n                                            AutomationBatchTargetKind::AgentChatDetached",
        "unsupported_batch_command_error(\n                                            AutomationBatchTargetKind::Notes",
        "unsupported_batch_command_error(\n                                            AutomationBatchTargetKind::ActionsDialog",
        "unsupported_batch_command_error(\n                                            AutomationBatchTargetKind::PromptPopup",
    ] {
        assert!(
            PROMPT_HANDLER.contains(required),
            "target branch must call the shared unsupported helper: {required}"
        );
    }
}

#[test]
fn agent_chat_wait_condition_classification_is_shared() {
    let helper_body = source_between(
        PROMPT_HANDLER,
        "fn is_agent_chat_wait_condition(",
        "\n/// Resolve an automation target",
    );

    for condition in [
        "AgentChatReady",
        "AgentChatPickerOpen",
        "AgentChatPickerClosed",
        "AgentChatItemAccepted",
        "AgentChatCursorAt",
        "AgentChatStatus",
        "AgentChatInputMatch",
        "AgentChatInputContains",
        "AgentChatAcceptedViaKey",
        "AgentChatAcceptedLabel",
        "AgentChatAcceptedCursorAt",
        "AgentChatInputLayoutMatch",
        "AgentChatSetupVisible",
        "AgentChatSetupReasonCode",
        "AgentChatSetupPrimaryAction",
        "AgentChatSetupAgentPickerOpen",
        "AgentChatSetupSelectedAgent",
    ] {
        assert!(
            helper_body.contains(condition),
            "Agent Chat wait classifier must include {condition}"
        );
    }

    assert!(PROMPT_HANDLER
        .contains("let is_agent_chat_condition = is_agent_chat_wait_condition(&condition);"));
    assert!(PROMPT_HANDLER.contains("let is_agent_chat = is_agent_chat_wait_condition(condition);"));
}

#[test]
fn batch_prompt_popup_routing_uses_the_resolved_target_kind() {
    let body = source_between(
        PROMPT_HANDLER,
        "PromptMessage::Batch {",
        "\n            PromptMessage::ForceSubmit",
    );

    assert!(body
        .contains("let batch_target_kind = batch_target_kind_for_resolved_target(&batch_target);"));
    assert!(body.contains("batch_target_kind == AutomationBatchTargetKind::PromptPopup"));
}

#[test]
fn automation_ai_target_routes_to_live_agent_chat_entity_for_batch_and_reads() {
    for marker in [
        "fn active_agent_chat_entity(",
        "automation.target.ai_routed_to_agent_chat_entity",
        "automation.agent_chat_target.ai_resolved_to_entity",
        "self.embedded_agent_chat_automation_entity().as_ref()",
    ] {
        assert!(
            PROMPT_HANDLER.contains(marker),
            "prompt_handler must include Ai→Agent Chat routing marker: {marker}"
        );
    }
}
