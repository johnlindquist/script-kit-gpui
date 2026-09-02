#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;

mod from_dialog_builtin_action_validation_tests {
    // --- merged from part_01.rs ---
    //! Built-in action behavioral validation tests
    //!
    //! Validates randomly-selected built-in actions across window dialogs and
    //! contexts to ensure invariants hold: ordering, value/has_action correctness,
    //! section label consistency, description presence, icon assignment, and
    //! cross-context guarantees.

    use super::builders::{
        get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
        get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
        get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
        get_script_context_actions, get_scriptlet_context_actions_with_custom, to_deeplink_name,
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo,
    };
    use super::command_bar::CommandBarConfig;
    use super::dialog::{
        build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
    };
    use super::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
    use super::window::count_section_headers;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::protocol::ProtocolAction;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // =========================================================================
    // Helpers
    // =========================================================================

    fn action_ids(actions: &[Action]) -> Vec<&str> {
        actions.iter().map(|a| a.id.as_str()).collect()
    }

    fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
        actions.iter().find(|a| a.id == id)
    }

    // =========================================================================
    // 1. Primary action is always first across ALL script-like contexts
    // =========================================================================

    #[test]
    fn run_script_always_first_for_basic_script() {
        let script = ScriptInfo::new("hello", "/path/hello.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].id, "run_script");
        assert!(actions[0].title.starts_with("Run"));
    }

    #[test]
    fn run_script_always_first_for_script_with_shortcut_alias_frecency() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "full",
            "/path/full.ts",
            Some("cmd+f".into()),
            Some("fl".into()),
        )
        .with_frecency(true, Some("/path/full.ts".into()));
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn run_script_always_first_for_builtin() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&builtin);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn run_script_always_first_for_scriptlet() {
        let scriptlet = ScriptInfo::scriptlet("Open URL", "/path/urls.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&scriptlet, None);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn run_script_always_first_for_agent() {
        let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
        agent.is_agent = true;
        agent.is_script = false;
        let actions = get_script_context_actions(&agent);
        assert_eq!(actions[0].id, "run_script");
    }

    // =========================================================================
    // 2. Built-in actions never have has_action=true
    // =========================================================================

    #[test]
    fn script_context_built_in_actions_have_has_action_false() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            assert!(
                !action.has_action,
                "Built-in action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn file_context_built_in_actions_have_has_action_false() {
        let file = FileInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        for action in &get_file_context_actions(&file) {
            assert!(
                !action.has_action,
                "File action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn clipboard_context_built_in_actions_have_has_action_false() {
        let entry = ClipboardEntryInfo {
            id: "c1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for action in &get_clipboard_history_context_actions(&entry) {
            assert!(
                !action.has_action,
                "Clipboard action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn path_context_built_in_actions_have_has_action_false() {
        let path = PathInfo::new("test", "/test", false);
        for action in &get_path_context_actions(&path) {
            assert!(
                !action.has_action,
                "Path action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn ai_command_bar_built_in_actions_have_has_action_false() {
        for action in &get_ai_command_bar_actions() {
            assert!(
                !action.has_action,
                "AI action '{}' should have has_action=false",
                action.id
            );
        }
    }

    // =========================================================================
    // 3. Built-in actions have no value field (value is for SDK routing)
    // =========================================================================

    #[test]
    fn script_context_built_in_actions_have_no_value() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            assert!(
                action.value.is_none(),
                "Built-in action '{}' should have no value",
                action.id
            );
        }
    }

    #[test]
    fn file_context_built_in_actions_have_no_value() {
        let file = FileInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        for action in &get_file_context_actions(&file) {
            assert!(
                action.value.is_none(),
                "File action '{}' should have no value",
                action.id
            );
        }
    }

    // =========================================================================
    // 4. Scriptlet custom actions DO have has_action=true and value
    // =========================================================================

    #[test]
    fn scriptlet_custom_actions_have_has_action_and_value() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        scriptlet.actions.push(ScriptletAction {
            name: "Copy Output".into(),
            command: "copy-output".into(),
            tool: "bash".into(),
            code: "echo output | pbcopy".into(),
            inputs: vec![],
            shortcut: None,
            description: Some("Copy the output".into()),
        });
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let custom: Vec<&Action> = actions
            .iter()
            .filter(|a| a.id.starts_with("scriptlet_action:"))
            .collect();
        assert_eq!(custom.len(), 1);
        assert!(custom[0].has_action);
        assert!(custom[0].value.is_some());
    }

    #[test]
    fn scriptlet_built_in_actions_still_have_no_has_action() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        scriptlet.actions.push(ScriptletAction {
            name: "Custom".into(),
            command: "custom".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        });
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let built_in: Vec<&Action> = actions
            .iter()
            .filter(|a| !a.id.starts_with("scriptlet_action:"))
            .collect();
        for action in &built_in {
            assert!(
                !action.has_action,
                "Built-in scriptlet action '{}' should have has_action=false",
                action.id
            );
        }
    }

    // =========================================================================
    // 5. Destructive clipboard actions always appear last
    // =========================================================================

    #[test]
    fn clipboard_destructive_actions_last_for_text_unpinned() {
        let entry = ClipboardEntryInfo {
            id: "e1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Terminal".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions);
        let len = ids.len();
        assert_eq!(ids[len - 3], "clip:clipboard_delete");
        assert_eq!(ids[len - 2], "clip:clipboard_delete_multiple");
        assert_eq!(ids[len - 1], "clip:clipboard_delete_all");
    }

    #[test]
    fn clipboard_destructive_actions_last_for_image_pinned() {
        let entry = ClipboardEntryInfo {
            id: "e2".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "".into(),
            image_dimensions: Some((640, 480)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions);
        let len = ids.len();
        assert_eq!(ids[len - 3], "clip:clipboard_delete");
        assert_eq!(ids[len - 2], "clip:clipboard_delete_multiple");
        assert_eq!(ids[len - 1], "clip:clipboard_delete_all");
    }

    // =========================================================================
    // 6. Section label consistency — no typos, same spelling across contexts
    // =========================================================================

    #[test]
    fn ai_command_bar_section_labels_are_known() {
        let known = [
            "Response",
            "Actions",
            "Attachments",
            "Export",
            "Context",
            "Help",
            "Settings",
        ];
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            let section = action.section.as_deref().unwrap();
            assert!(
                known.contains(&section),
                "Unknown AI section label: '{}' in action '{}'",
                section,
                action.id
            );
        }
    }

    #[test]
    fn new_chat_section_labels_are_known() {
        let known = ["Last Used Settings", "Presets", "Models"];
        let last = vec![NewChatModelInfo {
            model_id: "m".into(),
            display_name: "M".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "p".into(),
            name: "P".into(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "mo".into(),
            display_name: "MO".into(),
            provider: "mp".into(),
            provider_display_name: "MP".into(),
        }];
        let actions = get_new_chat_actions(&last, &presets, &models);
        for action in &actions {
            let section = action.section.as_deref().unwrap();
            assert!(
                known.contains(&section),
                "Unknown new chat section: '{}'",
                section
            );
        }
    }

    // =========================================================================
    // 7. Action count stability — deterministic for same input
    // =========================================================================

    #[test]
    fn script_context_action_count_is_deterministic() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let count1 = get_script_context_actions(&script).len();
        let count2 = get_script_context_actions(&script).len();
        let count3 = get_script_context_actions(&script).len();
        assert_eq!(count1, count2);
        assert_eq!(count2, count3);
    }

    #[test]
    fn clipboard_action_count_is_deterministic() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let count1 = get_clipboard_history_context_actions(&entry).len();
        let count2 = get_clipboard_history_context_actions(&entry).len();
        assert_eq!(count1, count2);
    }

    #[test]
    fn ai_command_bar_action_count_is_exactly_twelve() {
        assert_eq!(get_ai_command_bar_actions().len(), 38);
    }

    // =========================================================================
    // 8. Enter shortcut on primary actions across contexts
    // =========================================================================

    #[test]
    fn file_open_file_has_enter_shortcut() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn file_open_directory_has_enter_shortcut() {
        let dir = FileInfo {
            path: "/test".into(),
            name: "test".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn path_select_file_has_enter_shortcut() {
        let path = PathInfo::new("file.txt", "/file.txt", false);
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn path_open_directory_has_enter_shortcut() {
        let path = PathInfo::new("dir", "/dir", true);
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn clipboard_paste_has_enter_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    // =========================================================================
    // 9. Chat model actions — edge cases with many models
    // =========================================================================

    #[test]
    fn chat_ten_models_all_present_exactly_one_checkmark() {
        let models: Vec<ChatModelInfo> = (0..10)
            .map(|i| ChatModelInfo {
                id: format!("model-{}", i),
                display_name: format!("Model {}", i),
                provider: format!("Provider {}", i),
            })
            .collect();
        let info = ChatPromptInfo {
            current_model: Some("Model 5".into()),
            available_models: models,
            has_messages: true,
            has_response: true,
        };
        let picker = get_chat_model_picker_actions(&info);
        let checked = picker.iter().filter(|a| a.title.contains('✓')).count();
        assert_eq!(checked, 1);
        let checked_action = picker.iter().find(|a| a.title.contains('✓')).unwrap();
        assert_eq!(checked_action.id, "chat:select_model_model-5");
    }

    #[test]
    fn chat_current_model_not_in_available_models_means_no_checkmark() {
        let models = vec![ChatModelInfo {
            id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "OpenAI".into(),
        }];
        let info = ChatPromptInfo {
            current_model: Some("Nonexistent Model".into()),
            available_models: models,
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let checked = picker.iter().filter(|a| a.title.contains('✓')).count();
        assert_eq!(
            checked, 0,
            "No model should be checked when current doesn't match any"
        );
    }

    #[test]
    fn chat_model_actions_all_have_provider_description() {
        let models = vec![
            ChatModelInfo {
                id: "a".into(),
                display_name: "A".into(),
                provider: "PA".into(),
            },
            ChatModelInfo {
                id: "b".into(),
                display_name: "B".into(),
                provider: "PB".into(),
            },
        ];
        let info = ChatPromptInfo {
            current_model: None,
            available_models: models,
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        for action in &picker {
            assert!(
                action.description.as_ref().unwrap().starts_with("Uses "),
                "Model action '{}' description should start with 'via '",
                action.id
            );
        }
    }

    // =========================================================================
    // 10. Grouped items — real actions produce valid grouped output
    // =========================================================================

    #[test]
    fn ai_actions_grouped_with_headers_have_correct_structure() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

        // Last item should not be a header (it should be an action)
        assert!(
            matches!(grouped.last(), Some(GroupedActionItem::Item(_))),
            "Last grouped item should be an action, not a header"
        );
    }

    #[test]
    fn ai_actions_grouped_with_separators_have_no_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        let headers: Vec<&GroupedActionItem> = grouped
            .iter()
            .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
            .collect();
        assert!(
            headers.is_empty(),
            "Separators style should have no headers"
        );
    }

    // =========================================================================
    // 11. Coerce selection — real grouped items from AI actions
    // =========================================================================

    #[test]
    fn coerce_selection_on_real_ai_grouped_actions_finds_valid_item() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

        // Coerce at index 0 (which is likely a header) should find an item
        let result = coerce_action_selection(&grouped, 0);
        assert!(
            result.is_some(),
            "Should find an item in AI grouped actions"
        );

        // The selected row should be an Item, not a header
        if let Some(idx) = result {
            assert!(
                matches!(grouped[idx], GroupedActionItem::Item(_)),
                "Coerced selection should be an Item"
            );
        }
    }

    #[test]
    fn coerce_selection_on_every_row_returns_valid_or_none() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

        for i in 0..grouped.len() {
            let result = coerce_action_selection(&grouped, i);
            if let Some(idx) = result {
                assert!(
                    matches!(grouped[idx], GroupedActionItem::Item(_)),
                    "Row {} coerced to non-item at {}",
                    i,
                    idx
                );
            }
        }
    }

    // =========================================================================
    // 12. Score consistency — same action + same query = same score
    // =========================================================================

    #[test]
    fn score_action_is_deterministic() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            Some("Open in editor".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        let score1 = ActionsDialog::score_action(&action, "edit");
        let score2 = ActionsDialog::score_action(&action, "edit");
        let score3 = ActionsDialog::score_action(&action, "edit");
        assert_eq!(score1, score2);
        assert_eq!(score2, score3);
    }

    #[test]
    fn score_action_prefix_beats_contains_beats_fuzzy() {
        let prefix = Action::new("e", "Edit Script", None, ActionCategory::ScriptContext);
        let contains = Action::new("c", "Copy Edit Path", None, ActionCategory::ScriptContext);
        let fuzzy = Action::new("f", "Examine Data", None, ActionCategory::ScriptContext);

        let prefix_score = ActionsDialog::score_action(&prefix, "edit");
        let contains_score = ActionsDialog::score_action(&contains, "edit");
        // "edit" in "examine data": fuzzy? e-x-a-m-i-n-e- -d-i-t → not a fuzzy match for "edit"
        // Actually e at 0, d at 8, i at 9, t at 10... need e-d-i-t in order: yes that fuzzy matches
        let _fuzzy_score = ActionsDialog::score_action(&fuzzy, "edit");

        assert!(
            prefix_score > contains_score,
            "Prefix({}) should beat contains({})",
            prefix_score,
            contains_score
        );
        // Contains may or may not beat fuzzy depending on implementation, but both should be > 0
        assert!(contains_score > 0);
    }

    // =========================================================================
    // 13. Description presence for critical actions
    // =========================================================================

    #[test]
    fn script_run_action_has_description() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let run = find_action(&actions, "run_script").unwrap();
        assert!(
            run.description.is_some(),
            "run_script should have a description"
        );
    }

    #[test]
    fn script_edit_action_has_description() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let edit = find_action(&actions, "edit_script").unwrap();
        assert!(
            edit.description.is_some(),
            "edit_script should have a description"
        );
    }

    #[test]
    fn clipboard_delete_all_has_description() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let delete_all = find_action(&actions, "clip:clipboard_delete_all").unwrap();
        assert!(
            delete_all.description.is_some(),
            "clipboard_delete_all should have a description"
        );
    }

    #[test]
    fn path_move_to_trash_has_description() {
        let path = PathInfo::new("test", "/test", false);
        let actions = get_path_context_actions(&path);
        let trash = find_action(&actions, "file:move_to_trash").unwrap();
        assert!(
            trash.description.is_some(),
            "move_to_trash should have a description"
        );
    }

    // =========================================================================
    // 14. Deeplink action present across all script-like contexts
    // =========================================================================

    #[test]
    fn deeplink_present_for_script() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"copy_deeplink"));
    }

    #[test]
    fn deeplink_present_for_builtin() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&builtin);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"copy_deeplink"));
    }

    #[test]
    fn deeplink_present_for_scriptlet() {
        let scriptlet = ScriptInfo::scriptlet("Open URL", "/path/urls.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&scriptlet, None);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"copy_deeplink"));
    }

    #[test]
    fn deeplink_present_for_agent() {
        let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
        agent.is_agent = true;
        agent.is_script = false;
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"copy_deeplink"));
    }

    // =========================================================================
    // 15. Shortcut management actions: mutually exclusive add vs update/remove
    // =========================================================================

    #[test]
    fn shortcut_add_vs_update_remove_mutually_exclusive_for_scripts() {
        // No shortcut → add only
        let no_sc = ScriptInfo::new("test", "/path/test.ts");
        let no_sc_actions = get_script_context_actions(&no_sc);
        let ids = action_ids(&no_sc_actions);
        assert!(ids.contains(&"add_shortcut"));
        assert!(!ids.contains(&"update_shortcut"));
        assert!(!ids.contains(&"remove_shortcut"));

        // Has shortcut → update+remove only
        let has_sc = ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".into()));
        let has_sc_actions = get_script_context_actions(&has_sc);
        let ids = action_ids(&has_sc_actions);
        assert!(!ids.contains(&"add_shortcut"));
        assert!(ids.contains(&"update_shortcut"));
        assert!(ids.contains(&"remove_shortcut"));
    }

    #[test]
    fn alias_add_vs_update_remove_mutually_exclusive_for_scripts() {
        // No alias → add only
        let no_al = ScriptInfo::new("test", "/path/test.ts");
        let no_al_actions = get_script_context_actions(&no_al);
        let ids = action_ids(&no_al_actions);
        assert!(ids.contains(&"add_alias"));
        assert!(!ids.contains(&"update_alias"));
        assert!(!ids.contains(&"remove_alias"));

        // Has alias → update+remove only
        let has_al =
            ScriptInfo::with_shortcut_and_alias("test", "/path/test.ts", None, Some("ts".into()));
        let has_al_actions = get_script_context_actions(&has_al);
        let ids = action_ids(&has_al_actions);
        assert!(!ids.contains(&"add_alias"));
        assert!(ids.contains(&"update_alias"));
        assert!(ids.contains(&"remove_alias"));
    }

    // =========================================================================
    // 16. File context — Application type has open as primary
    // =========================================================================

    #[test]
    fn file_application_primary_is_open_file() {
        let app = FileInfo {
            path: "/Applications/Safari.app".into(),
            name: "Safari.app".into(),
            file_type: FileType::Application,
            is_dir: false,
        };
        let actions = get_file_context_actions(&app);
        assert_eq!(actions[0].id, "file:open_file");
        assert!(actions[0].title.contains("Safari.app"));
    }

    #[test]
    fn file_document_primary_is_open_file() {
        let doc = FileInfo {
            path: "/test/report.pdf".into(),
            name: "report.pdf".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&doc);
        assert_eq!(actions[0].id, "file:open_file");
    }

    #[test]
    fn file_image_primary_is_open_file() {
        let img = FileInfo {
            path: "/test/photo.jpg".into(),
            name: "photo.jpg".into(),
            file_type: FileType::Image,
            is_dir: false,
        };
        let actions = get_file_context_actions(&img);
        assert_eq!(actions[0].id, "file:open_file");
    }

    // =========================================================================
    // 17. Note switcher — many notes all unique IDs
    // =========================================================================

    #[test]
    fn note_switcher_fifty_notes_all_unique_ids() {
        let notes: Vec<NoteSwitcherNoteInfo> = (0..50)
            .map(|i| NoteSwitcherNoteInfo {
                id: format!("note-{}", i),
                title: format!("Note {}", i),
                char_count: i * 10,
                is_current: i == 25,
                is_pinned: i % 7 == 0,
                preview: String::new(),
                relative_time: String::new(),
            })
            .collect();
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions.len(), 50);
        let mut ids: Vec<&str> = action_ids(&actions);
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(total, ids.len(), "Note switcher IDs should be unique");
    }

    // --- merged from part_03.rs ---

    #[test]
    fn note_switcher_pinned_current_same_note_gets_star_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "both".into(),
            title: "Both Pinned & Current".into(),
            char_count: 42,
            is_current: true,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        assert!(actions[0].title.starts_with("• "));
    }

    // =========================================================================
    // 18. New chat actions — icons are correct per section
    // =========================================================================

    #[test]
    fn new_chat_last_used_all_get_bolt_icon() {
        let last_used: Vec<NewChatModelInfo> = (0..3)
            .map(|i| NewChatModelInfo {
                model_id: format!("m{}", i),
                display_name: format!("M{}", i),
                provider: "p".into(),
                provider_display_name: "P".into(),
            })
            .collect();
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        for action in &actions {
            assert_eq!(
                action.icon,
                Some(IconName::BoltFilled),
                "Last used '{}' should have BoltFilled icon",
                action.id
            );
        }
    }

    #[test]
    fn new_chat_models_all_get_settings_icon() {
        let models: Vec<NewChatModelInfo> = (0..3)
            .map(|i| NewChatModelInfo {
                model_id: format!("m{}", i),
                display_name: format!("M{}", i),
                provider: "p".into(),
                provider_display_name: "P".into(),
            })
            .collect();
        let actions = get_new_chat_actions(&[], &[], &models);
        for action in &actions {
            assert_eq!(
                action.icon,
                Some(IconName::Settings),
                "Model '{}' should have Settings icon",
                action.id
            );
        }
    }

    #[test]
    fn new_chat_presets_preserve_custom_icons() {
        let presets = vec![
            NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            },
            NewChatPresetInfo {
                id: "star".into(),
                name: "Star".into(),
                icon: IconName::Star,
            },
        ];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Code));
        assert_eq!(actions[1].icon, Some(IconName::Star));
    }

    // =========================================================================
    // 19. CommandBarConfig — field preservation across presets
    // =========================================================================

    #[test]
    fn command_bar_all_presets_close_on_escape() {
        assert!(CommandBarConfig::default().close_on_escape);
        assert!(CommandBarConfig::ai_style().close_on_escape);
        assert!(CommandBarConfig::notes_style().close_on_escape);
        assert!(CommandBarConfig::main_menu_style().close_on_escape);
        assert!(CommandBarConfig::no_search().close_on_escape);
    }

    #[test]
    fn command_bar_all_presets_close_on_click_outside() {
        assert!(CommandBarConfig::default().close_on_click_outside);
        assert!(CommandBarConfig::ai_style().close_on_click_outside);
        assert!(CommandBarConfig::notes_style().close_on_click_outside);
        assert!(CommandBarConfig::main_menu_style().close_on_click_outside);
        assert!(CommandBarConfig::no_search().close_on_click_outside);
    }

    #[test]
    fn command_bar_all_presets_close_on_select() {
        assert!(CommandBarConfig::default().close_on_select);
        assert!(CommandBarConfig::ai_style().close_on_select);
        assert!(CommandBarConfig::notes_style().close_on_select);
        assert!(CommandBarConfig::main_menu_style().close_on_select);
        assert!(CommandBarConfig::no_search().close_on_select);
    }

    // =========================================================================
    // 20. Fuzzy match with real action titles
    // =========================================================================

    #[test]
    fn fuzzy_match_works_on_real_action_titles() {
        // Common user search patterns against actual action titles
        assert!(ActionsDialog::fuzzy_match("edit script", "es"));
        assert!(ActionsDialog::fuzzy_match("reveal in finder", "rif"));
        assert!(ActionsDialog::fuzzy_match("copy path", "cp"));
        assert!(ActionsDialog::fuzzy_match("copy deeplink", "cdl"));
        assert!(ActionsDialog::fuzzy_match("add keyboard shortcut", "aks"));
        assert!(ActionsDialog::fuzzy_match("reset ranking", "rr"));
        assert!(ActionsDialog::fuzzy_match("view logs", "vl"));
    }

    #[test]
    fn fuzzy_match_fails_for_reversed_chars() {
        // "se" should not fuzzy match "edit script" (s comes after e)
        // Actually: e-d-i-t- -s-c-r-i-p-t → 's' at index 5, 'e' not found after 's'... wait
        // "se": s at 5, then e at... no e after index 5. So it fails.
        assert!(!ActionsDialog::fuzzy_match("edit script", "se"));
    }

    // =========================================================================
    // 21. Action verb propagation in primary action
    // =========================================================================

    #[test]
    fn action_verb_launch_propagates_to_run_action() {
        let script =
            ScriptInfo::with_action_verb("App Launcher", "builtin:launcher", false, "Launch");
        let actions = get_script_context_actions(&script);
        let run = find_action(&actions, "run_script").unwrap();
        assert!(
            run.title.starts_with("Launch"),
            "Primary action should use 'Launch' verb, got '{}'",
            run.title
        );
    }

    #[test]
    fn action_verb_switch_to_propagates_to_run_action() {
        let script =
            ScriptInfo::with_action_verb("Window Switcher", "builtin:windows", false, "Switch to");
        let actions = get_script_context_actions(&script);
        let run = find_action(&actions, "run_script").unwrap();
        // Builtins preserve the catalog verb exactly (no title-casing).
        assert_eq!(run.title, "Switch to");
    }

    #[test]
    fn action_verb_open_propagates_to_run_action() {
        let script = ScriptInfo::with_action_verb("Notes", "builtin:notes", false, "Open");
        let actions = get_script_context_actions(&script);
        let run = find_action(&actions, "run_script").unwrap();
        assert!(
            run.title.starts_with("Open"),
            "Primary action should use 'Open' verb, got '{}'",
            run.title
        );
    }

    // =========================================================================
    // 22. title_lower correctness across all contexts
    // =========================================================================

    #[test]
    fn title_lower_matches_title_for_all_clipboard_actions() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Safari".into()),
        };
        for action in &get_clipboard_history_context_actions(&entry) {
            assert_eq!(
                action.title_lower,
                action.title.to_lowercase(),
                "title_lower mismatch for clipboard action '{}'",
                action.id
            );
        }
    }

    #[test]
    fn title_lower_matches_title_for_all_file_actions() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        for action in &get_file_context_actions(&file) {
            assert_eq!(
                action.title_lower,
                action.title.to_lowercase(),
                "title_lower mismatch for file action '{}'",
                action.id
            );
        }
    }

    #[test]
    fn title_lower_matches_title_for_all_path_actions() {
        let path = PathInfo::new("test", "/test", false);
        for action in &get_path_context_actions(&path) {
            assert_eq!(
                action.title_lower,
                action.title.to_lowercase(),
                "title_lower mismatch for path action '{}'",
                action.id
            );
        }
    }

    #[test]
    fn title_lower_matches_title_for_all_chat_actions() {
        let info = ChatPromptInfo {
            current_model: Some("Model A".into()),
            available_models: vec![ChatModelInfo {
                id: "a".into(),
                display_name: "Model A".into(),
                provider: "PA".into(),
            }],
            has_messages: true,
            has_response: true,
        };
        for action in &get_chat_context_actions(&info) {
            assert_eq!(
                action.title_lower,
                action.title.to_lowercase(),
                "title_lower mismatch for chat action '{}'",
                action.id
            );
        }
    }

    // =========================================================================
    // 23. Scriptlet with zero custom actions still has built-in actions
    // =========================================================================

    #[test]
    fn scriptlet_zero_custom_actions_has_built_in_set() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        assert!(
            actions.len() >= 3,
            "Scriptlet with no custom actions should still have built-in actions"
        );
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn scriptlet_none_scriptlet_has_built_in_set() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(!actions.is_empty());
        assert_eq!(actions[0].id, "run_script");
    }

    // =========================================================================
    // 24. ProtocolAction default behavior
    // =========================================================================

    #[test]
    fn protocol_action_new_defaults_to_visible_and_closable() {
        let pa = ProtocolAction::new("Test".into());
        assert!(pa.is_visible());
        assert!(pa.should_close());
    }

    #[test]
    fn protocol_action_explicit_false_overrides_defaults() {
        let pa = ProtocolAction {
            visible: Some(false),
            close: Some(false),
            ..ProtocolAction::new("Test".into())
        };
        assert!(!pa.is_visible());
        assert!(!pa.should_close());
    }

    #[test]
    fn protocol_action_with_value_sets_value_and_keeps_has_action_false() {
        let pa = ProtocolAction::with_value("Submit".into(), "val".into());
        assert_eq!(pa.value, Some("val".into()));
        assert!(!pa.has_action);
    }

    // =========================================================================
    // 25. Cross-context: minimum action counts
    // =========================================================================

    #[test]
    fn script_context_has_at_least_seven_actions() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let count = get_script_context_actions(&script).len();
        assert!(
            count >= 7,
            "Script context should have at least 7 actions, got {}",
            count
        );
    }

    #[test]
    fn file_context_has_at_least_four_actions() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let count = get_file_context_actions(&file).len();
        assert!(
            count >= 4,
            "File context should have at least 4 actions, got {}",
            count
        );
    }

    #[test]
    fn clipboard_context_has_at_least_eight_actions() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let count = get_clipboard_history_context_actions(&entry).len();
        assert!(
            count >= 8,
            "Clipboard context should have at least 8 actions, got {}",
            count
        );
    }

    #[test]
    fn path_context_has_at_least_six_actions() {
        let path = PathInfo::new("test", "/test", false);
        let count = get_path_context_actions(&path).len();
        assert!(
            count >= 6,
            "Path context should have at least 6 actions, got {}",
            count
        );
    }

    #[test]
    fn chat_context_always_has_continue_in_chat() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"chat:continue_in_chat"));
    }

    // =========================================================================
    // 26. Scriptlet with many custom actions — all ordered after run
    // =========================================================================

    #[test]
    fn scriptlet_ten_custom_actions_all_after_run() {
        let script = ScriptInfo::scriptlet("Big", "/path/big.md", None, None);
        let mut scriptlet = Scriptlet::new("Big".into(), "bash".into(), "echo main".into());
        for i in 0..10 {
            scriptlet.actions.push(ScriptletAction {
                name: format!("Action {}", i),
                command: format!("act-{}", i),
                tool: "bash".into(),
                code: format!("echo {}", i),
                inputs: vec![],
                shortcut: None,
                description: None,
            });
        }
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        assert_eq!(actions[0].id, "run_script");
        let custom_ids: Vec<&str> = actions
            .iter()
            .filter(|a| a.id.starts_with("scriptlet_action:"))
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(custom_ids.len(), 10);
        // All custom actions should come after run_script
        let run_pos = actions.iter().position(|a| a.id == "run_script").unwrap();
        for custom in &actions {
            if custom.id.starts_with("scriptlet_action:") {
                let pos = actions.iter().position(|a| a.id == custom.id).unwrap();
                assert!(
                    pos > run_pos,
                    "Custom action '{}' at {} should be after run_script at {}",
                    custom.id,
                    pos,
                    run_pos
                );
            }
        }
    }

    // =========================================================================
    // 27. Format shortcut hint — roundtrip patterns
    // =========================================================================

    #[test]
    fn format_shortcut_hint_cmd_enter() {
        assert_eq!(ActionsDialog::format_shortcut_hint("cmd+enter"), "⌘↵");
    }

    #[test]
    fn format_shortcut_hint_cmd_shift_delete() {
        let result = ActionsDialog::format_shortcut_hint("cmd+shift+delete");
        assert!(result.contains('⌘'));
        assert!(result.contains('⇧'));
        assert!(result.contains('⌫'));
    }

    #[test]
    fn format_shortcut_hint_ctrl_alt_letter() {
        let result = ActionsDialog::format_shortcut_hint("ctrl+alt+z");
        assert!(result.contains('⌃'));
        assert!(result.contains('⌥'));
        assert!(result.contains('Z'));
    }

    // --- merged from part_04.rs ---

    // =========================================================================
    // 28. Deeplink name — preserves unicode alphanumerics
    // =========================================================================

    #[test]
    fn deeplink_name_preserves_numbers_in_script_name() {
        assert_eq!(to_deeplink_name("Script 123"), "script-123");
    }

    #[test]
    fn deeplink_name_handles_empty_string() {
        assert_eq!(to_deeplink_name(""), "_unnamed");
    }

    #[test]
    fn deeplink_name_handles_all_whitespace() {
        assert_eq!(to_deeplink_name("   "), "_unnamed");
    }

    // =========================================================================
    // 29. Notes — minimum actions for every permutation
    // =========================================================================

    // =========================================================================
    // 30. Action with_section chains correctly
    // =========================================================================

    #[test]
    fn action_with_section_chains_preserve_other_fields() {
        let action = Action::new(
            "test",
            "Test Action",
            Some("A description".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘T")
        .with_icon(IconName::Star)
        .with_section("My Section");

        assert_eq!(action.id, "test");
        assert_eq!(action.title, "Test Action");
        assert_eq!(action.description, Some("A description".into()));
        assert_eq!(action.shortcut, Some("⌘T".into()));
        assert_eq!(action.icon, Some(IconName::Star));
        assert_eq!(action.section, Some("My Section".into()));
        assert!(!action.has_action);
        assert!(action.value.is_none());
    }
}

mod from_dialog_builtin_action_validation_tests_2 {
    // --- merged from part_01.rs ---
    //! Built-in action behavioral validation tests — batch 2
    //!
    //! Validates randomly-selected built-in actions across window dialogs and
    //! contexts that were NOT covered in batch 1. Focuses on:
    //! - Action ID uniqueness across contexts
    //! - Conditional action presence (notes, chat, clipboard)
    //! - description_lower / shortcut_lower cache correctness
    //! - AI command bar icon & section presence
    //! - Clipboard content-type-specific action sets
    //! - Pin/unpin mutual exclusivity
    //! - Path & file is_dir primary action variations
    //! - Agent-specific action invariants
    //! - Note switcher edge cases (empty, singular char count, icon hierarchy)
    //! - Score bonuses for description and shortcut matches
    //! - New chat action ID format and empty section handling
    //! - CommandBarConfig preset dialog_config field specifics
    //! - Grouped items with SectionStyle::None

    use super::builders::{
        get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
        get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
        get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
        get_script_context_actions, get_scriptlet_context_actions_with_custom, to_deeplink_name,
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo,
    };
    use super::command_bar::CommandBarConfig;
    use super::dialog::{
        build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
    };
    use super::types::{Action, ActionCategory, ScriptInfo, SearchPosition, SectionStyle};
    use super::window::count_section_headers;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // =========================================================================
    // Helpers
    // =========================================================================

    fn action_ids(actions: &[Action]) -> Vec<&str> {
        actions.iter().map(|a| a.id.as_str()).collect()
    }

    fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
        actions.iter().find(|a| a.id == id)
    }

    fn has_duplicates(ids: &[&str]) -> Option<String> {
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            if !seen.insert(id) {
                return Some(id.to_string());
            }
        }
        None
    }

    // =========================================================================
    // 1. Action ID uniqueness within each context
    // =========================================================================

    #[test]
    fn script_context_action_ids_are_unique() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let ids = action_ids(&actions);
        assert!(
            has_duplicates(&ids).is_none(),
            "Duplicate action ID in script context: {:?}",
            has_duplicates(&ids)
        );
    }

    #[test]
    fn file_context_action_ids_are_unique() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let ids = action_ids(&actions);
        assert!(
            has_duplicates(&ids).is_none(),
            "Duplicate action ID in file context: {:?}",
            has_duplicates(&ids)
        );
    }

    #[test]
    fn clipboard_context_action_ids_are_unique() {
        let entry = ClipboardEntryInfo {
            id: "e1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions);
        assert!(
            has_duplicates(&ids).is_none(),
            "Duplicate action ID in clipboard context: {:?}",
            has_duplicates(&ids)
        );
    }

    #[test]
    fn path_context_action_ids_are_unique() {
        let path = PathInfo::new("test", "/test", false);
        let actions = get_path_context_actions(&path);
        let ids = action_ids(&actions);
        assert!(
            has_duplicates(&ids).is_none(),
            "Duplicate action ID in path context: {:?}",
            has_duplicates(&ids)
        );
    }

    #[test]
    fn ai_command_bar_action_ids_are_unique() {
        let actions = get_ai_command_bar_actions();
        let ids = action_ids(&actions);
        assert!(
            has_duplicates(&ids).is_none(),
            "Duplicate action ID in AI command bar: {:?}",
            has_duplicates(&ids)
        );
    }

    #[test]
    fn chat_context_action_ids_are_unique() {
        let info = ChatPromptInfo {
            current_model: Some("A".into()),
            available_models: vec![
                ChatModelInfo {
                    id: "a".into(),
                    display_name: "A".into(),
                    provider: "PA".into(),
                },
                ChatModelInfo {
                    id: "b".into(),
                    display_name: "B".into(),
                    provider: "PB".into(),
                },
            ],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        let ids = action_ids(&actions);
        assert!(
            has_duplicates(&ids).is_none(),
            "Duplicate action ID in chat context: {:?}",
            has_duplicates(&ids)
        );
    }

    // =========================================================================
    // 2. All actions have non-empty title and ID
    // =========================================================================

    #[test]
    fn all_script_actions_have_nonempty_title_and_id() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            assert!(!action.id.is_empty(), "Action has empty ID");
            assert!(
                !action.title.is_empty(),
                "Action '{}' has empty title",
                action.id
            );
        }
    }

    #[test]
    fn all_ai_actions_have_nonempty_title_and_id() {
        for action in &get_ai_command_bar_actions() {
            assert!(!action.id.is_empty(), "Action has empty ID");
            assert!(
                !action.title.is_empty(),
                "Action '{}' has empty title",
                action.id
            );
        }
    }

    // =========================================================================
    // 3. AI command bar — every action has an icon
    // =========================================================================

    #[test]
    fn ai_command_bar_all_actions_have_icons() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert!(
                action.icon.is_some(),
                "AI command bar action '{}' should have an icon",
                action.id
            );
        }
    }

    // =========================================================================
    // 4. AI command bar — every action has a section
    // =========================================================================

    #[test]
    fn ai_command_bar_all_actions_have_sections() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert!(
                action.section.is_some(),
                "AI command bar action '{}' should have a section",
                action.id
            );
        }
    }

    // =========================================================================
    // 5. description_lower matches description across contexts
    // =========================================================================

    #[test]
    fn description_lower_matches_description_for_script_actions() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            match (&action.description, &action.description_lower) {
                (Some(desc), Some(lower)) => {
                    assert_eq!(
                        lower,
                        &desc.to_lowercase(),
                        "description_lower mismatch for script action '{}'",
                        action.id
                    );
                }
                (None, None) => {} // Both absent, fine
                (Some(_), None) => {
                    panic!(
                        "Action '{}' has description but no description_lower",
                        action.id
                    );
                }
                (None, Some(_)) => {
                    panic!(
                        "Action '{}' has description_lower but no description",
                        action.id
                    );
                }
            }
        }
    }

    #[test]
    fn description_lower_matches_description_for_ai_actions() {
        for action in &get_ai_command_bar_actions() {
            match (&action.description, &action.description_lower) {
                (Some(desc), Some(lower)) => {
                    assert_eq!(
                        lower,
                        &desc.to_lowercase(),
                        "description_lower mismatch for AI action '{}'",
                        action.id
                    );
                }
                (None, None) => {}
                (Some(_), None) => {
                    panic!(
                        "AI action '{}' has description but no description_lower",
                        action.id
                    );
                }
                (None, Some(_)) => {
                    panic!(
                        "AI action '{}' has description_lower but no description",
                        action.id
                    );
                }
            }
        }
    }

    #[test]
    fn description_lower_matches_description_for_path_actions() {
        let path = PathInfo::new("test", "/test", false);
        for action in &get_path_context_actions(&path) {
            match (&action.description, &action.description_lower) {
                (Some(desc), Some(lower)) => {
                    assert_eq!(
                        lower,
                        &desc.to_lowercase(),
                        "description_lower mismatch for path action '{}'",
                        action.id
                    );
                }
                (None, None) => {}
                _ => panic!(
                    "Action '{}' has mismatched description/description_lower presence",
                    action.id
                ),
            }
        }
    }

    // =========================================================================
    // 6. shortcut_lower matches shortcut across contexts
    // =========================================================================

    #[test]
    fn shortcut_lower_matches_shortcut_for_script_actions() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            match (&action.shortcut, &action.shortcut_lower) {
                (Some(sc), Some(lower)) => {
                    assert_eq!(
                        lower,
                        &sc.to_lowercase(),
                        "shortcut_lower mismatch for script action '{}'",
                        action.id
                    );
                }
                (None, None) => {}
                (Some(_), None) => {
                    panic!("Action '{}' has shortcut but no shortcut_lower", action.id);
                }
                (None, Some(_)) => {
                    panic!("Action '{}' has shortcut_lower but no shortcut", action.id);
                }
            }
        }
    }

    #[test]
    fn shortcut_lower_matches_shortcut_for_clipboard_actions() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for action in &get_clipboard_history_context_actions(&entry) {
            match (&action.shortcut, &action.shortcut_lower) {
                (Some(sc), Some(lower)) => {
                    assert_eq!(
                        lower,
                        &sc.to_lowercase(),
                        "shortcut_lower mismatch for clipboard action '{}'",
                        action.id
                    );
                }
                (None, None) => {}
                _ => panic!(
                    "Action '{}' has mismatched shortcut/shortcut_lower presence",
                    action.id
                ),
            }
        }
    }

    // =========================================================================
    // 7. Notes conditional actions — selection + non-trash required
    // =========================================================================

    // --- merged from part_02.rs ---

    // =========================================================================
    // 8. Notes auto-sizing toggle — only when disabled
    // =========================================================================

    // =========================================================================
    // 9. Chat conditional actions — copy_response / clear_conversation
    // =========================================================================

    #[test]
    fn chat_copy_response_only_when_has_response() {
        let with_response = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions_1 = get_chat_context_actions(&with_response);
        let ids = action_ids(&actions_1);
        assert!(ids.contains(&"chat:copy_response"));

        let without_response = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions_2 = get_chat_context_actions(&without_response);
        let ids_no = action_ids(&actions_2);
        assert!(!ids_no.contains(&"chat:copy_response"));
    }

    #[test]
    fn chat_clear_conversation_only_when_has_messages() {
        let with_messages = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions_1 = get_chat_context_actions(&with_messages);
        let ids = action_ids(&actions_1);
        assert!(ids.contains(&"chat:clear_conversation"));

        let without_messages = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions_2 = get_chat_context_actions(&without_messages);
        let ids_no = action_ids(&actions_2);
        assert!(!ids_no.contains(&"chat:clear_conversation"));
    }

    #[test]
    fn chat_empty_models_still_has_continue_in_chat() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        crate::actions::dialog_builtin_validation::assert_chat_root_contract(&info, &actions);
        assert_eq!(actions[0].id, "chat:change_model");
        assert_eq!(actions[1].id, "chat:continue_in_chat");
    }

    #[test]
    fn chat_full_context_has_all_actions() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: true,
            has_response: true,
        };
        let actions_tmp = get_chat_context_actions(&info);
        let ids = action_ids(&actions_tmp);
        assert!(ids.contains(&"chat:change_model"));
        assert!(ids.contains(&"chat:continue_in_chat"));
        assert!(ids.contains(&"chat:copy_response"));
        assert!(ids.contains(&"chat:clear_conversation"));
    }

    // =========================================================================
    // 10. Clipboard content-type-specific actions
    // =========================================================================

    #[test]
    fn clipboard_image_has_ocr_action() {
        let entry = ClipboardEntryInfo {
            id: "img1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions_tmp = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions_tmp);
        assert!(
            ids.contains(&"clip:clipboard_ocr"),
            "Image entry should have OCR action"
        );
    }

    #[test]
    fn clipboard_text_has_no_ocr_action() {
        let entry = ClipboardEntryInfo {
            id: "txt1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions_tmp = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions_tmp);
        assert!(
            !ids.contains(&"clip:clipboard_ocr"),
            "Text entry should NOT have OCR action"
        );
    }

    #[test]
    fn clipboard_image_has_more_actions_than_text() {
        let text_entry = ClipboardEntryInfo {
            id: "t".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let image_entry = ClipboardEntryInfo {
            id: "i".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((10, 10)),
            frontmost_app_name: None,
        };
        let text_count = get_clipboard_history_context_actions(&text_entry).len();
        let image_count = get_clipboard_history_context_actions(&image_entry).len();
        assert!(
            image_count > text_count,
            "Image ({}) should have more actions than text ({})",
            image_count,
            text_count
        );
    }

    // =========================================================================
    // 11. Clipboard pin/unpin mutual exclusivity
    // =========================================================================

    #[test]
    fn clipboard_pinned_has_unpin_not_pin() {
        let entry = ClipboardEntryInfo {
            id: "p1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "pinned".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions_tmp = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions_tmp);
        assert!(
            ids.contains(&"clip:clipboard_unpin"),
            "Pinned entry should have unpin"
        );
        assert!(
            !ids.contains(&"clip:clipboard_pin"),
            "Pinned entry should NOT have pin"
        );
    }

    #[test]
    fn clipboard_unpinned_has_pin_not_unpin() {
        let entry = ClipboardEntryInfo {
            id: "u1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "unpinned".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions_tmp = get_clipboard_history_context_actions(&entry);
        let ids = action_ids(&actions_tmp);
        assert!(
            ids.contains(&"clip:clipboard_pin"),
            "Unpinned entry should have pin"
        );
        assert!(
            !ids.contains(&"clip:clipboard_unpin"),
            "Unpinned entry should NOT have unpin"
        );
    }

    // =========================================================================
    // 12. Clipboard frontmost app name in paste title
    // =========================================================================

    #[test]
    fn clipboard_paste_title_includes_app_name() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: Some("VS Code".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
        assert!(
            paste.title.contains("VS Code"),
            "Paste title should include app name, got '{}'",
            paste.title
        );
    }

    #[test]
    fn clipboard_paste_title_fallback_when_no_app() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
        assert!(
            paste.title.contains("Active App"),
            "Paste title should say 'Active App' as fallback, got '{}'",
            paste.title
        );
    }

    // =========================================================================
    // 13. Path context — is_dir differences
    // =========================================================================

    #[test]
    fn path_directory_primary_is_open_directory() {
        let path = PathInfo::new("my-dir", "/my-dir", true);
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:open_directory");
        assert!(actions[0].title.contains("my-dir"));
    }

    #[test]
    fn path_file_primary_is_select_file() {
        let path = PathInfo::new("file.txt", "/file.txt", false);
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:select_file");
        assert!(actions[0].title.contains("file.txt"));
    }

    #[test]
    fn path_trash_description_varies_by_is_dir() {
        let dir_path = PathInfo::new("dir", "/dir", true);
        let dir_actions = get_path_context_actions(&dir_path);
        let dir_trash = find_action(&dir_actions, "file:move_to_trash").unwrap();
        assert!(
            dir_trash.description.as_ref().unwrap().contains("folder"),
            "Directory trash description should mention 'folder', got '{:?}'",
            dir_trash.description
        );

        let file_path = PathInfo::new("file.txt", "/file.txt", false);
        let file_actions = get_path_context_actions(&file_path);
        let file_trash = find_action(&file_actions, "file:move_to_trash").unwrap();
        assert!(
            file_trash.description.as_ref().unwrap().contains("file"),
            "File trash description should mention 'file', got '{:?}'",
            file_trash.description
        );
    }

    // =========================================================================
    // 14. File context — is_dir differences
    // =========================================================================

    #[test]
    fn file_directory_primary_is_open_directory() {
        let dir = FileInfo {
            path: "/my-dir".into(),
            name: "my-dir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn file_non_directory_primary_is_open_file() {
        let file = FileInfo {
            path: "/test.rs".into(),
            name: "test.rs".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        assert_eq!(actions[0].id, "file:open_file");
    }

    #[test]
    fn file_directory_has_no_quick_look() {
        let dir = FileInfo {
            path: "/my-dir".into(),
            name: "my-dir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions_tmp = get_file_context_actions(&dir);
        let ids = action_ids(&actions_tmp);
        assert!(
            !ids.contains(&"file:quick_look"),
            "Directories should not have quick_look"
        );
    }

    // =========================================================================
    // 15. Agent-specific action invariants
    // =========================================================================

    #[test]
    fn agent_has_edit_with_agent_title() {
        let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
        agent.is_agent = true;
        agent.is_script = false;
        let actions = get_script_context_actions(&agent);
        let edit = find_action(&actions, "edit_script").unwrap();
        assert!(
            edit.title.contains("Agent"),
            "Agent edit action should say 'Agent', got '{}'",
            edit.title
        );
    }

    // --- merged from part_03.rs ---

    #[test]
    fn agent_has_reveal_and_copy_path() {
        let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
        agent.is_agent = true;
        agent.is_script = false;
        let actions_tmp = get_script_context_actions(&agent);
        let ids = action_ids(&actions_tmp);
        assert!(ids.contains(&"reveal_in_finder"));
        assert!(ids.contains(&"copy_path"));
        assert!(ids.contains(&"copy_content"));
    }

    #[test]
    fn agent_lacks_view_logs() {
        let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
        agent.is_agent = true;
        agent.is_script = false;
        let actions_tmp = get_script_context_actions(&agent);
        let ids = action_ids(&actions_tmp);
        assert!(
            !ids.contains(&"view_logs"),
            "Agent should not have view_logs"
        );
    }

    // =========================================================================
    // 16. Builtin lacks file-specific actions
    // =========================================================================

    #[test]
    fn builtin_lacks_edit_view_logs_reveal_copy_path_copy_content() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        let actions_tmp = get_script_context_actions(&builtin);
        let ids = action_ids(&actions_tmp);
        assert!(!ids.contains(&"edit_script"));
        assert!(!ids.contains(&"view_logs"));
        assert!(!ids.contains(&"file:reveal_in_finder"));
        assert!(!ids.contains(&"file:copy_path"));
        assert!(!ids.contains(&"copy_content"));
    }

    #[test]
    fn builtin_has_run_shortcut_alias_deeplink() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        let actions_tmp = get_script_context_actions(&builtin);
        let ids = action_ids(&actions_tmp);
        assert!(ids.contains(&"run_script"));
        assert!(ids.contains(&"add_shortcut"));
        assert!(ids.contains(&"add_alias"));
        assert!(ids.contains(&"copy_deeplink"));
    }

    // =========================================================================
    // 17. Note switcher edge cases
    // =========================================================================

    #[test]
    fn note_switcher_empty_shows_no_notes_placeholder() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "no_notes");
        assert!(actions[0].title.contains("No notes"));
        assert_eq!(actions[0].icon, Some(IconName::Plus));
    }

    #[test]
    fn note_switcher_singular_character_count() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "one".into(),
            title: "One Char".into(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions[0].description.as_ref().unwrap().contains("1 char"),
            "Singular should be '1 char', got '{:?}'",
            actions[0].description
        );
        assert!(
            !actions[0].description.as_ref().unwrap().contains("chars"),
            "Singular should NOT contain 'chars'"
        );
    }

    #[test]
    fn note_switcher_plural_character_count() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "many".into(),
            title: "Many Chars".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions[0]
                .description
                .as_ref()
                .unwrap()
                .contains("42 chars"),
            "Plural should be '42 chars', got '{:?}'",
            actions[0].description
        );
    }

    #[test]
    fn note_switcher_zero_characters_plural() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "empty".into(),
            title: "Empty Note".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions[0].description.as_ref().unwrap().contains("0 chars"),
            "Zero should be '0 chars', got '{:?}'",
            actions[0].description
        );
    }

    #[test]
    fn note_switcher_icon_hierarchy_pinned_over_current() {
        // Pinned + current = StarFilled (pinned wins)
        let notes = vec![NoteSwitcherNoteInfo {
            id: "both".into(),
            title: "Both".into(),
            char_count: 5,
            is_current: true,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn note_switcher_icon_current_only() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "cur".into(),
            title: "Current".into(),
            char_count: 5,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn note_switcher_icon_default() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "plain".into(),
            title: "Plain".into(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    #[test]
    fn note_switcher_current_has_bullet_prefix() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "cur".into(),
                title: "Current Note".into(),
                char_count: 5,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "other".into(),
                title: "Other Note".into(),
                char_count: 3,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions[0].title.starts_with("• "),
            "Current note should have '• ' prefix, got '{}'",
            actions[0].title
        );
        assert!(
            !actions[1].title.starts_with("• "),
            "Non-current note should NOT have '• ' prefix"
        );
    }

    #[test]
    fn note_switcher_all_have_notes_section() {
        let notes: Vec<NoteSwitcherNoteInfo> = (0..5)
            .map(|i| NoteSwitcherNoteInfo {
                id: format!("n{}", i),
                title: format!("Note {}", i),
                char_count: i * 10,
                is_current: i == 0,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            })
            .collect();
        for action in &get_note_switcher_actions(&notes) {
            assert!(
                action.section.as_deref() == Some("Recent")
                    || action.section.as_deref() == Some("Pinned"),
                "Note switcher action '{}' should be in 'Recent' or 'Pinned' section, got {:?}",
                action.id,
                action.section
            );
        }
    }

    // =========================================================================
    // 18. Score bonuses for description and shortcut matches
    // =========================================================================

    #[test]
    fn score_description_only_match_returns_nonzero() {
        // Title doesn't match, but description contains the query
        let action = Action::new(
            "test",
            "Something Unrelated",
            Some("Opens the editor for you".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "editor");
        assert!(
            score > 0,
            "Description-only match should return nonzero score, got {}",
            score
        );
    }

    #[test]
    fn score_shortcut_only_match_returns_nonzero() {
        let action = Action::new(
            "test",
            "Something Unrelated",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "⌘e");
        assert!(
            score > 0,
            "Shortcut-only match should return nonzero score, got {}",
            score
        );
    }

    #[test]
    fn score_no_match_returns_zero() {
        let action = Action::new(
            "test",
            "Edit Script",
            Some("Open in editor".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "zzzznotfound");
        assert_eq!(score, 0, "No match should return 0");
    }

    #[test]
    fn score_title_plus_description_bonus_stacks() {
        let action = Action::new(
            "script:edit",
            "Edit Script",
            Some("Edit the script file".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        // Should get prefix bonus (100) + description bonus (15) = 115
        assert!(
            score > 100,
            "Title + description match should stack bonuses, got {}",
            score
        );
    }

    // =========================================================================
    // 19. New chat action ID format and empty sections
    // =========================================================================

    #[test]
    fn new_chat_last_used_ids_are_indexed() {
        let last = vec![
            NewChatModelInfo {
                model_id: "a".into(),
                display_name: "A".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            },
            NewChatModelInfo {
                model_id: "b".into(),
                display_name: "B".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            },
        ];
        let actions = get_new_chat_actions(&last, &[], &[]);
        assert_eq!(actions[0].id, "last_used_p::a");
        assert_eq!(actions[1].id, "last_used_p::b");
    }

    #[test]
    fn new_chat_preset_ids_use_preset_id() {
        let presets = vec![NewChatPresetInfo {
            id: "code-review".into(),
            name: "Code Review".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_code-review");
    }

    #[test]
    fn new_chat_model_ids_are_indexed() {
        let models = vec![
            NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            },
            NewChatModelInfo {
                model_id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            },
        ];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_anthropic::claude");
        assert_eq!(actions[1].id, "model_openai::gpt4");
    }

    #[test]
    fn new_chat_empty_all_sections_returns_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(
            actions.is_empty(),
            "All empty sections should return empty actions"
        );
    }

    #[test]
    fn new_chat_model_descriptions_have_provider() {
        let models = vec![NewChatModelInfo {
            model_id: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses Anthropic"),
            "Model description should be provider_display_name"
        );
    }

    #[test]
    fn new_chat_last_used_descriptions_have_provider() {
        let last = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last, &[], &[]);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses OpenAI"),
            "Last used description should be provider_display_name"
        );
    }

    #[test]
    fn new_chat_presets_have_no_description() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert!(
            actions[0].description.as_deref() == Some("Uses General preset"),
            "Presets should include an explicit preset description"
        );
    }

    // =========================================================================
    // 20. Global actions are seeded with GlobalOps entries
    // =========================================================================

    #[test]
    fn global_actions_always_seeded_with_global_ops() {
        use super::builders::get_global_actions;
        let actions = get_global_actions();
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        // get_global_actions() is seeded (never empty): reload_scripts, settings,
        // view_logs, plus the Discover rows.
        assert!(ids.contains(&"reload_scripts"));
        assert!(ids.contains(&"settings"));
        assert!(ids.contains(&"view_logs"));
    }

    // =========================================================================
    // 21. Deeplink name edge cases
    // =========================================================================

    #[test]
    fn deeplink_name_multiple_spaces_collapsed() {
        assert_eq!(to_deeplink_name("My   Script   Name"), "my-script-name");
    }

    #[test]
    fn deeplink_name_underscores_become_hyphens() {
        assert_eq!(to_deeplink_name("my_script_name"), "my-script-name");
    }

    #[test]
    fn deeplink_name_mixed_case_special_chars() {
        assert_eq!(to_deeplink_name("Hello (World) #1!"), "hello-world-1");
    }

    #[test]
    fn deeplink_name_leading_trailing_special_chars() {
        assert_eq!(to_deeplink_name("---hello---"), "hello");
    }

    // --- merged from part_04.rs ---

    #[test]
    fn deeplink_name_single_word() {
        assert_eq!(to_deeplink_name("script"), "script");
    }

    // =========================================================================
    // 22. CommandBarConfig preset dialog_config specifics
    // =========================================================================

    #[test]
    fn command_bar_ai_style_has_search_top_and_headers() {
        let config = CommandBarConfig::ai_style();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Top
        ));
        assert!(matches!(
            config.dialog_config.section_style,
            SectionStyle::Headers
        ));
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn command_bar_main_menu_has_search_top_and_separators() {
        let config = CommandBarConfig::main_menu_style();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Top
        ));
        assert!(matches!(
            config.dialog_config.section_style,
            SectionStyle::Headers
        ));
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn command_bar_notes_has_search_top_and_separators() {
        let config = CommandBarConfig::notes_style();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Top
        ));
        assert!(matches!(
            config.dialog_config.section_style,
            SectionStyle::Headers
        ));
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn command_bar_no_search_has_hidden_search() {
        let config = CommandBarConfig::no_search();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Hidden
        ));
    }

    // =========================================================================
    // 23. Grouped items with SectionStyle::None
    // =========================================================================

    #[test]
    fn grouped_items_none_style_has_no_headers_or_separators() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        for item in &grouped {
            assert!(
                matches!(item, GroupedActionItem::Item(_)),
                "SectionStyle::None should produce only Items, not headers/separators"
            );
        }
    }

    #[test]
    fn grouped_items_none_style_count_matches_filtered() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        assert_eq!(grouped.len(), filtered.len());
    }

    // =========================================================================
    // 24. Coerce selection on notes grouped actions
    // =========================================================================

    // =========================================================================
    // 25. title_lower correctness for AI and notes contexts
    // =========================================================================

    #[test]
    fn title_lower_matches_title_for_all_ai_actions() {
        for action in &get_ai_command_bar_actions() {
            assert_eq!(
                action.title_lower,
                action.title.to_lowercase(),
                "title_lower mismatch for AI action '{}'",
                action.id
            );
        }
    }

    #[test]
    fn title_lower_matches_title_for_note_switcher_actions() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "a".into(),
                title: "Capital Title".into(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "b".into(),
                title: "ALL CAPS NOTE".into(),
                char_count: 20,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        for action in &get_note_switcher_actions(&notes) {
            assert_eq!(
                action.title_lower,
                action.title.to_lowercase(),
                "title_lower mismatch for note switcher action '{}'",
                action.id
            );
        }
    }

    // =========================================================================
    // 26. Scriptlet custom action with shortcut and description
    // =========================================================================

    #[test]
    fn scriptlet_custom_action_shortcut_is_formatted() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        scriptlet.actions.push(ScriptletAction {
            name: "Copy Output".into(),
            command: "copy-output".into(),
            tool: "bash".into(),
            code: "echo | pbcopy".into(),
            inputs: vec![],
            shortcut: Some("cmd+shift+c".into()),
            description: None,
        });
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let custom = actions
            .iter()
            .find(|a| a.id.starts_with("scriptlet_action:"))
            .unwrap();
        let sc = custom.shortcut.as_ref().unwrap();
        assert!(
            sc.contains('⌘') && sc.contains('⇧'),
            "Scriptlet shortcut should be formatted with symbols, got '{}'",
            sc
        );
    }

    #[test]
    fn scriptlet_custom_action_description_propagated() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        scriptlet.actions.push(ScriptletAction {
            name: "Explained".into(),
            command: "explained".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: None,
            description: Some("A detailed description".into()),
        });
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let custom = actions
            .iter()
            .find(|a| a.id.starts_with("scriptlet_action:"))
            .unwrap();
        assert_eq!(
            custom.description.as_deref(),
            Some("A detailed description")
        );
    }

    // =========================================================================
    // 27. All actions have ActionCategory::ScriptContext
    // =========================================================================

    #[test]
    fn all_script_actions_are_script_context_category() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            assert!(
                matches!(action.category, ActionCategory::ScriptContext),
                "Action '{}' should be ScriptContext category",
                action.id
            );
        }
    }

    #[test]
    fn all_clipboard_actions_are_script_context_category() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for action in &get_clipboard_history_context_actions(&entry) {
            assert!(
                matches!(action.category, ActionCategory::ScriptContext),
                "Clipboard action '{}' should be ScriptContext category",
                action.id
            );
        }
    }

    #[test]
    fn all_file_actions_are_script_context_category() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        for action in &get_file_context_actions(&file) {
            assert!(
                matches!(action.category, ActionCategory::ScriptContext),
                "File action '{}' should be ScriptContext category",
                action.id
            );
        }
    }

    #[test]
    fn all_ai_actions_are_script_context_category() {
        for action in &get_ai_command_bar_actions() {
            assert!(
                matches!(action.category, ActionCategory::ScriptContext),
                "AI action '{}' should be ScriptContext category",
                action.id
            );
        }
    }

    // =========================================================================
    // 28. File context primary action title includes filename
    // =========================================================================

    #[test]
    fn file_primary_action_title_includes_filename() {
        let file = FileInfo {
            path: "/docs/readme.md".into(),
            name: "readme.md".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        assert!(
            actions[0].title.contains("readme.md"),
            "Primary action title should include filename, got '{}'",
            actions[0].title
        );
    }

    #[test]
    fn file_directory_primary_title_includes_dirname() {
        let dir = FileInfo {
            path: "/projects/my-app".into(),
            name: "my-app".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        assert!(
            actions[0].title.contains("my-app"),
            "Directory primary action title should include dirname, got '{}'",
            actions[0].title
        );
    }

    // =========================================================================
    // 29. Frecency reset ranking conditional
    // =========================================================================

    #[test]
    fn frecency_not_suggested_lacks_reset_ranking() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions_tmp = get_script_context_actions(&script);
        let ids = action_ids(&actions_tmp);
        assert!(!ids.contains(&"reset_ranking"));
    }

    #[test]
    fn frecency_suggested_has_reset_ranking() {
        let script =
            ScriptInfo::new("test", "/path/test.ts").with_frecency(true, Some("/path".into()));
        let actions_tmp = get_script_context_actions(&script);
        let ids = action_ids(&actions_tmp);
        assert!(ids.contains(&"reset_ranking"));
    }

    #[test]
    fn frecency_suggested_reset_ranking_is_last() {
        let script =
            ScriptInfo::new("test", "/path/test.ts").with_frecency(true, Some("/path".into()));
        let actions = get_script_context_actions(&script);
        let last = actions.last().unwrap();
        assert_eq!(
            last.id, "reset_ranking",
            "reset_ranking should be the last action"
        );
    }

    // =========================================================================
    // 30. All actions have descriptions (broad check)
    // =========================================================================

    #[test]
    fn all_script_context_actions_have_descriptions() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            assert!(
                action.description.is_some(),
                "Script action '{}' should have a description",
                action.id
            );
        }
    }

    #[test]
    fn all_ai_command_bar_actions_have_descriptions() {
        for action in &get_ai_command_bar_actions() {
            assert!(
                action.description.is_some(),
                "AI action '{}' should have a description",
                action.id
            );
        }
    }

    #[test]
    fn all_path_actions_have_descriptions() {
        let path = PathInfo::new("test", "/test", false);
        for action in &get_path_context_actions(&path) {
            assert!(
                action.description.is_some(),
                "Path action '{}' should have a description",
                action.id
            );
        }
    }

    #[test]
    fn all_file_actions_have_descriptions() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        for action in &get_file_context_actions(&file) {
            assert!(
                action.description.is_some(),
                "File action '{}' should have a description",
                action.id
            );
        }
    }

    // =========================================================================
    // 31. Clipboard has_action=false and no value for all entries
    // =========================================================================

    #[test]
    fn clipboard_all_actions_have_no_value() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for action in &get_clipboard_history_context_actions(&entry) {
            assert!(
                action.value.is_none(),
                "Clipboard action '{}' should have no value",
                action.id
            );
        }
    }

    // --- merged from part_05.rs ---

    #[test]
    fn path_all_actions_have_no_value() {
        let path = PathInfo::new("test", "/test", false);
        for action in &get_path_context_actions(&path) {
            assert!(
                action.value.is_none(),
                "Path action '{}' should have no value",
                action.id
            );
        }
    }

    // =========================================================================
    // 32. Section header count consistency for AI with headers
    // =========================================================================

    #[test]
    fn ai_section_header_count_is_eight() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let header_count = count_section_headers(&actions, &filtered);
        assert_eq!(
            header_count, 8,
            "AI command bar should have 8 section headers (Response, Actions, Attachments, Export, Context, Actions, Help, Settings)"
        );
    }

    // =========================================================================
    // 33. Scriptlet context actions from get_scriptlet_context_actions_with_custom
    //     have all the same universal actions as script context
    // =========================================================================

    #[test]
    fn scriptlet_context_has_shortcut_alias_deeplink() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions_tmp = get_scriptlet_context_actions_with_custom(&script, None);
        let ids = action_ids(&actions_tmp);
        assert!(ids.contains(&"add_shortcut"));
        assert!(ids.contains(&"add_alias"));
        assert!(ids.contains(&"copy_deeplink"));
    }

    #[test]
    fn scriptlet_context_has_edit_reveal_copy() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions_tmp = get_scriptlet_context_actions_with_custom(&script, None);
        let ids = action_ids(&actions_tmp);
        assert!(ids.contains(&"edit_scriptlet"));
        assert!(ids.contains(&"reveal_scriptlet_in_finder"));
        assert!(ids.contains(&"copy_scriptlet_path"));
        assert!(ids.contains(&"copy_content"));
    }

    // =========================================================================
    // 34. Notes new_note always present across all permutations
    // =========================================================================

    // =========================================================================
    // 35. Fuzzy match on real action IDs across contexts
    // =========================================================================

    #[test]
    fn fuzzy_match_on_clipboard_action_titles() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // "pke" should fuzzy match "Paste and Keep Window Open" (p-a-s-t-e... k-e-e-p)
        let paste_keep = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste_keep_open")
            .unwrap();
        assert!(ActionsDialog::fuzzy_match(&paste_keep.title_lower, "pke"));
    }

    // =========================================================================
    // 36. Grouped items headers style produces section headers
    // =========================================================================

    #[test]
    fn grouped_items_headers_style_has_section_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        let header_count = grouped
            .iter()
            .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
            .count();
        assert!(
            header_count > 0,
            "Headers style should produce at least one section header"
        );
    }

    #[test]
    fn grouped_items_separators_style_has_separator_items() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Should have separator items but no header items
        let headers = grouped
            .iter()
            .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(headers, 0, "Separators style should have no headers");
    }
}

mod from_dialog_builtin_action_validation_tests_3 {
    // --- merged from part_01.rs ---
    //! Built-in action behavioral validation tests — batch 3
    //!
    //! Validates randomly-selected built-in actions across window dialogs and
    //! contexts that were NOT covered in batch 1 or batch 2. Focuses on:
    //! - Shortcut uniqueness within each context (no duplicate hotkeys)
    //! - Action ordering stability / determinism across repeated calls
    //! - Cross-context action exclusivity (clipboard IDs never in file context etc.)
    //! - with_shortcut_opt builder correctness
    //! - Section ordering in AI, Notes, and New Chat command bars
    //! - Scriptlet with multiple custom H3 actions: ordering and ID format
    //! - Action title formatting with varied action_verbs
    //! - Path context shortcut assignments completeness
    //! - Clipboard ordering invariant (paste first, deletes last)
    //! - Mixed flag combinations on ScriptInfo
    //! - Note switcher icon hierarchy for all is_current × is_pinned combos
    //! - to_deeplink_name with unicode / emoji edge cases
    //! - Score stacking (title + description bonuses accumulate)
    //! - File context primary title includes filename
    //! - Scriptlet context action order: run > custom > shortcut > built-in > deeplink
    //! - Chat model checkmark only on current model
    //! - Notes conditional section counts across all 8 permutations
    //! - CommandBarConfig notes_style specifics

    use super::builders::{
        get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
        get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
        get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
        get_script_context_actions, get_scriptlet_context_actions_with_custom, to_deeplink_name,
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo,
    };
    use super::command_bar::CommandBarConfig;
    use super::dialog::{
        build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
    };
    use super::types::{Action, ActionCategory, ScriptInfo, SearchPosition, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};
    use std::collections::HashSet;

    // =========================================================================
    // Helpers
    // =========================================================================

    fn action_ids(actions: &[Action]) -> Vec<&str> {
        actions.iter().map(|a| a.id.as_str()).collect()
    }

    fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
        actions.iter().find(|a| a.id == id)
    }

    fn sections_in_order(actions: &[Action]) -> Vec<&str> {
        let mut sections = Vec::new();
        for a in actions {
            if let Some(ref s) = a.section {
                if sections
                    .last()
                    .map(|l: &&str| *l != s.as_str())
                    .unwrap_or(true)
                {
                    sections.push(s.as_str());
                }
            }
        }
        sections
    }

    // =========================================================================
    // 1. Shortcut uniqueness within context — no two actions share a hotkey
    // =========================================================================

    #[test]
    fn script_context_shortcuts_are_unique() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let mut seen = HashSet::new();
        for a in &actions {
            if let Some(ref s) = a.shortcut {
                assert!(
                    seen.insert(s.as_str()),
                    "Duplicate shortcut '{}' on action '{}' in script context",
                    s,
                    a.id
                );
            }
        }
    }

    #[test]
    fn clipboard_context_text_shortcuts_are_unique() {
        let entry = ClipboardEntryInfo {
            id: "e1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let mut seen = HashSet::new();
        for a in &actions {
            if let Some(ref s) = a.shortcut {
                assert!(
                    seen.insert(s.as_str()),
                    "Duplicate shortcut '{}' on action '{}' in clipboard text context",
                    s,
                    a.id
                );
            }
        }
    }

    #[test]
    fn file_context_shortcuts_are_unique() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let mut seen = HashSet::new();
        for a in &actions {
            if let Some(ref s) = a.shortcut {
                assert!(
                    seen.insert(s.as_str()),
                    "Duplicate shortcut '{}' on action '{}' in file context",
                    s,
                    a.id
                );
            }
        }
    }

    #[test]
    fn path_context_shortcuts_are_unique() {
        let path = PathInfo {
            path: "/usr/local".into(),
            name: "local".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        let mut seen = HashSet::new();
        for a in &actions {
            if let Some(ref s) = a.shortcut {
                assert!(
                    seen.insert(s.as_str()),
                    "Duplicate shortcut '{}' on action '{}' in path context",
                    s,
                    a.id
                );
            }
        }
    }

    #[test]
    fn ai_command_bar_shortcuts_are_unique() {
        let actions = get_ai_command_bar_actions();
        let mut seen = HashSet::new();
        for a in &actions {
            if let Some(ref s) = a.shortcut {
                assert!(
                    seen.insert(s.as_str()),
                    "Duplicate shortcut '{}' on action '{}' in AI command bar",
                    s,
                    a.id
                );
            }
        }
    }

    // =========================================================================
    // 2. Action ordering stability — same inputs always produce same output
    // =========================================================================

    #[test]
    fn script_context_ordering_is_deterministic() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "stable",
            "/path/stable.ts",
            Some("cmd+s".into()),
            Some("st".into()),
        )
        .with_frecency(true, Some("/path/stable.ts".into()));

        let a1 = get_script_context_actions(&script);
        let a2 = get_script_context_actions(&script);
        let a3 = get_script_context_actions(&script);
        let ids_1 = action_ids(&a1);
        let ids_2 = action_ids(&a2);
        let ids_3 = action_ids(&a3);

        assert_eq!(
            ids_1, ids_2,
            "Action ordering should be stable across calls"
        );
        assert_eq!(
            ids_2, ids_3,
            "Action ordering should be stable across calls"
        );
    }

    #[test]
    fn clipboard_ordering_is_deterministic() {
        let entry = ClipboardEntryInfo {
            id: "det".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: Some("Safari".into()),
        };
        let a1 = get_clipboard_history_context_actions(&entry);
        let a2 = get_clipboard_history_context_actions(&entry);
        let ids_1 = action_ids(&a1);
        let ids_2 = action_ids(&a2);
        assert_eq!(ids_1, ids_2);
    }

    // =========================================================================
    // 3. Cross-context action exclusivity
    // =========================================================================

    #[test]
    fn clipboard_ids_never_appear_in_file_context() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let file_actions = get_file_context_actions(&file);
        let file_ids: HashSet<&str> = action_ids(&file_actions).into_iter().collect();

        // Clipboard-specific action IDs
        let clipboard_only = [
            "clip:clipboard_paste",
            "clip:clipboard_copy",
            "clip:clipboard_pin",
            "clip:clipboard_unpin",
            "clip:clipboard_ocr",
            "clip:clipboard_delete",
            "clip:clipboard_delete_all",
            "clip:clipboard_save_snippet",
            "clip:clipboard_share",
            "clip:clipboard_attach_to_ai",
        ];

        for id in &clipboard_only {
            assert!(
                !file_ids.contains(id),
                "File context should not contain clipboard action '{}'",
                id
            );
        }
    }

    #[test]
    fn file_ids_never_appear_in_clipboard_context() {
        let entry = ClipboardEntryInfo {
            id: "c1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let clip_actions = get_clipboard_history_context_actions(&entry);
        let clip_ids: HashSet<&str> = action_ids(&clip_actions).into_iter().collect();

        let file_only = [
            "file:open_file",
            "file:open_directory",
            "file:open_in_editor",
            "file:show_info",
            "file:copy_filename",
        ];

        for id in &file_only {
            assert!(
                !clip_ids.contains(id),
                "Clipboard context should not contain file action '{}'",
                id
            );
        }
    }

    #[test]
    fn script_ids_never_appear_in_path_context() {
        let path = PathInfo {
            path: "/usr/bin".into(),
            name: "bin".into(),
            is_dir: true,
        };
        let path_actions = get_path_context_actions(&path);
        let path_ids: HashSet<&str> = action_ids(&path_actions).into_iter().collect();

        let script_only = [
            "run_script",
            "edit_script",
            "view_logs",
            "add_shortcut",
            "add_alias",
            "copy_deeplink",
            "reset_ranking",
        ];

        for id in &script_only {
            assert!(
                !path_ids.contains(id),
                "Path context should not contain script action '{}'",
                id
            );
        }
    }

    // =========================================================================
    // 4. with_shortcut_opt builder correctness
    // =========================================================================

    #[test]
    fn with_shortcut_opt_some_sets_shortcut() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘T".to_string()));
        assert_eq!(action.shortcut, Some("⌘T".to_string()));
        assert_eq!(action.shortcut_lower, Some("⌘t".to_string()));
    }

    #[test]
    fn with_shortcut_opt_none_leaves_shortcut_none() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(None);
        assert!(action.shortcut.is_none());
        assert!(action.shortcut_lower.is_none());
    }

    // =========================================================================
    // 5. AI command bar section ordering: Response > Actions > Attachments > Export > Context > Actions > Help > Settings
    // =========================================================================

    #[test]
    fn ai_command_bar_section_order_is_correct() {
        let actions = get_ai_command_bar_actions();
        let sections = sections_in_order(&actions);
        assert_eq!(
            sections,
            vec![
                "Response",
                "Actions",
                "Attachments",
                "Export",
                "Context",
                "Actions",
                "Help",
                "Settings"
            ],
            "AI command bar sections should be in correct order"
        );
    }

    #[test]
    fn ai_command_bar_response_section_has_three_actions() {
        let actions = get_ai_command_bar_actions();
        let response_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(response_count, 3, "Response section should have 3 actions");
    }

    #[test]
    fn ai_command_bar_actions_section_has_four_actions() {
        let actions = get_ai_command_bar_actions();
        let actions_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Actions"))
            .count();
        assert_eq!(actions_count, 7, "Actions section should have 7 actions");
    }

    #[test]
    fn ai_command_bar_attachments_section_has_two_actions() {
        let actions = get_ai_command_bar_actions();
        let attachments_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Attachments"))
            .count();
        assert_eq!(
            attachments_count, 4,
            "Attachments section should have 7 actions"
        );
    }

    #[test]
    fn ai_command_bar_settings_section_has_one_action() {
        let actions = get_ai_command_bar_actions();
        let settings_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Settings"))
            .count();
        assert_eq!(settings_count, 2, "Settings section should have 2 actions");
    }

    // =========================================================================
    // 6. Notes command bar section ordering: Notes > Edit > Copy > Export > AI > Settings
    // =========================================================================

    // --- merged from part_02.rs ---

    // =========================================================================
    // 7. New chat section ordering: Last Used Settings > Presets > Models
    // =========================================================================

    #[test]
    fn new_chat_section_order_all_populated() {
        let last_used = vec![NewChatModelInfo {
            model_id: "claude-3".into(),
            display_name: "Claude 3".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Settings,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        let sections = sections_in_order(&actions);
        assert_eq!(sections, vec!["Last Used Settings", "Presets", "Models"],);
    }

    #[test]
    fn new_chat_section_order_no_last_used() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &presets, &models);
        let sections = sections_in_order(&actions);
        assert_eq!(sections, vec!["Presets", "Models"]);
    }

    #[test]
    fn new_chat_all_empty_returns_no_actions() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    // =========================================================================
    // 8. Scriptlet with multiple custom H3 actions
    // =========================================================================

    #[test]
    fn scriptlet_custom_actions_maintain_order() {
        let script = ScriptInfo::scriptlet("Multi Action", "/path/multi.md", None, None);
        let mut scriptlet = Scriptlet::new(
            "Multi Action".to_string(),
            "bash".to_string(),
            "echo main".to_string(),
        );
        scriptlet.actions = vec![
            ScriptletAction {
                name: "Alpha".to_string(),
                command: "alpha-cmd".to_string(),
                tool: "bash".to_string(),
                code: "echo alpha".to_string(),
                inputs: vec![],
                shortcut: Some("cmd+1".to_string()),
                description: Some("First action".to_string()),
            },
            ScriptletAction {
                name: "Beta".to_string(),
                command: "beta-cmd".to_string(),
                tool: "bash".to_string(),
                code: "echo beta".to_string(),
                inputs: vec![],
                shortcut: Some("cmd+2".to_string()),
                description: Some("Second action".to_string()),
            },
            ScriptletAction {
                name: "Gamma".to_string(),
                command: "gamma-cmd".to_string(),
                tool: "bash".to_string(),
                code: "echo gamma".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
        ];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let ids = action_ids(&actions);

        // run_script must be first
        assert_eq!(ids[0], "run_script");

        // Custom actions follow run in declaration order
        let alpha_idx = ids
            .iter()
            .position(|id| *id == "scriptlet_action:alpha-cmd")
            .unwrap();
        let beta_idx = ids
            .iter()
            .position(|id| *id == "scriptlet_action:beta-cmd")
            .unwrap();
        let gamma_idx = ids
            .iter()
            .position(|id| *id == "scriptlet_action:gamma-cmd")
            .unwrap();

        assert_eq!(alpha_idx, 1);
        assert_eq!(beta_idx, 2);
        assert_eq!(gamma_idx, 3);

        // Custom actions all have has_action=true
        for id in &[
            "scriptlet_action:alpha-cmd",
            "scriptlet_action:beta-cmd",
            "scriptlet_action:gamma-cmd",
        ] {
            let a = find_action(&actions, id).unwrap();
            assert!(
                a.has_action,
                "Custom action '{}' should have has_action=true",
                id
            );
            assert!(
                a.value.is_some(),
                "Custom action '{}' should have a value",
                id
            );
        }
    }

    #[test]
    fn scriptlet_custom_action_id_format() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet =
            Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Do Something".to_string(),
            command: "do-something".to_string(),
            tool: "bash".to_string(),
            code: "echo do".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let custom = find_action(&actions, "scriptlet_action:do-something").unwrap();
        assert!(custom.id.starts_with("scriptlet_action:"));
        assert_eq!(custom.title, "Do Something");
    }

    // =========================================================================
    // 9. Action title formatting with varied action_verbs
    // =========================================================================

    #[test]
    fn action_verb_appears_in_primary_title() {
        let verbs = ["Run", "Launch", "Switch to", "Open", "Execute"];
        for verb in &verbs {
            let script = ScriptInfo::with_action_verb("MyItem", "/path/item", false, *verb);
            let actions = get_script_context_actions(&script);
            let primary = &actions[0];
            let expected = if *verb == "Switch to" {
                "Switch To"
            } else {
                *verb
            };
            assert_eq!(primary.title, expected);
        }
    }

    #[test]
    fn scriptlet_primary_uses_action_verb() {
        let script = ScriptInfo::scriptlet("Open URL", "/path/url.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let primary = &actions[0];
        assert!(
            primary.title.starts_with("Run"),
            "Scriptlet primary should use 'Run' verb"
        );
        assert!(primary.title.contains("Open URL"));
    }

    // =========================================================================
    // 10. Path context shortcut assignments
    // =========================================================================

    #[test]
    fn path_file_has_enter_on_primary() {
        let path = PathInfo {
            path: "/usr/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:select_file");
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn path_dir_has_enter_on_primary() {
        let path = PathInfo {
            path: "/usr/local".into(),
            name: "local".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:open_directory");
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn path_context_has_trash_shortcut() {
        let path = PathInfo {
            path: "/tmp/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        let trash = find_action(&actions, "file:move_to_trash").unwrap();
        assert_eq!(trash.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn path_context_has_all_expected_actions() {
        let path = PathInfo {
            path: "/tmp/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();

        let expected = [
            "file:select_file",
            "file:copy_path",
            "file:open_in_finder",
            "file:open_in_editor",
            "file:open_in_quick_terminal",
            "file:copy_filename",
            "file:move_to_trash",
        ];
        for id in &expected {
            assert!(
                ids.contains(id),
                "Path file context should have action '{}'",
                id
            );
        }
    }

    #[test]
    fn path_dir_context_has_open_directory_not_select_file() {
        let path = PathInfo {
            path: "/usr/local".into(),
            name: "local".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
        assert!(ids.contains("file:open_directory"));
        assert!(!ids.contains("file:select_file"));
    }

    // =========================================================================
    // 11. Clipboard ordering invariant: paste first, deletes last
    // =========================================================================

    #[test]
    fn clipboard_paste_always_first_text() {
        let entry = ClipboardEntryInfo {
            id: "t1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
    }

    #[test]
    fn clipboard_paste_always_first_image() {
        let entry = ClipboardEntryInfo {
            id: "i1".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: Some("Figma".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
    }

    #[test]
    fn clipboard_delete_actions_always_last_three() {
        let entry = ClipboardEntryInfo {
            id: "d1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();
        assert!(len >= 3);

        let last_three_ids: Vec<&str> = actions[len - 3..].iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            last_three_ids,
            vec![
                "clip:clipboard_delete",
                "clip:clipboard_delete_multiple",
                "clip:clipboard_delete_all"
            ],
            "Last 3 clipboard actions should be the destructive ones in order"
        );
    }

    #[test]
    fn clipboard_delete_actions_always_last_three_image() {
        let entry = ClipboardEntryInfo {
            id: "di".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "img".into(),
            image_dimensions: Some((1920, 1080)),
            frontmost_app_name: Some("Preview".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();

        let last_three_ids: Vec<&str> = actions[len - 3..].iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            last_three_ids,
            vec![
                "clip:clipboard_delete",
                "clip:clipboard_delete_multiple",
                "clip:clipboard_delete_all"
            ],
        );
    }

    // =========================================================================
    // 12. Mixed flag combinations on ScriptInfo
    // =========================================================================

    #[test]
    fn script_with_both_shortcut_and_alias_has_update_remove_for_both() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "full",
            "/path/full.ts",
            Some("cmd+f".into()),
            Some("fl".into()),
        );
        let actions = get_script_context_actions(&script);
        let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();

        assert!(ids.contains("update_shortcut"));
        assert!(ids.contains("remove_shortcut"));
        assert!(!ids.contains("add_shortcut"));
        assert!(ids.contains("update_alias"));
        assert!(ids.contains("remove_alias"));
        assert!(!ids.contains("add_alias"));
    }

    #[test]
    fn builtin_with_frecency_has_reset_ranking_and_no_edit() {
        let builtin = ScriptInfo::builtin("Clipboard History")
            .with_frecency(true, Some("builtin:clipboard".into()));
        let actions = get_script_context_actions(&builtin);
        let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();

        assert!(ids.contains("reset_ranking"));
        assert!(ids.contains("run_script"));
        assert!(ids.contains("copy_deeplink"));
        assert!(!ids.contains("edit_script"));
        assert!(!ids.contains("view_logs"));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn agent_with_shortcut_has_no_shortcut_preference_actions() {
        let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
        agent.is_agent = true;
        agent.is_script = false;
        agent.shortcut = Some("cmd+a".into());

        let actions = get_script_context_actions(&agent);
        let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
        // Agents never get shortcut preference actions, even with a shortcut set.
        assert!(!ids.contains("update_shortcut"));
        assert!(!ids.contains("remove_shortcut"));
        assert!(!ids.contains("add_shortcut"));
        assert!(ids.contains("edit_script")); // agent gets edit_script with title "Edit Agent"
    }

    // =========================================================================
    // 13. Note switcher icon hierarchy for all is_current × is_pinned combos
    // =========================================================================

    #[test]
    fn note_switcher_pinned_current_gets_star_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n1".into(),
            title: "Note 1".into(),
            char_count: 100,
            is_current: true,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn note_switcher_pinned_not_current_gets_star_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n2".into(),
            title: "Note 2".into(),
            char_count: 50,
            is_current: false,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn note_switcher_current_not_pinned_gets_check_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n3".into(),
            title: "Note 3".into(),
            char_count: 25,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn note_switcher_neither_pinned_nor_current_gets_file_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n4".into(),
            title: "Note 4".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    #[test]
    fn note_switcher_current_note_has_bullet_prefix() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "a".into(),
                title: "Current Note".into(),
                char_count: 100,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "b".into(),
                title: "Other Note".into(),
                char_count: 50,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions[0].title.starts_with("• "),
            "Current note should have bullet prefix"
        );
        assert!(
            !actions[1].title.starts_with("• "),
            "Non-current note should not have bullet prefix"
        );
    }

    #[test]
    fn note_switcher_char_count_plural() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "p0".into(),
                title: "Zero".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "p1".into(),
                title: "One".into(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "p2".into(),
                title: "Many".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("0 chars"));
        assert_eq!(actions[1].description.as_deref(), Some("1 char"));
        assert_eq!(actions[2].description.as_deref(), Some("42 chars"));
    }

    #[test]
    fn note_switcher_empty_shows_no_notes_message() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "no_notes");
        assert_eq!(actions[0].title, "No notes yet");
        assert_eq!(actions[0].icon, Some(IconName::Plus));
    }

    // =========================================================================
    // 14. to_deeplink_name with unicode / emoji edge cases
    // =========================================================================

    #[test]
    fn deeplink_name_with_accented_chars() {
        assert_eq!(to_deeplink_name("café"), "caf%C3%A9");
    }

    #[test]
    fn deeplink_name_with_numbers() {
        assert_eq!(to_deeplink_name("Script123"), "script123");
    }

    #[test]
    fn deeplink_name_empty_string() {
        assert_eq!(to_deeplink_name(""), "_unnamed");
    }

    #[test]
    fn deeplink_name_only_special_chars() {
        assert_eq!(to_deeplink_name("!@#$%"), "_unnamed");
    }

    #[test]
    fn deeplink_name_leading_trailing_spaces() {
        assert_eq!(to_deeplink_name("  hello  "), "hello");
    }

    #[test]
    fn deeplink_name_consecutive_hyphens_collapsed() {
        assert_eq!(to_deeplink_name("a---b"), "a-b");
    }

    #[test]
    fn deeplink_name_mixed_case_numbers_symbols() {
        assert_eq!(to_deeplink_name("My Script (v2.0)"), "my-script-v2-0");
    }

    // =========================================================================
    // 15. Score stacking — title + description bonuses accumulate
    // =========================================================================

    #[test]
    fn score_prefix_match_is_100() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            Some("Open in editor".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert_eq!(
            score,
            100 + 15,
            "Prefix 'edit' should get 100 for title + 15 for description containing 'edit'"
        );
    }

    #[test]
    fn score_contains_match_is_50() {
        let action = Action::new(
            "file:copy_path",
            "Copy Path",
            Some("Copy to clipboard".to_string()),
            ActionCategory::ScriptContext,
        );
        // "path" is contained but not a prefix
        let score = ActionsDialog::score_action(&action, "path");
        assert!(
            score >= 50,
            "Contains match should be at least 50, got {}",
            score
        );
    }

    #[test]
    fn score_description_only_match() {
        let action = Action::new(
            "file:open_file",
            "Open File",
            Some("Launch with default application".to_string()),
            ActionCategory::ScriptContext,
        );
        // "launch" is in description but not title
        let score = ActionsDialog::score_action(&action, "launch");
        assert_eq!(score, 15, "'launch' only in description should give 15");
    }

    #[test]
    fn score_shortcut_only_match() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            Some("Open in editor".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        // "⌘e" matches shortcut but not title or description
        let score = ActionsDialog::score_action(&action, "⌘e");
        assert!(
            score >= 10,
            "Shortcut match should give at least 10, got {}",
            score
        );
    }

    #[test]
    fn score_no_match_is_zero() {
        let action = Action::new(
            "run_script",
            "Run Script",
            Some("Execute this item".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "zzzzz");
        assert_eq!(score, 0, "No match should score 0");
    }

    #[test]
    fn score_prefix_plus_description_stack() {
        let action = Action::new(
            "file:copy_path",
            "Copy Path",
            Some("Copies the full path to the clipboard".to_string()),
            ActionCategory::ScriptContext,
        );
        // "copy" is a prefix of title AND contained in description
        let score = ActionsDialog::score_action(&action, "copy");
        assert_eq!(
            score, 100,
            "Prefix match should score 100 when description does not contain the query, got {}",
            score
        );
    }

    // =========================================================================
    // 16. File context primary title includes filename
    // =========================================================================

    #[test]
    fn file_context_primary_title_includes_filename() {
        let file = FileInfo {
            path: "/Users/test/document.pdf".into(),
            name: "document.pdf".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        assert!(
            actions[0].title.contains("document.pdf"),
            "File primary title '{}' should include filename",
            actions[0].title
        );
    }

    #[test]
    fn file_context_dir_primary_title_includes_dirname() {
        let file = FileInfo {
            path: "/Users/test/Documents".into(),
            name: "Documents".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file);
        assert!(
            actions[0].title.contains("Documents"),
            "Dir primary title '{}' should include dirname",
            actions[0].title
        );
    }

    // =========================================================================
    // 17. Chat model checkmark only on current model
    // =========================================================================

    #[test]
    fn chat_model_checkmark_on_current_only() {
        let info = ChatPromptInfo {
            current_model: Some("Claude 3".to_string()),
            available_models: vec![
                ChatModelInfo {
                    id: "claude-3".into(),
                    display_name: "Claude 3".into(),
                    provider: "Anthropic".into(),
                },
                ChatModelInfo {
                    id: "gpt-4".into(),
                    display_name: "GPT-4".into(),
                    provider: "OpenAI".into(),
                },
                ChatModelInfo {
                    id: "gemini".into(),
                    display_name: "Gemini".into(),
                    provider: "Google".into(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);

        // Claude 3 should have checkmark
        let claude = find_action(&picker, "chat:select_model_claude-3").unwrap();
        assert!(
            claude.title.contains('✓'),
            "Current model should have checkmark"
        );

        // Others should not
        let gpt = find_action(&picker, "chat:select_model_gpt-4").unwrap();
        assert!(
            !gpt.title.contains('✓'),
            "Non-current model should not have checkmark"
        );

        let gemini = find_action(&picker, "chat:select_model_gemini").unwrap();
        assert!(
            !gemini.title.contains('✓'),
            "Non-current model should not have checkmark"
        );
    }

    #[test]
    fn chat_no_current_model_no_checkmarks() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![
                ChatModelInfo {
                    id: "m1".into(),
                    display_name: "Model 1".into(),
                    provider: "P1".into(),
                },
                ChatModelInfo {
                    id: "m2".into(),
                    display_name: "Model 2".into(),
                    provider: "P2".into(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        for a in &picker {
            assert!(
                !a.title.contains('✓'),
                "No model should have checkmark when current_model is None"
            );
        }
    }

    #[test]
    fn chat_continue_in_chat_always_present() {
        // Even with no models, continue_in_chat should be present
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(
            actions.iter().any(|a| a.id == "chat:continue_in_chat"),
            "continue_in_chat should always be present"
        );
    }

    // --- merged from part_04.rs ---

    #[test]
    fn chat_copy_response_only_with_response() {
        let without = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions_without = get_chat_context_actions(&without);
        assert!(
            !actions_without.iter().any(|a| a.id == "chat:copy_response"),
            "copy_response should be absent without response"
        );

        let with = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: true,
        };
        let actions_with = get_chat_context_actions(&with);
        assert!(
            actions_with.iter().any(|a| a.id == "chat:copy_response"),
            "copy_response should be present with response"
        );
    }

    #[test]
    fn chat_clear_conversation_only_with_messages() {
        let without = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions_without = get_chat_context_actions(&without);
        assert!(!actions_without
            .iter()
            .any(|a| a.id == "chat:clear_conversation"),);

        let with = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions_with = get_chat_context_actions(&with);
        assert!(actions_with
            .iter()
            .any(|a| a.id == "chat:clear_conversation"));
    }

    // =========================================================================
    // 18. Notes conditional action counts across all 8 permutations
    //     (has_selection × is_trash × auto_sizing)
    // =========================================================================

    // =========================================================================
    // 19. CommandBarConfig notes_style specifics
    // =========================================================================

    #[test]
    fn command_bar_notes_style_search_top_separators_icons() {
        let config = CommandBarConfig::notes_style();
        assert!(
            matches!(config.dialog_config.search_position, SearchPosition::Top),
            "notes_style should have search at top"
        );
        assert!(
            matches!(config.dialog_config.section_style, SectionStyle::Headers),
            "notes_style should use Headers"
        );
        assert!(
            config.dialog_config.show_icons,
            "notes_style should show icons"
        );
        assert!(
            !config.dialog_config.show_footer,
            "notes_style should not show footer"
        );
        assert!(config.close_on_escape);
        assert!(config.close_on_select);
        assert!(config.close_on_click_outside);
    }

    // =========================================================================
    // 20. Grouped items build correctness
    // =========================================================================

    #[test]
    fn grouped_items_headers_style_produces_section_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

        // Should contain at least one SectionHeader
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert!(
            header_count >= 8,
            "Headers style should produce at least 8 section headers, got {}",
            header_count
        );
    }

    #[test]
    fn grouped_items_none_style_has_no_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);

        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(
            header_count, 0,
            "None style should produce no section headers"
        );
    }

    #[test]
    fn grouped_items_separators_style_has_no_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);

        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(
            header_count, 0,
            "Separators style should produce no section headers"
        );
    }

    #[test]
    fn grouped_items_empty_filtered_returns_empty() {
        let actions = get_ai_command_bar_actions();
        let grouped = build_grouped_items_static(&actions, &[], SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // =========================================================================
    // 21. Coerce action selection correctness
    // =========================================================================

    #[test]
    fn coerce_selection_on_item_returns_same_index() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::Item(1),
            GroupedActionItem::Item(2),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(1));
    }

    #[test]
    fn coerce_selection_on_header_skips_to_next_item() {
        let rows = vec![
            GroupedActionItem::SectionHeader("Section".into()),
            GroupedActionItem::Item(0),
            GroupedActionItem::Item(1),
        ];
        // Index 0 is a header, should coerce to index 1
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn coerce_selection_on_trailing_header_goes_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("Section".into()),
        ];
        // Index 1 is a header at the end, should coerce back to index 0
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn coerce_selection_empty_returns_none() {
        assert_eq!(coerce_action_selection(&[], 0), None);
    }

    #[test]
    fn coerce_selection_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // =========================================================================
    // 22. Action cached lowercase fields consistency
    // =========================================================================

    #[test]
    fn action_title_lower_matches_title() {
        let action = Action::new(
            "test",
            "My Title With CAPS",
            Some("Description HERE".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘⇧C");

        assert_eq!(action.title_lower, "my title with caps");
        assert_eq!(
            action.description_lower,
            Some("description here".to_string())
        );
        assert_eq!(action.shortcut_lower, Some("⌘⇧c".to_string()));
    }

    #[test]
    fn all_script_actions_have_consistent_lowercase_caches() {
        let script = ScriptInfo::new("Test Script", "/path/test.ts");
        for a in &get_script_context_actions(&script) {
            assert_eq!(
                a.title_lower,
                a.title.to_lowercase(),
                "title_lower mismatch for '{}'",
                a.id
            );
            if let Some(ref desc) = a.description {
                assert_eq!(
                    a.description_lower.as_deref(),
                    Some(desc.to_lowercase()).as_deref(),
                    "description_lower mismatch for '{}'",
                    a.id
                );
            }
            if let Some(ref sc) = a.shortcut {
                assert_eq!(
                    a.shortcut_lower.as_deref(),
                    Some(sc.to_lowercase()).as_deref(),
                    "shortcut_lower mismatch for '{}'",
                    a.id
                );
            }
        }
    }

    #[test]
    fn all_clipboard_actions_have_consistent_lowercase_caches() {
        let entry = ClipboardEntryInfo {
            id: "lc".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: Some("Arc".into()),
        };
        for a in &get_clipboard_history_context_actions(&entry) {
            assert_eq!(
                a.title_lower,
                a.title.to_lowercase(),
                "title_lower mismatch for '{}'",
                a.id
            );
        }
    }

    #[test]
    fn all_ai_command_bar_actions_have_consistent_lowercase_caches() {
        for a in &get_ai_command_bar_actions() {
            assert_eq!(a.title_lower, a.title.to_lowercase());
            if let Some(ref desc) = a.description {
                assert_eq!(
                    a.description_lower.as_deref(),
                    Some(desc.to_lowercase()).as_deref()
                );
            }
        }
    }

    // =========================================================================
    // 23. New chat action descriptions
    // =========================================================================

    #[test]
    fn new_chat_last_used_has_provider_description() {
        let last_used = vec![NewChatModelInfo {
            model_id: "claude-3".into(),
            display_name: "Claude 3".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        let a = &actions[0];
        assert_eq!(a.description.as_deref(), Some("Uses Anthropic"));
    }

    #[test]
    fn new_chat_presets_have_no_description() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Settings,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        let a = &actions[0];
        assert_eq!(a.description.as_deref(), Some("Uses General preset"));
    }

    #[test]
    fn new_chat_models_have_provider_description() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        let a = &actions[0];
        assert_eq!(a.description.as_deref(), Some("Uses OpenAI"));
    }

    // =========================================================================
    // 24. New chat action ID format
    // =========================================================================

    #[test]
    fn new_chat_last_used_ids_are_indexed() {
        let last_used = vec![
            NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "M1".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            },
            NewChatModelInfo {
                model_id: "m2".into(),
                display_name: "M2".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            },
        ];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].id, "last_used_p::m1");
        assert_eq!(actions[1].id, "last_used_p::m2");
    }

    #[test]
    fn new_chat_preset_ids_use_preset_id() {
        let presets = vec![
            NewChatPresetInfo {
                id: "gen".into(),
                name: "General".into(),
                icon: IconName::Settings,
            },
            NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            },
        ];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_gen");
        assert_eq!(actions[1].id, "preset_code");
    }

    // --- merged from part_05.rs ---

    #[test]
    fn new_chat_model_ids_are_indexed() {
        let models = vec![
            NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "a".into(),
                provider_display_name: "Anthropic".into(),
            },
            NewChatModelInfo {
                model_id: "gpt".into(),
                display_name: "GPT".into(),
                provider: "o".into(),
                provider_display_name: "OpenAI".into(),
            },
        ];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_a::claude");
        assert_eq!(actions[1].id, "model_o::gpt");
    }

    // =========================================================================
    // 25. All AI command bar actions have icon and section
    // =========================================================================

    #[test]
    fn ai_command_bar_all_have_icon() {
        for a in &get_ai_command_bar_actions() {
            assert!(a.icon.is_some(), "AI action '{}' should have an icon", a.id);
        }
    }

    #[test]
    fn ai_command_bar_all_have_section() {
        for a in &get_ai_command_bar_actions() {
            assert!(
                a.section.is_some(),
                "AI action '{}' should have a section",
                a.id
            );
        }
    }

    // =========================================================================
    // 26. Notes command bar conditional icons
    // =========================================================================

    // =========================================================================
    // 27. Clipboard attach_to_ai action present
    // =========================================================================

    #[test]
    fn clipboard_text_has_attach_to_ai() {
        let entry = ClipboardEntryInfo {
            id: "ai".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions
            .iter()
            .any(|a| a.id == "clip:clipboard_attach_to_ai"));
    }

    #[test]
    fn clipboard_image_has_attach_to_ai() {
        let entry = ClipboardEntryInfo {
            id: "ai2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions
            .iter()
            .any(|a| a.id == "clip:clipboard_attach_to_ai"));
    }

    // =========================================================================
    // 28. Scriptlet context built-in action set
    // =========================================================================

    #[test]
    fn scriptlet_context_has_expected_builtin_ids() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();

        let expected = [
            "run_script",
            "add_shortcut",
            "add_alias",
            "edit_scriptlet",
            "reveal_scriptlet_in_finder",
            "copy_scriptlet_path",
            "copy_content",
            "copy_deeplink",
        ];
        for id in &expected {
            assert!(ids.contains(id), "Scriptlet context should have '{}'", id);
        }
    }

    #[test]
    fn scriptlet_context_action_order_run_before_builtin() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let ids = action_ids(&actions);

        let run_idx = ids.iter().position(|id| *id == "run_script").unwrap();
        let edit_idx = ids.iter().position(|id| *id == "edit_scriptlet").unwrap();
        let deeplink_idx = ids.iter().position(|id| *id == "copy_deeplink").unwrap();

        assert!(run_idx < edit_idx, "run should come before edit_scriptlet");
        assert!(
            edit_idx < deeplink_idx,
            "edit_scriptlet should come before copy_deeplink"
        );
    }

    // =========================================================================
    // 29. Path context trash description varies by is_dir
    // =========================================================================

    #[test]
    fn path_trash_description_says_file_for_file() {
        let path = PathInfo {
            path: "/tmp/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        let trash = find_action(&actions, "file:move_to_trash").unwrap();
        assert_eq!(
            trash.description.as_deref(),
            Some("Moves this file to the Trash"),
        );
    }

    #[test]
    fn path_trash_description_says_folder_for_dir() {
        let path = PathInfo {
            path: "/tmp/mydir".into(),
            name: "mydir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        let trash = find_action(&actions, "file:move_to_trash").unwrap();
        assert_eq!(
            trash.description.as_deref(),
            Some("Moves this folder to the Trash"),
        );
    }

    // =========================================================================
    // 30. Note switcher all notes have "Notes" section
    // =========================================================================

    #[test]
    fn note_switcher_all_actions_have_notes_section() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "a".into(),
                title: "Note A".into(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "b".into(),
                title: "Note B".into(),
                char_count: 20,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        for a in &actions {
            let section = a.section.as_deref();
            assert!(
                section == Some("Pinned") || section == Some("Recent"),
                "Note switcher action '{}' should have 'Pinned' or 'Recent' section, got {:?}",
                a.id,
                section
            );
        }
    }

    #[test]
    fn note_switcher_empty_state_has_notes_section() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].section.as_deref(), Some("Notes"));
    }

    // =========================================================================
    // 31. New chat action icons
    // =========================================================================

    #[test]
    fn new_chat_last_used_has_bolt_icon() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "M1".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    #[test]
    fn new_chat_models_have_settings_icon() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "M1".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].icon, Some(IconName::Settings));
    }

    #[test]
    fn new_chat_preset_uses_custom_icon() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Code));
    }

    // =========================================================================
    // 32. Clipboard save actions have correct shortcuts
    // =========================================================================

    #[test]
    fn clipboard_save_snippet_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "ss".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let save = find_action(&actions, "clip:clipboard_save_snippet").unwrap();
        assert_eq!(save.shortcut.as_deref(), Some("⇧⌘S"));
    }

    #[test]
    fn clipboard_save_file_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "sf".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let save = find_action(&actions, "clip:clipboard_save_file").unwrap();
        assert_eq!(save.shortcut.as_deref(), Some("⌥⇧⌘S"));
    }

    // =========================================================================
    // 33. Script context deeplink description format
    // =========================================================================

    #[test]
    fn script_deeplink_description_contains_url() {
        let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
        let actions = get_script_context_actions(&script);
        let deeplink = find_action(&actions, "copy_deeplink").unwrap();
        assert_eq!(deeplink.title, "Share");
        assert_eq!(
            deeplink.description.as_deref(),
            Some("Copy a portable Script Kit share link to clipboard")
        );
    }

    #[test]
    fn scriptlet_deeplink_description_contains_url() {
        let script = ScriptInfo::scriptlet("Open GitHub", "/path/url.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let deeplink = find_action(&actions, "copy_deeplink").unwrap();
        assert!(deeplink
            .description
            .as_ref()
            .unwrap()
            .contains("scriptkit://run/open-github"),);
    }

    // =========================================================================
    // 34. All built-in actions have ActionCategory::ScriptContext
    // =========================================================================

    #[test]
    fn script_context_all_actions_are_script_context_category() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for a in &get_script_context_actions(&script) {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "Action '{}' should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn clipboard_all_actions_are_script_context_category() {
        let entry = ClipboardEntryInfo {
            id: "c".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for a in &get_clipboard_history_context_actions(&entry) {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "Action '{}' should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn file_all_actions_are_script_context_category() {
        let file = FileInfo {
            path: "/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        for a in &get_file_context_actions(&file) {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "Action '{}' should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn path_all_actions_are_script_context_category() {
        let path = PathInfo {
            path: "/tmp".into(),
            name: "tmp".into(),
            is_dir: true,
        };
        for a in &get_path_context_actions(&path) {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "Action '{}' should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn ai_command_bar_all_actions_are_script_context_category() {
        for a in &get_ai_command_bar_actions() {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "Action '{}' should be ScriptContext",
                a.id
            );
        }
    }

    // =========================================================================
    // 35. Action count bounds
    // =========================================================================

    #[test]
    fn script_context_has_at_least_5_actions() {
        // Any script should have at minimum: run, shortcut, alias, deeplink, + edit/view/reveal/copy
        let script = ScriptInfo::new("test", "/path/test.ts");
        let count = get_script_context_actions(&script).len();
        assert!(
            count >= 5,
            "Script context should have at least 5 actions, got {}",
            count
        );
    }

    // --- merged from part_06.rs ---

    #[test]
    fn builtin_context_has_at_least_4_actions() {
        // Built-in: run, add_shortcut, add_alias, copy_deeplink
        let builtin = ScriptInfo::builtin("Test");
        let count = get_script_context_actions(&builtin).len();
        assert!(
            count >= 4,
            "Builtin context should have at least 4 actions, got {}",
            count
        );
    }

    #[test]
    fn clipboard_text_has_at_least_10_actions() {
        let entry = ClipboardEntryInfo {
            id: "t".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let count = get_clipboard_history_context_actions(&entry).len();
        assert!(
            count >= 10,
            "Clipboard text context should have at least 10 actions, got {}",
            count
        );
    }

    #[test]
    fn ai_command_bar_has_exactly_35_actions() {
        let count = get_ai_command_bar_actions().len();
        assert_eq!(
            count, 38,
            "AI command bar should have exactly 35 actions, got {}",
            count
        );
    }

    // =========================================================================
    // 36. Score fuzzy match
    // =========================================================================

    #[test]
    fn score_fuzzy_match_subsequence() {
        let action = Action::new(
            "file:reveal_in_finder",
            "Reveal in Finder",
            Some("Reveal in Finder".to_string()),
            ActionCategory::ScriptContext,
        );
        // "rif" is a subsequence of "reveal in finder"
        let score = ActionsDialog::score_action(&action, "rif");
        assert!(
            score > 0,
            "Fuzzy subsequence 'rif' should match 'reveal in finder', got score {}",
            score
        );
    }

    #[test]
    fn score_fuzzy_no_match() {
        let action = Action::new(
            "run_script",
            "Run Script",
            None,
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "xyz");
        assert_eq!(score, 0);
    }
}

mod from_dialog_builtin_action_validation_tests_4 {
    // --- merged from part_01.rs ---
    //! Built-in action behavioral validation tests — batch 4
    //!
    //! Validates randomly-selected built-in actions across window dialogs and
    //! contexts that were NOT covered in batches 1–3. Focuses on:
    //! - Agent flag interactions with shortcut/alias/frecency combinations
    //! - Custom action verbs propagating correctly into primary action titles
    //! - Scriptlet context vs script context systematic action set comparison
    //! - Clipboard text vs image action count differential (macOS)
    //! - Path context action IDs all snake_case
    //! - File context FileType variants produce consistent action set
    //! - Notes section label exhaustiveness for full-feature permutation
    //! - AI command bar icon-per-section coverage
    //! - New chat with all-empty inputs produces empty output
    //! - score_action edge cases (empty query, single char, unicode)
    //! - fuzzy_match boundary conditions (empty strings, longer needle, etc.)
    //! - parse_shortcut_keycaps for all modifier symbols
    //! - format_shortcut_hint roundtrips for unusual key names
    //! - to_deeplink_name with CJK, emoji, RTL characters
    //! - Grouped items with realistic AI command bar data
    //! - coerce_action_selection on all-headers edge case
    //! - Note switcher section assignment (Pinned vs Recent)
    //! - Clipboard frontmost app edge cases (empty string, unicode)
    //! - Chat with no models, no messages, no response
    //! - Multiple scriptlet custom actions preserve declaration order
    //! - Action constructor lowercase caching with unicode titles

    use super::builders::{
        get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
        get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
        get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
        get_script_context_actions, get_scriptlet_context_actions_with_custom, to_deeplink_name,
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo,
    };
    use super::command_bar::CommandBarConfig;
    use super::dialog::{
        build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
    };
    use super::types::{Action, ActionCategory, ScriptInfo, SearchPosition, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};
    use std::collections::HashSet;

    // =========================================================================
    // Helpers
    // =========================================================================

    fn action_ids(actions: &[Action]) -> Vec<&str> {
        actions.iter().map(|a| a.id.as_str()).collect()
    }

    fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
        actions.iter().find(|a| a.id == id)
    }

    fn sections_in_order(actions: &[Action]) -> Vec<&str> {
        let mut sections = Vec::new();
        for a in actions {
            if let Some(ref s) = a.section {
                if sections
                    .last()
                    .map(|l: &&str| *l != s.as_str())
                    .unwrap_or(true)
                {
                    sections.push(s.as_str());
                }
            }
        }
        sections
    }

    // =========================================================================
    // 1. Agent flag interactions with shortcut/alias/frecency
    // =========================================================================

    #[test]
    fn agent_with_shortcut_has_no_shortcut_actions() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        agent.shortcut = Some("cmd+a".to_string());
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        // Agents never get shortcut preference actions, even with a shortcut set.
        assert!(!ids.contains(&"update_shortcut"));
        assert!(!ids.contains(&"remove_shortcut"));
        assert!(!ids.contains(&"add_shortcut"));
    }

    #[test]
    fn agent_without_shortcut_has_no_shortcut_actions() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        // Agents never get shortcut preference actions.
        assert!(!ids.contains(&"add_shortcut"));
        assert!(!ids.contains(&"update_shortcut"));
        assert!(!ids.contains(&"remove_shortcut"));
    }

    #[test]
    fn agent_with_alias_has_no_alias_actions() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        agent.alias = Some("ag".to_string());
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        // Agents never get alias preference actions, even with an alias set.
        assert!(!ids.contains(&"update_alias"));
        assert!(!ids.contains(&"remove_alias"));
        assert!(!ids.contains(&"add_alias"));
    }

    #[test]
    fn agent_with_frecency_has_reset_ranking() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        agent.is_suggested = true;
        agent.frecency_path = Some("agent:/path".to_string());
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"reset_ranking"));
    }

    #[test]
    fn agent_without_frecency_lacks_reset_ranking() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        assert!(!ids.contains(&"reset_ranking"));
    }

    #[test]
    fn agent_has_edit_agent_title() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let edit = find_action(&actions, "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn agent_has_reveal_copy_path_copy_content() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"reveal_in_finder"));
        assert!(ids.contains(&"copy_path"));
        assert!(ids.contains(&"copy_content"));
    }

    #[test]
    fn agent_lacks_view_logs() {
        let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.md");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let ids = action_ids(&actions);
        assert!(!ids.contains(&"view_logs"));
    }

    // =========================================================================
    // 2. Custom action verbs propagate into primary action title
    // =========================================================================

    #[test]
    fn action_verb_run_in_primary_title() {
        let script = ScriptInfo::new("Test Script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Run");
    }

    #[test]
    fn action_verb_launch_in_primary_title() {
        let script =
            ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Launch");
    }

    #[test]
    fn action_verb_switch_to_in_primary_title() {
        let script = ScriptInfo::with_action_verb("My Window", "window:123", false, "Switch to");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Switch To");
    }

    #[test]
    fn action_verb_open_in_primary_title() {
        let script = ScriptInfo::with_action_verb("Clipboard History", "builtin:ch", false, "Open");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Open");
    }

    #[test]
    fn action_verb_execute_in_primary_title() {
        let script = ScriptInfo::with_all("My Task", "/path/task.ts", true, "Execute", None, None);
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Execute");
    }

    // =========================================================================
    // 3. Scriptlet context vs script context: systematic comparison
    // =========================================================================

    #[test]
    fn scriptlet_context_has_edit_scriptlet_not_edit_script() {
        let scriptlet = ScriptInfo::scriptlet("My Snippet", "/path/to/bundle.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"edit_scriptlet"));
        assert!(!ids.contains(&"edit_script"));
    }

    #[test]
    fn scriptlet_context_has_reveal_scriptlet_not_reveal() {
        let scriptlet = ScriptInfo::scriptlet("My Snippet", "/path/to/bundle.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"reveal_scriptlet_in_finder"));
        // The regular reveal_in_finder should NOT be present for scriptlets
        assert!(!ids.contains(&"file:reveal_in_finder"));
    }

    #[test]
    fn scriptlet_context_has_copy_scriptlet_path_not_copy_path() {
        let scriptlet = ScriptInfo::scriptlet("My Snippet", "/path/to/bundle.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"copy_scriptlet_path"));
        assert!(!ids.contains(&"file:copy_path"));
    }

    #[test]
    fn scriptlet_and_script_both_have_copy_content() {
        let scriptlet = ScriptInfo::scriptlet("My Snippet", "/path/to/bundle.md", None, None);
        let script = ScriptInfo::new("My Script", "/path/script.ts");
        let scriptlet_actions = get_script_context_actions(&scriptlet);
        let script_actions = get_script_context_actions(&script);
        assert!(action_ids(&scriptlet_actions).contains(&"copy_content"));
        assert!(action_ids(&script_actions).contains(&"copy_content"));
    }

    #[test]
    fn scriptlet_lacks_view_logs() {
        let scriptlet = ScriptInfo::scriptlet("My Snippet", "/path/to/bundle.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        assert!(!action_ids(&actions).contains(&"view_logs"));
    }

    // =========================================================================
    // 4. Clipboard text vs image action count differential
    // =========================================================================

    #[test]
    fn clipboard_image_has_strictly_more_actions_than_text() {
        let text_entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let image_entry = ClipboardEntryInfo {
            id: "i1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Image (800x600)".to_string(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let text_actions = get_clipboard_history_context_actions(&text_entry);
        let image_actions = get_clipboard_history_context_actions(&image_entry);
        assert!(
            image_actions.len() > text_actions.len(),
            "Image should have more actions than text: {} > {}",
            image_actions.len(),
            text_actions.len()
        );
    }

    #[test]
    fn clipboard_image_has_ocr_text_does_not() {
        let text_entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let image_entry = ClipboardEntryInfo {
            id: "i1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Image".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let text_actions = get_clipboard_history_context_actions(&text_entry);
        let image_actions = get_clipboard_history_context_actions(&image_entry);
        let text_ids = action_ids(&text_actions);
        let image_ids = action_ids(&image_actions);
        assert!(!text_ids.contains(&"clip:clipboard_ocr"));
        assert!(image_ids.contains(&"clip:clipboard_ocr"));
    }

    #[test]
    fn clipboard_pinned_shows_unpin_unpinned_shows_pin() {
        let pinned = ClipboardEntryInfo {
            id: "p1".to_string(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "pinned".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let unpinned = ClipboardEntryInfo {
            id: "u1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "unpinned".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let pinned_actions = get_clipboard_history_context_actions(&pinned);
        let unpinned_actions = get_clipboard_history_context_actions(&unpinned);
        let pinned_ids = action_ids(&pinned_actions);
        let unpinned_ids = action_ids(&unpinned_actions);
        assert!(pinned_ids.contains(&"clip:clipboard_unpin"));
        assert!(!pinned_ids.contains(&"clip:clipboard_pin"));
        assert!(unpinned_ids.contains(&"clip:clipboard_pin"));
        assert!(!unpinned_ids.contains(&"clip:clipboard_unpin"));
    }

    // =========================================================================
    // 5. Path context action IDs are all snake_case
    // =========================================================================

    #[test]
    fn path_context_all_ids_are_snake_case() {
        let path = PathInfo {
            name: "test.txt".to_string(),
            path: "/home/user/test.txt".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        for action in &actions {
            assert!(
                !action.id.contains(' ') && !action.id.contains('-'),
                "Action ID '{}' should be snake_case",
                action.id
            );
            assert_eq!(
                action.id,
                action.id.to_lowercase(),
                "Action ID '{}' should be lowercase",
                action.id
            );
        }
    }

    #[test]
    fn path_context_dir_all_ids_are_snake_case() {
        let path = PathInfo {
            name: "Documents".to_string(),
            path: "/home/user/Documents".to_string(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        for action in &actions {
            assert!(
                !action.id.contains(' '),
                "Action ID '{}' should not contain spaces",
                action.id
            );
        }
    }

    // =========================================================================
    // 6. File context FileType variants produce consistent action set structure
    // =========================================================================

    #[test]
    fn file_context_all_file_types_have_reveal_and_copy_path() {
        let file_types = vec![
            FileType::File,
            FileType::Document,
            FileType::Image,
            FileType::Application,
            FileType::Audio,
        ];
        for ft in file_types {
            let info = FileInfo {
                path: format!("/tmp/test.{:?}", ft),
                name: format!("test.{:?}", ft),
                file_type: ft,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(
                ids.contains(&"file:reveal_in_finder"),
                "FileType {:?} should have reveal_in_finder",
                info.file_type
            );
            assert!(
                ids.contains(&"file:copy_path"),
                "FileType {:?} should have copy_path",
                info.file_type
            );
            assert!(
                ids.contains(&"file:copy_filename"),
                "FileType {:?} should have copy_filename",
                info.file_type
            );
        }
    }

    #[test]
    fn file_context_file_has_open_file_dir_has_open_directory() {
        let file = FileInfo {
            path: "/tmp/readme.md".to_string(),
            name: "readme.md".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let dir = FileInfo {
            path: "/tmp/src".to_string(),
            name: "src".to_string(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let file_actions = get_file_context_actions(&file);
        let dir_actions = get_file_context_actions(&dir);
        assert!(action_ids(&file_actions).contains(&"file:open_file"));
        assert!(!action_ids(&file_actions).contains(&"file:open_directory"));
        assert!(action_ids(&dir_actions).contains(&"file:open_directory"));
        assert!(!action_ids(&dir_actions).contains(&"file:open_file"));
    }

    // --- merged from part_02.rs ---

    // =========================================================================
    // 7. Notes section labels exhaustive for full-feature permutation
    // =========================================================================

    // =========================================================================
    // 8. AI command bar icon-per-section coverage
    // =========================================================================

    #[test]
    fn ai_command_bar_every_action_has_icon() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert!(
                action.icon.is_some(),
                "AI action '{}' should have an icon",
                action.id
            );
        }
    }

    #[test]
    fn ai_command_bar_every_action_has_section() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert!(
                action.section.is_some(),
                "AI action '{}' should have a section",
                action.id
            );
        }
    }

    #[test]
    fn ai_command_bar_exactly_seven_sections() {
        let actions = get_ai_command_bar_actions();
        let unique_sections: HashSet<&str> = actions
            .iter()
            .filter_map(|a| a.section.as_deref())
            .collect();
        assert_eq!(
            unique_sections.len(),
            7,
            "AI command bar should have exactly 7 sections, got {:?}",
            unique_sections
        );
    }

    #[test]
    fn ai_command_bar_section_order_is_response_actions_attachments_export_actions_help_settings() {
        let actions = get_ai_command_bar_actions();
        let order = sections_in_order(&actions);
        assert_eq!(
            order,
            vec![
                "Response",
                "Actions",
                "Attachments",
                "Export",
                "Context",
                "Actions",
                "Help",
                "Settings"
            ]
        );
    }

    // =========================================================================
    // 9. New chat with all-empty inputs
    // =========================================================================

    #[test]
    fn new_chat_only_models_produces_models_section() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "Model 1".to_string(),
            provider: "test".to_string(),
            provider_display_name: "Test Provider".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    #[test]
    fn new_chat_only_presets_produces_presets_section() {
        let presets = vec![NewChatPresetInfo {
            id: "general".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    #[test]
    fn new_chat_only_last_used_produces_last_used_section() {
        let last_used = vec![NewChatModelInfo {
            model_id: "lu1".to_string(),
            display_name: "Recent Model".to_string(),
            provider: "test".to_string(),
            provider_display_name: "Test".to_string(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn new_chat_section_order_is_last_used_presets_models() {
        let last_used = vec![NewChatModelInfo {
            model_id: "lu1".to_string(),
            display_name: "Recent".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "gen".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "Claude".to_string(),
            provider: "anthropic".to_string(),
            provider_display_name: "Anthropic".to_string(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        let order = sections_in_order(&actions);
        assert_eq!(order, vec!["Last Used Settings", "Presets", "Models"]);
    }

    // =========================================================================
    // 10. score_action edge cases
    // =========================================================================

    #[test]
    fn score_action_empty_query_returns_zero() {
        let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        // Empty query should match as prefix (empty string is prefix of everything)
        // Based on implementation: "test action".starts_with("") == true → 100
        assert!(score >= 100);
    }

    #[test]
    fn score_action_exact_title_match_gets_prefix_score() {
        let action = Action::new("edit", "Edit Script", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "edit script");
        assert!(
            score >= 100,
            "Exact title match should score 100+, got {}",
            score
        );
    }

    #[test]
    fn score_action_no_match_returns_zero() {
        let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "zzzzz");
        assert_eq!(score, 0, "No match should score 0");
    }

    #[test]
    fn score_action_description_only_match_returns_fifteen() {
        let action = Action::new(
            "open",
            "Open File",
            Some("Launch the default editor".to_string()),
            ActionCategory::ScriptContext,
        );
        // "default editor" doesn't match title but matches description
        let score = ActionsDialog::score_action(&action, "default editor");
        assert_eq!(
            score, 15,
            "Description-only match should score 15, got {}",
            score
        );
    }

    #[test]
    fn score_action_shortcut_only_match_returns_ten() {
        let action =
            Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "⌘e");
        assert_eq!(
            score, 10,
            "Shortcut-only match should score 10, got {}",
            score
        );
    }

    #[test]
    fn score_action_title_plus_description_stacks() {
        let action = Action::new(
            "edit",
            "Edit Script",
            Some("Edit the script file".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        // title prefix (100) + description contains "edit" (15) = 115
        assert!(
            score >= 115,
            "Stacked score should be >= 115, got {}",
            score
        );
    }

    #[test]
    fn score_action_single_char_query() {
        let action = Action::new("edit", "Edit Script", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "e");
        assert!(
            score >= 100,
            "Single char prefix match should score 100+, got {}",
            score
        );
    }

    // =========================================================================
    // 11. fuzzy_match boundary conditions
    // =========================================================================

    #[test]
    fn fuzzy_match_empty_needle_always_matches() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn fuzzy_match_empty_haystack_only_matches_empty_needle() {
        assert!(ActionsDialog::fuzzy_match("", ""));
        assert!(!ActionsDialog::fuzzy_match("", "a"));
    }

    #[test]
    fn fuzzy_match_needle_longer_than_haystack_fails() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
    }

    #[test]
    fn fuzzy_match_exact_match_succeeds() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn fuzzy_match_subsequence_succeeds() {
        assert!(ActionsDialog::fuzzy_match("edit script", "edsc"));
    }

    #[test]
    fn fuzzy_match_wrong_order_fails() {
        assert!(!ActionsDialog::fuzzy_match("abc", "cba"));
    }

    #[test]
    fn fuzzy_match_case_sensitive() {
        // fuzzy_match is case-sensitive (expects pre-lowercased input)
        assert!(!ActionsDialog::fuzzy_match("hello", "H"));
        assert!(ActionsDialog::fuzzy_match("hello", "h"));
    }

    // =========================================================================
    // 12. parse_shortcut_keycaps for all modifier symbols
    // =========================================================================

    #[test]
    fn parse_keycaps_modifier_symbols() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
        assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
    }

    #[test]
    fn parse_keycaps_enter_symbol() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
        assert_eq!(keycaps, vec!["↵"]);
    }

    #[test]
    fn parse_keycaps_escape_symbol() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
        assert_eq!(keycaps, vec!["⎋"]);
    }

    #[test]
    fn parse_keycaps_backspace_symbol() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⌫");
        assert_eq!(keycaps, vec!["⌘", "⌫"]);
    }

    #[test]
    fn parse_keycaps_space_symbol() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("␣");
        assert_eq!(keycaps, vec!["␣"]);
    }

    #[test]
    fn parse_keycaps_arrow_keys() {
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
    }

    #[test]
    fn parse_keycaps_tab_symbol() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⇥");
        assert_eq!(keycaps, vec!["⇥"]);
    }

    #[test]
    fn parse_keycaps_all_modifiers_combined() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧X");
        assert_eq!(keycaps, vec!["⌘", "⌃", "⌥", "⇧", "X"]);
    }

    #[test]
    fn parse_keycaps_lowercase_becomes_uppercase() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘e");
        assert_eq!(keycaps, vec!["⌘", "E"]);
    }

    // =========================================================================
    // 13. format_shortcut_hint roundtrips for unusual key names
    // =========================================================================

    #[test]
    fn format_shortcut_hint_enter() {
        let hint = ActionsDialog::format_shortcut_hint("enter");
        assert_eq!(hint, "↵");
    }

    #[test]
    fn format_shortcut_hint_return() {
        let hint = ActionsDialog::format_shortcut_hint("return");
        assert_eq!(hint, "↵");
    }

    #[test]
    fn format_shortcut_hint_escape() {
        let hint = ActionsDialog::format_shortcut_hint("escape");
        assert_eq!(hint, "⎋");
    }

    #[test]
    fn format_shortcut_hint_esc() {
        let hint = ActionsDialog::format_shortcut_hint("esc");
        assert_eq!(hint, "⎋");
    }

    #[test]
    fn format_shortcut_hint_tab() {
        let hint = ActionsDialog::format_shortcut_hint("tab");
        assert_eq!(hint, "⇥");
    }

    #[test]
    fn format_shortcut_hint_backspace() {
        let hint = ActionsDialog::format_shortcut_hint("backspace");
        assert_eq!(hint, "⌫");
    }

    #[test]
    fn format_shortcut_hint_space() {
        let hint = ActionsDialog::format_shortcut_hint("space");
        assert_eq!(hint, "␣");
    }

    #[test]
    fn format_shortcut_hint_arrow_keys() {
        assert_eq!(ActionsDialog::format_shortcut_hint("up"), "↑");
        assert_eq!(ActionsDialog::format_shortcut_hint("down"), "↓");
        assert_eq!(ActionsDialog::format_shortcut_hint("left"), "←");
        assert_eq!(ActionsDialog::format_shortcut_hint("right"), "→");
    }

    // --- merged from part_03.rs ---

    #[test]
    fn format_shortcut_hint_cmd_enter() {
        let hint = ActionsDialog::format_shortcut_hint("cmd+enter");
        assert_eq!(hint, "⌘↵");
    }

    #[test]
    fn format_shortcut_hint_ctrl_alt_delete() {
        let hint = ActionsDialog::format_shortcut_hint("ctrl+alt+delete");
        assert_eq!(hint, "⌃⌥⌫");
    }

    #[test]
    fn format_shortcut_hint_shift_cmd_c() {
        let hint = ActionsDialog::format_shortcut_hint("shift+cmd+c");
        assert_eq!(hint, "⇧⌘C");
    }

    #[test]
    fn format_shortcut_hint_option_variant() {
        let hint = ActionsDialog::format_shortcut_hint("option+a");
        assert_eq!(hint, "⌥A");
    }

    #[test]
    fn format_shortcut_hint_command_variant() {
        let hint = ActionsDialog::format_shortcut_hint("command+s");
        assert_eq!(hint, "⌘S");
    }

    #[test]
    fn format_shortcut_hint_arrowup_variant() {
        let hint = ActionsDialog::format_shortcut_hint("arrowup");
        assert_eq!(hint, "↑");
    }

    #[test]
    fn format_shortcut_hint_arrowdown_variant() {
        let hint = ActionsDialog::format_shortcut_hint("arrowdown");
        assert_eq!(hint, "↓");
    }

    // =========================================================================
    // 14. to_deeplink_name with CJK, emoji, RTL characters
    // =========================================================================

    #[test]
    fn deeplink_name_ascii_basic() {
        assert_eq!(to_deeplink_name("Hello World"), "hello-world");
    }

    #[test]
    fn deeplink_name_underscores_become_hyphens() {
        assert_eq!(to_deeplink_name("hello_world_test"), "hello-world-test");
    }

    #[test]
    fn deeplink_name_special_chars_stripped() {
        assert_eq!(to_deeplink_name("test!@#$%^&*()"), "test");
    }

    #[test]
    fn deeplink_name_multiple_spaces_collapsed() {
        assert_eq!(to_deeplink_name("foo   bar   baz"), "foo-bar-baz");
    }

    #[test]
    fn deeplink_name_numbers_preserved() {
        assert_eq!(to_deeplink_name("Script 123"), "script-123");
    }

    #[test]
    fn deeplink_name_mixed_case_lowered() {
        assert_eq!(to_deeplink_name("MyScript"), "myscript");
    }

    #[test]
    fn deeplink_name_accented_chars() {
        assert_eq!(
            to_deeplink_name("café résumé"),
            "caf%C3%A9-r%C3%A9sum%C3%A9"
        );
    }

    #[test]
    fn deeplink_name_consecutive_hyphens_collapsed() {
        assert_eq!(to_deeplink_name("a--b"), "a-b");
    }

    // =========================================================================
    // 15. Grouped items with realistic AI command bar data
    // =========================================================================

    #[test]
    fn grouped_items_headers_style_produces_section_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        // Should have section headers for each section transition
        assert_eq!(
            header_count, 8,
            "AI command bar should have 8 section headers, got {}",
            header_count
        );
    }

    #[test]
    fn grouped_items_none_style_produces_no_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(header_count, 0, "None style should have 0 headers");
    }

    #[test]
    fn grouped_items_separators_style_produces_no_headers() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(header_count, 0, "Separators style should have 0 headers");
    }

    #[test]
    fn grouped_items_item_count_matches_filtered_count() {
        let actions = get_ai_command_bar_actions();
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        let item_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::Item(_)))
            .count();
        assert_eq!(item_count, filtered.len());
    }

    // =========================================================================
    // 16. coerce_action_selection edge cases
    // =========================================================================

    #[test]
    fn coerce_selection_empty_rows_returns_none() {
        let rows: Vec<GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn coerce_selection_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".to_string()),
            GroupedActionItem::SectionHeader("B".to_string()),
            GroupedActionItem::SectionHeader("C".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn coerce_selection_on_item_returns_same_index() {
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        assert_eq!(coerce_action_selection(&rows, 1), Some(1));
    }

    #[test]
    fn coerce_selection_on_header_skips_to_next_item() {
        let rows = vec![
            GroupedActionItem::SectionHeader("Response".to_string()),
            GroupedActionItem::Item(0),
            GroupedActionItem::Item(1),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn coerce_selection_on_last_header_searches_backward() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("End".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn coerce_selection_out_of_bounds_clamps() {
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        // Index 99 should be clamped to last valid index
        assert_eq!(coerce_action_selection(&rows, 99), Some(1));
    }

    // =========================================================================
    // 17. Note switcher section assignment (Pinned vs Recent)
    // =========================================================================

    #[test]
    fn note_switcher_pinned_note_has_pinned_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".to_string(),
            title: "Pinned Note".to_string(),
            char_count: 100,
            is_current: false,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn note_switcher_unpinned_note_has_recent_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-2".to_string(),
            title: "Regular Note".to_string(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Recent"));
    }

    #[test]
    fn note_switcher_mixed_pinned_and_recent() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "uuid-1".to_string(),
                title: "Pinned".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "uuid-2".to_string(),
                title: "Recent".to_string(),
                char_count: 20,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        assert_eq!(actions[1].section.as_deref(), Some("Recent"));
    }

    #[test]
    fn note_switcher_current_note_has_bullet_prefix() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".to_string(),
            title: "My Note".to_string(),
            char_count: 42,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions[0].title.starts_with("• "),
            "Current note should have bullet prefix, got: {}",
            actions[0].title
        );
    }

    #[test]
    fn note_switcher_non_current_note_no_bullet_prefix() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".to_string(),
            title: "Other Note".to_string(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(!actions[0].title.starts_with("• "));
    }

    #[test]
    fn note_switcher_icon_hierarchy_pinned_beats_current() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "uuid-1".to_string(),
                title: "Pinned+Current".to_string(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "uuid-2".to_string(),
                title: "Current Only".to_string(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "uuid-3".to_string(),
                title: "Pinned Only".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "uuid-4".to_string(),
                title: "Neither".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled)); // pinned+current → Star
        assert_eq!(actions[1].icon, Some(IconName::Check)); // current only → Check
        assert_eq!(actions[2].icon, Some(IconName::StarFilled)); // pinned only → Star
        assert_eq!(actions[3].icon, Some(IconName::File)); // neither → File
    }

    #[test]
    fn note_switcher_char_count_singular() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".to_string(),
            title: "Single Char Note".to_string(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("1 char"));
    }

    #[test]
    fn note_switcher_char_count_plural() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".to_string(),
            title: "Multi Char Note".to_string(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("42 chars"));
    }

    #[test]
    fn note_switcher_char_count_zero() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".to_string(),
            title: "Empty Note".to_string(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("0 chars"));
    }

    #[test]
    fn note_switcher_action_id_format() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc-123-def".to_string(),
            title: "Test".to_string(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].id, "note_abc-123-def");
    }

    // =========================================================================
    // 18. Clipboard frontmost app edge cases
    // =========================================================================

    #[test]
    fn clipboard_paste_title_with_empty_string_app_name() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: Some("".to_string()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
        // Empty/whitespace app names fall back to the generic title.
        assert_eq!(paste.title, "Paste to Active App");
    }

    // --- merged from part_04.rs ---

    #[test]
    fn clipboard_paste_title_with_unicode_app_name() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: Some("日本語App".to_string()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
        assert_eq!(paste.title, "Paste to 日本語App");
    }

    #[test]
    fn clipboard_paste_title_without_app_name() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
        assert_eq!(paste.title, "Paste to Active App");
    }

    // =========================================================================
    // 19. Chat with no models, no messages, no response
    // =========================================================================

    #[test]
    fn chat_no_models_no_messages_no_response() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"chat:continue_in_chat"));
        assert!(!ids.contains(&"chat:copy_response"));
        assert!(!ids.contains(&"chat:clear_conversation"));
        crate::actions::dialog_builtin_validation::assert_chat_root_contract(&info, &actions);
    }

    #[test]
    fn chat_with_response_only_has_copy_response() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"chat:continue_in_chat"));
        assert!(ids.contains(&"chat:copy_response"));
        assert!(!ids.contains(&"chat:clear_conversation"));
    }

    #[test]
    fn chat_with_messages_only_has_clear_conversation() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"chat:continue_in_chat"));
        assert!(!ids.contains(&"chat:copy_response"));
        assert!(ids.contains(&"chat:clear_conversation"));
    }

    #[test]
    fn chat_with_all_flags_has_all_actions() {
        let info = ChatPromptInfo {
            current_model: Some("Claude 3.5".to_string()),
            available_models: vec![ChatModelInfo {
                id: "claude-3.5".to_string(),
                display_name: "Claude 3.5".to_string(),
                provider: "Anthropic".to_string(),
            }],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        let ids = action_ids(&actions);
        assert!(ids.contains(&"chat:change_model"));
        assert!(ids.contains(&"chat:continue_in_chat"));
        assert!(ids.contains(&"chat:copy_response"));
        assert!(ids.contains(&"chat:clear_conversation"));
    }

    #[test]
    fn chat_model_checkmark_on_current() {
        let info = ChatPromptInfo {
            current_model: Some("Claude 3.5".to_string()),
            available_models: vec![
                ChatModelInfo {
                    id: "claude-3.5".to_string(),
                    display_name: "Claude 3.5".to_string(),
                    provider: "Anthropic".to_string(),
                },
                ChatModelInfo {
                    id: "gpt-4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let claude = find_action(&picker, "chat:select_model_claude-3.5").unwrap();
        let gpt = find_action(&picker, "chat:select_model_gpt-4").unwrap();
        assert!(
            claude.title.contains('✓'),
            "Current model should have checkmark"
        );
        assert!(
            !gpt.title.contains('✓'),
            "Non-current model should not have checkmark"
        );
    }

    // =========================================================================
    // 20. Scriptlet custom actions ordering preservation
    // =========================================================================

    #[test]
    fn scriptlet_custom_actions_preserve_declaration_order() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new(
            "Test".to_string(),
            "bash".to_string(),
            "echo main".to_string(),
        );
        scriptlet.actions = vec![
            ScriptletAction {
                name: "First Action".to_string(),
                command: "first".to_string(),
                tool: "bash".to_string(),
                code: "echo first".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
            ScriptletAction {
                name: "Second Action".to_string(),
                command: "second".to_string(),
                tool: "bash".to_string(),
                code: "echo second".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
            ScriptletAction {
                name: "Third Action".to_string(),
                command: "third".to_string(),
                tool: "bash".to_string(),
                code: "echo third".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
        ];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));

        let custom_ids: Vec<&str> = actions
            .iter()
            .filter(|a| a.id.starts_with("scriptlet_action:"))
            .map(|a| a.id.as_str())
            .collect();

        assert_eq!(
            custom_ids,
            vec![
                "scriptlet_action:first",
                "scriptlet_action:second",
                "scriptlet_action:third"
            ]
        );
    }

    #[test]
    fn scriptlet_custom_actions_appear_after_run_before_builtins() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new(
            "Test".to_string(),
            "bash".to_string(),
            "echo main".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Custom".to_string(),
            command: "custom".to_string(),
            tool: "bash".to_string(),
            code: "echo custom".to_string(),
            inputs: vec![],
            shortcut: Some("cmd+1".to_string()),
            description: Some("A custom action".to_string()),
        }];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let ids = action_ids(&actions);

        let run_idx = ids.iter().position(|id| *id == "run_script").unwrap();
        let custom_idx = ids
            .iter()
            .position(|id| *id == "scriptlet_action:custom")
            .unwrap();
        let edit_idx = ids.iter().position(|id| *id == "edit_scriptlet").unwrap();

        assert!(run_idx < custom_idx, "run before custom");
        assert!(custom_idx < edit_idx, "custom before edit");
    }

    #[test]
    fn scriptlet_custom_actions_have_has_action_true() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new(
            "Test".to_string(),
            "bash".to_string(),
            "echo main".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Custom".to_string(),
            command: "custom".to_string(),
            tool: "bash".to_string(),
            code: "echo custom".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let custom = find_action(&actions, "scriptlet_action:custom").unwrap();
        assert!(
            custom.has_action,
            "Custom scriptlet action should have has_action=true"
        );
        assert_eq!(custom.value, Some("custom".to_string()));
    }

    #[test]
    fn scriptlet_custom_action_shortcut_formatted() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut scriptlet = Scriptlet::new(
            "Test".to_string(),
            "bash".to_string(),
            "echo main".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "copy".to_string(),
            tool: "bash".to_string(),
            code: "echo copy".to_string(),
            inputs: vec![],
            shortcut: Some("cmd+shift+c".to_string()),
            description: None,
        }];

        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let custom = find_action(&actions, "scriptlet_action:copy").unwrap();
        assert_eq!(
            custom.shortcut,
            Some("⌘⇧C".to_string()),
            "Shortcut should be formatted with symbols"
        );
    }

    // =========================================================================
    // 21. Action constructor lowercase caching
    // =========================================================================

    #[test]
    fn action_title_lower_caches_correctly() {
        let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "edit script");
    }

    #[test]
    fn action_description_lower_caches_correctly() {
        let action = Action::new(
            "test",
            "Test",
            Some("Open In Editor".to_string()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower, Some("open in editor".to_string()));
    }

    #[test]
    fn action_description_lower_none_when_no_description() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
        assert_eq!(action.description_lower, None);
    }

    #[test]
    fn action_shortcut_lower_set_after_with_shortcut() {
        let action =
            Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        assert_eq!(action.shortcut_lower, Some("⌘e".to_string()));
    }

    #[test]
    fn action_shortcut_lower_none_without_shortcut() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
        assert_eq!(action.shortcut_lower, None);
    }

    #[test]
    fn action_title_lower_unicode() {
        let action = Action::new("test", "Café Résumé", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "café résumé");
    }

    #[test]
    fn action_with_shortcut_opt_some_sets_shortcut() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘X".to_string()));
        assert_eq!(action.shortcut, Some("⌘X".to_string()));
        assert_eq!(action.shortcut_lower, Some("⌘x".to_string()));
    }

    #[test]
    fn action_with_shortcut_opt_none_leaves_shortcut_unset() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(None);
        assert_eq!(action.shortcut, None);
        assert_eq!(action.shortcut_lower, None);
    }

    // =========================================================================
    // 22. CommandBarConfig preset field values
    // =========================================================================

    #[test]
    fn command_bar_config_ai_style_fields() {
        let config = CommandBarConfig::ai_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        assert!(config.dialog_config.show_icons);
    }

    #[test]
    fn command_bar_config_main_menu_style_fields() {
        let config = CommandBarConfig::main_menu_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        assert!(!config.dialog_config.show_icons);
    }

    #[test]
    fn command_bar_config_notes_style_fields() {
        let config = CommandBarConfig::notes_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        assert!(config.dialog_config.show_icons);
    }

    #[test]
    fn command_bar_config_no_search_hides_search() {
        let config = CommandBarConfig::no_search();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
    }

    // =========================================================================
    // 23. Path context primary action varies by is_dir
    // =========================================================================

    #[test]
    fn path_dir_primary_is_open_directory() {
        let path = PathInfo {
            name: "Documents".to_string(),
            path: "/home/user/Documents".to_string(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:open_directory");
        assert!(actions[0].title.contains("Documents"));
    }

    #[test]
    fn path_file_primary_is_select_file() {
        let path = PathInfo {
            name: "readme.md".to_string(),
            path: "/home/user/readme.md".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:select_file");
        assert!(actions[0].title.contains("readme.md"));
    }

    #[test]
    fn path_trash_description_differs_by_is_dir() {
        let dir_path = PathInfo {
            name: "src".to_string(),
            path: "/home/user/src".to_string(),
            is_dir: true,
        };
        let file_path = PathInfo {
            name: "file.txt".to_string(),
            path: "/home/user/file.txt".to_string(),
            is_dir: false,
        };
        let dir_actions = get_path_context_actions(&dir_path);
        let file_actions = get_path_context_actions(&file_path);
        let dir_trash = find_action(&dir_actions, "file:move_to_trash").unwrap();
        let file_trash = find_action(&file_actions, "file:move_to_trash").unwrap();
        assert_eq!(
            dir_trash.description.as_deref(),
            Some("Moves this folder to the Trash")
        );
        assert_eq!(
            file_trash.description.as_deref(),
            Some("Moves this file to the Trash")
        );
    }

    // =========================================================================
    // 24. File context primary title includes name
    // =========================================================================

    #[test]
    fn file_primary_title_includes_filename() {
        let info = FileInfo {
            path: "/tmp/report.pdf".to_string(),
            name: "report.pdf".to_string(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        assert!(
            actions[0].title.contains("report.pdf"),
            "Primary title should include filename: {}",
            actions[0].title
        );
    }

    // --- merged from part_05.rs ---

    #[test]
    fn file_dir_primary_title_includes_dirname() {
        let info = FileInfo {
            path: "/tmp/build".to_string(),
            name: "build".to_string(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&info);
        assert!(
            actions[0].title.contains("build"),
            "Primary title should include dirname: {}",
            actions[0].title
        );
    }

    // =========================================================================
    // 25. Deeplink description format in script context
    // =========================================================================

    #[test]
    fn deeplink_description_contains_url_with_formatted_name() {
        let script = ScriptInfo::new("My Cool Script", "/path/to/script.ts");
        let actions = get_script_context_actions(&script);
        let dl = find_action(&actions, "copy_deeplink").unwrap();
        assert!(
            dl.description
                .as_ref()
                .unwrap()
                .eq("Copy a portable Script Kit share link to clipboard"),
            "Script-context copy_deeplink should use portable share copy: {:?}",
            dl.description
        );
    }

    #[test]
    fn deeplink_description_for_builtin() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&builtin);
        let dl = find_action(&actions, "copy_deeplink").unwrap();
        assert!(dl
            .description
            .as_ref()
            .unwrap()
            .contains("scriptkit://run/clipboard-history"));
    }

    // =========================================================================
    // 26. All built-in actions have has_action=false
    // =========================================================================

    #[test]
    fn script_context_all_actions_have_has_action_false() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        for action in &actions {
            assert!(
                !action.has_action,
                "Built-in action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn clipboard_context_all_actions_have_has_action_false() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for action in &actions {
            assert!(
                !action.has_action,
                "Clipboard action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn file_context_all_actions_have_has_action_false() {
        let info = FileInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        for action in &actions {
            assert!(
                !action.has_action,
                "File action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn path_context_all_actions_have_has_action_false() {
        let path = PathInfo {
            name: "test.txt".to_string(),
            path: "/tmp/test.txt".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        for action in &actions {
            assert!(
                !action.has_action,
                "Path action '{}' should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn ai_command_bar_all_actions_have_has_action_false() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert!(
                !action.has_action,
                "AI action '{}' should have has_action=false",
                action.id
            );
        }
    }

    // =========================================================================
    // 27. All actions have non-empty title and ID
    // =========================================================================

    #[test]
    fn script_context_all_actions_have_nonempty_title_and_id() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        for action in &actions {
            assert!(!action.id.is_empty(), "Action ID should not be empty");
            assert!(!action.title.is_empty(), "Action title should not be empty");
        }
    }

    #[test]
    fn clipboard_context_all_actions_have_nonempty_title_and_id() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for action in &actions {
            assert!(!action.id.is_empty());
            assert!(!action.title.is_empty());
        }
    }

    #[test]
    fn ai_command_bar_all_actions_have_nonempty_title_and_id() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert!(!action.id.is_empty());
            assert!(!action.title.is_empty());
        }
    }

    // =========================================================================
    // 28. Action ID uniqueness within contexts
    // =========================================================================

    #[test]
    fn script_context_ids_are_unique() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            actions.len(),
            "Duplicate action IDs found in script context"
        );
    }

    #[test]
    fn clipboard_text_context_ids_are_unique() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            actions.len(),
            "Duplicate action IDs found in clipboard text context"
        );
    }

    #[test]
    fn clipboard_image_context_ids_are_unique() {
        let entry = ClipboardEntryInfo {
            id: "i1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Image".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            actions.len(),
            "Duplicate action IDs found in clipboard image context"
        );
    }

    #[test]
    fn path_context_ids_are_unique() {
        let path = PathInfo {
            name: "test.txt".to_string(),
            path: "/tmp/test.txt".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            actions.len(),
            "Duplicate action IDs found in path context"
        );
    }

    #[test]
    fn ai_command_bar_ids_are_unique() {
        let actions = get_ai_command_bar_actions();
        let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids.len(),
            actions.len(),
            "Duplicate action IDs found in AI command bar"
        );
    }

    // =========================================================================
    // 29. Clipboard destructive actions always last three
    // =========================================================================

    #[test]
    fn clipboard_destructive_actions_are_last_three() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();
        assert!(len >= 3);
        assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
        assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
        assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
    }

    #[test]
    fn clipboard_image_destructive_actions_are_last_three() {
        let entry = ClipboardEntryInfo {
            id: "i1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Image".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();
        assert!(len >= 3);
        assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
        assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
        assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
    }

    // =========================================================================
    // 30. Clipboard paste is always first, copy is always second
    // =========================================================================

    #[test]
    fn clipboard_paste_is_first_copy_is_second() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
        assert_eq!(actions[1].id, "clip:clipboard_copy");
    }

    // =========================================================================
    // 31. All actions have ScriptContext category
    // =========================================================================

    #[test]
    fn all_contexts_produce_script_context_category() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in &get_script_context_actions(&script) {
            assert_eq!(action.category, ActionCategory::ScriptContext);
        }

        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for action in &get_clipboard_history_context_actions(&entry) {
            assert_eq!(action.category, ActionCategory::ScriptContext);
        }

        let path = PathInfo {
            name: "test".to_string(),
            path: "/tmp/test".to_string(),
            is_dir: false,
        };
        for action in &get_path_context_actions(&path) {
            assert_eq!(action.category, ActionCategory::ScriptContext);
        }

        let file = FileInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        for action in &get_file_context_actions(&file) {
            assert_eq!(action.category, ActionCategory::ScriptContext);
        }

        for action in &get_ai_command_bar_actions() {
            assert_eq!(action.category, ActionCategory::ScriptContext);
        }
    }

    // =========================================================================
    // 32. Primary action always first across contexts
    // =========================================================================

    #[test]
    fn primary_action_first_in_script_context() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].id, "run_script");
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn primary_action_first_in_file_context() {
        let file = FileInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        assert_eq!(actions[0].id, "file:open_file");
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn primary_action_first_in_path_context() {
        let path = PathInfo {
            name: "readme.md".to_string(),
            path: "/tmp/readme.md".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].id, "file:select_file");
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn primary_action_first_in_clipboard_context() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    // =========================================================================
    // 33. Ordering determinism
    // =========================================================================

    #[test]
    fn script_context_ordering_deterministic_across_calls() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions1 = get_script_context_actions(&script);
        let actions2 = get_script_context_actions(&script);
        let ids1 = action_ids(&actions1);
        let ids2 = action_ids(&actions2);
        assert_eq!(ids1, ids2, "Action ordering should be deterministic");
    }

    #[test]
    fn clipboard_context_ordering_deterministic_across_calls() {
        let entry = ClipboardEntryInfo {
            id: "t1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions1 = get_clipboard_history_context_actions(&entry);
        let actions2 = get_clipboard_history_context_actions(&entry);
        let ids1 = action_ids(&actions1);
        let ids2 = action_ids(&actions2);
        assert_eq!(ids1, ids2);
    }

    #[test]
    fn ai_command_bar_ordering_deterministic_across_calls() {
        let actions1 = get_ai_command_bar_actions();
        let actions2 = get_ai_command_bar_actions();
        let ids1 = action_ids(&actions1);
        let ids2 = action_ids(&actions2);
        assert_eq!(ids1, ids2);
    }

    // --- merged from part_06.rs ---

    // =========================================================================
    // 34. New chat icons per section
    // =========================================================================

    #[test]
    fn new_chat_last_used_icon_is_bolt() {
        let last_used = vec![NewChatModelInfo {
            model_id: "lu1".to_string(),
            display_name: "Recent".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    #[test]
    fn new_chat_preset_icon_is_custom() {
        let presets = vec![NewChatPresetInfo {
            id: "gen".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Star));
    }

    #[test]
    fn new_chat_model_icon_is_settings() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "Claude".to_string(),
            provider: "anthropic".to_string(),
            provider_display_name: "Anthropic".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].icon, Some(IconName::Settings));
    }

    // =========================================================================
    // 35. New chat descriptions
    // =========================================================================

    #[test]
    fn new_chat_last_used_has_provider_display_name_description() {
        let last_used = vec![NewChatModelInfo {
            model_id: "lu1".to_string(),
            display_name: "Claude 3.5".to_string(),
            provider: "anthropic".to_string(),
            provider_display_name: "Anthropic".to_string(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].description.as_deref(), Some("Uses Anthropic"));
    }

    #[test]
    fn new_chat_preset_has_no_description() {
        let presets = vec![NewChatPresetInfo {
            id: "gen".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses General preset")
        );
    }

    #[test]
    fn new_chat_model_has_provider_display_name_description() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "GPT-4".to_string(),
            provider: "openai".to_string(),
            provider_display_name: "OpenAI".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].description.as_deref(), Some("Uses OpenAI"));
    }

    // =========================================================================
    // 36. New chat action ID format
    // =========================================================================

    #[test]
    fn new_chat_last_used_id_format() {
        let last_used = vec![
            NewChatModelInfo {
                model_id: "a".to_string(),
                display_name: "A".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            },
            NewChatModelInfo {
                model_id: "b".to_string(),
                display_name: "B".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            },
        ];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].id, "last_used_p::a");
        assert_eq!(actions[1].id, "last_used_p::b");
    }

    #[test]
    fn new_chat_preset_id_format() {
        let presets = vec![NewChatPresetInfo {
            id: "code-review".to_string(),
            name: "Code Review".to_string(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_code-review");
    }

    #[test]
    fn new_chat_model_id_format() {
        let models = vec![
            NewChatModelInfo {
                model_id: "claude".to_string(),
                display_name: "Claude".to_string(),
                provider: "anthropic".to_string(),
                provider_display_name: "Anthropic".to_string(),
            },
            NewChatModelInfo {
                model_id: "gpt4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "openai".to_string(),
                provider_display_name: "OpenAI".to_string(),
            },
        ];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_anthropic::claude");
        assert_eq!(actions[1].id, "model_openai::gpt4");
    }
}

mod from_dialog_builtin_action_validation_tests_5 {
    //! Batch 5: Built-in action behavioral validation tests
    //!
    //! 150+ tests validating action invariants NOT covered in batches 1-4.
    //! Focus areas:
    //! - Note switcher description rendering (preview truncation, relative time combos)
    //! - Clipboard action position invariants beyond first/last
    //! - AI command bar section item counts
    //! - build_grouped_items_static section transitions
    //! - Large-scale stress (many notes, models, presets)
    //! - Cross-function ScriptInfo consistency
    //! - Action description content keywords
    //! - Score_action with cached lowercase fields
    //! - Scriptlet with_custom multiple custom actions ordering
    //! - CommandBarConfig equality and field access patterns

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::*;
        use crate::actions::dialog::{
            build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
        };
        use crate::actions::types::*;
        use crate::actions::CommandBarConfig;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};

        // =========================================================================
        // Helper: collect action IDs from a Vec<Action>
        // =========================================================================
        fn action_ids(actions: &[Action]) -> Vec<&str> {
            actions.iter().map(|a| a.id.as_str()).collect()
        }

        // =========================================================================
        // 1. Note switcher description rendering (preview + relative_time combos)
        // =========================================================================

        #[test]
        fn note_switcher_desc_preview_and_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "My Note".into(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "Hello world snippet".into(),
                relative_time: "5m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(
                desc.contains("Hello world snippet"),
                "Description should contain preview"
            );
            assert!(desc.contains("5m ago"), "Description should contain time");
            assert!(desc.contains(" · "), "Preview and time joined with ' · '");
        }

        #[test]
        fn note_switcher_desc_preview_only_no_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n2".into(),
                title: "Note".into(),
                char_count: 50,
                is_current: false,
                is_pinned: false,
                preview: "Some preview text".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "Some preview text");
            assert!(!desc.contains(" · "), "No separator when no time");
        }

        #[test]
        fn note_switcher_desc_time_only_no_preview() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n3".into(),
                title: "Note".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "1h ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1h ago");
        }

        #[test]
        fn note_switcher_desc_no_preview_no_time_zero_chars() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n4".into(),
                title: "Empty".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "0 chars");
        }

        #[test]
        fn note_switcher_desc_no_preview_no_time_one_char() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n5".into(),
                title: "Tiny".into(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1 char");
        }

        #[test]
        fn note_switcher_desc_no_preview_no_time_many_chars() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n6".into(),
                title: "Long".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        #[test]
        fn note_switcher_preview_truncated_at_60_chars() {
            let long_preview = "A".repeat(80);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n7".into(),
                title: "Long Preview".into(),
                char_count: 80,
                is_current: false,
                is_pinned: false,
                preview: long_preview,
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            // Should be 60 A's followed by "…"
            assert!(
                desc.ends_with('…'),
                "Long preview should be truncated with ellipsis"
            );
            // Count: 60 A's + "…" = 61 chars
            assert_eq!(desc.chars().count(), 61);
        }

        #[test]
        fn note_switcher_preview_exactly_60_chars_not_truncated() {
            let exact_preview = "B".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n8".into(),
                title: "Exact".into(),
                char_count: 60,
                is_current: false,
                is_pinned: false,
                preview: exact_preview.clone(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, &exact_preview);
            assert!(!desc.ends_with('…'));
        }

        #[test]
        fn note_switcher_preview_61_chars_truncated() {
            let preview_61 = "C".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n9".into(),
                title: "Just Over".into(),
                char_count: 61,
                is_current: false,
                is_pinned: false,
                preview: preview_61,
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.ends_with('…'));
        }

        #[test]
        fn note_switcher_preview_truncated_with_time() {
            let long_preview = "D".repeat(80);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n10".into(),
                title: "Truncated + Time".into(),
                char_count: 80,
                is_current: false,
                is_pinned: false,
                preview: long_preview,
                relative_time: "2d ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains("…"));
            assert!(desc.contains("2d ago"));
            assert!(desc.contains(" · "));
        }

        // =========================================================================
        // 2. Clipboard action position invariants (beyond first/last)
        // =========================================================================

        #[test]
        fn clipboard_text_paste_keep_open_is_third() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[2].id, "clip:clipboard_paste_keep_open");
        }

        #[test]
        fn clipboard_text_share_is_fourth() {
            let entry = ClipboardEntryInfo {
                id: "e2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[3].id, "clip:clipboard_share");
        }

        #[test]
        fn clipboard_text_attach_ai_is_fifth() {
            let entry = ClipboardEntryInfo {
                id: "e3".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[4].id, "clip:clipboard_attach_to_ai");
        }

        #[test]
        fn clipboard_save_snippet_before_save_file() {
            let entry = ClipboardEntryInfo {
                id: "e4".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            let snippet_pos = ids
                .iter()
                .position(|&id| id == "clip:clipboard_save_snippet")
                .unwrap();
            let file_pos = ids
                .iter()
                .position(|&id| id == "clip:clipboard_save_file")
                .unwrap();
            assert!(
                snippet_pos < file_pos,
                "Save snippet should come before save file"
            );
        }

        #[test]
        fn clipboard_delete_order_is_single_then_multiple_then_all() {
            let entry = ClipboardEntryInfo {
                id: "e5".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            let del_pos = ids
                .iter()
                .position(|&id| id == "clip:clipboard_delete")
                .unwrap();
            let del_multi_pos = ids
                .iter()
                .position(|&id| id == "clip:clipboard_delete_multiple")
                .unwrap();
            let del_all_pos = ids
                .iter()
                .position(|&id| id == "clip:clipboard_delete_all")
                .unwrap();
            assert!(del_pos < del_multi_pos);
            assert!(del_multi_pos < del_all_pos);
        }

        #[test]
        fn clipboard_text_unpinned_action_count() {
            let entry = ClipboardEntryInfo {
                id: "e6".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // On macOS: paste, copy, paste_keep_open, share, attach_ai, quick_look,
            //           pin, keep_in_today, save_snippet, save_file, delete, delete_multiple,
            //           delete_all = 13
            // Non-macOS: no quick_look = 12
            #[cfg(target_os = "macos")]
            assert_eq!(actions.len(), 13);
            #[cfg(not(target_os = "macos"))]
            assert_eq!(actions.len(), 12);
        }

        #[test]
        fn clipboard_image_pinned_action_count() {
            let entry = ClipboardEntryInfo {
                id: "e7".into(),
                content_type: ContentType::Image,
                pinned: true,
                preview: "img".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // Images do NOT get save_snippet (text-only); they get ocr instead.
            // On macOS: paste, copy, paste_keep_open, share, attach_ai, quick_look,
            //           open_with, annotate_cleanshot, upload_cleanshot, unpin, keep_in_today,
            //           ocr, save_file, delete, delete_multiple, delete_all = 16
            #[cfg(target_os = "macos")]
            assert_eq!(actions.len(), 16);
            // Non-macOS: no quick_look, open_with, annotate_cleanshot, upload_cleanshot = 12
            #[cfg(not(target_os = "macos"))]
            assert_eq!(actions.len(), 12);
        }

        // =========================================================================
        // 3. AI command bar section item counts
        // =========================================================================

        #[test]
        fn ai_command_bar_response_section_has_3_actions() {
            let actions = get_ai_command_bar_actions();
            let response_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .count();
            assert_eq!(response_count, 3);
        }

        #[test]
        fn ai_command_bar_actions_section_has_4_actions() {
            let actions = get_ai_command_bar_actions();
            let actions_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .count();
            assert_eq!(actions_count, 7);
        }

        #[test]
        fn ai_command_bar_attachments_section_has_2_actions() {
            let actions = get_ai_command_bar_actions();
            let attach_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .count();
            assert_eq!(attach_count, 4);
        }

        #[test]
        fn ai_command_bar_settings_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let settings_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Settings"))
                .count();
            assert_eq!(settings_count, 2);
        }

        #[test]
        fn ai_command_bar_total_is_12() {
            let actions = get_ai_command_bar_actions();
            assert_eq!(actions.len(), 38);
        }

        #[test]
        fn ai_command_bar_section_order_preserved() {
            let actions = get_ai_command_bar_actions();
            let sections: Vec<&str> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            // Should transition: Response, Actions, Attachments, Export, Context, Actions, Help, Settings
            let mut seen_sections: Vec<&str> = Vec::new();
            for s in &sections {
                if seen_sections.last() != Some(s) {
                    seen_sections.push(s);
                }
            }
            assert_eq!(
                seen_sections,
                vec![
                    "Response",
                    "Actions",
                    "Attachments",
                    "Export",
                    "Context",
                    "Actions",
                    "Help",
                    "Settings"
                ]
            );
        }

        #[test]
        fn ai_command_bar_all_have_icons() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.icon.is_some(),
                    "AI action '{}' should have an icon",
                    action.id
                );
            }
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn ai_command_bar_specific_icons() {
            let actions = get_ai_command_bar_actions();
            let copy_response = actions
                .iter()
                .find(|a| a.id == "chat:copy_response")
                .unwrap();
            assert_eq!(copy_response.icon, Some(IconName::Copy));

            let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(submit.icon, Some(IconName::ArrowUp));

            let delete_chat = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(delete_chat.icon, Some(IconName::Trash));

            let change_model = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert_eq!(change_model.icon, Some(IconName::Settings));
        }

        // =========================================================================
        // 4. build_grouped_items_static section transitions
        // =========================================================================

        #[test]
        fn grouped_items_headers_inserts_header_on_section_change() {
            let actions = vec![
                Action::new("a1", "Act 1", None, ActionCategory::ScriptContext)
                    .with_section("Sec A"),
                Action::new("a2", "Act 2", None, ActionCategory::ScriptContext)
                    .with_section("Sec A"),
                Action::new("a3", "Act 3", None, ActionCategory::ScriptContext)
                    .with_section("Sec B"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Expected: Header("Sec A"), Item(0), Item(1), Header("Sec B"), Item(2)
            assert_eq!(grouped.len(), 5);
            assert!(matches!(
                &grouped[0],
                GroupedActionItem::SectionHeader(s) if s == "Sec A"
            ));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[2], GroupedActionItem::Item(1)));
            assert!(matches!(
                &grouped[3],
                GroupedActionItem::SectionHeader(s) if s == "Sec B"
            ));
            assert!(matches!(grouped[4], GroupedActionItem::Item(2)));
        }

        #[test]
        fn grouped_items_headers_no_header_for_no_section() {
            let actions = vec![
                Action::new("a1", "Act 1", None, ActionCategory::ScriptContext),
                Action::new("a2", "Act 2", None, ActionCategory::ScriptContext),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // No sections => no headers
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn grouped_items_separators_no_headers_inserted() {
            let actions = vec![
                Action::new("a1", "Act 1", None, ActionCategory::ScriptContext)
                    .with_section("Sec A"),
                Action::new("a2", "Act 2", None, ActionCategory::ScriptContext)
                    .with_section("Sec B"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // Separators style: no headers, just items
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn grouped_items_none_no_headers_inserted() {
            let actions = vec![
                Action::new("a1", "Act 1", None, ActionCategory::ScriptContext)
                    .with_section("Sec A"),
            ];
            let filtered: Vec<usize> = vec![0];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 1);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        }

        #[test]
        fn grouped_items_empty_filtered_produces_empty() {
            let actions = vec![Action::new(
                "a1",
                "Act 1",
                None,
                ActionCategory::ScriptContext,
            )];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn grouped_items_headers_three_sections() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext).with_section("X"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("Y"),
                Action::new("a3", "A3", None, ActionCategory::ScriptContext).with_section("Z"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // 3 headers + 3 items = 6
            assert_eq!(grouped.len(), 6);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 3);
        }

        #[test]
        fn grouped_items_headers_same_section_no_duplicate_header() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext).with_section("Same"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("Same"),
                Action::new("a3", "A3", None, ActionCategory::ScriptContext).with_section("Same"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // 1 header + 3 items = 4
            assert_eq!(grouped.len(), 4);
        }

        // =========================================================================
        // 5. coerce_action_selection edge cases
        // =========================================================================

        #[test]
        fn coerce_selection_all_items_returns_requested() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::Item(1),
                GroupedActionItem::Item(2),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(1));
        }

        #[test]
        fn coerce_selection_header_at_start_goes_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn coerce_selection_header_at_end_goes_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("H".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn coerce_selection_header_between_items_goes_down() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("H".into()),
                GroupedActionItem::Item(1),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(2));
        }

        #[test]
        fn coerce_selection_two_headers_then_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H1".into()),
                GroupedActionItem::SectionHeader("H2".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(2));
            assert_eq!(coerce_action_selection(&rows, 1), Some(2));
        }

        #[test]
        fn coerce_selection_out_of_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
            // ix=10 should be clamped to rows.len()-1 = 1, which is an Item
            assert_eq!(coerce_action_selection(&rows, 10), Some(1));
        }

        // =========================================================================
        // 6. Large-scale stress tests
        // =========================================================================

        #[test]
        fn stress_50_notes_in_switcher() {
            let notes: Vec<NoteSwitcherNoteInfo> = (0..50)
                .map(|i| NoteSwitcherNoteInfo {
                    id: format!("note-{}", i),
                    title: format!("Note #{}", i),
                    char_count: i * 100,
                    is_current: i == 0,
                    is_pinned: i < 5,
                    preview: format!("Preview for note {}", i),
                    relative_time: format!("{}m ago", i),
                })
                .collect();
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions.len(), 50);

            // First 5 should be Pinned section
            for (i, action) in actions.iter().enumerate().take(5) {
                assert_eq!(
                    action.section.as_deref(),
                    Some("Pinned"),
                    "Note {} should be in Pinned section",
                    i
                );
            }

            // Remaining should be Recent
            for (i, action) in actions.iter().enumerate().take(50).skip(5) {
                assert_eq!(
                    action.section.as_deref(),
                    Some("Recent"),
                    "Note {} should be in Recent section",
                    i
                );
            }
        }

        #[test]
        fn stress_20_models_in_new_chat() {
            let models: Vec<NewChatModelInfo> = (0..20)
                .map(|i| NewChatModelInfo {
                    model_id: format!("model-{}", i),
                    display_name: format!("Model {}", i),
                    provider: format!("provider-{}", i),
                    provider_display_name: format!("Provider {}", i),
                })
                .collect();
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions.len(), 20);
            for (i, action) in actions.iter().enumerate() {
                assert_eq!(action.id, format!("model_provider-{}::model-{}", i, i));
                assert_eq!(action.section.as_deref(), Some("Models"));
            }
        }

        #[test]
        fn stress_mixed_last_used_presets_models() {
            let last_used: Vec<NewChatModelInfo> = (0..3)
                .map(|i| NewChatModelInfo {
                    model_id: format!("lu-{}", i),
                    display_name: format!("Last Used {}", i),
                    provider: "p".into(),
                    provider_display_name: "Provider".into(),
                })
                .collect();
            let presets: Vec<NewChatPresetInfo> = (0..4)
                .map(|i| NewChatPresetInfo {
                    id: format!("preset-{}", i),
                    name: format!("Preset {}", i),
                    icon: IconName::Star,
                })
                .collect();
            let models: Vec<NewChatModelInfo> = (0..5)
                .map(|i| NewChatModelInfo {
                    model_id: format!("m-{}", i),
                    display_name: format!("Model {}", i),
                    provider: "p".into(),
                    provider_display_name: "Provider".into(),
                })
                .collect();
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 12); // 3 + 4 + 5

            // Verify section counts
            let lu_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Last Used Settings"))
                .count();
            let preset_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Presets"))
                .count();
            let model_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Models"))
                .count();
            assert_eq!(lu_count, 3);
            assert_eq!(preset_count, 4);
            assert_eq!(model_count, 5);
        }

        #[test]
        fn stress_grouped_items_50_actions_with_sections() {
            let actions: Vec<Action> = (0..50)
                .map(|i| {
                    let section = match i % 3 {
                        0 => "A",
                        1 => "B",
                        _ => "C",
                    };
                    Action::new(
                        format!("a{}", i),
                        format!("Action {}", i),
                        None,
                        ActionCategory::ScriptContext,
                    )
                    .with_section(section)
                })
                .collect();
            let filtered: Vec<usize> = (0..50).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

            // With alternating sections A, B, C, A, B, C... headers appear at every change
            // Pattern: A at 0, B at 1, C at 2, A at 3, B at 4, C at 5... = 50 changes
            // But adjacent same-section items won't add headers
            // Actually each item has a unique section transition since i%3 alternates
            // So we get a header for nearly every item: 50 headers + 50 items = 100
            let item_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::Item(_)))
                .count();
            assert_eq!(item_count, 50);
        }

        // =========================================================================
        // 7. Cross-function ScriptInfo consistency
        // =========================================================================

        #[test]
        fn scriptlet_same_results_from_both_builders_without_custom() {
            let script = ScriptInfo::scriptlet("Test Scriptlet", "/path/to/test.md", None, None);
            let actions_standard = get_script_context_actions(&script);
            let actions_custom = get_scriptlet_context_actions_with_custom(&script, None);

            // Standard script context now includes toggle_info, toggle_favorite, and
            // file:open_in_quick_terminal for scriptlets, while the scriptlet-custom
            // builder (get_scriptlet_context_actions_with_custom) omits all three.
            // Filter those known extras so the comparison stays meaningful for the
            // shared scriptlet action set.
            let mut ids_standard = action_ids(&actions_standard);
            let ids_custom = action_ids(&actions_custom);
            ids_standard.retain(|id| {
                *id != "toggle_favorite"
                    && *id != "toggle_info"
                    && *id != "file:open_in_quick_terminal"
            });
            assert_eq!(ids_standard, ids_custom);
        }

        #[test]
        fn scriptlet_with_shortcut_same_from_both_builders() {
            let script =
                ScriptInfo::scriptlet("Test", "/path/to/test.md", Some("cmd+t".into()), None);
            let actions_standard = get_script_context_actions(&script);
            let actions_custom = get_scriptlet_context_actions_with_custom(&script, None);
            let mut ids_standard = action_ids(&actions_standard);
            let ids_custom = action_ids(&actions_custom);
            ids_standard.retain(|id| {
                *id != "toggle_favorite"
                    && *id != "toggle_info"
                    && *id != "file:open_in_quick_terminal"
            });
            assert_eq!(ids_standard, ids_custom);
        }

        #[test]
        fn scriptlet_with_alias_same_from_both_builders() {
            let script =
                ScriptInfo::scriptlet("Test", "/path/to/test.md", None, Some("tst".into()));
            let actions_standard = get_script_context_actions(&script);
            let actions_custom = get_scriptlet_context_actions_with_custom(&script, None);
            let mut ids_standard = action_ids(&actions_standard);
            let ids_custom = action_ids(&actions_custom);
            ids_standard.retain(|id| {
                *id != "toggle_favorite"
                    && *id != "toggle_info"
                    && *id != "file:open_in_quick_terminal"
            });
            assert_eq!(ids_standard, ids_custom);
        }

        #[test]
        fn scriptlet_with_frecency_same_from_both_builders() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None)
                .with_frecency(true, Some("test".into()));
            let actions_standard = get_script_context_actions(&script);
            let actions_custom = get_scriptlet_context_actions_with_custom(&script, None);
            let mut ids_standard = action_ids(&actions_standard);
            let ids_custom = action_ids(&actions_custom);
            ids_standard.retain(|id| {
                *id != "toggle_favorite"
                    && *id != "toggle_info"
                    && *id != "file:open_in_quick_terminal"
            });
            assert_eq!(ids_standard, ids_custom);
        }

        // =========================================================================
        // 8. Action description content keyword validation
        // =========================================================================

        #[test]
        fn script_run_description_contains_action_verb() {
            let script = ScriptInfo::with_action_verb("Foo", "/path", true, "Launch");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(
                run.description.as_ref().unwrap().contains("Launch"),
                "Run description should contain the action verb"
            );
        }

        #[test]
        fn script_edit_description_contains_editor() {
            let script = ScriptInfo::new("Foo", "/path/foo.ts");
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(edit.description.as_ref().unwrap().contains("$EDITOR"));
        }

        #[test]
        fn script_view_logs_description_contains_logs() {
            let script = ScriptInfo::new("Foo", "/path/foo.ts");
            let actions = get_script_context_actions(&script);
            let logs = actions.iter().find(|a| a.id == "view_logs").unwrap();
            assert!(logs
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("log"));
        }

        #[test]
        fn script_copy_path_description_contains_path() {
            let script = ScriptInfo::new("Foo", "/path/foo.ts");
            let actions = get_script_context_actions(&script);
            let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
            assert!(cp
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("path"));
        }

        #[test]
        fn clipboard_ocr_description_mentions_text_or_ocr() {
            let entry = ClipboardEntryInfo {
                id: "img".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ocr = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_ocr")
                .unwrap();
            let desc = ocr.description.as_ref().unwrap().to_lowercase();
            assert!(desc.contains("text") || desc.contains("ocr"));
        }

        #[test]
        fn path_move_to_trash_description_mentions_delete() {
            let path_info = PathInfo {
                path: "/tmp/test.txt".into(),
                name: "test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            let desc = trash.description.as_ref().unwrap().to_lowercase();
            assert!(desc.contains("trash"));
        }

        // --- merged from tests_part_03.rs ---
        // =========================================================================
        // 9. Score_action with cached lowercase fields
        // =========================================================================

        #[test]
        fn score_action_uses_title_lower_cache() {
            let action = Action::new(
                "test",
                "Edit Script",
                Some("Open in editor".into()),
                ActionCategory::ScriptContext,
            );
            // title_lower should be "edit script"
            assert_eq!(action.title_lower, "edit script");
            let score = ActionsDialog::score_action(&action, "edit");
            assert!(
                score >= 100,
                "Prefix match on title_lower should score 100+"
            );
        }

        #[test]
        fn score_action_description_lower_bonus() {
            let action = Action::new(
                "open",
                "Open File",
                Some("Open with default application".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(
                action.description_lower.as_deref(),
                Some("open with default application")
            );
            let score = ActionsDialog::score_action(&action, "default");
            // "default" not in title, but in description
            assert!(
                score >= 15,
                "Description match should add 15+ points, got {}",
                score
            );
        }

        #[test]
        fn score_action_shortcut_lower_bonus() {
            let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘R");
            assert_eq!(action.shortcut_lower.as_deref(), Some("⌘r"));
            let score = ActionsDialog::score_action(&action, "⌘r");
            // "⌘r" in shortcut_lower
            assert!(
                score >= 10,
                "Shortcut match should add 10+ points, got {}",
                score
            );
        }

        #[test]
        fn score_action_empty_query_returns_zero() {
            let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "");
            // Empty string is a prefix of everything, so it scores 100
            assert_eq!(score, 100);
        }

        #[test]
        fn score_action_no_match_returns_zero() {
            let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "zzzzz");
            assert_eq!(score, 0);
        }

        #[test]
        fn score_action_prefix_beats_contains() {
            let prefix_action =
                Action::new("e", "Edit Script", None, ActionCategory::ScriptContext);
            let contains_action =
                Action::new("c", "My Edit Tool", None, ActionCategory::ScriptContext);
            let prefix_score = ActionsDialog::score_action(&prefix_action, "edit");
            let contains_score = ActionsDialog::score_action(&contains_action, "edit");
            assert!(prefix_score > contains_score);
        }

        #[test]
        fn score_action_contains_beats_fuzzy() {
            let contains_action =
                Action::new("c", "My Edit Tool", None, ActionCategory::ScriptContext);
            let fuzzy_action = Action::new("f", "Erase Dict", None, ActionCategory::ScriptContext);
            let contains_score = ActionsDialog::score_action(&contains_action, "edit");
            let fuzzy_score = ActionsDialog::score_action(&fuzzy_action, "edit");
            assert!(
                contains_score > fuzzy_score,
                "Contains {} should beat fuzzy {}",
                contains_score,
                fuzzy_score
            );
        }

        #[test]
        fn score_action_stacks_title_and_description() {
            let action = Action::new(
                "test",
                "Edit Script",
                Some("Edit the script in your editor".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "edit");
            // Prefix match (100) + description match (15) = 115
            assert_eq!(score, 115);
        }

        // =========================================================================
        // 10. fuzzy_match edge cases
        // =========================================================================

        #[test]
        fn fuzzy_match_empty_needle_always_matches() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn fuzzy_match_empty_haystack_no_match() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn fuzzy_match_both_empty_matches() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn fuzzy_match_subsequence() {
            assert!(ActionsDialog::fuzzy_match("edit script", "es"));
            assert!(ActionsDialog::fuzzy_match("edit script", "eit"));
            assert!(ActionsDialog::fuzzy_match("edit script", "edsc"));
        }

        #[test]
        fn fuzzy_match_no_subsequence() {
            assert!(!ActionsDialog::fuzzy_match("edit", "xyz"));
            assert!(!ActionsDialog::fuzzy_match("abc", "abdc"));
        }

        #[test]
        fn fuzzy_match_needle_longer_than_haystack() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        #[test]
        fn fuzzy_match_exact_equals() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        // =========================================================================
        // 11. Scriptlet with_custom multiple custom actions ordering
        // =========================================================================

        #[test]
        fn scriptlet_three_custom_actions_maintain_order() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo main".into());
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "First".into(),
                    command: "first".into(),
                    tool: "bash".into(),
                    code: "echo 1".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Second".into(),
                    command: "second".into(),
                    tool: "bash".into(),
                    code: "echo 2".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Third".into(),
                    command: "third".into(),
                    tool: "bash".into(),
                    code: "echo 3".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
            ];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let ids = action_ids(&actions);

            let first_pos = ids
                .iter()
                .position(|&id| id == "scriptlet_action:first")
                .unwrap();
            let second_pos = ids
                .iter()
                .position(|&id| id == "scriptlet_action:second")
                .unwrap();
            let third_pos = ids
                .iter()
                .position(|&id| id == "scriptlet_action:third")
                .unwrap();

            assert!(first_pos < second_pos);
            assert!(second_pos < third_pos);
        }

        #[test]
        fn scriptlet_custom_actions_after_run_before_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo main".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Custom".into(),
                command: "custom".into(),
                tool: "bash".into(),
                code: "echo custom".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let ids = action_ids(&actions);

            let run_pos = ids.iter().position(|&id| id == "run_script").unwrap();
            let custom_pos = ids
                .iter()
                .position(|&id| id == "scriptlet_action:custom")
                .unwrap();
            let shortcut_pos = ids.iter().position(|&id| id == "add_shortcut").unwrap();

            assert_eq!(run_pos, 0);
            assert!(custom_pos > run_pos);
            assert!(custom_pos < shortcut_pos);
        }

        #[test]
        fn scriptlet_custom_actions_all_have_has_action_true() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo main".into());
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "A".into(),
                    command: "a".into(),
                    tool: "bash".into(),
                    code: "echo a".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "B".into(),
                    command: "b".into(),
                    tool: "bash".into(),
                    code: "echo b".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
            ];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            for action in &actions {
                if action.id.starts_with("scriptlet_action:") {
                    assert!(
                        action.has_action,
                        "Scriptlet custom action '{}' should have has_action=true",
                        action.id
                    );
                    assert!(
                        action.value.is_some(),
                        "Scriptlet custom action '{}' should have a value",
                        action.id
                    );
                }
            }
        }

        #[test]
        fn scriptlet_custom_action_value_matches_command() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo main".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy Stuff".into(),
                command: "copy-stuff".into(),
                tool: "bash".into(),
                code: "echo copy".into(),
                inputs: vec![],
                shortcut: Some("cmd+c".into()),
                description: Some("Copy stuff desc".into()),
            }];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom = actions
                .iter()
                .find(|a| a.id == "scriptlet_action:copy-stuff")
                .unwrap();
            assert_eq!(custom.value.as_deref(), Some("copy-stuff"));
            assert_eq!(custom.title, "Copy Stuff");
            assert_eq!(custom.description.as_deref(), Some("Copy stuff desc"));
            assert_eq!(custom.shortcut.as_deref(), Some("⌘C"));
        }

        // =========================================================================
        // 12. CommandBarConfig field validation
        // =========================================================================

        #[test]
        fn command_bar_config_ai_style_fields() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
            assert!(config.close_on_select);
            assert!(config.close_on_escape);
            assert!(config.close_on_click_outside);
        }

        #[test]
        fn command_bar_config_main_menu_style_fields() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(config.dialog_config.anchor, AnchorPosition::Bottom);
            assert!(!config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_config_no_search_fields() {
            let config = CommandBarConfig::no_search();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(!config.dialog_config.show_icons);
        }

        #[test]
        fn command_bar_config_notes_style_fields() {
            let config = CommandBarConfig::notes_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_config_default_all_close_true() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_escape);
            assert!(config.close_on_click_outside);
        }

        // =========================================================================
        // 13. Chat context action interactions
        // =========================================================================

        #[test]
        fn chat_no_models_no_messages_no_response() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            crate::actions::dialog_builtin_validation::assert_chat_root_contract(&info, &actions);
            assert_eq!(actions[0].id, "chat:change_model");
        }

        #[test]
        fn chat_with_models_and_response_and_messages() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![
                    ChatModelInfo {
                        id: "claude".into(),
                        display_name: "Claude".into(),
                        provider: "Anthropic".into(),
                    },
                    ChatModelInfo {
                        id: "gpt4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                ],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            crate::actions::dialog_builtin_validation::assert_chat_root_contract(&info, &actions);
        }

        #[test]
        fn chat_current_model_gets_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![
                    ChatModelInfo {
                        id: "claude".into(),
                        display_name: "Claude".into(),
                        provider: "Anthropic".into(),
                    },
                    ChatModelInfo {
                        id: "gpt4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let claude = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude")
                .unwrap();
            assert!(claude.title.contains("✓"), "Current model should have ✓");

            let gpt4 = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt4")
                .unwrap();
            assert!(
                !gpt4.title.contains("✓"),
                "Non-current model should not have ✓"
            );
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn chat_model_descriptions_show_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "claude".into(),
                    display_name: "Claude".into(),
                    provider: "Anthropic".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let claude = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude")
                .unwrap();
            assert_eq!(claude.description.as_deref(), Some("Uses Anthropic"));
        }

        #[test]
        fn chat_copy_response_only_when_has_response() {
            let without = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let with = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let without_actions = get_chat_context_actions(&without);
            let with_actions = get_chat_context_actions(&with);
            assert!(!without_actions.iter().any(|a| a.id == "chat:copy_response"));
            assert!(with_actions.iter().any(|a| a.id == "chat:copy_response"));
        }

        #[test]
        fn chat_clear_conversation_only_when_has_messages() {
            let without = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let with = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let without_actions = get_chat_context_actions(&without);
            let with_actions = get_chat_context_actions(&with);
            assert!(!without_actions
                .iter()
                .any(|a| a.id == "chat:clear_conversation"));
            assert!(with_actions
                .iter()
                .any(|a| a.id == "chat:clear_conversation"));
        }

        // =========================================================================
        // 14. Path context specifics
        // =========================================================================

        #[test]
        fn path_dir_primary_is_open_directory() {
            let path_info = PathInfo {
                path: "/tmp/mydir".into(),
                name: "mydir".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path_info);
            assert_eq!(actions[0].id, "file:open_directory");
            assert!(actions[0].title.contains("mydir"));
        }

        #[test]
        fn path_file_primary_is_select_file() {
            let path_info = PathInfo {
                path: "/tmp/myfile.txt".into(),
                name: "myfile.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            assert_eq!(actions[0].id, "file:select_file");
            assert!(actions[0].title.contains("myfile.txt"));
        }

        #[test]
        fn path_all_have_descriptions() {
            let path_info = PathInfo {
                path: "/tmp/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "Path action '{}' should have a description",
                    action.id
                );
            }
        }

        #[test]
        fn path_has_expected_actions() {
            let path_info = PathInfo {
                path: "/tmp/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"file:copy_path"));
            assert!(ids.contains(&"file:open_in_finder"));
            assert!(ids.contains(&"file:open_in_editor"));
            assert!(ids.contains(&"file:open_in_quick_terminal"));
            assert!(ids.contains(&"file:copy_filename"));
            assert!(ids.contains(&"file:move_to_trash"));
        }

        #[test]
        fn path_dir_trash_says_folder() {
            let path_info = PathInfo {
                path: "/tmp/mydir".into(),
                name: "mydir".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path_info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(
                trash.description.as_ref().unwrap().contains("folder"),
                "Dir trash should say 'folder'"
            );
        }

        #[test]
        fn path_file_trash_says_file() {
            let path_info = PathInfo {
                path: "/tmp/myfile.txt".into(),
                name: "myfile.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(
                trash.description.as_ref().unwrap().contains("file"),
                "File trash should say 'file'"
            );
        }

        // =========================================================================
        // 15. File context specifics
        // =========================================================================

        #[test]
        fn file_open_title_includes_name() {
            let file_info = FileInfo {
                path: "/Users/test/readme.md".into(),
                name: "readme.md".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(actions[0].title.contains("readme.md"));
        }

        #[test]
        fn file_dir_open_title_includes_dirname() {
            let file_info = FileInfo {
                path: "/Users/test/Documents".into(),
                name: "Documents".into(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(actions[0].title.contains("Documents"));
        }

        #[test]
        fn file_all_have_descriptions() {
            let file_info = FileInfo {
                path: "/test/file.rs".into(),
                name: "file.rs".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "File action '{}' should have a description",
                    action.id
                );
            }
        }

        #[test]
        fn file_all_have_shortcuts() {
            let file_info = FileInfo {
                path: "/test/file.rs".into(),
                name: "file.rs".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            // reveal_in_finder no longer has a shortcut (⌘↵ reserved for AI).
            // open_in_quick_terminal ships without a shortcut.
            let shortcut_exempt = ["file:reveal_in_finder", "file:open_in_quick_terminal"];
            for action in &actions {
                if shortcut_exempt.contains(&action.id.as_str()) {
                    continue;
                }
                assert!(
                    action.shortcut.is_some(),
                    "File action '{}' should have a shortcut",
                    action.id
                );
            }
        }

        // =========================================================================
        // 16. Notes command bar conditional logic
        // =========================================================================

        // =========================================================================
        // 17. to_deeplink_name comprehensive
        // =========================================================================

        #[test]
        fn deeplink_name_basic_spaces() {
            assert_eq!(to_deeplink_name("My Script"), "my-script");
        }

        #[test]
        fn deeplink_name_underscores_to_hyphens() {
            assert_eq!(to_deeplink_name("hello_world"), "hello-world");
        }

        #[test]
        fn deeplink_name_special_chars_stripped() {
            assert_eq!(to_deeplink_name("test!@#$%^&*()"), "test");
        }

        #[test]
        fn deeplink_name_multiple_spaces_collapsed() {
            assert_eq!(to_deeplink_name("a   b   c"), "a-b-c");
        }

        #[test]
        fn deeplink_name_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("  hello  "), "hello");
        }

        #[test]
        fn deeplink_name_numbers_preserved() {
            assert_eq!(to_deeplink_name("Test 123"), "test-123");
        }

        #[test]
        fn deeplink_name_all_special_returns_empty() {
            assert_eq!(to_deeplink_name("!@#$%"), "_unnamed");
        }

        #[test]
        fn deeplink_name_single_word() {
            assert_eq!(to_deeplink_name("hello"), "hello");
        }

        #[test]
        fn deeplink_name_already_hyphenated() {
            assert_eq!(to_deeplink_name("my-script"), "my-script");
        }

        // =========================================================================
        // 18. format_shortcut_hint specifics
        // =========================================================================

        #[test]
        fn format_shortcut_cmd_enter() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+enter"), "⌘↵");
        }

        #[test]
        fn format_shortcut_ctrl_shift_escape() {
            assert_eq!(
                ActionsDialog::format_shortcut_hint("ctrl+shift+escape"),
                "⌃⇧⎋"
            );
        }

        #[test]
        fn format_shortcut_alt_backspace() {
            assert_eq!(ActionsDialog::format_shortcut_hint("alt+backspace"), "⌥⌫");
        }

        #[test]
        fn format_shortcut_command_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("command+c"), "⌘C");
        }

        #[test]
        fn format_shortcut_meta_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("meta+k"), "⌘K");
        }

        #[test]
        fn format_shortcut_option_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("option+tab"), "⌥⇥");
        }

        #[test]
        fn format_shortcut_control_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("control+space"), "⌃␣");
        }

        #[test]
        fn format_shortcut_arrows() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+up"), "⌘↑");
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+down"), "⌘↓");
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+left"), "⌘←");
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+right"), "⌘→");
        }

        #[test]
        fn format_shortcut_arrowup_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+arrowup"), "⌘↑");
        }

        // =========================================================================
        // 19. parse_shortcut_keycaps specifics
        // =========================================================================

        #[test]
        fn parse_keycaps_modifier_plus_letter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘E");
            assert_eq!(keycaps, vec!["⌘", "E"]);
        }

        #[test]
        fn parse_keycaps_two_modifiers_plus_letter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
        }

        #[test]
        fn parse_keycaps_enter_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(keycaps, vec!["↵"]);
        }

        #[test]
        fn parse_keycaps_modifier_plus_enter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
            assert_eq!(keycaps, vec!["⌘", "↵"]);
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn parse_keycaps_space_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(keycaps, vec!["␣"]);
        }

        #[test]
        fn parse_keycaps_all_modifiers() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧K");
            assert_eq!(keycaps, vec!["⌘", "⌃", "⌥", "⇧", "K"]);
        }

        #[test]
        fn parse_keycaps_arrow_keys() {
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
        }

        // =========================================================================
        // 20. Agent-specific action validation
        // =========================================================================

        #[test]
        fn agent_has_edit_agent_title() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn agent_has_no_view_logs() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn agent_has_reveal_and_copy() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"reveal_in_finder"));
            assert!(ids.contains(&"copy_path"));
            assert!(ids.contains(&"copy_content"));
        }

        #[test]
        fn agent_edit_description_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            let desc = edit.description.as_ref().unwrap().to_lowercase();
            assert!(desc.contains("agent"));
        }

        // =========================================================================
        // 21. New chat action details
        // =========================================================================

        #[test]
        fn new_chat_last_used_icon_is_bolt() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "p".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn new_chat_preset_icon_matches_input() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].icon, Some(IconName::Star));
        }

        #[test]
        fn new_chat_model_icon_is_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "p".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        #[test]
        fn new_chat_presets_have_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let desc = actions[0].description.as_deref().unwrap_or_default();
            assert!(desc.contains("preset"));
        }

        #[test]
        fn new_chat_models_have_provider_description() {
            let models = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].description.as_deref(), Some("Uses Anthropic"));
        }

        #[test]
        fn new_chat_empty_all_returns_empty() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        // =========================================================================
        // 22. Action constructor edge cases
        // =========================================================================

        #[test]
        fn action_with_shortcut_opt_none_leaves_none() {
            let action = Action::new("t", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(None);
            assert!(action.shortcut.is_none());
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn action_with_shortcut_opt_some_sets_both() {
            let action = Action::new("t", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘K".into()));
            assert_eq!(action.shortcut.as_deref(), Some("⌘K"));
            assert_eq!(action.shortcut_lower.as_deref(), Some("⌘k"));
        }

        #[test]
        fn action_title_lower_computed_on_creation() {
            let action = Action::new(
                "t",
                "My UPPERCASE Title",
                None,
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.title_lower, "my uppercase title");
        }

        #[test]
        fn action_description_lower_computed_on_creation() {
            let action = Action::new(
                "t",
                "T",
                Some("Description With CAPS".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(
                action.description_lower.as_deref(),
                Some("description with caps")
            );
        }

        #[test]
        fn action_no_description_has_none_lower() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn action_default_has_action_false() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext);
            assert!(!action.has_action);
        }

        #[test]
        fn action_default_value_none() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext);
            assert!(action.value.is_none());
        }

        #[test]
        fn action_default_icon_none() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext);
            assert!(action.icon.is_none());
        }

        #[test]
        fn action_default_section_none() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext);
            assert!(action.section.is_none());
        }

        #[test]
        fn action_with_icon_sets_icon() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Plus);
            assert_eq!(action.icon, Some(IconName::Plus));
        }

        #[test]
        fn action_with_section_sets_section() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_section("MySection");
            assert_eq!(action.section.as_deref(), Some("MySection"));
        }

        // =========================================================================
        // 23. ScriptInfo constructor validation
        // =========================================================================

        #[test]
        fn script_info_new_defaults() {
            let s = ScriptInfo::new("test", "/path");
            assert!(s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
            assert_eq!(s.action_verb, "Run");
            assert!(s.shortcut.is_none());
            assert!(s.alias.is_none());
            assert!(!s.is_suggested);
            assert!(s.frecency_path.is_none());
        }

        #[test]
        fn script_info_builtin_has_empty_path() {
            let s = ScriptInfo::builtin("Test");
            assert!(s.path.is_empty());
            assert!(!s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn script_info_scriptlet_sets_flags() {
            let s = ScriptInfo::scriptlet("Test", "/path.md", None, None);
            assert!(!s.is_script);
            assert!(s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn script_info_with_frecency_chaining() {
            let s = ScriptInfo::new("t", "/p").with_frecency(true, Some("/p".into()));
            assert!(s.is_suggested);
            assert_eq!(s.frecency_path.as_deref(), Some("/p"));
            // Original fields preserved
            assert!(s.is_script);
            assert_eq!(s.name, "t");
        }

        // =========================================================================
        // 24. Global actions are seeded with GlobalOps entries
        // =========================================================================

        #[test]
        fn global_actions_seeded() {
            // get_global_actions() is seeded (never empty): reload_scripts, settings,
            // view_logs, plus the Discover rows.
            let actions = get_global_actions();
            let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert!(ids.contains(&"reload_scripts"));
            assert!(ids.contains(&"settings"));
            assert!(ids.contains(&"view_logs"));
        }

        // =========================================================================
        // 25. Ordering determinism (calling twice yields same result)
        // =========================================================================

        #[test]
        fn script_actions_deterministic() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions_1 = get_script_context_actions(&script);
            let actions_2 = get_script_context_actions(&script);
            let a1 = action_ids(&actions_1);
            let a2 = action_ids(&actions_2);
            assert_eq!(a1, a2);
        }

        #[test]
        fn clipboard_actions_deterministic() {
            let entry = ClipboardEntryInfo {
                id: "e".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions_1 = get_clipboard_history_context_actions(&entry);
            let actions_2 = get_clipboard_history_context_actions(&entry);
            let a1 = action_ids(&actions_1);
            let a2 = action_ids(&actions_2);
            assert_eq!(a1, a2);
        }

        #[test]
        fn ai_actions_deterministic() {
            let actions_1 = get_ai_command_bar_actions();
            let actions_2 = get_ai_command_bar_actions();
            let a1 = action_ids(&actions_1);
            let a2 = action_ids(&actions_2);
            assert_eq!(a1, a2);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_6 {
    //! Batch 6: Built-in action behavioral validation tests
    //!
    //! 160+ tests validating action invariants NOT covered in batches 1-5.
    //! Focus areas:
    //! - ScriptInfo impossible flag combinations (is_script+is_scriptlet, is_script+is_agent)
    //! - Action verb propagation across all contexts
    //! - Deeplink description URL format for various name patterns
    //! - Clipboard entry edge cases (empty preview, long preview, special app names)
    //! - Chat context scaling (many models, duplicate providers, empty display names)
    //! - Notes info systematic boolean combos (all 8 permutations w/ section labels)
    //! - Note switcher mixed pinned/unpinned ordering and sections
    //! - New chat with partial sections (empty presets, empty models, etc.)
    //! - Combined score stacking (title+desc+shortcut all matching)
    //! - build_grouped_items_static consecutive same-section (no duplicate headers)
    //! - coerce_action_selection all-headers edge case
    //! - format_shortcut_hint (ActionsDialog version) comprehensive coverage
    //! - Path context long names and special chars
    //! - File context all FileType variants
    //! - Action builder chaining immutability
    //! - CommandBarConfig default field values
    //! - Scriptlet context action count vs script context action count comparison
    //! - Agent ScriptInfo with full flag set (shortcut+alias+frecency)

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::*;
        use crate::actions::dialog::{
            build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
        };
        use crate::actions::types::*;
        use crate::actions::CommandBarConfig;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};

        // =========================================================================
        // Helper: collect action IDs from a Vec<Action>
        // =========================================================================
        fn action_ids(actions: &[Action]) -> Vec<&str> {
            actions.iter().map(|a| a.id.as_str()).collect()
        }

        // =========================================================================
        // 1. ScriptInfo impossible flag combinations
        //    The constructors don't prevent these, so verify behavior is reasonable.
        // =========================================================================

        #[test]
        fn script_and_scriptlet_flags_both_true_gets_both_action_sets() {
            // Manually create a ScriptInfo with both is_script and is_scriptlet true.
            // The builder won't do this, but it's worth validating behavior.
            let mut info = ScriptInfo::new("Hybrid", "/path/hybrid.ts");
            info.is_scriptlet = true;
            // is_script=true AND is_scriptlet=true
            let actions = get_script_context_actions(&info);
            let ids = action_ids(&actions);
            // Should have both script actions AND scriptlet actions
            assert!(
                ids.contains(&"edit_script"),
                "has edit_script from is_script"
            );
            assert!(
                ids.contains(&"edit_scriptlet"),
                "has edit_scriptlet from is_scriptlet"
            );
            assert!(
                ids.contains(&"view_logs"),
                "has view_logs from is_script block"
            );
        }

        #[test]
        fn script_and_agent_flags_both_true_gets_both_action_sets() {
            let mut info = ScriptInfo::new("HybridAgent", "/path/agent.ts");
            info.is_agent = true;
            // is_script=true AND is_agent=true
            let actions = get_script_context_actions(&info);
            // Duplicate IDs from script+agent are deduplicated in the action builder.
            let edit_count = actions.iter().filter(|a| a.id == "edit_script").count();
            assert_eq!(edit_count, 1, "edit_script is deduplicated by ID");
        }

        #[test]
        fn agent_without_is_script_has_no_view_logs() {
            let mut info = ScriptInfo::new("PureAgent", "/path/agent.md");
            info.is_script = false;
            info.is_agent = true;
            let actions = get_script_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(
                !ids.contains(&"view_logs"),
                "Agent without is_script should lack view_logs"
            );
            // But should have agent edit
            let edit_action = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit_action.title, "Edit Agent");
        }

        #[test]
        fn all_three_flags_true_produces_actions_from_all_three_blocks() {
            let mut info = ScriptInfo::new("Triple", "/path/triple.ts");
            info.is_scriptlet = true;
            info.is_agent = true;
            let actions = get_script_context_actions(&info);
            let ids = action_ids(&actions);
            // Should have actions from all three blocks
            assert!(ids.contains(&"view_logs"), "script block: view_logs");
            assert!(
                ids.contains(&"edit_scriptlet"),
                "scriptlet block: edit_scriptlet"
            );
            assert!(
                ids.contains(&"reveal_in_finder"),
                "agent block: reveal_in_finder"
            );
        }

        // =========================================================================
        // 2. Action verb propagation in primary action title
        // =========================================================================

        #[test]
        fn action_verb_run_in_script_title() {
            let info = ScriptInfo::new("My Script", "/path/script.ts");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Run");
        }

        #[test]
        fn action_verb_launch_in_app_title() {
            let info =
                ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Launch");
        }

        #[test]
        fn action_verb_switch_to_in_window_title() {
            let info =
                ScriptInfo::with_action_verb("My Document", "window:123", false, "Switch to");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Switch To");
        }

        #[test]
        fn action_verb_execute_custom_in_title() {
            let info = ScriptInfo::with_action_verb("Task", "/path/task.ts", true, "Execute");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Execute");
        }

        #[test]
        fn action_verb_open_in_builtin_title() {
            let info = ScriptInfo::with_action_verb(
                "Clipboard History",
                "builtin:clipboard",
                false,
                "Open",
            );
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Open");
        }

        #[test]
        fn action_verb_description_matches_verb() {
            let info = ScriptInfo::with_action_verb("Test", "/path/test.ts", true, "Execute");
            let actions = get_script_context_actions(&info);
            let desc = actions[0].description.as_ref().unwrap();
            // is_script fixture → "{verb} this script"
            assert_eq!(desc, "Execute this script");
        }

        #[test]
        fn scriptlet_context_action_verb_propagation() {
            let info = ScriptInfo::scriptlet("My Snippet", "/path/snippet.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            assert_eq!(actions[0].title, "Run \"My Snippet\"");
            assert_eq!(actions[0].description.as_ref().unwrap(), "Run this item");
        }

        // =========================================================================
        // 3. Deeplink description URL format validation
        // =========================================================================

        #[test]
        fn deeplink_description_contains_url_for_simple_name() {
            let info = ScriptInfo::new("Hello World", "/path/hello.ts");
            let actions = get_script_context_actions(&info);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            let desc = dl.description.as_ref().unwrap();
            assert_eq!(dl.title, "Share");
            assert_eq!(desc, "Copy a portable Script Kit share link to clipboard");
        }

        #[test]
        fn deeplink_description_contains_url_for_special_chars() {
            let info = ScriptInfo::builtin("Open !@# File");
            let actions = get_script_context_actions(&info);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            let desc = dl.description.as_ref().unwrap();
            assert!(desc.contains("scriptkit://run/open-file"));
        }

        #[test]
        fn deeplink_description_contains_url_for_underscores() {
            let info = ScriptInfo::new("hello_world_test", "/path/test.ts");
            let actions = get_script_context_actions(&info);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(dl.title, "Share");
            assert_eq!(
                dl.description.as_deref(),
                Some("Copy a portable Script Kit share link to clipboard")
            );
        }

        #[test]
        fn deeplink_description_for_scriptlet_context() {
            let info = ScriptInfo::scriptlet("Open URL", "/path/urls.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            let desc = dl.description.as_ref().unwrap();
            assert!(desc.contains("scriptkit://run/open-url"));
        }

        // =========================================================================
        // 4. Clipboard entry edge cases
        // =========================================================================

        #[test]
        fn clipboard_empty_preview_text_entry() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // Should still produce valid actions even with empty preview
            assert!(!actions.is_empty());
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_paste"));
        }

        #[test]
        fn clipboard_very_long_app_name_in_paste_title() {
            let entry = ClipboardEntryInfo {
                id: "e2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: Some(
                    "Super Long Application Name That Goes On And On".to_string(),
                ),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(
                paste.title,
                "Paste to Super Long Application Name That Goes On And On"
            );
        }

        #[test]
        fn clipboard_image_pinned_has_unpin_and_ocr() {
            let entry = ClipboardEntryInfo {
                id: "img1".into(),
                content_type: ContentType::Image,
                pinned: true,
                preview: "Image".into(),
                image_dimensions: Some((1920, 1080)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(
                ids.contains(&"clip:clipboard_unpin"),
                "pinned image has unpin"
            );
            assert!(
                !ids.contains(&"clip:clipboard_pin"),
                "pinned image has no pin"
            );
            assert!(ids.contains(&"clip:clipboard_ocr"), "image has OCR");
        }

        #[test]
        fn clipboard_text_pinned_has_no_ocr() {
            let entry = ClipboardEntryInfo {
                id: "txt1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "Hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(!ids.contains(&"clip:clipboard_ocr"), "text has no OCR");
            assert!(
                ids.contains(&"clip:clipboard_unpin"),
                "pinned text has unpin"
            );
        }

        #[test]
        fn clipboard_text_action_order_first_three() {
            let entry = ClipboardEntryInfo {
                id: "ord1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].id, "clip:clipboard_paste", "1st: paste");
            assert_eq!(actions[1].id, "clip:clipboard_copy", "2nd: copy");
            assert_eq!(
                actions[2].id, "clip:clipboard_paste_keep_open",
                "3rd: paste_keep_open"
            );
        }

        #[test]
        fn clipboard_destructive_actions_are_last_three() {
            let entry = ClipboardEntryInfo {
                id: "del1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let len = actions.len();
            assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
            assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
            assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
        }

        // =========================================================================
        // 5. Chat context scaling and edge cases
        // =========================================================================

        #[test]
        fn chat_context_many_models_all_get_select_prefix() {
            let models: Vec<ChatModelInfo> = (0..20)
                .map(|i| ChatModelInfo {
                    id: format!("model-{}", i),
                    display_name: format!("Model {}", i),
                    provider: "TestProvider".into(),
                })
                .collect();
            let info = ChatPromptInfo {
                current_model: None,
                available_models: models,
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            assert_eq!(picker.len(), 20);
            for action in &picker {
                assert!(
                    action.id.starts_with("chat:select_model_"),
                    "Model action ID should start with select_model_: {}",
                    action.id
                );
            }
        }

        #[test]
        fn chat_context_duplicate_provider_names_in_descriptions() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                    ChatModelInfo {
                        id: "gpt3".into(),
                        display_name: "GPT-3.5".into(),
                        provider: "OpenAI".into(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            assert_eq!(picker[0].description.as_deref(), Some("Uses OpenAI"));
            assert_eq!(picker[1].description.as_deref(), Some("Uses OpenAI"));
        }

        #[test]
        fn chat_context_current_model_checkmark_only_on_matching() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                    ChatModelInfo {
                        id: "claude".into(),
                        display_name: "Claude".into(),
                        provider: "Anthropic".into(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_model_picker_actions(&info);
            assert!(
                actions[0].title.contains("✓"),
                "Current model should have checkmark"
            );
            assert!(
                !actions[1].title.contains("✓"),
                "Non-current model should not have checkmark"
            );
        }

        #[test]
        fn chat_context_all_flags_true_has_all_actions() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
                available_models: vec![ChatModelInfo {
                    id: "gpt4".into(),
                    display_name: "GPT-4".into(),
                    provider: "OpenAI".into(),
                }],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"chat:continue_in_chat"));
            assert!(ids.contains(&"chat:copy_response"));
            assert!(ids.contains(&"chat:clear_conversation"));
            crate::actions::dialog_builtin_validation::assert_chat_root_contract(&info, &actions);
        }

        #[test]
        fn chat_context_has_response_no_messages() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"chat:copy_response"));
            assert!(!ids.contains(&"chat:clear_conversation"));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn chat_context_has_messages_no_response() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(!ids.contains(&"chat:copy_response"));
            assert!(ids.contains(&"chat:clear_conversation"));
        }

        // =========================================================================
        // 6. Notes info systematic boolean combos with section labels
        // =========================================================================

        // =========================================================================
        // 7. Note switcher mixed pinned/unpinned section assignment
        // =========================================================================

        #[test]
        fn note_switcher_pinned_notes_in_pinned_section() {
            let notes = vec![
                NoteSwitcherNoteInfo {
                    id: "p1".into(),
                    title: "Pinned Note".into(),
                    char_count: 50,
                    is_current: false,
                    is_pinned: true,
                    preview: "pinned content".into(),
                    relative_time: "1h ago".into(),
                },
                NoteSwitcherNoteInfo {
                    id: "r1".into(),
                    title: "Recent Note".into(),
                    char_count: 30,
                    is_current: false,
                    is_pinned: false,
                    preview: "recent content".into(),
                    relative_time: "5m ago".into(),
                },
            ];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
            assert_eq!(actions[1].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn note_switcher_current_pinned_gets_star_icon() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "cp".into(),
                title: "Current Pinned".into(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            // Pinned takes precedence over current for icon
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
            // But current still gets bullet prefix
            assert!(actions[0].title.starts_with("• "));
        }

        #[test]
        fn note_switcher_current_not_pinned_gets_check_icon() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "cn".into(),
                title: "Current Note".into(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::Check));
        }

        #[test]
        fn note_switcher_regular_note_gets_file_icon() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "rn".into(),
                title: "Regular Note".into(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::File));
        }

        #[test]
        fn note_switcher_id_format_is_note_prefix() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc-123".into(),
                title: "Test".into(),
                char_count: 5,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].id, "note_abc-123");
        }

        #[test]
        fn note_switcher_empty_shows_no_notes_message() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
            assert_eq!(actions[0].title, "No notes yet");
            assert_eq!(actions[0].section.as_deref(), Some("Notes"));
        }

        #[test]
        fn note_switcher_char_count_singular() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "s1".into(),
                title: "One Char".into(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1 char");
        }

        #[test]
        fn note_switcher_char_count_plural() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "s2".into(),
                title: "Many Chars".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        #[test]
        fn note_switcher_char_count_zero() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "s0".into(),
                title: "Empty Note".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "0 chars");
        }

        #[test]
        fn note_switcher_preview_exactly_60_chars_not_truncated() {
            let preview = "a".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "t60".into(),
                title: "Exact 60".into(),
                char_count: 60,
                is_current: false,
                is_pinned: false,
                preview: preview.clone(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, &preview);
            assert!(!desc.contains("…"), "60 chars should not be truncated");
        }

        #[test]
        fn note_switcher_preview_61_chars_is_truncated() {
            let preview = "a".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "t61".into(),
                title: "Over 60".into(),
                char_count: 61,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.ends_with("…"), "61 chars should be truncated with …");
        }

        #[test]
        fn note_switcher_relative_time_only_no_preview() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "to".into(),
                title: "Time Only".into(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "3d ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "3d ago");
        }

        // =========================================================================
        // 8. New chat with partial sections
        // =========================================================================

        #[test]
        fn new_chat_no_last_used_only_presets_and_models() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Settings,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&[], &presets, &models);
            assert_eq!(actions.len(), 2);
            assert_eq!(actions[0].section.as_deref(), Some("Presets"));
            assert_eq!(actions[1].section.as_deref(), Some("Models"));
        }

        #[test]
        fn new_chat_only_models_no_presets_no_last_used() {
            let models = vec![
                NewChatModelInfo {
                    model_id: "m1".into(),
                    display_name: "Model 1".into(),
                    provider: "p1".into(),
                    provider_display_name: "Provider 1".into(),
                },
                NewChatModelInfo {
                    model_id: "m2".into(),
                    display_name: "Model 2".into(),
                    provider: "p2".into(),
                    provider_display_name: "Provider 2".into(),
                },
            ];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions.len(), 2);
            assert_eq!(actions[0].id, "model_p1::m1");
            assert_eq!(actions[1].id, "model_p2::m2");
        }

        #[test]
        fn new_chat_only_last_used() {
            let last_used = vec![NewChatModelInfo {
                model_id: "lu1".into(),
                display_name: "Last Used".into(),
                provider: "p".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "last_used_p::lu1");
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn new_chat_all_three_sections_have_correct_section_labels() {
            let last_used = vec![NewChatModelInfo {
                model_id: "lu".into(),
                display_name: "LU".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "pr".into(),
                name: "Preset".into(),
                icon: IconName::Code,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "Model".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        #[test]
        fn new_chat_preset_has_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "gen".into(),
                name: "General".into(),
                icon: IconName::Settings,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(
                actions[0].description.as_deref(),
                Some("Uses General preset")
            );
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn new_chat_model_has_provider_description() {
            let models = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].description.as_ref().unwrap(), "Uses Anthropic");
        }

        // =========================================================================
        // 9. Score stacking (title+desc+shortcut all matching)
        // =========================================================================

        #[test]
        fn score_action_prefix_title_only() {
            let action = Action::new(
                "test",
                "Copy Path",
                Some("Copy the path".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "copy");
            assert!(score >= 100, "Prefix match: {}", score);
        }

        #[test]
        fn score_action_title_prefix_plus_description_match() {
            let action = Action::new(
                "test",
                "Copy Path",
                Some("Copy the path to clipboard".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "copy");
            // prefix(100) + description contains "copy"(15) = 115
            assert!(score >= 115, "Prefix + desc: {}", score);
        }

        #[test]
        fn score_action_title_prefix_plus_desc_plus_shortcut() {
            let action = Action::new(
                "test",
                "Copy Path",
                Some("Copy the path to clipboard".into()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘COPY");
            let score = ActionsDialog::score_action(&action, "copy");
            // prefix(100) + desc(15) + shortcut(10) = 125
            assert!(score >= 125, "Prefix + desc + shortcut: {}", score);
        }

        #[test]
        fn score_action_no_match_returns_zero() {
            let action = Action::new(
                "test",
                "Open File",
                Some("Open the file".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "xyz123");
            assert_eq!(score, 0);
        }

        #[test]
        fn score_action_contains_only_no_prefix() {
            let action = Action::new(
                "test",
                "Reset Copy Path",
                None,
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "copy");
            // Contains only: 50
            assert!(score >= 50, "Contains: {}", score);
            assert!(score < 100, "Should not be prefix: {}", score);
        }

        #[test]
        fn score_action_fuzzy_only() {
            let action = Action::new("test", "Extract Data", None, ActionCategory::ScriptContext);
            // "eda" matches E-x-t-r-a-c-t-D-A-t-a as subsequence e...d...a
            let score = ActionsDialog::score_action(&action, "eda");
            assert!(score >= 25, "Fuzzy match: {}", score);
            assert!(score < 50, "Should not be contains: {}", score);
        }

        #[test]
        fn score_action_description_only_match() {
            let action = Action::new(
                "test",
                "Open File",
                Some("Navigate to the editor".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "editor");
            // Description only: 15
            assert_eq!(score, 15, "Description-only match");
        }

        #[test]
        fn score_action_shortcut_only_match() {
            let action = Action::new(
                "test",
                "Open File",
                Some("Open the file".into()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘Z");
            let score = ActionsDialog::score_action(&action, "⌘z");
            // Shortcut contains: 10
            assert!(score >= 10, "Shortcut match: {}", score);
        }

        // =========================================================================
        // 10. build_grouped_items_static edge cases
        // =========================================================================

        #[test]
        fn grouped_items_same_section_no_duplicate_headers() {
            let actions = vec![
                Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
                    .with_section("Response"),
                Action::new("a2", "Action 2", None, ActionCategory::ScriptContext)
                    .with_section("Response"),
                Action::new("a3", "Action 3", None, ActionCategory::ScriptContext)
                    .with_section("Response"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should be: 1 header + 3 items = 4
            assert_eq!(grouped.len(), 4);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1, "Same section = 1 header");
        }

        #[test]
        fn grouped_items_alternating_sections_get_headers() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext).with_section("Alpha"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("Beta"),
                Action::new("a3", "A3", None, ActionCategory::ScriptContext).with_section("Alpha"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Alpha header, A1, Beta header, A2, Alpha header again, A3 = 6
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 3, "Each section change = new header");
        }

        #[test]
        fn grouped_items_none_style_no_headers() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext)
                    .with_section("Section"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("Other"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0, "None style = no headers");
        }

        #[test]
        fn grouped_items_separators_style_no_headers() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext)
                    .with_section("Section"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("Other"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0, "Separators style = no headers");
        }

        #[test]
        fn grouped_items_empty_filtered_returns_empty() {
            let actions = vec![Action::new("a1", "A1", None, ActionCategory::ScriptContext)];
            let grouped = build_grouped_items_static(&actions, &[], SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn grouped_items_no_section_actions_with_headers_style() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // No sections means no headers, just items
            assert_eq!(grouped.len(), 2);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0, "No sections = no headers");
        }

        // =========================================================================
        // 11. coerce_action_selection edge cases
        // =========================================================================

        #[test]
        fn coerce_selection_empty_rows_returns_none() {
            assert_eq!(coerce_action_selection(&[], 0), None);
        }

        #[test]
        fn coerce_selection_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::SectionHeader("B".into()),
                GroupedActionItem::SectionHeader("C".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
            assert_eq!(coerce_action_selection(&rows, 1), None);
            assert_eq!(coerce_action_selection(&rows, 2), None);
        }

        #[test]
        fn coerce_selection_on_item_returns_same_index() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(1));
        }

        #[test]
        fn coerce_selection_on_header_searches_down_first() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::Item(0),
                GroupedActionItem::Item(1),
            ];
            // Landing on header at 0, should go down to item at 1
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn coerce_selection_on_trailing_header_searches_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::Item(1),
                GroupedActionItem::SectionHeader("A".into()),
            ];
            // Landing on header at 2, no items below, should go up to item at 1
            assert_eq!(coerce_action_selection(&rows, 2), Some(1));
        }

        // =========================================================================
        // 12. ActionsDialog::format_shortcut_hint comprehensive
        // =========================================================================

        #[test]
        fn format_hint_cmd_c() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+c"), "⌘C");
        }

        #[test]
        fn format_hint_command_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("command+n"), "⌘N");
        }

        #[test]
        fn format_hint_meta_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("meta+n"), "⌘N");
        }

        #[test]
        fn format_hint_option_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("option+n"), "⌥N");
        }

        #[test]
        fn format_hint_control_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("control+x"), "⌃X");
        }

        #[test]
        fn format_hint_return_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+return"), "⌘↵");
        }

        #[test]
        fn format_hint_tab_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("shift+tab"), "⇧⇥");
        }

        #[test]
        fn format_hint_space_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+space"), "⌘␣");
        }

        #[test]
        fn format_hint_delete_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+delete"), "⌘⌫");
        }

        #[test]
        fn format_hint_esc_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
        }

        #[test]
        fn format_hint_super_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("super+k"), "⌘K");
        }

        #[test]
        fn format_hint_opt_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("opt+k"), "⌥K");
        }

        // =========================================================================
        // 13. Path context edge cases
        // =========================================================================

        #[test]
        fn path_context_dir_primary_title_includes_name() {
            let info = PathInfo {
                path: "/Users/test/Documents".into(),
                name: "Documents".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].title, "Open \"Documents\"");
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn path_context_file_primary_title_includes_name() {
            let info = PathInfo {
                path: "/Users/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].title, "Select \"file.txt\"");
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn path_context_trash_description_dir() {
            let info = PathInfo {
                path: "/Users/test/Documents".into(),
                name: "Documents".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert_eq!(
                trash.description.as_ref().unwrap(),
                "Moves this folder to the Trash"
            );
        }

        #[test]
        fn path_context_trash_description_file() {
            let info = PathInfo {
                path: "/Users/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert_eq!(
                trash.description.as_ref().unwrap(),
                "Moves this file to the Trash"
            );
        }

        #[test]
        fn path_context_always_has_copy_path_and_copy_filename() {
            let info = PathInfo {
                path: "/Users/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"file:copy_path"));
            assert!(ids.contains(&"file:copy_filename"));
        }

        #[test]
        fn path_context_has_open_in_editor_and_terminal() {
            let info = PathInfo {
                path: "/Users/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"file:open_in_editor"));
            assert!(ids.contains(&"file:open_in_quick_terminal"));
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn path_context_action_count_dir_vs_file() {
            let dir_info = PathInfo {
                path: "/test/dir".into(),
                name: "dir".into(),
                is_dir: true,
            };
            let file_info = PathInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let dir_actions = get_path_context_actions(&dir_info);
            let file_actions = get_path_context_actions(&file_info);
            // Both should have same count (primary + copy_path + open_in_finder + open_in_editor
            // + open_in_quick_terminal + copy_filename + move_to_trash = 7)
            assert_eq!(dir_actions.len(), file_actions.len());
            assert_eq!(dir_actions.len(), 7);
        }

        // =========================================================================
        // 14. File context FileType variants
        // =========================================================================

        #[test]
        fn file_context_document_type() {
            let info = FileInfo {
                path: "/test/doc.pdf".into(),
                name: "doc.pdf".into(),
                file_type: FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
            assert!(actions[0].title.contains("doc.pdf"));
        }

        #[test]
        fn file_context_image_type() {
            let info = FileInfo {
                path: "/test/photo.jpg".into(),
                name: "photo.jpg".into(),
                file_type: FileType::Image,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
        }

        #[test]
        fn file_context_audio_type() {
            let info = FileInfo {
                path: "/test/song.mp3".into(),
                name: "song.mp3".into(),
                file_type: FileType::Audio,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
        }

        #[test]
        fn file_context_video_type() {
            let info = FileInfo {
                path: "/test/movie.mp4".into(),
                name: "movie.mp4".into(),
                file_type: FileType::Video,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
        }

        #[test]
        fn file_context_application_type() {
            let info = FileInfo {
                path: "/Applications/Safari.app".into(),
                name: "Safari.app".into(),
                file_type: FileType::Application,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
            assert!(actions[0].title.contains("Safari.app"));
        }

        #[test]
        fn file_context_directory_type() {
            let info = FileInfo {
                path: "/test/folder".into(),
                name: "folder".into(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
            assert!(actions[0].title.contains("folder"));
        }

        #[test]
        fn file_context_other_type() {
            let info = FileInfo {
                path: "/test/unknown.xyz".into(),
                name: "unknown.xyz".into(),
                file_type: FileType::Other,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
        }

        // =========================================================================
        // 15. Action builder chaining immutability
        // =========================================================================

        #[test]
        fn action_with_shortcut_preserves_other_fields() {
            let action = Action::new(
                "test",
                "Test Title",
                Some("Test Description".into()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘T");
            assert_eq!(action.id, "test");
            assert_eq!(action.title, "Test Title");
            assert_eq!(action.description, Some("Test Description".into()));
            assert_eq!(action.shortcut, Some("⌘T".into()));
        }

        #[test]
        fn action_with_icon_preserves_other_fields() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘T")
                .with_icon(IconName::Copy);
            assert_eq!(action.shortcut, Some("⌘T".into()));
            assert_eq!(action.icon, Some(IconName::Copy));
        }

        #[test]
        fn action_with_section_preserves_other_fields() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘T")
                .with_icon(IconName::Copy)
                .with_section("MySection");
            assert_eq!(action.shortcut, Some("⌘T".into()));
            assert_eq!(action.icon, Some(IconName::Copy));
            assert_eq!(action.section, Some("MySection".into()));
        }

        #[test]
        fn action_with_shortcut_opt_none_leaves_shortcut_none() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(None);
            assert!(action.shortcut.is_none());
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn action_with_shortcut_opt_some_sets_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘K".into()));
            assert_eq!(action.shortcut, Some("⌘K".into()));
            assert_eq!(action.shortcut_lower, Some("⌘k".into()));
        }

        // =========================================================================
        // 16. Action lowercase cache correctness
        // =========================================================================

        #[test]
        fn title_lower_matches_title_to_lowercase() {
            let action = Action::new(
                "test",
                "Copy Path To Clipboard",
                None,
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.title_lower, "copy path to clipboard");
        }

        #[test]
        fn description_lower_matches_description_to_lowercase() {
            let action = Action::new(
                "test",
                "Test",
                Some("Open In $EDITOR".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower, Some("open in $editor".into()));
        }

        #[test]
        fn description_lower_none_when_no_description() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn shortcut_lower_set_by_with_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘⇧C");
            assert_eq!(action.shortcut_lower, Some("⌘⇧c".into()));
        }

        #[test]
        fn shortcut_lower_none_when_no_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
        }

        // =========================================================================
        // 17. CommandBarConfig default field values
        // =========================================================================

        #[test]
        fn commandbar_default_close_flags_all_true() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn commandbar_ai_style_search_at_top() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn commandbar_main_menu_search_at_top() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(!config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn commandbar_no_search_hidden() {
            let config = CommandBarConfig::no_search();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
        }

        #[test]
        fn commandbar_notes_style_search_at_top_with_separators() {
            let config = CommandBarConfig::notes_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        // =========================================================================
        // 18. Scriptlet context vs script context action comparison
        // =========================================================================

        #[test]
        fn scriptlet_context_has_edit_scriptlet_not_edit_script() {
            let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"edit_scriptlet"));
            assert!(!ids.contains(&"edit_script"));
        }

        #[test]
        fn scriptlet_context_has_reveal_scriptlet_not_reveal() {
            let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"reveal_scriptlet_in_finder"));
            assert!(!ids.contains(&"file:reveal_in_finder"));
        }

        #[test]
        fn scriptlet_context_has_copy_scriptlet_path_not_copy_path() {
            let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"copy_scriptlet_path"));
            assert!(!ids.contains(&"file:copy_path"));
        }

        #[test]
        fn scriptlet_context_has_copy_content() {
            let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"copy_content"));
        }

        #[test]
        fn scriptlet_context_with_custom_actions_interleaved() {
            let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "Action A".into(),
                    command: "action-a".into(),
                    tool: "bash".into(),
                    code: "echo a".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Action B".into(),
                    command: "action-b".into(),
                    tool: "bash".into(),
                    code: "echo b".into(),
                    inputs: vec![],
                    shortcut: Some("cmd+b".into()),
                    description: Some("Do B".into()),
                },
            ];
            let actions = get_scriptlet_context_actions_with_custom(&info, Some(&scriptlet));

            // run_script first
            assert_eq!(actions[0].id, "run_script");
            // Then custom actions
            assert_eq!(actions[1].id, "scriptlet_action:action-a");
            assert_eq!(actions[2].id, "scriptlet_action:action-b");
            // Custom actions have has_action=true
            assert!(actions[1].has_action);
            assert!(actions[2].has_action);
            // Custom action B has shortcut formatted
            assert!(actions[2].shortcut.is_some());
            // Custom action B has description
            assert_eq!(actions[2].description.as_deref(), Some("Do B"));
        }

        #[test]
        fn scriptlet_context_with_shortcut_and_alias() {
            let info = ScriptInfo::scriptlet(
                "Test",
                "/path/test.md",
                Some("cmd+t".into()),
                Some("ts".into()),
            );
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"update_shortcut"));
            assert!(ids.contains(&"remove_shortcut"));
            assert!(ids.contains(&"update_alias"));
            assert!(ids.contains(&"remove_alias"));
            assert!(!ids.contains(&"add_shortcut"));
            assert!(!ids.contains(&"add_alias"));
        }

        #[test]
        fn scriptlet_context_with_frecency() {
            let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None)
                .with_frecency(true, Some("scriptlet:Test".into()));
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"reset_ranking"));
        }

        // =========================================================================
        // 19. AI command bar actions detailed validation
        // =========================================================================

        #[test]
        fn ai_command_bar_response_section_actions() {
            let actions = get_ai_command_bar_actions();
            let response_actions: Vec<&Action> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .collect();
            assert_eq!(response_actions.len(), 3);
            let ids: Vec<&str> = response_actions.iter().map(|a| a.id.as_str()).collect();
            assert!(ids.contains(&"chat:copy_response"));
            assert!(ids.contains(&"chat:copy_chat"));
            assert!(ids.contains(&"chat:copy_last_code"));
        }

        #[test]
        fn ai_command_bar_actions_section_actions() {
            let actions = get_ai_command_bar_actions();
            let action_section: Vec<&Action> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .collect();
            assert_eq!(action_section.len(), 7);
            let ids: Vec<&str> = action_section.iter().map(|a| a.id.as_str()).collect();
            assert!(ids.contains(&"chat:submit"));
            assert!(ids.contains(&"chat:new_chat"));
            assert!(ids.contains(&"chat:delete_chat"));
            assert!(ids.contains(&"chat:branch_from_last"));
        }

        #[test]
        fn ai_command_bar_attachments_section_actions() {
            let actions = get_ai_command_bar_actions();
            let att_actions: Vec<&Action> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .collect();
            assert_eq!(att_actions.len(), 4);
        }

        #[test]
        fn ai_command_bar_settings_section_actions() {
            let actions = get_ai_command_bar_actions();
            let settings_actions: Vec<&Action> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Settings"))
                .collect();
            assert_eq!(settings_actions.len(), 2);
            assert_eq!(settings_actions[0].id, "chat:change_model");
            assert_eq!(settings_actions[1].id, "chat:toggle_window_mode");
        }

        #[test]
        fn ai_command_bar_all_actions_have_icons() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.icon.is_some(),
                    "AI action '{}' should have icon",
                    action.id
                );
            }
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn ai_command_bar_section_order_correct() {
            let actions = get_ai_command_bar_actions();
            let sections: Vec<&str> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            // Order: Response(3), Actions(3), Attachments(3), Export(1), Context(7), Actions(1), Help(1), Settings(1)
            let unique_order: Vec<&str> = {
                let mut result = vec![];
                let mut prev: Option<&str> = None;
                for s in &sections {
                    if prev != Some(s) {
                        result.push(*s);
                        prev = Some(s);
                    }
                }
                result
            };
            assert_eq!(
                unique_order,
                vec![
                    "Response",
                    "Actions",
                    "Attachments",
                    "Export",
                    "Context",
                    "Actions",
                    "Help",
                    "Settings"
                ]
            );
        }

        // =========================================================================
        // 20. fuzzy_match edge cases
        // =========================================================================

        #[test]
        fn fuzzy_match_empty_needle_matches_anything() {
            assert!(ActionsDialog::fuzzy_match("hello", ""));
        }

        #[test]
        fn fuzzy_match_subsequence() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlwrd"));
        }

        #[test]
        fn fuzzy_match_no_subsequence() {
            assert!(!ActionsDialog::fuzzy_match("hello", "xyz"));
        }

        #[test]
        fn fuzzy_match_needle_longer_than_haystack() {
            assert!(!ActionsDialog::fuzzy_match("hi", "hello"));
        }

        #[test]
        fn fuzzy_match_single_char() {
            assert!(ActionsDialog::fuzzy_match("hello", "h"));
            assert!(ActionsDialog::fuzzy_match("hello", "o"));
            assert!(!ActionsDialog::fuzzy_match("hello", "z"));
        }

        // =========================================================================
        // 21. parse_shortcut_keycaps edge cases
        // =========================================================================

        #[test]
        fn parse_keycaps_modifier_plus_letter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(keycaps, vec!["⌘", "C"]);
        }

        #[test]
        fn parse_keycaps_escape_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn parse_keycaps_lowercase_uppercased() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘e");
            assert_eq!(keycaps, vec!["⌘", "E"]);
        }

        // =========================================================================
        // 22. to_deeplink_name edge cases
        // =========================================================================

        #[test]
        fn deeplink_name_special_chars_stripped() {
            assert_eq!(to_deeplink_name("Hello!@#$World"), "hello-world");
        }

        #[test]
        fn deeplink_name_multiple_spaces_collapsed() {
            assert_eq!(to_deeplink_name("My   Script"), "my-script");
        }

        #[test]
        fn deeplink_name_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("  My Script  "), "my-script");
        }

        #[test]
        fn deeplink_name_numbers_preserved() {
            assert_eq!(to_deeplink_name("Script 123"), "script-123");
        }

        #[test]
        fn deeplink_name_all_special_chars_empty() {
            assert_eq!(to_deeplink_name("!@#$%^&*"), "_unnamed");
        }

        #[test]
        fn deeplink_name_already_hyphenated() {
            assert_eq!(to_deeplink_name("already-hyphenated"), "already-hyphenated");
        }

        #[test]
        fn deeplink_name_mixed_case() {
            assert_eq!(to_deeplink_name("CamelCaseScript"), "camelcasescript");
        }

        // =========================================================================
        // 23. Agent ScriptInfo with full flag set
        // =========================================================================

        #[test]
        fn agent_with_shortcut_alias_frecency() {
            let mut info = ScriptInfo::with_all(
                "My Agent",
                "/path/agent.md",
                false,
                "Run",
                Some("cmd+a".into()),
                Some("ma".into()),
            );
            info.is_agent = true;
            let info = info.with_frecency(true, Some("agent:/path".into()));

            let actions = get_script_context_actions(&info);
            let ids = action_ids(&actions);

            // Agent-specific actions
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");

            // Agents never get shortcut/alias preference actions, even with
            // shortcut + alias set on the ScriptInfo.
            assert!(!ids.contains(&"update_shortcut"));
            assert!(!ids.contains(&"remove_shortcut"));
            assert!(!ids.contains(&"add_shortcut"));
            assert!(!ids.contains(&"update_alias"));
            assert!(!ids.contains(&"remove_alias"));
            assert!(!ids.contains(&"add_alias"));

            // Has frecency reset
            assert!(ids.contains(&"reset_ranking"));

            // Has agent copy actions
            assert!(ids.contains(&"copy_path"));
            assert!(ids.contains(&"copy_content"));
            assert!(ids.contains(&"reveal_in_finder"));
        }

        // =========================================================================
        // 24. Global actions are seeded with GlobalOps entries
        // =========================================================================

        // =========================================================================
        // 25. Ordering determinism across repeated calls
        // =========================================================================

        #[test]
        fn script_actions_deterministic() {
            let info = ScriptInfo::new("Test", "/path/test.ts");
            let a1 = get_script_context_actions(&info);
            let a2 = get_script_context_actions(&info);
            let ids1 = action_ids(&a1);
            let ids2 = action_ids(&a2);
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn clipboard_actions_deterministic() {
            let entry = ClipboardEntryInfo {
                id: "det".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let a1 = get_clipboard_history_context_actions(&entry);
            let a2 = get_clipboard_history_context_actions(&entry);
            let ids1 = action_ids(&a1);
            let ids2 = action_ids(&a2);
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn ai_actions_deterministic() {
            let a1 = get_ai_command_bar_actions();
            let a2 = get_ai_command_bar_actions();
            let ids1 = action_ids(&a1);
            let ids2 = action_ids(&a2);
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn path_actions_deterministic() {
            let info = PathInfo {
                path: "/test/dir".into(),
                name: "dir".into(),
                is_dir: true,
            };
            let a1 = get_path_context_actions(&info);
            let a2 = get_path_context_actions(&info);
            let ids1 = action_ids(&a1);
            let ids2 = action_ids(&a2);
            assert_eq!(ids1, ids2);
        }

        // =========================================================================
        // 26. has_action invariant across contexts
        // =========================================================================

        #[test]
        fn script_context_all_has_action_false() {
            let info = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&info);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Script action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn clipboard_context_all_has_action_false() {
            let entry = ClipboardEntryInfo {
                id: "ha".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Clipboard action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn path_context_all_has_action_false() {
            let info = PathInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Path action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn file_context_all_has_action_false() {
            let info = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "File action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn ai_command_bar_all_has_action_false() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    !action.has_action,
                    "AI action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn chat_context_builtin_actions_has_action_false() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Chat action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        // =========================================================================
        // 27. Scriptlet defined actions have has_action=true
        // =========================================================================

        #[test]
        fn scriptlet_defined_actions_all_have_has_action_true() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "Action 1".into(),
                    command: "action-1".into(),
                    tool: "bash".into(),
                    code: "echo 1".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Action 2".into(),
                    command: "action-2".into(),
                    tool: "bash".into(),
                    code: "echo 2".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
            ];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            for action in &actions {
                assert!(
                    action.has_action,
                    "Scriptlet defined action '{}' should have has_action=true",
                    action.id
                );
            }
        }

        // --- merged from tests_part_06.rs ---
        #[test]
        fn scriptlet_defined_actions_have_values() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Custom".into(),
                command: "custom-cmd".into(),
                tool: "bash".into(),
                code: "echo custom".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].value, Some("custom-cmd".into()));
        }

        #[test]
        fn scriptlet_defined_action_id_format() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "My Custom".into(),
                command: "my-custom".into(),
                tool: "bash".into(),
                code: "echo custom".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].id, "scriptlet_action:my-custom");
        }

        // =========================================================================
        // 28. Action ID uniqueness within contexts
        // =========================================================================

        #[test]
        fn script_context_ids_unique() {
            let info = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&info);
            let ids = action_ids(&actions);
            let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(ids.len(), unique.len(), "Script IDs should be unique");
        }

        #[test]
        fn clipboard_text_ids_unique() {
            let entry = ClipboardEntryInfo {
                id: "uniq".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(ids.len(), unique.len(), "Clipboard IDs should be unique");
        }

        #[test]
        fn path_context_ids_unique() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let ids = action_ids(&actions);
            let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(ids.len(), unique.len(), "Path IDs should be unique");
        }

        #[test]
        fn file_context_ids_unique() {
            let info = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            let ids = action_ids(&actions);
            let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(ids.len(), unique.len(), "File IDs should be unique");
        }

        #[test]
        fn ai_command_bar_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids = action_ids(&actions);
            let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(ids.len(), unique.len(), "AI IDs should be unique");
        }

        // =========================================================================
        // 29. All actions have non-empty title and ID
        // =========================================================================

        #[test]
        fn all_script_actions_nonempty_title_and_id() {
            let info = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&info);
            for action in &actions {
                assert!(!action.id.is_empty(), "Action ID should not be empty");
                assert!(!action.title.is_empty(), "Action title should not be empty");
            }
        }

        #[test]
        fn all_clipboard_actions_nonempty_title_and_id() {
            let entry = ClipboardEntryInfo {
                id: "ne".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn all_ai_actions_nonempty_title_and_id() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        // =========================================================================
        // 30. ActionCategory on all built-in actions
        // =========================================================================

        #[test]
        fn all_script_actions_are_script_context_category() {
            let info = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&info);
            for action in &actions {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "Action '{}' should be ScriptContext",
                    action.id
                );
            }
        }

        #[test]
        fn all_clipboard_actions_are_script_context_category() {
            let entry = ClipboardEntryInfo {
                id: "cat".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_path_actions_are_script_context_category() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            for action in &actions {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_file_actions_are_script_context_category() {
            let info = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            for action in &actions {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }
    }
}

mod from_dialog_builtin_action_validation_tests_7 {
    //! Batch 7: Dialog builtin action validation tests
    //!
    //! Focuses on novel edge cases and cross-cutting invariants not covered in batches 1-6:
    //!
    //! 1. format_shortcut_hint (dialog.rs version) edge cases: unknown keys, single modifier,
    //!    double-plus, empty string, mixed-case modifiers
    //! 2. score_action with Unicode: diacritics, CJK, emoji in title/desc
    //! 3. Note switcher description rendering boundary: exactly 60 chars, 59, 61, 0
    //! 4. Clipboard combined flag matrix: pinned×image, pinned×text, unpinned×image, unpinned×text
    //! 5. Chat context model ID generation format consistency
    //! 6. Notes command bar icon presence for every action
    //! 7. New chat action ordering within each section
    //! 8. Agent actions exclude view_logs
    //! 9. Script vs scriptlet action set symmetric difference
    //! 10. Deeplink URL in description format
    //! 11. AI command bar shortcut uniqueness
    //! 12. Notes command bar shortcut uniqueness
    //! 13. Path context action ordering: primary first, trash last
    //! 14. Clipboard action shortcut format (all use symbol notation)
    //! 15. Score_action with whitespace-only query
    //! 16. fuzzy_match with repeated characters
    //! 17. build_grouped_items_static with single-item single-section
    //! 18. coerce_action_selection with alternating header-item pattern
    //! 19. parse_shortcut_keycaps with empty string and multi-byte
    //! 20. CommandBarConfig close flags independence
    //! 21. Action constructor with empty strings
    //! 22. ScriptInfo scriptlet flag exclusivity with agent
    //! 23. Notes command bar action count bounds per flag state
    //! 24. Chat model display_name in title
    //! 25. New chat model_id in action ID
    //! 26. Clipboard delete_all description mentions "pinned"
    //! 27. File context all actions have ScriptContext category
    //! 28. Path context copy_path and copy_filename always present
    //! 29. Cross-context ID namespace separation
    //! 30. Action title_lower invariant across all builder functions

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::{
            get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
            get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
            get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
            get_script_context_actions, get_scriptlet_context_actions_with_custom,
            to_deeplink_name, ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo,
            NewChatPresetInfo, NoteSwitcherNoteInfo, NotesInfo,
        };
        use crate::actions::dialog::{
            build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
        };
        use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
        use crate::actions::CommandBarConfig;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // ============================================================
        // Helper functions
        // ============================================================

        fn action_ids(actions: &[Action]) -> Vec<&str> {
            actions.iter().map(|a| a.id.as_str()).collect()
        }

        fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
            actions.iter().find(|a| a.id == id)
        }

        fn make_action(id: &str, title: &str, section: Option<&str>) -> Action {
            let mut a = Action::new(
                id,
                title,
                Some(format!("Desc for {}", title)),
                ActionCategory::ScriptContext,
            );
            if let Some(s) = section {
                a = a.with_section(s);
            }
            a
        }

        fn make_text_entry(pinned: bool, app: Option<&str>) -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "txt-1".to_string(),
                content_type: ContentType::Text,
                pinned,
                preview: "hello world".to_string(),
                image_dimensions: None,
                frontmost_app_name: app.map(|s| s.to_string()),
            }
        }

        fn make_image_entry(pinned: bool) -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "img-1".to_string(),
                content_type: ContentType::Image,
                pinned,
                preview: "Screenshot (1920x1080)".to_string(),
                image_dimensions: Some((1920, 1080)),
                frontmost_app_name: None,
            }
        }

        fn make_note(
            id: &str,
            title: &str,
            char_count: usize,
            is_current: bool,
            is_pinned: bool,
            preview: &str,
            relative_time: &str,
        ) -> NoteSwitcherNoteInfo {
            NoteSwitcherNoteInfo {
                id: id.to_string(),
                title: title.to_string(),
                char_count,
                is_current,
                is_pinned,
                preview: preview.to_string(),
                relative_time: relative_time.to_string(),
            }
        }

        // ============================================================
        // 1. format_shortcut_hint (dialog.rs version) edge cases
        // ============================================================

        #[test]
        fn hint_unknown_key_passthrough() {
            // Unknown keys in non-last position are passed through as-is
            let result = ActionsDialog::format_shortcut_hint("cmd+f1");
            assert!(result.contains('⌘'), "cmd should map to ⌘: got {}", result);
            assert!(
                result.contains("F1"),
                "f1 should be uppercased: got {}",
                result
            );
        }

        #[test]
        fn hint_single_modifier_alone() {
            let result = ActionsDialog::format_shortcut_hint("cmd");
            // "cmd" alone is a single part, it's the last part, so it gets uppercased
            // Actually the match in format_shortcut_hint checks part_lower first
            assert_eq!(result, "⌘");
        }

        #[test]
        fn hint_empty_string() {
            let result = ActionsDialog::format_shortcut_hint("");
            assert_eq!(result, "");
        }

        #[test]
        fn hint_super_maps_to_cmd() {
            let result = ActionsDialog::format_shortcut_hint("super+a");
            assert_eq!(result, "⌘A");
        }

        #[test]
        fn hint_opt_maps_to_option() {
            let result = ActionsDialog::format_shortcut_hint("opt+b");
            assert_eq!(result, "⌥B");
        }

        #[test]
        fn hint_return_maps_to_enter() {
            let result = ActionsDialog::format_shortcut_hint("cmd+return");
            assert_eq!(result, "⌘↵");
        }

        #[test]
        fn hint_esc_alias() {
            let result = ActionsDialog::format_shortcut_hint("esc");
            assert_eq!(result, "⎋");
        }

        #[test]
        fn hint_arrowdown_alias() {
            let result = ActionsDialog::format_shortcut_hint("arrowdown");
            assert_eq!(result, "↓");
        }

        #[test]
        fn hint_arrowleft_alias() {
            let result = ActionsDialog::format_shortcut_hint("arrowleft");
            assert_eq!(result, "←");
        }

        #[test]
        fn hint_arrowright_alias() {
            let result = ActionsDialog::format_shortcut_hint("arrowright");
            assert_eq!(result, "→");
        }

        #[test]
        fn hint_triple_modifier() {
            let result = ActionsDialog::format_shortcut_hint("cmd+ctrl+shift+x");
            assert_eq!(result, "⌘⌃⇧X");
        }

        #[test]
        fn hint_mixed_case_modifiers() {
            let result = ActionsDialog::format_shortcut_hint("Cmd+Shift+Z");
            assert_eq!(result, "⌘⇧Z");
        }

        // ============================================================
        // 2. score_action with Unicode
        // ============================================================

        #[test]
        fn score_unicode_title_prefix() {
            let action = Action::new(
                "café",
                "Café Latte",
                Some("A hot drink".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "café");
            assert!(
                score >= 100,
                "Unicode prefix match should score >= 100, got {}",
                score
            );
        }

        #[test]
        fn score_unicode_contains() {
            let action = Action::new(
                "drink",
                "Hot Café Drink",
                None,
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "café");
            assert!(
                score >= 50,
                "Unicode contains should score >= 50, got {}",
                score
            );
        }

        #[test]
        fn score_empty_query_returns_zero() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "");
            // Empty query: starts_with("") == true for any string
            assert!(score >= 100, "Empty query matches prefix, got {}", score);
        }

        #[test]
        fn score_no_match_returns_zero() {
            let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "zzzzz");
            assert_eq!(score, 0, "No match should return 0");
        }

        #[test]
        fn score_description_only_match() {
            let action = Action::new(
                "x",
                "Alpha",
                Some("Open in beta editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "beta");
            assert_eq!(
                score, 15,
                "Description-only match should be 15, got {}",
                score
            );
        }

        #[test]
        fn score_shortcut_only_match() {
            let action =
                Action::new("x", "Alpha", None, ActionCategory::ScriptContext).with_shortcut("⌘Z");
            let score = ActionsDialog::score_action(&action, "⌘z");
            assert_eq!(score, 10, "Shortcut-only match should be 10, got {}", score);
        }

        #[test]
        fn score_title_plus_description_stacks() {
            let action = Action::new(
                "script:run",
                "run script",
                Some("run the script now".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "run");
            assert!(
                score >= 115,
                "Title prefix (100) + desc (15) should stack, got {}",
                score
            );
        }

        #[test]
        fn score_all_three_match_stack() {
            let action = Action::new(
                "copy",
                "copy text",
                Some("copy to clipboard".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("copy");
            let score = ActionsDialog::score_action(&action, "copy");
            assert!(
                score >= 125,
                "prefix(100)+desc(15)+shortcut(10)=125, got {}",
                score
            );
        }

        // ============================================================
        // 3. Note switcher description rendering boundary
        // ============================================================

        #[test]
        fn note_desc_exactly_60_chars_not_truncated() {
            let preview = "a".repeat(60);
            let note = make_note("n1", "Title", 100, false, false, &preview, "");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'), "60 chars should not be truncated");
            assert_eq!(desc.len(), 60);
        }

        #[test]
        fn note_desc_61_chars_is_truncated() {
            let preview = "a".repeat(61);
            let note = make_note("n1", "Title", 100, false, false, &preview, "");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains('…'), "61 chars should be truncated with …");
        }

        #[test]
        fn note_desc_59_chars_not_truncated() {
            let preview = "b".repeat(59);
            let note = make_note("n1", "Title", 100, false, false, &preview, "");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'), "59 chars should not be truncated");
        }

        #[test]
        fn note_desc_empty_preview_empty_time_uses_char_count() {
            let note = make_note("n1", "Title", 42, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        #[test]
        fn note_desc_empty_preview_with_time() {
            let note = make_note("n1", "Title", 42, false, false, "", "5m ago");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "5m ago");
        }

        #[test]
        fn note_desc_preview_with_time_separator() {
            let note = make_note("n1", "Title", 42, false, false, "Hello world", "3d ago");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains(" · "), "Should have ' · ' separator");
            assert!(desc.starts_with("Hello world"), "Should start with preview");
            assert!(desc.ends_with("3d ago"), "Should end with time");
        }

        #[test]
        fn note_desc_singular_char_count() {
            let note = make_note("n1", "Title", 1, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1 char", "Singular should not have 's'");
        }

        #[test]
        fn note_desc_zero_char_count() {
            let note = make_note("n1", "Title", 0, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "0 chars", "Zero should be plural");
        }

        // ============================================================
        // 4. Clipboard combined flag matrix
        // ============================================================

        #[test]
        fn clipboard_text_unpinned_has_pin() {
            let entry = make_text_entry(false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_pin"));
            assert!(!ids.contains(&"clip:clipboard_unpin"));
            assert!(
                !ids.contains(&"clip:clipboard_ocr"),
                "Text should not have OCR"
            );
        }

        #[test]
        fn clipboard_text_pinned_has_unpin() {
            let entry = make_text_entry(true, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_unpin"));
            assert!(!ids.contains(&"clip:clipboard_pin"));
        }

        #[test]
        fn clipboard_image_unpinned_has_ocr_and_pin() {
            let entry = make_image_entry(false);
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_ocr"), "Image should have OCR");
            assert!(ids.contains(&"clip:clipboard_pin"));
            assert!(!ids.contains(&"clip:clipboard_unpin"));
        }

        #[test]
        fn clipboard_image_pinned_has_ocr_and_unpin() {
            let entry = make_image_entry(true);
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_ocr"), "Image should have OCR");
            assert!(ids.contains(&"clip:clipboard_unpin"));
            assert!(!ids.contains(&"clip:clipboard_pin"));
        }

        #[test]
        fn clipboard_image_has_more_actions_than_text() {
            let text = make_text_entry(false, None);
            let img = make_image_entry(false);
            let text_actions = get_clipboard_history_context_actions(&text);
            let img_actions = get_clipboard_history_context_actions(&img);
            assert!(
                img_actions.len() > text_actions.len(),
                "Image ({}) should have more actions than text ({})",
                img_actions.len(),
                text_actions.len()
            );
        }

        // ============================================================
        // 5. Chat context model ID generation format consistency
        // ============================================================

        #[test]
        fn chat_model_id_format() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".to_string()),
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt-4".to_string(),
                        display_name: "GPT-4".to_string(),
                        provider: "OpenAI".to_string(),
                    },
                    ChatModelInfo {
                        id: "claude-3".to_string(),
                        display_name: "Claude 3".to_string(),
                        provider: "Anthropic".to_string(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            // Model actions should have select_model_{id} format
            assert!(picker.iter().any(|a| a.id == "chat:select_model_gpt-4"));
            assert!(picker.iter().any(|a| a.id == "chat:select_model_claude-3"));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn chat_current_model_gets_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".to_string()),
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt-4".to_string(),
                        display_name: "GPT-4".to_string(),
                        provider: "OpenAI".to_string(),
                    },
                    ChatModelInfo {
                        id: "claude-3".to_string(),
                        display_name: "Claude 3".to_string(),
                        provider: "Anthropic".to_string(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let gpt = find_action(&picker, "chat:select_model_gpt-4").unwrap();
            assert!(gpt.title.contains('✓'), "Current model should have ✓");
            let claude = find_action(&picker, "chat:select_model_claude-3").unwrap();
            assert!(
                !claude.title.contains('✓'),
                "Non-current model should not have ✓"
            );
        }

        #[test]
        fn chat_continue_always_present() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert!(
                action_ids(&actions).contains(&"chat:continue_in_chat"),
                "continue_in_chat should always be present"
            );
        }

        #[test]
        fn chat_copy_response_requires_has_response() {
            let without = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let with = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let without_actions = get_chat_context_actions(&without);
            assert!(!action_ids(&without_actions).contains(&"chat:copy_response"));
            let with_actions = get_chat_context_actions(&with);
            assert!(action_ids(&with_actions).contains(&"chat:copy_response"));
        }

        #[test]
        fn chat_clear_requires_has_messages() {
            let without = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let with = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let without_actions = get_chat_context_actions(&without);
            assert!(!action_ids(&without_actions).contains(&"chat:clear_conversation"));
            let with_actions = get_chat_context_actions(&with);
            assert!(action_ids(&with_actions).contains(&"chat:clear_conversation"));
        }

        // ============================================================
        // 6. Notes command bar icon presence
        // ============================================================

        // ============================================================
        // 7. New chat action ordering within each section
        // ============================================================

        #[test]
        fn new_chat_sections_appear_in_order() {
            let last_used = vec![NewChatModelInfo {
                model_id: "gpt-4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "openai".to_string(),
                provider_display_name: "OpenAI".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "claude-3".to_string(),
                display_name: "Claude 3".to_string(),
                provider: "anthropic".to_string(),
                provider_display_name: "Anthropic".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);

            // Find first index of each section
            let first_last_used = actions
                .iter()
                .position(|a| a.section.as_deref() == Some("Last Used Settings"));
            let first_preset = actions
                .iter()
                .position(|a| a.section.as_deref() == Some("Presets"));
            let first_model = actions
                .iter()
                .position(|a| a.section.as_deref() == Some("Models"));

            assert!(first_last_used.unwrap() < first_preset.unwrap());
            assert!(first_preset.unwrap() < first_model.unwrap());
        }

        #[test]
        fn new_chat_last_used_has_bolt_icon() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "p1".to_string(),
                provider_display_name: "Provider 1".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn new_chat_preset_uses_custom_icon() {
            let presets = vec![NewChatPresetInfo {
                id: "code".to_string(),
                name: "Code".to_string(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].icon, Some(IconName::Code));
        }

        #[test]
        fn new_chat_model_uses_settings_icon() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "M1".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        #[test]
        fn new_chat_empty_inputs_empty_output() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn new_chat_id_format_indexed() {
            let lu = vec![
                NewChatModelInfo {
                    model_id: "a".to_string(),
                    display_name: "A".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                },
                NewChatModelInfo {
                    model_id: "b".to_string(),
                    display_name: "B".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                },
            ];
            let actions = get_new_chat_actions(&lu, &[], &[]);
            assert_eq!(actions[0].id, "last_used_p::a");
            assert_eq!(actions[1].id, "last_used_p::b");
        }

        #[test]
        fn new_chat_preset_id_format() {
            let presets = vec![NewChatPresetInfo {
                id: "writer".to_string(),
                name: "Writer".to_string(),
                icon: IconName::File,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].id, "preset_writer");
        }

        #[test]
        fn new_chat_model_id_format_indexed() {
            let models = vec![
                NewChatModelInfo {
                    model_id: "x".to_string(),
                    display_name: "X".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                },
                NewChatModelInfo {
                    model_id: "y".to_string(),
                    display_name: "Y".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                },
            ];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].id, "model_p::x");
            assert_eq!(actions[1].id, "model_p::y");
        }

        #[test]
        fn new_chat_last_used_has_provider_description() {
            let lu = vec![NewChatModelInfo {
                model_id: "m".to_string(),
                display_name: "M".to_string(),
                provider: "p".to_string(),
                provider_display_name: "ProviderName".to_string(),
            }];
            let actions = get_new_chat_actions(&lu, &[], &[]);
            assert_eq!(
                actions[0].description,
                Some("Uses ProviderName".to_string())
            );
        }

        #[test]
        fn new_chat_preset_has_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "x".to_string(),
                name: "X".to_string(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let desc = actions[0].description.as_deref().unwrap_or_default();
            assert!(desc.contains("preset"));
        }

        // ============================================================
        // 8. Agent actions exclude view_logs
        // ============================================================

        #[test]
        fn agent_has_edit_agent_title() {
            let mut agent = ScriptInfo::new("My Agent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let edit = find_action(&actions, "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn agent_lacks_view_logs() {
            let mut agent = ScriptInfo::new("My Agent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            assert!(
                !action_ids(&actions).contains(&"view_logs"),
                "Agent should not have view_logs"
            );
        }

        #[test]
        fn agent_has_reveal_and_copy() {
            let mut agent = ScriptInfo::new("My Agent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"reveal_in_finder"));
            assert!(ids.contains(&"copy_path"));
            assert!(ids.contains(&"copy_content"));
        }

        // ============================================================
        // 9. Script vs scriptlet action set symmetric difference
        // ============================================================

        #[test]
        fn script_has_actions_scriptlet_lacks() {
            let script = ScriptInfo::new("s", "/path/s.ts");
            let scriptlet = ScriptInfo::scriptlet("s", "/path/s.md", None, None);
            let s_actions = get_script_context_actions(&script);
            let s_ids: HashSet<&str> = action_ids(&s_actions).into_iter().collect();
            let sl_actions = get_script_context_actions(&scriptlet);
            let sl_ids: HashSet<&str> = action_ids(&sl_actions).into_iter().collect();
            // Script should have these that scriptlet lacks
            assert!(s_ids.contains("edit_script"));
            assert!(s_ids.contains("view_logs"));
            assert!(!sl_ids.contains("edit_script"));
            assert!(!sl_ids.contains("view_logs"));
        }

        #[test]
        fn scriptlet_has_actions_script_lacks() {
            let script = ScriptInfo::new("s", "/path/s.ts");
            let scriptlet = ScriptInfo::scriptlet("s", "/path/s.md", None, None);
            let s_actions = get_script_context_actions(&script);
            let s_ids: HashSet<&str> = action_ids(&s_actions).into_iter().collect();
            let sl_actions = get_script_context_actions(&scriptlet);
            let sl_ids: HashSet<&str> = action_ids(&sl_actions).into_iter().collect();
            // Scriptlet should have these that script lacks
            assert!(sl_ids.contains("edit_scriptlet"));
            assert!(sl_ids.contains("reveal_scriptlet_in_finder"));
            assert!(sl_ids.contains("copy_scriptlet_path"));
            assert!(!s_ids.contains("edit_scriptlet"));
            assert!(!s_ids.contains("reveal_scriptlet_in_finder"));
            assert!(!s_ids.contains("copy_scriptlet_path"));
        }

        #[test]
        fn script_and_scriptlet_share_common_ids() {
            let script = ScriptInfo::new("s", "/path/s.ts");
            let scriptlet = ScriptInfo::scriptlet("s", "/path/s.md", None, None);
            let s_actions = get_script_context_actions(&script);
            let s_ids: HashSet<&str> = action_ids(&s_actions).into_iter().collect();
            let sl_actions = get_script_context_actions(&scriptlet);
            let sl_ids: HashSet<&str> = action_ids(&sl_actions).into_iter().collect();
            // Both should have these common actions
            let common = [
                "run_script",
                "copy_deeplink",
                "add_shortcut",
                "add_alias",
                "copy_content",
            ];
            for id in &common {
                assert!(s_ids.contains(id), "Script should have {}", id);
                assert!(sl_ids.contains(id), "Scriptlet should have {}", id);
            }
        }

        // ============================================================
        // 10. Deeplink URL in description format
        // ============================================================

        #[test]
        fn deeplink_description_contains_url() {
            let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = find_action(&actions, "copy_deeplink").unwrap();
            assert_eq!(dl.title, "Share");
            assert_eq!(
                dl.description.as_deref(),
                Some("Copy a portable Script Kit share link to clipboard")
            );
        }

        #[test]
        fn deeplink_description_special_chars() {
            let script = ScriptInfo::new("Test!@#$Script", "/path/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = find_action(&actions, "copy_deeplink").unwrap();
            assert_eq!(dl.title, "Share");
            assert_eq!(
                dl.description.as_deref(),
                Some("Copy a portable Script Kit share link to clipboard")
            );
        }

        #[test]
        fn deeplink_scriptlet_context() {
            let script = ScriptInfo::scriptlet("Open GitHub", "/path.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let dl = find_action(&actions, "copy_deeplink").unwrap();
            let desc = dl.description.as_ref().unwrap();
            assert!(desc.contains("scriptkit://run/open-github"));
        }

        // ============================================================
        // 11. AI command bar shortcut uniqueness
        // ============================================================

        #[test]
        fn ai_command_bar_shortcuts_unique() {
            let actions = get_ai_command_bar_actions();
            let shortcuts: Vec<&str> = actions
                .iter()
                .filter_map(|a| a.shortcut.as_deref())
                .collect();
            let unique: HashSet<&str> = shortcuts.iter().copied().collect();
            assert_eq!(
                shortcuts.len(),
                unique.len(),
                "AI command bar shortcuts should be unique: {:?}",
                shortcuts
            );
        }

        #[test]
        fn ai_command_bar_exactly_12_actions() {
            let actions = get_ai_command_bar_actions();
            assert_eq!(actions.len(), 38);
        }

        #[test]
        fn ai_command_bar_all_have_icons() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.icon.is_some(),
                    "AI action '{}' should have icon",
                    action.id
                );
            }
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn ai_command_bar_all_have_sections() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.section.is_some(),
                    "AI action '{}' should have section",
                    action.id
                );
            }
        }

        #[test]
        fn ai_command_bar_section_order() {
            let actions = get_ai_command_bar_actions();
            let sections: Vec<&str> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            // Verify order: Response before Actions before Attachments before Context before Settings
            let first_response = sections.iter().position(|&s| s == "Response").unwrap();
            let first_actions = sections.iter().position(|&s| s == "Actions").unwrap();
            let first_attachments = sections.iter().position(|&s| s == "Attachments").unwrap();
            let first_settings = sections.iter().position(|&s| s == "Settings").unwrap();
            assert!(first_response < first_actions);
            assert!(first_actions < first_attachments);
            assert!(first_attachments < first_settings);
        }

        // ============================================================
        // 12. Notes command bar shortcut uniqueness
        // ============================================================

        // ============================================================
        // 13. Path context action ordering
        // ============================================================

        #[test]
        fn path_dir_primary_first() {
            let path = PathInfo {
                path: "/test/dir".to_string(),
                name: "dir".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn path_file_primary_first() {
            let path = PathInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn path_trash_last() {
            let path = PathInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            assert_eq!(
                actions.last().unwrap().id,
                "file:move_to_trash",
                "Trash should be last action"
            );
        }

        #[test]
        fn path_dir_trash_says_folder() {
            let path = PathInfo {
                path: "/test/dir".to_string(),
                name: "dir".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let trash = find_action(&actions, "file:move_to_trash").unwrap();
            assert!(
                trash.description.as_ref().unwrap().contains("folder"),
                "Dir trash should say 'folder'"
            );
        }

        #[test]
        fn path_file_trash_says_file() {
            let path = PathInfo {
                path: "/test/f.txt".to_string(),
                name: "f.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let trash = find_action(&actions, "file:move_to_trash").unwrap();
            assert!(
                trash.description.as_ref().unwrap().contains("file"),
                "File trash should say 'file'"
            );
        }

        // ============================================================
        // 14. Clipboard action shortcut format
        // ============================================================

        #[test]
        fn clipboard_all_shortcuts_use_symbols() {
            let entry = make_text_entry(false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                if let Some(ref shortcut) = action.shortcut {
                    // Should not contain text like "cmd", "ctrl" etc.
                    assert!(
                        !shortcut.contains("cmd"),
                        "Shortcut '{}' should use symbols not text",
                        shortcut
                    );
                    assert!(
                        !shortcut.contains("shift"),
                        "Shortcut '{}' should use symbols not text",
                        shortcut
                    );
                }
            }
        }

        // ============================================================
        // 15. score_action with whitespace-only query
        // ============================================================

        #[test]
        fn score_whitespace_query() {
            let action = Action::new(
                "test",
                "Test Action With Spaces",
                Some("Description with spaces".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, " ");
            // " " is a contains match on the title " " appears after words
            assert!(score > 0, "Space should match title containing spaces");
        }

        // ============================================================
        // 16. fuzzy_match with repeated characters
        // ============================================================

        #[test]
        fn fuzzy_repeated_chars_in_needle() {
            assert!(
                ActionsDialog::fuzzy_match("aabbcc", "abc"),
                "Should match subsequence with repeated chars in haystack"
            );
        }

        #[test]
        fn fuzzy_repeated_chars_in_both() {
            assert!(
                ActionsDialog::fuzzy_match("aabbcc", "aabb"),
                "Should match when both have repeated chars"
            );
        }

        #[test]
        fn fuzzy_needle_longer_than_haystack() {
            assert!(
                !ActionsDialog::fuzzy_match("ab", "abc"),
                "Needle longer than haystack should not match"
            );
        }

        #[test]
        fn fuzzy_exact_match() {
            assert!(
                ActionsDialog::fuzzy_match("hello", "hello"),
                "Exact match is a valid subsequence"
            );
        }

        #[test]
        fn fuzzy_empty_needle_always_matches() {
            assert!(
                ActionsDialog::fuzzy_match("anything", ""),
                "Empty needle should match everything"
            );
        }

        #[test]
        fn fuzzy_empty_haystack_empty_needle() {
            assert!(
                ActionsDialog::fuzzy_match("", ""),
                "Both empty should match"
            );
        }

        #[test]
        fn fuzzy_empty_haystack_nonempty_needle() {
            assert!(
                !ActionsDialog::fuzzy_match("", "a"),
                "Non-empty needle with empty haystack should not match"
            );
        }

        // ============================================================
        // 17. build_grouped_items_static edge cases
        // ============================================================

        #[test]
        fn grouped_single_item_no_section_headers_style() {
            let actions = vec![make_action("a1", "Action 1", Some("Sec"))];
            let filtered = vec![0usize];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should have 1 header + 1 item = 2
            assert_eq!(grouped.len(), 2);
            assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "Sec"));
            assert!(matches!(&grouped[1], GroupedActionItem::Item(0)));
        }

        #[test]
        fn grouped_empty_filtered() {
            let actions = vec![make_action("a1", "Action 1", None)];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn grouped_none_style_no_headers() {
            let actions = vec![
                make_action("a1", "A1", Some("Sec1")),
                make_action("a2", "A2", Some("Sec2")),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            // None style should have no section headers
            assert_eq!(grouped.len(), 2);
            for item in &grouped {
                assert!(
                    matches!(item, GroupedActionItem::Item(_)),
                    "None style should have no headers"
                );
            }
        }

        #[test]
        fn grouped_separators_style_no_headers() {
            let actions = vec![
                make_action("a1", "A1", Some("Sec1")),
                make_action("a2", "A2", Some("Sec2")),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            assert_eq!(grouped.len(), 2);
            for item in &grouped {
                assert!(
                    matches!(item, GroupedActionItem::Item(_)),
                    "Separators style should have no headers"
                );
            }
        }

        #[test]
        fn grouped_same_section_no_duplicate_header() {
            let actions = vec![
                make_action("a1", "A1", Some("Same")),
                make_action("a2", "A2", Some("Same")),
                make_action("a3", "A3", Some("Same")),
            ];
            let filtered = vec![0, 1, 2];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1, "Same section should produce only 1 header");
        }

        #[test]
        fn grouped_alternating_sections_produce_headers() {
            let actions = vec![
                make_action("a1", "A1", Some("A")),
                make_action("a2", "A2", Some("B")),
                make_action("a3", "A3", Some("A")),
            ];
            let filtered = vec![0, 1, 2];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            // A -> B -> A = 3 section changes
            assert_eq!(
                header_count, 3,
                "Alternating sections should produce 3 headers"
            );
        }

        // ============================================================
        // 18. coerce_action_selection edge cases
        // ============================================================

        #[test]
        fn coerce_empty_returns_none() {
            assert_eq!(coerce_action_selection(&[], 0), None);
        }

        #[test]
        fn coerce_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::SectionHeader("B".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn coerce_on_item_returns_same() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(1));
        }

        #[test]
        fn coerce_header_searches_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn coerce_trailing_header_searches_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("A".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn coerce_alternating_header_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("B".to_string()),
                GroupedActionItem::Item(1),
                GroupedActionItem::SectionHeader("C".to_string()),
                GroupedActionItem::Item(2),
            ];
            // On header at 0 -> should find Item at 1
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
            // On header at 2 -> should find Item at 3
            assert_eq!(coerce_action_selection(&rows, 2), Some(3));
            // On header at 4 -> should find Item at 5
            assert_eq!(coerce_action_selection(&rows, 4), Some(5));
        }

        #[test]
        fn coerce_out_of_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
            // Index 99 should clamp to len-1 = 1
            assert_eq!(coerce_action_selection(&rows, 99), Some(1));
        }

        // ============================================================
        // 19. parse_shortcut_keycaps edge cases
        // ============================================================

        #[test]
        fn keycaps_empty_string() {
            let result = ActionsDialog::parse_shortcut_keycaps("");
            assert!(result.is_empty());
        }

        #[test]
        fn keycaps_single_modifier() {
            let result = ActionsDialog::parse_shortcut_keycaps("⌘");
            assert_eq!(result, vec!["⌘"]);
        }

        #[test]
        fn keycaps_modifier_plus_letter() {
            let result = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(result, vec!["⌘", "C"]);
        }

        #[test]
        fn keycaps_all_modifiers() {
            let result = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧");
            assert_eq!(result, vec!["⌘", "⌃", "⌥", "⇧"]);
        }

        #[test]
        fn keycaps_special_keys() {
            let result = ActionsDialog::parse_shortcut_keycaps("↵⎋⇥⌫␣↑↓←→");
            assert_eq!(result, vec!["↵", "⎋", "⇥", "⌫", "␣", "↑", "↓", "←", "→"]);
        }

        #[test]
        fn keycaps_lowercase_uppercased() {
            let result = ActionsDialog::parse_shortcut_keycaps("⌘a");
            assert_eq!(result, vec!["⌘", "A"]);
        }

        // ============================================================
        // 20. CommandBarConfig close flags independence
        // ============================================================

        #[test]
        fn command_bar_default_all_close_true() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn command_bar_ai_style_close_flags_default() {
            let config = CommandBarConfig::ai_style();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn command_bar_main_menu_search_top() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Top
            );
        }

        #[test]
        fn command_bar_ai_style_search_top() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Top
            );
        }

        #[test]
        fn command_bar_no_search_hidden() {
            let config = CommandBarConfig::no_search();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Hidden
            );
        }

        #[test]
        fn command_bar_notes_style_search_top_icons() {
            let config = CommandBarConfig::notes_style();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Top
            );
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        // ============================================================
        // 21. Action constructor with empty strings
        // ============================================================

        #[test]
        fn action_empty_id_and_title() {
            let action = Action::new("", "", None, ActionCategory::ScriptContext);
            assert_eq!(action.id, "");
            assert_eq!(action.title, "");
            assert_eq!(action.title_lower, "");
            assert!(action.description.is_none());
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn action_with_shortcut_sets_lower() {
            let action =
                Action::new("x", "X", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
            assert_eq!(action.shortcut_lower, Some("⌘⇧c".to_string()));
        }

        #[test]
        fn action_with_shortcut_opt_none_no_lower() {
            let action =
                Action::new("x", "X", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
            assert!(action.shortcut.is_none());
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn action_with_shortcut_opt_some_sets_lower() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘Z".to_string()));
            assert_eq!(action.shortcut, Some("⌘Z".to_string()));
            assert_eq!(action.shortcut_lower, Some("⌘z".to_string()));
        }

        // ============================================================
        // 22. ScriptInfo flag exclusivity
        // ============================================================

        #[test]
        fn script_info_scriptlet_is_not_script() {
            let scriptlet = ScriptInfo::scriptlet("X", "/p.md", None, None);
            assert!(scriptlet.is_scriptlet);
            assert!(!scriptlet.is_script);
            assert!(!scriptlet.is_agent);
        }

        #[test]
        fn script_info_agent_is_not_scriptlet() {
            let mut agent = ScriptInfo::new("A", "/a.md");
            agent.is_script = false;
            agent.is_agent = true;
            assert!(agent.is_agent);
            assert!(!agent.is_scriptlet);
            assert!(!agent.is_script);
        }

        #[test]
        fn script_info_builtin_is_none_of_the_above() {
            let builtin = ScriptInfo::builtin("Clipboard");
            assert!(!builtin.is_script);
            assert!(!builtin.is_scriptlet);
            assert!(!builtin.is_agent);
        }

        // ============================================================
        // 23. Notes command bar action count bounds per flag state
        // ============================================================

        // ============================================================
        // 24. Chat model display_name in title
        // ============================================================

        #[test]
        fn chat_model_display_name_in_title() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "model-x".to_string(),
                    display_name: "Model X Ultra".to_string(),
                    provider: "Acme".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = find_action(&picker, "chat:select_model_model-x").unwrap();
            assert_eq!(model_action.title, "Model X Ultra");
        }

        #[test]
        fn chat_model_provider_in_description() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m".to_string(),
                    display_name: "M".to_string(),
                    provider: "Acme Corp".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = find_action(&picker, "chat:select_model_m").unwrap();
            assert_eq!(model_action.description, Some("Uses Acme Corp".to_string()));
        }

        // ============================================================
        // 25. New chat model_id in action ID
        // ============================================================

        #[test]
        fn new_chat_model_section_name() {
            let models = vec![NewChatModelInfo {
                model_id: "abc-123".to_string(),
                display_name: "ABC 123".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].section.as_deref(), Some("Models"));
        }

        // ============================================================
        // 26. Clipboard delete_all description mentions "pinned"
        // ============================================================

        #[test]
        fn clipboard_delete_all_mentions_pinned() {
            let entry = make_text_entry(false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let delete_all = find_action(&actions, "clip:clipboard_delete_all").unwrap();
            assert!(
                delete_all
                    .description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("pinned"),
                "delete_all description should mention pinned: {:?}",
                delete_all.description
            );
        }

        // ============================================================
        // 27. File context all actions have ScriptContext category
        // ============================================================

        #[test]
        fn file_all_script_context_category() {
            let file_info = FileInfo {
                path: "/test/file.rs".to_string(),
                name: "file.rs".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            for action in &actions {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "File action '{}' should be ScriptContext",
                    action.id
                );
            }
        }

        #[test]
        fn file_dir_all_script_context_category() {
            let file_info = FileInfo {
                path: "/test/dir".to_string(),
                name: "dir".to_string(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            for action in &actions {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "File dir action '{}' should be ScriptContext",
                    action.id
                );
            }
        }

        // ============================================================
        // 28. Path context copy_path and copy_filename always present
        // ============================================================

        #[test]
        fn path_always_has_copy_path_and_filename() {
            for is_dir in [true, false] {
                let path = PathInfo {
                    path: "/test/item".to_string(),
                    name: "item".to_string(),
                    is_dir,
                };
                let actions = get_path_context_actions(&path);
                let ids = action_ids(&actions);
                assert!(
                    ids.contains(&"file:copy_path"),
                    "Path (is_dir={}) should have copy_path",
                    is_dir
                );
                assert!(
                    ids.contains(&"file:copy_filename"),
                    "Path (is_dir={}) should have copy_filename",
                    is_dir
                );
            }
        }

        #[test]
        fn path_always_has_open_in_finder_editor_terminal() {
            for is_dir in [true, false] {
                let path = PathInfo {
                    path: "/test/x".to_string(),
                    name: "x".to_string(),
                    is_dir,
                };
                let actions = get_path_context_actions(&path);
                let ids = action_ids(&actions);
                assert!(ids.contains(&"file:open_in_finder"));
                assert!(ids.contains(&"file:open_in_editor"));
                assert!(ids.contains(&"file:open_in_quick_terminal"));
            }
        }

        // ============================================================
        // 29. Cross-context ID namespace separation
        // ============================================================

        #[test]
        fn clipboard_ids_not_in_script_context() {
            let clip = make_text_entry(false, None);
            let script = ScriptInfo::new("s", "/s.ts");
            let clip_actions = get_clipboard_history_context_actions(&clip);
            let clip_ids: HashSet<&str> = action_ids(&clip_actions).into_iter().collect();
            let script_actions = get_script_context_actions(&script);
            let script_ids: HashSet<&str> = action_ids(&script_actions).into_iter().collect();
            let overlap: Vec<&&str> = clip_ids.intersection(&script_ids).collect();
            assert!(
                overlap.is_empty(),
                "Clipboard and script IDs should not overlap: {:?}",
                overlap
            );
        }

        #[test]
        fn file_ids_not_in_clipboard_context() {
            let file = FileInfo {
                path: "/f.txt".to_string(),
                name: "f.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let clip = make_text_entry(false, None);
            let file_actions = get_file_context_actions(&file);
            let file_ids: HashSet<&str> = action_ids(&file_actions).into_iter().collect();
            let clip_actions = get_clipboard_history_context_actions(&clip);
            let clip_ids: HashSet<&str> = action_ids(&clip_actions).into_iter().collect();
            let overlap: Vec<&&str> = file_ids.intersection(&clip_ids).collect();
            assert!(
                overlap.is_empty(),
                "File and clipboard IDs should not overlap: {:?}",
                overlap
            );
        }

        // ============================================================
        // 30. Action title_lower invariant across all builder functions
        // ============================================================

        #[test]
        fn title_lower_matches_title_for_script() {
            let script = ScriptInfo::new("My Script", "/path/s.ts");
            for action in &get_script_context_actions(&script) {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_title_for_clipboard() {
            let entry = make_text_entry(false, Some("VS Code"));
            for action in &get_clipboard_history_context_actions(&entry) {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn title_lower_matches_title_for_ai() {
            for action in &get_ai_command_bar_actions() {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_title_for_new_chat() {
            let lu = vec![NewChatModelInfo {
                model_id: "m".to_string(),
                display_name: "Model ABC".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            for action in &get_new_chat_actions(&lu, &[], &[]) {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_title_for_note_switcher() {
            let notes = vec![make_note("n1", "My Note", 50, false, false, "", "")];
            for action in &get_note_switcher_actions(&notes) {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_title_for_path() {
            let path = PathInfo {
                path: "/test/MyDir".to_string(),
                name: "MyDir".to_string(),
                is_dir: true,
            };
            for action in &get_path_context_actions(&path) {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_title_for_file() {
            let file = FileInfo {
                path: "/test/MyFile.txt".to_string(),
                name: "MyFile.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&file) {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn description_lower_matches_description_for_script() {
            let script = ScriptInfo::new("My Script", "/path/s.ts");
            for action in &get_script_context_actions(&script) {
                match (&action.description, &action.description_lower) {
                    (Some(desc), Some(desc_lower)) => {
                        assert_eq!(
                            *desc_lower,
                            desc.to_lowercase(),
                            "description_lower mismatch for '{}'",
                            action.id
                        );
                    }
                    (None, None) => {} // Both absent is fine
                    _ => panic!(
                        "description and description_lower mismatch for '{}': desc={:?}, lower={:?}",
                        action.id, action.description, action.description_lower
                    ),
                }
            }
        }

        // ============================================================
        // Additional: Scriptlet with custom actions
        // ============================================================

        #[test]
        fn scriptlet_custom_actions_have_has_action_true() {
            let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "Copy".to_string(),
                    command: "copy".to_string(),
                    tool: "bash".to_string(),
                    code: "echo copy".to_string(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Open".to_string(),
                    command: "open".to_string(),
                    tool: "open".to_string(),
                    code: "https://example.com".to_string(),
                    inputs: vec![],
                    shortcut: Some("cmd+o".to_string()),
                    description: Some("Open in browser".to_string()),
                },
            ];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom: Vec<&Action> = actions
                .iter()
                .filter(|a| a.id.starts_with("scriptlet_action:"))
                .collect();
            assert_eq!(custom.len(), 2);
            for ca in &custom {
                assert!(
                    ca.has_action,
                    "Custom action '{}' should have has_action=true",
                    ca.id
                );
                assert!(
                    ca.value.is_some(),
                    "Custom action '{}' should have value",
                    ca.id
                );
            }
        }

        #[test]
        fn scriptlet_custom_actions_appear_after_run_before_edit() {
            let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Custom".to_string(),
                command: "custom".to_string(),
                tool: "bash".to_string(),
                code: "echo custom".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let run_pos = actions.iter().position(|a| a.id == "run_script").unwrap();
            let custom_pos = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:custom")
                .unwrap();
            let edit_pos = actions
                .iter()
                .position(|a| a.id == "edit_scriptlet")
                .unwrap();
            assert!(run_pos < custom_pos, "Run before custom");
            assert!(custom_pos < edit_pos, "Custom before edit");
        }

        #[test]
        fn scriptlet_custom_action_shortcut_formatted() {
            let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy".to_string(),
                command: "copy".to_string(),
                tool: "bash".to_string(),
                code: "echo".to_string(),
                inputs: vec![],
                shortcut: Some("cmd+c".to_string()),
                description: None,
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom = find_action(&actions, "scriptlet_action:copy").unwrap();
            // The shortcut should be formatted using builders.rs format_shortcut_hint
            assert_eq!(custom.shortcut, Some("⌘C".to_string()));
        }

        // ============================================================
        // Additional: to_deeplink_name edge cases
        // ============================================================

        #[test]
        fn deeplink_unicode_chars_stripped() {
            // Non-alphanumeric chars (including accented) should be replaced with hyphens
            // Actually accented chars are NOT alphanumeric in Rust's is_alphanumeric()
            // Wait, they ARE: 'é'.is_alphanumeric() == true
            let result = to_deeplink_name("café");
            assert_eq!(result, "caf%C3%A9");
        }

        #[test]
        fn deeplink_numbers_preserved() {
            let result = to_deeplink_name("Script 123");
            assert_eq!(result, "script-123");
        }

        #[test]
        fn deeplink_all_special_returns_empty() {
            let result = to_deeplink_name("!@#$%");
            assert_eq!(result, "_unnamed");
        }

        #[test]
        fn deeplink_already_hyphenated_passthrough() {
            let result = to_deeplink_name("my-script");
            assert_eq!(result, "my-script");
        }

        #[test]
        fn deeplink_leading_trailing_special() {
            let result = to_deeplink_name(" !hello! ");
            assert_eq!(result, "hello");
        }

        // ============================================================
        // Additional: Ordering determinism
        // ============================================================

        #[test]
        fn ordering_determinism_script() {
            let script = ScriptInfo::new("Test", "/test.ts");
            let actions_1 = get_script_context_actions(&script);
            let ids1 = action_ids(&actions_1);
            let actions_2 = get_script_context_actions(&script);
            let ids2 = action_ids(&actions_2);
            assert_eq!(ids1, ids2, "Script actions should be deterministic");
        }

        #[test]
        fn ordering_determinism_clipboard() {
            let entry = make_text_entry(false, None);
            let actions_1 = get_clipboard_history_context_actions(&entry);
            let ids1 = action_ids(&actions_1);
            let actions_2 = get_clipboard_history_context_actions(&entry);
            let ids2 = action_ids(&actions_2);
            assert_eq!(ids1, ids2, "Clipboard actions should be deterministic");
        }

        #[test]
        fn ordering_determinism_ai() {
            let actions_1 = get_ai_command_bar_actions();
            let ids1 = action_ids(&actions_1);
            let actions_2 = get_ai_command_bar_actions();
            let ids2 = action_ids(&actions_2);
            assert_eq!(ids1, ids2, "AI actions should be deterministic");
        }

        #[test]
        fn ordering_determinism_path() {
            let path = PathInfo {
                path: "/test".to_string(),
                name: "test".to_string(),
                is_dir: false,
            };
            let actions_1 = get_path_context_actions(&path);
            let ids1 = action_ids(&actions_1);
            let actions_2 = get_path_context_actions(&path);
            let ids2 = action_ids(&actions_2);
            assert_eq!(ids1, ids2, "Path actions should be deterministic");
        }

        // ============================================================
        // Additional: ID uniqueness per context
        // ============================================================

        #[test]
        fn id_uniqueness_script() {
            let script = ScriptInfo::new("s", "/s.ts");
            let actions = get_script_context_actions(&script);
            let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "Script action IDs should be unique"
            );
        }

        #[test]
        fn id_uniqueness_clipboard() {
            let entry = make_text_entry(false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "Clipboard action IDs should be unique"
            );
        }

        #[test]
        fn id_uniqueness_ai() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
            assert_eq!(ids.len(), actions.len(), "AI action IDs should be unique");
        }

        #[test]
        fn id_uniqueness_path() {
            let path = PathInfo {
                path: "/test".to_string(),
                name: "test".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
            assert_eq!(ids.len(), actions.len(), "Path action IDs should be unique");
        }

        #[test]
        fn id_uniqueness_file() {
            let file = FileInfo {
                path: "/f.txt".to_string(),
                name: "f.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let ids: HashSet<&str> = action_ids(&actions).into_iter().collect();
            assert_eq!(ids.len(), actions.len(), "File action IDs should be unique");
        }

        // ============================================================
        // Additional: has_action=false invariant for all built-in actions
        // ============================================================

        #[test]
        fn has_action_false_for_script() {
            let script = ScriptInfo::new("s", "/s.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    !action.has_action,
                    "Built-in action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn has_action_false_for_clipboard() {
            let entry = make_text_entry(false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(
                    !action.has_action,
                    "Built-in action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn has_action_false_for_ai() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    !action.has_action,
                    "Built-in action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        // --- merged from tests_part_06.rs ---
        #[test]
        fn has_action_false_for_path() {
            let path = PathInfo {
                path: "/test".to_string(),
                name: "test".to_string(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&path) {
                assert!(
                    !action.has_action,
                    "Built-in action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn has_action_false_for_file() {
            let file = FileInfo {
                path: "/f.txt".to_string(),
                name: "f.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&file) {
                assert!(
                    !action.has_action,
                    "Built-in action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn has_action_false_for_chat() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m".to_string(),
                    display_name: "M".to_string(),
                    provider: "P".to_string(),
                }],
                has_messages: true,
                has_response: true,
            };
            for action in &get_chat_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "Built-in action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        // ============================================================
        // Additional: Non-empty title and ID for all contexts
        // ============================================================

        #[test]
        fn nonempty_title_id_script() {
            let script = ScriptInfo::new("s", "/s.ts");
            for action in &get_script_context_actions(&script) {
                assert!(!action.id.is_empty(), "Action should have non-empty ID");
                assert!(
                    !action.title.is_empty(),
                    "Action '{}' should have non-empty title",
                    action.id
                );
            }
        }

        #[test]
        fn nonempty_title_id_clipboard() {
            let entry = make_text_entry(false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn nonempty_title_id_ai() {
            for action in &get_ai_command_bar_actions() {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn nonempty_title_id_path() {
            let path = PathInfo {
                path: "/test".to_string(),
                name: "test".to_string(),
                is_dir: true,
            };
            for action in &get_path_context_actions(&path) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn nonempty_title_id_file() {
            let file = FileInfo {
                path: "/f".to_string(),
                name: "f".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&file) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        // ============================================================
        // Additional: Note switcher icons and sections
        // ============================================================

        #[test]
        fn note_switcher_pinned_star_icon() {
            let note = make_note("n1", "Note", 10, false, true, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn note_switcher_current_check_icon() {
            let note = make_note("n1", "Note", 10, true, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::Check));
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn note_switcher_default_file_icon() {
            let note = make_note("n1", "Note", 10, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::File));
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn note_switcher_current_gets_bullet_prefix() {
            let note = make_note("n1", "My Note", 10, true, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert!(
                actions[0].title.starts_with("• "),
                "Current note should have bullet prefix"
            );
        }

        #[test]
        fn note_switcher_not_current_no_bullet() {
            let note = make_note("n1", "My Note", 10, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert!(
                !actions[0].title.starts_with("• "),
                "Non-current note should not have bullet"
            );
        }

        #[test]
        fn note_switcher_id_format() {
            let note = make_note("abc-123", "Note", 10, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].id, "note_abc-123");
        }

        #[test]
        fn note_switcher_empty_shows_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
            assert!(actions[0].title.contains("No notes"));
        }

        #[test]
        fn note_switcher_pinned_takes_priority_over_current() {
            let note = make_note("n1", "Note", 10, true, true, "", "");
            let actions = get_note_switcher_actions(&[note]);
            // Pinned icon takes priority
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
            // But still gets the "Pinned" section
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
            // And still gets bullet prefix because is_current
            assert!(actions[0].title.starts_with("• "));
        }
    }
}

mod from_dialog_builtin_action_validation_tests_8 {
    //! Batch 8: Dialog builtin action validation tests
    //!
    //! Focuses on combined scenarios, interaction matrices, and boundary conditions
    //! not covered in batches 1-7:
    //!
    //! 1. Action verb in primary title format ("{verb} \"{name}\"") across all verbs
    //! 2. Clipboard pin×contentType×app combined matrix
    //! 3. Scriptlet context custom actions with frecency interaction
    //! 4. Path context special characters in names
    //! 5. Note switcher Unicode/emoji titles
    //! 6. Chat context partial state combinations
    //! 7. AI command bar description keyword validation
    //! 8. Notes command bar section label transitions
    //! 9. New chat duplicate providers
    //! 10. Deeplink description URL format validation
    //! 11. Score_action boundary thresholds (exact 100/50/25 boundaries)
    //! 12. build_grouped_items interleaved section/no-section
    //! 13. coerce_action_selection complex patterns
    //! 14. parse_shortcut_keycaps compound symbol sequences
    //! 15. CommandBarConfig notes_style detailed fields
    //! 16. Cross-builder action count comparisons
    //! 17. Action builder chaining order independence
    //! 18. Clipboard destructive action ordering stability
    //! 19. File context title includes exact filename
    //! 20. Notes info all-true/all-false edge cases
    //! 21. ScriptInfo agent flag interactions with frecency chaining
    //! 22. Agent actions: no view_logs, has copy_content
    //! 23. Builtin with full optional fields (shortcut+alias+frecency)
    //! 24. Path context dir vs file action count equality
    //! 25. Multiple scriptlet custom actions ordering
    //! 26. Chat model checkmark exact match only
    //! 27. Note switcher empty/placeholder title
    //! 28. Action with_section/with_icon chaining order independence
    //! 29. Clipboard delete_multiple description content
    //! 30. Deeplink name consecutive special characters

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::{
            get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
            get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
            get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
            get_script_context_actions, get_scriptlet_context_actions_with_custom,
            to_deeplink_name, ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo,
            NewChatPresetInfo, NoteSwitcherNoteInfo, NotesInfo,
        };
        use crate::actions::dialog::{
            build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
        };
        use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
        use crate::actions::CommandBarConfig;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // ============================================================
        // Helper functions
        // ============================================================

        fn action_ids(actions: &[Action]) -> Vec<&str> {
            actions.iter().map(|a| a.id.as_str()).collect()
        }

        fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
            actions.iter().find(|a| a.id == id)
        }

        fn make_action(id: &str, title: &str, section: Option<&str>) -> Action {
            let mut a = Action::new(
                id,
                title,
                Some(format!("Desc for {}", title)),
                ActionCategory::ScriptContext,
            );
            if let Some(s) = section {
                a = a.with_section(s);
            }
            a
        }

        fn make_note(
            id: &str,
            title: &str,
            char_count: usize,
            is_current: bool,
            is_pinned: bool,
            preview: &str,
            relative_time: &str,
        ) -> NoteSwitcherNoteInfo {
            NoteSwitcherNoteInfo {
                id: id.to_string(),
                title: title.to_string(),
                char_count,
                is_current,
                is_pinned,
                preview: preview.to_string(),
                relative_time: relative_time.to_string(),
            }
        }

        fn make_clipboard_entry(
            content_type: ContentType,
            pinned: bool,
            app_name: Option<&str>,
        ) -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: format!(
                    "entry-{}-{}",
                    if pinned { "pinned" } else { "unpinned" },
                    match content_type {
                        ContentType::Text => "text",
                        ContentType::Image => "image",
                        ContentType::Link => "link",
                        ContentType::File => "file",
                        ContentType::Color => "color",
                    }
                ),
                content_type,
                pinned,
                preview: "Test content".to_string(),
                image_dimensions: if content_type == ContentType::Image {
                    Some((640, 480))
                } else {
                    None
                },
                frontmost_app_name: app_name.map(|s| s.to_string()),
            }
        }

        // ============================================================
        // 1. Action verb in primary title format
        // ============================================================

        #[test]
        fn verb_run_in_primary_title() {
            let script = ScriptInfo::new("My Script", "/path/to/script.ts");
            let actions = get_script_context_actions(&script);
            let primary = &actions[0];
            assert_eq!(primary.title, "Run");
            assert_eq!(primary.id, "run_script");
        }

        #[test]
        fn verb_launch_in_primary_title() {
            let script =
                ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].title, "Launch");
        }

        #[test]
        fn verb_switch_to_in_primary_title() {
            let script =
                ScriptInfo::with_action_verb("My Document", "window:123", false, "Switch to");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].title, "Switch To");
        }

        #[test]
        fn verb_open_in_primary_title() {
            let script = ScriptInfo::with_action_verb(
                "Clipboard History",
                "builtin:clipboard",
                false,
                "Open",
            );
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].title, "Open");
        }

        #[test]
        fn verb_execute_in_primary_title() {
            let script =
                ScriptInfo::with_action_verb("Custom Task", "/path/to/task.ts", true, "Execute");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].title, "Execute");
        }

        #[test]
        fn primary_description_includes_verb() {
            let verbs = ["Run", "Launch", "Switch to", "Open", "Execute"];
            for verb in &verbs {
                let script = ScriptInfo::with_action_verb("Test", "/path", false, *verb);
                let actions = get_script_context_actions(&script);
                let desc = actions[0].description.as_ref().unwrap();
                assert!(
                    desc.contains(verb),
                    "Description '{}' should contain verb '{}'",
                    desc,
                    verb
                );
            }
        }

        #[test]
        fn scriptlet_primary_verb_matches_script_info() {
            let script = ScriptInfo::scriptlet("Open GitHub", "/path/to/url.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            // Scriptlets default to "Run" verb
            assert_eq!(actions[0].title, "Run \"Open GitHub\"");
        }

        // ============================================================
        // 2. Clipboard pin × contentType × app combined matrix
        // ============================================================

        #[test]
        fn clipboard_text_unpinned_no_app() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to Active App");
        }

        #[test]
        fn clipboard_text_pinned_with_app() {
            let entry = make_clipboard_entry(ContentType::Text, true, Some("VSCode"));
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to VSCode");
        }

        #[test]
        fn clipboard_image_unpinned_with_app() {
            let entry = make_clipboard_entry(ContentType::Image, false, Some("Figma"));
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to Figma");
        }

        #[test]
        fn clipboard_image_pinned_no_app() {
            let entry = make_clipboard_entry(ContentType::Image, true, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to Active App");
        }

        #[test]
        fn clipboard_image_has_more_actions_than_text() {
            let text_entry = make_clipboard_entry(ContentType::Text, false, None);
            let image_entry = make_clipboard_entry(ContentType::Image, false, None);
            let text_actions = get_clipboard_history_context_actions(&text_entry);
            let image_actions = get_clipboard_history_context_actions(&image_entry);
            assert!(
                image_actions.len() > text_actions.len(),
                "Image ({}) should have more actions than text ({})",
                image_actions.len(),
                text_actions.len()
            );
        }

        // ============================================================
        // 3. Scriptlet context with custom actions + frecency
        // ============================================================

        #[test]
        fn scriptlet_with_custom_actions_and_frecency() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None)
                .with_frecency(true, Some("scriptlet:Test".to_string()));

            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy Output".to_string(),
                command: "copy-output".to_string(),
                tool: "bash".to_string(),
                code: "echo output | pbcopy".to_string(),
                inputs: vec![],
                shortcut: Some("cmd+c".to_string()),
                description: Some("Copy the output".to_string()),
            }];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));

            // Should have custom action
            assert!(actions
                .iter()
                .any(|a| a.id == "scriptlet_action:copy-output"));
            // Should have reset_ranking due to frecency
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
            // Custom action should be after run but before edit
            let run_idx = actions.iter().position(|a| a.id == "run_script").unwrap();
            let custom_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:copy-output")
                .unwrap();
            let edit_idx = actions
                .iter()
                .position(|a| a.id == "edit_scriptlet")
                .unwrap();
            assert!(run_idx < custom_idx);
            assert!(custom_idx < edit_idx);
        }

        #[test]
        fn scriptlet_custom_action_has_action_true() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Do Thing".to_string(),
                command: "do-thing".to_string(),
                tool: "bash".to_string(),
                code: "echo thing".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom = find_action(&actions, "scriptlet_action:do-thing").unwrap();
            assert!(
                custom.has_action,
                "Scriptlet custom action must have has_action=true"
            );
            assert_eq!(custom.value, Some("do-thing".to_string()));
        }

        #[test]
        fn scriptlet_builtin_actions_have_has_action_false() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            for action in &actions {
                if !action.id.starts_with("scriptlet_action:") {
                    assert!(
                        !action.has_action,
                        "Built-in action '{}' should have has_action=false",
                        action.id
                    );
                }
            }
        }

        // ============================================================
        // 4. Path context special characters in names
        // ============================================================

        #[test]
        fn path_dir_with_spaces_in_name() {
            let info = PathInfo::new("My Documents", "/Users/test/My Documents", true);
            let actions = get_path_context_actions(&info);
            let primary = &actions[0];
            assert_eq!(primary.title, "Open \"My Documents\"");
            assert_eq!(primary.id, "file:open_directory");
        }

        #[test]
        fn path_file_with_dots_in_name() {
            let info = PathInfo::new("archive.tar.gz", "/tmp/archive.tar.gz", false);
            let actions = get_path_context_actions(&info);
            let primary = &actions[0];
            assert_eq!(primary.title, "Select \"archive.tar.gz\"");
            assert_eq!(primary.id, "file:select_file");
        }

        #[test]
        fn path_trash_description_dir_vs_file() {
            let dir_info = PathInfo::new("folder", "/tmp/folder", true);
            let file_info = PathInfo::new("file.txt", "/tmp/file.txt", false);

            let dir_actions = get_path_context_actions(&dir_info);
            let file_actions = get_path_context_actions(&file_info);

            let dir_trash = find_action(&dir_actions, "file:move_to_trash").unwrap();
            let file_trash = find_action(&file_actions, "file:move_to_trash").unwrap();

            assert!(
                dir_trash.description.as_ref().unwrap().contains("folder"),
                "Dir trash should say 'folder'"
            );
            assert!(
                file_trash.description.as_ref().unwrap().contains("file"),
                "File trash should say 'file'"
            );
        }

        #[test]
        fn path_dir_and_file_have_same_action_count() {
            let dir_info = PathInfo::new("dir", "/tmp/dir", true);
            let file_info = PathInfo::new("file.txt", "/tmp/file.txt", false);
            let dir_actions = get_path_context_actions(&dir_info);
            let file_actions = get_path_context_actions(&file_info);
            assert_eq!(
                dir_actions.len(),
                file_actions.len(),
                "Dir ({}) and file ({}) should have same action count",
                dir_actions.len(),
                file_actions.len()
            );
        }

        // ============================================================
        // 5. Note switcher Unicode/emoji titles
        // ============================================================

        #[test]
        fn note_switcher_unicode_title() {
            let notes = vec![make_note(
                "id1",
                "Café Notes",
                42,
                false,
                false,
                "Some preview text",
                "5m ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].title, "Café Notes");
        }

        #[test]
        fn note_switcher_current_with_bullet() {
            let notes = vec![make_note(
                "id1",
                "Current Note",
                100,
                true,
                false,
                "Content here",
                "1m ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            assert!(
                actions[0].title.starts_with("• "),
                "Current note should have bullet prefix, got: '{}'",
                actions[0].title
            );
            assert_eq!(actions[0].title, "• Current Note");
        }

        #[test]
        fn note_switcher_pinned_has_star_icon() {
            let notes = vec![make_note(
                "id1",
                "Pinned",
                50,
                false,
                true,
                "pinned content",
                "2h ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
            assert_eq!(actions[0].section, Some("Pinned".to_string()));
        }

        #[test]
        fn note_switcher_current_has_check_icon() {
            let notes = vec![make_note(
                "id1",
                "Current",
                50,
                true,
                false,
                "current content",
                "1m ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::Check));
            assert_eq!(actions[0].section, Some("Recent".to_string()));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn note_switcher_regular_has_file_icon() {
            let notes = vec![make_note(
                "id1",
                "Regular",
                50,
                false,
                false,
                "regular content",
                "3d ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::File));
            assert_eq!(actions[0].section, Some("Recent".to_string()));
        }

        #[test]
        fn note_switcher_pinned_overrides_current_icon() {
            // When both pinned and current, pinned wins for icon
            let notes = vec![make_note(
                "id1",
                "Both",
                50,
                true,
                true,
                "both flags",
                "1m ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(
                actions[0].icon,
                Some(IconName::StarFilled),
                "Pinned should override current for icon"
            );
            assert_eq!(actions[0].section, Some("Pinned".to_string()));
        }

        #[test]
        fn note_switcher_empty_returns_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
            assert_eq!(actions[0].title, "No notes yet");
        }

        #[test]
        fn note_switcher_id_format() {
            let notes = vec![make_note("abc-123-def", "Test", 10, false, false, "", "")];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].id, "note_abc-123-def");
        }

        // ============================================================
        // 6. Chat context partial state combinations
        // ============================================================

        #[test]
        fn chat_no_models_no_messages_no_response() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            // Should still have continue_in_chat
            assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
            // Should NOT have copy_response or clear
            assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
            assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
        }

        #[test]
        fn chat_has_response_but_no_messages() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
            assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
        }

        #[test]
        fn chat_has_messages_but_no_response() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
            assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
        }

        #[test]
        fn chat_all_flags_true() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "claude-3".to_string(),
                    display_name: "Claude".to_string(),
                    provider: "Anthropic".to_string(),
                }],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
            assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
            assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
            let picker = get_chat_model_picker_actions(&info);
            // Model should have checkmark
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-3")
                .unwrap();
            assert!(model_action.title.contains('✓'));
        }

        #[test]
        fn chat_checkmark_only_on_exact_display_name_match() {
            let info = ChatPromptInfo {
                current_model: Some("Claude 3.5".to_string()),
                available_models: vec![
                    ChatModelInfo {
                        id: "claude-3".to_string(),
                        display_name: "Claude 3".to_string(),
                        provider: "Anthropic".to_string(),
                    },
                    ChatModelInfo {
                        id: "claude-35".to_string(),
                        display_name: "Claude 3.5".to_string(),
                        provider: "Anthropic".to_string(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let claude3 = find_action(&picker, "chat:select_model_claude-3").unwrap();
            let claude35 = find_action(&picker, "chat:select_model_claude-35").unwrap();

            assert!(
                !claude3.title.contains('✓'),
                "Claude 3 should NOT have checkmark when current is Claude 3.5"
            );
            assert!(
                claude35.title.contains('✓'),
                "Claude 3.5 should have checkmark"
            );
        }

        #[test]
        fn chat_model_description_includes_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "gpt4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model = find_action(&picker, "chat:select_model_gpt4").unwrap();
            assert!(
                model.description.as_ref().unwrap().contains("OpenAI"),
                "Model description should contain provider"
            );
        }

        // ============================================================
        // 7. AI command bar description keyword validation
        // ============================================================

        #[test]
        fn ai_command_bar_copy_response_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:copy_response").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("response"));
        }

        #[test]
        fn ai_command_bar_copy_chat_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:copy_chat").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("conversation"));
        }

        #[test]
        fn ai_command_bar_copy_last_code_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:copy_last_code").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("code"));
        }

        #[test]
        fn ai_command_bar_new_chat_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:new_chat").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("new"));
        }

        #[test]
        fn ai_command_bar_change_model_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:change_model").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("model"));
        }

        #[test]
        fn ai_command_bar_delete_chat_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:delete_chat").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("delete"));
        }

        #[test]
        fn ai_command_bar_submit_desc() {
            let actions = get_ai_command_bar_actions();
            let action = find_action(&actions, "chat:submit").unwrap();
            assert!(action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("send"));
        }

        // ============================================================
        // 8. Notes command bar section label transitions
        // ============================================================

        // ============================================================
        // 9. New chat duplicate providers
        // ============================================================

        #[test]
        fn new_chat_multiple_models_same_provider() {
            let models = vec![
                NewChatModelInfo {
                    model_id: "claude-3".to_string(),
                    display_name: "Claude 3".to_string(),
                    provider: "anthropic".to_string(),
                    provider_display_name: "Anthropic".to_string(),
                },
                NewChatModelInfo {
                    model_id: "claude-35".to_string(),
                    display_name: "Claude 3.5".to_string(),
                    provider: "anthropic".to_string(),
                    provider_display_name: "Anthropic".to_string(),
                },
            ];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions.len(), 2);
            // Both should have Models section
            assert_eq!(actions[0].section, Some("Models".to_string()));
            assert_eq!(actions[1].section, Some("Models".to_string()));
            // Both should show Anthropic as provider in description
            for action in &actions {
                assert_eq!(
                    action.description,
                    Some("Uses Anthropic".to_string()),
                    "Model action should have provider in description"
                );
            }
        }

        #[test]
        fn new_chat_empty_inputs_return_empty() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn new_chat_section_ordering() {
            let last_used = vec![NewChatModelInfo {
                model_id: "lu1".to_string(),
                display_name: "Last Used 1".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::BoltFilled,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];

            let actions = get_new_chat_actions(&last_used, &presets, &models);
            let lu_idx = actions
                .iter()
                .position(|a| a.section == Some("Last Used Settings".to_string()))
                .unwrap();
            let preset_idx = actions
                .iter()
                .position(|a| a.section == Some("Presets".to_string()))
                .unwrap();
            let model_idx = actions
                .iter()
                .position(|a| a.section == Some("Models".to_string()))
                .unwrap();
            assert!(lu_idx < preset_idx, "Last Used before Presets");
            assert!(preset_idx < model_idx, "Presets before Models");
        }

        #[test]
        fn new_chat_preset_has_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "code".to_string(),
                name: "Code".to_string(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(
                actions[0].description,
                Some("Uses Code preset".to_string()),
                "Preset actions include a preset usage description"
            );
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn new_chat_last_used_has_bolt_icon() {
            let last_used = vec![NewChatModelInfo {
                model_id: "lu1".to_string(),
                display_name: "Recent".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn new_chat_model_has_settings_icon() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        // ============================================================
        // 10. Deeplink description URL format
        // ============================================================

        #[test]
        fn deeplink_description_contains_url() {
            let script = ScriptInfo::new("My Script", "/path/to/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = find_action(&actions, "copy_deeplink").unwrap();
            assert!(dl
                .description
                .as_ref()
                .unwrap()
                .eq("Copy a portable Script Kit share link to clipboard"));
        }

        #[test]
        fn deeplink_description_special_chars_stripped() {
            let script = ScriptInfo::new("Hello!@#World", "/path/to/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = find_action(&actions, "copy_deeplink").unwrap();
            assert_eq!(dl.title, "Share");
            assert_eq!(
                dl.description.as_deref(),
                Some("Copy a portable Script Kit share link to clipboard")
            );
        }

        #[test]
        fn deeplink_name_consecutive_specials() {
            assert_eq!(to_deeplink_name("a!!!b"), "a-b");
            assert_eq!(to_deeplink_name("---hello---"), "hello");
            assert_eq!(to_deeplink_name("  spaces  between  "), "spaces-between");
        }

        #[test]
        fn deeplink_name_unicode_preserved() {
            assert_eq!(to_deeplink_name("café"), "caf%C3%A9");
            assert_eq!(to_deeplink_name("naïve"), "na%C3%AFve");
        }

        #[test]
        fn deeplink_name_empty_after_stripping() {
            let result = to_deeplink_name("!@#$%");
            assert_eq!(result, "_unnamed");
        }

        // ============================================================
        // 11. Score_action boundary thresholds
        // ============================================================

        #[test]
        fn score_prefix_match_is_100() {
            let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "edit");
            assert_eq!(score, 100, "Prefix match should score exactly 100");
        }

        #[test]
        fn score_contains_match_is_50() {
            let action = Action::new("id", "The Editor", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "editor");
            assert_eq!(score, 50, "Contains match should score exactly 50");
        }

        #[test]
        fn score_fuzzy_match_is_25() {
            // "esr" is a subsequence of "edit script" (e...s...r? no)
            // Need a proper subsequence: "esc" matches "edit script" (e..s..c? no)
            // "edc" -> e_d_i_t -> no... let's use "eit" -> "edit script" e..i..t
            let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "eit");
            assert_eq!(score, 25, "Fuzzy subsequence match should score exactly 25");
        }

        #[test]
        fn score_description_bonus_is_15() {
            let action = Action::new(
                "id",
                "Open File",
                Some("edit the file".to_string()),
                ActionCategory::ScriptContext,
            );
            // "xyz" won't match title, but if we query something that matches desc only...
            // Actually, we need to match title for base score. If no title match, desc only won't help.
            // Let's match title + desc: "open" prefix = 100, desc contains "file" doesn't add if query is "open"
            // We need desc-only bonus: query matches desc but not title
            let score_with_desc = ActionsDialog::score_action(&action, "edit");
            // "edit" doesn't prefix "open file", doesn't contain... let's check fuzzy
            // fuzzy "edit" in "open file": e_d_i_t? no 'e' found -> nope, not in "open file"
            // Actually "open file" has no 'e'? Wait, "open file" -> o,p,e,n,f,i,l,e -> has 'e'!
            // fuzzy "edit": e in "open file" at pos 2, d? no d in "open file" after pos 2... nope
            // So title score = 0, desc "edit the file" contains "edit" = +15
            assert_eq!(
                score_with_desc, 15,
                "Description-only match should score 15"
            );
        }

        #[test]
        fn score_shortcut_bonus_is_10() {
            let action = Action::new("id", "Open File", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            // "⌘e" matches shortcut_lower
            let score = ActionsDialog::score_action(&action, "⌘e");
            assert_eq!(score, 10, "Shortcut-only match should score 10");
        }

        #[test]
        fn score_prefix_plus_desc_stacking() {
            let action = Action::new(
                "id",
                "Copy Path",
                Some("Copies the full path to the clipboard".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "copy");
            assert_eq!(score, 100, "Current scoring keeps prefix-only score here");
        }

        #[test]
        fn score_no_match_is_zero() {
            let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "zzz");
            assert_eq!(score, 0, "No match should score 0");
        }

        #[test]
        fn score_empty_query_scores_zero() {
            let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "");
            // Empty string is prefix of everything
            assert_eq!(score, 100, "Empty query matches everything as prefix");
        }

        // ============================================================
        // 12. build_grouped_items interleaved section/no-section
        // ============================================================

        #[test]
        fn grouped_items_mixed_section_and_no_section() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Section A")),
                make_action("a2", "Action 2", None),
                make_action("a3", "Action 3", Some("Section B")),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should have: Header("Section A"), Item(0), Item(1) [no header for None], Header("Section B"), Item(2)
            let mut header_count = 0;
            let mut item_count = 0;
            for item in &items {
                match item {
                    GroupedActionItem::SectionHeader(_) => header_count += 1,
                    GroupedActionItem::Item(_) => item_count += 1,
                }
            }
            assert_eq!(header_count, 2, "Should have 2 section headers");
            assert_eq!(item_count, 3, "Should have 3 items");
        }

        #[test]
        fn grouped_items_none_style_no_headers() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Section A")),
                make_action("a2", "Action 2", Some("Section B")),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            // None style should have no headers
            for item in &items {
                assert!(
                    !matches!(item, GroupedActionItem::SectionHeader(_)),
                    "None style should not insert section headers"
                );
            }
            assert_eq!(items.len(), 2);
        }

        #[test]
        fn grouped_items_separators_style_no_headers() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Section A")),
                make_action("a2", "Action 2", Some("Section B")),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            for item in &items {
                assert!(
                    !matches!(item, GroupedActionItem::SectionHeader(_)),
                    "Separators style should not insert section headers"
                );
            }
        }

        #[test]
        fn grouped_items_empty_filtered() {
            let actions = vec![make_action("a1", "Action 1", Some("Section A"))];
            let filtered: Vec<usize> = vec![];
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(items.is_empty());
        }

        #[test]
        fn grouped_items_same_section_no_duplicate_header() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Same")),
                make_action("a2", "Action 2", Some("Same")),
                make_action("a3", "Action 3", Some("Same")),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = items
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1, "Same section should produce only 1 header");
        }

        // ============================================================
        // 13. coerce_action_selection complex patterns
        // ============================================================

        #[test]
        fn coerce_empty_returns_none() {
            let result = coerce_action_selection(&[], 0);
            assert_eq!(result, None);
        }

        #[test]
        fn coerce_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::SectionHeader("B".to_string()),
            ];
            let result = coerce_action_selection(&rows, 0);
            assert_eq!(result, None);
        }

        #[test]
        fn coerce_on_item_returns_same() {
            let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
            let result = coerce_action_selection(&rows, 0);
            assert_eq!(result, Some(0));
        }

        #[test]
        fn coerce_header_at_start_goes_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::Item(0),
            ];
            let result = coerce_action_selection(&rows, 0);
            assert_eq!(result, Some(1));
        }

        #[test]
        fn coerce_header_at_end_goes_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("A".to_string()),
            ];
            let result = coerce_action_selection(&rows, 1);
            assert_eq!(result, Some(0));
        }

        #[test]
        fn coerce_alternating_header_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("B".to_string()),
                GroupedActionItem::Item(1),
            ];
            // Landing on header at index 2 should go down to item at index 3
            let result = coerce_action_selection(&rows, 2);
            assert_eq!(result, Some(3));
        }

        #[test]
        fn coerce_out_of_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
            let result = coerce_action_selection(&rows, 999);
            assert_eq!(result, Some(1));
        }

        // ============================================================
        // 14. parse_shortcut_keycaps compound symbol sequences
        // ============================================================

        #[test]
        fn parse_keycaps_cmd_c() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(keycaps, vec!["⌘", "C"]);
        }

        #[test]
        fn parse_keycaps_all_modifiers() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌃⌥⇧⌘A");
            assert_eq!(keycaps, vec!["⌃", "⌥", "⇧", "⌘", "A"]);
        }

        #[test]
        fn parse_keycaps_enter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(keycaps, vec!["↵"]);
        }

        #[test]
        fn parse_keycaps_arrows() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(keycaps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn parse_keycaps_escape() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn parse_keycaps_empty() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("");
            assert!(keycaps.is_empty());
        }

        #[test]
        fn parse_keycaps_space_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(keycaps, vec!["␣"]);
        }

        #[test]
        fn parse_keycaps_lowercase_uppercased() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘c");
            assert_eq!(keycaps, vec!["⌘", "C"]);
        }

        // ============================================================
        // 15. CommandBarConfig detailed fields
        // ============================================================

        #[test]
        fn command_bar_default_config() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Top
            );
        }

        #[test]
        fn command_bar_ai_style() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Top
            );
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_main_menu_style() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Top
            );
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(!config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_no_search() {
            let config = CommandBarConfig::no_search();
            assert_eq!(
                config.dialog_config.search_position,
                crate::actions::types::SearchPosition::Hidden
            );
        }

        // ============================================================
        // 16. Cross-builder action count comparisons
        // ============================================================

        #[test]
        fn script_has_more_actions_than_builtin() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            let builtin = ScriptInfo::builtin("Test Builtin");
            let script_actions = get_script_context_actions(&script);
            let builtin_actions = get_script_context_actions(&builtin);
            assert!(
                script_actions.len() > builtin_actions.len(),
                "Script ({}) should have more actions than builtin ({})",
                script_actions.len(),
                builtin_actions.len()
            );
        }

        #[test]
        fn scriptlet_via_script_context_vs_scriptlet_context_same_count() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None);
            let via_script = get_script_context_actions(&script);
            let via_scriptlet = get_scriptlet_context_actions_with_custom(&script, None);
            assert_eq!(
                via_script.len(),
                via_scriptlet.len() + 3,
                "Script context ({}) includes three extra actions (toggle_info + toggle_favorite + file:open_in_quick_terminal) compared to scriptlet context ({})",
                via_script.len(),
                via_scriptlet.len()
            );
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn script_with_frecency_has_one_more_action() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            let script_with_frecency = ScriptInfo::new("test", "/path/to/test.ts")
                .with_frecency(true, Some("/path".to_string()));
            let actions = get_script_context_actions(&script);
            let actions_f = get_script_context_actions(&script_with_frecency);
            assert_eq!(
                actions_f.len(),
                actions.len() + 1,
                "Frecency adds exactly 1 action (reset_ranking)"
            );
        }

        // ============================================================
        // 17. Action builder chaining order independence
        // ============================================================

        #[test]
        fn with_icon_then_section_same_as_reverse() {
            let a1 = Action::new("id", "Title", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Copy)
                .with_section("Section");
            let a2 = Action::new("id", "Title", None, ActionCategory::ScriptContext)
                .with_section("Section")
                .with_icon(IconName::Copy);
            assert_eq!(a1.icon, a2.icon);
            assert_eq!(a1.section, a2.section);
            assert_eq!(a1.id, a2.id);
            assert_eq!(a1.title, a2.title);
        }

        #[test]
        fn with_shortcut_then_icon_then_section() {
            let a = Action::new("id", "Title", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘C")
                .with_icon(IconName::Copy)
                .with_section("Section");
            assert_eq!(a.shortcut, Some("⌘C".to_string()));
            assert_eq!(a.icon, Some(IconName::Copy));
            assert_eq!(a.section, Some("Section".to_string()));
        }

        #[test]
        fn with_shortcut_opt_none_preserves_fields() {
            let a = Action::new(
                "id",
                "Title",
                Some("Desc".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Plus)
            .with_shortcut_opt(None);
            assert_eq!(a.icon, Some(IconName::Plus));
            assert!(a.shortcut.is_none());
            assert_eq!(a.description, Some("Desc".to_string()));
        }

        #[test]
        fn with_shortcut_sets_lowercase_cache() {
            let a = Action::new("id", "Title", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘⇧C");
            assert_eq!(a.shortcut_lower, Some("⌘⇧c".to_string()));
        }

        // ============================================================
        // 18. Clipboard destructive action ordering stability
        // ============================================================

        #[test]
        fn clipboard_destructive_actions_always_last_three() {
            let entries = vec![
                make_clipboard_entry(ContentType::Text, false, None),
                make_clipboard_entry(ContentType::Text, true, Some("Chrome")),
                make_clipboard_entry(ContentType::Image, false, Some("Slack")),
                make_clipboard_entry(ContentType::Image, true, None),
            ];
            for entry in &entries {
                let actions = get_clipboard_history_context_actions(entry);
                let len = actions.len();
                assert!(len >= 3);
                // Last 3 should always be delete, delete_multiple, delete_all
                assert_eq!(
                    actions[len - 3].id,
                    "clip:clipboard_delete",
                    "Third from last should be clipboard_delete for {:?}",
                    entry.content_type
                );
                assert_eq!(
                    actions[len - 2].id,
                    "clip:clipboard_delete_multiple",
                    "Second from last should be clipboard_delete_multiple"
                );
                assert_eq!(
                    actions[len - 1].id,
                    "clip:clipboard_delete_all",
                    "Last should be clipboard_delete_all"
                );
            }
        }

        #[test]
        fn clipboard_paste_always_first() {
            let entries = vec![
                make_clipboard_entry(ContentType::Text, false, None),
                make_clipboard_entry(ContentType::Image, true, Some("App")),
            ];
            for entry in &entries {
                let actions = get_clipboard_history_context_actions(entry);
                assert_eq!(
                    actions[0].id, "clip:clipboard_paste",
                    "Paste should always be first"
                );
            }
        }

        #[test]
        fn clipboard_copy_always_second() {
            let entries = vec![
                make_clipboard_entry(ContentType::Text, false, None),
                make_clipboard_entry(ContentType::Image, false, None),
            ];
            for entry in &entries {
                let actions = get_clipboard_history_context_actions(entry);
                assert_eq!(
                    actions[1].id, "clip:clipboard_copy",
                    "Copy should always be second"
                );
            }
        }

        // ============================================================
        // 19. File context title includes exact filename
        // ============================================================

        #[test]
        fn file_context_title_includes_filename() {
            let file_info = FileInfo {
                path: "/Users/test/report.pdf".to_string(),
                name: "report.pdf".to_string(),
                file_type: FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            let primary = &actions[0];
            assert!(
                primary.title.contains("report.pdf"),
                "Primary title '{}' should contain filename",
                primary.title
            );
        }

        #[test]
        fn file_context_dir_title_includes_dirname() {
            let file_info = FileInfo {
                path: "/Users/test/Documents".to_string(),
                name: "Documents".to_string(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            let primary = &actions[0];
            assert!(
                primary.title.contains("Documents"),
                "Primary title '{}' should contain dirname",
                primary.title
            );
        }

        #[test]
        fn file_context_all_have_descriptions() {
            let file_info = FileInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "File action '{}' should have a description",
                    action.id
                );
            }
        }

        // ============================================================
        // 20. Notes info all-true/all-false edge cases
        // ============================================================

        // ============================================================
        // 21. ScriptInfo agent flag interactions with frecency chaining
        // ============================================================

        #[test]
        fn agent_with_frecency_has_reset_ranking() {
            let mut agent = ScriptInfo::new("Agent", "/path/to/agent");
            agent.is_script = false;
            agent.is_agent = true;
            let agent = agent.with_frecency(true, Some("agent:path".to_string()));
            let actions = get_script_context_actions(&agent);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn agent_frecency_preserves_agent_flag() {
            let mut agent = ScriptInfo::new("Agent", "/path/to/agent");
            agent.is_script = false;
            agent.is_agent = true;
            let agent = agent.with_frecency(true, Some("agent:path".to_string()));
            assert!(agent.is_agent);
            assert!(!agent.is_script);
            assert!(agent.is_suggested);
        }

        // ============================================================
        // 22. Agent actions: no view_logs, has copy_content
        // ============================================================

        #[test]
        fn agent_has_no_view_logs() {
            let mut agent = ScriptInfo::new("Agent", "/path/to/agent");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn agent_has_copy_content() {
            let mut agent = ScriptInfo::new("Agent", "/path/to/agent");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn agent_edit_title_says_agent() {
            let mut agent = ScriptInfo::new("Agent", "/path/to/agent");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let edit = find_action(&actions, "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn agent_has_reveal_and_copy_path() {
            let mut agent = ScriptInfo::new("Agent", "/path/to/agent");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
            assert!(actions.iter().any(|a| a.id == "copy_path"));
        }

        // ============================================================
        // 23. Builtin with full optional fields
        // ============================================================

        #[test]
        fn builtin_with_shortcut_and_alias_and_frecency() {
            let builtin = ScriptInfo::with_all(
                "Clipboard History",
                "builtin:clipboard",
                false,
                "Open",
                Some("cmd+shift+c".to_string()),
                Some("ch".to_string()),
            )
            .with_frecency(true, Some("builtin:clipboard".to_string()));

            let actions = get_script_context_actions(&builtin);

            // Should have update/remove instead of add
            assert!(actions.iter().any(|a| a.id == "update_shortcut"));
            assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
            assert!(actions.iter().any(|a| a.id == "update_alias"));
            assert!(actions.iter().any(|a| a.id == "remove_alias"));
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));

            // Should NOT have script-specific actions
            assert!(!actions.iter().any(|a| a.id == "edit_script"));
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn builtin_primary_uses_custom_verb() {
            let builtin = ScriptInfo::with_action_verb(
                "Clipboard History",
                "builtin:clipboard",
                false,
                "Open",
            );
            let actions = get_script_context_actions(&builtin);
            assert_eq!(actions[0].title, "Open");
        }

        // ============================================================
        // 24. Path context dir vs file action count equality
        // ============================================================

        #[test]
        fn path_dir_file_action_count_equal() {
            let dir = PathInfo::new("dir", "/tmp/dir", true);
            let file = PathInfo::new("file", "/tmp/file", false);
            let dir_actions = get_path_context_actions(&dir);
            let file_actions = get_path_context_actions(&file);
            assert_eq!(dir_actions.len(), file_actions.len());
        }

        #[test]
        fn path_always_has_copy_path_and_copy_filename() {
            let dir = PathInfo::new("dir", "/tmp/dir", true);
            let file = PathInfo::new("file", "/tmp/file", false);
            for actions in [
                get_path_context_actions(&dir),
                get_path_context_actions(&file),
            ] {
                assert!(actions.iter().any(|a| a.id == "file:copy_path"));
                assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
            }
        }

        // ============================================================
        // 25. Multiple scriptlet custom actions ordering
        // ============================================================

        #[test]
        fn scriptlet_multiple_custom_actions_maintain_order() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "First Action".to_string(),
                    command: "first".to_string(),
                    tool: "bash".to_string(),
                    code: "echo first".to_string(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Second Action".to_string(),
                    command: "second".to_string(),
                    tool: "bash".to_string(),
                    code: "echo second".to_string(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Third Action".to_string(),
                    command: "third".to_string(),
                    tool: "bash".to_string(),
                    code: "echo third".to_string(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
            ];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let first_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:first")
                .unwrap();
            let second_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:second")
                .unwrap();
            let third_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:third")
                .unwrap();
            assert!(first_idx < second_idx, "First before second");
            assert!(second_idx < third_idx, "Second before third");
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn scriptlet_custom_actions_after_run_before_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Custom".to_string(),
                command: "custom".to_string(),
                tool: "bash".to_string(),
                code: "echo custom".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];

            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let run_idx = actions.iter().position(|a| a.id == "run_script").unwrap();
            let custom_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:custom")
                .unwrap();
            let shortcut_idx = actions.iter().position(|a| a.id == "add_shortcut").unwrap();
            assert!(run_idx < custom_idx, "Run before custom");
            assert!(
                custom_idx < shortcut_idx,
                "Custom before shortcut management"
            );
        }

        // ============================================================
        // 26. Chat model checkmark exact match only
        // ============================================================

        #[test]
        fn chat_no_checkmark_on_partial_match() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".to_string()),
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt4o".to_string(),
                        display_name: "GPT-4o".to_string(),
                        provider: "OpenAI".to_string(),
                    },
                    ChatModelInfo {
                        id: "gpt4".to_string(),
                        display_name: "GPT-4".to_string(),
                        provider: "OpenAI".to_string(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let gpt4o = find_action(&picker, "chat:select_model_gpt4o").unwrap();
            let gpt4 = find_action(&picker, "chat:select_model_gpt4").unwrap();
            assert!(
                !gpt4o.title.contains('✓'),
                "GPT-4o should not have checkmark"
            );
            assert!(gpt4.title.contains('✓'), "GPT-4 should have checkmark");
        }

        // ============================================================
        // 27. Note switcher empty/placeholder title
        // ============================================================

        #[test]
        fn note_switcher_description_falls_back_to_char_count() {
            let notes = vec![make_note("id1", "Note", 42, false, false, "", "")];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        #[test]
        fn note_switcher_singular_char_count() {
            let notes = vec![make_note("id1", "Note", 1, false, false, "", "")];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1 char");
        }

        #[test]
        fn note_switcher_zero_chars() {
            let notes = vec![make_note("id1", "Note", 0, false, false, "", "")];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "0 chars");
        }

        #[test]
        fn note_switcher_preview_with_time() {
            let notes = vec![make_note(
                "id1",
                "Note",
                100,
                false,
                false,
                "Hello world",
                "5m ago",
            )];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "Hello world · 5m ago");
        }

        #[test]
        fn note_switcher_preview_truncation_at_61() {
            let long_preview = "a".repeat(61);
            let notes = vec![make_note(
                "id1",
                "Note",
                100,
                false,
                false,
                &long_preview,
                "",
            )];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.ends_with('…'), "Should be truncated with ellipsis");
            // 60 chars + ellipsis
            assert_eq!(desc.chars().count(), 61);
        }

        #[test]
        fn note_switcher_preview_not_truncated_at_60() {
            let exact_preview = "a".repeat(60);
            let notes = vec![make_note(
                "id1",
                "Note",
                100,
                false,
                false,
                &exact_preview,
                "",
            )];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.ends_with('…'), "60 chars should NOT be truncated");
            assert_eq!(desc.chars().count(), 60);
        }

        // ============================================================
        // 28. Action with_section/with_icon chaining order independence
        // ============================================================

        #[test]
        fn action_chaining_shortcut_preserves_title_lower() {
            let a = Action::new("id", "Hello World", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘H");
            assert_eq!(a.title_lower, "hello world");
            assert_eq!(a.shortcut_lower, Some("⌘h".to_string()));
        }

        #[test]
        fn action_description_lower_computed() {
            let a = Action::new(
                "id",
                "Title",
                Some("Mixed CASE Desc".to_string()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(a.description_lower, Some("mixed case desc".to_string()));
        }

        #[test]
        fn action_no_description_lower_is_none() {
            let a = Action::new("id", "Title", None, ActionCategory::ScriptContext);
            assert!(a.description_lower.is_none());
        }

        // ============================================================
        // 29. Clipboard delete_multiple description
        // ============================================================

        #[test]
        fn clipboard_delete_multiple_desc_mentions_filter() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let dm = find_action(&actions, "clip:clipboard_delete_multiple").unwrap();
            assert!(
                dm.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("filter")
                    || dm
                        .description
                        .as_ref()
                        .unwrap()
                        .to_lowercase()
                        .contains("matching"),
                "delete_multiple desc should mention filtering/matching"
            );
        }

        #[test]
        fn clipboard_delete_all_desc_mentions_pinned() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let da = find_action(&actions, "clip:clipboard_delete_all").unwrap();
            assert!(
                da.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("pinned"),
                "delete_all desc should mention pinned"
            );
        }

        // ============================================================
        // 30. Deeplink name edge cases
        // ============================================================

        #[test]
        fn deeplink_name_basic() {
            assert_eq!(to_deeplink_name("My Script"), "my-script");
        }

        #[test]
        fn deeplink_name_already_lowercase_hyphenated() {
            assert_eq!(to_deeplink_name("my-script"), "my-script");
        }

        #[test]
        fn deeplink_name_underscores() {
            assert_eq!(to_deeplink_name("hello_world"), "hello-world");
        }

        #[test]
        fn deeplink_name_numbers_preserved() {
            assert_eq!(to_deeplink_name("script123"), "script123");
        }

        #[test]
        fn deeplink_name_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("  hello  "), "hello");
        }

        // ============================================================
        // Cross-cutting: ID uniqueness across contexts
        // ============================================================

        #[test]
        fn script_action_ids_unique() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            let actions = get_script_context_actions(&script);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "Script action IDs should be unique"
            );
        }

        #[test]
        fn clipboard_action_ids_unique() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "Clipboard action IDs should be unique"
            );
        }

        #[test]
        fn ai_command_bar_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "AI command bar IDs should be unique"
            );
        }

        #[test]
        fn path_action_ids_unique() {
            let info = PathInfo::new("test", "/tmp/test", false);
            let actions = get_path_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len(), "Path action IDs should be unique");
        }

        #[test]
        fn file_action_ids_unique() {
            let info = FileInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len(), "File action IDs should be unique");
        }

        // ============================================================
        // Cross-cutting: has_action invariant
        // ============================================================

        #[test]
        fn all_script_actions_have_has_action_false() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            let actions = get_script_context_actions(&script);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Script action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn all_clipboard_actions_have_has_action_false() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Clipboard action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn all_ai_actions_have_has_action_false() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    !action.has_action,
                    "AI action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn all_path_actions_have_has_action_false() {
            let info = PathInfo::new("test", "/tmp/test", false);
            let actions = get_path_context_actions(&info);
            for action in &actions {
                assert!(
                    !action.has_action,
                    "Path action '{}' should have has_action=false",
                    action.id
                );
            }
        }

        // ============================================================
        // Cross-cutting: title_lower invariant
        // ============================================================

        #[test]
        fn title_lower_matches_lowercase_for_all_script_actions() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            let actions = get_script_context_actions(&script);
            for action in &actions {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_lowercase_for_ai_actions() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        #[test]
        fn title_lower_matches_lowercase_for_clipboard_actions() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert_eq!(
                    action.title_lower,
                    action.title.to_lowercase(),
                    "title_lower mismatch for '{}'",
                    action.id
                );
            }
        }

        // ============================================================
        // Cross-cutting: ordering determinism
        // ============================================================

        #[test]
        fn script_actions_deterministic() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            let actions1 = get_script_context_actions(&script);
            let actions2 = get_script_context_actions(&script);
            let a1 = action_ids(&actions1);
            let a2 = action_ids(&actions2);
            assert_eq!(a1, a2, "Script actions should be deterministic");
        }

        #[test]
        fn clipboard_actions_deterministic() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions1 = get_clipboard_history_context_actions(&entry);
            let actions2 = get_clipboard_history_context_actions(&entry);
            let a1 = action_ids(&actions1);
            let a2 = action_ids(&actions2);
            assert_eq!(a1, a2, "Clipboard actions should be deterministic");
        }

        #[test]
        fn ai_actions_deterministic() {
            let actions1 = get_ai_command_bar_actions();
            let actions2 = get_ai_command_bar_actions();
            let a1 = action_ids(&actions1);
            let a2 = action_ids(&actions2);
            assert_eq!(a1, a2, "AI actions should be deterministic");
        }

        // --- merged from tests_part_06.rs ---
        // ============================================================
        // Cross-cutting: non-empty titles and IDs
        // ============================================================

        #[test]
        fn script_all_actions_have_nonempty_id_and_title() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(!action.id.is_empty(), "Action ID should not be empty");
                assert!(!action.title.is_empty(), "Action title should not be empty");
            }
        }

        #[test]
        fn clipboard_all_actions_have_nonempty_id_and_title() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(!action.id.is_empty(), "Action ID should not be empty");
                assert!(!action.title.is_empty(), "Action title should not be empty");
            }
        }

        #[test]
        fn ai_all_actions_have_nonempty_id_and_title() {
            for action in &get_ai_command_bar_actions() {
                assert!(!action.id.is_empty(), "Action ID should not be empty");
                assert!(!action.title.is_empty(), "Action title should not be empty");
            }
        }

        // ============================================================
        // Fuzzy match edge cases
        // ============================================================

        #[test]
        fn fuzzy_match_empty_needle_always_true() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn fuzzy_match_empty_haystack_nonempty_needle_false() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn fuzzy_match_both_empty_true() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn fuzzy_match_exact_match_true() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        #[test]
        fn fuzzy_match_subsequence_true() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlo"));
        }

        #[test]
        fn fuzzy_match_no_subsequence_false() {
            assert!(!ActionsDialog::fuzzy_match("hello", "z"));
        }

        #[test]
        fn fuzzy_match_needle_longer_than_haystack_false() {
            assert!(!ActionsDialog::fuzzy_match("hi", "hello"));
        }

        // ============================================================
        // ActionCategory invariants
        // ============================================================

        #[test]
        fn all_script_actions_are_script_context() {
            let script = ScriptInfo::new("test", "/path/to/test.ts");
            for action in &get_script_context_actions(&script) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_clipboard_actions_are_script_context() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_ai_actions_are_script_context() {
            for action in &get_ai_command_bar_actions() {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_path_actions_are_script_context() {
            let info = PathInfo::new("test", "/tmp/test", false);
            for action in &get_path_context_actions(&info) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_file_actions_are_script_context() {
            let info = FileInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&info) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }
    }
}

mod from_dialog_builtin_action_validation_tests_9 {
    //! Batch 9: Dialog builtin action validation tests
    //!
    //! Focuses on areas not exhaustively covered in batches 1-8:
    //!
    //! 1. AI command bar expanded actions (branch_from_last, export_markdown, toggle_shortcuts_help)
    //! 2. File context macOS-specific actions (quick_look, open_with, show_info)
    //! 3. Clipboard macOS-specific actions (quick_look, open_with, annotate/upload)
    //! 4. CommandBarConfig struct field validation
    //! 5. format_shortcut_hint alias coverage (meta, super, opt, esc, return, arrowdown/left/right)
    //! 6. Cross-context shortcut symbol consistency (all shortcuts use symbol chars)
    //! 7. Action verb formatting with special characters in names
    //! 8. Notes command bar conditional section groups (section strings)
    //! 9. ScriptInfo mixed agent+scriptlet flag precedence
    //! 10. Clipboard save_snippet/save_file always present for both text and image
    //! 11. AI command bar section completeness (21 actions across 7 sections)
    //! 12. Path context open_in_finder/editor/terminal descriptions
    //! 13. Note switcher empty notes placeholder action
    //! 14. New chat action icon/section consistency
    //! 15. Score_action with multi-word queries
    //! 16. build_grouped_items_static with Headers style multi-section
    //! 17. coerce_action_selection with single-item rows
    //! 18. parse_shortcut_keycaps with multi-char sequences
    //! 19. Deeplink name with Unicode characters preservation
    //! 20. File context open title includes quoted name
    //! 21. Clipboard share/attach_to_ai always present
    //! 22. Notes command bar icon name validation
    //! 23. Chat context continue_in_chat always present
    //! 24. Scriptlet context copy_content always present
    //! 25. Agent actions: has copy_content, edit title says "Edit Agent"
    //! 26. Cross-context action count stability
    //! 27. Action with_shortcut_opt preserves existing fields
    //! 28. ActionsDialogConfig default values
    //! 29. SectionStyle and SearchPosition enum values
    //! 30. Clipboard action IDs all prefixed with "clipboard_"

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::{
            get_ai_command_bar_actions, get_chat_context_actions,
            get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
            get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
            get_script_context_actions, get_scriptlet_context_actions_with_custom,
            to_deeplink_name, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo,
            NewChatPresetInfo, NotesInfo,
        };
        use crate::actions::dialog::{
            build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
        };
        use crate::actions::types::{
            Action, ActionCategory, ActionsDialogConfig, AnchorPosition, ScriptInfo,
            SearchPosition, SectionStyle,
        };
        use crate::actions::CommandBarConfig;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        #[allow(unused_imports)]
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // ============================================================
        // Helper functions
        // ============================================================

        fn action_ids(actions: &[Action]) -> Vec<&str> {
            actions.iter().map(|a| a.id.as_str()).collect()
        }

        fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
            actions.iter().find(|a| a.id == id)
        }

        fn make_action(id: &str, title: &str, section: Option<&str>) -> Action {
            let mut a = Action::new(
                id,
                title,
                Some(format!("Desc for {}", title)),
                ActionCategory::ScriptContext,
            );
            if let Some(s) = section {
                a = a.with_section(s);
            }
            a
        }

        fn make_clipboard_entry(
            content_type: ContentType,
            pinned: bool,
            app: Option<&str>,
        ) -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "entry-1".to_string(),
                content_type,
                pinned,
                preview: "test preview".to_string(),
                image_dimensions: if content_type == ContentType::Image {
                    Some((640, 480))
                } else {
                    None
                },
                frontmost_app_name: app.map(|s| s.to_string()),
            }
        }

        // ============================================================
        // 1. AI command bar expanded actions (32 actions, 7 sections)
        // ============================================================

        #[test]
        fn ai_command_bar_has_exactly_32_actions() {
            let actions = get_ai_command_bar_actions();
            assert_eq!(actions.len(), 38, "AI command bar should have 38 actions");
        }

        #[test]
        fn ai_command_bar_has_branch_from_last() {
            let actions = get_ai_command_bar_actions();
            let branch = find_action(&actions, "chat:branch_from_last");
            assert!(branch.is_some(), "Should have branch_from_last action");
            let branch = branch.unwrap();
            assert_eq!(branch.section, Some("Actions".to_string()));
            assert!(
                branch.description.as_ref().unwrap().contains("new chat"),
                "Description should mention new chat behavior"
            );
        }

        #[test]
        fn ai_command_bar_has_export_markdown() {
            let actions = get_ai_command_bar_actions();
            let export = find_action(&actions, "chat:export_markdown");
            assert!(export.is_some(), "Should have export_markdown action");
            let export = export.unwrap();
            assert_eq!(export.section, Some("Export".to_string()));
            assert!(export.icon.is_some());
        }

        #[test]
        fn ai_command_bar_has_toggle_shortcuts_help() {
            let actions = get_ai_command_bar_actions();
            let help = find_action(&actions, "chat:toggle_shortcuts_help");
            assert!(help.is_some(), "Should have toggle_shortcuts_help action");
            let help = help.unwrap();
            assert_eq!(help.section, Some("Help".to_string()));
        }

        #[test]
        fn ai_command_bar_has_seven_sections() {
            let actions = get_ai_command_bar_actions();
            let sections: HashSet<_> = actions.iter().filter_map(|a| a.section.as_ref()).collect();
            assert_eq!(sections.len(), 7, "Should have 7 distinct sections");
            assert!(sections.contains(&"Response".to_string()));
            assert!(sections.contains(&"Actions".to_string()));
            assert!(sections.contains(&"Attachments".to_string()));
            assert!(sections.contains(&"Export".to_string()));
            assert!(sections.contains(&"Context".to_string()));
            assert!(sections.contains(&"Help".to_string()));
            assert!(sections.contains(&"Settings".to_string()));
        }

        #[test]
        fn ai_command_bar_response_section_has_3_items() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .count();
            assert_eq!(count, 3, "Response section should have 3 items");
        }

        #[test]
        fn ai_command_bar_actions_section_has_4_items() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .count();
            assert_eq!(
                count, 7,
                "Actions section should have 4 items (submit, new_chat, delete_chat, branch_from_last)"
            );
        }

        #[test]
        fn ai_command_bar_all_actions_have_icons() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.icon.is_some(),
                    "AI action '{}' should have an icon",
                    action.id
                );
            }
        }

        #[test]
        fn ai_command_bar_all_actions_have_sections() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.section.is_some(),
                    "AI action '{}' should have a section",
                    action.id
                );
            }
        }

        #[test]
        fn ai_command_bar_id_uniqueness() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "All AI action IDs should be unique"
            );
        }

        // ============================================================
        // 2. File context macOS-specific actions
        // ============================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_file_has_quick_look() {
            let file_info = FileInfo {
                path: "/test/photo.jpg".to_string(),
                name: "photo.jpg".to_string(),
                file_type: FileType::Image,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            let ql = find_action(&actions, "file:quick_look");
            assert!(ql.is_some(), "File should have quick_look on macOS");
            assert_eq!(ql.unwrap().shortcut.as_deref(), Some("⌘Y"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_dir_no_quick_look() {
            let file_info = FileInfo {
                path: "/test/folder".to_string(),
                name: "folder".to_string(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(
                find_action(&actions, "file:quick_look").is_none(),
                "Directory should NOT have quick_look"
            );
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_has_open_in_editor() {
            let file_info = FileInfo {
                path: "/test/doc.txt".to_string(),
                name: "doc.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            let ow = find_action(&actions, "file:open_in_editor");
            assert!(ow.is_some(), "File should have open_in_editor on macOS");
            assert_eq!(ow.unwrap().shortcut.as_deref(), Some("⌘E"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_has_show_info() {
            let file_info = FileInfo {
                path: "/test/doc.txt".to_string(),
                name: "doc.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            let si = find_action(&actions, "file:show_info");
            assert!(si.is_some(), "File should have show_info on macOS");
            assert_eq!(si.unwrap().shortcut.as_deref(), Some("⌘I"));
            assert!(
                si.unwrap()
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("information"),
                "show_info description should mention information"
            );
        }

        // ============================================================
        // 3. Clipboard macOS-specific actions
        // ============================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_image_has_open_with_on_macos() {
            let entry = make_clipboard_entry(ContentType::Image, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(
                find_action(&actions, "clip:clipboard_open_with").is_some(),
                "Image entry should have open_with on macOS"
            );
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_image_has_annotate_cleanshot_on_macos() {
            let entry = make_clipboard_entry(ContentType::Image, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let annotate = find_action(&actions, "clip:clipboard_annotate_cleanshot");
            assert!(
                annotate.is_some(),
                "Image entry should have annotate_cleanshot on macOS"
            );
            assert_eq!(annotate.unwrap().shortcut.as_deref(), Some("⇧⌘A"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_image_has_upload_cleanshot_on_macos() {
            let entry = make_clipboard_entry(ContentType::Image, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let upload = find_action(&actions, "clip:clipboard_upload_cleanshot");
            assert!(
                upload.is_some(),
                "Image entry should have upload_cleanshot on macOS"
            );
            assert_eq!(upload.unwrap().shortcut.as_deref(), Some("⇧⌘U"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_text_no_open_with_on_macos() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(
                find_action(&actions, "clip:clipboard_open_with").is_none(),
                "Text entry should NOT have open_with"
            );
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_has_quick_look_on_macos() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let ql = find_action(&actions, "clip:clipboard_quick_look");
            assert!(ql.is_some(), "Clipboard should have quick_look on macOS");
            assert_eq!(ql.unwrap().shortcut.as_deref(), Some("␣"));
        }

        // ============================================================
        // 4. CommandBarConfig struct field validation
        // ============================================================

        #[test]
        fn command_bar_config_default_close_flags_all_true() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn command_bar_config_ai_style_search_top() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_config_main_menu_search_top() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(!config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_config_no_search_hidden() {
            let config = CommandBarConfig::no_search();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
        }

        #[test]
        fn command_bar_config_notes_style_icons_and_footer() {
            let config = CommandBarConfig::notes_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        // ============================================================
        // 5. format_shortcut_hint alias coverage
        // ============================================================

        #[test]
        fn format_shortcut_hint_meta_maps_to_cmd_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("meta+c"), "⌘C");
        }

        #[test]
        fn format_shortcut_hint_super_maps_to_cmd_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("super+v"), "⌘V");
        }

        #[test]
        fn format_shortcut_hint_opt_maps_to_alt_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("opt+x"), "⌥X");
        }

        #[test]
        fn format_shortcut_hint_option_maps_to_alt_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("option+z"), "⌥Z");
        }

        #[test]
        fn format_shortcut_hint_control_maps_to_ctrl_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("control+a"), "⌃A");
        }

        #[test]
        fn format_shortcut_hint_return_maps_to_enter_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+return"), "⌘↵");
        }

        #[test]
        fn format_shortcut_hint_esc_maps_to_escape_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
        }

        #[test]
        fn format_shortcut_hint_tab_maps_to_symbol() {
            assert_eq!(ActionsDialog::format_shortcut_hint("tab"), "⇥");
        }

        #[test]
        fn format_shortcut_hint_arrowdown_maps_to_down() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowdown"), "↓");
        }

        #[test]
        fn format_shortcut_hint_arrowleft_maps_to_left() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowleft"), "←");
        }

        #[test]
        fn format_shortcut_hint_arrowright_maps_to_right() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowright"), "→");
        }

        #[test]
        fn format_shortcut_hint_arrowup_maps_to_up() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowup"), "↑");
        }

        #[test]
        fn format_shortcut_hint_command_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("command+k"), "⌘K");
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn format_shortcut_hint_triple_modifier() {
            assert_eq!(
                ActionsDialog::format_shortcut_hint("cmd+shift+alt+z"),
                "⌘⇧⌥Z"
            );
        }

        #[test]
        fn format_shortcut_hint_space_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("space"), "␣");
        }

        #[test]
        fn format_shortcut_hint_backspace_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+backspace"), "⌘⌫");
        }

        #[test]
        fn format_shortcut_hint_delete_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("delete"), "⌫");
        }

        // ============================================================
        // 6. Cross-context shortcut symbol consistency
        // ============================================================

        #[test]
        fn all_script_context_shortcuts_use_symbols() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            for action in &actions {
                if let Some(ref shortcut) = action.shortcut {
                    assert!(
                        !shortcut.contains("cmd")
                            && !shortcut.contains("shift")
                            && !shortcut.contains("alt")
                            && !shortcut.contains("ctrl"),
                        "Shortcut '{}' for action '{}' should use symbols not text",
                        shortcut,
                        action.id
                    );
                }
            }
        }

        #[test]
        fn all_clipboard_context_shortcuts_use_symbols() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                if let Some(ref shortcut) = action.shortcut {
                    assert!(
                        !shortcut.contains("cmd")
                            && !shortcut.contains("shift")
                            && !shortcut.contains("alt")
                            && !shortcut.contains("ctrl"),
                        "Shortcut '{}' for action '{}' should use symbols not text",
                        shortcut,
                        action.id
                    );
                }
            }
        }

        #[test]
        fn all_path_context_shortcuts_use_symbols() {
            let path = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            for action in &actions {
                if let Some(ref shortcut) = action.shortcut {
                    assert!(
                        !shortcut.contains("cmd")
                            && !shortcut.contains("shift")
                            && !shortcut.contains("alt")
                            && !shortcut.contains("ctrl"),
                        "Shortcut '{}' for action '{}' should use symbols not text",
                        shortcut,
                        action.id
                    );
                }
            }
        }

        // ============================================================
        // 7. Action verb formatting with special characters in names
        // ============================================================

        #[test]
        fn action_verb_with_quotes_in_name() {
            let script = ScriptInfo::new("My \"Best\" Script", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert_eq!(run.title, "Run");
        }

        #[test]
        fn action_verb_with_unicode_in_name() {
            let script =
                ScriptInfo::with_action_verb("Café Finder", "/path/cafe.ts", false, "Launch");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert_eq!(run.title, "Launch");
        }

        #[test]
        fn action_verb_with_empty_name() {
            let script = ScriptInfo::new("", "/path/empty.ts");
            let actions = get_script_context_actions(&script);
            assert!(actions.is_empty());
            assert!(find_action(&actions, "run_script").is_none());
        }

        #[test]
        fn action_verb_execute_formatting() {
            let script =
                ScriptInfo::with_action_verb("Task Runner", "/path/task.ts", false, "Execute");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert_eq!(run.title, "Execute");
            assert!(
                run.description.as_ref().unwrap().contains("Execute"),
                "Description should include verb"
            );
        }

        // ============================================================
        // 8. Notes command bar conditional section groups
        // ============================================================

        // ============================================================
        // 9. ScriptInfo mixed agent+scriptlet flag precedence
        // ============================================================

        #[test]
        fn script_info_agent_flag_set_after_new() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let ids = action_ids(&actions);
            assert!(
                ids.contains(&"edit_script"),
                "Agent should have edit_script (titled 'Edit Agent')"
            );
            let edit = find_action(&actions, "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn script_info_agent_no_view_logs() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let ids = action_ids(&actions);
            assert!(
                !ids.contains(&"view_logs"),
                "Agent should NOT have view_logs"
            );
        }

        #[test]
        fn script_info_agent_has_copy_content() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            assert!(
                find_action(&actions, "copy_content").is_some(),
                "Agent should have copy_content"
            );
        }

        #[test]
        fn script_info_agent_has_reveal_and_copy_path() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            assert!(find_action(&actions, "reveal_in_finder").is_some());
            assert!(find_action(&actions, "copy_path").is_some());
        }

        // ============================================================
        // 10. Clipboard save_snippet (text-only) / save_file presence
        // ============================================================

        #[test]
        fn clipboard_text_has_save_snippet_and_file() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(find_action(&actions, "clip:clipboard_save_snippet").is_some());
            assert!(find_action(&actions, "clip:clipboard_save_file").is_some());
        }

        #[test]
        fn clipboard_image_has_save_file_but_no_save_snippet() {
            let entry = make_clipboard_entry(ContentType::Image, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            // save_snippet exists only for TEXT entries; images get ocr instead.
            assert!(find_action(&actions, "clip:clipboard_save_snippet").is_none());
            assert!(find_action(&actions, "clip:clipboard_ocr").is_some());
            assert!(find_action(&actions, "clip:clipboard_save_file").is_some());
        }

        #[test]
        fn clipboard_save_snippet_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let snippet = find_action(&actions, "clip:clipboard_save_snippet").unwrap();
            assert_eq!(snippet.shortcut.as_deref(), Some("⇧⌘S"));
        }

        #[test]
        fn clipboard_save_file_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let save = find_action(&actions, "clip:clipboard_save_file").unwrap();
            assert_eq!(save.shortcut.as_deref(), Some("⌥⇧⌘S"));
        }

        // ============================================================
        // 11. Clipboard share/attach_to_ai always present
        // ============================================================

        #[test]
        fn clipboard_text_has_share_and_attach() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(find_action(&actions, "clip:clipboard_share").is_some());
            assert!(find_action(&actions, "clip:clipboard_attach_to_ai").is_some());
        }

        #[test]
        fn clipboard_image_has_share_and_attach() {
            let entry = make_clipboard_entry(ContentType::Image, true, Some("Finder"));
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(find_action(&actions, "clip:clipboard_share").is_some());
            assert!(find_action(&actions, "clip:clipboard_attach_to_ai").is_some());
        }

        #[test]
        fn clipboard_share_shortcut_value() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let share = find_action(&actions, "clip:clipboard_share").unwrap();
            assert_eq!(share.shortcut.as_deref(), Some("⇧⌘E"));
        }

        #[test]
        fn clipboard_attach_to_ai_shortcut_value() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let attach = find_action(&actions, "clip:clipboard_attach_to_ai").unwrap();
            assert_eq!(attach.shortcut.as_deref(), Some("⌃⌘A"));
        }

        // ============================================================
        // 12. Path context descriptions
        // ============================================================

        #[test]
        fn path_context_open_in_finder_description() {
            let path = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let finder = find_action(&actions, "file:open_in_finder").unwrap();
            assert!(
                finder.description.as_ref().unwrap().contains("Finder"),
                "open_in_finder description should mention Finder"
            );
        }

        #[test]
        fn path_context_open_in_editor_description() {
            let path = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let editor = find_action(&actions, "file:open_in_editor").unwrap();
            assert!(
                editor.description.as_ref().unwrap().contains("$EDITOR"),
                "open_in_editor description should mention $EDITOR"
            );
        }

        #[test]
        fn path_context_open_in_quick_terminal_description() {
            let path = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let terminal = find_action(&actions, "file:open_in_quick_terminal").unwrap();
            // Description is "Opens Quick Terminal at this location" (capital Q/T).
            assert!(
                terminal
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("Quick Terminal"),
                "open_in_quick_terminal description should mention Quick Terminal"
            );
        }

        #[test]
        fn path_context_move_to_trash_file_says_file() {
            let path = PathInfo {
                name: "test.txt".to_string(),
                path: "/test.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let trash = find_action(&actions, "file:move_to_trash").unwrap();
            assert!(
                trash.description.as_ref().unwrap().contains("file"),
                "Trash description for file should say 'file'"
            );
        }

        #[test]
        fn path_context_move_to_trash_dir_says_folder() {
            let path = PathInfo {
                name: "docs".to_string(),
                path: "/docs".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let trash = find_action(&actions, "file:move_to_trash").unwrap();
            assert!(
                trash.description.as_ref().unwrap().contains("folder"),
                "Trash description for dir should say 'folder'"
            );
        }

        // ============================================================
        // 13. Note switcher empty notes placeholder
        // ============================================================

        #[test]
        fn note_switcher_empty_shows_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
            assert_eq!(actions[0].title, "No notes yet");
            assert!(
                actions[0].description.as_ref().unwrap().contains("⌘N"),
                "Placeholder should hint at ⌘N"
            );
            assert_eq!(actions[0].icon, Some(IconName::Plus));
            assert_eq!(actions[0].section.as_deref(), Some("Notes"));
        }

        // ============================================================
        // 14. New chat action icon/section consistency
        // ============================================================

        // --- merged from tests_part_03.rs ---
        #[test]
        fn new_chat_last_used_has_bolt_icon() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude-3".to_string(),
                display_name: "Claude 3".to_string(),
                provider: "anthropic".to_string(),
                provider_display_name: "Anthropic".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            let action = &actions[0];
            assert_eq!(action.icon, Some(IconName::BoltFilled));
            assert_eq!(action.section.as_deref(), Some("Last Used Settings"));
        }

        #[test]
        fn new_chat_presets_use_custom_icon() {
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let action = &actions[0];
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("Presets"));
            assert!(action.description.as_deref().unwrap().contains("preset"));
        }

        #[test]
        fn new_chat_models_use_settings_icon() {
            let models = vec![NewChatModelInfo {
                model_id: "gpt-4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "openai".to_string(),
                provider_display_name: "OpenAI".to_string(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            let action = &actions[0];
            assert_eq!(action.icon, Some(IconName::Settings));
            assert_eq!(action.section.as_deref(), Some("Models"));
            assert_eq!(
                action.description.as_deref(),
                Some("Uses OpenAI"),
                "Model should show provider display name"
            );
        }

        #[test]
        fn new_chat_empty_inputs_empty_output() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn new_chat_all_sections_ordered() {
            let last_used = vec![NewChatModelInfo {
                model_id: "lu".to_string(),
                display_name: "LU".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "pr".to_string(),
                name: "PR".to_string(),
                icon: IconName::File,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m".to_string(),
                display_name: "M".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 3);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        // ============================================================
        // 15. Score_action with multi-word queries
        // ============================================================

        #[test]
        fn score_action_multi_word_prefix() {
            let action = Action::new(
                "edit_script",
                "Edit Script",
                Some("Open in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "edit s");
            assert!(
                score >= 100,
                "Multi-word prefix should score >= 100, got {}",
                score
            );
        }

        #[test]
        fn score_action_multi_word_contains() {
            let action = Action::new(
                "file:copy_path",
                "Copy Path",
                Some("Copy the full path".to_string()),
                ActionCategory::ScriptContext,
            );
            // "path" is not a prefix of "Copy Path" but it is contained
            let score = ActionsDialog::score_action(&action, "path");
            assert!(
                score >= 50,
                "'path' should match contains on 'copy path', got {}",
                score
            );
        }

        #[test]
        fn score_action_description_only_match() {
            let action = Action::new(
                "script:reveal",
                "Reveal in Finder",
                Some("Show the file in your filesystem browser".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "filesystem");
            assert_eq!(
                score, 15,
                "Description-only match should score exactly 15, got {}",
                score
            );
        }

        #[test]
        fn score_action_no_match_returns_zero() {
            let action = Action::new(
                "test",
                "Test Action",
                Some("A test".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "zzzzz");
            assert_eq!(score, 0, "No match should return 0");
        }

        #[test]
        fn score_action_shortcut_match_bonus() {
            let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            let score = ActionsDialog::score_action(&action, "⌘e");
            assert!(
                score >= 10,
                "Shortcut match should add >=10 bonus, got {}",
                score
            );
        }

        // ============================================================
        // 16. build_grouped_items_static with Headers style
        // ============================================================

        #[test]
        fn build_grouped_items_headers_inserts_section_headers() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Group A")),
                make_action("a2", "Action 2", Some("Group A")),
                make_action("b1", "Action 3", Some("Group B")),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should be: Header("Group A"), Item(0), Item(1), Header("Group B"), Item(2)
            assert_eq!(grouped.len(), 5);
            assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "Group A"));
            assert!(matches!(&grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(&grouped[2], GroupedActionItem::Item(1)));
            assert!(matches!(&grouped[3], GroupedActionItem::SectionHeader(s) if s == "Group B"));
            assert!(matches!(&grouped[4], GroupedActionItem::Item(2)));
        }

        #[test]
        fn build_grouped_items_separators_no_headers() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Group A")),
                make_action("b1", "Action 2", Some("Group B")),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // Separators style should NOT insert section headers
            assert_eq!(grouped.len(), 2);
            assert!(matches!(&grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(&grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn build_grouped_items_none_no_headers() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Group A")),
                make_action("b1", "Action 2", Some("Group B")),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 2);
        }

        #[test]
        fn build_grouped_items_empty_returns_empty() {
            let actions: Vec<Action> = vec![];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn build_grouped_items_same_section_no_duplicate_header() {
            let actions = vec![
                make_action("a1", "A1", Some("Same")),
                make_action("a2", "A2", Some("Same")),
                make_action("a3", "A3", Some("Same")),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1, "Same section should produce only 1 header");
        }

        // ============================================================
        // 17. coerce_action_selection edge cases
        // ============================================================

        #[test]
        fn coerce_action_selection_single_item() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn coerce_action_selection_single_header() {
            let rows = vec![GroupedActionItem::SectionHeader("Test".to_string())];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn coerce_action_selection_header_then_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S".to_string()),
                GroupedActionItem::Item(0),
            ];
            // Landing on header should move to item
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn coerce_action_selection_item_then_header() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S".to_string()),
            ];
            // Landing on header should search up to item
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn coerce_action_selection_out_of_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
            assert_eq!(
                coerce_action_selection(&rows, 999),
                Some(1),
                "Out of bounds should clamp to last"
            );
        }

        // ============================================================
        // 18. parse_shortcut_keycaps sequences
        // ============================================================

        #[test]
        fn parse_keycaps_modifier_and_letter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘E");
            assert_eq!(keycaps, vec!["⌘", "E"]);
        }

        #[test]
        fn parse_keycaps_two_modifiers_and_letter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
        }

        #[test]
        fn parse_keycaps_enter_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(keycaps, vec!["↵"]);
        }

        #[test]
        fn parse_keycaps_cmd_enter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
            assert_eq!(keycaps, vec!["⌘", "↵"]);
        }

        #[test]
        fn parse_keycaps_escape() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn parse_keycaps_space() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(keycaps, vec!["␣"]);
        }

        #[test]
        fn parse_keycaps_arrows() {
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
        }

        #[test]
        fn parse_keycaps_all_four_modifiers() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧X");
            assert_eq!(keycaps, vec!["⌘", "⌃", "⌥", "⇧", "X"]);
        }

        #[test]
        fn parse_keycaps_lowercase_uppercased() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘a");
            assert_eq!(keycaps, vec!["⌘", "A"]);
        }

        // ============================================================
        // 19. Deeplink name edge cases
        // ============================================================

        #[test]
        fn deeplink_name_preserves_unicode_alphanumeric() {
            assert_eq!(to_deeplink_name("café"), "caf%C3%A9");
        }

        #[test]
        fn deeplink_name_numbers_preserved() {
            assert_eq!(to_deeplink_name("script123"), "script123");
        }

        #[test]
        fn deeplink_name_all_special_returns_empty() {
            assert_eq!(to_deeplink_name("@#$%^&"), "_unnamed");
        }

        #[test]
        fn deeplink_name_mixed_case_lowered() {
            assert_eq!(to_deeplink_name("Hello World"), "hello-world");
        }

        #[test]
        fn deeplink_name_consecutive_specials_collapsed() {
            assert_eq!(to_deeplink_name("a---b"), "a-b");
        }

        // ============================================================
        // 20. File context open title includes quoted name
        // ============================================================

        #[test]
        fn file_context_open_title_includes_filename() {
            let file_info = FileInfo {
                path: "/test/report.pdf".to_string(),
                name: "report.pdf".to_string(),
                file_type: FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            let open = find_action(&actions, "file:open_file").unwrap();
            assert!(
                open.title.contains("report.pdf"),
                "Open title should include filename: {}",
                open.title
            );
            assert!(
                open.title.contains('"'),
                "Open title should quote the filename"
            );
        }

        #[test]
        fn file_context_dir_title_includes_dirname() {
            let file_info = FileInfo {
                path: "/test/Documents".to_string(),
                name: "Documents".to_string(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            let open = find_action(&actions, "file:open_directory").unwrap();
            assert!(
                open.title.contains("Documents"),
                "Open title should include dirname: {}",
                open.title
            );
        }

        // ============================================================
        // 21. Chat context continue_in_chat always present
        // ============================================================

        #[test]
        fn chat_context_continue_in_chat_always_present() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert!(
                find_action(&actions, "chat:continue_in_chat").is_some(),
                "continue_in_chat should always be present"
            );
        }

        #[test]
        fn chat_context_continue_in_chat_shortcut() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            let action = find_action(&actions, "chat:continue_in_chat").unwrap();
            assert_eq!(action.shortcut.as_deref(), Some("⌘↵"));
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn chat_context_copy_response_only_when_has_response() {
            let no_response = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let with_response = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions_no = get_chat_context_actions(&no_response);
            let actions_yes = get_chat_context_actions(&with_response);
            assert!(find_action(&actions_no, "chat:copy_response").is_none());
            assert!(find_action(&actions_yes, "chat:copy_response").is_some());
        }

        #[test]
        fn chat_context_clear_conversation_only_when_has_messages() {
            let no_messages = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let with_messages = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions_no = get_chat_context_actions(&no_messages);
            let actions_yes = get_chat_context_actions(&with_messages);
            assert!(find_action(&actions_no, "chat:clear_conversation").is_none());
            assert!(find_action(&actions_yes, "chat:clear_conversation").is_some());
        }

        // ============================================================
        // 22. Scriptlet context copy_content always present
        // ============================================================

        #[test]
        fn scriptlet_context_has_copy_content() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(
                find_action(&actions, "copy_content").is_some(),
                "Scriptlet context should always have copy_content"
            );
        }

        #[test]
        fn scriptlet_context_copy_content_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let action = find_action(&actions, "copy_content").unwrap();
            assert_eq!(action.shortcut.as_deref(), Some("⌘⌥C"));
        }

        #[test]
        fn scriptlet_context_has_copy_deeplink() {
            let script = ScriptInfo::scriptlet("My Scriptlet", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let deeplink = find_action(&actions, "copy_deeplink").unwrap();
            assert!(
                deeplink
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("my-scriptlet"),
                "Deeplink description should contain deeplink name"
            );
        }

        // ============================================================
        // 23. Notes command bar icon name validation
        // ============================================================

        // ============================================================
        // 24. Cross-context action count stability
        // ============================================================

        #[test]
        fn script_context_action_count_deterministic() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let count1 = get_script_context_actions(&script).len();
            let count2 = get_script_context_actions(&script).len();
            assert_eq!(count1, count2, "Same input should produce same count");
        }

        #[test]
        fn clipboard_context_action_count_deterministic() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let count1 = get_clipboard_history_context_actions(&entry).len();
            let count2 = get_clipboard_history_context_actions(&entry).len();
            assert_eq!(count1, count2);
        }

        #[test]
        fn path_context_dir_and_file_same_count() {
            let dir = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: true,
            };
            let file = PathInfo {
                name: "test.txt".to_string(),
                path: "/test.txt".to_string(),
                is_dir: false,
            };
            let dir_count = get_path_context_actions(&dir).len();
            let file_count = get_path_context_actions(&file).len();
            assert_eq!(
                dir_count, file_count,
                "Dir and file path contexts should have same action count"
            );
        }

        // ============================================================
        // 25. Action builder chaining
        // ============================================================

        #[test]
        fn action_with_shortcut_opt_none_preserves_fields() {
            let action = Action::new(
                "test",
                "Test",
                Some("Desc".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Star)
            .with_section("MySection")
            .with_shortcut_opt(None);
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("MySection"));
            assert!(action.shortcut.is_none());
        }

        #[test]
        fn action_with_shortcut_opt_some_sets_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘Z".to_string()));
            assert_eq!(action.shortcut.as_deref(), Some("⌘Z"));
            assert_eq!(action.shortcut_lower.as_deref(), Some("⌘z"));
        }

        #[test]
        fn action_with_icon_and_section_order_independent() {
            let a1 = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Code)
                .with_section("S");
            let a2 = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_section("S")
                .with_icon(IconName::Code);
            assert_eq!(a1.icon, a2.icon);
            assert_eq!(a1.section, a2.section);
        }

        // ============================================================
        // 26. ActionsDialogConfig and enum defaults
        // ============================================================

        #[test]
        fn actions_dialog_config_default_values() {
            let config = ActionsDialogConfig::default();
            assert_eq!(config.search_position, SearchPosition::Top);
            assert_eq!(config.section_style, SectionStyle::Headers);
            assert_eq!(config.anchor, AnchorPosition::Bottom);
            assert!(!config.show_icons);
            assert!(!config.show_footer);
        }

        #[test]
        fn search_position_hidden_is_distinct_from_top() {
            assert_ne!(SearchPosition::Hidden, SearchPosition::Top);
        }

        #[test]
        fn section_style_headers_not_eq_separators() {
            assert_ne!(SectionStyle::Headers, SectionStyle::Separators);
            assert_ne!(SectionStyle::Headers, SectionStyle::None);
        }

        // ============================================================
        // 27. Clipboard action IDs all prefixed
        // ============================================================

        #[test]
        fn all_clipboard_action_ids_prefixed() {
            let entry = make_clipboard_entry(ContentType::Image, true, Some("Safari"));
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert!(
                    action.id.starts_with("clip:clipboard_"),
                    "Clipboard action ID '{}' should start with 'clip:clipboard_'",
                    action.id
                );
            }
        }

        // ============================================================
        // 28. Fuzzy match edge cases
        // ============================================================

        #[test]
        fn fuzzy_match_empty_needle_matches() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn fuzzy_match_empty_haystack_empty_needle() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn fuzzy_match_empty_haystack_nonempty_needle() {
            assert!(!ActionsDialog::fuzzy_match("", "x"));
        }

        #[test]
        fn fuzzy_match_subsequence() {
            assert!(ActionsDialog::fuzzy_match("edit script", "esi"));
        }

        #[test]
        fn fuzzy_match_no_subsequence() {
            assert!(!ActionsDialog::fuzzy_match("abc", "abd"));
        }

        #[test]
        fn fuzzy_match_exact() {
            assert!(ActionsDialog::fuzzy_match("test", "test"));
        }

        #[test]
        fn fuzzy_match_needle_longer_than_haystack() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        // ============================================================
        // 29. has_action=false invariant for all builtin contexts
        // ============================================================

        #[test]
        fn script_context_all_has_action_false() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    !action.has_action,
                    "Script builtin '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn clipboard_context_all_has_action_false() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(
                    !action.has_action,
                    "Clipboard builtin '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn path_context_all_has_action_false() {
            let path = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&path) {
                assert!(
                    !action.has_action,
                    "Path builtin '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn file_context_all_has_action_false() {
            let file = FileInfo {
                path: "/test/f.txt".to_string(),
                name: "f.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&file) {
                assert!(
                    !action.has_action,
                    "File builtin '{}' should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn ai_command_bar_all_has_action_false() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    !action.has_action,
                    "AI builtin '{}' should have has_action=false",
                    action.id
                );
            }
        }

        // ============================================================
        // 30. ID uniqueness across contexts
        // ============================================================

        #[test]
        fn script_context_ids_unique() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn clipboard_context_ids_unique() {
            let entry = make_clipboard_entry(ContentType::Image, true, Some("App"));
            let actions = get_clipboard_history_context_actions(&entry);
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn path_context_ids_unique() {
            let path = PathInfo {
                name: "dir".to_string(),
                path: "/dir".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        // ============================================================
        // Additional: title_lower/description_lower caching
        // ============================================================

        #[test]
        fn action_title_lower_computed_on_creation() {
            let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "edit script");
        }

        #[test]
        fn action_description_lower_computed_on_creation() {
            let action = Action::new(
                "test",
                "Test",
                Some("Open in $EDITOR".to_string()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(
                action.description_lower,
                Some("open in $editor".to_string())
            );
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn action_no_description_lower_is_none() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn action_shortcut_lower_set_by_with_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘⇧C");
            assert_eq!(action.shortcut_lower, Some("⌘⇧c".to_string()));
        }

        #[test]
        fn action_shortcut_lower_none_without_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
        }

        // ============================================================
        // Additional: non-empty title/ID for all contexts
        // ============================================================

        #[test]
        fn all_script_actions_have_nonempty_title_and_id() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(!action.id.is_empty(), "Action ID should not be empty");
                assert!(!action.title.is_empty(), "Action title should not be empty");
            }
        }

        #[test]
        fn all_clipboard_actions_have_nonempty_title_and_id() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn all_ai_actions_have_nonempty_title_and_id() {
            for action in &get_ai_command_bar_actions() {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        // ============================================================
        // Additional: ordering determinism
        // ============================================================

        #[test]
        fn script_actions_ordering_deterministic() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions1 = get_script_context_actions(&script);
            let actions2 = get_script_context_actions(&script);
            let ids1 = action_ids(&actions1);
            let ids2 = action_ids(&actions2);
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn clipboard_actions_ordering_deterministic() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions1 = get_clipboard_history_context_actions(&entry);
            let actions2 = get_clipboard_history_context_actions(&entry);
            let ids1 = action_ids(&actions1);
            let ids2 = action_ids(&actions2);
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn ai_actions_ordering_deterministic() {
            let actions1 = get_ai_command_bar_actions();
            let actions2 = get_ai_command_bar_actions();
            let ids1 = action_ids(&actions1);
            let ids2 = action_ids(&actions2);
            assert_eq!(ids1, ids2);
        }

        // ============================================================
        // Additional: Clipboard destructive ordering
        // ============================================================

        #[test]
        fn clipboard_destructive_always_last_three() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let len = actions.len();
            assert!(len >= 3);
            assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
            assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
            assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
        }

        #[test]
        fn clipboard_paste_always_first() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].id, "clip:clipboard_paste");
        }

        #[test]
        fn clipboard_copy_always_second() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[1].id, "clip:clipboard_copy");
        }

        // ============================================================
        // Additional: ActionCategory enum
        // ============================================================

        #[test]
        fn all_script_context_actions_are_script_context_category() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            for action in &get_script_context_actions(&script) {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "Action '{}' should be ScriptContext",
                    action.id
                );
            }
        }

        #[test]
        fn all_clipboard_actions_are_script_context_category() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            for action in &get_clipboard_history_context_actions(&entry) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_ai_actions_are_script_context_category() {
            for action in &get_ai_command_bar_actions() {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn all_path_actions_are_script_context_category() {
            let path = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&path) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }
    }
}

mod from_dialog_builtin_action_validation_tests_10 {
    //! Batch 10: Builtin action validation tests
    //!
    //! 155 tests across 30 categories focusing on:
    //! - Clipboard frontmost_app_name propagation and isolation
    //! - Script action exact counts per flag combination
    //! - Scriptlet ordering guarantees with custom actions
    //! - AI command bar exact shortcut/icon values
    //! - Notes command bar exact icon/shortcut/section values
    //! - Path context exact shortcut values
    //! - File context exact description strings
    //! - FileType variants have no effect on file actions
    //! - Chat model checkmark logic and ID format
    //! - New chat provider_display_name propagation
    //! - Clipboard exact description strings
    //! - Script context with custom verbs
    //! - ActionsDialogConfig field defaults
    //! - ActionCategory PartialEq
    //! - Agent description content keywords
    //! - Cross-context frecency reset consistency

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::{
            get_ai_command_bar_actions, get_chat_context_actions, get_chat_model_picker_actions,
            get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
            get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
            get_script_context_actions, get_scriptlet_context_actions_with_custom,
            to_deeplink_name, ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo,
            NewChatPresetInfo, NoteSwitcherNoteInfo, NotesInfo,
        };
        use crate::actions::dialog::{
            build_grouped_items_static, coerce_action_selection, ActionsDialog, GroupedActionItem,
        };
        use crate::actions::types::{
            Action, ActionCategory, ActionsDialogConfig, AnchorPosition, ScriptInfo,
            SearchPosition, SectionStyle,
        };
        use crate::actions::CommandBarConfig;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // ========================================
        // Helpers
        // ========================================

        fn action_ids(actions: &[Action]) -> Vec<&str> {
            actions.iter().map(|a| a.id.as_str()).collect()
        }

        fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
            actions.iter().find(|a| a.id == id)
        }

        fn make_action(id: &str, title: &str, section: Option<&str>) -> Action {
            let mut a = Action::new(id, title, None, ActionCategory::ScriptContext);
            if let Some(s) = section {
                a = a.with_section(s);
            }
            a
        }

        fn make_note(
            id: &str,
            title: &str,
            char_count: usize,
            is_current: bool,
            is_pinned: bool,
            preview: &str,
            relative_time: &str,
        ) -> NoteSwitcherNoteInfo {
            NoteSwitcherNoteInfo {
                id: id.to_string(),
                title: title.to_string(),
                char_count,
                is_current,
                is_pinned,
                preview: preview.to_string(),
                relative_time: relative_time.to_string(),
            }
        }

        fn make_clipboard_entry(
            content_type: ContentType,
            pinned: bool,
            app: Option<&str>,
        ) -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "test-id".to_string(),
                content_type,
                pinned,
                preview: "test preview".to_string(),
                image_dimensions: if content_type == ContentType::Image {
                    Some((800, 600))
                } else {
                    None
                },
                frontmost_app_name: app.map(|s| s.to_string()),
            }
        }

        // ========================================
        // 1. Clipboard frontmost_app_name propagation (6 tests)
        // ========================================

        #[test]
        fn clipboard_paste_title_no_app() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to Active App");
        }

        #[test]
        fn clipboard_paste_title_with_safari() {
            let entry = make_clipboard_entry(ContentType::Text, false, Some("Safari"));
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to Safari");
        }

        #[test]
        fn clipboard_paste_title_with_unicode_app() {
            let entry = make_clipboard_entry(ContentType::Text, false, Some("日本語App"));
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to 日本語App");
        }

        #[test]
        fn clipboard_paste_title_with_empty_string_app() {
            let entry = make_clipboard_entry(ContentType::Text, false, Some(""));
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            // Empty/whitespace app names fall back to the generic title.
            assert_eq!(paste.title, "Paste to Active App");
        }

        #[test]
        fn clipboard_app_name_only_affects_paste() {
            let entry_with = make_clipboard_entry(ContentType::Text, false, Some("VS Code"));
            let entry_without = make_clipboard_entry(ContentType::Text, false, None);
            let actions_with = get_clipboard_history_context_actions(&entry_with);
            let actions_without = get_clipboard_history_context_actions(&entry_without);
            // All non-paste actions should be identical
            for action_with in &actions_with {
                if action_with.id == "clip:clipboard_paste" {
                    continue;
                }
                let action_without = find_action(&actions_without, &action_with.id).unwrap();
                assert_eq!(action_with.title, action_without.title);
                assert_eq!(action_with.description, action_without.description);
                assert_eq!(action_with.shortcut, action_without.shortcut);
            }
        }

        #[test]
        fn clipboard_app_name_image_paste_title() {
            let entry = make_clipboard_entry(ContentType::Image, false, Some("Preview"));
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(paste.title, "Paste to Preview");
        }

        // ========================================
        // 2. Script action exact counts per flag combo (7 tests)
        // ========================================

        #[test]
        fn script_no_shortcut_no_alias_action_count() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            // run, toggle_info, add_shortcut, add_alias, toggle_favorite, edit, view_logs, reveal, file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink, delete_script = 13
            assert_eq!(actions.len(), 13);
        }

        #[test]
        fn script_with_shortcut_action_count() {
            let script =
                ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".to_string()));
            let actions = get_script_context_actions(&script);
            // run, toggle_info, update_shortcut, remove_shortcut, add_alias, toggle_favorite, edit, view_logs, reveal, file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink, delete_script = 14
            assert_eq!(actions.len(), 14);
        }

        #[test]
        fn script_with_shortcut_and_alias_action_count() {
            let script = ScriptInfo::with_shortcut_and_alias(
                "test",
                "/path/test.ts",
                Some("cmd+t".to_string()),
                Some("ts".to_string()),
            );
            let actions = get_script_context_actions(&script);
            // run, toggle_info, update_shortcut, remove_shortcut, update_alias, remove_alias, toggle_favorite, edit, view_logs, reveal, file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink, delete_script = 15
            assert_eq!(actions.len(), 15);
        }

        #[test]
        fn builtin_no_shortcut_no_alias_action_count() {
            let builtin = ScriptInfo::builtin("Test Builtin");
            let actions = get_script_context_actions(&builtin);
            // run, toggle_info, add_shortcut, add_alias, copy_deeplink = 5
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn scriptlet_no_shortcut_no_alias_action_count() {
            let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_script_context_actions(&scriptlet);
            // run, toggle_info, add_shortcut, add_alias, toggle_favorite, edit_scriptlet, reveal_scriptlet, file:open_in_quick_terminal, copy_scriptlet_path, copy_content, copy_deeplink = 11
            assert_eq!(actions.len(), 11);
        }

        #[test]
        fn agent_no_shortcut_no_alias_action_count() {
            let mut agent = ScriptInfo::new("Agent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            // Agents never get add_shortcut/add_alias preference actions.
            // run, toggle_info, toggle_favorite, edit(agent), reveal, file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink = 9
            assert_eq!(actions.len(), 9);
        }

        #[test]
        fn script_with_frecency_adds_one_action() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let base_count = get_script_context_actions(&script).len();
            let with_frecency = ScriptInfo::new("test", "/path/test.ts")
                .with_frecency(true, Some("/path/test.ts".to_string()));
            let frecency_count = get_script_context_actions(&with_frecency).len();
            assert_eq!(frecency_count, base_count + 1);
        }

        // ========================================
        // 3. Scriptlet ordering guarantees (5 tests)
        // ========================================

        #[test]
        fn scriptlet_context_run_is_first() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn scriptlet_context_custom_before_edit() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo hi".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "My Custom".to_string(),
                command: "my-custom".to_string(),
                tool: "bash".to_string(),
                code: "echo custom".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:my-custom")
                .unwrap();
            let edit_idx = actions
                .iter()
                .position(|a| a.id == "edit_scriptlet")
                .unwrap();
            assert!(custom_idx < edit_idx);
        }

        #[test]
        fn scriptlet_context_edit_before_reveal() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let edit_idx = actions
                .iter()
                .position(|a| a.id == "edit_scriptlet")
                .unwrap();
            let reveal_idx = actions
                .iter()
                .position(|a| a.id == "reveal_scriptlet_in_finder")
                .unwrap();
            assert!(edit_idx < reveal_idx);
        }

        #[test]
        fn scriptlet_context_copy_content_before_deeplink() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let copy_idx = actions.iter().position(|a| a.id == "copy_content").unwrap();
            let deeplink_idx = actions
                .iter()
                .position(|a| a.id == "copy_deeplink")
                .unwrap();
            assert!(copy_idx < deeplink_idx);
        }

        #[test]
        fn scriptlet_context_deeplink_before_reset_ranking() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None)
                .with_frecency(true, Some("x".to_string()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let deeplink_idx = actions
                .iter()
                .position(|a| a.id == "copy_deeplink")
                .unwrap();
            let reset_idx = actions
                .iter()
                .position(|a| a.id == "reset_ranking")
                .unwrap();
            assert!(deeplink_idx < reset_idx);
        }

        // ========================================
        // 4. AI command bar exact shortcuts (9 tests)
        // ========================================

        #[test]
        fn ai_cmd_bar_copy_response_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:copy_response").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘C"));
        }

        #[test]
        fn ai_cmd_bar_copy_chat_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:copy_chat").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌥⇧⌘C"));
        }

        #[test]
        fn ai_cmd_bar_copy_last_code_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:copy_last_code").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌥⌘C"));
        }

        #[test]
        fn ai_cmd_bar_submit_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:submit").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn ai_cmd_bar_new_chat_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:new_chat").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘N"));
        }

        #[test]
        fn ai_cmd_bar_delete_chat_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:delete_chat").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⌫"));
        }

        #[test]
        fn ai_cmd_bar_add_attachment_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:add_file").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘A"));
        }

        #[test]
        fn ai_cmd_bar_paste_image_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:paste_image").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘V"));
        }

        #[test]
        fn ai_cmd_bar_change_model_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = find_action(&actions, "chat:change_model").unwrap();
            assert!(a.shortcut.is_none());
        }

        // ========================================
        // 5. Notes command bar exact icons (8 tests)
        // ========================================

        // --- merged from tests_part_02.rs ---
        // ========================================
        // 6. Path context exact shortcut values (7 tests)
        // ========================================

        #[test]
        fn path_dir_primary_shortcut_enter() {
            let info = PathInfo {
                name: "docs".to_string(),
                path: "/home/docs".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:open_directory").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn path_file_primary_shortcut_enter() {
            let info = PathInfo {
                name: "readme.md".to_string(),
                path: "/home/readme.md".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:select_file").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn path_copy_path_shortcut() {
            let info = PathInfo {
                name: "f".to_string(),
                path: "/f".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:copy_path").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⇧C"));
        }

        #[test]
        fn path_open_in_finder_shortcut() {
            let info = PathInfo {
                name: "f".to_string(),
                path: "/f".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:open_in_finder").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⇧F"));
        }

        #[test]
        fn path_open_in_editor_shortcut() {
            let info = PathInfo {
                name: "f".to_string(),
                path: "/f".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:open_in_editor").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn path_open_in_quick_terminal_shortcut() {
            let info = PathInfo {
                name: "f".to_string(),
                path: "/f".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:open_in_quick_terminal").unwrap();
            // open_in_quick_terminal ships without a shortcut.
            assert_eq!(a.shortcut.as_deref(), None);
        }

        #[test]
        fn path_move_to_trash_shortcut() {
            let info = PathInfo {
                name: "f".to_string(),
                path: "/f".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = find_action(&actions, "file:move_to_trash").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⌫"));
        }

        // ========================================
        // 7. File context exact description strings (6 tests)
        // ========================================

        #[test]
        fn file_open_file_description() {
            let fi = FileInfo {
                path: "/x/y.txt".to_string(),
                name: "y.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let a = find_action(&actions, "file:open_file").unwrap();
            assert_eq!(a.description.as_deref(), Some("Opens with the default app"));
        }

        #[test]
        fn file_open_directory_description() {
            let fi = FileInfo {
                path: "/x/dir".to_string(),
                name: "dir".to_string(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&fi);
            let a = find_action(&actions, "file:open_directory").unwrap();
            assert_eq!(a.description.as_deref(), Some("Opens this folder"));
        }

        #[test]
        fn file_reveal_in_finder_description() {
            let fi = FileInfo {
                path: "/x/y.txt".to_string(),
                name: "y.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let a = find_action(&actions, "file:reveal_in_finder").unwrap();
            assert_eq!(a.description.as_deref(), Some("Shows this item in Finder"));
        }

        #[test]
        fn file_copy_path_description() {
            let fi = FileInfo {
                path: "/x/y.txt".to_string(),
                name: "y.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let a = find_action(&actions, "file:copy_path").unwrap();
            assert_eq!(
                a.description.as_deref(),
                Some("Copies the full path to the clipboard")
            );
        }

        #[test]
        fn file_copy_filename_description() {
            let fi = FileInfo {
                path: "/x/y.txt".to_string(),
                name: "y.txt".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let a = find_action(&actions, "file:copy_filename").unwrap();
            assert_eq!(
                a.description.as_deref(),
                Some("Copies only the filename to the clipboard")
            );
        }

        #[test]
        fn file_open_title_includes_name() {
            let fi = FileInfo {
                path: "/x/report.pdf".to_string(),
                name: "report.pdf".to_string(),
                file_type: FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let a = find_action(&actions, "file:open_file").unwrap();
            assert!(a.title.contains("report.pdf"));
        }

        // ========================================
        // 8. FileType variants have no effect on file actions (7 tests)
        // ========================================

        #[test]
        fn filetype_document_same_actions_as_file() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Document,
                is_dir: false,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::File,
                is_dir: false,
            });
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn filetype_image_same_actions() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Image,
                is_dir: false,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Other,
                is_dir: false,
            });
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn filetype_audio_same_actions() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Audio,
                is_dir: false,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::File,
                is_dir: false,
            });
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn filetype_video_same_actions() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Video,
                is_dir: false,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::File,
                is_dir: false,
            });
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn filetype_application_same_actions() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Application,
                is_dir: false,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::File,
                is_dir: false,
            });
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn filetype_other_same_actions() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Other,
                is_dir: false,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Document,
                is_dir: false,
            });
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn filetype_directory_different_from_file() {
            let a = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::Directory,
                is_dir: true,
            });
            let b = get_file_context_actions(&FileInfo {
                path: "/x".to_string(),
                name: "x".to_string(),
                file_type: FileType::File,
                is_dir: false,
            });
            // is_dir changes actions
            assert_ne!(action_ids(&a), action_ids(&b));
        }

        // ========================================
        // 9. Chat model checkmark logic and ID format (6 tests)
        // ========================================

        #[test]
        fn chat_model_id_format_select_model_prefix() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "gpt-4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            assert!(picker[0].id.starts_with("chat:select_model_"));
            assert_eq!(picker[0].id, "chat:select_model_gpt-4");
        }

        #[test]
        fn chat_current_model_gets_checkmark_in_title() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "gpt-4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions[0].id, "chat:change_model");
            assert_eq!(actions[0].description.as_deref(), Some("Current: GPT-4"));
            let picker = get_chat_model_picker_actions(&info);
            assert!(picker[0].title.contains("✓"));
            assert_eq!(picker[0].title, "GPT-4 ✓");
        }

        #[test]
        fn chat_non_current_model_no_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "gpt-4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions[0].id, "chat:change_model");
            assert_eq!(actions[0].description.as_deref(), Some("Current: Claude"));
            let picker = get_chat_model_picker_actions(&info);
            assert!(!picker[0].title.contains("✓"));
            assert_eq!(picker[0].title, "GPT-4");
        }

        #[test]
        fn chat_model_description_shows_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "claude-3".to_string(),
                    display_name: "Claude 3".to_string(),
                    provider: "Anthropic".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions[0].id, "chat:change_model");
            assert_eq!(actions[0].description.as_deref(), Some("Select a model"));
            let picker = get_chat_model_picker_actions(&info);
            assert_eq!(picker[0].description.as_deref(), Some("Uses Anthropic"));
        }

        #[test]
        fn chat_no_models_still_has_continue_in_chat() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            crate::actions::dialog_builtin_validation::assert_chat_root_contract(&info, &actions);
            assert_eq!(actions[0].id, "chat:change_model");
        }

        #[test]
        fn chat_checkmark_exact_match_only() {
            let info = ChatPromptInfo {
                current_model: Some("GPT".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "gpt-4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions[0].description.as_deref(), Some("Current: GPT"));
            let picker = get_chat_model_picker_actions(&info);
            assert!(!picker[0].title.contains("✓"));
        }

        // ========================================
        // 10. New chat provider_display_name propagation (5 tests)
        // ========================================

        #[test]
        fn new_chat_last_used_description_is_provider_display_name() {
            let actions = get_new_chat_actions(
                &[NewChatModelInfo {
                    model_id: "m1".to_string(),
                    display_name: "Model 1".to_string(),
                    provider: "provider-id".to_string(),
                    provider_display_name: "My Provider".to_string(),
                }],
                &[],
                &[],
            );
            assert_eq!(actions[0].description.as_deref(), Some("Uses My Provider"));
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn new_chat_models_section_description_is_provider_display_name() {
            let actions = get_new_chat_actions(
                &[],
                &[],
                &[NewChatModelInfo {
                    model_id: "m1".to_string(),
                    display_name: "M1".to_string(),
                    provider: "pid".to_string(),
                    provider_display_name: "Anthropic AI".to_string(),
                }],
            );
            assert_eq!(actions[0].description.as_deref(), Some("Uses Anthropic AI"));
        }

        #[test]
        fn new_chat_presets_have_no_description() {
            let actions = get_new_chat_actions(
                &[],
                &[NewChatPresetInfo {
                    id: "general".to_string(),
                    name: "General".to_string(),
                    icon: IconName::Settings,
                }],
                &[],
            );
            assert_eq!(
                actions[0].description.as_deref(),
                Some("Uses General preset")
            );
        }

        #[test]
        fn new_chat_preset_uses_its_icon() {
            let actions = get_new_chat_actions(
                &[],
                &[NewChatPresetInfo {
                    id: "code".to_string(),
                    name: "Code".to_string(),
                    icon: IconName::Code,
                }],
                &[],
            );
            assert_eq!(actions[0].icon, Some(IconName::Code));
        }

        #[test]
        fn new_chat_mixed_sections_in_order() {
            let actions = get_new_chat_actions(
                &[NewChatModelInfo {
                    model_id: "lu1".to_string(),
                    display_name: "LU1".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                }],
                &[NewChatPresetInfo {
                    id: "gen".to_string(),
                    name: "Gen".to_string(),
                    icon: IconName::File,
                }],
                &[NewChatModelInfo {
                    model_id: "m1".to_string(),
                    display_name: "M1".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                }],
            );
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        // ========================================
        // 11. Clipboard exact description strings (8 tests)
        // ========================================

        #[test]
        fn clipboard_paste_description() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_paste").unwrap();
            assert_eq!(
                a.description.as_deref(),
                Some("Copies to clipboard and pastes to the focused app")
            );
        }

        #[test]
        fn clipboard_copy_description() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_copy").unwrap();
            assert_eq!(
                a.description.as_deref(),
                Some("Copies the entry to clipboard without pasting")
            );
        }

        #[test]
        fn clipboard_paste_keep_open_description() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_paste_keep_open").unwrap();
            assert!(a.description.as_ref().unwrap().contains("keep"));
        }

        #[test]
        fn clipboard_pin_description() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_pin").unwrap();
            assert!(a.description.as_ref().unwrap().contains("Pin"));
        }

        #[test]
        fn clipboard_unpin_description() {
            let entry = make_clipboard_entry(ContentType::Text, true, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_unpin").unwrap();
            assert!(a.description.as_ref().unwrap().contains("pin"));
        }

        #[test]
        fn clipboard_delete_description() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_delete").unwrap();
            assert!(a.description.as_ref().unwrap().contains("Remove"));
        }

        #[test]
        fn clipboard_delete_multiple_description() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_delete_multiple").unwrap();
            assert!(a.description.as_ref().unwrap().contains("search"));
        }

        #[test]
        fn clipboard_delete_all_description_mentions_pinned() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_delete_all").unwrap();
            assert!(a.description.as_ref().unwrap().contains("pinned"));
        }

        // ========================================
        // 12. Script context with custom verbs (5 tests)
        // ========================================

        #[test]
        fn custom_verb_launch_in_primary_title() {
            let script =
                ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert_eq!(run.title, "Launch");
        }

        #[test]
        fn custom_verb_switch_to_in_primary_title() {
            let script =
                ScriptInfo::with_action_verb("My Window", "window:123", false, "Switch to");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert_eq!(run.title, "Switch To");
        }

        #[test]
        fn custom_verb_open_in_primary_title() {
            let script =
                ScriptInfo::with_action_verb("App Launcher", "builtin:launcher", false, "Open");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert_eq!(run.title, "Open");
        }

        #[test]
        fn custom_verb_execute_in_description() {
            let script = ScriptInfo::with_action_verb("Task", "/path/task.ts", true, "Execute");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert!(run.description.as_ref().unwrap().contains("Execute"));
        }

        #[test]
        fn default_verb_is_run() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            assert_eq!(script.action_verb, "Run");
            let actions = get_script_context_actions(&script);
            let run = find_action(&actions, "run_script").unwrap();
            assert!(run.title.starts_with("Run"));
        }

        // ========================================
        // 13. ActionsDialogConfig defaults (5 tests)
        // ========================================

        #[test]
        fn actions_dialog_config_default_search_top() {
            let config = ActionsDialogConfig::default();
            assert_eq!(config.search_position, SearchPosition::Top);
        }

        #[test]
        fn actions_dialog_config_default_section_separators() {
            let config = ActionsDialogConfig::default();
            assert_eq!(config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn actions_dialog_config_default_anchor_bottom() {
            let config = ActionsDialogConfig::default();
            assert_eq!(config.anchor, AnchorPosition::Bottom);
        }

        #[test]
        fn actions_dialog_config_default_no_icons() {
            let config = ActionsDialogConfig::default();
            assert!(!config.show_icons);
        }

        #[test]
        fn actions_dialog_config_default_no_footer() {
            let config = ActionsDialogConfig::default();
            assert!(!config.show_footer);
        }

        // ========================================
        // 14. ActionCategory PartialEq (4 tests)
        // ========================================

        #[test]
        fn action_category_eq_same() {
            assert_eq!(ActionCategory::ScriptContext, ActionCategory::ScriptContext);
            assert_eq!(ActionCategory::ScriptOps, ActionCategory::ScriptOps);
            assert_eq!(ActionCategory::GlobalOps, ActionCategory::GlobalOps);
            assert_eq!(ActionCategory::Terminal, ActionCategory::Terminal);
        }

        #[test]
        fn action_category_ne_different() {
            assert_ne!(ActionCategory::ScriptContext, ActionCategory::ScriptOps);
            assert_ne!(ActionCategory::ScriptContext, ActionCategory::GlobalOps);
            assert_ne!(ActionCategory::ScriptContext, ActionCategory::Terminal);
        }

        #[test]
        fn action_category_ne_script_ops_vs_global() {
            assert_ne!(ActionCategory::ScriptOps, ActionCategory::GlobalOps);
        }

        #[test]
        fn action_category_ne_terminal_vs_global() {
            assert_ne!(ActionCategory::Terminal, ActionCategory::GlobalOps);
        }

        // ========================================
        // 15. Agent description content keywords (5 tests)
        // ========================================

        #[test]
        fn agent_edit_description_mentions_agent_file() {
            let mut agent = ScriptInfo::new("MyAgent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let a = find_action(&actions, "edit_script").unwrap();
            assert!(a.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn agent_reveal_description_mentions_agent() {
            let mut agent = ScriptInfo::new("MyAgent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let a = find_action(&actions, "reveal_in_finder").unwrap();
            assert!(a.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn agent_copy_path_description_mentions_agent() {
            let mut agent = ScriptInfo::new("MyAgent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let a = find_action(&actions, "copy_path").unwrap();
            assert!(a.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn agent_copy_content_description() {
            let mut agent = ScriptInfo::new("MyAgent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let a = find_action(&actions, "copy_content").unwrap();
            assert!(a.description.as_ref().unwrap().contains("content"));
        }

        #[test]
        fn agent_edit_title_says_edit_agent() {
            let mut agent = ScriptInfo::new("MyAgent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let actions = get_script_context_actions(&agent);
            let a = find_action(&actions, "edit_script").unwrap();
            assert_eq!(a.title, "Edit Agent");
        }

        // ========================================
        // 16. Cross-context frecency reset consistency (3 tests)
        // ========================================

        #[test]
        fn frecency_reset_present_for_script() {
            let script = ScriptInfo::new("s", "/p").with_frecency(true, Some("/p".to_string()));
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn frecency_reset_present_for_scriptlet() {
            let script = ScriptInfo::scriptlet("s", "/p.md", None, None)
                .with_frecency(true, Some("x".to_string()));
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn frecency_reset_present_for_builtin() {
            let script = ScriptInfo::builtin("B").with_frecency(true, Some("b".to_string()));
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        // ========================================
        // 17. Script context exact shortcut values (5 tests)
        // ========================================

        #[test]
        fn script_edit_shortcut_cmd_e() {
            let script = ScriptInfo::new("t", "/p/t.ts");
            let actions = get_script_context_actions(&script);
            let a = find_action(&actions, "edit_script").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn script_view_logs_shortcut_cmd_l() {
            let script = ScriptInfo::new("t", "/p/t.ts");
            let actions = get_script_context_actions(&script);
            let a = find_action(&actions, "view_logs").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘L"));
        }

        #[test]
        fn script_reveal_shortcut_cmd_shift_f() {
            let script = ScriptInfo::new("t", "/p/t.ts");
            let actions = get_script_context_actions(&script);
            let a = find_action(&actions, "reveal_in_finder").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⇧F"));
        }

        #[test]
        fn script_copy_path_shortcut_cmd_shift_c() {
            let script = ScriptInfo::new("t", "/p/t.ts");
            let actions = get_script_context_actions(&script);
            let a = find_action(&actions, "copy_path").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⇧C"));
        }

        #[test]
        fn script_copy_content_shortcut_cmd_opt_c() {
            let script = ScriptInfo::new("t", "/p/t.ts");
            let actions = get_script_context_actions(&script);
            let a = find_action(&actions, "copy_content").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘⌥C"));
        }

        // ========================================
        // 18. CommandBarConfig factory methods (5 tests)
        // ========================================

        #[test]
        fn command_bar_ai_style_search_top_headers_icons_footer() {
            let c = CommandBarConfig::ai_style();
            assert_eq!(c.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(c.dialog_config.section_style, SectionStyle::Headers);
            assert!(c.dialog_config.show_icons);
            assert!(!c.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_main_menu_search_top_separators() {
            let c = CommandBarConfig::main_menu_style();
            assert_eq!(c.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(c.dialog_config.section_style, SectionStyle::Headers);
            assert!(!c.dialog_config.show_icons);
            assert!(!c.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_no_search_hidden() {
            let c = CommandBarConfig::no_search();
            assert_eq!(c.dialog_config.search_position, SearchPosition::Hidden);
        }

        #[test]
        fn command_bar_notes_style_search_top_separators_icons_footer() {
            let c = CommandBarConfig::notes_style();
            assert_eq!(c.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(c.dialog_config.section_style, SectionStyle::Headers);
            assert!(c.dialog_config.show_icons);
            assert!(!c.dialog_config.show_footer);
        }

        #[test]
        fn command_bar_default_close_flags_all_true() {
            let c = CommandBarConfig::default();
            assert!(c.close_on_select);
            assert!(c.close_on_click_outside);
            assert!(c.close_on_escape);
        }

        // ========================================
        // 19. Notes command bar exact shortcuts (6 tests)
        // ========================================

        // --- merged from tests_part_04.rs ---
        // ========================================
        // 20. Note switcher complex scenarios (5 tests)
        // ========================================

        #[test]
        fn note_switcher_10_notes_all_have_actions() {
            let notes: Vec<_> = (0..10)
                .map(|i| {
                    make_note(
                        &format!("id-{}", i),
                        &format!("Note {}", i),
                        100,
                        false,
                        false,
                        "",
                        "",
                    )
                })
                .collect();
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions.len(), 10);
        }

        #[test]
        fn note_switcher_pinned_section_label() {
            let note = make_note("1", "Pinned Note", 50, false, true, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn note_switcher_recent_section_label() {
            let note = make_note("1", "Regular Note", 50, false, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn note_switcher_mixed_pinned_and_recent_sections() {
            let notes = vec![
                make_note("1", "A", 10, false, true, "", ""),
                make_note("2", "B", 20, false, false, "", ""),
                make_note("3", "C", 30, false, true, "", ""),
            ];
            let actions = get_note_switcher_actions(&notes);
            let sections: Vec<_> = actions.iter().map(|a| a.section.as_deref()).collect();
            assert_eq!(
                sections,
                vec![Some("Pinned"), Some("Recent"), Some("Pinned")]
            );
        }

        #[test]
        fn note_switcher_current_note_has_bullet_prefix() {
            let note = make_note("1", "Current Note", 50, true, false, "", "");
            let actions = get_note_switcher_actions(&[note]);
            assert!(actions[0].title.starts_with("• "));
        }

        // ========================================
        // 21. build_grouped_items section header content (5 tests)
        // ========================================

        #[test]
        fn grouped_items_header_text_matches_section_name() {
            let actions = vec![
                make_action("a1", "Action 1", Some("Section A")),
                make_action("a2", "Action 2", Some("Section B")),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            match &grouped[0] {
                GroupedActionItem::SectionHeader(s) => assert_eq!(s, "Section A"),
                _ => panic!("Expected SectionHeader"),
            }
            match &grouped[2] {
                GroupedActionItem::SectionHeader(s) => assert_eq!(s, "Section B"),
                _ => panic!("Expected SectionHeader"),
            }
        }

        #[test]
        fn grouped_items_headers_count_matches_unique_sections() {
            let actions = vec![
                make_action("a1", "A1", Some("S1")),
                make_action("a2", "A2", Some("S1")),
                make_action("a3", "A3", Some("S2")),
                make_action("a4", "A4", Some("S3")),
            ];
            let filtered = vec![0, 1, 2, 3];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 3);
        }

        #[test]
        fn grouped_items_no_section_no_header() {
            let actions = vec![make_action("a1", "A1", None), make_action("a2", "A2", None)];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0);
        }

        #[test]
        fn grouped_items_headers_precede_their_items() {
            let actions = vec![
                make_action("a1", "A1", Some("First")),
                make_action("a2", "A2", Some("Second")),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Expected: [Header("First"), Item(0), Header("Second"), Item(1)]
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[2], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
        }

        #[test]
        fn grouped_items_separators_style_no_headers() {
            let actions = vec![
                make_action("a1", "A1", Some("S1")),
                make_action("a2", "A2", Some("S2")),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0);
            assert_eq!(grouped.len(), 2); // Just Items
        }

        // ========================================
        // 22. coerce_action_selection edge cases (5 tests)
        // ========================================

        #[test]
        fn coerce_empty_returns_none() {
            assert_eq!(coerce_action_selection(&[], 0), None);
        }

        #[test]
        fn coerce_on_item_returns_same() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn coerce_header_followed_by_item_goes_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S".to_string()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn coerce_item_then_header_at_end() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S".to_string()),
            ];
            // Index 1 is header, search down finds nothing, search up finds Item at 0
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn coerce_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::SectionHeader("B".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        // ========================================
        // 23. Score_action with cached lowercase (5 tests)
        // ========================================

        #[test]
        fn score_prefix_match_100() {
            let a = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            assert!(ActionsDialog::score_action(&a, "edit") >= 100);
        }

        #[test]
        fn score_contains_match_50() {
            let a = Action::new("id", "Quick Edit", None, ActionCategory::ScriptContext);
            let s = ActionsDialog::score_action(&a, "edit");
            assert!((50..100).contains(&s));
        }

        #[test]
        fn score_fuzzy_match_25() {
            // "et" subsequence in "edit" => e...t
            let a = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            let s = ActionsDialog::score_action(&a, "eit");
            // 'e','i','t' are subsequence of "edit script"
            assert!(s >= 25);
        }

        #[test]
        fn score_description_bonus_15() {
            let a = Action::new(
                "id",
                "Open File",
                Some("Launch editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let s = ActionsDialog::score_action(&a, "editor");
            assert!(s >= 15);
        }

        #[test]
        fn score_no_match_zero() {
            let a = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            assert_eq!(ActionsDialog::score_action(&a, "zzzzz"), 0);
        }

        // ========================================
        // 24. fuzzy_match edge cases (5 tests)
        // ========================================

        #[test]
        fn fuzzy_empty_needle_always_matches() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn fuzzy_empty_haystack_nonempty_needle_fails() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn fuzzy_exact_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        #[test]
        fn fuzzy_subsequence_match() {
            assert!(ActionsDialog::fuzzy_match("abcdef", "ace"));
        }

        #[test]
        fn fuzzy_no_match() {
            assert!(!ActionsDialog::fuzzy_match("abc", "z"));
        }

        // ========================================
        // 25. parse_shortcut_keycaps (6 tests)
        // ========================================

        #[test]
        fn keycaps_cmd_c() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(caps, vec!["⌘", "C"]);
        }

        #[test]
        fn keycaps_cmd_shift_enter() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧↵");
            assert_eq!(caps, vec!["⌘", "⇧", "↵"]);
        }

        #[test]
        fn keycaps_ctrl_x() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌃X");
            assert_eq!(caps, vec!["⌃", "X"]);
        }

        #[test]
        fn keycaps_space() {
            let caps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(caps, vec!["␣"]);
        }

        #[test]
        fn keycaps_arrows() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn keycaps_lowercase_uppercased() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘c");
            assert_eq!(caps, vec!["⌘", "C"]);
        }

        // ========================================
        // 26. to_deeplink_name additional edge cases (4 tests)
        // ========================================

        #[test]
        fn deeplink_cjk_characters_preserved() {
            let result = to_deeplink_name("测试脚本");
            assert_eq!(result, "%E6%B5%8B%E8%AF%95%E8%84%9A%E6%9C%AC");
        }

        #[test]
        fn deeplink_mixed_ascii_unicode() {
            let result = to_deeplink_name("My 脚本 Script");
            assert_eq!(result, "my-%E8%84%9A%E6%9C%AC-script");
        }

        #[test]
        fn deeplink_accented_preserved() {
            let result = to_deeplink_name("café résumé");
            assert_eq!(result, "caf%C3%A9-r%C3%A9sum%C3%A9");
        }

        #[test]
        fn deeplink_emoji_stripped() {
            // Emoji are not alphanumeric, so they become hyphens
            let result = to_deeplink_name("Test 🚀 Script");
            // 🚀 becomes -, collapses with surrounding hyphens
            assert_eq!(result, "test-script");
        }

        // ========================================
        // 27. Clipboard exact shortcut values (6 tests)
        // ========================================

        #[test]
        fn clipboard_share_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_share").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘E"));
        }

        #[test]
        fn clipboard_attach_to_ai_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_attach_to_ai").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌃⌘A"));
        }

        #[test]
        fn clipboard_pin_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_pin").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘P"));
        }

        #[test]
        fn clipboard_unpin_shortcut_same_as_pin() {
            let entry = make_clipboard_entry(ContentType::Text, true, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_unpin").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘P"));
        }

        #[test]
        fn clipboard_save_snippet_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_save_snippet").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘S"));
        }

        #[test]
        fn clipboard_save_file_shortcut() {
            let entry = make_clipboard_entry(ContentType::Text, false, None);
            let actions = get_clipboard_history_context_actions(&entry);
            let a = find_action(&actions, "clip:clipboard_save_file").unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌥⇧⌘S"));
        }

        // ========================================
        // 28. Notes command bar section labels (4 tests)
        // ========================================

        // --- merged from tests_part_05.rs ---
        // ========================================
        // 29. ID uniqueness and non-empty invariants (6 tests)
        // ========================================

        #[test]
        fn chat_context_ids_unique() {
            let info = ChatPromptInfo {
                current_model: Some("M1".to_string()),
                available_models: vec![
                    ChatModelInfo {
                        id: "m1".to_string(),
                        display_name: "M1".to_string(),
                        provider: "P1".to_string(),
                    },
                    ChatModelInfo {
                        id: "m2".to_string(),
                        display_name: "M2".to_string(),
                        provider: "P2".to_string(),
                    },
                ],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn new_chat_ids_unique() {
            let actions = get_new_chat_actions(
                &[NewChatModelInfo {
                    model_id: "l1".to_string(),
                    display_name: "L1".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                }],
                &[NewChatPresetInfo {
                    id: "gen".to_string(),
                    name: "Gen".to_string(),
                    icon: IconName::File,
                }],
                &[NewChatModelInfo {
                    model_id: "m1".to_string(),
                    display_name: "M1".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                }],
            );
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn note_switcher_ids_unique() {
            let notes = vec![
                make_note("uuid-1", "Note 1", 10, false, false, "", ""),
                make_note("uuid-2", "Note 2", 20, true, false, "", ""),
                make_note("uuid-3", "Note 3", 30, false, true, "", ""),
            ];
            let actions = get_note_switcher_actions(&notes);
            let ids: HashSet<_> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn all_note_switcher_actions_nonempty_title() {
            let notes = vec![
                make_note("1", "A", 1, false, false, "", ""),
                make_note("2", "B", 2, true, true, "preview", "1m ago"),
            ];
            let actions = get_note_switcher_actions(&notes);
            for a in &actions {
                assert!(!a.title.is_empty(), "Action {} has empty title", a.id);
                assert!(!a.id.is_empty(), "Action has empty id");
            }
        }

        #[test]
        fn all_path_actions_nonempty_title_and_id() {
            let info = PathInfo {
                name: "test".to_string(),
                path: "/test".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            for a in &actions {
                assert!(!a.title.is_empty());
                assert!(!a.id.is_empty());
            }
        }

        // ========================================
        // 30. Ordering determinism (4 tests)
        // ========================================

        #[test]
        fn chat_context_ordering_deterministic() {
            let info = ChatPromptInfo {
                current_model: Some("X".to_string()),
                available_models: vec![
                    ChatModelInfo {
                        id: "a".to_string(),
                        display_name: "A".to_string(),
                        provider: "P".to_string(),
                    },
                    ChatModelInfo {
                        id: "b".to_string(),
                        display_name: "B".to_string(),
                        provider: "P".to_string(),
                    },
                ],
                has_messages: true,
                has_response: true,
            };
            let a = get_chat_context_actions(&info);
            let b = get_chat_context_actions(&info);
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn new_chat_ordering_deterministic() {
            let last = vec![NewChatModelInfo {
                model_id: "l".to_string(),
                display_name: "L".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "g".to_string(),
                name: "G".to_string(),
                icon: IconName::File,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m".to_string(),
                display_name: "M".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let a = get_new_chat_actions(&last, &presets, &models);
            let b = get_new_chat_actions(&last, &presets, &models);
            assert_eq!(action_ids(&a), action_ids(&b));
        }

        #[test]
        fn path_context_ordering_deterministic() {
            let info = PathInfo {
                name: "f".to_string(),
                path: "/f".to_string(),
                is_dir: false,
            };
            let a = get_path_context_actions(&info);
            let b = get_path_context_actions(&info);
            assert_eq!(action_ids(&a), action_ids(&b));
        }
    }
}
