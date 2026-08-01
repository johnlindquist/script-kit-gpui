//! Tests for list_item module
//!
//! These are in a separate file because list_item.rs hits GPUI's
//! macro recursion limit when #[test] attributes are added.

#[cfg(test)]
mod tests {
    use crate::list_item::{
        coerce_eligibility_selection, first_selectable_eligibility_index, format_shortcut_display,
        last_selectable_eligibility_index, selectable_eligibility_index_at_or_after,
        selectable_eligibility_index_at_or_before, should_show_row_shortcut,
        should_show_search_description, should_show_search_shortcut, RowEligibility,
        RowShortcutVisibilityPolicy, LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED,
    };

    #[test]
    fn row_eligibility_navigation_skips_inert_rows_and_keeps_disabled_explanations() {
        let rows = [
            RowEligibility::inert(),
            RowEligibility::enabled_action(),
            RowEligibility::inert(),
            RowEligibility::disabled_explanation(),
        ];

        assert_eq!(first_selectable_eligibility_index(&rows), Some(1));
        assert_eq!(last_selectable_eligibility_index(&rows), Some(3));
        assert_eq!(selectable_eligibility_index_at_or_before(&rows, 2), Some(1));
        assert_eq!(selectable_eligibility_index_at_or_after(&rows, 2), Some(3));
        assert_eq!(coerce_eligibility_selection(&rows, 0), Some(1));
        assert_eq!(coerce_eligibility_selection(&rows, 2), Some(3));
        assert_eq!(coerce_eligibility_selection(&rows, 99), Some(3));
        assert!(!rows[3].activatable);
    }

    #[test]
    fn row_eligibility_navigation_returns_none_without_selectable_rows() {
        let rows = [RowEligibility::inert(), RowEligibility::inert()];
        assert_eq!(first_selectable_eligibility_index(&rows), None);
        assert_eq!(last_selectable_eligibility_index(&rows), None);
        assert_eq!(coerce_eligibility_selection(&rows, 0), None);
    }

    #[test]
    #[should_panic(expected = "activatable rows must be selectable")]
    fn row_eligibility_rejects_activatable_non_selectable_rows() {
        let _ = RowEligibility::new(true, false, true);
    }

    #[test]
    #[should_panic(expected = "selectable rows must be focusable")]
    fn row_eligibility_rejects_selectable_non_focusable_rows() {
        let _ = RowEligibility::new(false, true, false);
    }

    #[test]
    fn legacy_rows_share_the_disabled_hover_tooltip_policy() {
        assert!(!LIST_ITEM_MOUSE_HOVER_TOOLTIPS_ENABLED);
    }

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
        let policy = RowShortcutVisibilityPolicy::SelectedOnly;
        assert!(should_show_row_shortcut(policy, true, false));
        assert!(should_show_row_shortcut(policy, true, true));
        assert!(!should_show_row_shortcut(policy, false, false));
        assert!(!should_show_row_shortcut(policy, false, true));
    }

    #[test]
    fn test_row_shortcut_visibility_all_rows_policy() {
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
}
