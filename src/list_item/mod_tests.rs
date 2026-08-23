#[cfg(test)]
mod grouped_list_state_tests {
    use super::{GroupedListItem, GroupedListState, SourceChipStatusKind, SourceChipStatusRow};

    fn loading_status() -> GroupedListItem {
        GroupedListItem::Status(SourceChipStatusRow {
            source: crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory,
            source_name: "Browser History".to_owned(),
            status_kind: SourceChipStatusKind::Loading,
            label: "Loading browser history".to_owned(),
            shown: 0,
            loaded: 0,
            total: None,
        })
    }

    #[test]
    fn first_selectable_skips_all_inert_prefix_rows() {
        let rows = vec![
            GroupedListItem::ReservedSectionSlot,
            GroupedListItem::SectionHeader("Loading".to_owned(), None),
            loading_status(),
            GroupedListItem::SectionHeader("Commands".to_owned(), None),
            GroupedListItem::Item(0),
        ];
        let state = GroupedListState::from_items(&rows);

        assert_eq!(state.first_selectable, 4);
        for index in 0..4 {
            assert!(state.is_header(index), "row {index} is inert");
        }
        assert!(!state.is_header(4));
    }

    #[test]
    fn navigation_skips_interleaved_status_and_headers() {
        let rows = vec![
            GroupedListItem::SectionHeader("First".to_owned(), None),
            GroupedListItem::Item(0),
            loading_status(),
            GroupedListItem::SectionHeader("Second".to_owned(), None),
            GroupedListItem::ReservedSectionSlot,
            GroupedListItem::Item(1),
            loading_status(),
            GroupedListItem::Item(2),
        ];
        let state = GroupedListState::from_items(&rows);

        assert_eq!(state.first_selectable, 1);
        assert_eq!(state.next_selectable(1), Some(5));
        assert_eq!(state.next_selectable(5), Some(7));
        assert_eq!(state.next_selectable(7), None);
        assert_eq!(state.prev_selectable(7), Some(5));
        assert_eq!(state.prev_selectable(5), Some(1));
        assert_eq!(state.prev_selectable(1), None);
    }

    #[test]
    fn empty_and_inert_only_lists_preserve_zero_fallback() {
        let empty = GroupedListState::from_items(&[]);
        assert_eq!(empty.first_selectable, 0);
        assert_eq!(empty.next_selectable(0), None);

        let rows = vec![
            GroupedListItem::ReservedSectionSlot,
            GroupedListItem::SectionHeader("Loading".to_owned(), None),
            loading_status(),
        ];
        let inert = GroupedListState::from_items(&rows);
        assert_eq!(inert.first_selectable, 0);
        assert_eq!(inert.next_selectable(0), None);
        assert_eq!(inert.prev_selectable(2), None);

        let grouped = GroupedListState::from_groups(&[("empty", 0), ("ready", 2)]);
        assert_eq!(grouped.first_selectable, 1);
    }
}

#[cfg(test)]
mod icon_kind_tests {
    use super::IconKind;

    #[test]
    fn test_icon_kind_from_icon_hint_returns_svg_when_known_icon_name() {
        match IconKind::from_icon_hint("terminal") {
            Some(IconKind::Svg(name)) => assert_eq!(name, "terminal"),
            _ => panic!("expected SVG icon from known icon hint"),
        }
    }

    #[test]
    fn test_icon_kind_from_icon_hint_returns_emoji_when_symbol_glyph() {
        match IconKind::from_icon_hint("📄") {
            Some(IconKind::Emoji(emoji)) => assert_eq!(emoji, "📄"),
            _ => panic!("expected emoji icon for symbol glyph"),
        }
    }

    #[test]
    fn test_icon_kind_from_icon_hint_returns_none_for_unknown_ascii_word() {
        assert!(IconKind::from_icon_hint("unknown-icon-name").is_none());
    }
}

#[cfg(test)]
mod list_item_colors_tests {
    use super::{ListItemColors, ListItemMetricsOverride, ALPHA_DIVIDER};

    #[test]
    fn test_from_theme_sets_text_on_accent_from_theme_text_on_accent() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.text.primary = 0x010203;
        theme.colors.text.on_accent = 0xa1b2c3;

        let colors = ListItemColors::from_theme(&theme);

        assert_eq!(colors.text_on_accent, theme.colors.text.on_accent);
        assert_ne!(colors.text_on_accent, theme.colors.text.primary);
    }

    #[test]
    fn test_from_design_with_dark_mode_uses_theme_row_opacity_ladders() {
        let design = crate::designs::DesignColors::default();
        let dark = ListItemColors::from_design_with_dark_mode(&design, true);
        let light = ListItemColors::from_design_with_dark_mode(&design, false);
        let dark_opacity = crate::theme::types::BackgroundOpacity::dark_default();
        let light_opacity = crate::theme::types::BackgroundOpacity::light_default();

        assert_eq!(dark.selected_opacity, dark_opacity.selected);
        assert_eq!(dark.hover_opacity, dark_opacity.hover);
        assert_eq!(light.selected_opacity, light_opacity.selected);
        assert_eq!(light.hover_opacity, light_opacity.hover);
    }

    #[test]
    fn test_alpha_divider_matches_ui_foundation_constant() {
        assert_eq!(ALPHA_DIVIDER, crate::ui_foundation::ALPHA_DIVIDER as u32);
    }

    #[test]
    fn list_item_render_inputs_resolve_through_canonical_state_palette() {
        let theme = crate::theme::Theme::dark_default();
        let mut colors = ListItemColors::from_theme(&theme);
        colors.text_primary = 0x102030;
        colors.accent_selected = 0x405060;
        colors.text_on_accent = 0xF0E0D0;
        colors.alpha_name = 0xA1;
        colors.hover_opacity = 0.22;
        let mut metrics = ListItemMetricsOverride::from_main_menu_theme(
            crate::designs::MainMenuThemeVariant::InfoBarBase,
        );
        metrics.row_hover_fill_alpha = 0x12;
        metrics.row_selected_fill_alpha = 0x20;

        let palette = crate::theme::resolve_main_menu_row_state_palette_from_parts(
            crate::theme::MainMenuRowColorInputs {
                row_kind: crate::designs::MainMenuRowKind::IconTile,
                row_hover_fill_alpha: metrics.row_hover_fill_alpha as u8,
                row_selected_fill_alpha: metrics.row_selected_fill_alpha as u8,
                theme_hover_opacity: colors.hover_opacity,
                text_primary_hex: colors.text_primary,
                accent_selected_hex: colors.accent_selected,
                text_on_accent_hex: colors.text_on_accent,
                primary_name_alpha: colors.alpha_name as u8,
            },
        );

        assert_eq!(palette.rest.primary_foreground_rgba, 0x102030A1);
        assert_eq!(palette.hover.background_rgba, Some(0x10203038));
        assert_eq!(palette.active.background_rgba, Some(0x10203020));
        assert_eq!(palette.active.primary_foreground_rgba, 0x102030FF);
    }
}

#[cfg(test)]
mod selection_marker_tests {
    use super::{list_item_selection_marker_geometry, ListItemMetricsOverride};

    #[test]
    fn approved_marker_is_an_absolute_overlay_inside_the_row_surface() {
        let metrics = ListItemMetricsOverride::from_main_menu_theme(
            crate::designs::MainMenuThemeVariant::InfoBarBase,
        );
        let marker = list_item_selection_marker_geometry(metrics, true)
            .expect("selected launcher rows paint a marker");

        assert_eq!(marker.left, metrics.row_outer_padding_x + 6.0);
        assert_eq!(
            marker.top,
            (metrics.row_selected_marker_center_height - 16.0) / 2.0
        );
        assert_eq!(marker.width, 2.0);
        assert_eq!(marker.height, 16.0);
        assert_eq!(marker.radius, 1.0);
        assert_eq!(marker.alpha, 0xFF);
        assert!(marker.left >= metrics.row_outer_padding_x);
        assert!(marker.top >= metrics.row_outer_padding_y);
        assert!(marker.top + marker.height <= metrics.item_height - metrics.row_outer_padding_y);
    }

    #[test]
    fn marker_visibility_depends_only_on_selected_location() {
        let metrics = ListItemMetricsOverride::default_main_menu();
        assert!(list_item_selection_marker_geometry(metrics, true).is_some());
        assert!(list_item_selection_marker_geometry(metrics, false).is_none());
    }

    #[test]
    fn marker_tokens_do_not_change_row_content_geometry() {
        let mut changed_marker = ListItemMetricsOverride::from_main_menu_theme(
            crate::designs::MainMenuThemeVariant::InfoBarBase,
        );
        let baseline = changed_marker;
        changed_marker.row_selected_marker_width = 3.0;
        changed_marker.row_selected_marker_height = 18.0;
        changed_marker.row_selected_marker_inset_x = 7.0;

        assert_eq!(changed_marker.item_height, baseline.item_height);
        assert_eq!(
            changed_marker.row_outer_padding_x,
            baseline.row_outer_padding_x
        );
        assert_eq!(
            changed_marker.row_outer_padding_y,
            baseline.row_outer_padding_y
        );
        assert_eq!(
            changed_marker.row_inner_padding_x,
            baseline.row_inner_padding_x
        );
        assert_eq!(
            changed_marker.row_inner_padding_y,
            baseline.row_inner_padding_y
        );
        assert_eq!(
            changed_marker.icon_container_size,
            baseline.icon_container_size
        );
        assert_eq!(changed_marker.icon_text_gap, baseline.icon_text_gap);
        assert_eq!(changed_marker.accessory_gap, baseline.accessory_gap);
    }
}

#[cfg(test)]
mod row_chrome_rgba_tests {
    use super::{row_type_accessory_rgba, ListItemColors};

    #[test]
    fn test_row_type_accessory_rgba_uses_theme_icon_and_strong_alphas() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.opacity = Some(crate::theme::types::BackgroundOpacity {
            text_icon: 0.42,
            text_strong: 0.88,
            ..theme.get_opacity()
        });
        let colors = ListItemColors::from_theme(&theme);

        let idle = row_type_accessory_rgba(&colors, false);
        let selected = row_type_accessory_rgba(&colors, true);

        assert_eq!(idle & 0xFF, colors.alpha_icon as u32);
        assert_eq!(selected & 0xFF, colors.alpha_strong as u32);
        assert_eq!(idle >> 8, colors.accent_selected);
    }
}

#[cfg(test)]
mod row_shortcut_policy_tests {
    use super::{
        should_show_row_shortcut, should_show_search_shortcut, RowShortcutVisibilityPolicy,
    };

    #[test]
    fn selected_only_shows_shortcut_on_focused_row() {
        let p = RowShortcutVisibilityPolicy::SelectedOnly;
        assert!(should_show_row_shortcut(p, true, false));
        assert!(should_show_row_shortcut(p, true, true));
    }

    #[test]
    fn selected_only_hides_shortcut_on_unfocused_row() {
        let p = RowShortcutVisibilityPolicy::SelectedOnly;
        assert!(!should_show_row_shortcut(p, false, false));
        assert!(!should_show_row_shortcut(p, false, true));
    }

    #[test]
    fn all_rows_always_shows_shortcut() {
        let p = RowShortcutVisibilityPolicy::AllRows;
        assert!(should_show_row_shortcut(p, true, false));
        assert!(should_show_row_shortcut(p, true, true));
        assert!(should_show_row_shortcut(p, false, false));
        assert!(should_show_row_shortcut(p, false, true));
    }

    #[test]
    fn search_shortcut_delegates_to_selected_only() {
        // Dense launcher rows use SelectedOnly — only selected rows show shortcuts.
        assert!(should_show_search_shortcut(true, true, false));
        assert!(!should_show_search_shortcut(true, false, false));
        assert!(!should_show_search_shortcut(false, false, false));
    }
}

#[cfg(test)]
mod render_section_header_source_tests {
    use super::{
        ensure_launcher_section_slot, grouped_list_item_eligibility,
        resolve_section_header_presentation, GroupedListItem, SectionPresentationFamily,
        SectionTextTier,
    };

    const SOURCE: &str = include_str!("mod.rs");

    fn render_section_header_source() -> String {
        let start = SOURCE
            .find("pub fn render_section_header(")
            .expect("render_section_header should exist");
        let rest = &SOURCE[start..];
        let end = rest
            .find("// Note: GPUI rendering tests omitted")
            .expect("sentinel comment should exist after render_section_header");
        rest[..end].to_string()
    }

    #[test]
    fn section_header_presentation_preserves_semantics_and_transforms_display_only() {
        let authored = "Straße · résumé";
        let launcher = resolve_section_header_presentation(
            authored,
            Some("star"),
            Some("5"),
            SectionPresentationFamily::Launcher,
        );
        assert_eq!(launcher.semantic_label.as_ref(), authored);
        assert_eq!(launcher.display_label.as_ref(), "STRASSE · RÉSUMÉ");
        assert_eq!(launcher.label_tier, SectionTextTier::Strong);
        assert_eq!(launcher.count_tier, SectionTextTier::Muted);
        assert_eq!(launcher.icon_tier, SectionTextTier::Muted);

        let preserved = resolve_section_header_presentation(
            authored,
            None,
            None,
            SectionPresentationFamily::PreserveAuthored,
        );
        assert_eq!(preserved.semantic_label.as_ref(), authored);
        assert_eq!(preserved.display_label.as_ref(), authored);
        assert_eq!(preserved.label_tier, SectionTextTier::Muted);
    }

    #[test]
    fn launcher_section_slot_is_stable_inert_and_non_semantic() {
        let mut ungrouped = vec![GroupedListItem::Item(0)];
        ensure_launcher_section_slot(&mut ungrouped);
        assert!(matches!(
            ungrouped.as_slice(),
            [
                GroupedListItem::ReservedSectionSlot,
                GroupedListItem::Item(0)
            ]
        ));
        let reserved = grouped_list_item_eligibility(&ungrouped[0]);
        assert!(!reserved.focusable && !reserved.selectable && !reserved.activatable);

        let mut grouped = vec![
            GroupedListItem::SectionHeader("Recent".into(), None),
            GroupedListItem::Item(0),
        ];
        ensure_launcher_section_slot(&mut grouped);
        assert_eq!(grouped.len(), 2, "a real first header must keep the slot");

        let mut empty = Vec::new();
        ensure_launcher_section_slot(&mut empty);
        assert!(
            empty.is_empty(),
            "zero results must not expose an empty slot"
        );
    }

    #[test]
    fn section_header_geometry_comes_from_metrics() {
        let body = render_section_header_source();
        for required in [
            "metrics.section_padding_x",
            "metrics.section_padding_bottom",
            "metrics.section_gap",
            "metrics.section_icon_size",
            "metrics.section_weight",
        ] {
            assert!(
                body.contains(required),
                "render_section_header should use {required}"
            );
        }
        for forbidden in [
            "SECTION_PADDING_X",
            "SECTION_PADDING_BOTTOM",
            "SECTION_GAP",
            "SECTION_HEADER_ICON_SIZE",
            "FontWeight::SEMIBOLD",
        ] {
            assert!(
                !body.contains(forbidden),
                "render_section_header should not use local {forbidden}"
            );
        }
    }

    #[test]
    fn section_headers_do_not_render_separator_lines() {
        let body = render_section_header_source();
        assert!(
            !body.contains("border_t_1"),
            "section headers should rely on spacing, not separator lines"
        );
    }

    #[test]
    fn section_header_docs_do_not_reference_removed_top_border_behavior() {
        let body = render_section_header_source();
        assert!(
            !body.contains("suppresses top border"),
            "render_section_header docs should not describe removed separator behavior"
        );
    }
}
