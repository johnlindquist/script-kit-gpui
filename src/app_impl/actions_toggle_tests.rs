#[cfg(test)]
mod on_close_reentrancy_tests {
    use std::fs;

    /// The popup-window toggle paths (detached vibrancy ActionsWindow).
    /// Every entry must route open/close through the shared helpers so
    /// close re-entrancy and filter resync cannot drift per surface. A new
    /// toggle path must be added HERE (and use the shared helpers), not
    /// counted silently — exact-count assertions rotted three times before
    /// this enumeration replaced them (see Source Audit Test Policy).
    const POPUP_WINDOW_TOGGLE_FNS: &[&str] = &[
        "fn toggle_actions(",
        "fn toggle_root_file_actions(",
        "fn toggle_root_unified_result_actions(",
        "fn toggle_webcam_actions(",
        "fn toggle_terminal_commands(",
        "fn toggle_chat_actions(",
    ];

    fn impl_source() -> String {
        let source = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("Failed to read src/app_impl/actions_toggle.rs");
        source
            .split("\n#[cfg(test)]")
            .next()
            .expect("Expected implementation section before tests")
            .to_string()
    }

    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("missing function signature: {signature}"));
        let tail = &source[start + signature.len()..];
        let end = ["\n    fn ", "\n    pub fn ", "\n    pub(crate) fn "]
            .iter()
            .filter_map(|marker| tail.find(marker))
            .min()
            .unwrap_or(tail.len());
        &tail[..end]
    }

    #[test]
    fn test_actions_toggle_on_close_defers_script_list_app_updates() {
        let impl_source = impl_source();

        for signature in POPUP_WINDOW_TOGGLE_FNS {
            let body = function_body(&impl_source, signature);
            assert!(
                body.contains("d.set_on_close(Self::make_actions_window_on_close_callback("),
                "{signature} should use the shared on_close callback factory"
            );
        }
        assert!(
            impl_source.contains("cx.defer(move |cx| {"),
            "actions_toggle on_close callback factory should defer ScriptListApp updates"
        );
        assert!(
            impl_source.contains("if !app.show_actions_popup && app.actions_dialog.is_none()"),
            "actions_toggle on_close callbacks should guard already-closed popup state"
        );
    }

    #[test]
    fn test_toggle_actions_paths_resync_filter_input_state() {
        let impl_source = impl_source();

        // toggle_arg_actions opens inline (no popup window) but must still
        // resync the canonical filter input before opening.
        for signature in POPUP_WINDOW_TOGGLE_FNS
            .iter()
            .chain(std::iter::once(&"fn toggle_arg_actions("))
        {
            let body = function_body(&impl_source, signature);
            assert!(
                body.contains("self.resync_filter_input_after_actions_if_needed(window, cx);"),
                "{signature} should resync canonical filter input before opening"
            );
        }
        assert!(
            impl_source.contains("app.mark_filter_resync_after_actions_if_needed();"),
            "shared actions window on_close callback should mark filter resync for next render"
        );
    }

    #[test]
    fn test_actions_toggle_uses_shared_spawn_open_actions_window_helper() {
        let impl_source = impl_source();

        for signature in POPUP_WINDOW_TOGGLE_FNS {
            let body = function_body(&impl_source, signature);
            assert!(
                body.contains("Self::spawn_open_actions_window("),
                "{signature} should open the detached window through the shared spawn helper"
            );
        }

        // The slim wrapper delegates to the _with_parent_id variant, which
        // owns the actual open_actions_window match and focus handoff.
        let wrapper_body = function_body(&impl_source, "fn spawn_open_actions_window(");
        assert!(
            wrapper_body.contains("Self::spawn_open_actions_window_with_parent_id("),
            "spawn_open_actions_window should delegate to spawn_open_actions_window_with_parent_id"
        );
        let helper_body =
            function_body(&impl_source, "fn spawn_open_actions_window_with_parent_id(");
        assert!(
            helper_body.contains("match open_actions_window("),
            "spawn_open_actions_window_with_parent_id should own the open_actions_window match block"
        );
        let outside_helper = impl_source.replacen(helper_body, "", 1);
        assert!(
            !outside_helper.contains("match open_actions_window("),
            "open_actions_window match block should live only in spawn_open_actions_window_with_parent_id"
        );
        assert!(
            helper_body.contains("dialog.set_skip_track_focus(true);"),
            "spawn_open_actions_window_with_parent_id should centralize detached popup focus ownership"
        );
    }

    #[test]
    fn test_begin_actions_popup_window_open_is_used_by_popup_window_toggles_only() {
        let impl_source = impl_source();

        assert!(
            impl_source.contains("fn begin_actions_popup_window_open("),
            "actions_toggle should define begin_actions_popup_window_open helper"
        );

        for signature in POPUP_WINDOW_TOGGLE_FNS {
            let body = function_body(&impl_source, signature);
            assert!(
                body.contains("self.begin_actions_popup_window_open(cx, window);"),
                "{signature} should mark popup-window open state via the shared helper"
            );
        }

        let toggle_arg_actions_source = function_body(&impl_source, "fn toggle_arg_actions(");
        assert!(
            !toggle_arg_actions_source.contains("self.begin_actions_popup_window_open(cx, window);"),
            "toggle_arg_actions should not use begin_actions_popup_window_open (inline dialog, not a window)"
        );

        // toggle_arg_actions must still follow the same state contract as window-based toggles
        assert!(
            toggle_arg_actions_source.contains("self.gpui_input_focused = false;"),
            "toggle_arg_actions must clear gpui_input_focused on open (same contract as begin_actions_popup_window_open)"
        );
        assert!(
            toggle_arg_actions_source.contains("cx.notify();"),
            "toggle_arg_actions must end with cx.notify() (same contract as other popup toggles)"
        );

        let toggle_terminal_commands_source = impl_source
            .split("pub fn toggle_terminal_commands")
            .nth(1)
            .and_then(|section| section.split("pub fn toggle_chat_actions").next())
            .expect("toggle_terminal_commands source section should exist");
        assert!(
            toggle_terminal_commands_source
                .contains("self.begin_actions_popup_window_open(cx, window);"),
            "toggle_terminal_commands should open a vibrancy popup window for native blur"
        );
    }
}

#[cfg(test)]
mod terminal_command_shortcut_tests {
    use super::*;
    use crate::actions::{AnchorPosition, SearchPosition, SectionStyle};
    use crate::designs::icon_variations::IconName;
    use std::fs;

    #[test]
    fn test_terminal_actions_for_dialog_shows_cmd_shift_k_for_clear_terminal() {
        let clear_action = terminal_actions_for_dialog()
            .into_iter()
            .find(|action| action.id == TERM_PROMPT_CLEAR_ACTION_ID)
            .expect("clear action should exist in terminal actions");

        assert_eq!(
            clear_action.shortcut.as_deref(),
            Some(TERM_PROMPT_CLEAR_SHORTCUT)
        );
    }

    #[test]
    fn test_terminal_actions_for_dialog_adds_cmd_k_toggle_shortcut() {
        let toggle_actions = terminal_actions_for_dialog()
            .into_iter()
            .find(|action| action.id == TERM_PROMPT_ACTIONS_TOGGLE_ACTION_ID)
            .expect("toggle actions entry should exist in terminal actions");

        assert_eq!(
            toggle_actions.shortcut.as_deref(),
            Some(TERM_PROMPT_ACTIONS_TOGGLE_SHORTCUT)
        );
    }

    #[test]
    fn test_terminal_actions_for_dialog_groups_sections_and_icons() {
        let actions = terminal_actions_for_dialog();

        let copy_action = actions
            .iter()
            .find(|action| action.id == "copy")
            .expect("copy action should exist");
        assert_eq!(copy_action.section.as_deref(), Some("Clipboard"));
        assert_eq!(copy_action.icon, Some(IconName::Copy));

        let find_action = actions
            .iter()
            .find(|action| action.id == "find")
            .expect("find action should exist");
        assert_eq!(find_action.section.as_deref(), Some("Search"));
        assert_eq!(find_action.icon, Some(IconName::MagnifyingGlass));

        let scroll_to_top_action = actions
            .iter()
            .find(|action| action.id == "scroll_to_top")
            .expect("scroll_to_top action should exist");
        assert_eq!(scroll_to_top_action.section.as_deref(), Some("Navigation"));
        assert_eq!(scroll_to_top_action.icon, Some(IconName::ArrowUp));

        let scroll_to_bottom_action = actions
            .iter()
            .find(|action| action.id == TERM_PROMPT_SCROLL_TO_BOTTOM_ACTION_ID)
            .expect("scroll_to_bottom action should exist");
        assert_eq!(
            scroll_to_bottom_action.section.as_deref(),
            Some("Navigation")
        );
        assert_eq!(scroll_to_bottom_action.icon, Some(IconName::ArrowDown));

        let clear_action = actions
            .iter()
            .find(|action| action.id == TERM_PROMPT_CLEAR_ACTION_ID)
            .expect("clear action should exist");
        assert_eq!(clear_action.section.as_deref(), Some("Session"));
        assert_eq!(clear_action.icon, Some(IconName::Trash));

        let reset_action = actions
            .iter()
            .find(|action| action.id == "reset")
            .expect("reset action should exist");
        assert_eq!(reset_action.section.as_deref(), Some("Session"));
        assert_eq!(reset_action.icon, Some(IconName::Refresh));
    }

    #[test]
    fn test_terminal_actions_dialog_config_enables_visual_features() {
        let config = terminal_actions_dialog_config();

        assert_eq!(config.search_position, SearchPosition::Top);
        assert_eq!(config.section_style, SectionStyle::Headers);
        assert_eq!(config.anchor, AnchorPosition::Top);
        assert!(config.show_icons);
        assert!(
            !config.show_footer,
            "Terminal actions should stay footerless because shortcuts are rendered inline"
        );
    }

    #[test]
    fn test_toggle_terminal_commands_sets_terminal_context_title() {
        let source = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("Failed to read src/app_impl/actions_toggle.rs");

        assert!(
            source.contains("d.set_context_title(Some(\"Terminal\".to_string()));"),
            "toggle_terminal_commands should set terminal context title"
        );
    }
}

#[cfg(test)]
mod root_file_action_tests {
    use super::*;
    use crate::file_search::{FileResult, FileType};

    fn root_file(file_type: FileType) -> FileResult {
        FileResult {
            path: "/Users/example/Desktop/fix spelling.png".to_string(),
            name: "fix spelling.png".to_string(),
            size: 0,
            modified: 0,
            file_type,
        }
    }

    #[test]
    fn root_file_actions_for_regular_file_adds_browse_parent_folder() {
        let actions = root_file_actions_for(&root_file(FileType::Image));
        let titles = actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            titles,
            vec![
                "Open File",
                "Browse Parent Folder",
                "Reveal in Finder",
                "Copy Path",
                "Copy Name",
                "Quick Look"
            ]
        );
        assert_eq!(
            actions[1].id,
            crate::action_helpers::ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID
        );
        assert_eq!(actions.len(), 6);
    }

    #[test]
    fn root_file_actions_for_regular_file_displays_parent_folder_with_tilde_home() {
        let home = dirs::home_dir()
            .and_then(|path| path.to_str().map(|value| value.to_string()))
            .expect("home path should be valid UTF-8");
        let file = FileResult {
            path: format!("{home}/dev/script-kit-gpui/README.md"),
            name: "README.md".to_string(),
            size: 0,
            modified: 0,
            file_type: FileType::Document,
        };

        let actions = root_file_actions_for(&file);
        let browse_parent = actions
            .iter()
            .find(|action| {
                action.id == crate::action_helpers::ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID
            })
            .expect("Browse Parent Folder action");

        assert_eq!(browse_parent.title, "Browse Parent Folder");
        assert_eq!(
            browse_parent.description.as_deref(),
            Some("Opens ~/dev/script-kit-gpui/ in File Search")
        );
        assert!(!browse_parent
            .description
            .as_deref()
            .unwrap_or_default()
            .contains(&home));
    }

    #[test]
    fn root_file_actions_for_directory_adds_search_inside_folder() {
        let actions = root_file_actions_for(&root_file(FileType::Directory));
        let titles = actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>();

        assert_eq!(actions[0].title, "Open Folder");
        assert_eq!(
            actions[0].id,
            crate::action_helpers::ROOT_FILE_OPEN_ACTION_ID
        );
        assert_eq!(
            titles,
            vec![
                "Open Folder",
                "Search Inside Folder",
                "Reveal in Finder",
                "Copy Path",
                "Copy Name",
                "Quick Look"
            ]
        );
        assert_eq!(
            actions[1].id,
            crate::action_helpers::ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID
        );
        assert!(!actions
            .iter()
            .any(|action| action.id
                == crate::action_helpers::ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID));
    }

    #[test]
    fn root_file_action_ids_are_stable() {
        let actions = root_file_actions_for(&root_file(FileType::Image));
        let ids = actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                crate::action_helpers::ROOT_FILE_OPEN_ACTION_ID,
                crate::action_helpers::ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID,
                crate::action_helpers::ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID,
                crate::action_helpers::ROOT_FILE_COPY_PATH_ACTION_ID,
                crate::action_helpers::ROOT_FILE_COPY_NAME_ACTION_ID,
                crate::action_helpers::ROOT_FILE_QUICK_LOOK_ACTION_ID,
            ]
        );
    }

    #[test]
    fn root_file_actions_do_not_include_deferred_file_search_actions() {
        let actions = root_file_actions_for(&root_file(FileType::Image));
        let ids = actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>();

        for deferred in [
            "open_with",
            "show_info",
            "attach_to_ai",
            "copy_filename",
            "move_to_trash",
            "duplicate_file",
            "copy_file",
            "file:open_with",
            "file:show_info",
            "file:attach_to_ai",
            "file:copy_filename",
            "file:move_to_trash",
            "file:duplicate_path",
        ] {
            assert!(
                !ids.contains(&deferred),
                "root file action palette should not include deferred action {deferred}"
            );
        }
    }
}
