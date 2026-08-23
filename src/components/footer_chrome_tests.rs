#[cfg(test)]
mod launcher_primary_footer_button_tests {
    use super::launcher_primary_footer_button;
    use crate::footer_popup::FooterAction;

    #[test]
    fn unavailable_command_disables_footer_activation_and_keyboard_routing() {
        let reason = "Resolve the permission request first.";
        let button = launcher_primary_footer_button("Run".to_string(), false, Some(reason));

        assert_eq!(button.action, FooterAction::Run);
        assert!(!button.enabled);
        assert!(!button.shortcut_routable);
        assert_eq!(
            button.disabled_reason.as_ref().map(ToString::to_string),
            Some(reason.to_string())
        );
    }

    #[test]
    fn ready_command_enables_footer_activation_and_keyboard_routing() {
        let button = launcher_primary_footer_button("Run".to_string(), false, None);

        assert_eq!(button.action, FooterAction::Run);
        assert!(button.enabled);
        assert!(button.shortcut_routable);
        assert_eq!(button.disabled_reason.as_deref(), None);
    }

    #[test]
    fn global_confirmation_lock_takes_precedence_over_command_specific_reason() {
        let button = launcher_primary_footer_button(
            "Run".to_string(),
            true,
            Some("This reason cannot override the confirmation lock."),
        );

        assert!(!button.enabled);
        assert!(!button.shortcut_routable);
        assert_eq!(button.disabled_reason.as_deref(), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .expect("function signature should exist");
        let source = &source[start..];
        let open = source.find('{').expect("function body should open");
        let mut depth = 0usize;
        for (offset, byte) in source.as_bytes()[open..].iter().copied().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open + 1..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("function body should close")
    }

    #[test]
    fn config_rail_maps_every_footer_action_to_shared_chrome_slots() {
        use crate::footer_popup::FooterAction;

        for (action, expected) in [
            (FooterAction::Run, FooterActionSlot::Run),
            (FooterAction::Actions, FooterActionSlot::Actions),
            (FooterAction::Ai, FooterActionSlot::Ai),
            (FooterAction::Apply, FooterActionSlot::Apply),
            (FooterAction::Replace, FooterActionSlot::Replace),
            (FooterAction::Append, FooterActionSlot::Append),
            (FooterAction::Copy, FooterActionSlot::Copy),
            (FooterAction::Expand, FooterActionSlot::Expand),
            (FooterAction::Retry, FooterActionSlot::Retry),
            (FooterAction::Close, FooterActionSlot::Close),
            (FooterAction::Stop, FooterActionSlot::Stop),
            (FooterAction::PasteResponse, FooterActionSlot::PasteResponse),
            (FooterAction::Cwd, FooterActionSlot::Ai),
            (FooterAction::AgentModel, FooterActionSlot::Ai),
            (FooterAction::Tips, FooterActionSlot::Ai),
        ] {
            assert_eq!(footer_config_action_slot(action), expected);
        }
    }

    #[test]
    fn config_rail_preserves_explicit_and_contextual_left_pinning() {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        assert!(footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::Cwd, "⇥", "Project")
        ));
        assert!(footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::AgentModel, "⇧⇥", "Agent · Model")
        ));
        assert!(footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::Close, "Esc", "Terminate").left_pinned()
        ));
        assert!(!footer_config_button_is_left_pinned(
            &FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
        ));
    }

    /// Optical keycap corrections (user report 2026-07-11): the ⌘ ink sat
    /// low-left of chip center, and word keycaps ("Space") rendered with zero
    /// horizontal padding and the single-glyph down-nudge crowding the
    /// descender against the bottom border. Lock the per-token resolution so
    /// the corrections can't silently regress to the uniform values.
    #[test]
    fn keycap_glyph_nudges_and_padding_resolve_per_token() {
        let metrics = crate::designs::MainMenuThemeVariant::InfoBarBase
            .base_def()
            .footer
            .metrics;

        // ⌘ gets its own optical x/y correction.
        assert_eq!(footer_key_glyph_nudge_x("⌘"), metrics.cmd_glyph_nudge_x);
        assert_eq!(footer_key_glyph_nudge_y("⌘"), metrics.cmd_glyph_nudge_y);
        assert!(metrics.cmd_glyph_nudge_x > 0.0, "⌘ ink sits left of center");
        assert!(
            metrics.cmd_glyph_nudge_y < metrics.key_glyph_nudge_y,
            "⌘ must not inherit the full single-glyph down-nudge"
        );

        // Word tokens ride high of the single-glyph nudge and gain padding.
        assert!(is_footer_word_key_token("Space"));
        assert!(!is_footer_word_key_token("⌘"));
        assert_eq!(
            footer_key_glyph_nudge_y("Space"),
            metrics.word_glyph_nudge_y
        );
        assert!(
            metrics.word_glyph_nudge_y < metrics.key_glyph_nudge_y,
            "descender words sat low in the chip"
        );
        assert!(
            metrics.word_keycap_padding_x > metrics.keycap_padding_x,
            "word keycaps need horizontal breathing room"
        );

        // Single glyphs keep the uniform treatment.
        assert_eq!(footer_key_glyph_nudge_x("K"), 0.0);
        assert_eq!(footer_key_glyph_nudge_y("K"), metrics.key_glyph_nudge_y);
        assert_eq!(
            footer_key_glyph_nudge_y(";"),
            metrics.semicolon_glyph_nudge_y
        );
        assert_eq!(
            footer_key_glyph_nudge_y("↵"),
            metrics.key_glyph_nudge_y + metrics.return_glyph_nudge_y
        );
        assert_eq!(footer_appkit_glyph_x("⌘", 20.0, 10.0), 5.5);
        assert_eq!(footer_appkit_glyph_x("↵", 20.0, 10.0), 5.0);
        assert_eq!(
            footer_appkit_glyph_y("↵", 20.0, 10.0),
            5.0 - metrics.key_glyph_nudge_y as f64 - metrics.return_glyph_nudge_y as f64
        );
    }

    #[test]
    fn split_footer_shortcut_parses_simple_and_complex_keys() {
        assert_eq!(split_footer_shortcut(""), Vec::<String>::new());
        assert_eq!(split_footer_shortcut("↵"), vec!["↵"]);
        assert_eq!(split_footer_shortcut("⌘K"), vec!["⌘", "K"]);
        assert_eq!(split_footer_shortcut("⌥↵"), vec!["⌥", "↵"]);
        assert_eq!(split_footer_shortcut("Enter"), vec!["↵"]);
        assert_eq!(split_footer_shortcut("esc"), vec!["⎋"]);
        assert_eq!(split_footer_shortcut("Escape"), vec!["⎋"]);
        assert_eq!(split_footer_shortcut("Cmd+K"), vec!["⌘", "K"]);
        assert_eq!(split_footer_shortcut("⌘F1"), vec!["⌘", "F1"]);
        assert_eq!(split_footer_shortcut("⌥⌘I"), vec!["⌥", "⌘", "I"]);
        assert_eq!(split_footer_shortcut("click"), vec!["click"]);
        // A trailing '+' is the plus key (terminal Zoom In), never an
        // empty keycap.
        assert_eq!(split_footer_shortcut("⌘+"), vec!["⌘", "+"]);
        assert_eq!(split_footer_shortcut("Ctrl++"), vec!["⌃", "+"]);
        assert_eq!(split_footer_shortcut("⌘-"), vec!["⌘", "-"]);
        assert_eq!(split_footer_shortcut("⌃\\"), vec!["⌃", "\\"]);
    }

    #[test]
    fn split_footer_shortcut_covers_help_guidance_tokens() {
        assert_eq!(split_footer_shortcut("/"), vec!["/"]);
        assert_eq!(split_footer_shortcut("@"), vec!["@"]);
        assert_eq!(split_footer_shortcut("⇧↵"), vec!["⇧", "↵"]);
        assert_eq!(split_footer_shortcut("⌘P"), vec!["⌘", "P"]);
        assert_eq!(split_footer_shortcut(";todo"), vec![";TODO"]);
        assert_eq!(split_footer_shortcut(":tag:"), vec![":TAG:"]);
    }

    #[test]
    fn footer_action_frame_shrinks_with_flex_content_not_estimated_widths() {
        let source = include_str!("footer_chrome.rs");
        let frame_start = source
            .find("pub(crate) fn render_footer_hint_action_button_frame")
            .expect("action button frame renderer should exist");
        let frame_source = &source[frame_start..];
        let frame_body = &frame_source[..frame_source
            .find("\n}\n")
            .expect("frame renderer should terminate")];

        assert!(
            frame_body.contains("render_footer_hint_content_flex_with_layout"),
            "shrink-to-content frames must hug the rendered flex content"
        );
        assert!(
            !frame_body.contains("footer_hint_action_visual_width_px"),
            "shrink-to-content frames must not derive widths from per-char text estimates"
        );
        assert!(
            frame_body.contains(".max_w(px(spec.slot_width_px))"),
            "the content-hugging frame must stay bounded by the fixed slot"
        );

        // Universal-chrome lock (2026-07-20): glass in-window footers across
        // surfaces render ONE config-driven rail so the GPUI fallback can
        // never drift from the native footer language. The renderer's input
        // must stay MainWindowFooterConfig (the same model the native footer
        // consumes) and it must compose the shared frames above.
        let rail_start = source
            .find("pub(crate) fn render_main_window_footer_config_rail")
            .expect("config-driven footer rail renderer should exist");
        let rail_source = &source[rail_start..];
        let rail_body = &rail_source[..rail_source
            .find("\n}\n")
            .expect("config rail renderer should terminate")];
        assert!(
            rail_body.contains("MainWindowFooterConfig"),
            "the shared rail must consume the same config model as the native footer"
        );
        assert!(
            rail_body.contains("render_footer_hint_action_button_frame"),
            "the shared rail must compose the shared footer button frames"
        );

        // Liquid Glass consistency lock: the footer rail itself never reaches
        // into AppKit. Both row and standalone opt-ins converge on the one
        // painted-bounds wrapper, and that wrapper is the only owner here of
        // raw native sync + group/index hover routing.
        let footer_rail = function_body(
            source,
            "pub(crate) fn render_footer_action_rail_with_leading",
        );
        assert!(footer_rail.contains("render_footer_action_rail_with_leading_and_cleanup("));
        assert!(!footer_rail.contains("glass_button_host::sync_for_window"));

        let capsule_row = function_body(source, "pub(crate) fn glass_capsule_row");
        assert!(capsule_row.contains("glass_capsule_row_with_cleanup("));
        let capsule_row_core = function_body(source, "fn glass_capsule_row_with_cleanup");
        for required in [
            ".on_children_prepainted(",
            "glass_button_host::sync_for_window(window, group, &frames)",
            "glass_button_host::set_hover(window, group, index, *hovered)",
        ] {
            assert!(
                capsule_row_core.contains(required),
                "shared glass row must retain {required}"
            );
        }

        let standalone = function_body(source, "pub(crate) fn glass_capsule(");
        assert!(standalone.contains("glass_capsule_row("));
    }

    #[test]
    fn key_anchored_footer_content_keeps_symmetric_outer_padding() {
        assert_eq!(FOOTER_KEY_ANCHORED_CONTENT_PADDING_X_PX, 6.0);
    }

    #[test]
    fn footer_key_glyph_nudges_match_footer_contract() {
        assert!(is_footer_return_key_glyph("↵"));
        assert!(!is_footer_return_key_glyph("Enter"));
        assert_eq!(footer_key_glyph_nudge_x("⌘"), 0.5);
        assert_eq!(footer_key_glyph_nudge_y("⌘"), 0.0);
        assert_eq!(footer_key_glyph_nudge_y("Space"), -0.75);
        assert_eq!(footer_key_glyph_nudge_y("↵"), 2.0);
        assert_eq!(footer_key_glyph_nudge_y(";"), -1.0);
        assert_eq!(footer_appkit_glyph_y("⌘", 20.0, 10.0), 5.0);
        assert_eq!(footer_appkit_glyph_y("↵", 20.0, 10.0), 3.0);
        assert_eq!(footer_appkit_glyph_y(";", 20.0, 10.0), 6.0);
        assert_eq!(footer_button_height(32.0), 28.0);
    }

    #[test]
    fn footer_horizontal_run_width_uses_gap_only_between_items() {
        // 40 + 20 + 20 + 2 gaps * 2px = 84
        assert_eq!(
            footer_horizontal_run_width_px(&[40.0, 20.0, 20.0], FOOTER_ACTION_ITEM_GAP_PX),
            84.0
        );
        assert_eq!(
            footer_horizontal_run_width_px(&[], FOOTER_ACTION_ITEM_GAP_PX),
            0.0
        );
        // A single item has no inter-item gap.
        assert_eq!(
            footer_horizontal_run_width_px(&[40.0], FOOTER_ACTION_ITEM_GAP_PX),
            40.0
        );
    }

    #[test]
    fn footer_horizontal_run_origins_use_constant_gap() {
        assert_eq!(
            footer_horizontal_run_origins_px(&[40.0, 20.0, 20.0], FOOTER_ACTION_ITEM_GAP_PX, 0.0),
            vec![0.0, 42.0, 64.0]
        );
        // The same run anchored at a non-zero origin just shifts every item.
        assert_eq!(
            footer_horizontal_run_origins_px(&[40.0, 20.0], FOOTER_ACTION_ITEM_GAP_PX, 10.0),
            vec![10.0, 52.0]
        );
    }

    #[test]
    fn floating_glass_rail_is_edge_flush_while_legacy_rail_keeps_its_inset() {
        let ordinary = crate::window_resize::main_layout::HINT_STRIP_PADDING_X;
        assert_eq!(footer_rail_side_inset_px(true, ordinary), 0.0);
        assert_eq!(footer_rail_side_inset_px(false, ordinary), ordinary);
    }

    #[test]
    fn footer_action_chrome_matches_canonical_main_menu_row_palette() {
        assert_eq!(FOOTER_ACTION_ITEM_GAP_PX, 2.0);
        assert_eq!(FOOTER_GLASS_BUTTON_GAP_PX, 6.0);
        assert_eq!(FOOTER_ACTION_CONTENT_GAP_PX, 4.0);
        assert_eq!(FOOTER_ACTION_CONTENT_PADDING_X_PX, 4.0);
        assert_eq!(FOOTER_ACTION_BUTTON_RADIUS_PX, 6.0);
        assert_eq!(footer_centered_action_edge_padding_x(), 10.0);
        assert_eq!(FOOTER_RUN_SLOT_MIN_WIDTH_PX, 92.0);
        assert_eq!(FOOTER_RUN_SLOT_MAX_WIDTH_PX, 242.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Actions), 92.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Ai), 52.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Apply), 84.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Close), 84.0);
        assert_eq!(footer_action_slot_width(FooterActionSlot::Stop), 76.0);
        assert_eq!(
            footer_action_slot_width(FooterActionSlot::PasteResponse),
            140.0
        );

        let mut theme = Theme::dark_default();
        let mut opacity = theme.get_opacity();
        opacity.hover = 0.12;
        opacity.selected = 0.31;
        theme.opacity = Some(opacity);

        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let palette = crate::theme::resolve_main_menu_row_state_palette(
            &theme,
            crate::designs::current_main_menu_theme(),
        );
        let rail = footer_rail_chrome(&theme);
        assert_eq!(rail.height_px, current_main_menu_footer_height());
        assert_eq!(
            rail.side_inset_px,
            crate::window_resize::main_layout::HINT_STRIP_PADDING_X
        );
        assert_eq!(rail.surface_rgba, chrome.inline_dropdown_surface_rgba);
        assert_eq!(rail.divider_rgba, chrome.divider_rgba);
        assert_eq!(
            rail.hover_rgba,
            palette.hover.background_rgba.expect("hover has a fill")
        );
        assert_eq!(
            rail.active_rgba,
            palette.active.background_rgba.expect("active has a fill")
        );
        assert_eq!(
            themed_footer_button_rest_rgba(&theme),
            palette.rest.background_rgba
        );
        assert_eq!(themed_footer_button_hover_rgba(&theme), rail.hover_rgba);
        assert_eq!(themed_footer_button_active_rgba(&theme), rail.active_rgba);
        assert_eq!(
            footer_hint_text_color(&theme),
            gpui::rgba(palette.rest.primary_foreground_rgba)
        );
        let expected_hover_foreground: gpui::Hsla =
            gpui::rgba(palette.hover.primary_foreground_rgba).into();
        assert_eq!(
            footer_hover_text_color(&theme, None),
            expected_hover_foreground
        );
        assert_eq!(rail.button_radius_px, FOOTER_ACTION_BUTTON_RADIUS_PX);
    }

    #[test]
    fn footer_button_colors_match_main_menu_rows_in_dark_and_light() {
        for theme in [Theme::dark_default(), Theme::light_default()] {
            for variant in crate::designs::MainMenuThemeVariant::all() {
                let row_palette =
                    crate::theme::resolve_main_menu_row_state_palette(&theme, *variant);
                let footer_palette =
                    resolved_footer_button_visual_colors_for_variant(&theme, *variant).row_states;

                assert_eq!(
                    footer_palette, row_palette,
                    "footer/list palette drift for {:?}",
                    variant
                );
                for state in [
                    crate::theme::MainMenuRowState::Rest,
                    crate::theme::MainMenuRowState::Hover,
                    crate::theme::MainMenuRowState::Active,
                ] {
                    let footer_state = footer_palette.for_state(state);
                    let row_state = row_palette.for_state(state);
                    assert_eq!(footer_state.background_rgba, row_state.background_rgba);
                    assert_eq!(
                        footer_state.primary_foreground_rgba,
                        row_state.primary_foreground_rgba
                    );
                }
            }
        }
    }

    #[test]
    fn footer_keycap_border_alpha_is_visible_and_stronger_on_hover() {
        let mut theme = Theme::dark_default();
        let mut opacity = theme.get_opacity();
        opacity.hover = 0.12;
        opacity.selected = 0.31;
        theme.opacity = Some(opacity);

        assert_eq!(
            footer_keycap_border_alpha(&theme, false),
            FOOTER_CHIP_BORDER_ALPHA
        );
        assert_eq!(
            footer_keycap_border_alpha(&theme, true),
            FOOTER_CHIP_BORDER_SELECTED_ALPHA
        );
        assert!(
            (footer_keycap_border_hover_alpha(&theme) - FOOTER_CHIP_BORDER_HOVER_ALPHA).abs()
                <= 0.01
        );
        assert!(footer_keycap_border_color(&theme).a >= FOOTER_CHIP_BORDER_ALPHA - 0.01);
        assert!(
            footer_keycap_border_hover_color_with_alpha(&theme, None).a
                >= FOOTER_CHIP_BORDER_HOVER_ALPHA - 0.01
        );
        assert!(
            footer_keycap_border_color_for_state(&theme, true).a
                >= FOOTER_CHIP_BORDER_SELECTED_ALPHA - 0.01
        );
    }

    #[test]
    fn footer_keycap_border_policy_matches_rest_hover_active_contract() {
        let rest = 0.11;
        let hover = 0.37;
        let active = 0.73;
        assert_eq!(
            footer_keycap_border_alpha_for_state_values(
                crate::theme::MainMenuRowState::Rest,
                rest,
                hover,
                active,
            ),
            rest
        );
        assert_eq!(
            footer_keycap_border_alpha_for_state_values(
                crate::theme::MainMenuRowState::Hover,
                rest,
                hover,
                active,
            ),
            hover
        );
        assert_eq!(
            footer_keycap_border_alpha_for_state_values(
                crate::theme::MainMenuRowState::Active,
                rest,
                hover,
                active,
            ),
            active
        );
    }
}
