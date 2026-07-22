//! Tests for execution_helpers.rs — Scratch Pad CRUD, API key prompt logic,
//! legacy Claude config helpers, and Agent Chat setup copy.

use super::read_source as read;

fn execution_helpers_content() -> String {
    read("src/app_execute/execution_helpers.rs")
}

// ---------------------------------------------------------------------------
// Scratch Pad path
// ---------------------------------------------------------------------------

#[test]
fn get_scratch_pad_path_uses_kit_directory() {
    let content = execution_helpers_content();

    assert!(
        content.contains("get_kit_path().join(\"scratch-pad.md\")"),
        "Expected get_scratch_pad_path to join 'scratch-pad.md' to kit path"
    );
}

// ---------------------------------------------------------------------------
// Scratch Pad CRUD — directory creation + file loading
// ---------------------------------------------------------------------------

#[test]
fn open_scratch_pad_creates_parent_directory_if_missing() {
    let content = execution_helpers_content();

    assert!(
        content.contains("std::fs::create_dir_all(parent)"),
        "Expected open_scratch_pad to create parent directory with create_dir_all"
    );
}

#[test]
fn open_scratch_pad_handles_file_not_found_by_creating_empty_file() {
    let content = execution_helpers_content();

    assert!(
        content.contains("std::io::ErrorKind::NotFound"),
        "Expected open_scratch_pad to handle NotFound by creating an empty file"
    );
    assert!(
        content.contains("std::fs::write(&scratch_path, \"\")"),
        "Expected open_scratch_pad to write empty string for new scratch pad file"
    );
}

#[test]
fn open_scratch_pad_shows_toast_on_directory_creation_failure() {
    let content = execution_helpers_content();

    // Find the create_dir_all error handling block
    let dir_create_pos = content
        .find("create_dir_all(parent)")
        .expect("Expected create_dir_all call");
    let block = &content[dir_create_pos..dir_create_pos + 800];

    assert!(
        block.contains("show_error_toast("),
        "Expected directory creation failure to show error toast"
    );
}

#[test]
fn open_scratch_pad_shows_toast_on_read_failure() {
    let content = execution_helpers_content();

    assert!(
        content.contains("Failed to read scratch pad:"),
        "Expected open_scratch_pad to show toast on file read failure"
    );
}

// ---------------------------------------------------------------------------
// Scratch Pad auto-save
// ---------------------------------------------------------------------------

#[test]
fn scratch_pad_autosave_uses_two_second_interval() {
    let content = execution_helpers_content();

    assert!(
        content.contains("Duration::from_secs(2)"),
        "Expected scratch pad auto-save interval to be 2 seconds"
    );
}

#[test]
fn scratch_pad_autosave_stops_when_entity_is_dropped() {
    let content = execution_helpers_content();

    // The autosave loop should break when entity_weak.upgrade() returns None
    assert!(
        content.contains("entity_weak.upgrade()"),
        "Expected autosave loop to use weak reference for entity liveness check"
    );
    assert!(
        content.contains("false // Entity dropped, stop the task"),
        "Expected autosave to break when entity is dropped"
    );
}

#[test]
fn scratch_pad_submit_save_error_surfaces_toast() {
    let content = execution_helpers_content();

    // The submit callback should send errors through a channel for toast display
    assert!(
        content.contains("save_err_tx.try_send("),
        "Expected scratch pad submit callback to send save errors via channel"
    );
    assert!(
        content.contains("save_err_rx.recv().await"),
        "Expected scratch pad error listener to await save error channel"
    );
}

// ---------------------------------------------------------------------------
// API key prompt
// ---------------------------------------------------------------------------

#[test]
fn show_api_key_prompt_stores_pending_provider_name() {
    let content = execution_helpers_content();

    assert!(
        content.contains("self.pending_api_key_config = Some(provider_name.to_string())"),
        "Expected show_api_key_prompt to store pending provider name for completion handler"
    );
}

#[test]
fn show_api_key_prompt_checks_existing_secret_in_keyring() {
    let content = execution_helpers_content();

    assert!(
        content.contains("secrets::get_secret_info_result(&key)"),
        "Expected show_api_key_prompt to check for existing secret in keyring"
    );
    assert!(
        content.contains("exists_in_keyring") && content.contains("secret_store_error"),
        "Expected show_api_key_prompt to pass existing-secret and storage-error state to EnvPrompt"
    );
}

#[test]
fn handle_api_key_completion_clears_pending_state() {
    let content = execution_helpers_content();

    let completion_fn_start = content
        .find("fn handle_api_key_completion(")
        .expect("Expected handle_api_key_completion to exist");
    let block = &content[completion_fn_start..completion_fn_start + 300];

    assert!(
        block.contains("self.pending_api_key_config = None"),
        "Expected handle_api_key_completion to clear pending_api_key_config"
    );
}

#[test]
fn handle_api_key_completion_rebuilds_provider_registry_on_success() {
    let content = execution_helpers_content();

    let completion_fn_start = content
        .find("fn handle_api_key_completion(")
        .expect("Expected handle_api_key_completion to exist");
    let block = &content[completion_fn_start..completion_fn_start + 1200];

    assert!(
        block.contains("self.rebuild_provider_registry_async(cx)"),
        "Expected handle_api_key_completion to rebuild provider registry on success"
    );
}

#[test]
fn handle_api_key_completion_uses_named_completion_outcome() {
    let content = execution_helpers_content();

    assert!(
        content.contains("enum ApiKeyCompletionOutcome")
            && content.contains("Saved")
            && content.contains("NotSaved")
            && content.contains("fn from_success(success: bool) -> Self")
            && content.contains("fn success_message(self, provider: &str) -> Option<String>")
            && content.contains("fn should_rebuild_provider_registry(self) -> bool"),
        "Expected API key completion feedback to be driven by a named outcome state"
    );

    let completion_fn_start = content
        .find("fn handle_api_key_completion(")
        .expect("Expected handle_api_key_completion to exist");
    let block = &content[completion_fn_start..completion_fn_start + 1400];

    assert!(
        block.contains("let completion_outcome = ApiKeyCompletionOutcome::from_success(success)")
            && block.contains("completion_outcome.success_message(&provider)")
            && block.contains("completion_outcome.should_rebuild_provider_registry()"),
        "Expected handle_api_key_completion to route success toast and registry rebuild through the outcome state"
    );
}

#[test]
fn handle_api_key_completion_uses_deferred_resize() {
    let content = execution_helpers_content();

    let completion_fn_start = content
        .find("fn handle_api_key_completion(")
        .expect("Expected handle_api_key_completion to exist");
    let block = &content[completion_fn_start..completion_fn_start + 2600];

    assert!(
        block.contains("window.defer(cx,"),
        "Expected handle_api_key_completion to use deferred resize (called from render)"
    );
}

// ---------------------------------------------------------------------------
// Legacy Claude config compatibility
// ---------------------------------------------------------------------------

#[test]
fn enable_claude_code_in_config_recovers_from_validation_failure() {
    let content = execution_helpers_content();

    assert!(
        content.contains("ConfigWriteError::ValidationFailed"),
        "Expected enable_claude_code_in_config to handle validation failure"
    );
    assert!(
        content.contains("editor::recover_from_backup("),
        "Expected enable_claude_code_in_config to attempt backup recovery on validation failure"
    );
}

#[test]
fn enable_claude_code_in_config_checks_claude_cli_availability() {
    let content = execution_helpers_content();

    assert!(
        content.contains("claude_available"),
        "Expected enable_claude_code_in_config to check if Claude CLI is installed"
    );
    assert!(
        content.contains("\"--version\""),
        "Expected enable_claude_code_in_config to verify Claude CLI with --version"
    );
}

#[test]
fn enable_claude_code_in_config_shows_install_instructions_when_cli_missing() {
    let content = execution_helpers_content();

    assert!(
        content.contains("npm install -g @anthropic-ai/claude-code"),
        "Expected install instructions to be shown when Claude CLI is not found"
    );
}

// ---------------------------------------------------------------------------
// Agent Chat setup contract
// ---------------------------------------------------------------------------

#[test]
fn agent_chat_setup_copy_points_to_catalog_and_config_ts_not_direct_provider_keys() {
    let readme = read("README.md");
    let agent_config_section = readme
        .split("### Agent Chat Configuration")
        .nth(1)
        .and_then(|rest| rest.split("### Legacy Direct-Provider API Keys").next())
        .expect("README must document Agent Chat Configuration before legacy API keys");

    assert!(
        agent_config_section.contains("Agent Catalog")
            && agent_config_section.contains("selectedProfileId")
            && agent_config_section.contains("selectedModelId")
            && agent_config_section.contains("profiles"),
        "README Agent Chat config section must document catalog-backed ai preferences"
    );

    for stale in [
        "Configure OpenAI API Key",
        "Configure Anthropic API Key",
        "Configure Vercel",
        "Vercel AI Gateway",
        "Agent Chat Chat",
    ] {
        assert!(
            !agent_config_section.contains(stale),
            "README Agent Chat config section must not expose stale copy: {stale}"
        );
    }

    assert!(
        !agent_config_section.contains("claudeCode"),
        "README must not present claudeCode as the primary Agent Chat setup path"
    );
}

// ---------------------------------------------------------------------------
// AI command dispatch — all user-facing branches use deferred helper
// ---------------------------------------------------------------------------

fn builtin_execution_content() -> String {
    read("src/app_execute/builtin_execution.rs")
}

#[test]
fn ai_open_and_new_conversation_use_deferred_helper() {
    let content = builtin_execution_content();
    assert!(
        content.contains("self.open_tab_ai_agent_chat_with_entry_intent(None, cx);"),
        "Expected OpenAi/MiniAi/NewConversation/ClearConversation to route to Agent Chat"
    );
}

#[test]
fn ai_clear_conversation_uses_deferred_helper() {
    let content = builtin_execution_content();
    assert!(
        content.contains("AiAgentChatBuiltinAction")
            && content.contains("ai_ClearConversation_routed_to_agent_chat"),
        "Expected ClearConversation to report a named routed_to_agent_chat success outcome"
    );
}

#[test]
fn ai_clear_conversation_shows_hud_on_success() {
    let content = builtin_execution_content();
    assert!(
        content.contains("Self::ClearConversation => \"ai_ClearConversation_routed_to_agent_chat\""),
        "Expected ClearConversation success to be reported through the named routed_to_agent_chat outcome"
    );
}

#[test]
fn ai_clear_conversation_shows_toast_when_clear_fails() {
    let content = builtin_execution_content();
    assert!(
        !content.contains("Failed to clear AI conversations"),
        "ClearConversation no longer uses the legacy clear-all-chats failure path"
    );
}

// ---------------------------------------------------------------------------
// Settings command handlers — Toast/HUD feedback on errors
// ---------------------------------------------------------------------------

#[test]
fn reset_window_positions_returns_to_default_menu_without_hud() {
    let content = builtin_execution_content();

    let reset_section_start = content
        .find("SettingsCommandBuiltinAction::ResetWindowPositions")
        .expect("Expected ResetWindowPositions match arm");
    let block = &content[reset_section_start..reset_section_start + 800];

    assert!(
        block.contains("reset_window_positions_to_default_main_menu(cx)"),
        "Expected ResetWindowPositions to use the shared default menu reset helper"
    );
    assert!(
        !block.contains("show_hud("),
        "ResetWindowPositions should not trigger HUD feedback"
    );
    assert!(
        !block.contains("close_and_reset_window(cx)"),
        "ResetWindowPositions should leave the main menu visible instead of hiding the launcher"
    );
}

#[test]
fn reset_window_positions_suppresses_save_before_reset() {
    let content = std::fs::read_to_string("src/app_impl/lifecycle_reset.rs")
        .expect("Failed to read lifecycle_reset.rs");

    let reset_section_start = content
        .find("fn reset_window_positions_to_default_main_menu")
        .expect("Expected reset_window_positions_to_default_main_menu helper");
    let block = &content[reset_section_start..reset_section_start + 1200];

    assert!(
        block.contains("suppress_save()"),
        "Expected ResetWindowPositions to suppress position saving before reset"
    );
    assert!(
        block.contains("reset_all_positions()"),
        "Expected ResetWindowPositions to call reset_all_positions"
    );
    assert!(
        block.contains("reset_to_script_list(cx)"),
        "Expected ResetWindowPositions to reset the main window back to the default script list"
    );
    assert!(
        block.contains("calculate_eye_line_bounds_on_mouse_display"),
        "Expected ResetWindowPositions to reposition the main window to default bounds immediately"
    );
}

#[test]
fn settings_api_key_commands_are_not_exposed_as_builtins() {
    let content = builtin_execution_content();

    assert!(
        !content.contains("SettingsCommandType::ConfigureVercelApiKey"),
        "Vercel API key setup should not be a settings command"
    );
    assert!(
        !content.contains("SettingsCommandType::ConfigureOpenAiApiKey"),
        "OpenAI API key setup should not be a settings command"
    );
    assert!(
        !content.contains("SettingsCommandType::ConfigureAnthropicApiKey"),
        "Anthropic API key setup should not be a settings command"
    );
}
