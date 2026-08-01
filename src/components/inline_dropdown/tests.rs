use super::component::{
    inline_dropdown_clamp_selected_index, inline_dropdown_select_next, inline_dropdown_select_prev,
    inline_dropdown_visible_range, inline_dropdown_visible_range_from_start,
};
use super::row::{
    picker_row_state_palette, CONTEXT_SELECTOR_ROW_HEIGHT, SELECTED_ROW_OPACITY,
    SOFT_COMPACT_PICKER_ROW_HEIGHT, SOFT_COMPACT_SELECTED_ROW_OPACITY,
};

#[test]
fn inline_dropdown_navigation_wraps() {
    assert_eq!(inline_dropdown_select_prev(0, 4), 3);
    assert_eq!(inline_dropdown_select_next(3, 4), 0);
}

#[test]
fn inline_dropdown_navigation_empty_list() {
    assert_eq!(inline_dropdown_select_prev(0, 0), 0);
    assert_eq!(inline_dropdown_select_next(0, 0), 0);
}

#[test]
fn inline_dropdown_navigation_single_item() {
    assert_eq!(inline_dropdown_select_prev(0, 1), 0);
    assert_eq!(inline_dropdown_select_next(0, 1), 0);
}

#[test]
fn inline_dropdown_clamps_after_filter_shrink() {
    assert_eq!(inline_dropdown_clamp_selected_index(5, 0), 0);
    assert_eq!(inline_dropdown_clamp_selected_index(5, 2), 1);
    assert_eq!(inline_dropdown_clamp_selected_index(0, 3), 0);
    assert_eq!(inline_dropdown_clamp_selected_index(2, 3), 2);
}

#[test]
fn inline_dropdown_rows_match_launcher_row_height() {
    assert_eq!(
        CONTEXT_SELECTOR_ROW_HEIGHT,
        crate::list_item::LIST_ITEM_HEIGHT
    );
}

#[test]
fn soft_compact_picker_row_is_exactly_36px_and_tighter_than_launcher_rows() {
    assert_eq!(SOFT_COMPACT_PICKER_ROW_HEIGHT, 36.0);
    assert!(SOFT_COMPACT_PICKER_ROW_HEIGHT < CONTEXT_SELECTOR_ROW_HEIGHT);
}

#[test]
fn dense_and_soft_compact_row_palettes_preserve_legacy_state_bytes() {
    let foreground: gpui::Hsla = gpui::rgb(0x112233).into();
    let muted: gpui::Hsla = gpui::rgb(0x445566).into();
    let dense = picker_row_state_palette(foreground, muted, SELECTED_ROW_OPACITY);
    let soft = picker_row_state_palette(foreground, muted, SOFT_COMPACT_SELECTED_ROW_OPACITY);

    assert_eq!(dense.rest.background_rgba, None);
    assert_eq!(dense.hovered.background_rgba, Some(0x1122330F));
    assert_eq!(dense.selected.background_rgba, Some(0x1122333A));
    assert_eq!(dense.active, dense.selected);
    assert_eq!(dense.rest.primary_foreground_rgba, 0x112233A5);
    assert_eq!(dense.selected.primary_foreground_rgba, 0x112233FF);

    assert_eq!(soft.rest.background_rgba, None);
    assert_eq!(soft.hovered.background_rgba, Some(0x1122330F));
    assert_eq!(soft.selected.background_rgba, Some(0x1122332D));
    assert_eq!(soft.active, soft.selected);
}

#[test]
fn compact_row_disabled_hover_uses_disabled_state() {
    let foreground: gpui::Hsla = gpui::rgb(0x112233).into();
    let palette = picker_row_state_palette(foreground, foreground, SELECTED_ROW_OPACITY);
    assert_eq!(
        palette.for_flags(crate::theme::RowStateFlags {
            selected: false,
            hovered: true,
            active: false,
            disabled: true,
        }),
        palette.disabled
    );
}

#[test]
fn soft_compact_meta_badge_uses_menu_tint_instead_of_dark_badge_surface() {
    let row_source = include_str!("row.rs");
    let badge_start = row_source
        .find("fn render_soft_compact_meta_badge")
        .expect("soft compact meta badge renderer should exist");
    let badge_source = &row_source[badge_start..];

    assert!(
        badge_source.contains(".bg(colors.foreground.opacity(GHOST))"),
        "soft compact metadata badges should use the same low-opacity foreground tint as launcher menu badges"
    );
    assert!(
        !badge_source.contains("chrome.badge_bg_rgba"),
        "soft compact metadata badges must not use the darker global badge surface"
    );
}

#[test]
fn compact_synopsis_uses_footer_surface_tokens_and_grows_for_description() {
    let row_source = include_str!("row.rs");
    let synopsis_start = row_source
        .find("pub(crate) fn render_compact_synopsis_strip")
        .expect("compact synopsis renderer should exist");
    let synopsis_source = &row_source[synopsis_start..];

    assert!(
        synopsis_source.contains("PromptFooterColors::from_theme")
            && synopsis_source.contains("prompt_footer::footer_surface_rgba")
            && synopsis_source.contains("HINT_STRIP_PADDING_X"),
        "focused item synopsis must share the main/dictation footer surface and spacing tokens"
    );
    assert!(
        synopsis_source.contains(".min_h(px(CONTEXT_SELECTOR_SYNOPSIS_HEIGHT))")
            && !synopsis_source.contains(".h(px(description_height))"),
        "synopsis strip must size intrinsically from measured text (min-height floor only), \
         not from a chars-per-line height estimate"
    );
    assert!(
        synopsis_source.contains("line_clamp(CONTEXT_SELECTOR_SYNOPSIS_MAX_LINES)"),
        "synopsis description must stay capped by line_clamp so intrinsic growth is bounded"
    );
}

#[test]
fn inline_dropdown_visible_range_small_list() {
    // List smaller than max_visible — show everything.
    assert_eq!(inline_dropdown_visible_range(0, 3, 8), 0..3);
}

#[test]
fn inline_dropdown_visible_range_holds_first_page_until_selection_leaves_it() {
    assert_eq!(inline_dropdown_visible_range(6, 20, 8), 0..8);
    assert_eq!(inline_dropdown_visible_range(7, 20, 8), 0..8);
    assert_eq!(inline_dropdown_visible_range(8, 20, 8), 1..9);
}

#[test]
fn inline_dropdown_visible_range_from_start_waits_for_top_and_bottom_edges() {
    assert_eq!(inline_dropdown_visible_range_from_start(8, 8, 20, 8), 8..16);
    assert_eq!(
        inline_dropdown_visible_range_from_start(8, 15, 20, 8),
        8..16
    );
    assert_eq!(inline_dropdown_visible_range_from_start(8, 7, 20, 8), 7..15);
    assert_eq!(
        inline_dropdown_visible_range_from_start(8, 16, 20, 8),
        9..17
    );
}

#[test]
fn inline_dropdown_visible_range_clamps_to_end() {
    assert_eq!(inline_dropdown_visible_range(19, 20, 8), 12..20);
}

#[test]
fn inline_dropdown_visible_range_clamps_to_start() {
    assert_eq!(inline_dropdown_visible_range(0, 20, 8), 0..8);
}

#[test]
fn inline_dropdown_visible_range_empty_list() {
    assert_eq!(inline_dropdown_visible_range(0, 0, 8), 0..0);
}
