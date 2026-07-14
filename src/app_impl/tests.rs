use super::{calculate_fallback_error_message, ScriptListApp};
use std::fs;

#[test]
fn test_sync_builtin_query_state_updates_query_and_selection_when_changed() {
    let mut query = String::from("old");
    let mut selected_index = 3;

    let changed = ScriptListApp::sync_builtin_query_state(&mut query, &mut selected_index, "new");

    assert!(changed);
    assert_eq!(query, "new");
    assert_eq!(selected_index, 0);
}

#[test]
fn test_sync_builtin_query_state_noop_when_query_is_unchanged() {
    let mut query = String::from("same");
    let mut selected_index = 4;

    let changed = ScriptListApp::sync_builtin_query_state(&mut query, &mut selected_index, "same");

    assert!(!changed);
    assert_eq!(query, "same");
    assert_eq!(selected_index, 4);
}

#[test]
fn test_clear_builtin_query_state_clears_text_and_resets_selection() {
    let mut query = String::from("abc");
    let mut selected_index = 2;

    ScriptListApp::clear_builtin_query_state(&mut query, &mut selected_index);

    assert!(query.is_empty());
    assert_eq!(selected_index, 0);
}

#[test]
fn test_theme_chooser_filter_changes_repreview_first_match() {
    // Both filter entry points — real typing (handle_filter_input_change)
    // and protocol setFilter (set_filter_text_immediate) — must route
    // through the shared side-effects helper so automation and users see
    // identical preview behavior.
    let typing_path = fs::read_to_string("src/app_impl/filter_input_change.rs")
        .expect("Failed to read src/app_impl/filter_input_change.rs");
    let protocol_path = fs::read_to_string("src/app_impl/filter_input_updates.rs")
        .expect("Failed to read src/app_impl/filter_input_updates.rs");
    assert!(
        typing_path.contains("self.apply_theme_chooser_filter_change_effects(cx);"),
        "Typed theme chooser filter changes should run the shared preview side effects"
    );
    assert!(
        protocol_path.contains("self.apply_theme_chooser_filter_change_effects(cx);"),
        "Protocol setFilter on ThemeChooserView should run the shared preview side effects"
    );

    let helper = fs::read_to_string("src/render_builtins/theme_chooser.rs")
        .expect("Failed to read src/render_builtins/theme_chooser.rs");
    assert!(
        helper.contains("theme_chooser_filter_preview"),
        "Theme chooser filter changes should trigger a live preview refresh"
    );
    assert!(
        helper.contains("self.preview_theme_chooser_catalog_entry("),
        "Theme chooser filter effects should reuse the unified catalog preview pipeline"
    );
    assert!(
        helper.contains("Self::theme_chooser_catalog_filtered_indices("),
        "Theme chooser filter effects must filter the unified catalog, not presets only"
    );
    assert!(
        helper.contains("self.apply_theme_chooser_filter_hex_preview(hex, cx);"),
        "Pasted #RRGGBB queries should live-preview as the accent color"
    );
}

#[test]
fn test_calculate_fallback_error_message_includes_expression_and_recovery() {
    let message = calculate_fallback_error_message("2 + )");
    assert!(message.contains("2 + )"));
    assert!(message.contains("Could not evaluate expression"));
    assert!(message.contains("Check the syntax and try again"));
}

#[test]
fn test_shift_tab_no_longer_routes_through_harness_entry_intent() {
    let startup_tab = fs::read_to_string("src/app_impl/startup_new_tab.rs")
        .expect("Failed to read src/app_impl/startup_new_tab.rs");

    assert!(
        !startup_tab.contains("submit_to_current_or_new_tab_ai_harness_from_text"),
        "Shift+Tab in ScriptList should no longer route through the quick-submit planner"
    );
}

#[test]
fn test_cmd_enter_routes_to_harness_terminal_in_startup_new_tab() {
    let startup_tab = fs::read_to_string("src/app_impl/startup_new_tab.rs")
        .expect("Failed to read src/app_impl/startup_new_tab.rs");

    assert!(
        startup_tab.contains("try_route_global_cmd_enter_to_agent_chat_context_capture"),
        "Cmd+Enter in startup_new_tab.rs should route through the global AI-entry helper"
    );
}

#[test]
fn test_tab_interceptor_matches_tab_key_case_insensitive() {
    let startup_tab = fs::read_to_string("src/app_impl/startup_new_tab.rs")
        .expect("Failed to read src/app_impl/startup_new_tab.rs");

    assert!(
        startup_tab.contains("eq_ignore_ascii_case(\"tab\")"),
        "Tab interceptor should match tab key case-insensitively"
    );
}

#[test]
fn test_emoji_picker_arrow_interceptor_consumes_left_right_keys_before_input() {
    let startup = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");

    assert!(
        startup.contains("let is_left = crate::ui_foundation::is_key_left(key);")
            && startup.contains("let is_right = crate::ui_foundation::is_key_right(key);"),
        "arrow interceptor should normalize both left/right key variants"
    );

    assert!(
        startup.contains("if (is_left || is_right) && no_direction_modifiers"),
        "arrow interceptor should explicitly handle plain left/right arrows"
    );

    let horizontal_interceptor_start = startup
        .find("if (is_left || is_right) && no_direction_modifiers")
        .expect("left/right arrow interceptor block should exist in startup.rs");
    let horizontal_interceptor_end = (horizontal_interceptor_start + 2400).min(startup.len());
    let horizontal_interceptor = &startup[horizontal_interceptor_start..horizontal_interceptor_end];
    assert!(
        horizontal_interceptor.contains("if let AppView::EmojiPickerView {")
            && horizontal_interceptor.contains("*selected_index = if is_left")
            && horizontal_interceptor.contains("cx.stop_propagation();"),
        "EmojiPickerView left/right handling must navigate and stop propagation to Input"
    );
}

#[test]
fn test_browser_tabs_builtin_is_available_to_stdin_trigger_routes() {
    // Registry contract: the canonical name + every legacy alias must
    // resolve to TriggerBuiltin::BrowserTabs. If this fails, no amount
    // of dispatcher wiring will route browser-tabs correctly.
    use crate::builtins::trigger_registry::{registry, TriggerBuiltin};
    for alias in [
        "browser-tabs",
        "browsertabs",
        "tabs",
        "builtin/browser-tabs",
    ] {
        assert_eq!(
            registry().resolve(alias),
            Some(TriggerBuiltin::BrowserTabs),
            "alias '{alias}' should resolve to TriggerBuiltin::BrowserTabs"
        );
    }

    // Dispatcher parity: the three stdin ingress sites (the real one +
    // the two orphan audit mirrors) MUST all delegate to the shared
    // helper so the match arms can never drift apart again. They must
    // pass the whole `ExternalCommand` through the wrapper (not the raw
    // `name`), so the canonical-vs-deprecated `builtin_id` / `name`
    // normalization in `ExternalCommand::trigger_builtin_ref` runs in
    // exactly one place.
    for path in [
        "src/main_entry/runtime_stdin_match_core.rs",
        "src/main_entry/runtime_stdin.rs",
        "src/main_entry/app_run_setup.rs",
    ] {
        let source = fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {path}"));
        assert!(
            source.contains("view.dispatch_trigger_builtin(cmd, window, ctx)"),
            "{path} must delegate triggerBuiltin to the shared dispatcher helper \
             via the `cmd @ ExternalCommand::TriggerBuiltin {{ .. }}` rebinding \
             (see src/app_impl/trigger_builtin_dispatch.rs)"
        );
        assert!(
            !source.contains("dispatch_trigger_builtin_name(name, window, ctx)"),
            "{path} must not call the raw-name helper directly — route through \
             `dispatch_trigger_builtin` so deprecated-name counters fire"
        );
        assert!(
            !source.contains("\"browser-tabs\" | \"browsertabs\" | \"tabs\""),
            "{path} must not contain inline browser-tabs match arms — dispatch \
             lives in the shared helper"
        );
    }
}

// ---------------------------------------------------------------------------
// Tab AI harness routing contract tests (startup.rs parity)
// ---------------------------------------------------------------------------

#[test]
fn script_list_shift_tab_no_longer_routes_into_harness_entry_intent_in_standard_startup() {
    let source = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");

    assert!(
        !source.contains("submit_to_current_or_new_tab_ai_harness_from_text"),
        "Shift+Tab in ScriptList must no longer route the filter text through the quick-submit planner"
    );
}

#[test]
fn quick_terminal_tab_is_written_directly_to_pty_in_standard_startup() {
    let source = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");

    assert!(
        source.contains("b\"\\t\""),
        "QuickTerminal must forward Tab directly to the PTY"
    );
    assert!(
        source.contains("b\"\\x1b[Z\""),
        "QuickTerminal must forward Shift+Tab/backtab directly to the PTY"
    );
    assert!(
        source.contains("term.terminal.input(bytes)"),
        "QuickTerminal Tab handling must write raw bytes to the PTY"
    );
}

#[test]
fn plain_tab_in_script_list_no_longer_routes_to_agent_chat_in_standard_startup() {
    let source = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");

    assert!(
        source.contains("Tab-to-Agent deprecated: Cmd+Enter is the AI entry.")
            && !source.contains("this.try_route_plain_tab_to_agent_chat_context_capture(cx)"),
        "Plain Tab in ScriptList must not route through the Agent Chat handoff helper in startup.rs"
    );
}

#[test]
fn plain_tab_agent_chat_helper_is_removed() {
    let source = fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
        .expect("Failed to read src/app_impl/agent_handoff/mod.rs");

    assert!(
        !source.contains("try_route_plain_tab_to_agent_chat_context_capture")
            && !source.contains("agent_chat_plain_tab_entry_deprecated")
            && !source.contains("tab_ai_plain_tab_routed_to_agent_chat")
            && !source.contains("tab_ai_plain_tab_submitted_to_detached_agent_chat"),
        "Plain Tab must not keep an Agent Chat route, shim, or telemetry marker"
    );
}

#[test]
fn simulate_key_tab_does_not_use_plain_tab_agent_chat_helper() {
    let helper_source = fs::read_to_string("src/app_impl/simulate_key_dispatch.rs")
        .expect("Failed to read src/app_impl/simulate_key_dispatch.rs");
    let app_run_setup = fs::read_to_string("src/main_entry/app_run_setup.rs")
        .expect("Failed to read src/main_entry/app_run_setup.rs");
    let runtime_match = fs::read_to_string("src/main_entry/runtime_stdin_match_simulate_key.rs")
        .expect("Failed to read src/main_entry/runtime_stdin_match_simulate_key.rs");

    assert!(
        !helper_source.contains("try_route_plain_tab_to_agent_chat_context_capture"),
        "simulateKey Tab must not route through the deprecated plain-Tab Agent Chat helper"
    );
    assert!(
        app_run_setup.contains("dispatch_simulate_key")
            && runtime_match.contains("dispatch_simulate_key"),
        "Both entry points must delegate to dispatch_simulate_key helper"
    );
}

/// Contract test for `tool-table-driven-simulatekey` (AFK Run 2 Pass #4).
///
/// The unified simulateKey dispatcher must emit a structured `unhandled_view` code
/// when the current view has no arm, instead of a silent debug log. This
/// guards against a regression where a refactor drops the loud receipt and
/// unhandled views become undetectable from outside the dispatcher.
#[test]
fn simulate_key_dispatchers_emit_unhandled_view_receipt() {
    let helper_source = fs::read_to_string("src/app_impl/simulate_key_dispatch.rs")
        .expect("Failed to read src/app_impl/simulate_key_dispatch.rs");

    assert!(
        helper_source.contains("simulateKey_unhandled_view"),
        "simulate_key_dispatch.rs must emit a `simulateKey_unhandled_view` tracing event from its catch-all arm"
    );
    assert!(
        helper_source.contains("code = \"unhandled_view\""),
        "simulate_key_dispatch.rs must label the unhandled-view event with `code = \"unhandled_view\"` so receipts are machine-parseable"
    );
    assert!(
        helper_source.contains("SimulateKey: UNHANDLED_VIEW"),
        "simulate_key_dispatch.rs must also write a plain-text `SimulateKey: UNHANDLED_VIEW` line via logging::log for operator logs"
    );
    assert!(
        helper_source.contains("view.app_view_name()"),
        "simulate_key_dispatch.rs must name the unhandled view via `app_view_name()` instead of emitting an opaque discriminant"
    );
}

#[test]
fn confirm_popup_guards_global_launcher_interceptors() {
    let startup = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");
    let startup_new_tab = fs::read_to_string("src/app_impl/startup_new_tab.rs")
        .expect("Failed to read src/app_impl/startup_new_tab.rs");
    let startup_new_arrow = fs::read_to_string("src/app_impl/startup_new_arrow.rs")
        .expect("Failed to read src/app_impl/startup_new_arrow.rs");
    let startup_new_actions = fs::read_to_string("src/app_impl/startup_new_actions.rs")
        .expect("Failed to read src/app_impl/startup_new_actions.rs");

    for (label, source) in [
        ("startup.rs", startup.as_str()),
        ("startup_new_tab.rs", startup_new_tab.as_str()),
        ("startup_new_arrow.rs", startup_new_arrow.as_str()),
        ("startup_new_actions.rs", startup_new_actions.as_str()),
    ] {
        assert!(
            source.contains("confirm::consume_main_window_key_while_confirm_open("),
            "{label} must guard global key interceptors while a confirm popup is open"
        );
    }
}

#[test]
fn render_impl_routes_modifier_aware_keys_into_confirm_popup_guard() {
    let source = fs::read_to_string("src/main_sections/render_impl.rs")
        .expect("Failed to read src/main_sections/render_impl.rs");

    assert!(
        source.contains("confirm::consume_main_window_key_while_confirm_open("),
        "render_impl.rs must route root key capture through the confirm popup guard"
    );
    assert!(
        source.contains("&event.keystroke.modifiers"),
        "render_impl.rs must pass real modifiers so Shift+Tab stays inside the confirm popup"
    );
}

#[test]
fn script_list_cmd_v_routes_large_pastes_into_agent_chat() {
    let render_script_list = fs::read_to_string("src/render_script_list/mod.rs")
        .expect("Failed to read src/render_script_list/mod.rs");
    let startup = fs::read_to_string("src/app_impl/startup.rs")
        .expect("Failed to read src/app_impl/startup.rs");
    let agent_handoff = fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
        .expect("Failed to read src/app_impl/agent_handoff/mod.rs");

    assert!(
        render_script_list
            .contains("\"v\" if this.route_large_script_list_paste_to_agent_chat(cx)"),
        "Cmd+V in ScriptList should first try the large-paste Agent Chat handoff"
    );
    // The focused filter input consumes Cmd+V (and strips newlines) before the
    // render_script_list key listener can see it, so the live route MUST also
    // exist in the window-level actions interceptor, which runs first.
    assert!(
        startup.contains("&& this.route_large_script_list_paste_to_agent_chat(cx)"),
        "The startup.rs actions interceptor must route ScriptList Cmd+V multi-line/large \
         pastes to Agent Chat before the filter input's newline-stripping paste handler runs"
    );
    assert!(
        agent_handoff.contains("script_list_large_paste_routed_to_agent_chat")
            && agent_handoff.contains("clipboard://pasted-text/")
            && agent_handoff.contains("open_tab_ai_agent_chat_with_context_part(part, \"script_list_large_paste\", cx);"),
        "Large ScriptList pastes should become Agent Chat text-block context instead of staying in the launcher filter"
    );
}

#[test]
fn script_list_cmd_v_routes_clipboard_images_into_agent_chat() {
    let agent_handoff = fs::read_to_string("src/app_impl/agent_handoff/mod.rs")
        .expect("Failed to read src/app_impl/agent_handoff/mod.rs");

    assert!(
        agent_handoff.contains("clipboard.get_image()")
            && agent_handoff.contains("script_list_clipboard_image_routed_to_agent_chat")
            && agent_handoff.contains(
                "open_tab_ai_agent_chat_with_context_part(part, \"script_list_clipboard_image\", cx);"
            ),
        "ScriptList Cmd+V should route clipboard images straight into Agent Chat as file attachments"
    );
}

#[test]
fn agent_chat_clipboard_image_parts_stage_as_pasted_image_pills() {
    let agent_chat_view = fs::read_to_string("src/ai/agent_chat/ui/view.rs")
        .expect("Failed to read src/ai/agent_chat/ui/view.rs");
    let context_mentions = fs::read_to_string("src/ai/context_mentions/mod.rs")
        .expect("Failed to read src/ai/context_mentions/mod.rs");

    assert!(
        agent_chat_view.contains("pasted_image_tokens")
            && agent_chat_view.contains("paste_image_from_clipboard")
            && agent_chat_view.contains("crate::pasted_image::remove_pasted_image_token_at_cursor")
            && agent_chat_view.contains("pasted_image_pill_ranges"),
        "Agent Chat should give pasted clipboard images their own pill registry, paste handler, and atomic delete path"
    );
    assert!(
        context_mentions.contains("crate::pasted_image::token_for_label(label)"),
        "Clipboard-image file parts should map back to their stable @img:pasteN inline token"
    );
}
