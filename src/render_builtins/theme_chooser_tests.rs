#[cfg(test)]
mod theme_chooser_zero_match_paint_tests {
    use super::*;
    use gpui::AppContext;

    struct TestThemeChooserZeroMatch;

    impl gpui::Render for TestThemeChooserZeroMatch {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let list_colors =
                crate::list_item::ListItemColors::from_theme(&crate::theme::Theme::default());

            render_theme_chooser_browser_main(
                "definitely-no-theme-matches",
                0,
                div().child("unexpected result row").into_any_element(),
                div()
                    .flex_1()
                    .child("No matching themes")
                    .into_any_element(),
                div().w_1_2().child("Theme Preview").into_any_element(),
                list_colors,
            )
        }
    }

    #[gpui::test]
    fn zero_match_renders_builtin_leading_separator_with_non_zero_paint_bounds(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::px;

        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(748.0), px(320.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestThemeChooserZeroMatch))
                .expect("theme chooser zero-match paint test window should open")
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                let measurement = window
                    .debug_bounds_entries()
                    .iter()
                    .find(|entry| {
                        entry.selector
                            == crate::components::builtin_leading_separator::BUILTIN_LEADING_SEPARATOR_ID
                    })
                    .expect("zero-match theme chooser should paint the shared leading separator");

                assert!(measurement.bounds.size.width > px(0.0));
                assert!(measurement.bounds.size.height > px(0.0));
            })
            .expect("theme chooser zero-match paint test window should remain available");
    }
}

#[cfg(test)]
mod theme_chooser_chrome_audit {
    //! Source audits for footer/chrome decisions in the Theme Designer.
    //! Forbidden-pattern needles are built with `concat!` so the audit can
    //! never match its own assertion text (the failure mode that previously
    //! made these tests fail on a clean tree).

    #[test]
    fn theme_chooser_uses_truthful_actions_footer() {
        let source = include_str!("theme_chooser.rs");
        let generic_hints = concat!("universal_prompt_", "hints()");
        assert!(
            !source.contains(generic_hints),
            "theme_chooser should use its own truthful hint set"
        );
        assert!(
            source.contains("render_simple_hint_strip("),
            "theme_chooser should use render_simple_hint_strip"
        );
        for label in ["↵ Done", "⌘K Actions", "Esc Undo"] {
            assert!(
                source.contains(&format!("SharedString::from(\"{label}\")")),
                "theme_chooser footer should advertise truthful '{label}' hint"
            );
        }
        // Chaos OF-7: the footer is capped at the three-affordance budget
        // (.impeccable.md); Customize/Remix moved to ⌘K discovery. Their
        // shortcut labels must NOT reappear as footer hints.
        for banished in [
            "SharedString::from(\"⌘E Customize\")",
            "SharedString::from(\"⌘J Remix\")",
        ] {
            assert!(
                !source.contains(banished),
                "footer is capped at 3 affordances; {banished} belongs in the ⌘K actions dialog"
            );
        }
    }

    #[test]
    fn theme_chooser_footer_holds_three_affordance_budget() {
        // Behavior-rung lock for the same OF-7 decision: the hint items
        // themselves stay within the .impeccable.md footer budget.
        let hints = crate::ScriptListApp::theme_chooser_hint_items();
        assert_eq!(
            hints.len(),
            3,
            "footer ≤3 affordances; discovery lives in ⌘K"
        );
    }

    #[test]
    fn theme_chooser_has_no_legacy_multi_shortcut_footer() {
        let source = include_str!("theme_chooser.rs");
        let legacy = concat!(".child(short", "cut(\"⌘[]\", \"Accent\"))");
        assert!(
            !source.contains(legacy),
            "theme_chooser should not have legacy multi-shortcut footer"
        );
    }

    #[test]
    fn theme_chooser_has_no_prompt_footer() {
        let source = include_str!("theme_chooser.rs");
        let legacy = concat!("Prompt", "Footer::new(");
        assert!(
            !source.contains(legacy),
            "theme_chooser should not use PromptFooter"
        );
    }

    #[test]
    fn theme_chooser_live_preview_uses_shared_shell() {
        let source = include_str!("theme_chooser.rs");
        assert!(
            source.contains("render_minimal_list_prompt_shell("),
            "theme_chooser live preview should use the shared minimal-list prompt shell"
        );
        let bespoke_keycap = concat!("render_theme_chooser_preview_", "keycap");
        assert!(
            !source.contains(bespoke_keycap),
            "bespoke preview keycap helper should be removed"
        );
    }
}

#[cfg(test)]
mod theme_chooser_filter_tests {
    use super::*;

    fn preset_only_catalog() -> Vec<ThemeChooserCatalogEntry> {
        let presets = theme::presets::presets_cached();
        let preview_colors = theme::presets::preset_preview_colors_cached();
        presets
            .iter()
            .enumerate()
            .map(|(index, preset)| ThemeChooserCatalogEntry {
                kind: ThemeChooserCatalogKind::BuiltIn(index),
                name: preset.name.to_string(),
                description: preset.description.to_string(),
                is_dark: preset.is_dark,
                theme: theme::presets::preset_theme_cached(index),
                preview_colors: preview_colors[index],
            })
            .collect()
    }

    #[test]
    fn test_theme_chooser_catalog_filter_returns_all_entries_when_filter_empty() {
        let catalog = preset_only_catalog();
        let filtered = ScriptListApp::theme_chooser_catalog_filtered_indices("", &catalog);
        assert_eq!(filtered.len(), catalog.len());
    }

    #[test]
    fn test_theme_chooser_catalog_filter_matches_ascii_filter_case_insensitively() {
        let catalog = preset_only_catalog();
        let presets = theme::presets::presets_cached();
        let dracula_index = presets
            .iter()
            .position(|preset| preset.id == "dracula")
            .expect("dracula preset should exist");

        let filtered = ScriptListApp::theme_chooser_catalog_filtered_indices("DRAC", &catalog);
        assert!(filtered.contains(&dracula_index));
    }

    #[test]
    fn test_theme_chooser_catalog_filter_matches_user_theme_slug() {
        let mut catalog = preset_only_catalog();
        catalog.insert(
            0,
            ThemeChooserCatalogEntry {
                kind: ThemeChooserCatalogKind::User {
                    slug: "my-night-shift".to_string(),
                    source_fingerprint: "fixture-source".to_string(),
                },
                name: "Night Shift".to_string(),
                description: "User theme saved in ~/.scriptkit/themes".to_string(),
                is_dark: true,
                theme: theme::presets::preset_theme_cached(0),
                preview_colors: theme::presets::preset_preview_colors_cached()[0],
            },
        );

        let by_slug =
            ScriptListApp::theme_chooser_catalog_filtered_indices("night-shift", &catalog);
        assert!(
            by_slug.contains(&0),
            "user themes must stay findable by slug"
        );
        let by_kind = ScriptListApp::theme_chooser_catalog_filtered_indices("custom", &catalog);
        assert!(
            by_kind.contains(&0),
            "user themes must match the 'custom' keyword"
        );
    }

    #[test]
    fn test_theme_chooser_catalog_index_prefers_exact_user_theme_fingerprint() {
        let mut catalog = preset_only_catalog();
        let user_theme = theme::presets::preset_theme_cached(3);
        catalog.insert(
            0,
            ThemeChooserCatalogEntry {
                kind: ThemeChooserCatalogKind::User {
                    slug: "saved-copy".to_string(),
                    source_fingerprint: "fixture-source".to_string(),
                },
                name: "Saved Copy".to_string(),
                description: "User theme saved in ~/.scriptkit/themes".to_string(),
                is_dark: user_theme.has_dark_colors(),
                theme: std::sync::Arc::clone(&user_theme),
                preview_colors: theme::presets::preset_preview_colors_cached()[3],
            },
        );

        let index =
            ScriptListApp::theme_chooser_catalog_index_for_theme(&catalog, user_theme.as_ref());
        assert_eq!(
            index, 0,
            "opening the designer should land on the matching user theme row"
        );
    }

    #[test]
    fn test_theme_chooser_filter_hex_parses_only_full_hex_queries() {
        assert_eq!(
            ScriptListApp::theme_chooser_filter_hex("#FF8800"),
            Some(0xFF8800)
        );
        assert_eq!(
            ScriptListApp::theme_chooser_filter_hex("  #ff8800  "),
            Some(0xFF8800)
        );
        assert_eq!(ScriptListApp::theme_chooser_filter_hex("ff8800"), None);
        assert_eq!(ScriptListApp::theme_chooser_filter_hex("#ff88"), None);
        assert_eq!(ScriptListApp::theme_chooser_filter_hex("nord"), None);
    }

    #[test]
    fn test_accent_on_text_color_prefers_background_for_bright_accent() {
        let bg_main = 0x1E1E1E;
        assert_eq!(best_contrast_of_two(0xFBBF24, 0xFFFFFF, bg_main), bg_main);
    }

    #[test]
    fn test_accent_on_text_color_prefers_white_for_dark_accent() {
        let bg_main = 0x1E1E1E;
        assert_eq!(best_contrast_of_two(0x312E81, 0xFFFFFF, bg_main), 0xFFFFFF);
    }

    #[test]
    fn test_theme_chooser_uses_shared_list_item_row() {
        // The theme chooser uses the shared ListItem component for preset rows,
        // matching the main menu's selection background, description reveal, and spacing.
        let source = include_str!("theme_chooser.rs");
        assert!(
            source.contains("self.theme_chooser_list_state.clone(),"),
            "theme chooser should use the variable-height list state for mixed header/row heights"
        );
        let uniform = concat!("uniform_", "list(");
        assert!(
            !source.contains(uniform),
            "theme chooser should not use uniform_list because rows can grow"
        );
        assert!(
            source.contains("ListItem::new(name.to_string(), list_colors)"),
            "theme chooser preset rows should use the shared ListItem primitive"
        );
        assert!(
            source.contains("leading_accessory(color_bar)"),
            "theme chooser should pass color swatch as leading accessory"
        );
        assert!(
            source.contains("trailing_accessory_opt(saved_badge)"),
            "theme chooser should pass Saved badge as trailing accessory"
        );
    }
}

#[cfg(test)]
mod theme_chooser_actions_dialog_sync_tests {
    #[test]
    fn theme_chooser_preview_updates_open_actions_dialog_theme() {
        let source = include_str!("theme_chooser.rs");
        let preview_fn = source
            .split("fn apply_theme_chooser_theme_preview(")
            .nth(1)
            .and_then(|section| section.split("fn apply_and_persist_theme(").next())
            .expect("missing apply_theme_chooser_theme_preview");
        let restore_fn = source
            .split("fn restore_theme_chooser_theme(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn preview_theme_chooser_catalog_entry(")
                    .next()
            })
            .expect("missing restore_theme_chooser_theme");

        assert!(
            preview_fn.contains("self.sync_open_actions_dialog_theme(cx);"),
            "theme chooser preview mutations should propagate to open actions dialogs"
        );
        assert!(
            restore_fn.contains("self.sync_open_actions_dialog_theme(cx);"),
            "theme chooser restore should propagate to open actions dialogs"
        );
    }

    #[test]
    fn theme_chooser_pure_cancel_paths_do_not_persist_theme_json() {
        // Esc / ⌘W / "Undo Changes and Close" restore the opening snapshot in
        // memory. Persisting on cancel used to rewrite the on-disk theme
        // override on every abandoned browse session.
        let source = include_str!("theme_chooser.rs");
        for restore_reason in [
            "\"theme_chooser_escape_undo\"",
            "\"theme_chooser_close_undo\"",
            "\"theme_chooser_action_undo\"",
        ] {
            let restore_site = source
                .split(restore_reason)
                .next()
                .map(|before| before.len())
                .expect("restore reason should exist");
            let after = &source[restore_site..(restore_site + 600).min(source.len())];
            assert!(
                !after.contains("persist_theme_and_sync_all_windows"),
                "cancel path `{restore_reason}` must not persist the theme override"
            );
        }
    }
}
