// Comprehensive test coverage for ALL action variants in handle_action/.
//
// Acceptance criteria:
// 1. Every action variant has at least one test asserting its feedback type
// 2. Source-scanning tests verify no unwrap/expect in production action code
// 3. Source-scanning tests verify all error paths use toast (not hud)
// 4. Source-scanning tests verify named duration constants used (no raw ms)

use super::{count_occurrences, read_all_handle_action_sources, read_source as read};

fn handle_action_content() -> String {
    read_all_handle_action_sources()
}

fn helpers_content() -> String {
    read("src/app_actions/helpers.rs")
}

fn handler_branch_block<'a>(content: &'a str, branch: &str, len: usize) -> &'a str {
    let handler_pos = content
        .find("fn handle_file_action")
        .or_else(|| content.find("fn handle_script_action"))
        .or_else(|| content.find("fn handle_clipboard_action"))
        .or_else(|| content.find("fn handle_emoji_action"))
        .unwrap_or(0);
    let branch_pos = content[handler_pos..]
        .find(branch)
        .map(|pos| handler_pos + pos)
        .unwrap_or_else(|| panic!("Expected {branch} handler"));

    &content[branch_pos..content.len().min(branch_pos + len)]
}

// ===========================================================================
// Source-scanning: no unwrap/expect in production action code
// ===========================================================================

#[test]
fn handle_action_has_no_unwrap_calls() {
    let content = handle_action_content();

    // Filter out the one known acceptable usage: .unwrap_or() which is safe
    let lines: Vec<(usize, &str)> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                return false;
            }
            trimmed.contains(".unwrap()")
        })
        .collect();

    assert!(
        lines.is_empty(),
        "handle_action/ contains .unwrap() calls in production code at lines: {:?}",
        lines
            .iter()
            .map(|(n, l)| format!("L{}: {}", n + 1, l.trim()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn handle_action_has_no_expect_calls() {
    let content = handle_action_content();

    let lines: Vec<(usize, &str)> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                return false;
            }
            trimmed.contains(".expect(")
        })
        .collect();

    assert!(
        lines.is_empty(),
        "handle_action/ contains .expect() calls in production code at lines: {:?}",
        lines
            .iter()
            .map(|(n, l)| format!("L{}: {}", n + 1, l.trim()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn helpers_production_code_has_no_unwrap_or_expect() {
    let content = helpers_content();

    // Split at #[cfg(test)] to only check production code
    let production_code = content.split("#[cfg(test)]").next().unwrap_or(&content);

    let unwrap_lines: Vec<(usize, &str)> = production_code
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                return false;
            }
            // Allow unwrap_or, unwrap_or_else, unwrap_or_default — those are safe
            if trimmed.contains(".unwrap_or") {
                return false;
            }
            trimmed.contains(".unwrap()") || trimmed.contains(".expect(")
        })
        .collect();

    assert!(
        unwrap_lines.is_empty(),
        "helpers.rs production code contains .unwrap()/.expect() at lines: {:?}",
        unwrap_lines
            .iter()
            .map(|(n, l)| format!("L{}: {}", n + 1, l.trim()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn sdk_actions_has_no_unwrap_or_expect() {
    let content = read("src/app_actions/sdk_actions.rs");

    let production_code = content.split("#[cfg(test)]").next().unwrap_or(&content);

    let violations: Vec<(usize, &str)> = production_code
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                return false;
            }
            if trimmed.contains(".unwrap_or") {
                return false;
            }
            trimmed.contains(".unwrap()") || trimmed.contains(".expect(")
        })
        .collect();

    assert!(
        violations.is_empty(),
        "sdk_actions.rs production code contains .unwrap()/.expect() at lines: {:?}",
        violations
            .iter()
            .map(|(n, l)| format!("L{}: {}", n + 1, l.trim()))
            .collect::<Vec<_>>()
    );
}

// ===========================================================================
// Source-scanning: all error paths use toast (not hud)
// ===========================================================================

#[test]
fn error_paths_use_show_error_toast_not_show_hud() {
    let content = handle_action_content();

    // Find all show_hud calls and verify none contain error-like messages
    let error_keywords = ["Failed", "failed", "Error", "error", "Cannot", "cannot"];

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("tracing")
        {
            continue;
        }
        if trimmed.contains("show_hud(") {
            for keyword in &error_keywords {
                // Allow "failed to" in tracing context and "Err" variable patterns
                if trimmed.contains(&format!("\"{}", keyword)) {
                    // Some HUD messages reference errors in log context, check carefully
                    // show_hud should never contain "Failed to", "Error:", "Cannot "
                    assert!(
                        !trimmed.contains("\"Failed to") &&
                        !trimmed.contains("\"Error:") &&
                        !trimmed.contains("\"Cannot "),
                        "Error message found in show_hud() call — should use show_error_toast() instead:\n  {}",
                        trimmed
                    );
                }
            }
        }
    }
}

#[test]
fn all_show_error_toast_calls_use_toast_not_hud() {
    let content = handle_action_content();

    // Verify show_error_toast exists and is the standard error path
    let error_toast_count = count_occurrences(&content, "show_error_toast(");
    assert!(
        error_toast_count >= 10,
        "Expected at least 10 show_error_toast() calls in handle_action/ (found {error_toast_count})"
    );

    // Verify the show_error_toast helper uses Toast::error with TOAST_ERROR_MS
    let helper_content = handle_action_content();
    assert!(
        helper_content.contains("Toast::error(msg, &self.theme)")
            && helper_content.contains("TOAST_ERROR_MS"),
        "show_error_toast helper must use Toast::error with TOAST_ERROR_MS"
    );
}

// ===========================================================================
// Source-scanning: named duration constants (no raw ms values)
// ===========================================================================

#[test]
fn handle_action_uses_only_named_duration_constants() {
    let content = handle_action_content();

    // Look for raw numeric literals in show_hud() calls
    let re = regex::Regex::new(r"show_hud\([^)]*Some\(\d+\)").expect("regex");
    let violations: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("//") && re.is_match(trimmed)
        })
        .collect();

    assert!(
        violations.is_empty(),
        "show_hud() calls must use named constants (HUD_SHORT_MS, etc.), not raw numbers:\n{}",
        violations.join("\n")
    );
}

#[test]
fn visible_attach_actions_use_agent_chat_label() {
    for (path, label) in [
        (
            "src/render_builtins/actions.rs",
            "dictation history actions",
        ),
        (
            "src/app_impl/root_unified_result_actions.rs",
            "root unified result actions",
        ),
        ("src/actions/dialog.rs", "actions dialog docs"),
    ] {
        let source = read(path);
        assert!(
            source.contains("Attach to Agent Chat"),
            "{label} should use the concrete Agent Chat product label"
        );
        assert!(
            !source.contains("Attach to AI"),
            "{label} should not use the generic Attach to AI label"
        );
    }
}

#[test]
fn markdown_copy_actions_do_not_advertise_file_export() {
    let agent_chat_builder = read("src/actions/builders/script_context.rs");
    let agent_chat_handler = read("src/app_actions/handle_action/mod.rs");
    let chat_builder = read("src/actions/builders/chat.rs");

    assert!(
        agent_chat_builder.contains("\"agent_chat_export_markdown\"")
            && agent_chat_builder.contains("\"Copy Conversation as Markdown\""),
        "Agent Chat markdown action should advertise clipboard copy behavior"
    );
    assert!(
        agent_chat_handler
            .contains("cx.write_to_clipboard(gpui::ClipboardItem::new_string(markdown))")
            && agent_chat_handler.contains("Copied Agent Chat conversation as markdown"),
        "Agent Chat markdown handler should still prove clipboard copy behavior"
    );
    assert!(
        chat_builder.contains("\"chat:export_markdown\"")
            && chat_builder.contains("\"Copy Chat as Markdown\"")
            && chat_builder.contains("Copy chat as Markdown to the clipboard"),
        "Legacy chat markdown action should advertise clipboard copy behavior"
    );
    assert!(
        !agent_chat_builder.contains("\"Export as Markdown\"")
            && !chat_builder.contains("\"Export as Markdown\""),
        "Markdown copy actions must not use export wording unless they create a file"
    );
}

#[test]
fn theme_chooser_actions_do_not_advertise_dialog_reserved_escape_shortcut() {
    let builder = read("src/render_builtins/actions.rs");
    let undo_action = builder
        .split("\"theme_chooser_undo_close\"")
        .nth(1)
        .and_then(|section| section.split("\"theme_chooser_remix\"").next())
        .expect("theme chooser undo/close action should be present");
    let router = read("src/app_impl/actions_dialog.rs");
    let escape_branch = router
        .split("if is_key_escape(key) {")
        .nth(1)
        .and_then(|section| {
            section
                .split("// Check if keystroke matches any action shortcut")
                .next()
        })
        .expect("actions dialog escape branch should precede shortcut matching");

    assert!(
        undo_action.contains("\"Undo Changes and Close\""),
        "Theme Designer should still offer the explicit undo-and-close action"
    );
    assert!(
        !undo_action.contains(".with_shortcut(\"Esc\")"),
        "Theme Designer actions dialog must not advertise Esc for undo/close because Esc is reserved for dialog close/back while the popup is open"
    );
    assert!(
        escape_branch.contains("d.handle_escape(cx)")
            && escape_branch.contains("self.close_actions_popup(host, window, cx);"),
        "Actions dialog Escape should remain reserved for popup close/back before action shortcut matching"
    );
}

#[test]
fn handle_action_uses_only_named_toast_duration_constants() {
    let content = handle_action_content();

    // Look for raw numeric literals in duration_ms() calls
    let re = regex::Regex::new(r"duration_ms\(Some\(\d+\)").expect("regex");
    let violations: Vec<&str> = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("//") && re.is_match(trimmed)
        })
        .collect();

    assert!(
        violations.is_empty(),
        "duration_ms() calls must use named constants (TOAST_ERROR_MS, etc.), not raw numbers:\n{}",
        violations.join("\n")
    );
}

// ===========================================================================
// Action variant coverage: clipboard_pin / clipboard_unpin
// ===========================================================================

#[test]
fn clipboard_pin_unpin_uses_hud_short_ms_on_success() {
    let content = handle_action_content();

    let pin_pos = content
        .find("\"clipboard_pin\" | \"clipboard_unpin\"")
        .expect("Expected clipboard_pin/unpin handler");
    let block = &content[pin_pos..content.len().min(pin_pos + 4000)];

    assert!(
        block.contains("pin_action.success_hud()"),
        "clipboard_pin/unpin should use ClipboardPinHandlerAction for success message"
    );
    assert!(
        block.contains("HUD_SHORT_MS"),
        "clipboard_pin/unpin success should use HUD_SHORT_MS"
    );
}

#[test]
fn clipboard_pin_unpin_shows_error_toast_on_failure() {
    let content = handle_action_content();

    let pin_pos = content
        .find("\"clipboard_pin\" | \"clipboard_unpin\"")
        .expect("Expected clipboard_pin/unpin handler");
    let block = &content[pin_pos..content.len().min(pin_pos + 4000)];

    assert!(
        block.contains("pin_action.failure_message(e)")
            && block.contains("self.show_error_toast(pin_action.failure_message(e), cx)"),
        "clipboard_pin/unpin error should use show_error_toast"
    );
}

#[test]
fn clipboard_pin_unpin_shows_error_when_no_entry() {
    let content = handle_action_content();

    let pin_pos = content
        .find("\"clipboard_pin\" | \"clipboard_unpin\"")
        .expect("Expected clipboard_pin/unpin handler");
    let block = &content[pin_pos..content.len().min(pin_pos + 4000)];

    assert!(
        block.contains("pin_action.selection_required_message()")
            && content.contains("Self::Pin | Self::Unpin => \"No clipboard entry selected\""),
        "clipboard_pin/unpin should show error when no entry is selected"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_copy
// ===========================================================================

#[test]
fn clipboard_copy_shows_hud_on_success() {
    let content = handle_action_content();

    let copy_pos = content
        .find("\"clipboard_paste\" | \"clipboard_copy\" | \"clipboard_paste_keep_open\"")
        .expect("Expected clipboard_copy handler");
    let block = &content[copy_pos..content.len().min(copy_pos + 3000)];

    assert!(
        block.contains("copy_paste_action.success_hud()")
            && content.contains("Copied to clipboard"),
        "clipboard_copy should show 'Copied to clipboard' HUD"
    );
    assert!(
        block.contains("HUD_SHORT_MS"),
        "clipboard_copy should use HUD_SHORT_MS"
    );
}

#[test]
fn clipboard_copy_shows_error_toast_on_failure() {
    let content = handle_action_content();

    let copy_pos = content
        .find("\"clipboard_paste\" | \"clipboard_copy\" | \"clipboard_paste_keep_open\"")
        .expect("Expected clipboard_copy handler");
    let block = &content[copy_pos..content.len().min(copy_pos + 3000)];

    assert!(
        block.contains("copy_paste_action.failure_message(e)")
            && content.contains("Self::CopyOnly => \"Failed to copy\""),
        "clipboard_copy error should use show_error_toast"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_paste_keep_open
// ===========================================================================

#[test]
fn clipboard_paste_keep_open_spawns_paste_simulation_on_success() {
    let content = handle_action_content();

    let paste_pos = content
        .find("\"clipboard_paste\" | \"clipboard_copy\" | \"clipboard_paste_keep_open\"")
        .expect("Expected clipboard_paste_keep_open handler");
    let block = &content[paste_pos..content.len().min(paste_pos + 3000)];

    assert!(
        block.contains("copy_paste_action.paste_close_behavior()")
            && block.contains("finalize_paste_after_clipboard_ready(")
            && content.contains("Self::PasteKeepOpen => Some(PasteCloseBehavior::KeepWindowOpen)"),
        "clipboard_paste_keep_open should finalize paste while keeping the window open"
    );
}

#[test]
fn clipboard_paste_keep_open_does_not_hide_window() {
    let content = handle_action_content();

    let paste_pos = content
        .find("fn paste_close_behavior")
        .expect("Expected clipboard_paste_keep_open handler");
    // Use a tight window (just this handler) to avoid bleeding
    // into subsequent handlers that do call hide_main_and_reset.
    let end = content[paste_pos..]
        .find("fn success_event")
        .map(|offset| paste_pos + offset)
        .unwrap_or_else(|| content.len().min(paste_pos + 1000));
    let block = &content[paste_pos..end];

    assert!(
        block.contains("Self::PasteKeepOpen => Some(PasteCloseBehavior::KeepWindowOpen)")
            && !block.contains("Self::PasteKeepOpen => Some(PasteCloseBehavior::HideWindow)"),
        "clipboard_paste_keep_open should NOT hide the main window"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_quick_look
// ===========================================================================

#[test]
fn clipboard_quick_look_shows_error_toast_on_failure() {
    let content = handle_action_content();

    let ql_pos = content
        .find("\"clipboard_quick_look\"")
        .expect("Expected clipboard_quick_look handler");
    let block = &content[ql_pos..content.len().min(ql_pos + 3000)];

    assert!(
        block.contains("show_error_toast(external_action.quick_look_failure_message(e), cx)"),
        "clipboard_quick_look error should use named action state feedback"
    );
}

#[test]
fn clipboard_quick_look_shows_error_when_no_entry() {
    let content = handle_action_content();

    let ql_pos = content
        .find("\"clipboard_quick_look\"")
        .expect("Expected clipboard_quick_look handler");
    let block = &content[ql_pos..content.len().min(ql_pos + 3000)];

    assert!(
        block.contains("external_action.selection_required_message()"),
        "clipboard_quick_look should show named action state error when no entry is selected"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_attach_to_ai
// ===========================================================================

#[test]
fn clipboard_attach_to_ai_handles_all_content_types() {
    let content = handle_action_content();

    assert!(
        content.contains("\"clipboard_attach_to_ai\""),
        "Expected clipboard_attach_to_ai handler"
    );

    assert!(
        content.contains("ClipboardAttachToAiHandlerAction::from_content_type")
            && content.contains("ContentType::Text"),
        "clipboard_attach_to_ai should handle Text content"
    );
    assert!(
        content.contains("ContentType::Image"),
        "clipboard_attach_to_ai should handle Image content"
    );
    assert!(
        content.contains("ContentType::File"),
        "clipboard_attach_to_ai should handle File content"
    );
}

#[test]
fn clipboard_attach_to_ai_uses_deferred_agent_chat_action() {
    let content = handle_action_content();

    let attach_pos = content
        .find("\"clipboard_attach_to_ai\"")
        .expect("Expected clipboard_attach_to_ai handler");
    let block = &content[attach_pos..content.len().min(attach_pos + 5000)];

    assert!(
        block.contains("open_agent_chat_after_main_hide("),
        "clipboard_attach_to_ai should use open_agent_chat_after_main_hide"
    );
    assert!(
        content.contains("DeferredAgentChatAction::SetInput")
            && content.contains("DeferredAgentChatAction::AddAttachment")
            && block.contains("DeferredAgentChatAction::SetInputWithImage"),
        "clipboard_attach_to_ai should route clipboard content through a deferred Agent Chat handoff action"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_open_with
// ===========================================================================

#[test]
fn clipboard_open_with_handler_exists() {
    let content = handle_action_content();

    assert!(
        content.contains("\"clipboard_open_with\""),
        "Expected handle_action/ to handle clipboard_open_with"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_annotate_cleanshot / clipboard_upload_cleanshot
// ===========================================================================

#[test]
fn clipboard_cleanshot_actions_exist() {
    let content = handle_action_content();

    assert!(
        content.contains("\"clipboard_annotate_cleanshot\""),
        "Expected handle_action/ to handle clipboard_annotate_cleanshot"
    );
    assert!(
        content.contains("\"clipboard_upload_cleanshot\""),
        "Expected handle_action/ to handle clipboard_upload_cleanshot"
    );
}

#[test]
fn clipboard_cleanshot_actions_show_error_when_no_entry() {
    let content = handle_action_content();

    // Both annotate and upload should guard on selected entry
    let cleanshot_pos = content
        .find("\"clipboard_annotate_cleanshot\" | \"clipboard_upload_cleanshot\"")
        .expect("Expected combined CleanShot handler");
    let cleanshot_block = &content[cleanshot_pos..content.len().min(cleanshot_pos + 5000)];

    assert!(
        cleanshot_block.contains("cleanshot_action.selection_required_message()")
            && content.contains("Self::Annotate | Self::Upload => \"No clipboard entry selected\""),
        "clipboard_annotate_cleanshot should error when no entry selected"
    );

    assert!(
        cleanshot_block.contains("cleanshot_action.selection_required_message()")
            && content.contains("Self::Annotate | Self::Upload => \"No clipboard entry selected\""),
        "clipboard_upload_cleanshot should error when no entry selected"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_delete (single entry)
// ===========================================================================

#[test]
fn clipboard_delete_single_shows_hud_on_success() {
    let content = handle_action_content();

    let delete_pos = content
        .find("\"clipboard_delete\"")
        .expect("Expected clipboard_delete handler");
    let block = &content[delete_pos..content.len().min(delete_pos + 3000)];

    assert!(
        block.contains("delete_action.success_hud()"),
        "clipboard_delete should show success HUD through named action state"
    );
    assert!(
        block.contains("HUD_SHORT_MS"),
        "clipboard_delete should use HUD_SHORT_MS"
    );
}

#[test]
fn clipboard_delete_single_shows_error_toast_on_failure() {
    let content = handle_action_content();

    let delete_pos = content
        .find("\"clipboard_delete\"")
        .expect("Expected clipboard_delete handler");
    let block = &content[delete_pos..content.len().min(delete_pos + 3000)];

    assert!(
        block.contains("show_error_toast("),
        "clipboard_delete error should use show_error_toast"
    );
}

// ===========================================================================
// Action variant coverage: clipboard_save_file
// ===========================================================================

#[test]
fn clipboard_save_file_shows_hud_on_success() {
    let content = handle_action_content();

    let save_pos = content
        .find("\"clipboard_save_file\"")
        .expect("Expected clipboard_save_file handler");
    let block = &content[save_pos..content.len().min(save_pos + 4000)];

    assert!(
        block.contains("save_file_action.saved_hud(&save_path)"),
        "clipboard_save_file should show saved-path HUD through named action state"
    );
    assert!(
        block.contains("HUD_LONG_MS"),
        "clipboard_save_file should use HUD_LONG_MS"
    );
}

#[test]
fn clipboard_save_file_shows_error_toast_on_failure() {
    let content = handle_action_content();

    let save_pos = content
        .find("\"clipboard_save_file\"")
        .expect("Expected clipboard_save_file handler");
    let block = &content[save_pos..content.len().min(save_pos + 3000)];

    assert!(
        block.contains("show_error_toast("),
        "clipboard_save_file error should use show_error_toast"
    );
}

// ===========================================================================
// Action variant coverage: create_script
// ===========================================================================

#[test]
fn create_script_shows_hud_on_success() {
    let content = handle_action_content();

    let create_pos = content
        .find("\"create_script\" => {")
        .expect("Expected create_script handler");
    let block = &content[create_pos..content.len().min(create_pos + 3000)];

    assert!(
        block.contains("management_action.success_hud()")
            && content.contains("Opened scripts folder"),
        "create_script should show 'Opened scripts folder' HUD"
    );
    assert!(
        block.contains("HUD_SHORT_MS"),
        "create_script should use HUD_SHORT_MS"
    );
}

#[test]
fn create_script_shows_error_toast_on_failure() {
    let content = handle_action_content();

    let create_pos = content
        .find("\"create_script\" => {")
        .expect("Expected create_script handler");
    let block = &content[create_pos..content.len().min(create_pos + 3000)];

    assert!(
        block.contains("management_action.open_failure_message(e)")
            && block.contains("show_error_toast("),
        "create_script failure should use show_error_toast"
    );
}

// ===========================================================================
// Action variant coverage: run_script
// ===========================================================================

#[test]
fn run_script_calls_execute_selected() {
    let content = handle_action_content();

    let run_pos = content
        .find("\"run_script\"")
        .expect("Expected run_script handler");
    let block = &content[run_pos..content.len().min(run_pos + 3000)];

    assert!(
        block.contains("self.execute_selected(cx)"),
        "run_script should call execute_selected"
    );
}

// ===========================================================================
// Action variant coverage: view_logs
// ===========================================================================

#[test]
fn view_logs_toggles_log_panel() {
    let content = handle_action_content();

    let logs_pos = content
        .find("\"view_logs\"")
        .expect("Expected view_logs handler");
    let block = &content[logs_pos..content.len().min(logs_pos + 3000)];

    assert!(
        block.contains("self.toggle_logs(cx)"),
        "view_logs should call toggle_logs"
    );
}

// ===========================================================================
// Action variant coverage: reveal_in_finder
// ===========================================================================

#[test]
fn reveal_in_finder_shows_hud_on_success() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"reveal_in_finder\" =>", 3000);

    assert!(
        block.contains("reveal_action.success_hud()"),
        "reveal_in_finder should show success HUD through named action state"
    );
    assert!(
        block.contains("HUD_SHORT_MS"),
        "reveal_in_finder should use HUD_SHORT_MS"
    );
}

#[test]
fn reveal_in_finder_shows_error_toast_for_unsupported_types() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"reveal_in_finder\" =>", 4000);

    assert!(
        block.contains("reveal_action.unsupported_message()"),
        "reveal_in_finder should show unsupported-type error through named action state"
    );
}

// ===========================================================================
// Action variant coverage: copy_path
// ===========================================================================

#[test]
fn copy_path_uses_clipboard_feedback_helper() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"copy_path\" =>", 3000);

    assert!(
        block.contains("copy_to_clipboard_with_feedback("),
        "copy_path should use copy_to_clipboard_with_feedback"
    );
}

#[test]
fn copy_path_shows_error_for_unsupported_types() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"copy_path\" =>", 3000);

    assert!(
        block.contains("extract_path_for_copy") || block.contains("resolve_file_action_path"),
        "copy_path should use shared path extraction helper for type-specific errors"
    );
}

#[test]
fn copy_path_shows_selection_required_when_no_selection() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"copy_path\" =>", 3000);

    assert!(
        block.contains("selection_required_message_for_action(action_id)"),
        "copy_path should use selection_required_message when no selection"
    );
}

// ===========================================================================
// Action variant coverage: copy_deeplink
// ===========================================================================

#[test]
fn copy_deeplink_uses_clipboard_feedback_helper() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"copy_deeplink\" =>", 3000);

    assert!(
        block.contains("copy_to_clipboard_with_feedback("),
        "copy_deeplink should use copy_to_clipboard_with_feedback"
    );
}

#[test]
fn copy_deeplink_formats_scriptkit_url() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"copy_deeplink\" =>", 3000);

    assert!(
        block.contains("Self::deeplink_for_result(&result)")
            && content.contains("scriptkit://run/"),
        "copy_deeplink should route URL generation through deeplink_for_result with a scriptkit:// fallback"
    );
}

#[test]
fn copy_deeplink_shows_selection_required_when_no_selection() {
    let content = handle_action_content();

    let block = handler_branch_block(&content, "\"copy_deeplink\" =>", 3000);

    assert!(
        block.contains("selection_required_message_for_action(action_id)"),
        "copy_deeplink should use selection_required_message when no selection"
    );
}

// ===========================================================================
// Action variant coverage: quit
// ===========================================================================

#[test]
fn quit_calls_cx_quit() {
    let content = handle_action_content();

    let quit_pos = content.find("\"quit\"").expect("Expected quit handler");
    let block = &content[quit_pos..content.len().min(quit_pos + 3000)];

    assert!(block.contains("cx.quit()"), "quit should call cx.quit()");
}

// ===========================================================================
// Action variant coverage: clipboard_paste (main)
// ===========================================================================

#[test]
fn clipboard_paste_hides_window_and_spawns_paste_simulation() {
    let content = handle_action_content();

    let paste_pos = content
        .find("\"clipboard_paste\" | \"clipboard_copy\" | \"clipboard_paste_keep_open\"")
        .expect("Expected clipboard_paste handler");
    let block = &content[paste_pos..content.len().min(paste_pos + 3000)];

    assert!(
        block.contains("copy_paste_action.paste_close_behavior()")
            && block.contains("finalize_paste_after_clipboard_ready(")
            && content.contains("Self::PasteAndClose => Some(PasteCloseBehavior::HideWindow)"),
        "clipboard_paste should hide the main window before pasting"
    );
    assert!(
        block.contains("finalize_paste_after_clipboard_ready("),
        "clipboard_paste should call the shared paste finalizer"
    );
}

// ===========================================================================
// Cross-cutting: all clipboard actions guard on selected entry
// ===========================================================================

#[test]
fn all_clipboard_actions_guard_on_no_entry_selected() {
    let content = handle_action_content();

    let clipboard_actions = [
        "clipboard_pin",
        "clipboard_share",
        "clipboard_paste",
        "clipboard_attach_to_ai",
        "clipboard_copy",
        "clipboard_paste_keep_open",
        "clipboard_quick_look",
    ];

    for action in &clipboard_actions {
        let pos = content
            .find(&format!("\"{}\"", action))
            .unwrap_or_else(|| panic!("Expected {} handler", action));
        let block = &content[pos..content.len().min(pos + 3000)];

        assert!(
            block.contains("No clipboard entry selected")
                || block.contains("selected_clipboard_entry"),
            "{action} should guard on clipboard entry being selected"
        );
    }
}

// ===========================================================================
// Cross-cutting: all error messages are surfaced via show_error_toast
// ===========================================================================

#[test]
fn error_toast_helper_is_defined_and_uses_correct_duration() {
    let content = handle_action_content();

    assert!(
        content.contains("fn show_error_toast("),
        "show_error_toast helper should be defined in handle_action/"
    );
    assert!(
        content.contains("TOAST_ERROR_MS"),
        "show_error_toast should use TOAST_ERROR_MS"
    );
}

// ===========================================================================
// Cross-cutting: all HUD calls use named constants
// ===========================================================================

#[test]
fn all_show_hud_calls_use_named_duration_constants() {
    let content = handle_action_content();

    let named_constants = [
        "HUD_FLASH_MS",
        "HUD_SHORT_MS",
        "HUD_MEDIUM_MS",
        "HUD_2200_MS",
        "HUD_2500_MS",
        "HUD_LONG_MS",
        "HUD_CONFLICT_MS",
        "HUD_SLOW_MS",
    ];

    // Every show_hud call with Some() should use a named constant
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }
        if trimmed.contains("show_hud(") && trimmed.contains("Some(") {
            let uses_named = named_constants.iter().any(|c| trimmed.contains(c));
            assert!(
                uses_named,
                "show_hud() call must use a named duration constant:\n  {}",
                trimmed
            );
        }
    }
}

// ===========================================================================
// Completeness: every action string literal has a handler
// ===========================================================================

#[test]
fn all_known_action_ids_are_handled() {
    let content = handle_action_content();

    let expected_actions = [
        "clipboard_pin",
        "clipboard_unpin",
        "clipboard_share",
        "clipboard_paste",
        "clipboard_attach_to_ai",
        "clipboard_copy",
        "clipboard_paste_keep_open",
        "clipboard_quick_look",
        "clipboard_open_with",
        "clipboard_annotate_cleanshot",
        "clipboard_upload_cleanshot",
        "clipboard_ocr",
        "clipboard_delete",
        "clipboard_delete_multiple",
        "clipboard_delete_all",
        "clipboard_save_file",
        "clipboard_save_snippet",
        "create_script",
        "run_script",
        "view_logs",
        "reveal_in_finder",
        "copy_path",
        "copy_deeplink",
        "configure_shortcut",
        "add_shortcut",
        "update_shortcut",
        "remove_shortcut",
        "add_alias",
        "update_alias",
        "remove_alias",
        "edit_script",
        "remove_script",
        "delete_script",
        "reload_scripts",
        "settings",
        "quit",
        "__cancel__",
        "open_file",
        "open_directory",
        "quick_look",
        "open_with",
        "show_info",
        "attach_to_ai",
        "copy_filename",
        "edit_scriptlet",
        "reveal_scriptlet_in_finder",
        "copy_scriptlet_path",
        "copy_content",
        "reset_ranking",
    ];

    let missing: Vec<&&str> = expected_actions
        .iter()
        .filter(|id| !content.contains(&format!("\"{}\"", id)))
        .collect();

    assert!(
        missing.is_empty(),
        "Action IDs missing handlers in handle_action/: {:?}",
        missing
    );
}

// ===========================================================================
// Source-scanning: no raw duration values in ANY action file
// ===========================================================================

/// Collect the source code of all action-related files that should follow
/// the named-duration-constant rule.
fn all_action_file_contents() -> Vec<(&'static str, String)> {
    let paths: &[&str] = &[
        "src/app_actions/handle_action/mod.rs",
        "src/app_actions/handle_action/clipboard.rs",
        "src/app_actions/handle_action/scripts.rs",
        "src/app_actions/handle_action/shortcuts.rs",
        "src/app_actions/handle_action/files.rs",
        "src/app_actions/handle_action/scriptlets.rs",
        "src/app_actions/helpers.rs",
        "src/app_actions/sdk_actions.rs",
    ];

    paths
        .iter()
        .filter_map(|path| {
            // Some files may not exist yet (transitional refactor state); skip those.
            std::fs::read_to_string(path)
                .ok()
                .map(|content| (*path, content))
        })
        .collect()
}

#[test]
fn no_raw_duration_ms_values_in_any_action_file() {
    let re = regex::Regex::new(r"duration_ms\(Some\(\d+\)").expect("regex");

    let mut violations = Vec::new();
    for (path, content) in all_action_file_contents() {
        // Only check production code — skip everything after #[cfg(test)]
        let production_code = content.split("#[cfg(test)]").next().unwrap_or(&content);

        for (line_no, line) in production_code.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if re.is_match(trimmed) {
                violations.push(format!("{}:L{}: {}", path, line_no + 1, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "duration_ms() calls must use named constants (TOAST_ERROR_MS, HUD_SHORT_MS, etc.), \
         not raw numbers:\n{}",
        violations.join("\n")
    );
}

#[test]
fn no_raw_show_hud_duration_values_in_any_action_file() {
    let re = regex::Regex::new(r"show_hud\([^)]*Some\(\d+\)").expect("regex");

    let mut violations = Vec::new();
    for (path, content) in all_action_file_contents() {
        let production_code = content.split("#[cfg(test)]").next().unwrap_or(&content);

        for (line_no, line) in production_code.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if re.is_match(trimmed) {
                violations.push(format!("{}:L{}: {}", path, line_no + 1, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "show_hud() calls must use named constants, not raw numbers:\n{}",
        violations.join("\n")
    );
}

// ===========================================================================
// Source-scanning: tracing calls use structured fields, not format strings
// ===========================================================================

#[test]
fn tracing_calls_use_structured_fields_not_format_strings() {
    // Matches patterns like: tracing::info!("message {} value", x)
    // These should use structured fields instead: tracing::info!(key = %x, "message")
    let format_string_re =
        regex::Regex::new(r#"tracing::(info|warn|error|debug|trace)!\(\s*"[^"]*\{[^}]*\}"#)
            .expect("regex");

    // Also matches: tracing::info!(format!(...))
    let format_macro_re =
        regex::Regex::new(r#"tracing::(info|warn|error|debug|trace)!\(\s*format!"#).expect("regex");

    let mut violations = Vec::new();
    for (path, content) in all_action_file_contents() {
        let production_code = content.split("#[cfg(test)]").next().unwrap_or(&content);

        for (line_no, line) in production_code.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if format_string_re.is_match(trimmed) || format_macro_re.is_match(trimmed) {
                violations.push(format!("{}:L{}: {}", path, line_no + 1, trimmed));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Tracing calls must use structured fields (key = %val, \"message\"), \
         not format strings (\"message {{}}\", val):\n{}",
        violations.join("\n")
    );
}

// ===========================================================================
// Source-scanning: no show_hud + show_error_toast in the same action arm
// ===========================================================================

#[test]
fn no_mixed_hud_and_error_toast_in_same_action_function() {
    // Each handle_*_action function should not have any single match arm
    // that calls both show_hud for success AND show_error_toast for an
    // error *outside* of a proper Ok/Err or if/else branch. This test
    // catches the gross case: a function that calls both in a way that
    // suggests the same code path uses both feedback types.
    //
    // We check each match arm independently by splitting on the action
    // string literal pattern.
    let content = handle_action_content();

    // Split at each "action_id" => { ... pattern to get approximate match arms
    let action_re = regex::Regex::new(r#""[a-z_]+" =>"#).expect("regex");
    let arms: Vec<_> = action_re.split(&content).collect();

    // We don't need to flag arms that properly separate success/error paths.
    // The test catches arms that call show_hud THEN show_error_toast (or vice
    // versa) at the same nesting level — a sign of mixed feedback.
    //
    // For now we just verify the aggregate: in the helper methods, show_hud
    // is never directly followed by show_error_toast in the same method body.
    // This is a safeguard, not a perfect static analysis.
    for (i, arm) in arms.iter().enumerate() {
        let has_hud = arm.contains("show_hud(");
        let has_error_toast = arm.contains("show_error_toast(");

        if has_hud && has_error_toast {
            // Acceptable if they're on different branches (Ok/Err, if/else).
            // Simple heuristic: if "Ok(" or "Err(" or "match " appears, it's
            // branched and acceptable.
            let is_branched = arm.contains("Ok(")
                || arm.contains("Err(")
                || arm.contains("match ")
                || arm.contains("if let ")
                || arm.contains("} else {");

            assert!(
                is_branched,
                "Match arm #{} appears to call both show_hud and show_error_toast \
                 without a clear success/error branch separation. \
                 Use show_hud for success, show_error_toast for errors, never both on the same path.\n\
                 Arm preview: {}",
                i,
                &arm[..arm.len().min(300)]
            );
        }
    }
}

// ===========================================================================
// Source-scanning: every handle_*_action sub-handler returns bool
// ===========================================================================

#[test]
fn all_handle_action_sub_handlers_return_bool() {
    let sub_handler_files: &[&str] = &[
        "src/app_actions/handle_action/clipboard.rs",
        "src/app_actions/handle_action/scripts.rs",
        "src/app_actions/handle_action/shortcuts.rs",
        "src/app_actions/handle_action/files.rs",
        "src/app_actions/handle_action/scriptlets.rs",
    ];

    let fn_sig_re = regex::Regex::new(r"fn handle_\w+_action\(").expect("regex");

    for path in sub_handler_files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // File may not exist yet during refactoring
        };

        // Find every handle_*_action function signature
        for mat in fn_sig_re.find_iter(&content) {
            let start = mat.start();
            // Look ahead for the return type annotation
            let sig_block = &content[start..content.len().min(start + 300)];

            assert!(
                sig_block.contains("-> DispatchOutcome"),
                "Sub-handler in {} must return DispatchOutcome. Found signature: {}",
                path,
                &sig_block[..sig_block.len().min(120)]
            );
        }
    }

    // Also check the monolithic file for the same signatures
    let monolithic = handle_action_content();
    for mat in fn_sig_re.find_iter(&monolithic) {
        let start = mat.start();
        let sig_block = &monolithic[start..monolithic.len().min(start + 300)];

        assert!(
            sig_block.contains("-> DispatchOutcome"),
            "Sub-handler in handle_action/ must return DispatchOutcome. Found signature: {}",
            &sig_block[..sig_block.len().min(120)]
        );
    }
}

// ===========================================================================
// Source-scanning: destructive builtins require confirmation
// ===========================================================================

#[test]
fn destructive_builtin_ids_are_in_default_confirmation_commands() {
    let defaults_content = read("src/config/defaults.rs");

    // Extract the DEFAULT_CONFIRMATION_COMMANDS list entries
    let conf_start = defaults_content
        .find("DEFAULT_CONFIRMATION_COMMANDS")
        .expect("Expected DEFAULT_CONFIRMATION_COMMANDS in defaults.rs");
    let conf_block = &defaults_content[conf_start..];
    let conf_end = conf_block.find("];").expect("Expected closing ];");
    let conf_list = &conf_block[..conf_end];

    // Destructive builtin IDs — any builtin command that deletes data, shuts down,
    // or performs an irreversible system action must appear in this list.
    let destructive_ids = [
        "builtin/shut-down",
        "builtin/restart",
        "builtin/log-out",
        "builtin/empty-trash",
        "builtin/sleep",
        "builtin/force-quit",
        "builtin/stop-all-processes",
        "builtin/clear-suggested",
    ];

    let missing: Vec<&&str> = destructive_ids
        .iter()
        .filter(|id| !conf_list.contains(*id))
        .collect();

    assert!(
        missing.is_empty(),
        "Destructive builtin IDs missing from DEFAULT_CONFIRMATION_COMMANDS: {:?}\n\
         All destructive builtins must require confirmation by default.",
        missing
    );
}

#[test]
fn is_destructive_action_catches_all_known_patterns() {
    // Verify the is_destructive_action function exists and covers key patterns
    let dialog_content = read("src/actions/dialog.rs");

    let fn_start = dialog_content
        .find("fn is_destructive_action(")
        .expect("Expected is_destructive_action function in dialog.rs");
    let fn_block = &dialog_content[fn_start..dialog_content.len().min(fn_start + 600)];

    // Must check for remove_, delete_, _delete, _trash patterns
    let required_patterns = [
        "remove_",
        "delete_",
        "_delete",
        "_trash",
        "force_quit_app",
        "reset_ranking",
        "clear_conversation",
    ];

    for pattern in &required_patterns {
        assert!(
            fn_block.contains(pattern),
            "is_destructive_action must check for '{}' pattern",
            pattern
        );
    }
}

#[test]
fn actions_dialog_state_uses_shared_destructive_detection() {
    let prompt_handler = read("src/prompt_handler/mod.rs");
    let actions_mod = read("src/actions/mod.rs");

    assert!(
        prompt_handler.contains("\"visibleActions\"")
            && prompt_handler.contains("\"destructive\": crate::actions::is_destructive_action(action)"),
        "getState actionsDialog.visibleActions must report the same destructive status used by the renderer"
    );
    assert!(
        actions_mod.contains("pub(crate) use dialog::")
            && actions_mod.contains("is_destructive_action"),
        "shared destructive detection should be available to state receipts"
    );
}
