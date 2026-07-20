// Tests for list_item module
//
// These are in a separate file because list_item.rs hits GPUI's
// macro recursion limit when #[test] attributes are added.

use crate::list_item::{
    format_shortcut_display, should_show_row_shortcut, should_show_search_description,
    should_show_search_shortcut, RowShortcutVisibilityPolicy,
};

#[test]
fn test_format_shortcut_display_plus_delimited() {
    assert_eq!(format_shortcut_display("cmd+shift+k"), "⌘⇧K");
    assert_eq!(format_shortcut_display("ctrl+c"), "⌃C");
    assert_eq!(format_shortcut_display("alt+enter"), "⌥↩");
}

#[test]
fn test_format_shortcut_display_space_delimited() {
    // Script Kit metadata uses space-delimited format like "opt i"
    assert_eq!(format_shortcut_display("opt i"), "⌥I");
    assert_eq!(format_shortcut_display("cmd shift k"), "⌘⇧K");
    assert_eq!(format_shortcut_display("ctrl alt t"), "⌃⌥T");
    assert_eq!(format_shortcut_display("cmd g"), "⌘G");
    assert_eq!(format_shortcut_display("opt space"), "⌥␣");
    assert_eq!(format_shortcut_display("cmd shift m"), "⌘⇧M");
}

#[test]
fn test_format_shortcut_display_already_native() {
    assert_eq!(format_shortcut_display("⌘⇧K"), "⌘⇧K");
    assert_eq!(format_shortcut_display("⌥I"), "⌥I");
}

#[test]
fn test_format_shortcut_display_special_keys() {
    assert_eq!(format_shortcut_display("escape"), "⎋");
    assert_eq!(format_shortcut_display("tab"), "⇥");
    assert_eq!(format_shortcut_display("backspace"), "⌫");
    assert_eq!(format_shortcut_display("up"), "↑");
    assert_eq!(format_shortcut_display("down"), "↓");
}

#[test]
fn test_format_shortcut_display_mixed_format() {
    // Plus and space mixed (e.g., "cmd + shift + k")
    assert_eq!(format_shortcut_display("cmd + shift + k"), "⌘⇧K");
}

#[test]
fn test_should_show_search_shortcut_selected_only() {
    // Dense launcher rows show shortcut chrome only on the selected (focused) row.
    assert!(should_show_search_shortcut(true, true, false));
    assert!(should_show_search_shortcut(true, true, true));
    assert!(should_show_search_shortcut(false, true, false));
    // Non-selected rows hide shortcuts regardless of hover/filter state.
    assert!(!should_show_search_shortcut(true, false, false));
    assert!(!should_show_search_shortcut(true, false, true));
    assert!(!should_show_search_shortcut(false, false, false));
}

#[test]
fn test_row_shortcut_visibility_selected_only_policy() {
    // SelectedOnly: only the focused row shows shortcuts.
    let policy = RowShortcutVisibilityPolicy::SelectedOnly;
    assert!(should_show_row_shortcut(policy, true, false));
    assert!(should_show_row_shortcut(policy, true, true));
    assert!(!should_show_row_shortcut(policy, false, false));
    assert!(!should_show_row_shortcut(policy, false, true));
}

#[test]
fn test_row_shortcut_visibility_all_rows_policy() {
    // AllRows: every row shows shortcuts regardless of selection/hover.
    let policy = RowShortcutVisibilityPolicy::AllRows;
    assert!(should_show_row_shortcut(policy, true, false));
    assert!(should_show_row_shortcut(policy, true, true));
    assert!(should_show_row_shortcut(policy, false, false));
    assert!(should_show_row_shortcut(policy, false, true));
}

#[test]
fn test_should_show_search_description_only_when_selected_hovered_or_description_matches() {
    assert!(!should_show_search_description(
        false, // selected
        false, // hovered
        false  // has_description_match
    ));
    assert!(should_show_search_description(
        true,  // selected
        false, // hovered
        false
    ));
    assert!(should_show_search_description(
        false, // selected
        true,  // hovered
        false
    ));
    assert!(should_show_search_description(
        false, // selected
        false, // hovered
        true   // has_description_match
    ));
}

// =============================================================================
// Accessory slot builder tests
// =============================================================================

/// Verify that ListItem can be constructed with leading and trailing accessory
/// slots via the builder API without panicking.
#[test]
fn test_list_item_accessory_builders_accept_elements() {
    use crate::list_item::{ListItem, ListItemColors};
    use gpui::*;

    let colors = ListItemColors {
        text_primary: 0xFFFFFF,
        text_secondary: 0xCCCCCC,
        text_muted: 0x999999,
        text_dimmed: 0x666666,
        accent_selected: 0xFBBF24,
        accent_selected_subtle: 0xFBBF24,
        background: 0x1E1E1E,
        background_selected: 0x2A2A2A,
        selected_opacity: 0.15,
        hover_opacity: 0.10,
        warning_bg: 0xFF0000,
        text_on_accent: 0x000000,
    };

    // Leading only
    let _item = ListItem::new("With leading", colors)
        .leading_accessory(div().w(px(40.0)).h(px(8.0)));

    // Trailing only
    let _item = ListItem::new("With trailing", colors)
        .trailing_accessory(div().child("Saved"));

    // Both
    let _item = ListItem::new("With both", colors)
        .leading_accessory(div().w(px(40.0)).h(px(8.0)))
        .trailing_accessory(div().child("Saved"));
}

/// Verify that _opt variants accept None without panicking.
#[test]
fn test_list_item_accessory_opt_builders_accept_none() {
    use crate::list_item::{ListItem, ListItemColors};

    let colors = ListItemColors {
        text_primary: 0xFFFFFF,
        text_secondary: 0xCCCCCC,
        text_muted: 0x999999,
        text_dimmed: 0x666666,
        accent_selected: 0xFBBF24,
        accent_selected_subtle: 0xFBBF24,
        background: 0x1E1E1E,
        background_selected: 0x2A2A2A,
        selected_opacity: 0.15,
        hover_opacity: 0.10,
        warning_bg: 0xFF0000,
        text_on_accent: 0x000000,
    };

    let _item = ListItem::new("No accessories", colors)
        .leading_accessory_opt(None)
        .trailing_accessory_opt(None);
}

/// Verify that accessory slots compose with existing builder methods.
#[test]
fn test_list_item_accessory_composes_with_existing_builders() {
    use crate::list_item::{ListItem, ListItemColors};
    use gpui::*;

    let colors = ListItemColors {
        text_primary: 0xFFFFFF,
        text_secondary: 0xCCCCCC,
        text_muted: 0x999999,
        text_dimmed: 0x666666,
        accent_selected: 0xFBBF24,
        accent_selected_subtle: 0xFBBF24,
        background: 0x1E1E1E,
        background_selected: 0x2A2A2A,
        selected_opacity: 0.15,
        hover_opacity: 0.10,
        warning_bg: 0xFF0000,
        text_on_accent: 0x000000,
    };

    // Full chain: icon + description + leading + trailing accessories
    let _item = ListItem::new("Full chain", colors)
        .icon("📄")
        .description("A description")
        .selected(true)
        .leading_accessory(div().w(px(40.0)).h(px(8.0)))
        .trailing_accessory(div().child("Saved"));
}
