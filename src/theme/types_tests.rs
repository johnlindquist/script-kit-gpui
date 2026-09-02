#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LayoutConfig, ScriptKitUserPreferences, ThemeSelectionPreferences};
    use crate::test_utils::lock_theme_cache_test;

    fn with_theme_test_workspace(run: impl FnOnce(&std::path::Path)) {
        let _path_guard = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let workspace = tempfile::tempdir().unwrap();
        temp_env::with_var("SK_PATH", Some(workspace.path().as_os_str()), || {
            run(workspace.path());
        });
    }

    #[test]
    fn persisted_custom_theme_wins_restart_over_current_and_legacy_presets() {
        with_theme_test_workspace(|root| {
            std::fs::write(
                root.join("config.ts"),
                concat!(
                    "import type { Config } from '@scriptkit/sdk';\n",
                    "export default { theme: { presetId: 'nord' } } satisfies Config;\n",
                ),
            )
            .unwrap();
            std::fs::write(
                root.join("settings.json"),
                r#"{"theme":{"presetId":"dracula"}}"#,
            )
            .unwrap();
            let preset = try_load_theme_with_appearance(|| {
                panic!("an explicit preset must not sample system appearance")
            })
            .unwrap();
            let nord = crate::theme::presets::all_presets()
                .into_iter()
                .find(|preset| preset.id == "nord")
                .unwrap()
                .create_theme();
            assert_eq!(preset.colors.accent.selected, nord.colors.accent.selected);

            let mut custom = Theme::dark_default();
            custom.colors.accent.selected = 0x24_68_AC;
            custom.colors.background.main = 0x1B_22_33;
            crate::theme::presets::write_theme_to_disk(&custom).unwrap();

            assert!(crate::config::load_user_preferences().theme.preset_id.is_none());
            let restarted = try_load_theme_with_appearance(|| true).unwrap();
            assert_eq!(restarted.colors.accent.selected, custom.colors.accent.selected);
            assert_eq!(restarted.colors.background.main, custom.colors.background.main);
            assert!(!root.join(".theme-save-transaction.json").exists());
        });
    }

    #[test]
    fn failed_custom_save_preserves_saved_theme_and_malformed_reload_refuses() {
        with_theme_test_workspace(|root| {
            let _theme_guard = lock_theme_cache_test();
            let mut saved = Theme::dark_default();
            saved.colors.accent.selected = 0x35_79_BD;
            crate::theme::presets::write_theme_to_disk(&saved).unwrap();
            let publication = get_theme_snapshot();

            std::fs::write(root.join("config.ts"), "export default {").unwrap();
            let mut attempted = saved.clone();
            attempted.colors.accent.selected = 0xAC_68_24;
            assert!(crate::theme::presets::write_theme_to_disk(&attempted).is_err());
            let restarted = try_load_theme_with_appearance(|| true).unwrap();
            assert_eq!(restarted.colors.accent.selected, saved.colors.accent.selected);
            assert!(Arc::ptr_eq(&publication, &get_theme_snapshot()));

            std::fs::write(root.join("theme.json"), "{").unwrap();
            assert!(try_load_theme_with_appearance(|| true).is_err());
            assert!(Arc::ptr_eq(&publication, &get_theme_snapshot()));
        });
    }

    fn poison_theme_cache_with(theme: Theme) {
        let cache = &*THEME_CACHE;
        let _ = std::thread::spawn(move || {
            let mut guard = cache.lock().expect("theme cache lock should succeed");
            guard.snapshot = Arc::new(super::super::live_edit::PublishedTheme {
                revision: guard.snapshot.revision + 1,
                resolved: Arc::new(super::super::live_edit::ResolvedLiveTheme::from_theme(&theme)),
                theme: Arc::new(theme),
            });
            panic!("intentional poison for theme cache recovery test");
        })
        .join();
        assert!(cache.is_poisoned(), "theme cache lock should be poisoned");
    }

    fn clear_theme_cache_poison_and_restore() {
        let cache = &*THEME_CACHE;
        cache.clear_poison();
        let mut guard = cache
            .lock()
            .expect("theme cache lock should be healthy after clear_poison");
        *guard = ThemeCache::default();
    }


    #[test]
    fn test_default_text_opacity_ladder_is_liquid_glass_quiet() {
        for opacity in [
            BackgroundOpacity::dark_default(),
            BackgroundOpacity::light_default(),
        ] {
            assert_eq!(opacity.text_name, TEXT_NAME_OPACITY);
            assert_eq!(opacity.text_strong, TEXT_STRONG_OPACITY);
            assert_eq!(opacity.text_muted_alpha, TEXT_MUTED_OPACITY);
            assert_eq!(opacity.text_hint, TEXT_HINT_OPACITY);
            assert_eq!(opacity.text_placeholder, TEXT_PLACEHOLDER_OPACITY);
            assert_eq!(opacity.text_icon, TEXT_ICON_OPACITY);
            assert!(opacity.text_strong > opacity.text_muted_alpha);
            assert!(opacity.text_muted_alpha > opacity.text_hint);
            assert!(opacity.text_hint > opacity.text_placeholder);
            assert!(opacity.text_icon < opacity.text_strong);
            assert_eq!(opacity_to_alpha(opacity.text_muted_alpha), 0xA5);
            assert_eq!(opacity_to_alpha(opacity.text_hint), 0x72);
            assert_eq!(opacity_to_alpha(opacity.text_placeholder), 0x66);
        }
    }

    fn preferences_with_preset(preset_id: Option<&str>) -> ScriptKitUserPreferences {
        ScriptKitUserPreferences {
            layout: LayoutConfig::default(),
            theme: ThemeSelectionPreferences {
                preset_id: preset_id.map(ToString::to_string),
            },
            dictation: Default::default(),
            ai: Default::default(),
            window_management: Default::default(),
            effects: Default::default(),
        }
    }

    fn focus_scheme_from_theme(theme: &Theme, cursor: Option<CursorStyle>) -> FocusColorScheme {
        FocusColorScheme {
            background: theme.colors.background.clone(),
            text: theme.colors.text.clone(),
            accent: theme.colors.accent.clone(),
            ui: theme.colors.ui.clone(),
            cursor,
            terminal: theme.colors.terminal.clone(),
        }
    }

    #[test]
    fn test_theme_from_user_preferences_loads_matching_preset() {
        let preferences = preferences_with_preset(Some("nord"));

        let from_preferences =
            theme_from_user_preferences(&preferences, "test-correlation").expect("theme expected");
        let expected = crate::theme::presets::all_presets()
            .into_iter()
            .find(|preset| preset.id == "nord")
            .expect("preset should exist")
            .create_theme();

        assert_eq!(
            from_preferences.colors.background.main,
            expected.colors.background.main
        );
        assert_eq!(
            from_preferences.colors.accent.selected,
            expected.colors.accent.selected
        );
    }

    #[test]
    fn test_theme_from_user_preferences_returns_none_for_unknown_preset() {
        let preferences = preferences_with_preset(Some("unknown-preset-id"));
        assert!(theme_from_user_preferences(&preferences, "test-correlation").is_none());
    }

    #[test]
    fn test_theme_from_user_preferences_returns_none_when_preset_unset() {
        let preferences = preferences_with_preset(None);
        assert!(theme_from_user_preferences(&preferences, "test-correlation").is_none());
    }

    #[test]
    fn test_get_cached_theme_recovers_from_poisoned_mutex_without_defaulting() {
        let _guard = lock_theme_cache_test();
        let mut custom_theme = Theme::light_default();
        custom_theme.colors.background.main = 0x12_34_56;

        poison_theme_cache_with(custom_theme.clone());
        let cached_theme = get_cached_theme();

        assert_eq!(
            cached_theme.colors.background.main,
            custom_theme.colors.background.main
        );
        assert_ne!(
            cached_theme.colors.background.main,
            Theme::dark_default().colors.background.main
        );

        clear_theme_cache_poison_and_restore();
    }

    #[test]
    fn atomic_publication_recovers_poison_and_rejects_stale_before_bridge_mutation() {
        let _guard = lock_theme_cache_test();
        poison_theme_cache_with(Theme::dark_default());
        let baseline = get_theme_snapshot();
        let prepared = super::super::live_edit::prepare_theme(Theme::light_default()).unwrap();
        let mut bridge_calls = 0;
        let publication = commit_prepared_theme(baseline.revision, prepared, |_| bridge_calls += 1).unwrap();
        assert_eq!(bridge_calls, 1);
        assert_eq!(publication.revision, baseline.revision + 1);
        assert_eq!(get_theme_snapshot().theme.colors.background.main, Theme::light_default().colors.background.main);
        let prepared = super::super::live_edit::prepare_theme(Theme::dark_default()).unwrap();
        assert!(matches!(commit_prepared_theme(baseline.revision, prepared, |_| bridge_calls += 1),
            Err(super::super::service::ThemePublishError::StaleRevision { .. })));
        assert_eq!(bridge_calls, 1);
        assert!(Arc::ptr_eq(&publication, &get_theme_snapshot()));
        assert_eq!(baseline.theme.colors.background.main, Theme::dark_default().colors.background.main);
        clear_theme_cache_poison_and_restore();
    }

    #[test]
    fn malformed_reload_decoding_never_changes_the_publication() {
        let _guard = lock_theme_cache_test();
        let baseline = get_theme_snapshot();
        for invalid in ["{", "[]", r##"{"colors":{"background":{"main":"bad-color"}}}"##] {
            assert!(decode_theme_json(invalid, true).is_err());
            assert!(Arc::ptr_eq(&baseline, &get_theme_snapshot()));
        }
        let decoded = decode_theme_json(r##"{"appearance":"light","opacity":{"hover":-0.1}}"##, true).unwrap();
        assert_eq!(decoded.get_opacity().hover, 0.0);
        assert!(matches!(decoded.appearance, AppearanceMode::Light));
    }

    #[test]
    fn test_normalize_theme_primary_text_uses_pure_black_or_white_for_main_and_focus_aware() {
        let mut theme = Theme::light_default();
        theme.colors.background.main = 0xF8FAFC;
        theme.colors.text.primary = 0x223344;
        theme.focus_aware = Some(FocusAwareColorScheme {
            focused: Some(FocusColorScheme {
                background: BackgroundColors {
                    main: 0x121212,
                    ..theme.colors.background.clone()
                },
                text: TextColors {
                    primary: 0x654321,
                    ..theme.colors.text.clone()
                },
                accent: theme.colors.accent.clone(),
                ui: theme.colors.ui.clone(),
                cursor: None,
                terminal: theme.colors.terminal.clone(),
            }),
            unfocused: Some(FocusColorScheme {
                background: BackgroundColors {
                    main: 0xF5F5F5,
                    ..theme.colors.background.clone()
                },
                text: TextColors {
                    primary: 0x123456,
                    ..theme.colors.text.clone()
                },
                accent: theme.colors.accent.clone(),
                ui: theme.colors.ui.clone(),
                cursor: None,
                terminal: theme.colors.terminal.clone(),
            }),
        });

        let normalized = normalize_theme_primary_text(theme);
        let focus_aware = normalized.focus_aware.expect("focus aware colors expected");

        assert_eq!(normalized.colors.text.primary, 0x000000);
        assert_eq!(
            focus_aware
                .focused
                .expect("focused scheme expected")
                .text
                .primary,
            0xFFFFFF
        );
        assert_eq!(
            focus_aware
                .unfocused
                .expect("unfocused scheme expected")
                .text
                .primary,
            0x000000
        );
    }

    #[test]
    fn test_merge_json_preserves_user_light_colors_when_overlaying_defaults() {
        let mut base = serde_json::to_value(Theme::light_default()).expect("serialize theme");
        let overlay = serde_json::json!({
            "appearance": "light",
            "colors": {
                "background": {
                    "main": 1193046
                }
            }
        });

        merge_json(&mut base, overlay);
        let merged_theme: Theme = serde_json::from_value(base).expect("deserialize merged theme");

        assert_eq!(merged_theme.colors.background.main, 1_193_046);
        assert_eq!(
            merged_theme.colors.background.title_bar,
            ColorScheme::light_default().background.title_bar
        );
    }

    #[test]
    fn test_merge_json_replaces_non_object_values_when_overlay_is_leaf() {
        let mut base = serde_json::json!({
            "opacity": {
                "main": 0.85,
                "title_bar": 0.85
            }
        });

        merge_json(&mut base, serde_json::json!({ "opacity": null }));

        assert_eq!(base["opacity"], serde_json::Value::Null);
    }

    #[test]
    fn test_set_requested_appearance_on_theme_json_overrides_default_after_merge_when_user_omits_appearance(
    ) {
        let mut merged_theme_json =
            serde_json::to_value(Theme::light_default()).expect("serialize light default theme");

        merge_json(
            &mut merged_theme_json,
            serde_json::json!({
                "colors": {
                    "background": {
                        "main": 0x12_34_56
                    }
                }
            }),
        );

        set_requested_appearance_on_theme_json(&mut merged_theme_json, AppearanceMode::Auto);

        let merged_theme: Theme =
            serde_json::from_value(merged_theme_json).expect("deserialize merged theme");
        assert_eq!(merged_theme.appearance, AppearanceMode::Auto);
        assert_eq!(merged_theme.colors.background.main, 0x12_34_56);
    }

    #[test]
    fn test_get_opacity_uses_appearance_aware_defaults_when_opacity_missing() {
        let mut light_theme = Theme::light_default();
        light_theme.opacity = None;
        let light_opacity = light_theme.get_opacity();
        assert_eq!(light_opacity.main, BackgroundOpacity::light_default().main);
        assert!(
            light_opacity.hover < light_opacity.selected,
            "light theme hover should remain quieter than focused selection"
        );

        let mut dark_theme = Theme::dark_default();
        dark_theme.opacity = None;
        let dark_opacity = dark_theme.get_opacity();
        assert_eq!(dark_opacity.main, BackgroundOpacity::dark_default().main);
        assert!(
            dark_opacity.hover < dark_opacity.selected,
            "dark theme hover should remain quieter than focused selection"
        );
    }

    #[test]
    fn test_light_default_row_state_opacity_uses_reduced_light_ladder() {
        let light_opacity = BackgroundOpacity::light_default();
        let dark_opacity = BackgroundOpacity::dark_default();

        assert_eq!(light_opacity.selected, LIGHT_ROW_SELECTED_OPACITY);
        assert_eq!(light_opacity.hover, LIGHT_ROW_HOVER_OPACITY);
        assert!(
            light_opacity.hover < light_opacity.selected,
            "light theme hover should stay below focused selection"
        );
        assert!(
            light_opacity.selected < dark_opacity.selected,
            "light theme focus should be quieter than dark focus"
        );
        assert!(
            light_opacity.hover < dark_opacity.hover,
            "light theme hover should be quieter than dark hover"
        );
    }

    #[test]
    fn test_background_opacity_clamped_clamps_all_fields_when_values_out_of_range() {
        let clamped = BackgroundOpacity {
            main: -0.1,
            title_bar: 1.1,
            search_box: 0.5,
            log_panel: -3.0,
            selected: 4.0,
            hover: -0.2,
            preview: 0.2,
            dialog: 2.0,
            input: -1.0,
            panel: 0.7,
            input_inactive: 1.2,
            input_active: -0.4,
            border_inactive: 0.3,
            border_active: 1.9,
            vibrancy_background: Some(-0.3),
            ..BackgroundOpacity::dark_default()
        }
        .clamped();

        assert_eq!(clamped.main, 0.0);
        assert_eq!(clamped.title_bar, 1.0);
        assert_eq!(clamped.search_box, 0.5);
        assert_eq!(clamped.log_panel, 0.0);
        assert_eq!(clamped.selected, 1.0);
        assert_eq!(clamped.hover, 0.0);
        assert_eq!(clamped.preview, 0.2);
        assert_eq!(clamped.dialog, 1.0);
        assert_eq!(clamped.input, 0.0);
        assert_eq!(clamped.panel, 0.7);
        assert_eq!(clamped.input_inactive, 1.0);
        assert_eq!(clamped.input_active, 0.0);
        assert_eq!(clamped.border_inactive, 0.3);
        assert_eq!(clamped.border_active, 1.0);
        assert_eq!(clamped.vibrancy_background, Some(0.0));
    }

    #[test]
    fn test_background_gradient_defaults_to_none_and_clamps_when_enabled() {
        assert!(Theme::dark_default().active_background_gradient().is_none());

        let mut theme = Theme::dark_default();
        theme.background_gradient = Some(BackgroundGradient {
            enabled: true,
            from: 0x111111,
            to: 0x222222,
            angle: 725.0,
            opacity: 2.0,
            layers: Vec::new(),
        });

        let gradient = theme
            .active_background_gradient()
            .expect("enabled gradient should be active");
        assert_eq!(gradient.angle, 5.0);
        assert_eq!(gradient.opacity, 1.0);
    }

    #[test]
    fn test_get_opacity_clamps_configured_values_before_returning() {
        let mut theme = Theme::dark_default();
        theme.opacity = Some(BackgroundOpacity {
            main: 2.0,
            title_bar: -0.5,
            search_box: 0.4,
            log_panel: 0.3,
            selected: 0.2,
            hover: 0.1,
            preview: 0.0,
            dialog: 0.9,
            input: 0.8,
            panel: 0.7,
            input_inactive: 0.6,
            input_active: 0.5,
            border_inactive: -0.1,
            border_active: 3.0,
            vibrancy_background: Some(1.4),
            ..BackgroundOpacity::dark_default()
        });

        let opacity = theme.get_opacity();

        assert_eq!(opacity.main, 1.0);
        assert_eq!(opacity.title_bar, 0.0);
        assert_eq!(opacity.search_box, 0.4);
        assert_eq!(opacity.border_inactive, 0.0);
        assert_eq!(opacity.border_active, 1.0);
        assert_eq!(opacity.vibrancy_background, Some(1.0));
    }

    #[test]
    fn test_get_cursor_style_returns_default_focused_when_focus_aware_cursor_is_omitted() {
        let mut theme = Theme::dark_default();
        theme.focus_aware = Some(FocusAwareColorScheme {
            focused: Some(focus_scheme_from_theme(&theme, None)),
            unfocused: None,
        });

        let cursor = theme
            .get_cursor_style(true)
            .expect("focused cursor should be present");
        let expected = CursorStyle::default_focused();

        assert_eq!(cursor.color, expected.color);
        assert_eq!(cursor.blink_interval_ms, expected.blink_interval_ms);
    }

    #[test]
    fn test_get_cursor_style_returns_configured_cursor_when_focus_aware_cursor_is_present() {
        let mut theme = Theme::dark_default();
        let configured_cursor = CursorStyle {
            color: 0x12_34_56,
            blink_interval_ms: 321,
        };
        theme.focus_aware = Some(FocusAwareColorScheme {
            focused: Some(focus_scheme_from_theme(
                &theme,
                Some(configured_cursor.clone()),
            )),
            unfocused: None,
        });

        let cursor = theme
            .get_cursor_style(true)
            .expect("focused cursor should be present");

        assert_eq!(cursor.color, configured_cursor.color);
        assert_eq!(
            cursor.blink_interval_ms,
            configured_cursor.blink_interval_ms
        );
    }

    #[test]
    fn test_get_cursor_style_returns_none_when_window_is_not_focused_even_with_focus_aware_cursor()
    {
        let mut theme = Theme::dark_default();
        theme.focus_aware = Some(FocusAwareColorScheme {
            focused: Some(focus_scheme_from_theme(
                &theme,
                Some(CursorStyle {
                    color: 0x65_43_21,
                    blink_interval_ms: 250,
                }),
            )),
            unfocused: None,
        });

        assert!(theme.get_cursor_style(false).is_none());
    }

    #[test]
    fn test_get_drop_shadow_clamps_opacity_when_out_of_range() {
        let mut theme = Theme::dark_default();
        theme.drop_shadow = Some(DropShadow {
            opacity: 1.7,
            ..DropShadow::default()
        });

        let shadow = theme.get_drop_shadow();
        assert_eq!(shadow.opacity, 1.0);
    }

    #[test]
    fn test_get_drop_shadow_clamps_negative_blur_and_spread_to_zero() {
        let mut theme = Theme::dark_default();
        theme.drop_shadow = Some(DropShadow {
            blur_radius: -4.0,
            spread_radius: -2.5,
            ..DropShadow::default()
        });

        let shadow = theme.get_drop_shadow();
        assert_eq!(shadow.blur_radius, 0.0);
        assert_eq!(shadow.spread_radius, 0.0);
    }

    #[test]
    fn test_get_drop_shadow_preserves_valid_values() {
        let mut theme = Theme::dark_default();
        let configured = DropShadow {
            enabled: false,
            blur_radius: 12.0,
            spread_radius: 3.0,
            offset_x: 6.0,
            offset_y: 4.0,
            color: 0x11_22_33,
            opacity: 0.45,
        };
        theme.drop_shadow = Some(configured.clone());

        let shadow = theme.get_drop_shadow();

        assert_eq!(shadow.enabled, configured.enabled);
        assert_eq!(shadow.blur_radius, configured.blur_radius);
        assert_eq!(shadow.spread_radius, configured.spread_radius);
        assert_eq!(shadow.offset_x, configured.offset_x);
        assert_eq!(shadow.offset_y, configured.offset_y);
        assert_eq!(shadow.color, configured.color);
        assert_eq!(shadow.opacity, configured.opacity);
    }

    #[test]
    fn test_get_drop_shadow_allows_negative_offsets() {
        let mut theme = Theme::dark_default();
        theme.drop_shadow = Some(DropShadow {
            offset_x: -5.0,
            offset_y: -8.0,
            ..DropShadow::default()
        });

        let shadow = theme.get_drop_shadow();
        assert_eq!(shadow.offset_x, -5.0);
        assert_eq!(shadow.offset_y, -8.0);
    }

    #[test]
    fn test_hydrate_terminal_colors_for_deserialize_sets_light_palette_for_focus_aware_when_light_mode(
    ) {
        let mut merged_theme_json =
            serde_json::to_value(Theme::light_default()).expect("serialize light default theme");

        let mut focused_json =
            serde_json::to_value(ColorScheme::light_default()).expect("serialize color scheme");
        focused_json
            .as_object_mut()
            .expect("color scheme must be object")
            .remove("terminal");

        merge_json(
            &mut merged_theme_json,
            serde_json::json!({
                "appearance": "light",
                "focus_aware": {
                    "focused": focused_json
                }
            }),
        );

        hydrate_terminal_colors_for_deserialize(&mut merged_theme_json, true);

        let merged_theme: Theme =
            serde_json::from_value(merged_theme_json).expect("deserialize hydrated theme");
        let focused_terminal = merged_theme
            .focus_aware
            .expect("focus aware colors expected")
            .focused
            .expect("focused colors expected")
            .terminal;
        let light_defaults = TerminalColors::light_default();

        assert_eq!(focused_terminal.blue, light_defaults.blue);
        assert_eq!(focused_terminal.bright_white, light_defaults.bright_white);
    }

    #[test]
    fn test_hydrate_terminal_colors_for_deserialize_preserves_override_when_auto_mode_is_dark() {
        let mut merged_theme_json = serde_json::json!({
            "appearance": "auto",
            "colors": {
                "terminal": {
                    "red": 1122867
                }
            }
        });

        hydrate_terminal_colors_for_deserialize(&mut merged_theme_json, true);

        let hydrated_terminal: TerminalColors =
            serde_json::from_value(merged_theme_json["colors"]["terminal"].clone())
                .expect("deserialize hydrated terminal");
        let dark_defaults = TerminalColors::dark_default();

        assert_eq!(hydrated_terminal.red, 1_122_867);
        assert_eq!(hydrated_terminal.blue, dark_defaults.blue);
        assert_eq!(hydrated_terminal.bright_white, dark_defaults.bright_white);
    }

    #[test]
    fn test_terminal_colors_serde_defaults_use_light_palette_when_light_hint_is_set() {
        let terminal = with_terminal_default_palette_hint(TerminalDefaultPalette::Light, || {
            serde_json::from_value::<TerminalColors>(serde_json::json!({ "red": 0x11_22_33 }))
                .expect("deserialize terminal with light defaults")
        });
        let light_defaults = TerminalColors::light_default();

        assert_eq!(terminal.red, 0x11_22_33);
        assert_eq!(terminal.green, light_defaults.green);
        assert_eq!(terminal.bright_white, light_defaults.bright_white);
    }

    #[test]
    fn test_terminal_colors_serde_defaults_use_dark_palette_when_dark_hint_is_set() {
        let terminal = with_terminal_default_palette_hint(TerminalDefaultPalette::Dark, || {
            serde_json::from_value::<TerminalColors>(serde_json::json!({ "red": 0x22_33_44 }))
                .expect("deserialize terminal with dark defaults")
        });
        let dark_defaults = TerminalColors::dark_default();

        assert_eq!(terminal.red, 0x22_33_44);
        assert_eq!(terminal.green, dark_defaults.green);
        assert_eq!(terminal.bright_white, dark_defaults.bright_white);
    }

    #[test]
    fn test_hydrate_terminal_colors_for_deserialize_uses_light_palette_for_focus_aware_partial_terminal(
    ) {
        let overridden_red = 0x11_22_33;
        let mut merged_theme_json =
            serde_json::to_value(Theme::light_default()).expect("serialize light default theme");

        let mut focused_json =
            serde_json::to_value(ColorScheme::light_default()).expect("serialize color scheme");
        focused_json
            .as_object_mut()
            .expect("color scheme must be object")
            .insert(
                "terminal".to_string(),
                serde_json::json!({ "red": overridden_red }),
            );

        merge_json(
            &mut merged_theme_json,
            serde_json::json!({
                "appearance": "light",
                "focus_aware": {
                    "focused": focused_json
                }
            }),
        );
        hydrate_terminal_colors_for_deserialize(&mut merged_theme_json, true);

        let merged_theme: Theme =
            serde_json::from_value(merged_theme_json).expect("deserialize hydrated theme");
        let focused_terminal = merged_theme
            .focus_aware
            .expect("focus aware colors expected")
            .focused
            .expect("focused colors expected")
            .terminal;

        assert_eq!(focused_terminal.red, overridden_red);
        assert_eq!(focused_terminal.green, 0x00bc00);
        assert_ne!(focused_terminal.green, TerminalColors::dark_default().green);
    }

    #[test]
    fn test_hydrate_terminal_colors_for_deserialize_uses_dark_palette_for_focus_aware_partial_terminal(
    ) {
        let overridden_red = 0x33_22_11;
        let mut merged_theme_json =
            serde_json::to_value(Theme::dark_default()).expect("serialize dark default theme");

        let mut focused_json =
            serde_json::to_value(ColorScheme::dark_default()).expect("serialize color scheme");
        focused_json
            .as_object_mut()
            .expect("color scheme must be object")
            .insert(
                "terminal".to_string(),
                serde_json::json!({ "red": overridden_red }),
            );

        merge_json(
            &mut merged_theme_json,
            serde_json::json!({
                "appearance": "dark",
                "focus_aware": {
                    "focused": focused_json
                }
            }),
        );
        hydrate_terminal_colors_for_deserialize(&mut merged_theme_json, false);

        let merged_theme: Theme =
            serde_json::from_value(merged_theme_json).expect("deserialize hydrated theme");
        let focused_terminal = merged_theme
            .focus_aware
            .expect("focus aware colors expected")
            .focused
            .expect("focused colors expected")
            .terminal;

        assert_eq!(focused_terminal.red, overridden_red);
        assert_eq!(focused_terminal.green, 0x50fa7b);
        assert_ne!(
            focused_terminal.green,
            TerminalColors::light_default().green
        );
    }

    #[test]
    fn test_dark_default_uses_dark_on_accent_text() {
        let dark_theme = Theme::dark_default();
        // Dark text on bright yellow (#FBBF24) accent for WCAG contrast
        assert_eq!(dark_theme.colors.text.on_accent, 0x1e1e1e);
    }

    #[test]
    fn test_to_unfocused_does_not_brighten_dark_backgrounds() {
        let dark_scheme = ColorScheme::dark_default();
        let unfocused = dark_scheme.to_unfocused();

        let original_luminance = relative_luminance_srgb(dark_scheme.background.main);
        let unfocused_luminance = relative_luminance_srgb(unfocused.background.main);

        assert!(
            unfocused_luminance <= original_luminance,
            "expected unfocused dark background luminance ({unfocused_luminance}) to be <= original ({original_luminance})",
        );
    }

    #[test]
    fn test_to_unfocused_light_theme_lightens_primary_text() {
        let light_scheme = ColorScheme::light_default();
        let unfocused = light_scheme.to_unfocused();

        let original_luminance = relative_luminance_srgb(light_scheme.text.primary);
        let unfocused_luminance = relative_luminance_srgb(unfocused.text.primary);

        assert!(
            unfocused_luminance >= original_luminance,
            "expected unfocused light theme text luminance ({unfocused_luminance}) to be >= original ({original_luminance})",
        );
    }
}
