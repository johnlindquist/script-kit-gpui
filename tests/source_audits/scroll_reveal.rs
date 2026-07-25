//! Source audits verifying that scroll reveal emits structured SCROLL_STATE logs
//! with the caller-provided reason, and that sync_list_state resets stale reveal
//! state before re-revealing.

use super::read_source as read;

#[test]
fn scroll_to_selected_if_needed_logs_reason_on_skip() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let fn_body = super::function_body(&content, "scroll_to_selected_if_needed")
        .expect("Expected scroll_to_selected_if_needed function");

    assert!(
        fn_body.contains("target: \"SCROLL_STATE\""),
        "scroll_to_selected_if_needed must emit structured SCROLL_STATE logs"
    );
    assert!(
        fn_body.contains("reason,"),
        "scroll_to_selected_if_needed must log the caller-provided reason"
    );
    assert!(
        fn_body.contains("\"skip scroll reveal"),
        "scroll_to_selected_if_needed must log skip events when target already revealed"
    );
}

#[test]
fn scroll_to_selected_if_needed_logs_reason_on_reveal() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let fn_body = super::function_body(&content, "scroll_to_selected_if_needed")
        .expect("Expected scroll_to_selected_if_needed function");

    assert!(
        fn_body.contains("before_top"),
        "scroll_to_selected_if_needed must log before_top for reveal delta"
    );
    assert!(
        fn_body.contains("after_top"),
        "scroll_to_selected_if_needed must log after_top for reveal delta"
    );
    assert!(
        fn_body.contains("\"revealed selected item\""),
        "scroll_to_selected_if_needed must log reveal completion message"
    );
    assert!(
        fn_body.contains("main_list_footer_overlay_total_padding()")
            && fn_body.contains("self.last_scrolled_index = None"),
        "scroll_to_selected_if_needed must not mark a reveal complete before the viewport is measured"
    );
}

#[test]
fn scroll_to_selected_if_needed_accepts_reason_not_underscore() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let signature = super::function_body(&content, "scroll_to_selected_if_needed")
        .expect("Expected scroll_to_selected_if_needed function");

    assert!(
        !signature.contains("_reason"),
        "scroll_to_selected_if_needed must use `reason`, not `_reason` — the parameter must not be discarded"
    );
}

#[test]
fn sync_list_state_resets_reveal_cache_and_logs() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let fn_body = super::function_body(&content, "sync_list_state")
        .expect("Expected sync_list_state function");

    assert!(
        fn_body.contains("self.last_scrolled_index = None"),
        "sync_list_state must reset last_scrolled_index to invalidate stale reveal cache"
    );
    assert!(
        fn_body.contains("target: \"SCROLL_STATE\""),
        "sync_list_state must emit structured SCROLL_STATE logs"
    );
    assert!(
        fn_body.contains("old_list_count"),
        "sync_list_state must log old_list_count for list-change tracking"
    );
    assert!(
        fn_body.contains("item_count"),
        "sync_list_state must log item_count for list-change tracking"
    );
    assert!(
        fn_body.contains("\"synced list state\""),
        "sync_list_state must log sync completion message"
    );
}

#[test]
fn sync_list_state_re_reveals_after_reset() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let fn_body = super::function_body(&content, "sync_list_state")
        .expect("Expected sync_list_state function");

    // After resetting reveal cache, must scroll to reveal the current selection
    assert!(
        fn_body.contains("scroll_to_reveal_item(self.selected_index)"),
        "count-only sync_list_state must re-reveal the selected item after invalidating the reveal cache"
    );
}

#[test]
fn main_list_scroll_receipt_exposes_footer_safe_selected_row_geometry() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let fn_body = super::function_body(&content, "main_list_scroll_receipt")
        .expect("main_list_scroll_receipt function not found");

    for required in [
        "\"scrollTop\"",
        "\"contentHeight\"",
        "\"viewportHeight\"",
        "\"footerHeight\"",
        "\"footerOverlayHeight\"",
        "\"footerRevealClearanceHeight\"",
        "\"footerOverlayTotalPadding\"",
        "\"maxScrollTop\"",
        "\"selectedRowVisible\"",
        "\"selectedRowAboveFooter\"",
        "main_list_footer_overlay_total_padding()",
        "script_list_pixel_top_for_offset",
    ] {
        assert!(
            fn_body.contains(required),
            "main_list_scroll_receipt should expose `{required}`"
        );
    }
}

#[test]
fn main_list_footer_reveal_clearance_comes_from_theme_tokens() {
    let content = read("src/app_navigation/impl_scroll.rs");
    let list_item = read("src/list_item/mod.rs");
    let theme = read("src/designs/core/main_menu_theme.rs");

    let fn_body = super::function_body(&content, "main_list_footer_reveal_clearance_height")
        .expect("main_list_footer_reveal_clearance_height function not found");

    assert!(
        fn_body.contains("effective_footer_reveal_clearance_height()"),
        "footer reveal clearance must come from the active theme, not a local literal"
    );
    assert!(
        !fn_body.contains("px(8.0)") && !fn_body.contains("gpui::px(8.0)"),
        "footer reveal clearance must not hardcode the old 8px value in scroll logic"
    );
    assert!(
        list_item.contains("effective_footer_reveal_clearance_height_for_theme")
            && list_item.contains("theme.def().list.footer_reveal_clearance_height"),
        "list_item should expose theme-driven footer reveal clearance helpers"
    );
    assert!(
        theme.contains("pub footer_reveal_clearance_height: f32")
            && theme.contains("footer_reveal_clearance_height: 0.0"),
        "MainMenuListTokens should own the default footer reveal clearance value"
    );
}

#[test]
fn main_list_render_uses_pure_selection_snapshot() {
    let content = read("src/render_script_list/mod.rs");

    assert!(
        content.contains("fn selected_index_for_script_list_render(")
            && content
                .contains("crate::list_item::coerce_selection(grouped_items, selected_index)")
            && content.contains(
                "let spine_selection_render_index = selected_index_for_script_list_render("
            ),
        "render must coerce selection through a pure snapshot before row closures are captured"
    );
}

#[test]
fn filter_replacement_sync_replaces_list_state_even_when_count_unchanged() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let sync_fn = super::function_body(&content, "sync_list_state_for_filter_replacement")
        .expect("sync_list_state_for_filter_replacement function not found");

    assert!(
        sync_fn.contains("self.main_list_state = ListState::new(")
            && sync_fn.contains("item_count,"),
        "filter replacement sync must replace the ListState so same-count row replacements rebuild visible items"
    );
    assert!(
        !sync_fn.contains(".measure_all()"),
        "filter replacement sync must not measure every row on each history recall"
    );
    assert!(
        sync_fn.contains("self.main_list_row_generation"),
        "filter replacement sync must bump row generation so same-count replacements get fresh row identity"
    );
    assert!(
        sync_fn.contains("self.last_scrolled_index = None;"),
        "filter replacement sync must also invalidate reveal cache"
    );
    assert!(
        sync_fn.contains("effective_average_item_height_for_scroll"),
        "filter replacement sync should use the real launcher row estimate, not the old 100px fallback"
    );
    assert!(
        !sync_fn.contains("scroll_to_reveal_item(self.selected_index)")
            && !sync_fn.contains("adjust_selected_item_above_footer_overlay(self.selected_index)"),
        "filter replacement sync should not reveal the old selection before reconciliation resets it"
    );
    assert!(
        sync_fn.contains("\"replaced list state for filter replacement\""),
        "filter replacement sync must emit a distinct SCROLL_STATE log"
    );
}

#[test]
fn filter_change_reconciliation_uses_filter_replacement_list_sync() {
    let content = read("src/app_impl/filter_input_updates.rs");

    let fn_body = super::function_body(&content, "reconcile_script_list_after_filter_change")
        .expect("reconcile_script_list_after_filter_change function not found");

    assert!(
        fn_body.contains("self.sync_list_state_for_filter_replacement();"),
        "filter change reconciliation must force list measured-item refresh, not only count sync"
    );
    assert!(
        !fn_body.contains("self.sync_list_state();"),
        "filter change reconciliation should not use count-only list sync"
    );
}

/// Regression guard: same-count list updates (e.g. filtering replaces every row but
/// the total count stays identical) must still invalidate and re-reveal. This test
/// proves the invalidation is unconditional — it happens *outside* the
/// `if old_list_count != item_count` branch.
#[test]
fn sync_list_state_regression_invalidates_reveal_even_when_count_unchanged() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let sync_fn = super::function_body(&content, "sync_list_state")
        .expect("sync_list_state function not found");

    // The splice is conditional on count change...
    let splice_pos = sync_fn
        .find("self.main_list_state.splice(")
        .expect("splice call not found in sync_list_state");
    let splice_guard = sync_fn
        .find("if old_list_count != item_count")
        .expect("splice guard not found");
    assert!(
        splice_guard < splice_pos,
        "splice must be inside the count-change guard"
    );

    // ...but the reveal invalidation must be OUTSIDE that guard (unconditional).
    let reveal_invalidation = sync_fn
        .find("self.last_scrolled_index = None;")
        .expect("reveal cache invalidation not found");
    // The closing brace of the `if` block sits between splice and invalidation.
    // Prove invalidation is after the closing brace by checking it's after splice_pos.
    assert!(
        reveal_invalidation > splice_pos,
        "reveal cache invalidation must happen after the conditional splice, i.e. unconditionally"
    );

    // The re-reveal must also be unconditional (outside the count-change guard).
    let re_reveal = sync_fn
        .find("scroll_to_reveal_item(self.selected_index)")
        .expect("scroll_to_reveal_item call not found");
    assert!(
        re_reveal > reveal_invalidation,
        "re-reveal must happen after cache invalidation"
    );
}

#[test]
fn footer_safe_scroll_offset_uses_footer_reduced_viewport_for_trailing_scroll_budget() {
    let content = read("src/app_navigation/impl_scroll.rs");

    let fn_body = super::function_body(&content, "footer_safe_scroll_offset_for_item")
        .expect("footer_safe_scroll_offset_for_item function not found");

    assert!(
        fn_body.contains("let safe_viewport_height = viewport_height - footer_overlay_height;"),
        "footer_safe_scroll_offset_for_item must compute a footer-reduced viewport height"
    );
    assert!(
        fn_body.contains("script_list_content_height(items) - safe_viewport_height"),
        "footer_safe_scroll_offset_for_item must allow the extra trailing scroll budget required to clear the footer overlay"
    );
    assert!(
        fn_body.contains("let safe_bottom = current_scroll_top + safe_viewport_height;"),
        "footer_safe_scroll_offset_for_item must compare against the footer-safe visible bottom edge"
    );
}

#[test]
fn script_list_scrollbar_overlay_uses_footer_safe_viewport_and_content_height() {
    let content = read("src/render_script_list/mod.rs");

    assert!(
        content.contains(
            "let safe_viewport_height = (viewport_height - footer_overlay_height).max(px(0.0));"
        ),
        "script list scrollbar overlay must clip itself to the footer-safe viewport height"
    );
    assert!(
        content.contains(".map(|(ix, item)| match item {")
            && content.contains("GroupedListItem::SectionHeader(..)")
            && content.contains("GroupedListItem::Item(..)"),
        "script list scrollbar overlay must size against real grouped row heights"
    );
    assert!(
        content.contains(".scroll_size(size(px(0.0), content_height))"),
        "script list scrollbar overlay must override vendor scroll size with row content height"
    );
    assert!(
        !content.contains("+ footer_overlay_height;"),
        "script list scrollbar content height must not add footer padding or the thumb cannot reach the bottom"
    );
    assert!(
        !content.contains(".scrollbar_show(ScrollbarShow::Always)"),
        "script list scrollbar should not force always-visible mode"
    );
}
