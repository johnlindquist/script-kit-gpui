//! Source-scanning regression tests that verify structural patterns in production code.
//!
//! These tests read source files and assert the presence of expected patterns,
//! ensuring coding conventions are maintained across the codebase.

// Re-export shared test utilities so submodules can use `super::*`.
pub use script_kit_gpui::test_utils::{
    count_occurrences, read_all_handle_action_sources, read_source, LIVE_HANDLE_ACTION_FILES,
};

#[path = "source_audits/builtin_confirmation.rs"]
mod builtin_confirmation;

#[path = "source_audits/clipboard_actions.rs"]
mod clipboard_actions;

#[path = "source_audits/clipboard_image_contract.rs"]
mod clipboard_image_contract;

#[path = "source_audits/computer_capture_native_window_contract.rs"]
mod computer_capture_native_window_contract;

#[path = "source_audits/fields_prompt_contract.rs"]
mod fields_prompt_contract;

#[path = "source_audits/execution_helpers.rs"]
mod execution_helpers;

#[path = "source_audits/env_prompt_secret_store.rs"]
mod env_prompt_secret_store;

#[path = "source_audits/shortcut_alias_file_actions.rs"]
mod shortcut_alias_file_actions;

#[path = "source_audits/shortcut_config_source.rs"]
mod shortcut_config_source;

#[path = "source_audits/action_coverage_audit.rs"]
mod action_coverage_audit;

#[path = "source_audits/action_file_clipboard_tools.rs"]
mod action_file_clipboard_tools;

#[path = "source_audits/action_script_management.rs"]
mod action_script_management;

#[path = "source_audits/action_scriptlet_ranking.rs"]
mod action_scriptlet_ranking;

#[path = "source_audits/action_shortcut_alias.rs"]
mod action_shortcut_alias;

#[path = "source_audits/file_action_path_helpers.rs"]
mod file_action_path_helpers;

#[path = "source_audits/structured_logging.rs"]
mod structured_logging;

#[path = "source_audits/memory_skills_discoverability.rs"]
mod memory_skills_discoverability;

#[path = "source_audits/trace_propagation.rs"]
mod trace_propagation;

#[path = "source_audits/consistent_structured_fields.rs"]
mod consistent_structured_fields;

#[path = "source_audits/builtin_dispatch_consistency.rs"]
mod builtin_dispatch_consistency;

#[path = "source_audits/builtin_command_text.rs"]
mod builtin_command_text;

#[path = "source_audits/resize_presentation_contract.rs"]
mod resize_presentation_contract;

#[path = "source_audits/menu_syntax_ai_stale_proposal.rs"]
mod menu_syntax_ai_stale_proposal;

#[path = "source_audits/menu_syntax_handler_form_contract.rs"]
mod menu_syntax_handler_form_contract;

#[path = "source_audits/trigger_builtin_sdk_literals.rs"]
mod trigger_builtin_sdk_literals;

#[path = "source_audits/trigger_builtin_registry_consistency.rs"]
mod trigger_builtin_registry_consistency;

#[path = "source_audits/script_kit_selfie.rs"]
mod script_kit_selfie;

#[path = "source_audits/hotkey_builtin_visibility.rs"]
mod hotkey_builtin_visibility;

#[path = "source_audits/keyword_expansion_latency_contract.rs"]
mod keyword_expansion_latency_contract;

#[path = "source_audits/keyword_monitor_activation_contract.rs"]
mod keyword_monitor_activation_contract;

#[path = "source_audits/emoji_picker.rs"]
mod emoji_picker;

#[path = "source_audits/paste_parity.rs"]
mod paste_parity;

#[path = "source_audits/dialog_tab_navigation.rs"]
mod dialog_tab_navigation;

#[path = "source_audits/no_popup_confirm_callers.rs"]
mod no_popup_confirm_callers;

#[path = "source_audits/confirm_modal_shared_shell.rs"]
mod confirm_modal_shared_shell;

#[path = "source_audits/arrow_interceptor_filtered_bounds.rs"]
mod arrow_interceptor_filtered_bounds;

#[path = "source_audits/mini_main_window.rs"]
mod mini_main_window;

#[path = "source_audits/scroll_reveal.rs"]
mod scroll_reveal;

#[path = "source_audits/main_menu_history_render_perf.rs"]
mod main_menu_history_render_perf;

#[path = "source_audits/launcher_speed_gate_contract.rs"]
mod launcher_speed_gate_contract;

#[path = "source_audits/actions_popup_contract.rs"]
mod actions_popup_contract;

#[path = "source_audits/agent_chat_no_legacy_backend.rs"]
mod agent_chat_no_legacy_backend;

#[path = "source_audits/agent_chat_boundary_and_frozen_contracts.rs"]
mod agent_chat_boundary_and_frozen_contracts;

#[path = "source_audits/main_window_footer_global_left_chips.rs"]
mod main_window_footer_global_left_chips;

#[path = "source_audits/main_window_header_input_parity.rs"]
mod main_window_header_input_parity;

#[path = "source_audits/scriptlist_hide_bounds_reset.rs"]
mod scriptlist_hide_bounds_reset;

#[path = "source_audits/stdin_check_accessibility_wired.rs"]
mod stdin_check_accessibility_wired;

#[path = "source_audits/stdin_get_window_bounds_wired.rs"]
mod stdin_get_window_bounds_wired;

#[path = "source_audits/stdin_frontmost_window_wired.rs"]
mod stdin_frontmost_window_wired;

#[path = "source_audits/stdin_get_selected_text_wired.rs"]
mod stdin_get_selected_text_wired;

#[path = "source_audits/stdin_request_accessibility_wired.rs"]
mod stdin_request_accessibility_wired;

#[path = "source_audits/stdin_set_selected_text_wired.rs"]
mod stdin_set_selected_text_wired;

#[path = "source_audits/selected_text_clipboard_restore.rs"]
mod selected_text_clipboard_restore;

#[path = "source_audits/passive_selection_capture_no_cmd_c.rs"]
mod passive_selection_capture_no_cmd_c;

#[path = "source_audits/root_file_search_contract.rs"]
mod root_file_search_contract;

#[path = "source_audits/root_unified_search_stability_contract.rs"]
mod root_unified_search_stability_contract;

#[path = "source_audits/root_unified_source_filters_contract.rs"]
mod root_unified_source_filters_contract;

#[path = "source_audits/root_unified_source_actions_contract.rs"]
mod root_unified_source_actions_contract;

#[path = "source_audits/root_unified_passive_snapshot_contract.rs"]
mod root_unified_passive_snapshot_contract;

#[path = "source_audits/root_unified_ai_vault_contract.rs"]
mod root_unified_ai_vault_contract;

#[path = "source_audits/root_unified_passive_budget_contract.rs"]
mod root_unified_passive_budget_contract;

#[path = "source_audits/root_unified_passive_source_perf_contract.rs"]
mod root_unified_passive_source_perf_contract;

#[path = "source_audits/sdk_computer_use_contract.rs"]
mod sdk_computer_use_contract;

#[path = "source_audits/root_unified_browser_history_contract.rs"]
mod root_unified_browser_history_contract;

#[path = "source_audits/root_unified_browser_tabs_contract.rs"]
mod root_unified_browser_tabs_contract;

#[path = "source_audits/mcp_computer_list_tray_menu_observation_only.rs"]
mod mcp_computer_list_tray_menu_observation_only;

#[path = "source_audits/root_unified_agent_chat_history_contract.rs"]
mod root_unified_agent_chat_history_contract;

#[path = "source_audits/root_unified_clipboard_history_contract.rs"]
mod root_unified_clipboard_history_contract;

#[path = "source_audits/context_selector_portal_builtin_parity_contract.rs"]
mod context_selector_portal_builtin_parity_contract;

#[path = "source_audits/removed_inline_context_selector.rs"]
mod removed_inline_context_selector;

#[path = "source_audits/day_page_no_inline_spine_overlay.rs"]
mod day_page_no_inline_spine_overlay;

#[path = "source_audits/root_unified_dictation_history_contract.rs"]
mod root_unified_dictation_history_contract;

#[path = "source_audits/root_unified_notes_contract.rs"]
mod root_unified_notes_contract;

#[path = "source_audits/root_unified_windows_contract.rs"]
mod root_unified_windows_contract;

#[path = "source_audits/permiso_builtin_contract.rs"]
mod permiso_builtin_contract;

#[path = "source_audits/permiso_teardown_contract.rs"]
mod permiso_teardown_contract;

#[path = "source_audits/permiso_no_prompt_contract.rs"]
mod permiso_no_prompt_contract;

#[path = "source_audits/mcp_computer_list_permissions_observation_only.rs"]
mod mcp_computer_list_permissions_observation_only;

#[path = "source_audits/computer_list_permissions_contract.rs"]
mod computer_list_permissions_contract;

#[path = "source_audits/verify_shot_pixel_audit_contract.rs"]
mod verify_shot_pixel_audit_contract;

#[path = "source_audits/timestamp_formatting_contract.rs"]
mod timestamp_formatting_contract;

#[path = "source_audits/theme_chooser_single_select_controls.rs"]
mod theme_chooser_single_select_controls;

#[path = "source_audits/focused_text_capture_ax_wired.rs"]
mod focused_text_capture_ax_wired;

#[path = "source_audits/focused_text_geometry_ax_wired.rs"]
mod focused_text_geometry_ax_wired;

#[path = "source_audits/focused_text_mutation_wired.rs"]
mod focused_text_mutation_wired;

#[path = "source_audits/focused_text_agent_chat_entry.rs"]
mod focused_text_agent_chat_entry;

#[path = "source_audits/focused_text_agent_chat_actions.rs"]
mod focused_text_agent_chat_actions;

#[path = "source_audits/focused_text_agent_chat_submission.rs"]
mod focused_text_agent_chat_submission;

#[path = "source_audits/inline_ai_double_command_trigger_wired.rs"]
mod inline_ai_double_command_trigger_wired;

#[path = "source_audits/stdin_focused_text_protocol_wired.rs"]
mod stdin_focused_text_protocol_wired;

/// Regression guard: fails if the deleted monolithic `handle_action.rs` file
/// reappears or if any `.rs` file under `src/` or `tests/` references the old
/// monolith path. This prevents accidental resurrection of the pre-split handler.
#[cfg(test)]
mod no_old_monolith {
    use std::path::Path;

    /// The old monolithic handler file must not exist on disk.
    #[test]
    fn test_no_old_monolith_file_exists() {
        let old_monolith = Path::new("src/app_actions/handle_action.rs");
        assert!(
            !old_monolith.exists(),
            "Old monolithic handler file still exists at {old_monolith:?}. \
             It was replaced by the modular split under src/app_actions/handle_action/."
        );
    }

    /// No `.rs` source file should contain a string-literal reference to the
    /// old monolith path (`handle_action.rs` as a bare file, not the directory).
    #[test]
    fn test_no_old_monolith_references() {
        let needle = "handle_action.rs";
        let mut violations = Vec::new();

        for dir in &["src", "tests"] {
            collect_rs_files_with_needle(Path::new(dir), needle, &mut violations);
        }

        // Exclude this very test file — it legitimately mentions the old path.
        let self_path = Path::new("tests/source_audits.rs")
            .canonicalize()
            .unwrap_or_default();
        violations.retain(|v: &String| {
            let canon = Path::new(v.split(':').next().unwrap_or(""))
                .canonicalize()
                .unwrap_or_default();
            canon != self_path
        });

        assert!(
            violations.is_empty(),
            "Found references to the deleted monolith path `{needle}` in:\n{}",
            violations.join("\n")
        );
    }

    fn collect_rs_files_with_needle(dir: &Path, needle: &str, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files_with_needle(&path, needle, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (i, line) in content.lines().enumerate() {
                        if line.contains(needle) {
                            out.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
}
