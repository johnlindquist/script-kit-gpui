//! Source-audit contract for actions popup passive blur fallback.
//!
//! AppKit activation observation can miss the path where the popup renders
//! inactive after desktop click-away, so render must close through the same
//! guarded lifecycle path when both parent and actions windows are inactive.
//! Main-hosted actions still use main-window focus, but Notes-hosted actions
//! must not treat the main launcher as their parent focus proxy.

const ACTIONS_WINDOW: &str = include_str!("../src/actions/window.rs");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing start marker: {start}"));
    let after_start = &source[start_index..];
    let end_index = after_start
        .find(end)
        .unwrap_or_else(|| panic!("missing end marker after {start}: {end}"));
    &after_start[..end_index]
}

#[test]
fn actions_window_request_close_is_idempotent() {
    let request_close = source_between(
        ACTIONS_WINDOW,
        "fn request_close(",
        "fn ensure_activation_subscription",
    );

    assert!(request_close.contains("if self.close_requested"));
    assert!(request_close.contains("self.close_requested = true;"));
}

#[test]
fn actions_window_render_falls_back_to_autoclose_when_inactive() {
    let render = source_between(
        ACTIONS_WINDOW,
        "impl Render for ActionsWindow",
        "let handle_key =",
    );

    assert!(
        render.contains("actions_parent_window_focused(&self.parent_automation_id)"),
        "render auto-close must check the actual actions parent, not hard-code main focus"
    );
    assert!(
        render.contains("&self.parent_automation_id"),
        "render auto-close must use the stored parent identity captured when the popup opened"
    );
    assert!(
        !render.contains("let main_window_focused = platform::is_main_window_focused();"),
        "render auto-close must not close Notes-owned popups just because main is unfocused"
    );
    assert!(render
        .contains("should_auto_close_actions_window(parent_window_focused, window_is_active)"));
    assert!(render.contains("ACTIONS_WINDOW_LIFECYCLE render_auto_close"));
    assert!(render.contains("self.request_close(window, cx, \"render_focus_lost\", false);"));
}

#[test]
fn actions_window_tracks_parent_identity_for_focus_lifecycle() {
    let actions_window = source_between(
        ACTIONS_WINDOW,
        "pub struct ActionsWindow",
        "impl ActionsWindow",
    );
    assert!(actions_window.contains("parent_automation_id: String"));
    assert!(actions_window.contains("parent_kind: AutomationWindowKind"));

    let parent_focus = source_between(
        ACTIONS_WINDOW,
        "enum ActionsParentFocusState",
        "/// Actions window width",
    );
    assert!(parent_focus.contains("ActionsParentFocusState"));
    assert!(parent_focus.contains("focused_automation_window_id()"));
    assert!(parent_focus.contains("AutomationRegistryFocused"));
    assert!(parent_focus.contains("AutomationWindowKind::Main"));
    assert!(parent_focus.contains("platform::is_main_window_focused()"));
    assert!(parent_focus.contains("AutomationWindowKind::Notes"));
    assert!(parent_focus.contains("platform::is_notes_window_focused()"));

    let request_close = source_between(
        ACTIONS_WINDOW,
        "fn request_close(",
        "fn ensure_activation_subscription",
    );
    assert!(
        request_close.contains("if activate_main_window")
            && request_close.contains("if self.parent_kind == AutomationWindowKind::Main"),
        "actions close may activate main only for main-hosted popups"
    );
}

#[test]
fn deferred_actions_window_close_clears_automation_target() {
    let defer_close = source_between(ACTIONS_WINDOW, "fn defer_close(", "fn request_close(");

    assert!(defer_close.contains("clear_actions_popup_automation_snapshot()"));
    assert!(defer_close.contains("automation_surface_collector::remove_actions_dialog_snapshot"));
    assert!(defer_close.contains("remove_automation_window(\"actions-dialog\")"));
    assert!(defer_close.contains("clear_actions_window_handle(reason)"));
}
