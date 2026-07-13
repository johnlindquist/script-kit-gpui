//! Source-audit contract for passive main menu focus loss.
//!
//! Clicking away from the main ScriptList should hide the window without
//! clearing the user's filter/list state; explicit close/reset paths still reset.

const MAIN_RS: &str = include_str!("../src/main.rs");
const RENDER_IMPL: &str = include_str!("../src/main_sections/render_impl.rs");
const LIFECYCLE_RESET: &str = include_str!("../src/app_impl/lifecycle_reset.rs");
const WINDOW_VISIBILITY: &str = include_str!("../src/main_sections/window_visibility.rs");
const REGISTRIES_STATE: &str = include_str!("../src/app_impl/registries_state.rs");

fn source_block_after<'a>(source: &'a str, needle: &str, len: usize) -> &'a str {
    let start = source.find(needle).unwrap_or_else(|| {
        panic!("expected to find `{needle}`");
    });
    &source[start..source.len().min(start + len)]
}

#[test]
fn focus_loss_from_script_list_uses_preserve_state_hide() {
    let focus_branch = source_block_after(RENDER_IMPL, "self.is_dismissable_view()", 1800);

    assert!(focus_branch.contains("actions_popup_active_or_closing"));
    assert!(
        focus_branch.contains("matches!(self.current_view, AppView::ScriptList)")
            && focus_branch.contains("hide_main_window_preserving_state_for_focus_loss"),
        "ScriptList focus loss must hide with preserved state instead of reset close"
    );
    assert!(
        focus_branch.contains("dismissable non-ScriptList")
            && focus_branch.contains("self.close_and_reset_window(cx);"),
        "non-ScriptList dismissable focus loss must keep the explicit reset close path"
    );
}

#[test]
fn render_focus_loss_treats_deferred_actions_state_as_actions_open() {
    let focus_branch = source_block_after(
        RENDER_IMPL,
        "self.was_window_focused && !is_window_focused",
        5200,
    );

    assert!(focus_branch.contains("self.show_actions_popup"));
    assert!(focus_branch.contains("self.actions_dialog.is_some()"));
    assert!(focus_branch.contains("actions::is_actions_window_open()"));
    assert!(focus_branch.contains("!crate::dev_style_tool::window::is_dev_style_tool_open()"));
    assert!(focus_branch.contains("dev style tool is open - staying open"));
    assert!(focus_branch.contains("!actions_popup_active_or_closing"));
}

#[test]
fn preserve_state_hide_does_not_reset_or_cancel_script_list() {
    let helper = source_block_after(
        LIFECYCLE_RESET,
        "fn hide_main_window_preserving_state_for_focus_loss",
        2600,
    );

    assert!(helper.contains("mark_main_state_restore_after_focus_loss();"));
    assert!(helper.contains("set_main_window_visible(false)"));
    assert!(helper.contains("close_main_footer_popup"));
    assert!(!helper.contains("reset_to_script_list(cx);"));
    assert!(!helper.contains("cancel_script_execution(cx);"));
    assert!(!helper.contains("reset_script_list_filter_state"));
    assert!(!helper.contains("reset_script_list_selection_state"));
}

#[test]
fn next_show_consumes_restore_intent_without_selection_normalization() {
    assert!(MAIN_RS.contains("RESTORE_MAIN_STATE_AFTER_FOCUS_LOSS"));
    assert!(MAIN_RS.contains("fn consume_main_state_restore_after_focus_loss()"));

    let show_path = source_block_after(WINDOW_VISIBILITY, "let restore_after_focus_loss", 1300);

    assert!(show_path.contains("consume_main_state_restore_after_focus_loss()"));
    assert!(show_path
        .contains("restore_after_focus_loss && matches!(view.current_view, AppView::ScriptList)"));
    assert!(show_path.contains("view.focused_input = FocusedInput::MainFilter;"));
    assert!(show_path.contains("view.pending_focus = Some(FocusTarget::MainFilter);"));

    let restore_start = WINDOW_VISIBILITY
        .find("Restoring ScriptList exactly after focus-loss hide")
        .expect("restore-after-focus-loss branch must log its path");
    let ensure_start = WINDOW_VISIBILITY[restore_start..]
        .find("view.ensure_selection_at_first_item(ctx);")
        .map(|index| restore_start + index)
        .expect("normal show path should still normalize selection");
    let else_before_ensure = WINDOW_VISIBILITY[restore_start..ensure_start]
        .rfind("} else {")
        .is_some();
    assert!(
        else_before_ensure,
        "restore-after-focus-loss must not normalize the selected row/list state"
    );
}

#[test]
fn explicit_reset_paths_clear_focus_loss_restore_intent() {
    let close_path = source_block_after(LIFECYCLE_RESET, "fn prepare_main_window_close", 1800);
    assert!(close_path.contains("clear_main_state_restore_after_focus_loss();"));

    let reset_path = source_block_after(REGISTRIES_STATE, "fn reset_to_script_list_impl", 1800);
    assert!(reset_path.contains("clear_main_state_restore_after_focus_loss();"));
}

#[test]
fn close_and_reset_enqueues_native_hide_before_script_list_reset() {
    let preparation = source_block_after(LIFECYCLE_RESET, "fn prepare_main_window_close", 5200);
    let close_path = source_block_after(LIFECYCLE_RESET, "fn close_and_reset_window", 1200);
    let hidden = preparation
        .find("set_main_window_visible(false)")
        .expect("close_and_reset_window must mark the main window hidden");
    let footer_close = preparation
        .find("close_main_footer_popup")
        .expect("close_and_reset_window must close the main footer popup");
    let prepared = close_path
        .find(".prepare_main_window_close(cx, \"close_and_reset_window\", true, true)")
        .expect("close_and_reset_window must run shared preparation");
    let defer_hide = close_path
        .find("defer_hide_main_window(cx);")
        .expect("close_and_reset_window must use the main-panel-only deferred hide");
    let deferred_reset = close_path
        .find("defer_reset_to_script_list_after_main_window_hidden")
        .expect("close_and_reset_window must defer reset until after native hide is enqueued");

    assert!(
        prepared < defer_hide,
        "shared preparation must complete before native hide is enqueued"
    );
    assert!(
        footer_close < preparation.len(),
        "shared preparation must close the footer popup"
    );
    assert!(
        defer_hide < deferred_reset,
        "ScriptList reset must be deferred until after native hide is enqueued"
    );
    assert!(
        !preparation[hidden..].contains("reset_to_script_list(cx);"),
        "close path must not reset to ScriptList while the AppKit window can still be visible"
    );
}
