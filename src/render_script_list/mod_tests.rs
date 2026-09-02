#[cfg(test)]
mod render_script_list_footer_tests {
    use super::{
        app_shell_footer_colors, inline_calc_list_item_hint_text_color,
        inline_calc_list_item_result_text_color, inline_calc_list_item_selected_overlay_rgba,
        inline_calc_list_item_title, menu_syntax_single_line_text_for_gpui,
        script_list_footer_info_label,
    };
    use crate::designs::DesignVariant;
    use crate::theme::ColorResolver;

    #[test]
    fn test_app_shell_footer_colors_use_theme_accent_tokens() {
        let theme = crate::theme::Theme::default();
        let colors = app_shell_footer_colors(&theme);

        assert_eq!(colors.accent, theme.colors.accent.selected);
        assert_eq!(colors.background, theme.colors.accent.selected_subtle);
        assert_eq!(colors.border, theme.colors.ui.border);
        assert_eq!(colors.text_muted, theme.colors.text.muted);
    }

    #[test]
    fn test_universal_prompt_hints_support_custom_primary_label() {
        let hints = crate::components::universal_prompt_hints_with_primary_label("Open App");
        assert_eq!(hints[0].as_ref(), "↵ Open App");
    }

    #[test]
    fn test_script_list_footer_info_label_hidden_when_window_tweaker_disabled() {
        assert_eq!(
            script_list_footer_info_label(false, false, 75, "acrylic", "light"),
            None
        );
    }

    #[test]
    fn test_script_list_footer_info_label_hidden_in_dark_mode() {
        assert_eq!(
            script_list_footer_info_label(true, true, 75, "acrylic", "dark"),
            None
        );
    }

    #[test]
    fn test_script_list_footer_info_label_formats_window_tweaker_metadata() {
        assert_eq!(
            script_list_footer_info_label(true, false, 75, "acrylic", "light"),
            Some("75% | acrylic | light | ⌘-/+ ⌘M ⌘⇧A".to_string())
        );
    }

    #[test]
    fn test_truncate_str_chars_returns_valid_utf8_boundary_when_filter_text_is_multibyte() {
        let input = "é".repeat(45);
        let truncated = crate::utils::truncate_str_chars(&input, 27);

        assert_eq!(truncated.chars().count(), 27);
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn test_menu_syntax_single_line_text_for_gpui_replaces_newlines() {
        let rendered = menu_syntax_single_line_text_for_gpui("first\ns\r\nthird");

        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\r'));
        assert!(rendered.contains("first"));
        assert!(rendered.contains('s'));
        assert!(rendered.contains("third"));
    }

    #[test]
    fn test_inline_calc_list_item_title_prefixes_equals_sign() {
        assert_eq!(inline_calc_list_item_title("1500"), "= 1500");
    }

    #[test]
    fn test_inline_calc_result_text_color_does_use_resolver_accent_when_selected_non_default() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.accent.selected = 0x112233;
        let color_resolver = ColorResolver::new(&theme, DesignVariant::NeonCyberpunk);

        let color = inline_calc_list_item_result_text_color(
            true,
            DesignVariant::NeonCyberpunk,
            &theme,
            color_resolver,
        );

        assert_eq!(color, color_resolver.primary_accent());
        assert_ne!(color, theme.colors.accent.selected);
    }

    #[test]
    fn test_inline_calc_hint_text_color_does_use_color_resolver_muted_token() {
        let theme = crate::theme::Theme::default();
        let color_resolver = ColorResolver::new(&theme, DesignVariant::NeonCyberpunk);

        assert_eq!(
            inline_calc_list_item_hint_text_color(color_resolver),
            color_resolver.empty_text_color()
        );
    }

    #[test]
    fn test_inline_calc_selected_overlay_does_use_resolver_accent_with_theme_alpha() {
        let mut theme = crate::theme::Theme::default();
        theme.colors.accent.selected_subtle = 0x010203;
        let color_resolver = ColorResolver::new(&theme, DesignVariant::NeonCyberpunk);

        let expected_alpha =
            ((theme.get_opacity().selected.clamp(0.0, 1.0) * 255.0).round() as u32).max(
                crate::designs::MainMenuThemeVariant::InfoBarBase
                    .def()
                    .list
                    .inline_calc_selected_overlay_min_alpha,
            );
        let expected = (color_resolver.primary_accent() << 8) | expected_alpha;

        assert_eq!(
            inline_calc_list_item_selected_overlay_rgba(
                &theme,
                crate::designs::MainMenuThemeVariant::InfoBarBase.def().list,
                color_resolver
            ),
            expected
        );
    }
}

#[cfg(test)]
mod launcher_empty_info_state_contract_tests {
    use std::fs;

    #[test]
    fn launcher_empty_state_routes_through_info_state() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("failed to read src/render_script_list/mod.rs");
        let old_empty_title = concat!("No scripts or ", "snippets found");
        let old_empty_hint = concat!("Press ", "⌘N", " to create a new script");
        let old_generic_fallback =
            concat!("Try a different search term or press ", "⌘↵", " to ask AI");

        assert!(
            source.contains("render_launcher_empty_or_no_results"),
            "launcher empty/no-results must render through shared InfoState"
        );
        assert!(
            !source.contains(old_empty_title),
            "old launcher empty title must not return"
        );
        assert!(
            !source.contains(old_empty_hint),
            "old launcher empty hint must not return"
        );
        assert!(
            !source.contains(old_generic_fallback),
            "old generic no-results fallback must not return"
        );
    }
}
