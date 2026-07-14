mod arg_prompt_render_tests {
    use super::*;

    use crate::designs::{get_tokens, DesignColors, DesignVariant};
    use crate::theme::Theme;
    use crate::protocol::Choice;

    fn choice(name: &str, value: &str) -> Choice {
        Choice::new(name.to_string(), value.to_string())
    }

    #[test]
    fn arg_input_visible_chars_scale_with_width_and_char_size() {
        // Wider window fits more characters at the same glyph width.
        assert!(
            arg_input_visible_chars_for_width(900.0, 8.0)
                > arg_input_visible_chars_for_width(400.0, 8.0)
        );
        // Bigger glyphs fit fewer characters in the same window.
        assert!(
            arg_input_visible_chars_for_width(600.0, 7.0)
                > arg_input_visible_chars_for_width(600.0, 10.0)
        );
        // Tiny windows clamp up to a useful minimum.
        assert_eq!(arg_input_visible_chars_for_width(50.0, 10.0), 24);
        // Huge windows clamp down so windowing never fully disables.
        assert_eq!(arg_input_visible_chars_for_width(10_000.0, 1.0), 240);
        // Degenerate glyph widths cannot divide by zero.
        assert_eq!(arg_input_visible_chars_for_width(600.0, 0.0), 240);
    }

    #[test]
    fn prompt_actions_dialog_offsets_match_legacy_defaults() {
        let tokens = get_tokens(DesignVariant::Default);
        let spacing = tokens.spacing();
        let visual = tokens.visual();

        let (top, right) = prompt_actions_dialog_offsets(spacing.padding_sm, visual.border_thin);
        assert_eq!(top, 52.0);
        assert_eq!(right, 8.0);
    }

    #[test]
    fn test_prompt_render_context_new_extracts_design_tokens_and_offsets() {
        let theme = Theme::default();
        let variant = DesignVariant::Default;
        let tokens = get_tokens(variant);

        let context = PromptRenderContext::new(&theme, variant);
        let (expected_top, expected_right) = prompt_actions_dialog_offsets(
            context.design_spacing.padding_sm,
            context.design_visual.border_thin,
        );

        assert!(std::ptr::eq(context.theme, &theme));
        assert_eq!(context.design_colors, tokens.colors());
        assert_eq!(context.design_spacing, tokens.spacing());
        assert_eq!(context.design_typography, tokens.typography());
        assert_eq!(context.design_visual, tokens.visual());
        assert_eq!(context.actions_dialog_top, expected_top);
        assert_eq!(context.actions_dialog_right, expected_right);
    }

    #[test]
    fn prompt_footer_config_has_consistent_actions_defaults() {
        let config =
            prompt_footer_config_with_status("Continue", true, Some("Running".into()), None);
        assert_eq!(config.primary_label, "Continue");
        assert_eq!(config.primary_shortcut, "↵");
        assert_eq!(config.secondary_label, "Actions");
        assert_eq!(config.secondary_shortcut, "⌘K");
        assert!(config.show_secondary);
        assert_eq!(config.helper_text.as_deref(), Some("Running"));
    }

    #[test]
    fn prompt_footer_colors_use_shared_theme_surface() {
        let design_colors = DesignColors {
            background_secondary: 0x123456,
            background_selected: 0xabcdef,
            ..DesignColors::default()
        };

        let footer_colors = prompt_footer_colors_for_prompt(&design_colors, true);
        let theme = crate::theme::get_cached_theme();

        assert_eq!(footer_colors.background, theme.colors.accent.selected_subtle);
        assert_eq!(footer_colors.is_light_mode, !theme.is_dark_mode());
    }

    #[test]
    fn test_footer_surface_color_uses_surface_token_in_light_mode() {
        let footer = crate::components::prompt_footer::PromptFooterColors {
            accent: 0,
            text_muted: 0,
            text_primary: 0,
            border: 0,
            surface: 0x123456,
            background: 0x000000,
            hover_alpha: 0,
            selected_alpha: 0,
            is_light_mode: true,
        };

        assert_eq!(
            crate::components::prompt_footer::footer_surface_rgba(footer),
            0x123456ff
        );
    }

    #[test]
    fn running_status_text_is_contextual() {
        assert_eq!(
            running_status_text("awaiting input"),
            "Script running · awaiting input"
        );
    }

    #[test]
    fn test_resolve_arg_submit_outcome_returns_invalid_when_input_is_empty() {
        let outcome = resolve_arg_submit_outcome(None, "");
        assert_eq!(outcome, ArgSubmitOutcome::InvalidEmpty);
    }

    #[test]
    fn test_resolve_arg_submit_outcome_returns_selected_choice_value_when_available() {
        let outcome = resolve_arg_submit_outcome(Some("selected-choice"), "typed value");
        assert_eq!(
            outcome,
            ArgSubmitOutcome::SubmitChoice("selected-choice".to_string())
        );
    }

    #[test]
    fn test_resolve_arg_submit_outcome_returns_raw_text_when_no_selection_and_non_empty_input() {
        let outcome = resolve_arg_submit_outcome(None, "typed value");
        assert_eq!(
            outcome,
            ArgSubmitOutcome::SubmitText("typed value".to_string())
        );
    }

    #[test]
    fn test_resolve_arg_tab_completion_returns_single_choice_when_single_match() {
        let choices = [choice("Alpha", "alpha")];
        let filtered: Vec<(usize, &Choice)> = choices.iter().enumerate().collect();
        assert_eq!(
            resolve_arg_tab_completion(&filtered, 0),
            Some("Alpha".to_string())
        );
    }

    #[test]
    fn test_resolve_arg_tab_completion_uses_selected_choice_when_multiple_matches() {
        let choices = [choice("Alpha", "alpha"), choice("Bravo", "bravo")];
        let filtered: Vec<(usize, &Choice)> = choices.iter().enumerate().collect();
        assert_eq!(
            resolve_arg_tab_completion(&filtered, 1),
            Some("Bravo".to_string())
        );
    }

    #[test]
    fn test_resolve_arg_tab_completion_falls_back_to_first_choice_when_selection_is_oob() {
        let choices = [choice("Alpha", "alpha"), choice("Bravo", "bravo")];
        let filtered: Vec<(usize, &Choice)> = choices.iter().enumerate().collect();
        assert_eq!(
            resolve_arg_tab_completion(&filtered, 99),
            Some("Alpha".to_string())
        );
    }

    #[test]
    fn test_prompt_render_context_constructor_is_used_across_prompt_renderers() {
        assert!(
            include_str!("render.rs").contains("PromptRenderContext::new("),
            "arg prompt render should construct PromptRenderContext"
        );
        assert!(
            include_str!("../div.rs").contains("PromptRenderContext::new("),
            "div prompt render should construct PromptRenderContext"
        );
        assert!(
            include_str!("../editor.rs").contains("PromptRenderContext::new("),
            "editor prompt render should construct PromptRenderContext"
        );
        assert!(
            include_str!("../form/render.rs").contains("PromptRenderContext::new("),
            "form prompt render should construct PromptRenderContext"
        );
        assert!(
            include_str!("../term.rs").contains("PromptRenderContext::new("),
            "term prompt render should construct PromptRenderContext"
        );
        assert!(
            include_str!("../other.rs").contains("PromptRenderContext::new("),
            "other prompt render helpers should construct PromptRenderContext"
        );
    }

    #[test]
    fn test_key_preamble_helper_is_used_across_prompt_renderers() {
        assert!(
            include_str!("helpers.rs").contains("fn key_preamble("),
            "arg helpers should define key_preamble"
        );
        assert!(
            include_str!("render.rs").contains("handle_prompt_key_preamble_default("),
            "arg prompt key handler should use handle_prompt_key_preamble_default"
        );
        // Every preamble-based prompt renderer routes through the shared cfg
        // struct; none may declare its own dismissability — Escape ownership
        // derives from the DismissPolicy table inside key_preamble.
        for (name, source) in [
            ("div", include_str!("../div.rs")),
            ("editor", include_str!("../editor.rs")),
            ("form", include_str!("../form/render.rs")),
            ("term", include_str!("../term.rs")),
        ] {
            assert!(
                (source.contains("handle_prompt_key_preamble(")
                    || source.contains("handle_prompt_key_preamble_default("))
                    && source.contains("PromptKeyPreambleCfg {"),
                "{name} prompt key handler should use the shared preamble cfg"
            );
            assert!(
                !source.contains("is_dismissable:"),
                "{name} prompt must not declare per-renderer dismissability — \
                 Escape derives from AppView::dismiss_policy()"
            );
        }
        assert!(
            include_str!("helpers.rs").contains("GlobalShortcutEscape::FromDismissPolicy"),
            "key_preamble must derive Escape behavior from the dismiss-policy table"
        );
    }

    #[test]
    fn arg_prompt_uses_universal_hints() {
        let render_source = include_str!("render.rs");
        assert!(
            render_source.contains("universal_prompt_hints()"),
            "arg render should use universal_prompt_hints for the three-key footer"
        );
        assert!(
            !render_source.contains("arg_prompt_hints("),
            "arg render should no longer use custom arg_prompt_hints"
        );
    }

    #[test]
    fn arg_prompt_emits_chrome_audit() {
        let render_source = include_str!("render.rs");
        assert!(
            render_source.contains("PromptChromeAudit::minimal_list("),
            "arg render should emit a minimal_list chrome audit"
        );
        assert!(
            render_source.contains("\"render_prompts::arg\""),
            "arg chrome audit should identify surface as render_prompts::arg"
        );
        assert!(
            render_source.contains("emit_prompt_chrome_audit("),
            "arg render should call emit_prompt_chrome_audit"
        );
    }

    #[test]
    fn arg_render_prompt_legacy_file_removed() {
        // The legacy render_prompt.rs that used PromptFooter has been deleted.
        // Only render.rs (with minimal chrome) should exist as the active entry point.
        let arg_rs_source = include_str!("../arg.rs");
        assert!(
            !arg_rs_source.contains("render_prompt.rs"),
            "arg.rs should not include the legacy render_prompt.rs"
        );
        assert!(
            arg_rs_source.contains("include!(\"arg/render.rs\")"),
            "arg.rs should include the minimal chrome render.rs"
        );
    }
}
