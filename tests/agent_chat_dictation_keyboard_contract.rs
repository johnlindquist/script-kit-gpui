//! Source-level contracts for embedded Agent Chat keyboard/window close behavior
//! after dictation opens Agent Chat with an initial submitted prompt.

const STARTUP_SOURCE: &str = include_str!("../src/app_impl/startup.rs");
const STARTUP_NEW_ACTIONS_SOURCE: &str = include_str!("../src/app_impl/startup_new_actions.rs");
const APP_RUN_SETUP_SOURCE: &str = include_str!("../src/main_entry/app_run_setup.rs");
const TAB_AI_MODE_SOURCE: &str = include_str!("../src/app_impl/agent_handoff/mod.rs");
const AGENT_CHAT_VIEW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source.find(start).expect("start marker must exist");
    let tail = &source[start_idx..];
    let end_idx = tail.find(end).expect("end marker must exist");
    &tail[..end_idx]
}

fn assert_ordered(section: &str, before: &str, after: &str) {
    let before_idx = section.find(before).expect("before marker must exist");
    let after_idx = section.find(after).expect("after marker must exist");
    assert!(
        before_idx < after_idx,
        "`{before}` must appear before `{after}` in:\n{section}"
    );
}

#[test]
fn embedded_agent_chat_escape_routes_to_lifecycle_close_without_hiding_main() {
    for (name, source, end_marker) in [
        ("startup.rs", STARTUP_SOURCE, "// Handle Cmd+- to decrease"),
        (
            "startup_new_actions.rs",
            STARTUP_NEW_ACTIONS_SOURCE,
            "// Handle Cmd+I to toggle info panel",
        ),
    ] {
        let section = section_between(source, "let agent_chat_escape_popup_open", end_marker);
        assert!(
            section.contains("has_escape_dismissible_popup")
                && section.contains("!this.show_actions_popup")
                && section.contains("embedded_agent_chat_escape_return_to_origin")
                && section.contains("close_tab_ai_harness_terminal_with_window(window, cx)")
                && section.contains("cx.stop_propagation()"),
            "{name} must intercept Escape for embedded Agent Chat only when no Agent Chat/actions popup owns Escape"
        );
        assert!(
            !section.contains("close_and_reset_window(cx)"),
            "{name} Escape from embedded Agent Chat must return to origin, not hide the main window"
        );
    }
}

#[test]
fn embedded_agent_chat_cmd_w_closes_lifecycle_before_hiding_main() {
    let startup_intent = section_between(
        STARTUP_SOURCE,
        "fn main_window_actions_key_intent",
        "fn handle_main_window_global_key_intent",
    );
    assert!(
        startup_intent.contains("CloseEmbeddedAgentChatWindow")
            && startup_intent.contains("AppView::AgentChatView"),
        "startup.rs must classify Cmd+W on embedded Agent Chat as a named key intent"
    );

    let startup_handler = section_between(
        STARTUP_SOURCE,
        "fn handle_main_window_actions_key_intent",
        "pub(crate) fn new(",
    );
    assert!(
        startup_handler.contains("MainWindowActionsKeyIntent::CloseEmbeddedAgentChatWindow")
            && startup_handler.contains("embedded_agent_chat_cmd_w_close_window"),
        "startup.rs must handle embedded Agent Chat Cmd+W through the named actions-key intent"
    );
    assert_ordered(
        startup_handler,
        "close_tab_ai_harness_terminal_with_window(window, cx)",
        "close_and_reset_window(cx)",
    );

    let startup_new_actions_section = section_between(
        STARTUP_NEW_ACTIONS_SOURCE,
        "// Handle Cmd+W for AgentChatView",
        "let agent_chat_escape_cancelled_streaming",
    );
    assert!(
        startup_new_actions_section.contains("AppView::AgentChatView")
            && startup_new_actions_section.contains("embedded_agent_chat_cmd_w_close_window"),
        "startup_new_actions.rs must identify embedded Agent Chat Cmd+W distinctly"
    );
    assert_ordered(
        startup_new_actions_section,
        "close_tab_ai_harness_terminal_with_window(window, cx)",
        "close_and_reset_window(cx)",
    );
}

#[test]
fn focused_agent_chat_view_handles_escape_and_cmd_w_without_root_bubbling() {
    let escape_section = section_between(
        AGENT_CHAT_VIEW_SOURCE,
        "Escape with no open dialogs unwinds focused-text mini state",
        "Enter submits.",
    );
    assert!(
        escape_section.contains("embedded_agent_chat_escape_host_close_requested")
            && escape_section.contains("self.trigger_close_requested(window, cx)")
            && escape_section.contains("cx.stop_propagation()")
            && !escape_section.contains("cx.propagate()"),
        "focused embedded Agent Chat Escape must close through the host callback instead of relying on root propagation"
    );

    let cmd_w_section = section_between(
        AGENT_CHAT_VIEW_SOURCE,
        "if modifiers.platform && key.eq_ignore_ascii_case(\"w\")",
        "Cmd+. / Cmd+Shift+O",
    );
    assert!(
        cmd_w_section.contains("self.trigger_close_window_requested(window, cx)")
            && cmd_w_section.contains("cx.stop_propagation()"),
        "focused embedded Agent Chat Cmd+W must call the host window-close callback directly"
    );
}

#[test]
fn embedded_agent_chat_host_wires_close_window_shortcut_callback() {
    let section = section_between(
        TAB_AI_MODE_SOURCE,
        "fn wire_embedded_agent_chat_footer_callbacks",
        "let history_app = app_entity.clone();",
    );
    assert!(
        section.contains("set_on_close_requested")
            && section.contains("set_on_close_window_requested")
            && section.contains("if app.opened_from_main_menu")
            && section.contains("close_tab_ai_harness_terminal_with_window(window, cx)")
            && section.contains("close_agent_chat_main_window_state_first(cx)"),
        "embedded Agent Chat close must restore a launcher origin, while direct close and Cmd+W use the strict main-window hide path"
    );
    assert!(
        !section.contains("close_and_reset_window(cx)"),
        "footer callbacks must not synchronously combine origin restoration with main-window reset"
    );
}

#[test]
fn main_native_close_routes_agent_chat_through_lifecycle_close() {
    let section = section_between(
        APP_RUN_SETUP_SOURCE,
        "window.on_window_should_close",
        "// Store the entity for external access",
    );
    assert!(
        section.contains("AppView::AgentChatView")
            && section.contains("embedded_agent_chat_native_close_window")
            && section.contains("SurfaceClosedBySystem")
            && section.contains("SurfaceId::Main")
            && section.contains("false"),
        "main native close must hide the singleton main window while syncing orchestrator state"
    );
    assert_ordered(
        section,
        "close_tab_ai_harness_terminal_with_window(window, cx)",
        "close_and_reset_window(cx)",
    );
}

#[test]
fn embedded_agent_chat_close_helper_tears_down_surface_and_registry() {
    let section = section_between(
        TAB_AI_MODE_SOURCE,
        "fn close_tab_ai_harness_terminal_impl",
        "pub(crate) fn close_tab_ai_harness_terminal_with_window",
    );
    let transitions_source = include_str!("../src/app_impl/agent_chat_surface_transitions.rs");
    assert!(
        section.contains("closing_agent_chat")
            && section.contains("prepare_for_host_hide")
            && section.contains("embedded_agent_chat_return_origin_self_guarded")
            && section.contains("self.agent_chat_ready_script_path = None")
            && transitions_source.contains("rekey_main_automation_surface_from_current_view")
            && transitions_source.contains("ensure_embedded_ai_window(false)")
            && transitions_source.contains("AgentChatSurfaceEvent::EmbeddedClosed"),
        "embedded Agent Chat close helper must preserve the view, restore origin, and tear down Agent Chat surface bookkeeping"
    );
    assert_ordered(
        section,
        "prepare_for_host_hide",
        "self.restore_current_view_with_focus(return_view, return_focus_target)",
    );

    let exit_fn_start = transitions_source
        .find("pub(crate) fn exit_embedded_agent_chat_surface")
        .expect("exit fn must exist");
    let exit_section = &transitions_source[exit_fn_start..];
    assert_ordered(
        exit_section,
        "restore_current_view_with_focus",
        "rekey_main_automation_surface_from_current_view",
    );
}

#[test]
fn detached_agent_chat_cmd_w_stays_on_detached_window_path() {
    let section = section_between(
        AGENT_CHAT_VIEW_SOURCE,
        "// Cmd+W in detached window: close the window directly.",
        "this.handle_key_down(event, window, cx)",
    );
    assert!(
        section.contains("is_chat_window(window)") && section.contains("window.remove_window()"),
        "detached Agent Chat Cmd+W must keep using the detached window close path"
    );
}
