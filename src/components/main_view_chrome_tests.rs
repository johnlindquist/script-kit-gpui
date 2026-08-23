#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, InteractiveElement as _, IntoElement as _, Styled as _};

    use super::{
        main_view_content_frame, main_view_flow_spacing, main_view_header_metrics,
        main_view_multiline_input_height, resolved_main_view_input_height,
        resolved_main_view_main_bottom_inset, selection_hint_snippet,
    };

    #[test]
    fn prompt_search_modes_resolve_main_menu_search_geometry() {
        let search = crate::designs::MainMenuThemeVariant::default().def().search;
        let expected = super::PromptSearchInputGeometry::from_main_menu(search);
        let cases = [
            ("ArgPrompt", super::PromptSearchInputKind::EntityBacked),
            ("MiniPrompt", super::PromptSearchInputKind::EntityBacked),
            (
                "SelectPrompt",
                super::PromptSearchInputKind::ControllerOwned,
            ),
            ("PathPrompt", super::PromptSearchInputKind::PathPrefix),
        ];

        for (surface, kind) in cases {
            assert_eq!(
                super::prompt_search_input_geometry(search, kind),
                expected,
                "{surface} must resolve height/font/radius/text inset from MainMenuSearchTokens"
            );
        }
    }

    #[test]
    fn canonical_and_context_only_headers_share_one_theme_derived_geometry_model() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let canonical = main_view_header_metrics(def, Some(def.search.height));
        let context_only = main_view_header_metrics(def, None);

        assert_eq!(canonical.header_height, 58.0);
        assert_eq!(canonical.context_x, -6.0);
        assert_eq!(canonical.context_y, 4.0);
        assert_eq!(canonical.context_height, 22.0);
        assert_eq!(canonical.input_x, 2.0);
        assert_eq!(canonical.input_y, 28.0);
        assert_eq!(canonical.input_height, Some(26.0));
        assert_eq!(context_only.header_height, 30.0);
        assert_eq!(context_only.input_height, None);
    }

    #[test]
    fn footer_flush_list_chrome_has_exactly_one_footer_reservation() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        assert!(def.shell.content_inset_bottom > 0.0);
        assert_eq!(resolved_main_view_main_bottom_inset(def, false), 0.0);
        assert_eq!(
            resolved_main_view_main_bottom_inset(def, true),
            def.shell.content_inset_bottom
        );
    }

    #[test]
    fn multiline_input_keeps_the_main_menu_height_until_a_second_line_is_visible() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let line_height = def.search.height;

        assert_eq!(
            main_view_multiline_input_height(def.search.height, line_height, 0),
            def.search.height
        );
        assert_eq!(
            main_view_multiline_input_height(def.search.height, line_height, 1),
            def.search.height
        );
        assert_eq!(
            main_view_multiline_input_height(def.search.height, line_height, 3),
            def.search.height + line_height * 2.0
        );
    }

    #[test]
    fn input_horizontal_metrics_share_header_and_search_insets() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let metrics = super::main_view_input_horizontal_metrics(def, 480.0);
        assert_eq!(metrics.shell_x, def.shell.header_padding_x);
        assert_eq!(
            metrics.shell_width,
            480.0 - def.shell.header_padding_x * 2.0
        );
        assert_eq!(metrics.text_inset_left, def.search.text_inset_x);
        assert_eq!(metrics.text_inset_right, def.search.text_inset_x * 0.5);
        assert_eq!(
            metrics.text_width_after_trailing(24.0),
            metrics.shell_width - metrics.text_inset_left - metrics.text_inset_right - 24.0
        );
    }
    #[test]
    fn input_shell_accepts_taller_surface_height_without_shrinking_theme_default() {
        assert_eq!(resolved_main_view_input_height(26.0, None), 26.0);
        assert_eq!(resolved_main_view_input_height(26.0, Some(152.0)), 152.0);
        assert_eq!(resolved_main_view_input_height(26.0, Some(18.0)), 26.0);
    }

    #[test]
    fn content_frame_declares_one_container_edge_and_row_aligned_text_plane() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let spacing = crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing();
        let frame = main_view_content_frame(def, spacing);
        assert_eq!(frame.container_edge_x, def.shell.content_inset_x);
        assert_eq!(
            frame.text_plane_x,
            frame.container_edge_x + super::main_view_text_column_x(def)
        );
        assert_eq!(frame.text_inset_x(), super::main_view_text_column_x(def));
    }

    #[test]
    fn snippet_collapses_whitespace_and_truncates_at_char_boundary() {
        assert_eq!(
            selection_hint_snippet("hello   world\n\tnext", 24),
            "hello world next"
        );
        assert_eq!(
            selection_hint_snippet("the quick brown fox jumps over the lazy dog", 15),
            "the quick brown\u{2026}"
        );
        // Multi-byte chars must not split; count is in chars, not bytes.
        assert_eq!(
            selection_hint_snippet("héllö wörld ünïcödé", 7),
            "héllö w\u{2026}"
        );
    }

    #[test]
    fn snippet_short_text_passes_through_unchanged() {
        assert_eq!(selection_hint_snippet("short", 24), "short");
        assert_eq!(selection_hint_snippet("  padded  ", 24), "padded");
    }

    #[test]
    fn semantic_chip_role_action_matrix_rejects_cross_role_behavior() {
        use super::{SemanticChipAction as Action, SemanticChipRole as Role, SemanticChipSpec};
        let actions = [
            Action::OpenDetails,
            Action::RemoveContext,
            Action::OpenSelector,
            Action::OpenSurface,
            Action::SelectDestination,
        ];
        let body_allowed = |role, action| match role {
            Role::ContextAttachment => action == Action::OpenDetails,
            Role::Identity => matches!(
                action,
                Action::OpenDetails | Action::OpenSelector | Action::OpenSurface
            ),
            Role::DestinationSelector => action == Action::SelectDestination,
        };

        for role in [
            Role::ContextAttachment,
            Role::Identity,
            Role::DestinationSelector,
        ] {
            for action in actions {
                let result = SemanticChipSpec::try_new(
                    format!("chip-{role:?}-{action:?}"),
                    role,
                    "Chip",
                    Vec::new(),
                    true,
                    None,
                    Some(action),
                    None,
                );
                assert_eq!(
                    result.is_ok(),
                    body_allowed(role, action),
                    "{role:?}/{action:?} body action"
                );
            }
            for action in actions {
                let result = SemanticChipSpec::try_new(
                    format!("trailing-{role:?}-{action:?}"),
                    role,
                    "Chip",
                    Vec::new(),
                    true,
                    None,
                    None,
                    Some(action),
                );
                assert_eq!(
                    result.is_ok(),
                    role == Role::ContextAttachment && action == Action::RemoveContext,
                    "{role:?}/{action:?} trailing action"
                );
            }
        }
    }

    #[test]
    fn invalid_semantic_chip_constructors_become_inert_without_panicking() {
        use super::{SemanticChipAction, SemanticChipSpec};

        let chip =
            SemanticChipSpec::enabled_identity("", "", SemanticChipAction::RemoveContext, "⇥");
        assert_eq!(chip.semantic_id.as_ref(), "invalid-identity-chip");
        assert_eq!(chip.label.as_ref(), "Unavailable");
        assert!(!chip.enabled);
        assert!(chip.disabled_reason.is_some());
        assert!(chip.shortcut_tokens.is_empty());
        assert!(chip.body_action.is_none());
        assert!(chip.trailing_action.is_none());
    }

    #[test]
    fn semantic_chip_disabled_and_shortcut_contract_fails_closed() {
        use super::{SemanticChipAction, SemanticChipRole, SemanticChipSpec};
        assert!(SemanticChipSpec::try_new(
            "disabled",
            SemanticChipRole::Identity,
            "No cwd",
            Vec::new(),
            false,
            None,
            None,
            None,
        )
        .is_err());
        assert!(SemanticChipSpec::try_new(
            "disabled",
            SemanticChipRole::Identity,
            "No cwd",
            vec!["⇥".to_string()],
            false,
            Some("Unavailable".into()),
            None,
            None,
        )
        .is_err());
        assert!(SemanticChipSpec::try_new(
            "disabled",
            SemanticChipRole::Identity,
            "No cwd",
            Vec::new(),
            false,
            Some("Unavailable".into()),
            Some(SemanticChipAction::OpenSelector),
            None,
        )
        .is_err());
        assert!(SemanticChipSpec::try_new(
            "orientation",
            SemanticChipRole::Identity,
            "Project",
            vec!["⇥".to_string()],
            true,
            None,
            None,
            None,
        )
        .is_err());
    }

    #[test]
    fn main_context_zone_requires_unique_role_correct_ids() {
        use super::{
            MainViewContextZoneSpec, SemanticChipAction, SemanticChipSpec,
            MAIN_VIEW_CONTEXT_CWD_BUTTON_ID, MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID,
            MAIN_VIEW_CONTEXT_QUICK_AI_BUTTON_ID, MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID,
        };
        let cwd = SemanticChipSpec::enabled_identity(
            MAIN_VIEW_CONTEXT_CWD_BUTTON_ID,
            "Project",
            SemanticChipAction::OpenSelector,
            "⇥",
        );
        let model = SemanticChipSpec::enabled_identity(
            MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID,
            "Agent · Model",
            SemanticChipAction::OpenSelector,
            "⇧⇥",
        );
        let context = SemanticChipSpec::context_attachment(
            MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID,
            "Selected text",
            false,
        );
        let zone = MainViewContextZoneSpec::try_new(cwd.clone(), Some(context), model)
            .expect("valid main context zone");
        assert_eq!(zone.leading_identity.shortcut_tokens, vec!["⇥"]);
        assert_eq!(zone.trailing_identity.shortcut_tokens, vec!["⇧", "⇥"]);

        let quick = SemanticChipSpec::enabled_identity(
            MAIN_VIEW_CONTEXT_QUICK_AI_BUTTON_ID,
            "Quick AI",
            SemanticChipAction::OpenSurface,
            "⇥",
        );
        assert_ne!(quick.semantic_id, cwd.semantic_id);
        assert!(MainViewContextZoneSpec::try_new(
            cwd.clone(),
            None,
            SemanticChipSpec::enabled_identity(
                MAIN_VIEW_CONTEXT_CWD_BUTTON_ID,
                "Duplicate",
                SemanticChipAction::OpenSelector,
                "⇧⇥",
            ),
        )
        .is_err());
        assert!(MainViewContextZoneSpec::try_new(
            cwd,
            Some(SemanticChipSpec::context_attachment(
                MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID,
                "Removable",
                true,
            )),
            quick,
        )
        .is_err());
    }

    #[test]
    fn destination_selector_cannot_remove_context_or_open_identity_surfaces() {
        use super::{SemanticChipAction, SemanticChipRole, SemanticChipSpec};
        let destination = SemanticChipSpec::destination_selector("destination", "Paste");
        assert_eq!(
            destination.body_action,
            Some(SemanticChipAction::SelectDestination)
        );
        for action in [
            SemanticChipAction::RemoveContext,
            SemanticChipAction::OpenDetails,
            SemanticChipAction::OpenSelector,
            SemanticChipAction::OpenSurface,
        ] {
            assert!(SemanticChipSpec::try_new(
                "destination",
                SemanticChipRole::DestinationSelector,
                "Paste",
                Vec::new(),
                true,
                None,
                Some(action),
                None,
            )
            .is_err());
        }
    }

    #[test]
    fn flow_spacing_uses_balanced_shell_inset_and_safe_vertical_tokens() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let design_spacing = crate::designs::DesignSpacing::default();
        let flow = main_view_flow_spacing(def, design_spacing);

        assert_eq!(flow.inset_x, def.shell.content_inset_x);
        assert_eq!(flow.inset_y, design_spacing.padding_sm);
        assert_eq!(flow.section_gap, design_spacing.gap_lg);
        assert!(flow.inset_x > 0.0);
        assert!(flow.inset_y > 0.0);
        assert!(flow.section_gap > flow.inset_y);
    }

    struct TestMainViewOverlay;

    impl gpui::Render for TestMainViewOverlay {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let def = crate::designs::MainMenuThemeVariant::default().def();
            let theme = crate::theme::Theme::default();

            let chrome = super::MainViewChrome {
                header: super::MainViewHeaderChrome::canonical(
                    def,
                    gpui::div().into_any_element(),
                    gpui::div().into_any_element(),
                ),
                divider: super::MainViewDividerChrome {
                    margin_x: 0.0,
                    height: 1.0,
                    visible: true,
                },
                main: gpui::div()
                    .id("test-main-content")
                    .debug_selector(|| "test-main-content".to_string())
                    .size_full()
                    .into_any_element(),
                footer: None,
                overlays: vec![],
            };

            let root = super::render_main_view_shell();
            super::render_main_view_chrome_header_overlay_footer_flush(root, &theme, def, chrome)
        }
    }

    #[gpui::test]
    fn test_main_view_overlay_main_slot_clip_bounds(cx: &mut gpui::TestAppContext) {
        use gpui::px;

        let def = crate::designs::MainMenuThemeVariant::default().def();
        let header_height = main_view_header_metrics(def, Some(def.search.height)).header_height;

        // Test at window height 600
        let window_600 = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(800.0), px(600.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestMainViewOverlay))
                .unwrap()
        });

        cx.run_until_parked();

        window_600
            .update(cx, |_, window, _| {
                let bounds_entries = window.debug_bounds_entries();

                let clip_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_OVERLAY_CLIP_ID)
                    .expect("clip div should be rendered");

                let main_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_MAIN_ID)
                    .expect("main view main slot should be rendered");

                assert_eq!(main_entry.bounds.size.height, px(600.0));
                assert_eq!(clip_entry.bounds.origin.y, px(header_height));
                assert_eq!(clip_entry.bounds.size.height, px(600.0 - header_height));
                assert_eq!(main_entry.clip_bounds.origin.y, px(header_height));
                assert_eq!(
                    main_entry.clip_bounds.size.height,
                    px(600.0 - header_height)
                );
            })
            .unwrap();

        // Test at window height 800 (validates responsive layout after resize/size change)
        let window_800 = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(800.0), px(800.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestMainViewOverlay))
                .unwrap()
        });

        cx.run_until_parked();

        window_800
            .update(cx, |_, window, _| {
                let bounds_entries = window.debug_bounds_entries();

                let clip_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_OVERLAY_CLIP_ID)
                    .expect("clip div should be rendered");

                let main_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_MAIN_ID)
                    .expect("main view main slot should be rendered");

                assert_eq!(main_entry.bounds.size.height, px(800.0));
                assert_eq!(clip_entry.bounds.origin.y, px(header_height));
                assert_eq!(clip_entry.bounds.size.height, px(800.0 - header_height));
                assert_eq!(main_entry.clip_bounds.origin.y, px(header_height));
                assert_eq!(
                    main_entry.clip_bounds.size.height,
                    px(800.0 - header_height)
                );
            })
            .unwrap();
    }
}
