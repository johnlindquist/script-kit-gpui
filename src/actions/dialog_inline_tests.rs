#[cfg(test)]
mod tests {
    use super::{
        action_has_routable_shortcut, action_shortcut_parity_report, action_subtitle_for_display,
        actions_dialog_fixed_shell_viewport_height, actions_dialog_revealed_scroll_top,
        actions_dialog_scrollbar_fade_duration, actions_dialog_scrollbar_fade_opacity,
        actions_dialog_scrollbar_viewport_height, clear_duplicate_action_shortcuts,
        displayed_action_keybinding_specs, first_selectable_index, is_destructive_action,
        last_selectable_index, matching_action_id_for_keystroke,
        matching_filtered_action_id_for_keystroke, resolve_visible_action_shortcut,
        resolved_actions_dialog_row_metrics, selectable_index_at_or_after,
        selectable_index_at_or_before, should_render_section_separator,
        visible_action_shortcut_bindings, ActionsDialog, ActionsDialogChromeAudit,
        ActionsDialogRuntimeAudit, GroupedActionItem, MainListDisplayedActionShortcut,
    };
    use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
    use crate::menu_syntax::{MenuSyntaxAction, MenuSyntaxActionKind};
    use crate::menu_syntax_actions::{PowerSyntaxActionSection, SectionMode};

    #[test]
    fn actions_marker_centers_in_compact_host_without_changing_list_item_content_geometry() {
        let popup = crate::designs::base_actions_popup_theme();
        let main = crate::designs::MainMenuThemeVariant::InfoBarBase.def();
        let baseline = crate::list_item::ListItemMetricsOverride::from_main_menu_def(main);
        let metrics = resolved_actions_dialog_row_metrics(&popup, main);
        let marker = crate::list_item::list_item_selection_marker_geometry(metrics, true)
            .expect("selected Actions row has a marker");

        assert_eq!(metrics.item_height, baseline.item_height);
        assert_eq!(metrics.row_inner_padding_x, baseline.row_inner_padding_x);
        assert_eq!(metrics.icon_text_gap, baseline.icon_text_gap);
        assert_eq!(metrics.accessory_gap, baseline.accessory_gap);
        assert_eq!(
            metrics.row_selected_marker_center_height,
            popup.list.row_height
        );
        assert_eq!(marker.top, (popup.list.row_height - marker.height) / 2.0);
    }

    #[test]
    fn selectable_index_helpers_skip_section_headers_directionally() {
        let rows = vec![
            GroupedActionItem::SectionHeader("One".to_string()),
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("Two".to_string()),
            GroupedActionItem::Item(1),
        ];

        assert_eq!(first_selectable_index(&rows), Some(1));
        assert_eq!(last_selectable_index(&rows), Some(3));
        assert_eq!(selectable_index_at_or_before(&rows, 2), Some(1));
        assert_eq!(selectable_index_at_or_after(&rows, 2), Some(3));
    }

    #[test]
    fn destructive_detection_matches_known_ids() {
        let remove_action = Action::new(
            "remove_alias",
            "Remove Alias",
            Some("Remove alias".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(is_destructive_action(&remove_action));

        let trash_action = Action::new(
            "move_to_trash",
            "Move to Trash",
            Some("Move item to Trash".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(is_destructive_action(&trash_action));
    }

    #[test]
    fn destructive_detection_matches_title_prefix_fallback() {
        let delete_action = Action::new(
            "custom_action",
            "Delete Export Cache",
            Some("Delete cached export".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(is_destructive_action(&delete_action));

        let safe_action = Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy path".to_string()),
            ActionCategory::ScriptContext,
        );
        assert!(!is_destructive_action(&safe_action));
    }

    #[test]
    fn build_actions_applies_power_syntax_replace_and_prepend_modes() {
        fn focused_script() -> ScriptInfo {
            ScriptInfo {
                name: "Demo Script".to_string(),
                path: "/tmp/demo-script.ts".to_string(),
                is_script: true,
                action_verb: "Run".to_string(),
                ..ScriptInfo::default()
            }
        }

        fn power_syntax_section(mode: SectionMode) -> PowerSyntaxActionSection {
            PowerSyntaxActionSection {
                title: "Power Syntax".to_string(),
                mode,
                actions: vec![MenuSyntaxAction {
                    id: "capture.cancel".to_string(),
                    label: "Cancel without saving".to_string(),
                    kind: MenuSyntaxActionKind::Cancel,
                    enabled: true,
                }],
            }
        }

        let focused_script = Some(focused_script());
        let normal_actions = ActionsDialog::build_actions(&focused_script, &None, &None, &None);
        assert!(
            normal_actions
                .iter()
                .any(|action| action.id == "run_script"),
            "fixture must include normal selected-row actions"
        );

        let replace_section = Some(power_syntax_section(SectionMode::Replace));
        let replace_actions =
            ActionsDialog::build_actions(&focused_script, &None, &replace_section, &None);
        assert_eq!(replace_actions.len(), 1);
        assert_eq!(replace_actions[0].id, "menu_syntax:capture.cancel");
        assert!(
            !replace_actions
                .iter()
                .any(|action| action.id == "run_script"),
            "replace mode must wipe normal selected-row actions"
        );

        let prepend_section = Some(power_syntax_section(SectionMode::Prepend));
        let prepend_actions =
            ActionsDialog::build_actions(&focused_script, &None, &prepend_section, &None);
        assert_eq!(prepend_actions[0].id, "menu_syntax:capture.cancel");
        assert_eq!(&prepend_actions[1..], normal_actions.as_slice());
        assert!(
            prepend_actions[1..]
                .iter()
                .any(|action| action.id == "run_script"),
            "prepend mode must keep normal selected-row actions after Power Syntax"
        );
    }

    /// The actions menu is contextual to the focused item. Global/Discover
    /// rows and Agent Chat prompt export/handoff rows must never trail a
    /// focused item's actions; global rows are strictly a fallback for hosts
    /// where the focused row contributes nothing (so Cmd+K never opens empty).
    #[test]
    fn build_actions_keeps_global_rows_out_of_item_focused_menus() {
        let focused_script = Some(ScriptInfo {
            name: "Demo Script".to_string(),
            path: "/tmp/demo-script.ts".to_string(),
            is_script: true,
            action_verb: "Run".to_string(),
            ..ScriptInfo::default()
        });

        let focused_actions = ActionsDialog::build_actions(&focused_script, &None, &None, &None);
        assert!(
            focused_actions.iter().all(|action| {
                action.id != "reload_scripts"
                    && action.id != "sdk_reference"
                    && action.id != "open_settings_menu"
                    && !action.id.starts_with("prompt-action/")
                    && !action.id.starts_with("prompt-target/")
            }),
            "focused-item menu must not include global/Discover/prompt rows: {:?}",
            focused_actions
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>()
        );

        let fallback_actions = ActionsDialog::build_actions(&None, &None, &None, &None);
        assert!(
            fallback_actions
                .iter()
                .any(|action| action.id == "reload_scripts"),
            "empty-context menu must fall back to global rows"
        );
        assert!(
            fallback_actions.iter().all(|action| {
                !action.id.starts_with("prompt-action/") && !action.id.starts_with("prompt-target/")
            }),
            "prompt export/handoff rows are Agent Chat-owned and must not appear in the global fallback"
        );

        let host_section = Some(vec![Action::new(
            "day_page_today",
            "Today",
            Some("Host-owned row".to_string()),
            ActionCategory::ScriptContext,
        )]);
        let host_actions = ActionsDialog::build_actions(&None, &None, &None, &host_section);
        assert!(
            host_actions
                .iter()
                .all(|action| action.id != "reload_scripts"),
            "host-owned sections count as context and must suppress the global fallback"
        );
    }

    #[test]
    fn section_separator_only_shows_on_section_boundary() {
        let actions = vec![
            Action::new(
                "run_script",
                "Run Script",
                Some("Run".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Actions"),
            Action::new(
                "edit_script",
                "Edit Script",
                Some("Edit".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Edit"),
            Action::new(
                "copy_path",
                "Copy Path",
                Some("Copy".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Share"),
            Action::new(
                "copy_deeplink",
                "Copy Deeplink",
                Some("Copy".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_section("Share"),
        ];
        let filtered_actions = vec![0, 1, 2, 3];

        assert!(!should_render_section_separator(
            &actions,
            &filtered_actions,
            0
        ));
        assert!(should_render_section_separator(
            &actions,
            &filtered_actions,
            1
        ));
        assert!(should_render_section_separator(
            &actions,
            &filtered_actions,
            2
        ));
        assert!(!should_render_section_separator(
            &actions,
            &filtered_actions,
            3
        ));
    }

    #[test]
    fn test_scrollbar_viewport_subtracts_header_footer_and_search_height() {
        let total_content_height = 500.0;
        let viewport_height = actions_dialog_scrollbar_viewport_height(
            total_content_height,
            true,
            true,
            true,
            crate::actions::constants::POPUP_MAX_HEIGHT,
        );

        // POPUP_MAX_HEIGHT (400) - search (40) - context header (26) - footer (32)
        // - list padding (top 0 + bottom 6)
        assert_eq!(viewport_height, 296.0);
    }

    #[test]
    fn test_scrollbar_viewport_clamps_to_content_when_content_shorter_than_viewport() {
        let total_content_height = 120.0;
        let viewport_height = actions_dialog_scrollbar_viewport_height(
            total_content_height,
            true,
            true,
            true,
            crate::actions::constants::POPUP_MAX_HEIGHT,
        );

        assert_eq!(viewport_height, 120.0);
    }

    #[test]
    fn ux13_fixed_shell_viewport_does_not_clamp_to_short_or_empty_content() {
        let shell_height = 300.0;
        let viewport = actions_dialog_fixed_shell_viewport_height(shell_height, true, true, false);
        let tokens = crate::designs::current_actions_popup_theme();
        let expected = shell_height
            - tokens.shell.border_height
            - tokens.search.height
            - tokens.context_header.height
            - tokens.list.padding_top
            - tokens.list.padding_bottom;

        assert_eq!(viewport, expected);
        assert!(
            viewport > 120.0,
            "short content must leave safe empty space"
        );
    }

    #[test]
    fn ux13_hidden_search_preserves_the_same_shell_for_external_filtering() {
        let tokens = crate::designs::current_actions_popup_theme();
        let searchable = actions_dialog_fixed_shell_viewport_height(300.0, true, false, false);
        let hidden = actions_dialog_fixed_shell_viewport_height(300.0, false, false, false);

        assert_eq!(hidden - searchable, tokens.search.height);
    }

    #[test]
    fn test_scrollbar_reveal_offset_moves_down_when_selection_leaves_viewport() {
        let offset = actions_dialog_revealed_scroll_top(0.0, 120.0, 400.0, 144.0, 180.0);

        assert_eq!(offset, 60.0);
    }

    #[test]
    fn test_scrollbar_reveal_offset_moves_up_when_selection_is_above_viewport() {
        let offset = actions_dialog_revealed_scroll_top(160.0, 120.0, 400.0, 72.0, 108.0);

        assert_eq!(offset, 72.0);
    }

    #[test]
    fn test_scrollbar_reveal_offset_keeps_current_top_when_selection_is_visible() {
        let offset = actions_dialog_revealed_scroll_top(72.0, 120.0, 400.0, 96.0, 132.0);

        assert_eq!(offset, 72.0);
    }

    #[test]
    fn test_scrollbar_reveal_offset_clamps_to_max_scroll() {
        let offset = actions_dialog_revealed_scroll_top(240.0, 120.0, 300.0, 288.0, 324.0);

        assert_eq!(offset, 180.0);
    }

    #[test]
    fn test_scrollbar_fade_duration_matches_shared_scroll_feel() {
        assert_eq!(
            actions_dialog_scrollbar_fade_duration(),
            crate::transitions::DURATION_MEDIUM + std::time::Duration::from_millis(50)
        );
    }

    #[test]
    fn test_scrollbar_fade_opacity_starts_visible_and_ends_hidden() {
        assert_eq!(
            actions_dialog_scrollbar_fade_opacity(0.0),
            crate::transitions::Opacity::VISIBLE
        );
        assert_eq!(
            actions_dialog_scrollbar_fade_opacity(1.0),
            crate::transitions::Opacity::INVISIBLE
        );
    }

    #[test]
    fn test_action_subtitle_for_display_gates_on_show_subtitles() {
        let action_with_description = Action::new(
            "copy_path",
            "Copy Path",
            Some("Copy the selected path".to_string()),
            ActionCategory::ScriptContext,
        );
        let action_without_description = Action::new(
            "run_script",
            "Run Script",
            None,
            ActionCategory::ScriptContext,
        );

        // Action-menu hosts (show_subtitles = false) stay title-only.
        assert_eq!(
            action_subtitle_for_display(&action_with_description, false),
            None
        );
        // Switcher-style hosts opt in to render the description line.
        assert_eq!(
            action_subtitle_for_display(&action_with_description, true),
            Some("Copy the selected path")
        );
        assert_eq!(
            action_subtitle_for_display(&action_without_description, true),
            None
        );
    }

    #[test]
    fn test_matching_action_id_for_keystroke_uses_canonical_shortcut_normalization() {
        let actions = vec![
            Action::new(
                "history",
                "Agent Chat History",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘P"),
            Action::new(
                "copy_last_response",
                "Copy Last Response",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⇧⌘C"),
        ];

        let mut cmd_only = gpui::Modifiers::default();
        cmd_only.platform = true;
        assert_eq!(
            matching_action_id_for_keystroke(&actions, "p", &cmd_only),
            Some("history".to_string())
        );

        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;
        assert_eq!(
            matching_action_id_for_keystroke(&actions, "c", &shift_cmd),
            Some("copy_last_response".to_string())
        );

        assert_eq!(
            matching_action_id_for_keystroke(&actions, "x", &cmd_only),
            None
        );
    }

    #[test]
    fn test_matching_filtered_action_id_for_keystroke_ignores_hidden_actions() {
        let actions = vec![
            Action::new("rename_path", "Rename", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘R"),
            Action::new(
                "file:refresh_directory",
                "Refresh Directory",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘R"),
        ];

        let mut cmd_only = gpui::Modifiers::default();
        cmd_only.platform = true;

        assert_eq!(
            matching_filtered_action_id_for_keystroke(&actions, &[1], "r", &cmd_only),
            Some("file:refresh_directory".to_string())
        );
    }

    #[test]
    fn cmd_shift_k_matches_add_shortcut_display_shortcut() {
        let actions = vec![Action::new(
            "add_shortcut",
            "Add Keyboard Shortcut",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧K")];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            matching_action_id_for_keystroke(&actions, "k", &shift_cmd),
            Some("add_shortcut".to_string())
        );
    }

    #[test]
    fn cmd_shift_k_matches_builtin_add_shortcut_display_shortcut() {
        let builtin = ScriptInfo::with_all(
            "Theme Designer",
            "builtin:builtin/choose-theme",
            false,
            "Open",
            None,
            None,
        );
        let actions = crate::actions::get_script_context_actions(&builtin);
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        let add_shortcut = actions
            .iter()
            .find(|action| action.id == "add_shortcut")
            .expect("built-ins without an assigned shortcut must expose add_shortcut");
        assert_eq!(add_shortcut.shortcut.as_deref(), Some("⌘⇧K"));
        assert_eq!(
            matching_action_id_for_keystroke(&actions, "K", &shift_cmd),
            Some("add_shortcut".to_string())
        );
    }

    #[test]
    fn cmd_shift_k_matches_update_shortcut_display_shortcut() {
        let actions = vec![Action::new(
            "update_shortcut",
            "Edit Keyboard Shortcut",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧K")];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            matching_action_id_for_keystroke(&actions, "k", &shift_cmd),
            Some("update_shortcut".to_string())
        );
    }

    #[test]
    fn visible_shortcut_router_ignores_filtered_out_add_shortcut() {
        let actions = vec![
            Action::new(
                "add_shortcut",
                "Add Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧K"),
            Action::new(
                "copy_path",
                "Copy Path",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧C"),
        ];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            matching_filtered_action_id_for_keystroke(&actions, &[1], "k", &shift_cmd),
            None
        );
    }

    #[test]
    fn disabled_action_shortcut_is_neither_displayed_nor_routable() {
        let actions = vec![Action::new(
            "disabled",
            "Unavailable",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘D")
        .disabled("Requires a selected file")];
        let mut command = gpui::Modifiers::default();
        command.platform = true;

        assert!(visible_action_shortcut_bindings(&actions, &[0]).is_empty());
        assert_eq!(
            resolve_visible_action_shortcut(&actions, &[0], "d", &command),
            None
        );
        let report = action_shortcut_parity_report(&actions, &[0]);
        assert_eq!(report.displayed_shortcut_count, 0);
        assert_eq!(report.routable_shortcut_count, 0);
    }

    #[test]
    fn duplicate_visible_shortcuts_do_not_create_two_executable_routes() {
        let actions = vec![
            Action::new(
                "add_shortcut",
                "Add Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧K"),
            Action::new(
                "update_shortcut",
                "Edit Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+shift+k"),
        ];
        let mut shift_cmd = gpui::Modifiers::default();
        shift_cmd.platform = true;
        shift_cmd.shift = true;

        assert_eq!(
            resolve_visible_action_shortcut(&actions, &[0, 1], "k", &shift_cmd),
            None
        );
        let report = action_shortcut_parity_report(&actions, &[0, 1]);
        assert_eq!(report.displayed_shortcut_count, 0);
        assert_eq!(report.routable_shortcut_count, 0);
        assert_eq!(report.duplicate_shortcut_count, 2);
        assert!(report.unroutable_displayed_shortcuts.is_empty());
        assert!(!action_has_routable_shortcut(
            &actions,
            &[0, 1],
            "add_shortcut"
        ));
        assert!(!action_has_routable_shortcut(
            &actions,
            &[0, 1],
            "update_shortcut"
        ));
    }

    #[test]
    fn displayed_action_keybinding_specs_are_generated_from_routable_metadata() {
        let actions = vec![Action::new(
            "add_shortcut",
            "Add Keyboard Shortcut",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧K")];

        let specs = displayed_action_keybinding_specs(&actions, &[0]);

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].canonical, "cmd+shift+k");
        assert_eq!(specs[0].gpui_keystroke, "cmd-shift-k");
    }

    #[test]
    fn duplicate_displayed_action_shortcuts_do_not_generate_keybindings() {
        let actions = vec![
            Action::new(
                "add_shortcut",
                "Add Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧K"),
            Action::new(
                "update_shortcut",
                "Edit Keyboard Shortcut",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+shift+k"),
        ];

        let specs = displayed_action_keybinding_specs(&actions, &[0, 1]);

        assert!(specs.is_empty());
    }

    #[test]
    fn generated_displayed_action_binding_is_receivable_in_script_list_context() {
        let binding = gpui::KeyBinding::new(
            "cmd-shift-k",
            MainListDisplayedActionShortcut {
                shortcut: "cmd+shift+k".to_string(),
            },
            Some("script_list"),
        );
        let mut keymap = gpui::Keymap::default();
        keymap.add_bindings([binding]);

        let (matches, pending) = keymap.bindings_for_input(
            &[gpui::Keystroke::parse("cmd-shift-k").unwrap()],
            &[gpui::KeyContext::parse("script_list").unwrap()],
        );

        assert!(!pending);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_clear_duplicate_action_shortcuts_keeps_first_visible_binding() {
        let mut actions = vec![
            Action::new("rename_path", "Rename", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘R"),
            Action::new(
                "file:refresh_directory",
                "Refresh Directory",
                None,
                ActionCategory::ScriptContext,
            )
            .with_shortcut("cmd+r"),
            Action::new(
                "file:sort_name_asc",
                "Sort by Name",
                None,
                ActionCategory::ScriptContext,
            ),
        ];

        clear_duplicate_action_shortcuts(&mut actions);

        assert_eq!(actions[0].shortcut.as_deref(), Some("⌘R"));
        assert_eq!(actions[1].shortcut, None);
        assert_eq!(actions[1].shortcut_tokens, None);
        assert_eq!(actions[1].shortcut_lower, None);
        assert_eq!(actions[2].shortcut, None);
    }

    #[test]
    fn test_create_popup_shadow_returns_visible_shadow() {
        let shadows = ActionsDialog::create_popup_shadow();

        assert!(shadows.is_empty());
    }

    // ── Chrome contract tests (.impeccable.md) ──────────────────────────

    /// The live dialog omits a footer so shortcuts stay inline with rows.
    #[test]
    fn actions_dialog_omits_footer_hints() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.footer_hint_count, 0,
            "actions dialog must not show footer hints; shortcuts live in rows"
        );
    }

    /// The Storybook presenter must use the same rounded glass container mode
    /// as the live dialog.
    #[test]
    fn actions_dialog_story_presenter_uses_rounded_glass_container() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.container_mode, "rounded_glass",
            "container must expose Tahoe rounded glass chrome"
        );

        // Also verify the Storybook "current" variant agrees
        #[cfg(feature = "storybook")]
        {
            let (style, _) =
                crate::storybook::actions_dialog_variations::resolve_actions_dialog_style(Some(
                    "current",
                ));
            let story_audit = ActionsDialogChromeAudit::from_storybook_style(&style);
            assert_eq!(
                audit, story_audit,
                "live and storybook chrome audits must agree"
            );
        }
    }

    /// The search row must NOT render a divider/border — bare input per
    /// `.impeccable.md`: "Bare, no border, no background box."
    #[test]
    fn actions_dialog_story_presenter_does_not_render_search_divider() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert!(
            !audit.shows_search_divider,
            "search row must not show a divider per .impeccable.md bare input rule"
        );

        #[cfg(feature = "storybook")]
        {
            let (style, _) =
                crate::storybook::actions_dialog_variations::resolve_actions_dialog_style(Some(
                    "current",
                ));
            assert!(
                !style.show_search_divider,
                "storybook current variant must not show search divider"
            );
        }
    }

    /// Section grouping must use `SectionStyle::Headers` (spacing-defined
    /// groups), never `SectionStyle::Separators` (inline separator lines).
    #[test]
    fn actions_dialog_section_headers_require_header_mode_not_separator_mode() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.section_mode, "headers",
            "section style must be headers per .impeccable.md — no separator lines"
        );

        // Verify the default ActionsDialogConfig uses Headers
        let config = crate::actions::types::ActionsDialogConfig::default();
        assert_eq!(
            config.section_style,
            SectionStyle::Headers,
            "ActionsDialogConfig default must be SectionStyle::Headers"
        );
    }

    // ── Runtime audit tests ────────────────────────────────────────────

    #[test]
    fn actions_dialog_runtime_audit_reflects_actual_config() {
        use crate::actions::types::{ActionsDialogConfig, AnchorPosition, SearchPosition};
        let mut style = super::actions_dialog_default_style();
        // Use spec-compliant style for a clean validation pass.
        style.show_container_border = false;
        style.show_icons = true;
        let audit = ActionsDialogRuntimeAudit::from_parts(
            "test_actions_dialog",
            &ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Headers,
                anchor: AnchorPosition::Top,
                show_icons: true,
                show_footer: false,
                ..ActionsDialogConfig::default()
            },
            &style,
        );
        assert_eq!(audit.search_position, "top");
        assert_eq!(audit.section_mode, "headers");
        assert!(audit.show_icons);
        assert!(!audit.show_footer);
        assert!(!audit.shows_search_divider);
        assert!(audit.validate().is_empty());
    }

    #[test]
    fn actions_dialog_footerless_config_normalizes_legacy_footer_flag() {
        use crate::actions::types::ActionsDialogConfig;

        let config = super::actions_dialog_footerless_config(ActionsDialogConfig {
            show_footer: true,
            ..ActionsDialogConfig::default()
        });

        assert!(
            !config.show_footer,
            "actions dialogs should normalize legacy footer state to match the footerless render path"
        );
    }

    #[test]
    fn actions_dialog_runtime_audit_reports_resolved_icon_visibility() {
        use crate::actions::types::ActionsDialogConfig;
        let mut style = super::actions_dialog_default_style();
        style.show_icons = false;

        let audit = ActionsDialogRuntimeAudit::from_parts(
            "test_actions_dialog",
            &ActionsDialogConfig {
                show_icons: true,
                ..ActionsDialogConfig::default()
            },
            &style,
        );

        assert!(
            !audit.show_icons,
            "runtime audit should report rendered icon visibility, not only requested config"
        );
    }

    #[test]
    fn actions_dialog_runtime_audit_flags_separator_and_divider_regressions() {
        use crate::actions::types::{ActionsDialogConfig, AnchorPosition, SearchPosition};
        let mut style = super::actions_dialog_default_style();
        style.show_search_divider = true;
        style.show_container_border = true;
        let audit = ActionsDialogRuntimeAudit::from_parts(
            "test_actions_dialog",
            &ActionsDialogConfig {
                search_position: SearchPosition::Top,
                section_style: SectionStyle::Separators,
                anchor: AnchorPosition::Top,
                show_icons: true,
                show_footer: false,
                ..ActionsDialogConfig::default()
            },
            &style,
        );
        let violations = audit.validate();
        assert!(violations.iter().any(|v| v.field == "shows_search_divider"));
        assert!(violations.iter().any(|v| v.field == "section_mode"));
        assert!(violations
            .iter()
            .any(|v| v.field == "show_container_border"));
    }
}

// ── Focused spec tests (cargo test actions_dialog_spec_tests --lib) ──────

#[cfg(test)]
mod actions_dialog_spec_tests {
    use super::{
        ActionsDialogChromeAudit, ActionsDialogExpectedContract, ActionsDialogRuntimeAudit,
    };

    #[test]
    fn live_defaults_match_impeccable_contract() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(audit.container_mode, "rounded_glass");
        assert_eq!(audit.search_position, "top");
        assert!(!audit.shows_search_divider);
        assert_eq!(audit.section_mode, "headers");
        assert_eq!(audit.footer_hint_count, 0);
    }

    #[test]
    fn runtime_audit_flags_bottom_search() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "search_position"),
            "bottom search position should fail verification"
        );
    }

    #[test]
    fn ux13_runtime_audit_accepts_intentionally_hidden_search() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "external_search_surface",
            search_position: "hidden",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: false,
            show_container_border: false,
            footer_hint_count: 0,
        };

        assert!(audit.validate().is_empty());
    }

    #[test]
    fn runtime_audit_flags_visible_search_divider() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: true,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "shows_search_divider"),
            "visible search divider should fail verification"
        );
    }

    #[test]
    fn runtime_audit_flags_separator_sections() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "separators",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit.validate().iter().any(|v| v.field == "section_mode"),
            "separator sections should fail verification"
        );
    }

    #[test]
    fn runtime_audit_flags_visible_container_border() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "show_container_border"),
            "visible container border should fail verification"
        );
    }

    #[test]
    fn runtime_audit_flags_any_footer_presence() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: true,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 2,
        };
        assert!(
            audit.validate().iter().any(|v| v.field == "show_footer"),
            "any footer should fail verification"
        );
        assert!(
            audit
                .validate()
                .iter()
                .any(|v| v.field == "footer_hint_count"),
            "non-zero footer hint count should fail verification"
        );
    }

    #[test]
    fn spec_compliant_audit_passes_clean() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "top",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: false,
            footer_hint_count: 0,
        };
        assert!(
            audit.validate().is_empty(),
            "fully spec-compliant audit should produce zero violations"
        );
    }

    // ── Contract struct tests ─────────────────────────────────────────

    #[test]
    fn actions_dialog_live_defaults_match_top_search_contract() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        assert_eq!(
            audit.search_position,
            super::super::constants::ACTIONS_DIALOG_EXPECT_SEARCH_POSITION,
            "search position must match .impeccable.md top-search rule"
        );
    }

    #[test]
    fn actions_dialog_live_defaults_hide_container_border() {
        let audit = ActionsDialogChromeAudit::from_live_defaults();
        let style = super::actions_dialog_default_style();
        assert_eq!(
            audit.show_container_border, style.show_container_border,
            "chrome audit must reflect the actual live style value"
        );
        assert!(
            !audit.show_container_border,
            "live actions dialog defaults should stay footerless and borderless"
        );
    }

    #[test]
    fn actions_dialog_expected_contract_impeccable_matches_constants() {
        let contract = ActionsDialogExpectedContract::impeccable();
        assert_eq!(contract.search_position, "top");
        assert!(!contract.shows_search_divider);
        assert!(!contract.show_container_border);
        assert!(!contract.show_footer);
        assert_eq!(contract.footer_hint_count, 0);
    }

    #[test]
    fn actions_dialog_runtime_audit_reports_search_position_and_border_violations() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "actions_dialog.current",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        let violations = audit.validate_against(&ActionsDialogExpectedContract::impeccable());
        assert!(
            violations.iter().any(|v| v.field == "search_position"
                && v.expected == "top"
                && v.actual == "bottom"),
            "expected a search_position violation"
        );
        assert!(
            violations.iter().any(|v| v.field == "show_container_border"
                && v.expected == "false"
                && v.actual == "true"),
            "expected a show_container_border violation"
        );
    }

    #[test]
    fn actions_dialog_runtime_violations_serialize_as_machine_readable_json() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "actions_dialog.current",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        let violations = audit.validate_against(&ActionsDialogExpectedContract::impeccable());
        let json = serde_json::to_string(&violations).expect("serialize violations");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse violations");
        assert_eq!(value[0]["surface"], "actions_dialog.current");
        assert_eq!(value[0]["field"], "search_position");
        assert_eq!(value[0]["expected"], "top");
        assert_eq!(value[0]["actual"], "bottom");
    }

    #[test]
    fn actions_dialog_validate_delegates_to_validate_against_impeccable() {
        let audit = ActionsDialogRuntimeAudit {
            surface: "test_surface",
            search_position: "bottom",
            section_mode: "headers",
            shows_search_divider: false,
            show_footer: false,
            show_icons: true,
            show_container_border: true,
            footer_hint_count: 0,
        };
        let via_validate = audit.validate();
        let via_validate_against =
            audit.validate_against(&ActionsDialogExpectedContract::impeccable());
        assert_eq!(via_validate, via_validate_against);
    }
}

// ── Click contract tests ─────────────────────────────────────────────

#[cfg(test)]
mod actions_dialog_click_contract_tests {
    use super::should_submit_actions_dialog_row_click;

    #[test]
    fn actions_dialog_requires_second_single_click_after_mouse_selection() {
        assert!(!should_submit_actions_dialog_row_click(false, 1));
        assert!(should_submit_actions_dialog_row_click(true, 1));
    }

    #[test]
    fn actions_dialog_still_submits_on_native_double_click() {
        assert!(should_submit_actions_dialog_row_click(false, 2));
        assert!(should_submit_actions_dialog_row_click(false, 3));
    }
}
