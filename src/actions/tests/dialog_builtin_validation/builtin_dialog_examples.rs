#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;

mod from_dialog_builtin_action_validation_tests_21 {
    // --- merged from part_01.rs ---
    //! Batch 21: Built-in action validation tests
    //!
    //! 146 tests across 30 categories validating built-in dialog actions.

    use super::builders::{
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
        get_chat_model_picker_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom,
    };
    use super::command_bar::CommandBarConfig;
    use super::dialog::{GroupedActionItem, build_grouped_items_static, coerce_action_selection};
    use super::types::{
        Action, ActionCategory, ActionsDialogConfig, AnchorPosition, ScriptInfo, SearchPosition,
        SectionStyle,
    };
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // ============================================================
    // 1. Script context: is_script / is_scriptlet / is_agent mutually
    //    exclusive action sets
    // ============================================================

    #[test]
    fn batch21_script_only_has_view_logs() {
        let s = ScriptInfo::new("s", "/p");
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn batch21_scriptlet_no_view_logs() {
        let s = ScriptInfo::scriptlet("s", "/p", None, None);
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn batch21_agent_no_view_logs() {
        let mut s = ScriptInfo::new("a", "/p");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn batch21_builtin_no_view_logs() {
        let s = ScriptInfo::builtin("B");
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn batch21_script_has_edit_script_agent_has_edit_agent() {
        let script = ScriptInfo::new("s", "/p");
        let sa = get_script_context_actions(&script);
        let edit = sa.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Script");

        let mut agent = ScriptInfo::new("a", "/p");
        agent.is_agent = true;
        agent.is_script = false;
        let aa = get_script_context_actions(&agent);
        let edit2 = aa.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit2.title, "Edit Agent");
    }

    // ============================================================
    // 2. Script context: run_script always first
    // ============================================================

    #[test]
    fn batch21_run_script_is_first_for_script() {
        let s = ScriptInfo::new("s", "/p");
        let actions = get_script_context_actions(&s);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn batch21_run_script_is_first_for_builtin() {
        let s = ScriptInfo::builtin("B");
        let actions = get_script_context_actions(&s);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn batch21_run_script_is_first_for_scriptlet() {
        let s = ScriptInfo::scriptlet("s", "/p", None, None);
        let actions = get_script_context_actions(&s);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn batch21_run_script_is_first_for_agent() {
        let mut s = ScriptInfo::new("a", "/p");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn batch21_run_script_is_first_for_scriptlet_with_custom() {
        let s = ScriptInfo::scriptlet("s", "/p", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert_eq!(actions[0].id, "run_script");
    }

    // ============================================================
    // 3. File context: title format includes quoted name
    // ============================================================

    #[test]
    fn batch21_file_open_title_contains_quoted_name() {
        let fi = FileInfo {
            path: "/tmp/readme.md".into(),
            name: "readme.md".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.title.contains("\"readme.md\""));
    }

    #[test]
    fn batch21_dir_open_title_contains_quoted_name() {
        let fi = FileInfo {
            path: "/tmp/docs".into(),
            name: "docs".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(open.title.contains("\"docs\""));
    }

    #[test]
    fn batch21_file_open_title_starts_with_open() {
        let fi = FileInfo {
            path: "/a".into(),
            name: "a".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.title.starts_with("Open"));
    }

    #[test]
    fn batch21_dir_open_title_starts_with_open() {
        let fi = FileInfo {
            path: "/a".into(),
            name: "a".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(open.title.starts_with("Open"));
    }

    // ============================================================
    // 4. Path context: dir primary=open_directory, file primary=select_file
    // ============================================================

    #[test]
    fn batch21_path_dir_primary_is_open_directory() {
        let pi = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn batch21_path_file_primary_is_select_file() {
        let pi = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn batch21_path_dir_title_contains_name() {
        let pi = PathInfo {
            path: "/mydir".into(),
            name: "mydir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        assert!(actions[0].title.contains("\"mydir\""));
    }

    #[test]
    fn batch21_path_file_title_contains_name() {
        let pi = PathInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert!(actions[0].title.contains("\"f.txt\""));
    }

    #[test]
    fn batch21_path_dir_and_file_same_action_count() {
        let dir = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        let file = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        assert_eq!(
            get_path_context_actions(&dir).len(),
            get_path_context_actions(&file).len()
        );
    }

    // ============================================================
    // 5. AI command bar: total action count and section counts
    // ============================================================

    #[test]
    fn batch21_ai_command_bar_total_12_actions() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    #[test]
    fn batch21_ai_command_bar_response_section_3() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(count, 3);
    }

    #[test]
    fn batch21_ai_command_bar_actions_section_4() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Actions"))
            .count();
        assert_eq!(count, 7);
    }

    #[test]
    fn batch21_ai_command_bar_attachments_section_2() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Attachments"))
            .count();
        assert_eq!(count, 4);
    }

    #[test]
    fn batch21_ai_command_bar_export_section_1() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Export"))
            .count();
        assert_eq!(count, 1);
    }

    // ============================================================
    // 6. AI command bar: copy_chat and copy_last_code details
    // ============================================================

    #[test]
    fn batch21_ai_copy_chat_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌥⇧⌘C"));
    }

    #[test]
    fn batch21_ai_copy_chat_icon_copy() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(a.icon, Some(IconName::Copy));
    }

    #[test]
    fn batch21_ai_copy_last_code_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌥⌘C"));
    }

    #[test]
    fn batch21_ai_copy_last_code_icon_code() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.icon, Some(IconName::Code));
    }

    #[test]
    fn batch21_ai_copy_last_code_section_response() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.section.as_deref(), Some("Response"));
    }

    // ============================================================
    // 7. AI command bar: paste_image details
    // ============================================================

    #[test]
    fn batch21_ai_paste_image_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘V"));
    }

    #[test]
    fn batch21_ai_paste_image_icon_file() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(a.icon, Some(IconName::File));
    }

    #[test]
    fn batch21_ai_paste_image_section_attachments() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(a.section.as_deref(), Some("Attachments"));
    }

    #[test]
    fn batch21_ai_add_attachment_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:add_file").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⇧⌘A"));
    }

    // ============================================================
    // 8. AI command bar: toggle_shortcuts_help details
    // ============================================================

    #[test]
    fn batch21_ai_toggle_shortcuts_help_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘/"));
    }

    #[test]
    fn batch21_ai_toggle_shortcuts_help_icon_star() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(a.icon, Some(IconName::Star));
    }

    #[test]
    fn batch21_ai_toggle_shortcuts_help_section_help() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(a.section.as_deref(), Some("Help"));
    }

    #[test]
    fn batch21_ai_change_model_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(a.shortcut.is_none());
    }

    #[test]
    fn batch21_ai_branch_from_last_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert!(a.shortcut.is_none());
    }

    // ============================================================
    // 9. Chat context: clear_conversation conditional
    // ============================================================

    #[test]
    fn batch21_chat_clear_absent_no_messages() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn batch21_chat_clear_present_with_messages() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn batch21_chat_clear_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let a = actions
            .iter()
            .find(|a| a.id == "chat:clear_conversation")
            .unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⌫"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn batch21_chat_copy_response_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_response")
            .unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘C"));
    }

    // ============================================================
    // 10. Chat context: continue_in_chat always after models
    // ============================================================

    #[test]
    fn batch21_chat_continue_after_models() {
        // After drill-down refactor: picker only has model rows,
        // continue_in_chat lives in root context actions.
        let info = ChatPromptInfo {
            current_model: Some("gpt-4".into()),
            available_models: vec![
                ChatModelInfo {
                    id: "gpt-4".into(),
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
        let picker = get_chat_model_picker_actions(&info);
        // Picker only has model rows
        assert!(
            picker
                .iter()
                .all(|a| a.id.starts_with("chat:select_model_"))
        );
        // continue_in_chat is in root context, not picker
        let root = get_chat_context_actions(&info);
        assert!(root.iter().any(|a| a.id == "chat:continue_in_chat"));
    }

    #[test]
    fn batch21_chat_continue_present_zero_models() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
    }

    #[test]
    fn batch21_chat_continue_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let a = actions
            .iter()
            .find(|a| a.id == "chat:continue_in_chat")
            .unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘↵"));
    }

    // ============================================================
    // 11. Notes command bar: copy section actions
    // ============================================================

    // ============================================================
    // 12. Notes command bar: enable_auto_sizing conditional
    // ============================================================

    // ============================================================
    // 13. Note switcher: relative_time propagation
    // ============================================================

    #[test]
    fn batch21_note_switcher_preview_and_time_joined() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "2m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert!(desc.contains("Hello world"));
        assert!(desc.contains("2m ago"));
        assert!(desc.contains(" · "));
    }

    #[test]
    fn batch21_note_switcher_no_time_no_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "Some text".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert_eq!(desc, "Some text");
        assert!(!desc.contains(" · "));
    }

    #[test]
    fn batch21_note_switcher_no_preview_with_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert_eq!(desc, "1h ago");
    }

    #[test]
    fn batch21_note_switcher_no_preview_no_time_char_count() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert_eq!(desc, "42 chars");
    }

    #[test]
    fn batch21_note_switcher_singular_char() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert_eq!(desc, "1 char");
    }

    // ============================================================
    // 14. New chat actions: ID format patterns
    // ============================================================

    #[test]
    fn batch21_new_chat_last_used_id_format() {
        let last_used = vec![NewChatModelInfo {
            model_id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].id, "last_used_openai::gpt-4");
    }

    #[test]
    fn batch21_new_chat_preset_id_format() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_general");
    }

    #[test]
    fn batch21_new_chat_model_id_format() {
        let models = vec![NewChatModelInfo {
            model_id: "claude-3".into(),
            display_name: "Claude 3".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_anthropic::claude-3");
    }

    #[test]
    fn batch21_new_chat_multiple_last_used_sequential_ids() {
        let last_used = vec![
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
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].id, "last_used_p::a");
        assert_eq!(actions[1].id, "last_used_p::b");
    }

    #[test]
    fn batch21_new_chat_empty_all_empty_result() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    // ============================================================
    // 15. Clipboard context: clipboard_copy description
    // ============================================================

    #[test]
    fn batch21_clipboard_copy_description_mentions_clipboard() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_copy")
            .unwrap();
        assert!(
            a.description
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    #[test]
    fn batch21_clipboard_paste_description_mentions_clipboard() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert!(
            a.description
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    #[test]
    fn batch21_clipboard_paste_keep_open_desc_mentions_keep() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste_keep_open")
            .unwrap();
        assert!(
            a.description
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("keep")
        );
    }

    #[test]
    fn batch21_clipboard_delete_all_desc_mentions_pinned() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete_all")
            .unwrap();
        assert!(
            a.description
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("pinned")
        );
    }

    // --- merged from part_03.rs ---

    // ============================================================
    // 16. Clipboard context: frontmost_app_name edge cases
    // ============================================================

    #[test]
    fn batch21_clipboard_paste_empty_string_app() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: Some("".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        // Empty string is treated like None: falls back to "Paste to Active App"
        assert_eq!(a.title, "Paste to Active App");
    }

    #[test]
    fn batch21_clipboard_paste_unicode_app() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Xcode \u{2013} Beta".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(a.title, "Paste to Xcode \u{2013} Beta");
    }

    // ============================================================
    // 17. CommandBarConfig preset field matrix
    // ============================================================

    #[test]
    fn batch21_config_default_search_top() {
        let c = CommandBarConfig::default();
        assert!(matches!(
            c.dialog_config.search_position,
            SearchPosition::Top
        ));
    }

    #[test]
    fn batch21_config_ai_style_anchor_top() {
        let c = CommandBarConfig::ai_style();
        assert!(matches!(c.dialog_config.anchor, AnchorPosition::Top));
    }

    #[test]
    fn batch21_config_main_menu_anchor_bottom() {
        let c = CommandBarConfig::main_menu_style();
        assert!(matches!(c.dialog_config.anchor, AnchorPosition::Bottom));
    }

    #[test]
    fn batch21_config_notes_style_icons_true() {
        let c = CommandBarConfig::notes_style();
        assert!(c.dialog_config.show_icons);
    }

    #[test]
    fn batch21_config_notes_style_footer_true() {
        let c = CommandBarConfig::notes_style();
        assert!(!c.dialog_config.show_footer);
    }

    // ============================================================
    // 18. ActionsDialogConfig default values
    // ============================================================

    #[test]
    fn batch21_dialog_config_default_search_top() {
        let c = ActionsDialogConfig::default();
        assert!(matches!(c.search_position, SearchPosition::Top));
    }

    #[test]
    fn batch21_dialog_config_default_section_separators() {
        let c = ActionsDialogConfig::default();
        assert!(matches!(c.section_style, SectionStyle::Headers));
    }

    #[test]
    fn batch21_dialog_config_default_anchor_bottom() {
        let c = ActionsDialogConfig::default();
        assert!(matches!(c.anchor, AnchorPosition::Bottom));
    }

    #[test]
    fn batch21_dialog_config_default_no_icons() {
        let c = ActionsDialogConfig::default();
        assert!(!c.show_icons);
    }

    #[test]
    fn batch21_dialog_config_default_no_footer() {
        let c = ActionsDialogConfig::default();
        assert!(!c.show_footer);
    }

    // ============================================================
    // 19. Action with_shortcut caching behavior
    // ============================================================

    #[test]
    fn batch21_action_with_shortcut_sets_shortcut_lower() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        assert_eq!(a.shortcut_lower, Some("⌘e".into()));
    }

    #[test]
    fn batch21_action_no_shortcut_lower_is_none() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(a.shortcut_lower.is_none());
    }

    #[test]
    fn batch21_action_title_lower_precomputed() {
        let a = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "edit script");
    }

    #[test]
    fn batch21_action_description_lower_precomputed() {
        let a = Action::new(
            "id",
            "T",
            Some("Open in $EDITOR".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(a.description_lower, Some("open in $editor".into()));
    }

    #[test]
    fn batch21_action_description_none_lower_none() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(a.description_lower.is_none());
    }

    // ============================================================
    // 20. Action builder chaining: with_icon, with_section
    // ============================================================

    #[test]
    fn batch21_action_with_icon_preserves_shortcut() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘X")
            .with_icon(IconName::Copy);
        assert_eq!(a.shortcut.as_deref(), Some("⌘X"));
        assert_eq!(a.icon, Some(IconName::Copy));
    }

    #[test]
    fn batch21_action_with_section_preserves_icon() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_icon(IconName::Star)
            .with_section("MySection");
        assert_eq!(a.icon, Some(IconName::Star));
        assert_eq!(a.section.as_deref(), Some("MySection"));
    }

    #[test]
    fn batch21_action_full_chain_all_fields() {
        let a = Action::new(
            "test_id",
            "Test Title",
            Some("Test Desc".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘T")
        .with_icon(IconName::Plus)
        .with_section("TestSection");

        assert_eq!(a.id, "test_id");
        assert_eq!(a.title, "Test Title");
        assert_eq!(a.description, Some("Test Desc".into()));
        assert_eq!(a.shortcut, Some("⌘T".into()));
        assert_eq!(a.icon, Some(IconName::Plus));
        assert_eq!(a.section, Some("TestSection".into()));
    }

    #[test]
    fn batch21_action_with_shortcut_opt_none_preserves() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘X")
            .with_shortcut_opt(None);
        assert_eq!(a.shortcut.as_deref(), Some("⌘X"));
    }

    #[test]
    fn batch21_action_with_shortcut_opt_some_sets() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘Y".into()));
        assert_eq!(a.shortcut.as_deref(), Some("⌘Y"));
    }

    // ============================================================
    // 21. build_grouped_items_static: section transitions
    // ============================================================

    #[test]
    fn batch21_grouped_items_headers_two_sections() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // S1 header, item0, S2 header, item1
        assert_eq!(grouped.len(), 4);
        assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
        assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
        assert!(matches!(grouped[2], GroupedActionItem::SectionHeader(_)));
        assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
    }

    #[test]
    fn batch21_grouped_items_headers_same_section_no_dup() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // S header, item0, item1
        assert_eq!(grouped.len(), 3);
        assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
    }

    #[test]
    fn batch21_grouped_items_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Just items, no headers
        assert_eq!(grouped.len(), 2);
        assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
    }

    #[test]
    fn batch21_grouped_items_empty_filtered() {
        let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // ============================================================
    // 22. coerce_action_selection: header skipping
    // ============================================================

    #[test]
    fn batch21_coerce_on_item_stays() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(1));
    }

    #[test]
    fn batch21_coerce_on_header_jumps_down() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn batch21_coerce_trailing_header_jumps_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn batch21_coerce_all_headers_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn batch21_coerce_empty_none() {
        let rows: Vec<GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ============================================================
    // 23. ScriptInfo constructor defaults
    // ============================================================

    #[test]
    fn batch21_scriptinfo_new_defaults() {
        let s = ScriptInfo::new("n", "/p");
        assert!(s.is_script);
        assert!(!s.is_scriptlet);
        assert!(!s.is_agent);
        assert_eq!(s.action_verb, "Run");
        assert!(!s.is_suggested);
        assert!(s.frecency_path.is_none());
    }

    #[test]
    fn batch21_scriptinfo_builtin_path_empty() {
        let s = ScriptInfo::builtin("B");
        assert!(s.path.is_empty());
        assert!(!s.is_script);
    }

    #[test]
    fn batch21_scriptinfo_scriptlet_flags() {
        let s = ScriptInfo::scriptlet("S", "/p", None, None);
        assert!(!s.is_script);
        assert!(s.is_scriptlet);
        assert!(!s.is_agent);
    }

    #[test]
    fn batch21_scriptinfo_with_frecency_chaining() {
        let s = ScriptInfo::new("n", "/p").with_frecency(true, Some("fp".into()));
        assert!(s.is_suggested);
        assert_eq!(s.frecency_path, Some("fp".into()));
        // Original fields preserved
        assert!(s.is_script);
    }

    // ============================================================
    // 24. Script context: copy_content shortcut consistent
    // ============================================================

    #[test]
    fn batch21_script_copy_content_shortcut() {
        let s = ScriptInfo::new("s", "/p");
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn batch21_scriptlet_copy_content_shortcut() {
        let s = ScriptInfo::scriptlet("s", "/p", None, None);
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn batch21_agent_copy_content_shortcut() {
        let mut s = ScriptInfo::new("a", "/p");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn batch21_scriptlet_with_custom_copy_content_shortcut() {
        let s = ScriptInfo::scriptlet("s", "/p", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let a = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⌥C"));
    }

    // ============================================================
    // 25. File vs path context: primary action IDs differ
    // ============================================================

    #[test]
    fn batch21_file_file_primary_is_open_file() {
        let fi = FileInfo {
            path: "/f".into(),
            name: "f".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        assert_eq!(actions[0].id, "file:open_file");
    }

    #[test]
    fn batch21_path_file_primary_is_select_file_in_file_vs_path() {
        let pi = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn batch21_file_dir_and_path_dir_same_primary_id() {
        let fi = FileInfo {
            path: "/d".into(),
            name: "d".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let pi = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        assert_eq!(
            get_file_context_actions(&fi)[0].id,
            get_path_context_actions(&pi)[0].id
        );
    }

    // ============================================================
    // 26. Path context: move_to_trash always last
    // ============================================================

    #[test]
    fn batch21_path_trash_last_for_dir() {
        let pi = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
    }

    // --- merged from part_04.rs ---

    #[test]
    fn batch21_path_trash_last_for_file() {
        let pi = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
    }

    #[test]
    fn batch21_path_trash_description_dir() {
        let pi = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(
            trash
                .description
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("folder")
        );
    }

    #[test]
    fn batch21_path_trash_description_file() {
        let pi = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(
            trash
                .description
                .as_deref()
                .unwrap()
                .to_lowercase()
                .contains("file")
        );
    }

    // ============================================================
    // 27. Cross-context: all built-in IDs are snake_case
    // ============================================================

    fn assert_snake_case_ids(actions: &[Action], context: &str) {
        for a in actions {
            // Scriptlet-defined actions have "scriptlet_action:" prefix and are allowed colons
            if a.id.starts_with("scriptlet_action:") {
                continue;
            }
            assert!(
                !a.id.contains(' '),
                "{} action '{}' has spaces (not snake_case)",
                context,
                a.id
            );
            assert!(
                !a.id.contains('-'),
                "{} action '{}' has hyphens (not snake_case)",
                context,
                a.id
            );
        }
    }

    #[test]
    fn batch21_snake_case_ids_script() {
        let s = ScriptInfo::new("s", "/p");
        assert_snake_case_ids(&get_script_context_actions(&s), "script");
    }

    #[test]
    fn batch21_snake_case_ids_builtin() {
        let s = ScriptInfo::builtin("B");
        assert_snake_case_ids(&get_script_context_actions(&s), "builtin");
    }

    #[test]
    fn batch21_snake_case_ids_scriptlet() {
        let s = ScriptInfo::scriptlet("S", "/p", None, None);
        assert_snake_case_ids(&get_script_context_actions(&s), "scriptlet");
    }

    #[test]
    fn batch21_snake_case_ids_clipboard() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        assert_snake_case_ids(&get_clipboard_history_context_actions(&entry), "clipboard");
    }

    #[test]
    fn batch21_snake_case_ids_ai() {
        assert_snake_case_ids(&get_ai_command_bar_actions(), "ai");
    }

    // ============================================================
    // 28. Cross-context: all actions have non-empty IDs and titles
    // ============================================================

    fn assert_nonempty_id_title(actions: &[Action], context: &str) {
        for a in actions {
            assert!(
                !a.id.is_empty(),
                "{}: action has empty ID (title={})",
                context,
                a.title
            );
            assert!(
                !a.title.is_empty(),
                "{}: action has empty title (id={})",
                context,
                a.id
            );
        }
    }

    #[test]
    fn batch21_nonempty_id_title_script() {
        let s = ScriptInfo::new("s", "/p");
        assert_nonempty_id_title(&get_script_context_actions(&s), "script");
    }

    #[test]
    fn batch21_nonempty_id_title_clipboard() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        assert_nonempty_id_title(
            &get_clipboard_history_context_actions(&entry),
            "clipboard_image",
        );
    }

    #[test]
    fn batch21_nonempty_id_title_path() {
        let pi = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        assert_nonempty_id_title(&get_path_context_actions(&pi), "path");
    }

    #[test]
    fn batch21_nonempty_id_title_file() {
        let fi = FileInfo {
            path: "/f".into(),
            name: "f".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        assert_nonempty_id_title(&get_file_context_actions(&fi), "file");
    }

    // ============================================================
    // 29. Script context: deeplink description URL format
    // ============================================================

    #[test]
    fn batch21_deeplink_url_format_script() {
        let s = ScriptInfo::new("My Script", "/p");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        let desc = dl.description.as_deref().unwrap();
        assert_eq!(dl.title, "Share");
        assert_eq!(desc, "Copy a portable Script Kit share link to clipboard");
    }

    #[test]
    fn batch21_deeplink_url_format_builtin() {
        let s = ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        let desc = dl.description.as_deref().unwrap();
        assert!(desc.contains("scriptkit://run/clipboard-history"));
    }

    #[test]
    fn batch21_deeplink_url_format_scriptlet() {
        let s = ScriptInfo::scriptlet("Open GitHub", "/p", None, None);
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        let desc = dl.description.as_deref().unwrap();
        assert_eq!(dl.title, "Share");
        assert_eq!(desc, "Copy a portable Script Kit share link to clipboard");
    }

    // ============================================================
    // 30. Cross-context: ID uniqueness within each context
    // ============================================================

    fn assert_unique_ids(actions: &[Action], context: &str) {
        let mut seen = std::collections::HashSet::new();
        for a in actions {
            assert!(
                seen.insert(&a.id),
                "{}: duplicate action ID '{}'",
                context,
                a.id
            );
        }
    }

    #[test]
    fn batch21_unique_ids_script() {
        let s = ScriptInfo::new("s", "/p");
        assert_unique_ids(&get_script_context_actions(&s), "script");
    }

    #[test]
    fn batch21_unique_ids_script_with_shortcut_and_alias() {
        let s = ScriptInfo::with_shortcut_and_alias("s", "/p", Some("⌘T".into()), Some("t".into()));
        assert_unique_ids(&get_script_context_actions(&s), "script_full");
    }

    #[test]
    fn batch21_unique_ids_clipboard_text() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        assert_unique_ids(
            &get_clipboard_history_context_actions(&entry),
            "clipboard_text",
        );
    }

    #[test]
    fn batch21_unique_ids_clipboard_image() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        assert_unique_ids(
            &get_clipboard_history_context_actions(&entry),
            "clipboard_image",
        );
    }

    #[test]
    fn batch21_unique_ids_ai() {
        assert_unique_ids(&get_ai_command_bar_actions(), "ai");
    }

    #[test]
    fn batch21_unique_ids_path_dir() {
        let pi = PathInfo {
            path: "/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        assert_unique_ids(&get_path_context_actions(&pi), "path_dir");
    }

    #[test]
    fn batch21_unique_ids_path_file() {
        let pi = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        assert_unique_ids(&get_path_context_actions(&pi), "path_file");
    }
}

mod from_dialog_builtin_action_validation_tests_22 {
    // --- merged from part_01.rs ---
    //! Batch 22: Built-in action validation tests
    //!
    //! ~140 tests across 30 categories validating built-in dialog actions.

    use super::builders::{
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
        get_chat_model_picker_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom,
    };
    use super::command_bar::CommandBarConfig;
    use super::dialog::{
        ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
    };
    use super::types::{
        Action, ActionCategory, ActionsDialogConfig, AnchorPosition, ScriptInfo, SearchPosition,
        SectionStyle,
    };
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // ============================================================
    // 1. format_shortcut_hint edge cases: empty, single char, trailing +
    // ============================================================

    #[test]
    fn batch22_format_shortcut_hint_empty_string() {
        let result = ActionsDialog::format_shortcut_hint("");
        assert_eq!(result, "");
    }

    #[test]
    fn batch22_format_shortcut_hint_single_letter() {
        // Single letter without '+' means it is both first and last part
        let result = ActionsDialog::format_shortcut_hint("c");
        assert_eq!(result, "C");
    }

    #[test]
    fn batch22_format_shortcut_hint_return_key() {
        let result = ActionsDialog::format_shortcut_hint("return");
        assert_eq!(result, "↵");
    }

    #[test]
    fn batch22_format_shortcut_hint_mixed_case_modifiers() {
        let result = ActionsDialog::format_shortcut_hint("Cmd+Shift+c");
        assert_eq!(result, "⌘⇧C");
    }

    #[test]
    fn batch22_format_shortcut_hint_opt_alias() {
        let result = ActionsDialog::format_shortcut_hint("opt+s");
        assert_eq!(result, "⌥S");
    }

    // ============================================================
    // 2. parse_shortcut_keycaps: single modifier, all modifiers, mixed
    // ============================================================

    #[test]
    fn batch22_parse_keycaps_single_modifier() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘");
        assert_eq!(caps, vec!["⌘"]);
    }

    #[test]
    fn batch22_parse_keycaps_all_four_modifiers() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧");
        assert_eq!(caps, vec!["⌘", "⌃", "⌥", "⇧"]);
    }

    #[test]
    fn batch22_parse_keycaps_lowercase_letter_uppercased() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘a");
        assert_eq!(caps, vec!["⌘", "A"]);
    }

    #[test]
    fn batch22_parse_keycaps_number_stays() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘1");
        assert_eq!(caps, vec!["⌘", "1"]);
    }

    #[test]
    fn batch22_parse_keycaps_empty_string() {
        let caps = ActionsDialog::parse_shortcut_keycaps("");
        assert!(caps.is_empty());
    }

    // ============================================================
    // 3. score_action: combined bonus from all fields
    // ============================================================

    #[test]
    fn batch22_score_prefix_plus_desc_plus_shortcut() {
        let action = Action::new(
            "id",
            "Edit Script",
            Some("Edit in editor".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "edit");
        // prefix=100 + desc(edit)=15 = 115
        assert!(score >= 115, "Expected >=115, got {}", score);
    }

    #[test]
    fn batch22_score_no_match_zero() {
        let action = Action::new("id", "Copy Path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "zzzzz");
        assert_eq!(score, 0);
    }

    #[test]
    fn batch22_score_desc_only_match() {
        let action = Action::new(
            "id",
            "Open File",
            Some("Launch the editor".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "editor");
        // No title match, desc match=15
        assert_eq!(score, 15);
    }

    #[test]
    fn batch22_score_shortcut_only_match() {
        let action =
            Action::new("id", "Run Script", None, ActionCategory::ScriptContext).with_shortcut("↵");
        let score = ActionsDialog::score_action(&action, "↵");
        // shortcut match=10
        assert!(score >= 10, "Expected >=10, got {}", score);
    }

    #[test]
    fn batch22_score_empty_search_is_prefix() {
        let action = Action::new("id", "Anything", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        // Empty string is prefix of everything
        assert!(score >= 100, "Expected >=100, got {}", score);
    }

    // ============================================================
    // 4. fuzzy_match: Unicode, emoji, repeated chars
    // ============================================================

    #[test]
    fn batch22_fuzzy_match_unicode_subsequence() {
        assert!(ActionsDialog::fuzzy_match("café latte", "cfl"));
    }

    #[test]
    fn batch22_fuzzy_match_exact() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn batch22_fuzzy_match_both_empty() {
        assert!(ActionsDialog::fuzzy_match("", ""));
    }

    #[test]
    fn batch22_fuzzy_match_needle_longer() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
    }

    #[test]
    fn batch22_fuzzy_match_repeated_chars() {
        assert!(ActionsDialog::fuzzy_match("banana", "aaa"));
    }

    // ============================================================
    // 5. to_deeplink_name: emoji stripped, single char, numeric
    // ============================================================

    #[test]
    fn batch22_deeplink_single_char() {
        assert_eq!(super::builders::to_deeplink_name("a"), "a");
    }

    #[test]
    fn batch22_deeplink_all_special_chars_returns_empty() {
        assert_eq!(super::builders::to_deeplink_name("@#$%^&*"), "_unnamed");
    }

    #[test]
    fn batch22_deeplink_numeric_only() {
        assert_eq!(super::builders::to_deeplink_name("42"), "42");
    }

    #[test]
    fn batch22_deeplink_underscores_to_hyphens() {
        assert_eq!(
            super::builders::to_deeplink_name("hello_world"),
            "hello-world"
        );
    }

    #[test]
    fn batch22_deeplink_empty_string() {
        assert_eq!(super::builders::to_deeplink_name(""), "_unnamed");
    }

    // ============================================================
    // 6. coerce_action_selection: item at end, headers at beginning
    // ============================================================

    #[test]
    fn batch22_coerce_item_at_end_headers_at_start() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(2));
    }

    #[test]
    fn batch22_coerce_single_item() {
        let rows = vec![GroupedActionItem::Item(0)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn batch22_coerce_single_header() {
        let rows = vec![GroupedActionItem::SectionHeader("H".into())];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn batch22_coerce_ix_beyond_bounds_clamped() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        // ix=999 should be clamped to len-1=1 which is an Item
        assert_eq!(coerce_action_selection(&rows, 999), Some(1));
    }

    #[test]
    fn batch22_coerce_empty_returns_none() {
        let rows: Vec<GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ============================================================
    // 7. build_grouped_items_static: section transitions from None to Some
    // ============================================================

    #[test]
    fn batch22_grouped_none_to_some_section() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Sec"),
        ];
        let filtered = vec![0, 1];
        let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // First action has no section → no header; second has section → header + item
        assert_eq!(items.len(), 3); // Item(0), SectionHeader("Sec"), Item(1)
    }

    #[test]
    fn batch22_grouped_rapid_alternation() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Y"),
            Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("X"),
        ];
        let filtered = vec![0, 1, 2];
        let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Each section change adds a header: X, item, Y, item, X, item = 6
        assert_eq!(items.len(), 6);
    }

    #[test]
    fn batch22_grouped_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Y"),
        ];
        let filtered = vec![0, 1];
        let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        assert_eq!(items.len(), 2); // No headers, just items
    }

    #[test]
    fn batch22_grouped_none_style_no_headers() {
        let actions =
            vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X")];
        let filtered = vec![0];
        let items = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        assert_eq!(items.len(), 1);
    }

    // ============================================================
    // 8. Script context: agent description mentions "agent"
    // ============================================================

    #[test]
    fn batch22_agent_edit_desc_mentions_agent() {
        let mut s = ScriptInfo::new("my-agent", "/p");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(
            edit.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent")
        );
    }

    #[test]
    fn batch22_agent_reveal_desc_mentions_agent() {
        let mut s = ScriptInfo::new("my-agent", "/p");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
        assert!(
            reveal
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent")
        );
    }

    #[test]
    fn batch22_agent_copy_path_desc_mentions_agent() {
        let mut s = ScriptInfo::new("my-agent", "/p");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
        assert!(
            cp.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent")
        );
    }

    #[test]
    fn batch22_script_edit_desc_mentions_editor() {
        let s = ScriptInfo::new("my-script", "/p");
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("$EDITOR"));
    }

    // ============================================================
    // 9. Script context: add/update/remove shortcut shortcuts are consistent
    // ============================================================

    #[test]
    fn batch22_add_shortcut_has_cmd_shift_k() {
        let s = ScriptInfo::new("s", "/p");
        let actions = get_script_context_actions(&s);
        let add = actions.iter().find(|a| a.id == "add_shortcut").unwrap();
        assert_eq!(add.shortcut.as_deref(), Some("⌘⇧K"));
    }

    #[test]
    fn batch22_update_shortcut_has_cmd_shift_k() {
        let s = ScriptInfo::with_shortcut("s", "/p", Some("cmd+x".into()));
        let actions = get_script_context_actions(&s);
        let upd = actions.iter().find(|a| a.id == "update_shortcut").unwrap();
        assert_eq!(upd.shortcut.as_deref(), Some("⌘⇧K"));
    }

    #[test]
    fn batch22_remove_shortcut_has_cmd_opt_k() {
        let s = ScriptInfo::with_shortcut("s", "/p", Some("cmd+x".into()));
        let actions = get_script_context_actions(&s);
        let rem = actions.iter().find(|a| a.id == "remove_shortcut").unwrap();
        assert_eq!(rem.shortcut.as_deref(), Some("⌘⌥K"));
    }

    #[test]
    fn batch22_add_alias_has_cmd_shift_a() {
        let s = ScriptInfo::new("s", "/p");
        let actions = get_script_context_actions(&s);
        let add = actions.iter().find(|a| a.id == "add_alias").unwrap();
        assert_eq!(add.shortcut.as_deref(), Some("⌘⇧A"));
    }

    #[test]
    fn batch22_remove_alias_has_cmd_opt_a() {
        let s = ScriptInfo::with_shortcut_and_alias("s", "/p", None, Some("a".into()));
        let actions = get_script_context_actions(&s);
        let rem = actions.iter().find(|a| a.id == "remove_alias").unwrap();
        assert_eq!(rem.shortcut.as_deref(), Some("⌘⌥A"));
    }

    // ============================================================
    // 10. Clipboard context: text vs image action count difference
    // ============================================================

    #[test]
    fn batch22_clipboard_text_action_count() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // Text has no OCR, no open_with, no annotate/upload cleanshot
        let text_count = actions.len();
        assert!(
            text_count >= 10,
            "Text should have >=10 actions, got {}",
            text_count
        );
    }

    #[test]
    fn batch22_clipboard_image_has_ocr() {
        let entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn batch22_clipboard_text_no_ocr() {
        let entry = ClipboardEntryInfo {
            id: "3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn batch22_clipboard_image_more_actions_than_text() {
        let text_entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let img_entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
            image_dimensions: Some((50, 50)),
            frontmost_app_name: None,
        };
        let t = get_clipboard_history_context_actions(&text_entry).len();
        let i = get_clipboard_history_context_actions(&img_entry).len();
        assert!(
            i > t,
            "Image {} should have more actions than text {}",
            i,
            t
        );
    }

    // ============================================================
    // 11. Clipboard context: pin/unpin toggle based on pinned state
    // ============================================================

    #[test]
    fn batch22_clipboard_unpinned_shows_pin() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
    }

    #[test]
    fn batch22_clipboard_pinned_shows_unpin() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
    }

    #[test]
    fn batch22_clipboard_pin_unpin_same_shortcut() {
        let pinned_entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let unpinned_entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let pa = get_clipboard_history_context_actions(&pinned_entry);
        let ua = get_clipboard_history_context_actions(&unpinned_entry);
        let unpin_shortcut = pa
            .iter()
            .find(|a| a.id == "clip:clipboard_unpin")
            .unwrap()
            .shortcut
            .as_deref();
        let pin_shortcut = ua
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap()
            .shortcut
            .as_deref();
        assert_eq!(unpin_shortcut, pin_shortcut);
        assert_eq!(pin_shortcut, Some("⇧⌘P"));
    }

    // ============================================================
    // 12. Chat context: model count affects total action count
    // ============================================================

    #[test]
    fn batch22_chat_zero_models_no_flags() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        // continue_in_chat + capture_screen_area
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].id, "chat:change_model");
    }

    #[test]
    fn batch22_chat_two_models_both_flags() {
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
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        // 2 models + continue + copy_response + clear_conversation + capture = 6
        assert_eq!(actions.len(), 5);
    }

    #[test]
    fn batch22_chat_current_model_checkmark() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let model_action = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(model_action.title.contains('✓'));
    }

    #[test]
    fn batch22_chat_non_current_model_no_checkmark() {
        let info = ChatPromptInfo {
            current_model: Some("Other".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let model_action = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(!model_action.title.contains('✓'));
    }

    // ============================================================
    // 13. AI command bar: every action has an icon
    // ============================================================

    #[test]
    fn batch22_ai_command_bar_all_have_icons() {
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
    fn batch22_ai_command_bar_all_have_sections() {
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
    fn batch22_ai_command_bar_total_is_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    #[test]
    fn batch22_ai_export_markdown_icon_is_filecode() {
        let actions = get_ai_command_bar_actions();
        let export = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(export.icon, Some(IconName::FileCode));
    }

    // ============================================================
    // 14. Notes command bar: trash mode removes most actions
    // ============================================================

    // ============================================================
    // 15. New chat actions: section assignment and ID patterns
    // ============================================================

    #[test]
    fn batch22_new_chat_last_used_section() {
        let last_used = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "OpenAI".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn batch22_new_chat_preset_section() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    #[test]
    fn batch22_new_chat_model_section() {
        let models = vec![NewChatModelInfo {
            model_id: "claude".into(),
            display_name: "Claude".into(),
            provider: "Anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    #[test]
    fn batch22_new_chat_id_patterns() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "M1".into(),
            provider: "P".into(),
            provider_display_name: "P".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m2".into(),
            display_name: "M2".into(),
            provider: "P".into(),
            provider_display_name: "P".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        assert_eq!(actions[0].id, "last_used_P::m1");
        assert_eq!(actions[1].id, "preset_code");
        assert_eq!(actions[2].id, "model_P::m2");
    }

    #[test]
    fn batch22_new_chat_empty_all_returns_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    // ============================================================
    // 16. Note switcher: icon priority hierarchy
    // ============================================================

    #[test]
    fn batch22_note_switcher_pinned_icon_starfilled() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: true,
            preview: "p".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn batch22_note_switcher_current_icon_check() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: true,
            is_pinned: false,
            preview: "p".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn batch22_note_switcher_regular_icon_file() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "p".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    #[test]
    fn batch22_note_switcher_pinned_trumps_current() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: true,
            is_pinned: true,
            preview: "p".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    // ============================================================
    // 17. Note switcher: description format (preview+time, char count)
    // ============================================================

    #[test]
    fn batch22_note_switcher_preview_plus_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "2m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.contains("Hello world"));
        assert!(desc.contains("2m ago"));
        assert!(desc.contains(" · "));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn batch22_note_switcher_no_preview_uses_char_count() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
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
    fn batch22_note_switcher_one_char_singular() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
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
    fn batch22_note_switcher_preview_truncation_at_60() {
        let long_preview = "a".repeat(70);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 70,
            is_current: false,
            is_pinned: false,
            preview: long_preview,
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.ends_with('…'));
    }

    // ============================================================
    // 18. Note switcher: current bullet prefix
    // ============================================================

    #[test]
    fn batch22_note_switcher_current_has_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "My Note".into(),
            char_count: 10,
            is_current: true,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(actions[0].title.starts_with("• "));
    }

    #[test]
    fn batch22_note_switcher_non_current_no_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "My Note".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(!actions[0].title.starts_with("• "));
    }

    // ============================================================
    // 19. Note switcher: section assignment by pin state
    // ============================================================

    #[test]
    fn batch22_note_switcher_pinned_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: true,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn batch22_note_switcher_unpinned_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Recent"));
    }

    // ============================================================
    // 20. File context: file vs dir primary action IDs
    // ============================================================

    #[test]
    fn batch22_file_context_file_primary_is_open_file() {
        let fi = FileInfo {
            name: "test.txt".into(),
            path: "/p/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        assert_eq!(actions[0].id, "file:open_file");
    }

    #[test]
    fn batch22_file_context_dir_primary_is_open_directory() {
        let fi = FileInfo {
            name: "docs".into(),
            path: "/p/docs".into(),
            is_dir: true,
            file_type: FileType::Directory,
        };
        let actions = get_file_context_actions(&fi);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn batch22_file_context_always_has_reveal_in_finder() {
        let fi = FileInfo {
            name: "f".into(),
            path: "/p/f".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
    }

    #[test]
    fn batch22_file_context_always_has_copy_path() {
        let fi = FileInfo {
            name: "f".into(),
            path: "/p/f".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        assert!(actions.iter().any(|a| a.id == "file:copy_path"));
    }

    #[test]
    fn batch22_file_context_copy_filename_has_shortcut() {
        let fi = FileInfo {
            name: "f".into(),
            path: "/p/f".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert_eq!(cf.shortcut.as_deref(), Some("⌘C"));
    }

    // ============================================================
    // 21. Path context: dir vs file primary action, trash always last
    // ============================================================

    #[test]
    fn batch22_path_dir_primary_is_open_directory() {
        let pi = PathInfo {
            name: "src".into(),
            path: "/src".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn batch22_path_file_primary_is_select_file() {
        let pi = PathInfo {
            name: "f.rs".into(),
            path: "/f.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn batch22_path_trash_is_always_last() {
        let pi_dir = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let pi_file = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let da = get_path_context_actions(&pi_dir);
        let fa = get_path_context_actions(&pi_file);
        assert_eq!(da.last().unwrap().id, "file:move_to_trash");
        assert_eq!(fa.last().unwrap().id, "file:move_to_trash");
    }

    #[test]
    fn batch22_path_trash_desc_dir_says_folder() {
        let pi = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(trash.description.as_ref().unwrap().contains("folder"));
    }

    #[test]
    fn batch22_path_trash_desc_file_says_file() {
        let pi = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(trash.description.as_ref().unwrap().contains("file"));
    }

    // ============================================================
    // 22. Path context: copy_filename has no shortcut
    // ============================================================

    #[test]
    fn batch22_path_copy_filename_no_shortcut() {
        let pi = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(cf.shortcut.is_none());
    }

    #[test]
    fn batch22_path_open_in_quick_terminal_shortcut() {
        let pi = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        let ot = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        // Quick Terminal intentionally ships without a shortcut.
        assert_eq!(ot.shortcut.as_deref(), None);
    }

    // ============================================================
    // 23. CommandBarConfig preset values
    // ============================================================

    #[test]
    fn batch22_command_bar_default_top_search() {
        let cfg = CommandBarConfig::default();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
    }

    #[test]
    fn batch22_command_bar_ai_top_search() {
        let cfg = CommandBarConfig::ai_style();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
        assert!(cfg.dialog_config.show_icons);
        assert!(!cfg.dialog_config.show_footer);
    }

    #[test]
    fn batch22_command_bar_no_search_hidden() {
        let cfg = CommandBarConfig::no_search();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Hidden);
    }

    #[test]
    fn batch22_command_bar_notes_separators() {
        let cfg = CommandBarConfig::notes_style();
        assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
        assert!(cfg.dialog_config.show_icons);
    }

    #[test]
    fn batch22_command_bar_main_menu_no_icons() {
        let cfg = CommandBarConfig::main_menu_style();
        assert!(!cfg.dialog_config.show_icons);
        assert!(!cfg.dialog_config.show_footer);
    }

    // ============================================================
    // 24. Action builder chaining preserves all fields
    // ============================================================

    #[test]
    fn batch22_action_chain_shortcut_icon_section() {
        let action = Action::new(
            "id",
            "Title",
            Some("Desc".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘T")
        .with_icon(IconName::Star)
        .with_section("Sec");
        assert_eq!(action.shortcut.as_deref(), Some("⌘T"));
        assert_eq!(action.icon, Some(IconName::Star));
        assert_eq!(action.section.as_deref(), Some("Sec"));
        assert_eq!(action.title, "Title");
        assert_eq!(action.description.as_deref(), Some("Desc"));
    }

    #[test]
    fn batch22_action_with_shortcut_opt_none_preserves() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘A")
            .with_shortcut_opt(None);
        // with_shortcut_opt(None) should NOT clear existing shortcut
        assert_eq!(action.shortcut.as_deref(), Some("⌘A"));
    }

    #[test]
    fn batch22_action_with_shortcut_opt_some_sets() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘B".into()));
        assert_eq!(action.shortcut.as_deref(), Some("⌘B"));
    }

    #[test]
    fn batch22_action_defaults_no_icon_no_section() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(action.icon.is_none());
        assert!(action.section.is_none());
        assert!(action.shortcut.is_none());
        assert!(!action.has_action);
        assert!(action.value.is_none());
    }

    // ============================================================
    // 25. Action lowercase caching correctness
    // ============================================================

    #[test]
    fn batch22_action_title_lower_precomputed() {
        let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "hello world");
    }

    #[test]
    fn batch22_action_description_lower_precomputed() {
        let action = Action::new(
            "id",
            "T",
            Some("Open In Editor".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower.as_deref(), Some("open in editor"));
    }

    #[test]
    fn batch22_action_shortcut_lower_after_with_shortcut() {
        let action =
            Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘e"));
    }

    #[test]
    fn batch22_action_no_shortcut_lower_is_none() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(action.shortcut_lower.is_none());
    }

    // ============================================================
    // 26. Scriptlet context with custom actions via get_scriptlet_context_actions_with_custom
    // ============================================================

    #[test]
    fn batch22_scriptlet_custom_run_is_first() {
        let script = ScriptInfo::scriptlet("My Script", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn batch22_scriptlet_custom_has_edit_scriptlet() {
        let script = ScriptInfo::scriptlet("My Script", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "edit_scriptlet"));
    }

    #[test]
    fn batch22_scriptlet_custom_has_copy_content() {
        let script = ScriptInfo::scriptlet("My Script", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "copy_content"));
    }

    #[test]
    fn batch22_scriptlet_custom_frecency_adds_reset() {
        let script = ScriptInfo::scriptlet("My Script", "/p.md", None, None)
            .with_frecency(true, Some("/frec".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        // Reset ranking should be last
        assert_eq!(actions.last().unwrap().id, "reset_ranking");
    }

    // ============================================================
    // 27. Cross-context: all actions have non-empty ID and title
    // ============================================================

    #[test]
    fn batch22_cross_script_non_empty_ids_titles() {
        let s = ScriptInfo::new("s", "/p");
        for a in get_script_context_actions(&s) {
            assert!(!a.id.is_empty(), "Action ID should not be empty");
            assert!(!a.title.is_empty(), "Action title should not be empty");
        }
    }

    // --- merged from part_04.rs ---

    #[test]
    fn batch22_cross_clipboard_non_empty_ids_titles() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for a in get_clipboard_history_context_actions(&entry) {
            assert!(!a.id.is_empty());
            assert!(!a.title.is_empty());
        }
    }

    #[test]
    fn batch22_cross_ai_non_empty_ids_titles() {
        for a in get_ai_command_bar_actions() {
            assert!(!a.id.is_empty());
            assert!(!a.title.is_empty());
        }
    }

    // ============================================================
    // 28. Cross-context: all built-in action IDs are snake_case
    // ============================================================

    fn is_snake_case(s: &str) -> bool {
        !s.contains(' ') && !s.contains('-') && s == s.to_lowercase()
            || s.starts_with("chat:select_model_") // model IDs may contain mixed case
            || s.starts_with("note_") // note IDs contain UUIDs
            || s.starts_with("last_used_")
            || s.starts_with("preset_")
            || s.starts_with("model_")
            || s.starts_with("scriptlet_action:")
    }

    #[test]
    fn batch22_cross_script_ids_snake_case() {
        let s = ScriptInfo::new("s", "/p");
        for a in get_script_context_actions(&s) {
            assert!(is_snake_case(&a.id), "ID '{}' should be snake_case", a.id);
        }
    }

    #[test]
    fn batch22_cross_ai_ids_snake_case() {
        for a in get_ai_command_bar_actions() {
            assert!(is_snake_case(&a.id), "ID '{}' should be snake_case", a.id);
        }
    }

    #[test]
    fn batch22_cross_clipboard_ids_snake_case() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for a in get_clipboard_history_context_actions(&entry) {
            assert!(is_snake_case(&a.id), "ID '{}' should be snake_case", a.id);
        }
    }

    #[test]
    fn batch22_cross_path_ids_snake_case() {
        let pi = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        for a in get_path_context_actions(&pi) {
            assert!(is_snake_case(&a.id), "ID '{}' should be snake_case", a.id);
        }
    }

    // ============================================================
    // 29. format_shortcut_hint: function keys and special aliases
    // ============================================================

    #[test]
    fn batch22_format_shortcut_control_alias() {
        let result = ActionsDialog::format_shortcut_hint("control+c");
        assert_eq!(result, "⌃C");
    }

    #[test]
    fn batch22_format_shortcut_meta_alias() {
        let result = ActionsDialog::format_shortcut_hint("meta+v");
        assert_eq!(result, "⌘V");
    }

    #[test]
    fn batch22_format_shortcut_super_alias() {
        let result = ActionsDialog::format_shortcut_hint("super+v");
        assert_eq!(result, "⌘V");
    }

    #[test]
    fn batch22_format_shortcut_option_alias() {
        let result = ActionsDialog::format_shortcut_hint("option+space");
        assert_eq!(result, "⌥␣");
    }

    #[test]
    fn batch22_format_shortcut_esc_alias() {
        let result = ActionsDialog::format_shortcut_hint("esc");
        assert_eq!(result, "⎋");
    }

    // ============================================================
    // 30. ActionsDialogConfig default values
    // ============================================================

    #[test]
    fn batch22_actions_dialog_config_default_search_top() {
        let cfg = ActionsDialogConfig::default();
        assert_eq!(cfg.search_position, SearchPosition::Top);
    }

    #[test]
    fn batch22_actions_dialog_config_default_section_separators() {
        let cfg = ActionsDialogConfig::default();
        assert_eq!(cfg.section_style, SectionStyle::Headers);
    }

    #[test]
    fn batch22_actions_dialog_config_default_anchor_bottom() {
        let cfg = ActionsDialogConfig::default();
        assert_eq!(cfg.anchor, AnchorPosition::Bottom);
    }

    #[test]
    fn batch22_actions_dialog_config_default_no_icons() {
        let cfg = ActionsDialogConfig::default();
        assert!(!cfg.show_icons);
    }

    #[test]
    fn batch22_actions_dialog_config_default_no_footer() {
        let cfg = ActionsDialogConfig::default();
        assert!(!cfg.show_footer);
    }
}

mod from_dialog_builtin_action_validation_tests_23 {
    // --- merged from part_01.rs ---
    //! Batch 23: Dialog builtin action validation tests
    //!
    //! 30 categories of tests validating random built-in action behaviors.

    use super::builders::*;
    use super::dialog::*;
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // ============================================================
    // 1. Script context: action_verb propagation in run_script title
    // ============================================================

    #[test]
    fn batch23_action_verb_run_default() {
        let script = ScriptInfo::new("my-script", "/path/to/script.ts");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.title.starts_with("Run"));
    }

    #[test]
    fn batch23_action_verb_launch() {
        let script =
            ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Launch");
    }

    #[test]
    fn batch23_action_verb_switch_to() {
        let script = ScriptInfo::with_action_verb("My Window", "window:123", false, "Switch to");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.title.starts_with("Switch To"));
    }

    #[test]
    fn batch23_action_verb_open() {
        let script = ScriptInfo::with_action_verb("Clipboard History", "builtin:ch", false, "Open");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.title.starts_with("Open"));
        assert!(run.description.as_ref().unwrap().contains("Open"));
    }

    #[test]
    fn batch23_action_verb_description_uses_verb() {
        let script = ScriptInfo::with_action_verb("Timer", "/path/timer.ts", true, "Start");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        // is_script=true → "{verb} this script"
        assert_eq!(run.description.as_ref().unwrap(), "Start this script");
    }

    // ============================================================
    // 2. Script context: action count varies by type flags
    // ============================================================

    #[test]
    fn batch23_script_action_count_full() {
        // is_script=true, no shortcut, no alias, not suggested
        let script = ScriptInfo::new("test", "/test.ts");
        let actions = get_script_context_actions(&script);
        // run_script + toggle_info + add_shortcut + add_alias + toggle_favorite + edit_script + view_logs + reveal_in_finder + file:open_in_quick_terminal + copy_path + copy_content + copy_deeplink + delete_script = 13
        assert_eq!(actions.len(), 13);
    }

    #[test]
    fn batch23_builtin_action_count() {
        let builtin = ScriptInfo::builtin("Test Built-in");
        let actions = get_script_context_actions(&builtin);
        // run_script + toggle_info + add_shortcut + add_alias + copy_deeplink = 5
        assert_eq!(actions.len(), 5);
    }

    #[test]
    fn batch23_scriptlet_action_count() {
        let scriptlet = ScriptInfo::scriptlet("Test", "/test.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        // run_script + toggle_info + add_shortcut + add_alias + toggle_favorite + edit_scriptlet + reveal_scriptlet_in_finder + file:open_in_quick_terminal + copy_scriptlet_path + copy_content + copy_deeplink = 11
        assert_eq!(actions.len(), 11);
    }

    #[test]
    fn batch23_script_with_shortcut_adds_two() {
        let script = ScriptInfo::with_shortcut("test", "/test.ts", Some("cmd+t".to_string()));
        let actions = get_script_context_actions(&script);
        // Same as full script (13) but shortcut adds one extra (update+remove instead of add = +1)
        assert_eq!(actions.len(), 14);
    }

    #[test]
    fn batch23_script_with_shortcut_and_alias_adds_two_more() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/test.ts",
            Some("cmd+t".to_string()),
            Some("ts".to_string()),
        );
        let actions = get_script_context_actions(&script);
        // script(13) + 1 extra shortcut + 1 extra alias = 15
        assert_eq!(actions.len(), 15);
    }

    // ============================================================
    // 3. Clipboard context: exact action ordering
    // ============================================================

    #[test]
    fn batch23_clipboard_text_first_three_actions() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
        assert_eq!(actions[1].id, "clip:clipboard_copy");
        assert_eq!(actions[2].id, "clip:clipboard_paste_keep_open");
    }

    #[test]
    fn batch23_clipboard_share_and_attach_after_paste_keep_open() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[3].id, "clip:clipboard_share");
        assert_eq!(actions[4].id, "clip:clipboard_attach_to_ai");
    }

    #[test]
    fn batch23_clipboard_destructive_actions_at_end() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();
        assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
        assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
        assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
    }

    #[test]
    fn batch23_clipboard_save_before_delete() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let snippet_idx = actions
            .iter()
            .position(|a| a.id == "clip:clipboard_save_snippet")
            .unwrap();
        let file_idx = actions
            .iter()
            .position(|a| a.id == "clip:clipboard_save_file")
            .unwrap();
        let delete_idx = actions
            .iter()
            .position(|a| a.id == "clip:clipboard_delete")
            .unwrap();
        assert!(snippet_idx < delete_idx);
        assert!(file_idx < delete_idx);
        assert!(file_idx == snippet_idx + 1);
    }

    // ============================================================
    // 4. Clipboard context: attach_to_ai shortcut
    // ============================================================

    #[test]
    fn batch23_clipboard_attach_to_ai_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let attach = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert_eq!(attach.shortcut.as_ref().unwrap(), "⌃⌘A");
    }

    #[test]
    fn batch23_clipboard_attach_to_ai_description() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let attach = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert!(
            attach
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent chat")
        );
    }

    #[test]
    fn batch23_clipboard_attach_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_attach_to_ai")
        );
    }

    // ============================================================
    // 5. Path context: exact action IDs in order for directory
    // ============================================================

    #[test]
    fn batch23_path_dir_action_ids_in_order() {
        let path = PathInfo::new("Documents", "/Users/test/Documents", true);
        let actions = get_path_context_actions(&path);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids[0], "file:open_directory");
        assert_eq!(ids[1], "file:copy_path");
        assert_eq!(ids[2], "file:open_in_finder");
        assert_eq!(ids[3], "file:open_in_quick_terminal");
        assert_eq!(ids[4], "file:open_in_editor");
        assert_eq!(ids[5], "file:copy_filename");
        assert_eq!(ids[6], "file:move_to_trash");
    }

    #[test]
    fn batch23_path_file_action_ids_in_order() {
        let path = PathInfo::new("readme.md", "/Users/test/readme.md", false);
        let actions = get_path_context_actions(&path);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids[0], "file:select_file");
        assert_eq!(ids[1], "file:copy_path");
        assert_eq!(ids[2], "file:open_in_finder");
        assert_eq!(ids[3], "file:open_in_quick_terminal");
        assert_eq!(ids[4], "file:open_in_editor");
        assert_eq!(ids[5], "file:copy_filename");
        assert_eq!(ids[6], "file:move_to_trash");
    }

    #[test]
    fn batch23_path_always_7_actions() {
        let dir = PathInfo::new("dir", "/dir", true);
        let file = PathInfo::new("file.txt", "/file.txt", false);
        assert_eq!(get_path_context_actions(&dir).len(), 7);
        assert_eq!(get_path_context_actions(&file).len(), 7);
    }

    // ============================================================
    // 6. Path context: open_in_editor description mentions $EDITOR
    // ============================================================

    #[test]
    fn batch23_path_open_in_editor_desc() {
        let path = PathInfo::new("test.txt", "/test.txt", false);
        let actions = get_path_context_actions(&path);
        let editor = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert!(editor.description.as_ref().unwrap().contains("$EDITOR"));
    }

    #[test]
    fn batch23_path_open_in_quick_terminal_desc() {
        let path = PathInfo::new("src", "/src", true);
        let actions = get_path_context_actions(&path);
        let terminal = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        assert!(
            terminal
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("terminal")
        );
    }

    #[test]
    fn batch23_path_copy_path_shortcut() {
        let path = PathInfo::new("test", "/test", false);
        let actions = get_path_context_actions(&path);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.shortcut.as_ref().unwrap(), "⌘⇧C");
    }

    // ============================================================
    // 7. File context: shortcut matrix
    // ============================================================

    #[test]
    fn batch23_file_open_shortcut_enter() {
        let file = FileInfo {
            path: "/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert_eq!(open.shortcut.as_ref().unwrap(), "↵");
    }

    #[test]
    fn batch23_file_reveal_shortcut() {
        let file = FileInfo {
            path: "/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let reveal = actions
            .iter()
            .find(|a| a.id == "file:reveal_in_finder")
            .unwrap();
        assert_eq!(reveal.shortcut.as_deref(), Some("⌘⇧F"));
    }

    #[test]
    fn batch23_file_copy_path_shortcut() {
        let file = FileInfo {
            path: "/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.shortcut.as_ref().unwrap(), "⌘⇧C");
    }

    #[test]
    fn batch23_file_copy_filename_shortcut() {
        let file = FileInfo {
            path: "/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert_eq!(cf.shortcut.as_ref().unwrap(), "⌘C");
    }

    // ============================================================
    // 8. File context: title includes quoted file name
    // ============================================================

    #[test]
    fn batch23_file_open_title_quotes_name() {
        let file = FileInfo {
            path: "/test/readme.md".to_string(),
            name: "readme.md".to_string(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.title.contains("\"readme.md\""));
    }

    #[test]
    fn batch23_file_dir_open_title_quotes_name() {
        let dir = FileInfo {
            path: "/test/src".to_string(),
            name: "src".to_string(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(open.title.contains("\"src\""));
    }

    #[test]
    fn batch23_file_open_dir_description() {
        let dir = FileInfo {
            path: "/test/docs".to_string(),
            name: "docs".to_string(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("folder")
        );
    }

    #[test]
    fn batch23_file_open_file_description() {
        let file = FileInfo {
            path: "/test/notes.txt".to_string(),
            name: "notes.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("default app")
        );
    }

    // --- merged from part_02.rs ---

    // ============================================================
    // 9. AI command bar: action shortcut presence/absence matrix
    // ============================================================

    #[test]
    fn batch23_ai_copy_response_has_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_response")
            .unwrap();
        assert_eq!(a.shortcut.as_ref().unwrap(), "⇧⌘C");
    }

    #[test]
    fn batch23_ai_copy_chat_has_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(a.shortcut.as_ref().unwrap(), "⌥⇧⌘C");
    }

    #[test]
    fn batch23_ai_copy_last_code_has_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.shortcut.as_ref().unwrap(), "⌥⌘C");
    }

    #[test]
    fn batch23_ai_branch_from_last_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert!(a.shortcut.is_none());
    }

    #[test]
    fn batch23_ai_change_model_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(a.shortcut.is_none());
    }

    // ============================================================
    // 10. AI command bar: description content validation
    // ============================================================

    #[test]
    fn batch23_ai_submit_description() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert!(
            a.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("send")
        );
    }

    #[test]
    fn batch23_ai_new_chat_description() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert!(
            a.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("new")
        );
    }

    #[test]
    fn batch23_ai_delete_chat_description() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert!(
            a.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("delete")
        );
    }

    #[test]
    fn batch23_ai_export_markdown_description() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert!(
            a.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("markdown")
        );
    }

    #[test]
    fn batch23_ai_paste_image_description() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert!(
            a.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    // ============================================================
    // 11. Chat context: model IDs use select_model_ prefix
    // ============================================================

    #[test]
    fn batch23_chat_model_id_prefix() {
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
    fn batch23_chat_multiple_models_sequential_ids() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![
                ChatModelInfo {
                    id: "claude-3".to_string(),
                    display_name: "Claude 3".to_string(),
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
        assert_eq!(picker[0].id, "chat:select_model_claude-3");
        assert_eq!(picker[1].id, "chat:select_model_gpt-4");
    }

    #[test]
    fn batch23_chat_model_descriptions_via_provider() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "opus".to_string(),
                display_name: "Claude Opus".to_string(),
                provider: "Anthropic".to_string(),
            }],
            has_messages: false,
            has_response: false,
        };
        // Model rows live in the drill-down picker
        let picker = get_chat_model_picker_actions(&info);
        assert_eq!(picker[0].description.as_ref().unwrap(), "Uses Anthropic");
    }

    #[test]
    fn batch23_chat_current_model_checkmark() {
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
        // Model rows live in the drill-down picker
        let picker = get_chat_model_picker_actions(&info);
        let gpt = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-4")
            .unwrap();
        assert!(gpt.title.contains("✓"));
        let claude = picker
            .iter()
            .find(|a| a.id == "chat:select_model_claude-3")
            .unwrap();
        assert!(!claude.title.contains("✓"));
    }

    // ============================================================
    // 12. Chat context: continue_in_chat is always present
    // ============================================================

    #[test]
    fn batch23_chat_continue_in_chat_always_present() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
    }

    #[test]
    fn batch23_chat_continue_in_chat_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let c = actions
            .iter()
            .find(|a| a.id == "chat:continue_in_chat")
            .unwrap();
        assert_eq!(c.shortcut.as_ref().unwrap(), "⌘↵");
    }

    #[test]
    fn batch23_chat_continue_after_models() {
        // After drill-down refactor: picker only has model rows,
        // continue_in_chat lives in root context actions.
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "m1".to_string(),
                display_name: "M1".to_string(),
                provider: "P".to_string(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        assert!(
            picker
                .iter()
                .all(|a| a.id.starts_with("chat:select_model_"))
        );
        let root = get_chat_context_actions(&info);
        assert!(root.iter().any(|a| a.id == "chat:continue_in_chat"));
    }

    // ============================================================
    // 13. Notes command bar: section icon assignments
    // ============================================================

    // ============================================================
    // 14. Notes command bar: shortcut assignments
    // ============================================================

    // ============================================================
    // 15. New chat actions: empty inputs produce empty output
    // ============================================================

    #[test]
    fn batch23_new_chat_all_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn batch23_new_chat_only_last_used() {
        let last_used = vec![NewChatModelInfo {
            model_id: "claude".to_string(),
            display_name: "Claude".to_string(),
            provider: "anthropic".to_string(),
            provider_display_name: "Anthropic".to_string(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_ref().unwrap(), "Last Used Settings");
    }

    #[test]
    fn batch23_new_chat_only_presets() {
        let presets = vec![NewChatPresetInfo {
            id: "general".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_ref().unwrap(), "Presets");
    }

    #[test]
    fn batch23_new_chat_only_models() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".to_string(),
            display_name: "GPT-4".to_string(),
            provider: "openai".to_string(),
            provider_display_name: "OpenAI".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_ref().unwrap(), "Models");
    }

    #[test]
    fn batch23_new_chat_mixed_sections_count() {
        let last = vec![NewChatModelInfo {
            model_id: "c".to_string(),
            display_name: "C".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "g".to_string(),
            name: "G".to_string(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m".to_string(),
            display_name: "M".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let actions = get_new_chat_actions(&last, &presets, &models);
        assert_eq!(actions.len(), 3);
    }

    // ============================================================
    // 16. New chat actions: icon assignments
    // ============================================================

    #[test]
    fn batch23_new_chat_last_used_icon() {
        let last = vec![NewChatModelInfo {
            model_id: "c".to_string(),
            display_name: "C".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let actions = get_new_chat_actions(&last, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn batch23_new_chat_model_icon() {
        let models = vec![NewChatModelInfo {
            model_id: "m".to_string(),
            display_name: "M".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].icon, Some(IconName::Settings));
    }

    #[test]
    fn batch23_new_chat_preset_icon_preserved() {
        let presets = vec![NewChatPresetInfo {
            id: "code".to_string(),
            name: "Code".to_string(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Code));
    }

    #[test]
    fn batch23_new_chat_preset_no_description() {
        let presets = vec![NewChatPresetInfo {
            id: "g".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses General preset")
        );
    }

    // ============================================================
    // 17. Note switcher: empty list produces placeholder
    // ============================================================

    #[test]
    fn batch23_note_switcher_empty_placeholder_id() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "no_notes");
    }

    #[test]
    fn batch23_note_switcher_empty_placeholder_title() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].title, "No notes yet");
    }

    #[test]
    fn batch23_note_switcher_empty_placeholder_icon() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].icon, Some(IconName::Plus));
    }

    #[test]
    fn batch23_note_switcher_empty_placeholder_section() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].section.as_ref().unwrap(), "Notes");
    }

    #[test]
    fn batch23_note_switcher_empty_placeholder_description() {
        let actions = get_note_switcher_actions(&[]);
        assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
    }

    // ============================================================
    // 18. Note switcher: multi-note section assignment
    // ============================================================

    #[test]
    fn batch23_note_switcher_pinned_and_recent_sections() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "1".to_string(),
                title: "Pinned Note".to_string(),
                char_count: 50,
                is_current: false,
                is_pinned: true,
                preview: "pinned content".to_string(),
                relative_time: "1h ago".to_string(),
            },
            NoteSwitcherNoteInfo {
                id: "2".to_string(),
                title: "Recent Note".to_string(),
                char_count: 30,
                is_current: false,
                is_pinned: false,
                preview: "recent content".to_string(),
                relative_time: "5m ago".to_string(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_ref().unwrap(), "Pinned");
        assert_eq!(actions[1].section.as_ref().unwrap(), "Recent");
    }

    #[test]
    fn batch23_note_switcher_all_pinned() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "1".to_string(),
                title: "A".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "2".to_string(),
                title: "B".to_string(),
                char_count: 20,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert!(
            actions
                .iter()
                .all(|a| a.section.as_ref().unwrap() == "Pinned")
        );
    }

    #[test]
    fn batch23_note_switcher_all_recent() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "1".to_string(),
            title: "A".to_string(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_ref().unwrap(), "Recent");
    }

    // ============================================================
    // 19. Note switcher: id format uses note_{uuid}
    // ============================================================

    #[test]
    fn batch23_note_switcher_id_format() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc-123".to_string(),
            title: "Test".to_string(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].id, "note_abc-123");
    }

    #[test]
    fn batch23_note_switcher_multiple_ids_unique() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "A".to_string(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
            NoteSwitcherNoteInfo {
                id: "b".to_string(),
                title: "B".to_string(),
                char_count: 2,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_ne!(actions[0].id, actions[1].id);
    }

    // ============================================================
    // 20. Scriptlet defined actions: has_action and value
    // ============================================================

    #[test]
    fn batch23_scriptlet_defined_has_action_true() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Act".to_string(),
            command: "act-cmd".to_string(),
            tool: "bash".to_string(),
            code: "echo act".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions[0].has_action);
    }

    #[test]
    fn batch23_scriptlet_defined_value_is_command() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "copy-cmd".to_string(),
            tool: "bash".to_string(),
            code: "pbcopy".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].value.as_ref().unwrap(), "copy-cmd");
    }

    #[test]
    fn batch23_scriptlet_defined_id_prefix() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "My Action".to_string(),
            command: "my-action".to_string(),
            tool: "bash".to_string(),
            code: "echo".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions[0].id.starts_with("scriptlet_action:"));
        assert_eq!(actions[0].id, "scriptlet_action:my-action");
    }

    #[test]
    fn batch23_scriptlet_defined_shortcut_formatted() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Test".to_string(),
            command: "test".to_string(),
            tool: "bash".to_string(),
            code: "echo".to_string(),
            inputs: vec![],
            shortcut: Some("cmd+shift+x".to_string()),
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].shortcut.as_ref().unwrap(), "⌘⇧X");
    }

    // ============================================================
    // 21. Scriptlet context with custom: ordering of custom vs built-in
    // ============================================================

    #[test]
    fn batch23_scriptlet_custom_between_run_and_shortcut() {
        let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
        let mut scriptlet =
            Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Custom".to_string(),
            command: "custom".to_string(),
            tool: "bash".to_string(),
            code: "echo".to_string(),
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
        assert_eq!(run_idx, 0);
        assert_eq!(custom_idx, 1);
        assert!(shortcut_idx > custom_idx);
    }

    #[test]
    fn batch23_scriptlet_custom_multiple_preserve_order() {
        let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
        let mut scriptlet =
            Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![
            ScriptletAction {
                name: "First".to_string(),
                command: "first".to_string(),
                tool: "bash".to_string(),
                code: "echo 1".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
            ScriptletAction {
                name: "Second".to_string(),
                command: "second".to_string(),
                tool: "bash".to_string(),
                code: "echo 2".to_string(),
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
        assert!(first_idx < second_idx);
        assert_eq!(first_idx, 1); // right after run_script
        assert_eq!(second_idx, 2);
    }

    // ============================================================
    // 22. to_deeplink_name: whitespace and mixed input
    // ============================================================

    #[test]
    fn batch23_deeplink_tabs_and_newlines() {
        assert_eq!(to_deeplink_name("hello\tworld\ntest"), "hello-world-test");
    }

    #[test]
    fn batch23_deeplink_multiple_spaces() {
        assert_eq!(to_deeplink_name("a   b"), "a-b");
    }

    #[test]
    fn batch23_deeplink_leading_trailing_specials() {
        assert_eq!(to_deeplink_name("--hello--"), "hello");
    }

    #[test]
    fn batch23_deeplink_mixed_alpha_numeric_special() {
        assert_eq!(to_deeplink_name("Script #1 (beta)"), "script-1-beta");
    }

    #[test]
    fn batch23_deeplink_unicode_preserved() {
        let result = to_deeplink_name("日本語スクリプト");
        assert!(result.contains("%E6%97%A5"));
    }

    // ============================================================
    // 23. format_shortcut_hint (ActionsDialog): modifier ordering
    // ============================================================

    #[test]
    fn batch23_format_cmd_c() {
        assert_eq!(ActionsDialog::format_shortcut_hint("cmd+c"), "⌘C");
    }

    #[test]
    fn batch23_format_ctrl_shift_delete() {
        assert_eq!(
            ActionsDialog::format_shortcut_hint("ctrl+shift+delete"),
            "⌃⇧⌫"
        );
    }

    #[test]
    fn batch23_format_alt_enter() {
        assert_eq!(ActionsDialog::format_shortcut_hint("alt+enter"), "⌥↵");
    }

    #[test]
    fn batch23_format_meta_is_cmd() {
        assert_eq!(ActionsDialog::format_shortcut_hint("meta+a"), "⌘A");
    }

    #[test]
    fn batch23_format_super_is_cmd() {
        assert_eq!(ActionsDialog::format_shortcut_hint("super+k"), "⌘K");
    }

    // ============================================================
    // 24. parse_shortcut_keycaps: multi-char and edge cases
    // ============================================================

    #[test]
    fn batch23_parse_keycaps_cmd_enter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
        assert_eq!(caps, vec!["⌘", "↵"]);
    }

    #[test]
    fn batch23_parse_keycaps_single_letter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("a");
        assert_eq!(caps, vec!["A"]);
    }

    #[test]
    fn batch23_parse_keycaps_arrows() {
        let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
        assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
    }

    #[test]
    fn batch23_parse_keycaps_all_modifiers() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧");
        assert_eq!(caps, vec!["⌘", "⌃", "⌥", "⇧"]);
    }

    #[test]
    fn batch23_parse_keycaps_lowercase_uppercased() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘c");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    // ============================================================
    // 25. score_action: fuzzy vs prefix vs contains
    // ============================================================

    #[test]
    fn batch23_score_prefix_highest() {
        let action = Action::new("a", "Copy Path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "copy");
        assert!(score >= 100);
    }

    #[test]
    fn batch23_score_contains_medium() {
        let action = Action::new("a", "Full Copy Path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "copy");
        assert!((50..100).contains(&score));
    }

    // --- merged from part_04.rs ---

    #[test]
    fn batch23_score_fuzzy_low() {
        let action = Action::new(
            "a",
            "Configure Options Pretty",
            None,
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "cop");
        // "cop" is a subsequence of "configure options pretty"
        assert!(score >= 25);
    }

    #[test]
    fn batch23_score_no_match_zero() {
        let action = Action::new("a", "Delete", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "xyz");
        assert_eq!(score, 0);
    }

    #[test]
    fn batch23_score_empty_search_prefix() {
        let action = Action::new("a", "Anything", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        assert!(score >= 100);
    }

    // ============================================================
    // 26. fuzzy_match: edge cases
    // ============================================================

    #[test]
    fn batch23_fuzzy_exact_match() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn batch23_fuzzy_subsequence() {
        assert!(ActionsDialog::fuzzy_match("hello world", "hwd"));
    }

    #[test]
    fn batch23_fuzzy_no_match() {
        assert!(!ActionsDialog::fuzzy_match("abc", "xyz"));
    }

    #[test]
    fn batch23_fuzzy_empty_needle_matches() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn batch23_fuzzy_needle_longer_fails() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
    }

    // ============================================================
    // 27. build_grouped_items_static: headers style adds section labels
    // ============================================================

    #[test]
    fn batch23_grouped_headers_two_sections() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // S1 header + item A + S2 header + item B = 4
        assert_eq!(grouped.len(), 4);
        assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "S1"));
        assert!(matches!(&grouped[2], GroupedActionItem::SectionHeader(s) if s == "S2"));
    }

    #[test]
    fn batch23_grouped_headers_same_section_no_dup() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S1"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // S1 header + item A + item B = 3 (no second header)
        assert_eq!(grouped.len(), 3);
    }

    #[test]
    fn batch23_grouped_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // No headers with Separators, just items
        assert_eq!(grouped.len(), 2);
        assert!(matches!(&grouped[0], GroupedActionItem::Item(_)));
        assert!(matches!(&grouped[1], GroupedActionItem::Item(_)));
    }

    #[test]
    fn batch23_grouped_empty_filtered() {
        let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // ============================================================
    // 28. coerce_action_selection: various scenarios
    // ============================================================

    #[test]
    fn batch23_coerce_empty_returns_none() {
        assert_eq!(coerce_action_selection(&[], 0), None);
    }

    #[test]
    fn batch23_coerce_item_returns_same() {
        let rows = vec![GroupedActionItem::Item(0)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn batch23_coerce_header_skips_to_item() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".to_string()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn batch23_coerce_header_at_end_searches_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn batch23_coerce_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".to_string()),
            GroupedActionItem::SectionHeader("B".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ============================================================
    // 29. Action builder: has_action defaults to false
    // ============================================================

    #[test]
    fn batch23_action_default_has_action_false() {
        let action = Action::new("id", "title", None, ActionCategory::ScriptContext);
        assert!(!action.has_action);
    }

    #[test]
    fn batch23_action_default_value_none() {
        let action = Action::new("id", "title", None, ActionCategory::ScriptContext);
        assert!(action.value.is_none());
    }

    #[test]
    fn batch23_action_default_icon_none() {
        let action = Action::new("id", "title", None, ActionCategory::ScriptContext);
        assert!(action.icon.is_none());
    }

    #[test]
    fn batch23_action_default_section_none() {
        let action = Action::new("id", "title", None, ActionCategory::ScriptContext);
        assert!(action.section.is_none());
    }

    #[test]
    fn batch23_action_with_all_builders() {
        let action = Action::new(
            "id",
            "Title",
            Some("Desc".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘A")
        .with_icon(IconName::Star)
        .with_section("S1");
        assert_eq!(action.shortcut.as_ref().unwrap(), "⌘A");
        assert_eq!(action.icon, Some(IconName::Star));
        assert_eq!(action.section.as_ref().unwrap(), "S1");
        assert_eq!(action.title_lower, "title");
        assert_eq!(action.description_lower.as_ref().unwrap(), "desc");
        assert_eq!(action.shortcut_lower.as_ref().unwrap(), "⌘a");
    }

    // ============================================================
    // 30. Cross-context: all contexts produce at least one action
    // ============================================================

    #[test]
    fn batch23_cross_script_has_actions() {
        let script = ScriptInfo::new("t", "/t.ts");
        assert!(!get_script_context_actions(&script).is_empty());
    }

    #[test]
    fn batch23_cross_builtin_has_actions() {
        let b = ScriptInfo::builtin("B");
        assert!(!get_script_context_actions(&b).is_empty());
    }

    #[test]
    fn batch23_cross_clipboard_text_has_actions() {
        let e = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        assert!(!get_clipboard_history_context_actions(&e).is_empty());
    }

    #[test]
    fn batch23_cross_clipboard_image_has_actions() {
        let e = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((1, 1)),
            frontmost_app_name: None,
        };
        assert!(!get_clipboard_history_context_actions(&e).is_empty());
    }

    #[test]
    fn batch23_cross_path_has_actions() {
        let p = PathInfo::new("t", "/t", false);
        assert!(!get_path_context_actions(&p).is_empty());
    }

    #[test]
    fn batch23_cross_file_has_actions() {
        let f = FileInfo {
            path: "/t".to_string(),
            name: "t".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        assert!(!get_file_context_actions(&f).is_empty());
    }

    #[test]
    fn batch23_cross_ai_has_actions() {
        assert!(!get_ai_command_bar_actions().is_empty());
    }

    #[test]
    fn batch23_cross_chat_has_actions() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        assert!(!get_chat_context_actions(&info).is_empty());
    }

    #[test]
    fn batch23_cross_note_switcher_empty_has_placeholder() {
        assert!(!get_note_switcher_actions(&[]).is_empty());
    }
}

mod from_dialog_builtin_action_validation_tests_24 {
    // --- merged from part_01.rs ---
    //! Batch 24: Dialog builtin action validation tests
    //!
    //! 131 tests across 30 categories validating random built-in action behaviors.

    use super::builders::*;
    use super::command_bar::CommandBarConfig;
    use super::dialog::*;
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // ============================================================
    // 1. Agent context: is_agent flag enables agent-specific actions
    // ============================================================

    #[test]
    fn batch24_agent_has_edit_agent_title() {
        let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn batch24_agent_has_copy_content() {
        let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "copy_content"));
    }

    #[test]
    fn batch24_agent_lacks_view_logs() {
        let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn batch24_agent_has_reveal_in_finder() {
        let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
    }

    // ============================================================
    // 2. Agent edit description mentions agent file
    // ============================================================

    #[test]
    fn batch24_agent_edit_desc_mentions_agent_file() {
        let mut script = ScriptInfo::new("Agent X", "/path/to/agent");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn batch24_agent_reveal_desc_mentions_agent() {
        let mut script = ScriptInfo::new("Agent X", "/path/to/agent");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
        assert!(reveal.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn batch24_agent_copy_path_desc_mentions_agent() {
        let mut script = ScriptInfo::new("Agent X", "/path/to/agent");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
        assert!(cp.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn batch24_script_edit_desc_mentions_editor() {
        let script = ScriptInfo::new("My Script", "/path/to/script.ts");
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("$EDITOR"));
    }

    // ============================================================
    // 3. ScriptInfo constructors: is_agent defaults to false
    // ============================================================

    #[test]
    fn batch24_new_is_agent_false() {
        let s = ScriptInfo::new("test", "/path");
        assert!(!s.is_agent);
    }

    #[test]
    fn batch24_builtin_is_agent_false() {
        let s = ScriptInfo::builtin("Clipboard");
        assert!(!s.is_agent);
    }

    #[test]
    fn batch24_scriptlet_is_agent_false() {
        let s = ScriptInfo::scriptlet("Open URL", "/path.md", None, None);
        assert!(!s.is_agent);
    }

    #[test]
    fn batch24_with_shortcut_is_agent_false() {
        let s = ScriptInfo::with_shortcut("test", "/path", Some("cmd+t".to_string()));
        assert!(!s.is_agent);
    }

    #[test]
    fn batch24_with_all_is_agent_false() {
        let s = ScriptInfo::with_all("test", "/path", true, "Run", None, None);
        assert!(!s.is_agent);
    }

    // ============================================================
    // 4. Chat context: has_response/has_messages flag combinations
    // ============================================================

    #[test]
    fn batch24_chat_no_response_no_messages() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        // continue_in_chat + capture_screen_area
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].id, "chat:change_model");
    }

    #[test]
    fn batch24_chat_response_only() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue_in_chat + copy_response + capture_screen_area = 4
        assert_eq!(actions.len(), 4);
        assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
        assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn batch24_chat_messages_only() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue_in_chat + clear_conversation + capture_screen_area = 4
        assert_eq!(actions.len(), 4);
        assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
        assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn batch24_chat_both_flags() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions.len(), 5);
        assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
        assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    // ============================================================
    // 5. Chat context: model checkmark only for current model
    // ============================================================

    #[test]
    fn batch24_chat_current_model_checkmark() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".to_string()),
            available_models: vec![
                ChatModelInfo {
                    id: "gpt4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                },
                ChatModelInfo {
                    id: "claude".to_string(),
                    display_name: "Claude".to_string(),
                    provider: "Anthropic".to_string(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let gpt4 = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(gpt4.title.contains("✓"));
        let claude = picker
            .iter()
            .find(|a| a.id == "chat:select_model_claude")
            .unwrap();
        assert!(!claude.title.contains("✓"));
    }

    #[test]
    fn batch24_chat_no_current_model_no_checkmark() {
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
        let gpt4 = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(!gpt4.title.contains("✓"));
    }

    #[test]
    fn batch24_chat_model_description_via_provider() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "m1".to_string(),
                display_name: "Model One".to_string(),
                provider: "TestProvider".to_string(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m1 = picker
            .iter()
            .find(|a| a.id == "chat:select_model_m1")
            .unwrap();
        assert_eq!(m1.description.as_ref().unwrap(), "Uses TestProvider");
    }

    // ============================================================
    // 6. Clipboard macOS-specific image actions (cfg(target_os = "macos"))
    // ============================================================

    #[cfg(target_os = "macos")]
    #[test]
    fn batch24_clipboard_image_has_open_with() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch24_clipboard_image_has_annotate_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_annotate_cleanshot")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch24_clipboard_image_has_upload_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_upload_cleanshot")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch24_clipboard_text_no_open_with() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
    }

    // ============================================================
    // 7. Clipboard: OCR only for image, not text
    // ============================================================

    #[test]
    fn batch24_clipboard_image_has_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn batch24_clipboard_text_no_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn batch24_clipboard_ocr_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert_eq!(ocr.shortcut.as_ref().unwrap(), "⇧⌘C");
    }

    // ============================================================
    // 8. Clipboard: image with None dimensions still gets image actions
    // ============================================================

    #[test]
    fn batch24_clipboard_image_no_dimensions_still_has_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn batch24_clipboard_image_no_dimensions_has_paste() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_paste"));
    }

    // ============================================================
    // 9. Notes: trash mode minimal actions
    // ============================================================

    // --- merged from part_02.rs ---

    // ============================================================
    // 10. Notes full mode with selection: maximum actions
    // ============================================================

    // ============================================================
    // 11. Notes icon assignments
    // ============================================================

    // ============================================================
    // 12. Note switcher: notes with empty preview fall back to char count
    // ============================================================

    #[test]
    fn batch24_note_switcher_empty_preview_zero_chars() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".to_string(),
            title: "Empty".to_string(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_ref().unwrap(), "0 chars");
    }

    #[test]
    fn batch24_note_switcher_empty_preview_one_char() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".to_string(),
            title: "One".to_string(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_ref().unwrap(), "1 char");
    }

    #[test]
    fn batch24_note_switcher_empty_preview_with_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".to_string(),
            title: "T".to_string(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "5m ago".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_ref().unwrap(), "5m ago");
    }

    #[test]
    fn batch24_note_switcher_preview_with_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".to_string(),
            title: "T".to_string(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "Some content".to_string(),
            relative_time: "2h ago".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(actions[0].description.as_ref().unwrap().contains(" · "));
    }

    // ============================================================
    // 13. Note switcher: pinned + current icon priority
    // ============================================================

    #[test]
    fn batch24_note_switcher_pinned_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".to_string(),
            title: "Pinned".to_string(),
            char_count: 10,
            is_current: false,
            is_pinned: true,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn batch24_note_switcher_current_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".to_string(),
            title: "Current".to_string(),
            char_count: 10,
            is_current: true,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn batch24_note_switcher_pinned_trumps_current() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".to_string(),
            title: "Both".to_string(),
            char_count: 10,
            is_current: true,
            is_pinned: true,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn batch24_note_switcher_regular_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".to_string(),
            title: "Regular".to_string(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    // ============================================================
    // 14. AI command bar: all 21 actions present
    // ============================================================

    #[test]
    fn batch24_ai_command_bar_total_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    #[test]
    fn batch24_ai_command_bar_all_have_icons() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(a.icon.is_some(), "Action {} missing icon", a.id);
        }
    }

    #[test]
    fn batch24_ai_command_bar_all_have_sections() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(a.section.is_some(), "Action {} missing section", a.id);
        }
    }

    #[test]
    fn batch24_ai_command_bar_response_section_count() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(count, 3);
    }

    #[test]
    fn batch24_ai_command_bar_actions_section_count() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Actions"))
            .count();
        assert_eq!(count, 7);
    }

    // ============================================================
    // 15. AI command bar: specific shortcut and icon pairs
    // ============================================================

    #[test]
    fn batch24_ai_export_markdown_shortcut_icon() {
        let actions = get_ai_command_bar_actions();
        let export = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(export.shortcut.as_ref().unwrap(), "⇧⌘E");
        assert_eq!(export.icon, Some(IconName::FileCode));
    }

    #[test]
    fn batch24_ai_branch_from_last_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let branch = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert!(branch.shortcut.is_none());
        assert_eq!(branch.icon, Some(IconName::ArrowRight));
    }

    #[test]
    fn batch24_ai_change_model_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let model = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(model.shortcut.is_none());
        assert_eq!(model.icon, Some(IconName::Settings));
    }

    #[test]
    fn batch24_ai_toggle_shortcuts_help_shortcut() {
        let actions = get_ai_command_bar_actions();
        let help = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(help.shortcut.as_ref().unwrap(), "⌘/");
    }

    // ============================================================
    // 16. New chat actions: empty inputs
    // ============================================================

    #[test]
    fn batch24_new_chat_all_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn batch24_new_chat_only_last_used() {
        let last = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "Model 1".to_string(),
            provider: "p".to_string(),
            provider_display_name: "Provider".to_string(),
        }];
        let actions = get_new_chat_actions(&last, &[], &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn batch24_new_chat_only_presets() {
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
    fn batch24_new_chat_only_models() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "Model 1".to_string(),
            provider: "p".to_string(),
            provider_display_name: "Provider".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    #[test]
    fn batch24_new_chat_mixed() {
        let last = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "M1".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "g".to_string(),
            name: "G".to_string(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m2".to_string(),
            display_name: "M2".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let actions = get_new_chat_actions(&last, &presets, &models);
        assert_eq!(actions.len(), 3);
    }

    // ============================================================
    // 17. New chat actions: icon assignments
    // ============================================================

    #[test]
    fn batch24_new_chat_last_used_icon_bolt() {
        let last = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "M1".to_string(),
            provider: "p".to_string(),
            provider_display_name: "P".to_string(),
        }];
        let actions = get_new_chat_actions(&last, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    #[test]
    fn batch24_new_chat_model_icon_settings() {
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
    fn batch24_new_chat_preset_icon_preserved() {
        let presets = vec![NewChatPresetInfo {
            id: "g".to_string(),
            name: "General".to_string(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Code));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn batch24_new_chat_preset_no_description() {
        let presets = vec![NewChatPresetInfo {
            id: "g".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses General preset")
        );
    }

    // ============================================================
    // 18. Path context: exact action IDs in order for directory
    // ============================================================

    #[test]
    fn batch24_path_dir_action_ids_ordered() {
        let p = PathInfo::new("Documents", "/Users/test/Documents", true);
        let actions = get_path_context_actions(&p);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "file:open_directory",
                "file:copy_path",
                "file:open_in_finder",
                "file:open_in_quick_terminal",
                "file:open_in_editor",
                "file:copy_filename",
                "file:move_to_trash",
            ]
        );
    }

    #[test]
    fn batch24_path_file_action_ids_ordered() {
        let p = PathInfo::new("file.txt", "/Users/test/file.txt", false);
        let actions = get_path_context_actions(&p);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "file:select_file",
                "file:copy_path",
                "file:open_in_finder",
                "file:open_in_quick_terminal",
                "file:open_in_editor",
                "file:copy_filename",
                "file:move_to_trash",
            ]
        );
    }

    #[test]
    fn batch24_path_always_7_actions() {
        let dir = PathInfo::new("d", "/d", true);
        let file = PathInfo::new("f", "/f", false);
        assert_eq!(get_path_context_actions(&dir).len(), 7);
        assert_eq!(get_path_context_actions(&file).len(), 7);
    }

    // ============================================================
    // 19. Path context: shortcut assignments
    // ============================================================

    #[test]
    fn batch24_path_copy_path_shortcut() {
        let p = PathInfo::new("f", "/f", false);
        let actions = get_path_context_actions(&p);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.shortcut.as_ref().unwrap(), "⌘⇧C");
    }

    #[test]
    fn batch24_path_open_in_finder_shortcut() {
        let p = PathInfo::new("f", "/f", false);
        let actions = get_path_context_actions(&p);
        let f = actions
            .iter()
            .find(|a| a.id == "file:open_in_finder")
            .unwrap();
        assert_eq!(f.shortcut.as_ref().unwrap(), "⌘⇧F");
    }

    #[test]
    fn batch24_path_open_in_quick_terminal_shortcut() {
        let p = PathInfo::new("f", "/f", false);
        let actions = get_path_context_actions(&p);
        let t = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        // Quick Terminal intentionally ships without a shortcut.
        assert!(t.shortcut.is_none());
    }

    #[test]
    fn batch24_path_copy_filename_no_shortcut() {
        let p = PathInfo::new("f", "/f", false);
        let actions = get_path_context_actions(&p);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(cf.shortcut.is_none());
    }

    #[test]
    fn batch24_path_move_to_trash_shortcut() {
        let p = PathInfo::new("f", "/f", false);
        let actions = get_path_context_actions(&p);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert_eq!(trash.shortcut.as_ref().unwrap(), "⌘⌫");
    }

    // ============================================================
    // 20. File context: macOS action count difference
    // ============================================================

    #[cfg(target_os = "macos")]
    #[test]
    fn batch24_file_context_macos_file_count() {
        let f = FileInfo {
            path: "/test/f.txt".to_string(),
            name: "f.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&f);
        // open_file, reveal, rename, move, duplicate, open_in_editor, open_in_quick_terminal, attach_to_ai, quick_look, show_info, copy_path, copy_filename, move_to_trash = 13
        assert_eq!(actions.len(), 13);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch24_file_context_macos_dir_count() {
        let f = FileInfo {
            path: "/test/d".to_string(),
            name: "d".to_string(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&f);
        // open_directory, reveal, rename, move, duplicate, open_in_editor, open_in_quick_terminal, show_info, copy_path, copy_filename, move_to_trash = 11
        // (no quick_look or attach_to_ai for dirs)
        assert_eq!(actions.len(), 11);
    }

    // ============================================================
    // 21. to_deeplink_name: additional edge cases
    // ============================================================

    #[test]
    fn batch24_deeplink_numeric_only() {
        assert_eq!(to_deeplink_name("123"), "123");
    }

    #[test]
    fn batch24_deeplink_single_char() {
        assert_eq!(to_deeplink_name("a"), "a");
    }

    #[test]
    fn batch24_deeplink_all_special_empty() {
        assert_eq!(to_deeplink_name("!@#$%"), "_unnamed");
    }

    #[test]
    fn batch24_deeplink_mixed_unicode() {
        let result = to_deeplink_name("Café Script");
        assert!(result.contains("caf"));
        assert!(result.contains("script"));
    }

    #[test]
    fn batch24_deeplink_underscores_to_hyphens() {
        assert_eq!(to_deeplink_name("hello_world"), "hello-world");
    }

    // ============================================================
    // 22. format_shortcut_hint (dialog.rs version): alias coverage
    // ============================================================

    #[test]
    fn batch24_format_hint_command_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("command+c"), "⌘C");
    }

    #[test]
    fn batch24_format_hint_meta_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("meta+c"), "⌘C");
    }

    #[test]
    fn batch24_format_hint_super_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("super+c"), "⌘C");
    }

    #[test]
    fn batch24_format_hint_control_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("control+c"), "⌃C");
    }

    #[test]
    fn batch24_format_hint_opt_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("opt+c"), "⌥C");
    }

    #[test]
    fn batch24_format_hint_option_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("option+c"), "⌥C");
    }

    #[test]
    fn batch24_format_hint_return_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("cmd+return"), "⌘↵");
    }

    #[test]
    fn batch24_format_hint_esc_alias() {
        assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
    }

    // ============================================================
    // 23. parse_shortcut_keycaps: modifiers and special keys
    // ============================================================

    #[test]
    fn batch24_keycaps_single_modifier() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘");
        assert_eq!(caps, vec!["⌘"]);
    }

    #[test]
    fn batch24_keycaps_modifier_and_letter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘C");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    #[test]
    fn batch24_keycaps_all_modifiers() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌃⌥⇧⌘C");
        assert_eq!(caps, vec!["⌃", "⌥", "⇧", "⌘", "C"]);
    }

    #[test]
    fn batch24_keycaps_arrows() {
        let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
        assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
    }

    #[test]
    fn batch24_keycaps_lowercase_uppercased() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘e");
        assert_eq!(caps, vec!["⌘", "E"]);
    }

    // ============================================================
    // 24. score_action: scoring tiers with cached lowercase
    // ============================================================

    #[test]
    fn batch24_score_prefix_match() {
        let action = Action::new(
            "id",
            "Edit Script",
            Some("Open editor".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(score >= 100);
    }

    #[test]
    fn batch24_score_contains_match() {
        let action = Action::new("id", "Copy Edit Path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(score >= 50);
        assert!(score < 100);
    }

    #[test]
    fn batch24_score_no_match() {
        let action = Action::new("id", "Copy Path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "xyz");
        assert_eq!(score, 0);
    }

    #[test]
    fn batch24_score_description_bonus() {
        let action = Action::new(
            "id",
            "Open File",
            Some("Edit in editor".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "editor");
        assert!(score >= 15);
    }

    #[test]
    fn batch24_score_shortcut_bonus() {
        let action =
            Action::new("id", "Open File", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "⌘e");
        assert!(score >= 10);
    }

    // ============================================================
    // 25. fuzzy_match: edge cases
    // ============================================================

    #[test]
    fn batch24_fuzzy_exact() {
        assert!(ActionsDialog::fuzzy_match("script:edit", "script:edit"));
    }

    #[test]
    fn batch24_fuzzy_subsequence() {
        assert!(ActionsDialog::fuzzy_match("edit script", "esc"));
    }

    #[test]
    fn batch24_fuzzy_no_match() {
        assert!(!ActionsDialog::fuzzy_match("abc", "xyz"));
    }

    #[test]
    fn batch24_fuzzy_empty_needle() {
        assert!(ActionsDialog::fuzzy_match("abc", ""));
    }

    #[test]
    fn batch24_fuzzy_needle_longer() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
    }

    // ============================================================
    // 26. build_grouped_items_static: section style effects
    // ============================================================

    #[test]
    fn batch24_grouped_headers_adds_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // S1 header, A item, S2 header, B item = 4
        assert_eq!(grouped.len(), 4);
    }

    #[test]
    fn batch24_grouped_same_section_one_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S1"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // S1 header, A, B = 3
        assert_eq!(grouped.len(), 3);
    }

    #[test]
    fn batch24_grouped_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Just items, no headers
        assert_eq!(grouped.len(), 2);
    }

    #[test]
    fn batch24_grouped_empty_filtered() {
        let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // ============================================================
    // 27. coerce_action_selection: header skipping
    // ============================================================

    #[test]
    fn batch24_coerce_on_item_stays() {
        let rows = vec![
            GroupedActionItem::SectionHeader("S".to_string()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(1));
    }

    #[test]
    fn batch24_coerce_header_skips_down() {
        let rows = vec![
            GroupedActionItem::SectionHeader("S".to_string()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn batch24_coerce_trailing_header_searches_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("S".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn batch24_coerce_all_headers_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".to_string()),
            GroupedActionItem::SectionHeader("B".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn batch24_coerce_empty_none() {
        let rows: Vec<GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ============================================================
    // 28. CommandBarConfig preset field values
    // ============================================================

    #[test]
    fn batch24_cmdbar_default_close_flags() {
        let config = CommandBarConfig::default();
        assert!(config.close_on_select);
        assert!(config.close_on_escape);
        assert!(config.close_on_click_outside);
    }

    #[test]
    fn batch24_cmdbar_ai_style_search_top() {
        let config = CommandBarConfig::ai_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn batch24_cmdbar_main_menu_search_top() {
        let config = CommandBarConfig::main_menu_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    // --- merged from part_04.rs ---

    #[test]
    fn batch24_cmdbar_no_search_hidden() {
        let config = CommandBarConfig::no_search();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
    }

    #[test]
    fn batch24_cmdbar_notes_style_separators() {
        let config = CommandBarConfig::notes_style();
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    // ============================================================
    // 29. Action builder: defaults and chaining
    // ============================================================

    #[test]
    fn batch24_action_default_has_action_false() {
        let a = Action::new("id", "Title", None, ActionCategory::ScriptContext);
        assert!(!a.has_action);
    }

    #[test]
    fn batch24_action_default_value_none() {
        let a = Action::new("id", "Title", None, ActionCategory::ScriptContext);
        assert!(a.value.is_none());
    }

    #[test]
    fn batch24_action_default_icon_none() {
        let a = Action::new("id", "Title", None, ActionCategory::ScriptContext);
        assert!(a.icon.is_none());
    }

    #[test]
    fn batch24_action_default_section_none() {
        let a = Action::new("id", "Title", None, ActionCategory::ScriptContext);
        assert!(a.section.is_none());
    }

    #[test]
    fn batch24_action_chain_preserves_all() {
        let a = Action::new(
            "id",
            "Title",
            Some("Desc".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘C")
        .with_icon(IconName::Copy)
        .with_section("Section");
        assert_eq!(a.shortcut.as_deref(), Some("⌘C"));
        assert_eq!(a.icon, Some(IconName::Copy));
        assert_eq!(a.section.as_deref(), Some("Section"));
        assert_eq!(a.description.as_deref(), Some("Desc"));
    }

    // ============================================================
    // 30. Cross-context: all actions have ScriptContext category
    // ============================================================

    #[test]
    fn batch24_cross_script_all_script_context() {
        let script = ScriptInfo::new("test", "/path");
        for a in get_script_context_actions(&script) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn batch24_cross_clipboard_all_script_context() {
        let entry = ClipboardEntryInfo {
            id: "1".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for a in get_clipboard_history_context_actions(&entry) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn batch24_cross_ai_all_script_context() {
        for a in get_ai_command_bar_actions() {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn batch24_cross_path_all_script_context() {
        let p = PathInfo::new("f", "/f", false);
        for a in get_path_context_actions(&p) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn batch24_cross_file_all_script_context() {
        let f = FileInfo {
            path: "/f.txt".to_string(),
            name: "f.txt".to_string(),
            file_type: FileType::File,
            is_dir: false,
        };
        for a in get_file_context_actions(&f) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_25 {
    //! Purged batch 25 validation tests.
    //!
    //! This file previously contained a large generated batch of micro-tests with
    //! heavy overlap with canonical dialog/action coverage in:
    //! - `src/actions/dialog_tests.rs`
    //! - `src/actions/dialog_window_tests.rs`
    //! - `src/actions/dialog_builtin_action_validation_tests_3.rs`
    //!
    //! The removed cases were largely duplicate/trivial assertions and provided
    //! low signal for maintenance cost.
}

mod from_dialog_builtin_action_validation_tests_26 {
    // --- merged from part_01.rs ---
    //! Batch 26 – Builtin action validation tests
    //!
    //! 30 categories · ~120 tests
    //! Focus areas:
    //!   - Scriptlet context with_custom: action ordering with multiple custom actions
    //!   - Clipboard share and attach_to_ai details
    //!   - Notes full mode section assignments
    //!   - Chat context edge: two models same provider
    //!   - AI command bar: Submit action details
    //!   - Path vs file context: description wording differences
    //!   - ScriptInfo constructor: is_agent mutually exclusive with is_script/is_scriptlet
    //!   - format_shortcut_hint: multi-key combos with numbers
    //!   - to_deeplink_name: Unicode edge cases
    //!   - Cross-context: description is always Some for built-in actions

    use super::builders::*;
    use super::command_bar::CommandBarConfig;
    use super::dialog::{GroupedActionItem, build_grouped_items_static, coerce_action_selection};
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // ─────────────────────────────────────────────
    // 1. Scriptlet context with_custom: three custom actions maintain order
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_01_three_custom_actions_maintain_insertion_order() {
        let script = ScriptInfo::scriptlet("Multi Act", "/p.md", None, None);
        let mut sl = Scriptlet::new("Multi Act".into(), "bash".into(), "echo hi".into());
        sl.actions = vec![
            ScriptletAction {
                name: "Alpha".into(),
                command: "alpha".into(),
                tool: "bash".into(),
                code: "a".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
            ScriptletAction {
                name: "Beta".into(),
                command: "beta".into(),
                tool: "bash".into(),
                code: "b".into(),
                inputs: vec![],
                shortcut: Some("cmd+b".into()),
                description: Some("Do beta".into()),
            },
            ScriptletAction {
                name: "Gamma".into(),
                command: "gamma".into(),
                tool: "bash".into(),
                code: "g".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            },
        ];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&sl));
        let custom_ids: Vec<&str> = actions
            .iter()
            .filter(|a| a.id.starts_with("scriptlet_action:"))
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(
            custom_ids,
            vec![
                "scriptlet_action:alpha",
                "scriptlet_action:beta",
                "scriptlet_action:gamma"
            ]
        );
    }

    #[test]
    fn cat26_01_custom_actions_all_have_has_action_true() {
        let script = ScriptInfo::scriptlet("X", "/x.md", None, None);
        let mut sl = Scriptlet::new("X".into(), "bash".into(), "echo".into());
        sl.actions = vec![ScriptletAction {
            name: "Do".into(),
            command: "do-it".into(),
            tool: "bash".into(),
            code: "d".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&sl));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:do-it")
            .unwrap();
        assert!(custom.has_action);
        assert_eq!(custom.value, Some("do-it".into()));
    }

    #[test]
    fn cat26_01_custom_action_with_shortcut_gets_formatted() {
        let script = ScriptInfo::scriptlet("S", "/s.md", None, None);
        let mut sl = Scriptlet::new("S".into(), "bash".into(), "e".into());
        sl.actions = vec![ScriptletAction {
            name: "Copy".into(),
            command: "cp".into(),
            tool: "bash".into(),
            code: "c".into(),
            inputs: vec![],
            shortcut: Some("cmd+shift+c".into()),
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&sl));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:cp")
            .unwrap();
        assert_eq!(custom.shortcut, Some("⌘⇧C".into()));
    }

    #[test]
    fn cat26_01_custom_actions_appear_after_run_before_shortcut_actions() {
        let script = ScriptInfo::scriptlet("T", "/t.md", None, None);
        let mut sl = Scriptlet::new("T".into(), "bash".into(), "echo".into());
        sl.actions = vec![ScriptletAction {
            name: "My Act".into(),
            command: "my-act".into(),
            tool: "bash".into(),
            code: "x".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&sl));
        let run_idx = actions.iter().position(|a| a.id == "run_script").unwrap();
        let custom_idx = actions
            .iter()
            .position(|a| a.id == "scriptlet_action:my-act")
            .unwrap();
        let shortcut_idx = actions.iter().position(|a| a.id == "add_shortcut").unwrap();
        assert!(run_idx < custom_idx);
        assert!(custom_idx < shortcut_idx);
    }

    // ─────────────────────────────────────────────
    // 2. Clipboard share shortcut and description
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_02_clipboard_share_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hi".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert_eq!(share.shortcut.as_deref(), Some("⇧⌘E"));
        assert_eq!(share.title, "Share...");
    }

    #[test]
    fn cat26_02_clipboard_attach_to_ai_description_mentions_ai() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hi".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let attach = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert!(
            attach
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent chat")
        );
    }

    #[test]
    fn cat26_02_clipboard_share_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "img".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Image".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_share"));
    }

    #[test]
    fn cat26_02_clipboard_attach_to_ai_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "img".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Img".into(),
            image_dimensions: Some((50, 50)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_attach_to_ai")
        );
    }

    // ─────────────────────────────────────────────
    // 3. Notes full mode: section assignments via icons
    // ─────────────────────────────────────────────

    // ─────────────────────────────────────────────
    // 4. Chat context: two models same provider
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_04_chat_two_models_same_provider_both_listed() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![
                ChatModelInfo {
                    id: "gpt-4".into(),
                    display_name: "GPT-4".into(),
                    provider: "OpenAI".into(),
                },
                ChatModelInfo {
                    id: "gpt-3.5".into(),
                    display_name: "GPT-3.5".into(),
                    provider: "OpenAI".into(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        assert!(picker.iter().any(|a| a.id == "chat:select_model_gpt-4"));
        assert!(picker.iter().any(|a| a.id == "chat:select_model_gpt-3.5"));
    }

    #[test]
    fn cat26_04_chat_current_model_has_checkmark() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let model_action = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-4")
            .unwrap();
        assert!(model_action.title.contains('✓'));
    }

    #[test]
    fn cat26_04_chat_non_current_model_no_checkmark() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt-3.5".into(),
                display_name: "GPT-3.5".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let model_action = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-3.5")
            .unwrap();
        assert!(!model_action.title.contains('✓'));
    }

    #[test]
    fn cat26_04_chat_model_description_via_provider() {
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
        let model_action = picker
            .iter()
            .find(|a| a.id == "chat:select_model_claude")
            .unwrap();
        assert_eq!(model_action.description.as_deref(), Some("Uses Anthropic"));
    }

    // ─────────────────────────────────────────────
    // 5. AI command bar: submit action details
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_05_ai_submit_action_shortcut() {
        let actions = get_ai_command_bar_actions();
        let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(submit.shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn cat26_05_ai_submit_action_icon() {
        let actions = get_ai_command_bar_actions();
        let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(submit.icon, Some(IconName::ArrowUp));
    }

    #[test]
    fn cat26_05_ai_submit_action_section() {
        let actions = get_ai_command_bar_actions();
        let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(submit.section.as_deref(), Some("Actions"));
    }

    #[test]
    fn cat26_05_ai_new_chat_action_icon_plus() {
        let actions = get_ai_command_bar_actions();
        let new_chat = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert_eq!(new_chat.icon, Some(IconName::Plus));
        assert_eq!(new_chat.shortcut.as_deref(), Some("⌘N"));
    }

    #[test]
    fn cat26_05_ai_delete_chat_action_icon_trash() {
        let actions = get_ai_command_bar_actions();
        let delete = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert_eq!(delete.icon, Some(IconName::Trash));
        assert_eq!(delete.shortcut.as_deref(), Some("⌘⌫"));
    }

    // ─────────────────────────────────────────────
    // 6. Path vs file context: description wording differences
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_06_file_context_open_desc_says_default_application() {
        let info = FileInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.description.as_ref().unwrap().contains("default app"));
    }

    #[test]
    fn cat26_06_path_context_file_desc_says_submit() {
        let info = PathInfo {
            name: "f.txt".into(),
            path: "/f.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        let sel = actions.iter().find(|a| a.id == "file:select_file").unwrap();
        assert!(sel.description.as_ref().unwrap().contains("Selects"));
    }

    #[test]
    fn cat26_06_file_dir_desc_says_folder() {
        let info = FileInfo {
            path: "/d".into(),
            name: "d".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&info);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(open.description.as_ref().unwrap().contains("folder"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn cat26_06_path_dir_desc_says_navigate() {
        let info = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&info);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .contains("Opens this directory")
        );
    }

    // ─────────────────────────────────────────────
    // 7. ScriptInfo: is_agent mutual exclusivity
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_07_script_info_new_is_not_agent() {
        let s = ScriptInfo::new("x", "/x.ts");
        assert!(!s.is_agent);
        assert!(s.is_script);
        assert!(!s.is_scriptlet);
    }

    #[test]
    fn cat26_07_script_info_builtin_is_not_agent() {
        let b = ScriptInfo::builtin("Clip");
        assert!(!b.is_agent);
        assert!(!b.is_script);
        assert!(!b.is_scriptlet);
    }

    #[test]
    fn cat26_07_script_info_scriptlet_is_not_agent() {
        let s = ScriptInfo::scriptlet("S", "/s.md", None, None);
        assert!(!s.is_agent);
        assert!(!s.is_script);
        assert!(s.is_scriptlet);
    }

    #[test]
    fn cat26_07_script_info_with_action_verb_defaults_no_agent() {
        let s = ScriptInfo::with_action_verb("App", "/a.app", false, "Launch");
        assert!(!s.is_agent);
    }

    // ─────────────────────────────────────────────
    // 8. format_shortcut_hint: multi-key combos with numbers
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_08_format_hint_cmd_1() {
        let result = super::ActionsDialog::format_shortcut_hint("cmd+1");
        assert_eq!(result, "⌘1");
    }

    #[test]
    fn cat26_08_format_hint_ctrl_shift_3() {
        let result = super::ActionsDialog::format_shortcut_hint("ctrl+shift+3");
        assert_eq!(result, "⌃⇧3");
    }

    #[test]
    fn cat26_08_format_hint_alt_f4() {
        let result = super::ActionsDialog::format_shortcut_hint("alt+f4");
        assert_eq!(result, "⌥F4");
    }

    #[test]
    fn cat26_08_format_hint_command_alias() {
        let result = super::ActionsDialog::format_shortcut_hint("command+k");
        assert_eq!(result, "⌘K");
    }

    #[test]
    fn cat26_08_format_hint_option_alias() {
        let result = super::ActionsDialog::format_shortcut_hint("option+delete");
        assert_eq!(result, "⌥⌫");
    }

    // ─────────────────────────────────────────────
    // 9. to_deeplink_name: Unicode with mixed scripts
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_09_deeplink_preserves_cjk() {
        let result = to_deeplink_name("日本語スクリプト");
        assert!(result.contains("%E6%97%A5%E6%9C%AC"));
    }

    #[test]
    fn cat26_09_deeplink_preserves_accented() {
        let result = to_deeplink_name("café résumé");
        assert!(result.contains("caf%C3%A9"));
        assert!(result.contains("r%C3%A9sum%C3%A9"));
    }

    #[test]
    fn cat26_09_deeplink_mixed_alpha_special_unicode() {
        let result = to_deeplink_name("Hello 世界!");
        assert_eq!(result, "hello-%E4%B8%96%E7%95%8C");
    }

    #[test]
    fn cat26_09_deeplink_emoji_stripped() {
        // Emojis are alphanumeric in Unicode, so they should be preserved
        let result = to_deeplink_name("🚀 Launch");
        // Rocket emoji is not alphanumeric (it's a symbol), so it becomes a hyphen
        // "Launch" becomes "launch"
        assert!(result.contains("launch"));
    }

    // ─────────────────────────────────────────────
    // 10. Cross-context: all built-in actions have Some description
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_10_script_actions_all_have_description() {
        let s = ScriptInfo::new("x", "/x.ts");
        let actions = get_script_context_actions(&s);
        for a in &actions {
            assert!(
                a.description.is_some(),
                "Action '{}' should have a description",
                a.id
            );
        }
    }

    #[test]
    fn cat26_10_clipboard_text_actions_all_have_description() {
        let entry = ClipboardEntryInfo {
            id: "t".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for a in &actions {
            assert!(
                a.description.is_some(),
                "Clipboard action '{}' should have a description",
                a.id
            );
        }
    }

    #[test]
    fn cat26_10_path_actions_all_have_description() {
        let info = PathInfo {
            name: "p".into(),
            path: "/p".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        for a in &actions {
            assert!(
                a.description.is_some(),
                "Path action '{}' should have a description",
                a.id
            );
        }
    }

    #[test]
    fn cat26_10_ai_actions_all_have_description() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(
                a.description.is_some(),
                "AI action '{}' should have a description",
                a.id
            );
        }
    }

    // ─────────────────────────────────────────────
    // 11. Clipboard: pin/unpin title and description content
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_11_clipboard_pin_title_says_pin_entry() {
        let entry = ClipboardEntryInfo {
            id: "u".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap();
        assert_eq!(pin.title, "Pin Entry");
    }

    #[test]
    fn cat26_11_clipboard_unpin_title_says_unpin_entry() {
        let entry = ClipboardEntryInfo {
            id: "p".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let unpin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_unpin")
            .unwrap();
        assert_eq!(unpin.title, "Unpin Entry");
    }

    #[test]
    fn cat26_11_clipboard_pin_desc_mentions_prevent() {
        let entry = ClipboardEntryInfo {
            id: "u".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap();
        assert!(
            pin.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("pin")
        );
    }

    // ─────────────────────────────────────────────
    // 12. Note switcher: multiple notes with diverse states
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_12_note_switcher_three_notes_three_items() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "a".into(),
                title: "Note A".into(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: "Hello".into(),
                relative_time: "1m ago".into(),
            },
            NoteSwitcherNoteInfo {
                id: "b".into(),
                title: "Note B".into(),
                char_count: 20,
                is_current: false,
                is_pinned: true,
                preview: "World".into(),
                relative_time: "5m ago".into(),
            },
            NoteSwitcherNoteInfo {
                id: "c".into(),
                title: "Note C".into(),
                char_count: 5,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "1h ago".into(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn cat26_12_note_switcher_pinned_note_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "b".into(),
            title: "Note B".into(),
            char_count: 20,
            is_current: false,
            is_pinned: true,
            preview: "World".into(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn cat26_12_note_switcher_unpinned_note_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "c".into(),
            title: "Note C".into(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Recent"));
    }

    #[test]
    fn cat26_12_note_switcher_empty_preview_with_time_shows_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "c".into(),
            title: "Note C".into(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("1h ago"));
    }

    // ─────────────────────────────────────────────
    // 13. New chat: last_used section and icon
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_13_new_chat_last_used_has_bolt_filled_icon() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "P".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    #[test]
    fn cat26_13_new_chat_last_used_section_name() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "P".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn cat26_13_new_chat_last_used_description_is_provider_display_name() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "P".into(),
            provider_display_name: "My Provider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].description.as_deref(), Some("Uses My Provider"));
    }

    #[test]
    fn cat26_13_new_chat_model_section_name() {
        let models = vec![NewChatModelInfo {
            model_id: "m2".into(),
            display_name: "Model 2".into(),
            provider: "P".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    // ─────────────────────────────────────────────
    // 14. CommandBarConfig: close flags default to true
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_14_command_bar_config_default_close_on_select() {
        let config = CommandBarConfig::default();
        assert!(config.close_on_select);
    }

    #[test]
    fn cat26_14_command_bar_config_default_close_on_click_outside() {
        let config = CommandBarConfig::default();
        assert!(config.close_on_click_outside);
    }

    #[test]
    fn cat26_14_command_bar_config_default_close_on_escape() {
        let config = CommandBarConfig::default();
        assert!(config.close_on_escape);
    }

    #[test]
    fn cat26_14_command_bar_config_ai_style_preserves_close_flags() {
        let config = CommandBarConfig::ai_style();
        assert!(config.close_on_select);
        assert!(config.close_on_escape);
    }

    // ─────────────────────────────────────────────
    // 15. Script context: edit shortcut is ⌘E for all editable types
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_15_script_edit_shortcut() {
        let s = ScriptInfo::new("s", "/s.ts");
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn cat26_15_scriptlet_edit_shortcut() {
        let s = ScriptInfo::scriptlet("s", "/s.md", None, None);
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
        assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn cat26_15_agent_edit_shortcut() {
        let mut s = ScriptInfo::new("a", "/a.md");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn cat26_15_agent_edit_title_says_agent() {
        let mut s = ScriptInfo::new("a", "/a.md");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.title.contains("Agent"));
    }

    // ─────────────────────────────────────────────
    // 16. Script context: view_logs only for is_script=true
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_16_script_has_view_logs() {
        let s = ScriptInfo::new("s", "/s.ts");
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "view_logs"));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn cat26_16_builtin_no_view_logs() {
        let b = ScriptInfo::builtin("B");
        let actions = get_script_context_actions(&b);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn cat26_16_scriptlet_no_view_logs() {
        let s = ScriptInfo::scriptlet("S", "/s.md", None, None);
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn cat26_16_agent_no_view_logs() {
        let mut s = ScriptInfo::new("a", "/a.md");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // ─────────────────────────────────────────────
    // 17. Script context: copy_deeplink always present
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_17_script_has_copy_deeplink() {
        let s = ScriptInfo::new("s", "/s.ts");
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "copy_deeplink"));
    }

    #[test]
    fn cat26_17_builtin_has_copy_deeplink() {
        let b = ScriptInfo::builtin("B");
        let actions = get_script_context_actions(&b);
        assert!(actions.iter().any(|a| a.id == "copy_deeplink"));
    }

    #[test]
    fn cat26_17_scriptlet_has_copy_deeplink() {
        let s = ScriptInfo::scriptlet("S", "/s.md", None, None);
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "copy_deeplink"));
    }

    // ─────────────────────────────────────────────
    // 18. File context: reveal_in_finder always present
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_18_file_reveal_always_present_file() {
        let info = FileInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
    }

    #[test]
    fn cat26_18_file_reveal_always_present_dir() {
        let info = FileInfo {
            path: "/d".into(),
            name: "d".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
    }

    #[test]
    fn cat26_18_file_reveal_shortcut() {
        let info = FileInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        let reveal = actions
            .iter()
            .find(|a| a.id == "file:reveal_in_finder")
            .unwrap();
        assert_eq!(reveal.shortcut.as_deref(), Some("⌘⇧F"));
    }

    // ─────────────────────────────────────────────
    // 19. Path context: open_in_quick_terminal and open_in_editor always present
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_19_path_has_open_in_quick_terminal_for_dir() {
        let info = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&info);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "file:open_in_quick_terminal")
        );
    }

    #[test]
    fn cat26_19_path_has_open_in_quick_terminal_for_file() {
        let info = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "file:open_in_quick_terminal")
        );
    }

    #[test]
    fn cat26_19_path_has_open_in_editor_for_file() {
        let info = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
    }

    #[test]
    fn cat26_19_path_open_in_editor_shortcut() {
        let info = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        let editor = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert_eq!(editor.shortcut.as_deref(), Some("⌘E"));
    }

    // ─────────────────────────────────────────────
    // 20. build_grouped_items_static: empty actions list
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_20_build_grouped_empty_actions_empty_result() {
        let result = build_grouped_items_static(&[], &[], SectionStyle::Headers);
        assert!(result.is_empty());
    }

    #[test]
    fn cat26_20_build_grouped_no_filtered_indices() {
        let actions = vec![Action::new(
            "a",
            "Action",
            Some("desc".into()),
            ActionCategory::ScriptContext,
        )];
        let result = build_grouped_items_static(&actions, &[], SectionStyle::Headers);
        assert!(result.is_empty());
    }

    #[test]
    fn cat26_20_build_grouped_single_action_no_section_no_header() {
        let actions = vec![Action::new(
            "a",
            "Action",
            Some("desc".into()),
            ActionCategory::ScriptContext,
        )];
        let result = build_grouped_items_static(&actions, &[0], SectionStyle::Headers);
        // No section on action, so no header added
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], GroupedActionItem::Item(0)));
    }

    #[test]
    fn cat26_20_build_grouped_single_action_with_section_has_header() {
        let actions = vec![
            Action::new(
                "a",
                "Action",
                Some("desc".into()),
                ActionCategory::ScriptContext,
            )
            .with_section("MySection"),
        ];
        let result = build_grouped_items_static(&actions, &[0], SectionStyle::Headers);
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0], GroupedActionItem::SectionHeader(s) if s == "MySection"));
        assert!(matches!(result[1], GroupedActionItem::Item(0)));
    }

    // ─────────────────────────────────────────────
    // 21. coerce_action_selection: mixed header/item patterns
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_21_coerce_item_header_item_on_header_goes_down() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("S".into()),
            GroupedActionItem::Item(1),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(2));
    }

    #[test]
    fn cat26_21_coerce_header_item_on_header_goes_down() {
        let rows = vec![
            GroupedActionItem::SectionHeader("S".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn cat26_21_coerce_item_header_on_header_goes_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("S".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn cat26_21_coerce_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ─────────────────────────────────────────────
    // 22. Action: title_lower and description_lower caching
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_22_action_title_lower_precomputed() {
        let a = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "hello world");
    }

    #[test]
    fn cat26_22_action_description_lower_precomputed() {
        let a = Action::new(
            "id",
            "T",
            Some("My Description".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(a.description_lower.as_deref(), Some("my description"));
    }

    #[test]
    fn cat26_22_action_shortcut_lower_set_by_with_shortcut() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
        assert_eq!(a.shortcut_lower.as_deref(), Some("⌘⇧c"));
    }

    #[test]
    fn cat26_22_action_no_shortcut_lower_is_none() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(a.shortcut_lower.is_none());
    }

    // ─────────────────────────────────────────────
    // 23. score_action: combined bonus stacking variations
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_23_score_prefix_match_at_least_100() {
        let a = Action::new("id", "Copy Path", None, ActionCategory::ScriptContext);
        let score = super::ActionsDialog::score_action(&a, "copy");
        assert!(score >= 100);
    }

    #[test]
    fn cat26_23_score_contains_match_50_to_99() {
        let a = Action::new("id", "My Copy Path", None, ActionCategory::ScriptContext);
        let score = super::ActionsDialog::score_action(&a, "copy");
        assert!(score >= 50);
        // It's a contains match not a prefix match
        assert!(score < 100 || a.title_lower.starts_with("copy"));
    }

    #[test]
    fn cat26_23_score_no_match_zero() {
        let a = Action::new("id", "Delete", None, ActionCategory::ScriptContext);
        let score = super::ActionsDialog::score_action(&a, "xyz");
        assert_eq!(score, 0);
    }

    #[test]
    fn cat26_23_score_empty_search_is_prefix() {
        let a = Action::new("id", "Anything", None, ActionCategory::ScriptContext);
        let score = super::ActionsDialog::score_action(&a, "");
        assert!(score >= 100, "Empty search should match as prefix");
    }

    // ─────────────────────────────────────────────
    // 24. fuzzy_match: various patterns
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_24_fuzzy_exact_match() {
        assert!(super::ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn cat26_24_fuzzy_subsequence_match() {
        assert!(super::ActionsDialog::fuzzy_match("hello world", "hlo"));
    }

    #[test]
    fn cat26_24_fuzzy_no_match() {
        assert!(!super::ActionsDialog::fuzzy_match("abc", "abd"));
    }

    #[test]
    fn cat26_24_fuzzy_empty_needle_matches() {
        assert!(super::ActionsDialog::fuzzy_match("anything", ""));
    }

    // ─────────────────────────────────────────────
    // 25. Clipboard: paste description mentions clipboard
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_25_paste_desc_mentions_clipboard() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hi".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert!(
            paste
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    #[test]
    fn cat26_25_copy_desc_mentions_clipboard() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hi".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let copy = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_copy")
            .unwrap();
        assert!(
            copy.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    #[test]
    fn cat26_25_paste_keep_open_desc_mentions_keep() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hi".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let keep = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste_keep_open")
            .unwrap();
        assert!(
            keep.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("keep")
        );
    }

    // ─────────────────────────────────────────────
    // 26. Script context: shortcut toggle (add vs update/remove)
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_26_no_shortcut_shows_add_shortcut() {
        let s = ScriptInfo::new("s", "/s.ts");
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "add_shortcut"));
        assert!(!actions.iter().any(|a| a.id == "update_shortcut"));
        assert!(!actions.iter().any(|a| a.id == "remove_shortcut"));
    }

    #[test]
    fn cat26_26_with_shortcut_shows_update_and_remove() {
        let s = ScriptInfo::with_shortcut("s", "/s.ts", Some("cmd+s".into()));
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
        assert!(actions.iter().any(|a| a.id == "update_shortcut"));
        assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
    }

    #[test]
    fn cat26_26_no_alias_shows_add_alias() {
        let s = ScriptInfo::new("s", "/s.ts");
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "add_alias"));
        assert!(!actions.iter().any(|a| a.id == "update_alias"));
        assert!(!actions.iter().any(|a| a.id == "remove_alias"));
    }

    #[test]
    fn cat26_26_with_alias_shows_update_and_remove() {
        let s = ScriptInfo::with_shortcut_and_alias("s", "/s.ts", None, Some("al".into()));
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "add_alias"));
        assert!(actions.iter().any(|a| a.id == "update_alias"));
        assert!(actions.iter().any(|a| a.id == "remove_alias"));
    }

    // ─────────────────────────────────────────────
    // 27. Notes command bar: find_in_note section and icon
    // ─────────────────────────────────────────────

    // --- merged from part_04.rs ---

    // ─────────────────────────────────────────────
    // 28. AI command bar: export_markdown details
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_28_ai_export_markdown_shortcut() {
        let actions = get_ai_command_bar_actions();
        let export = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(export.shortcut.as_deref(), Some("⇧⌘E"));
    }

    #[test]
    fn cat26_28_ai_export_markdown_icon() {
        let actions = get_ai_command_bar_actions();
        let export = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(export.icon, Some(IconName::FileCode));
    }

    #[test]
    fn cat26_28_ai_export_markdown_section() {
        let actions = get_ai_command_bar_actions();
        let export = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(export.section.as_deref(), Some("Export"));
    }

    #[test]
    fn cat26_28_ai_export_desc_mentions_markdown() {
        let actions = get_ai_command_bar_actions();
        let export = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert!(
            export
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("markdown")
        );
    }

    // ─────────────────────────────────────────────
    // 29. parse_shortcut_keycaps: various inputs
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_29_parse_keycaps_cmd_c() {
        let caps = super::ActionsDialog::parse_shortcut_keycaps("⌘C");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    #[test]
    fn cat26_29_parse_keycaps_modifier_only() {
        let caps = super::ActionsDialog::parse_shortcut_keycaps("⌘");
        assert_eq!(caps, vec!["⌘"]);
    }

    #[test]
    fn cat26_29_parse_keycaps_enter() {
        let caps = super::ActionsDialog::parse_shortcut_keycaps("↵");
        assert_eq!(caps, vec!["↵"]);
    }

    #[test]
    fn cat26_29_parse_keycaps_all_modifiers_and_key() {
        let caps = super::ActionsDialog::parse_shortcut_keycaps("⌃⌥⇧⌘K");
        assert_eq!(caps, vec!["⌃", "⌥", "⇧", "⌘", "K"]);
    }

    // ─────────────────────────────────────────────
    // 30. Cross-context: action count comparison across types
    // ─────────────────────────────────────────────

    #[test]
    fn cat26_30_script_more_actions_than_builtin() {
        let script = ScriptInfo::new("s", "/s.ts");
        let builtin = ScriptInfo::builtin("B");
        let script_actions = get_script_context_actions(&script);
        let builtin_actions = get_script_context_actions(&builtin);
        assert!(script_actions.len() > builtin_actions.len());
    }

    #[test]
    fn cat26_30_scriptlet_more_actions_than_builtin() {
        let scriptlet = ScriptInfo::scriptlet("S", "/s.md", None, None);
        let builtin = ScriptInfo::builtin("B");
        let scriptlet_actions = get_script_context_actions(&scriptlet);
        let builtin_actions = get_script_context_actions(&builtin);
        assert!(scriptlet_actions.len() > builtin_actions.len());
    }

    #[test]
    fn cat26_30_builtin_exactly_4_actions() {
        let b = ScriptInfo::builtin("B");
        let actions = get_script_context_actions(&b);
        assert_eq!(actions.len(), 5); // run, toggle_info, add_shortcut, add_alias, copy_deeplink
    }

    #[test]
    fn cat26_30_script_exactly_13_actions() {
        let s = ScriptInfo::new("s", "/s.ts");
        let actions = get_script_context_actions(&s);
        // run_script, toggle_info, add_shortcut, add_alias, toggle_favorite, edit_script,
        // view_logs, reveal_in_finder, file:open_in_quick_terminal, copy_path, copy_content,
        // copy_deeplink, delete_script
        assert_eq!(actions.len(), 13);
    }

    #[test]
    fn cat26_30_scriptlet_exactly_11_actions() {
        let s = ScriptInfo::scriptlet("S", "/s.md", None, None);
        let actions = get_script_context_actions(&s);
        // run_script, toggle_info, add_shortcut, add_alias, toggle_favorite, edit_scriptlet,
        // reveal_scriptlet_in_finder, file:open_in_quick_terminal, copy_scriptlet_path,
        // copy_content, copy_deeplink
        assert_eq!(actions.len(), 11);
    }

    #[test]
    fn cat26_30_agent_more_actions_than_builtin() {
        let mut a = ScriptInfo::new("a", "/a.md");
        a.is_script = false;
        a.is_agent = true;
        let b = ScriptInfo::builtin("B");
        let agent_actions = get_script_context_actions(&a);
        let builtin_actions = get_script_context_actions(&b);
        assert!(agent_actions.len() > builtin_actions.len());
    }
}

mod from_dialog_builtin_action_validation_tests_27 {
    // --- merged from part_01.rs ---
    //! Batch 27 – Builtin action validation tests
    //!
    //! 30 categories · ~119 tests
    //! Focus areas:
    //!   - Clipboard: frontmost_app_name dynamic paste title
    //!   - Scriptlet context: edit_scriptlet shortcut and desc vs script edit_script
    //!   - Script context: agent action count and ordering invariants
    //!   - Notes command bar: conditional auto_sizing action presence
    //!   - AI command bar: paste_image shortcut and section
    //!   - Chat context: zero models, response+messages combo action counts
    //!   - New chat: model_idx ID pattern and section assignments
    //!   - Note switcher: empty preview+empty time falls back to char count
    //!   - File context: copy_filename vs copy_path shortcut difference
    //!   - Path context: copy_filename has no shortcut (unlike file context)
    //!   - format_shortcut_hint (dialog.rs): intermediate modifier in non-last position
    //!   - to_deeplink_name: repeated hyphens from mixed punctuation
    //!   - score_action: fuzzy bonus value is 25 (not 50)
    //!   - build_grouped_items_static: None section skips header even in Headers mode
    //!   - CommandBarConfig: notes_style uses Separators not Headers
    //!   - Cross-context: every context's first action has shortcut ↵

    use super::builders::*;
    use super::command_bar::CommandBarConfig;
    use super::dialog::{GroupedActionItem, build_grouped_items_static, coerce_action_selection};
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // ─────────────────────────────────────────────
    // 1. Clipboard: frontmost_app_name dynamic paste title
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_01_clipboard_paste_title_with_app_name() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Safari".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].title, "Paste to Safari");
    }

    #[test]
    fn cat27_01_clipboard_paste_title_without_app_name() {
        let entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].title, "Paste to Active App");
    }

    #[test]
    fn cat27_01_clipboard_paste_title_with_long_app_name() {
        let entry = ClipboardEntryInfo {
            id: "3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Visual Studio Code".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions[0].title.contains("Visual Studio Code"));
    }

    #[test]
    fn cat27_01_clipboard_paste_id_is_clipboard_paste() {
        let entry = ClipboardEntryInfo {
            id: "4".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Finder".into()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
    }

    // ─────────────────────────────────────────────
    // 2. Scriptlet context: edit_scriptlet desc mentions markdown
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_02_scriptlet_edit_desc_mentions_markdown() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
        assert!(
            edit.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("markdown")
        );
    }

    #[test]
    fn cat27_02_scriptlet_reveal_desc_mentions_bundle() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let reveal = actions
            .iter()
            .find(|a| a.id == "reveal_scriptlet_in_finder")
            .unwrap();
        assert!(
            reveal
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("bundle")
        );
    }

    #[test]
    fn cat27_02_scriptlet_copy_path_id_is_copy_scriptlet_path() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"copy_scriptlet_path"));
    }

    #[test]
    fn cat27_02_scriptlet_has_copy_content() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"copy_content"));
    }

    // ─────────────────────────────────────────────
    // 3. Script context: agent action count and ordering
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_03_agent_has_exactly_8_actions_no_shortcut_no_alias() {
        let mut script = ScriptInfo::builtin("my-agent");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        // Agents never get shortcut/alias preference actions; empty path drops toggle_favorite.
        // run_script, toggle_info, edit_script(agent), reveal_in_finder,
        // file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink
        assert_eq!(actions.len(), 8);
    }

    #[test]
    fn cat27_03_agent_first_action_is_run_script() {
        let mut script = ScriptInfo::builtin("my-agent");
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn cat27_03_agent_last_action_is_copy_deeplink() {
        let mut script = ScriptInfo::builtin("my-agent");
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert_eq!(actions.last().unwrap().id, "copy_deeplink");
    }

    #[test]
    fn cat27_03_agent_with_suggested_last_is_reset_ranking() {
        let mut script = ScriptInfo::builtin("my-agent");
        script.is_agent = true;
        script.is_suggested = true;
        let actions = get_script_context_actions(&script);
        assert_eq!(actions.last().unwrap().id, "reset_ranking");
    }

    // ─────────────────────────────────────────────
    // 4. Notes command bar: auto_sizing action is conditional
    // ─────────────────────────────────────────────

    // ─────────────────────────────────────────────
    // 5. AI command bar: paste_image details
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_05_ai_paste_image_shortcut() {
        let actions = get_ai_command_bar_actions();
        let paste = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(paste.shortcut.as_deref(), Some("⌘V"));
    }

    #[test]
    fn cat27_05_ai_paste_image_section_is_attachments() {
        let actions = get_ai_command_bar_actions();
        let paste = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(paste.section.as_deref(), Some("Attachments"));
    }

    #[test]
    fn cat27_05_ai_paste_image_icon_is_file() {
        let actions = get_ai_command_bar_actions();
        let paste = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(paste.icon, Some(IconName::File));
    }

    #[test]
    fn cat27_05_ai_paste_image_desc_mentions_clipboard() {
        let actions = get_ai_command_bar_actions();
        let paste = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert!(
            paste
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    // ─────────────────────────────────────────────
    // 6. Chat context: zero models, combo action counts
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_06_chat_zero_models_no_flags_one_action() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].id, "chat:change_model");
    }

    #[test]
    fn cat27_06_chat_one_model_both_flags_four_actions() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        // 1 model + continue + copy_response + clear + capture = 5
        assert_eq!(actions.len(), 5);
    }

    #[test]
    fn cat27_06_chat_three_models_no_flags() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![
                ChatModelInfo {
                    id: "a".into(),
                    display_name: "A".into(),
                    provider: "P".into(),
                },
                ChatModelInfo {
                    id: "b".into(),
                    display_name: "B".into(),
                    provider: "P".into(),
                },
                ChatModelInfo {
                    id: "c".into(),
                    display_name: "C".into(),
                    provider: "P".into(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue + capture = 3 (models in drill-down picker)
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn cat27_06_chat_response_without_messages() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue + copy_response + capture = 4
        assert_eq!(actions.len(), 4);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"chat:copy_response"));
        assert!(!ids.contains(&"chat:clear_conversation"));
    }

    // ─────────────────────────────────────────────
    // 7. New chat: model_idx ID pattern and section
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_07_new_chat_model_ids_use_model_prefix() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt-4o".into(),
            display_name: "GPT-4o".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_openai::gpt-4o");
    }

    #[test]
    fn cat27_07_new_chat_model_section_is_models() {
        let models = vec![NewChatModelInfo {
            model_id: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    #[test]
    fn cat27_07_new_chat_preset_ids_use_preset_prefix() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_general");
    }

    #[test]
    fn cat27_07_new_chat_last_used_ids_use_last_used_prefix() {
        let last = vec![NewChatModelInfo {
            model_id: "x".into(),
            display_name: "X Model".into(),
            provider: "p".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&last, &[], &[]);
        assert_eq!(actions[0].id, "last_used_p::x");
    }

    // ─────────────────────────────────────────────
    // 8. Note switcher: empty preview+empty time→char count
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_08_note_switcher_empty_preview_empty_time_shows_chars() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Test".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("42 chars"));
    }

    #[test]
    fn cat27_08_note_switcher_empty_preview_with_time_shows_time_only() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Test".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("5m ago"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn cat27_08_note_switcher_singular_char_count() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("1 char"));
    }

    #[test]
    fn cat27_08_note_switcher_zero_chars() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Empty".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("0 chars"));
    }

    // ─────────────────────────────────────────────
    // 9. File context: copy_filename vs copy_path shortcut
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_09_file_copy_filename_shortcut_is_cmd_c() {
        let file = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file);
        let copy_fn = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert_eq!(copy_fn.shortcut.as_deref(), Some("⌘C"));
    }

    #[test]
    fn cat27_09_file_copy_path_shortcut_is_cmd_shift_c() {
        let file = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file);
        let copy_p = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(copy_p.shortcut.as_deref(), Some("⌘⇧C"));
    }

    #[test]
    fn cat27_09_file_dir_copy_filename_also_cmd_c() {
        let dir = FileInfo {
            name: "mydir".into(),
            path: "/tmp/mydir".into(),
            is_dir: true,
            file_type: FileType::Directory,
        };
        let actions = get_file_context_actions(&dir);
        let copy_fn = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert_eq!(copy_fn.shortcut.as_deref(), Some("⌘C"));
    }

    // ─────────────────────────────────────────────
    // 10. Path context: copy_filename has no shortcut
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_10_path_copy_filename_no_shortcut() {
        let path = PathInfo {
            name: "file.rs".into(),
            path: "/tmp/file.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        let copy_fn = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(copy_fn.shortcut.is_none());
    }

    #[test]
    fn cat27_10_path_copy_path_has_shortcut() {
        let path = PathInfo {
            name: "file.rs".into(),
            path: "/tmp/file.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        let copy_p = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(copy_p.shortcut.as_deref(), Some("⌘⇧C"));
    }

    #[test]
    fn cat27_10_path_dir_copy_filename_still_no_shortcut() {
        let path = PathInfo {
            name: "src".into(),
            path: "/tmp/src".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path);
        let copy_fn = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(copy_fn.shortcut.is_none());
    }

    // ─────────────────────────────────────────────
    // 11. format_shortcut_hint: intermediate modifier handling
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_11_format_hint_cmd_shift_c() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("cmd+shift+c"),
            "⌘⇧C"
        );
    }

    #[test]
    fn cat27_11_format_hint_ctrl_alt_delete() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("ctrl+alt+delete"),
            "⌃⌥⌫"
        );
    }

    #[test]
    fn cat27_11_format_hint_single_key_enter() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("enter"),
            "↵"
        );
    }

    #[test]
    fn cat27_11_format_hint_super_alias() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("super+k"),
            "⌘K"
        );
    }

    #[test]
    fn cat27_11_format_hint_option_space() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("option+space"),
            "⌥␣"
        );
    }

    // ─────────────────────────────────────────────
    // 12. to_deeplink_name: mixed punctuation collapses
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_12_deeplink_mixed_punctuation_collapses() {
        assert_eq!(to_deeplink_name("a...b"), "a-b");
    }

    #[test]
    fn cat27_12_deeplink_parens_and_brackets() {
        assert_eq!(to_deeplink_name("foo (bar) [baz]"), "foo-bar-baz");
    }

    #[test]
    fn cat27_12_deeplink_ampersand_and_at() {
        assert_eq!(to_deeplink_name("copy & paste @ home"), "copy-paste-home");
    }

    #[test]
    fn cat27_12_deeplink_slash_and_backslash() {
        assert_eq!(to_deeplink_name("path/to\\file"), "path-to-file");
    }

    // ─────────────────────────────────────────────
    // 13. score_action: fuzzy bonus value
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_13_score_fuzzy_match_gives_25() {
        // "rn" is a subsequence of "run script" but not prefix or contains
        let action = Action::new(
            "run_script",
            "Run Script",
            None,
            ActionCategory::ScriptContext,
        );
        // "rp" → r...(u)(n)(space)(s)(c)(r)(i)(p) - subsequence r,p
        let score = super::dialog::ActionsDialog::score_action(&action, "rp");
        // fuzzy match gives 25
        assert_eq!(score, 25);
    }

    #[test]
    fn cat27_13_score_prefix_gives_at_least_100() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            None,
            ActionCategory::ScriptContext,
        );
        let score = super::dialog::ActionsDialog::score_action(&action, "edit");
        assert!(score >= 100);
    }

    #[test]
    fn cat27_13_score_contains_gives_50() {
        let action = Action::new("test", "Open Editor", None, ActionCategory::ScriptContext);
        let score = super::dialog::ActionsDialog::score_action(&action, "editor");
        // "editor" is contained in "open editor" but not a prefix
        assert!(score >= 50);
    }

    #[test]
    fn cat27_13_score_no_match_gives_0() {
        let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext);
        let score = super::dialog::ActionsDialog::score_action(&action, "xyz");
        assert_eq!(score, 0);
    }

    // ─────────────────────────────────────────────
    // 14. build_grouped_items_static: None section in Headers mode
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_14_headers_mode_no_section_skips_header() {
        let actions = vec![
            Action::new("a", "Alpha", None, ActionCategory::ScriptContext),
            Action::new("b", "Beta", None, ActionCategory::ScriptContext),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // No sections → no headers, just items
        assert_eq!(grouped.len(), 2);
        assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
    }

    #[test]
    fn cat27_14_headers_mode_with_section_adds_header() {
        let actions = vec![
            Action::new("a", "Alpha", None, ActionCategory::ScriptContext).with_section("Group A"),
            Action::new("b", "Beta", None, ActionCategory::ScriptContext).with_section("Group A"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // 1 header + 2 items
        assert_eq!(grouped.len(), 3);
        assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
    }

    #[test]
    fn cat27_14_headers_mode_two_sections_two_headers() {
        let actions = vec![
            Action::new("a", "Alpha", None, ActionCategory::ScriptContext).with_section("X"),
            Action::new("b", "Beta", None, ActionCategory::ScriptContext).with_section("Y"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // 2 headers + 2 items
        assert_eq!(grouped.len(), 4);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(header_count, 2);
    }

    #[test]
    fn cat27_14_separators_mode_never_adds_headers() {
        let actions = vec![
            Action::new("a", "Alpha", None, ActionCategory::ScriptContext).with_section("X"),
            Action::new("b", "Beta", None, ActionCategory::ScriptContext).with_section("Y"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // No headers in Separators mode
        assert_eq!(grouped.len(), 2);
    }

    // ─────────────────────────────────────────────
    // 15. CommandBarConfig: notes_style uses Separators
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_15_notes_style_uses_separators() {
        let cfg = CommandBarConfig::notes_style();
        assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
    }

    #[test]
    fn cat27_15_ai_style_uses_headers() {
        let cfg = CommandBarConfig::ai_style();
        assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
    }

    #[test]
    fn cat27_15_main_menu_uses_separators() {
        let cfg = CommandBarConfig::main_menu_style();
        assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
    }

    #[test]
    fn cat27_15_no_search_uses_hidden() {
        let cfg = CommandBarConfig::no_search();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Hidden);
    }

    // ─────────────────────────────────────────────
    // 16. Cross-context: first action shortcut is ↵
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_16_script_first_shortcut_is_enter() {
        let script = ScriptInfo::new("test", "/p.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn cat27_16_clipboard_first_shortcut_is_enter() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn cat27_16_path_file_first_shortcut_is_enter() {
        let path = PathInfo {
            name: "f.txt".into(),
            path: "/f.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn cat27_16_file_context_first_shortcut_is_enter() {
        let file = FileInfo {
            name: "a.txt".into(),
            path: "/a.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file);
        assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
    }

    // ─────────────────────────────────────────────
    // 17. Clipboard: image text action difference (image has OCR)
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_17_clipboard_image_has_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"clip:clipboard_ocr"));
    }

    #[test]
    fn cat27_17_clipboard_text_no_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(!ids.contains(&"clip:clipboard_ocr"));
    }

    #[test]
    fn cat27_17_clipboard_image_more_actions_than_text() {
        let text_entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let img_entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((10, 10)),
            frontmost_app_name: None,
        };
        let text_actions = get_clipboard_history_context_actions(&text_entry);
        let img_actions = get_clipboard_history_context_actions(&img_entry);
        assert!(img_actions.len() > text_actions.len());
    }

    // ─────────────────────────────────────────────
    // 18. coerce_action_selection: all items selectable
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_18_coerce_all_items_stays_at_index() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::Item(1),
            GroupedActionItem::Item(2),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(1));
    }

    #[test]
    fn cat27_18_coerce_index_beyond_len_clamps() {
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        // ix=10 → clamped to len-1 = 1
        assert_eq!(coerce_action_selection(&rows, 10), Some(1));
    }

    #[test]
    fn cat27_18_coerce_header_at_0_jumps_to_1() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn cat27_18_coerce_only_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ─────────────────────────────────────────────
    // 19. Action: with_shortcut sets shortcut_lower
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_19_with_shortcut_sets_shortcut_lower() {
        let action =
            Action::new("t", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        assert_eq!(action.shortcut_lower, Some("⌘e".into()));
    }

    #[test]
    fn cat27_19_no_shortcut_shortcut_lower_is_none() {
        let action = Action::new("t", "Test", None, ActionCategory::ScriptContext);
        assert!(action.shortcut_lower.is_none());
    }

    #[test]
    fn cat27_19_title_lower_is_precomputed() {
        let action = Action::new("t", "Edit Script", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "edit script");
    }

    #[test]
    fn cat27_19_description_lower_is_precomputed() {
        let action = Action::new(
            "t",
            "Test",
            Some("Open in $EDITOR".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower, Some("open in $editor".into()));
    }

    // ─────────────────────────────────────────────
    // 20. Script context: scriptlet vs script edit action IDs differ
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_20_script_edit_id_is_edit_script() {
        let script = ScriptInfo::new("test", "/p.ts");
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"edit_script"));
        assert!(!ids.contains(&"edit_scriptlet"));
    }

    #[test]
    fn cat27_20_scriptlet_edit_id_is_edit_scriptlet() {
        let script = ScriptInfo::scriptlet("s", "/p.md", None, None);
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"edit_scriptlet"));
        assert!(!ids.contains(&"edit_script"));
    }

    #[test]
    fn cat27_20_agent_edit_id_is_edit_script() {
        let mut script = ScriptInfo::builtin("agent");
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"edit_script"));
    }

    #[test]
    fn cat27_20_agent_edit_title_says_agent() {
        let mut script = ScriptInfo::builtin("agent");
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.title.contains("Agent"));
    }

    // ─────────────────────────────────────────────
    // 21. Notes command bar: copy section requires selection+not trash
    // ─────────────────────────────────────────────

    // ─────────────────────────────────────────────
    // 22. AI command bar: section counts
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_22_ai_response_section_has_3_actions() {
        let actions = get_ai_command_bar_actions();
        let response_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(response_count, 3);
    }

    #[test]
    fn cat27_22_ai_actions_section_has_4_actions() {
        let actions = get_ai_command_bar_actions();
        let action_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Actions"))
            .count();
        assert_eq!(action_count, 7);
    }

    #[test]
    fn cat27_22_ai_attachments_section_has_2_actions() {
        let actions = get_ai_command_bar_actions();
        let attach_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Attachments"))
            .count();
        assert_eq!(attach_count, 4);
    }

    #[test]
    fn cat27_22_ai_total_is_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    // ─────────────────────────────────────────────
    // 23. parse_shortcut_keycaps: various combos
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_23_parse_keycaps_cmd_e() {
        let caps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘E");
        assert_eq!(caps, vec!["⌘", "E"]);
    }

    #[test]
    fn cat27_23_parse_keycaps_all_modifiers_and_key() {
        let caps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧A");
        assert_eq!(caps, vec!["⌘", "⌃", "⌥", "⇧", "A"]);
    }

    #[test]
    fn cat27_23_parse_keycaps_enter_alone() {
        let caps = super::dialog::ActionsDialog::parse_shortcut_keycaps("↵");
        assert_eq!(caps, vec!["↵"]);
    }

    #[test]
    fn cat27_23_parse_keycaps_lowercase_uppercased() {
        let caps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘c");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    // ─────────────────────────────────────────────
    // 24. fuzzy_match: various patterns
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_24_fuzzy_match_exact() {
        assert!(super::dialog::ActionsDialog::fuzzy_match(
            "script:edit",
            "script:edit"
        ));
    }

    #[test]
    fn cat27_24_fuzzy_match_subsequence() {
        assert!(super::dialog::ActionsDialog::fuzzy_match(
            "edit script",
            "es"
        ));
    }

    #[test]
    fn cat27_24_fuzzy_match_no_match() {
        assert!(!super::dialog::ActionsDialog::fuzzy_match(
            "script:edit",
            "z"
        ));
    }

    #[test]
    fn cat27_24_fuzzy_match_empty_needle() {
        assert!(super::dialog::ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn cat27_24_fuzzy_match_needle_longer_fails() {
        assert!(!super::dialog::ActionsDialog::fuzzy_match("ab", "abc"));
    }

    // ─────────────────────────────────────────────
    // 25. Script context: view_logs exclusive to is_script
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_25_script_has_view_logs() {
        let script = ScriptInfo::new("test", "/p.ts");
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"view_logs"));
    }

    #[test]
    fn cat27_25_scriptlet_no_view_logs() {
        let script = ScriptInfo::scriptlet("s", "/p.md", None, None);
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(!ids.contains(&"view_logs"));
    }

    #[test]
    fn cat27_25_builtin_no_view_logs() {
        let script = ScriptInfo::builtin("Clipboard");
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(!ids.contains(&"view_logs"));
    }

    #[test]
    fn cat27_25_agent_no_view_logs() {
        let mut script = ScriptInfo::builtin("agent");
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(!ids.contains(&"view_logs"));
    }

    // ─────────────────────────────────────────────
    // 26. Clipboard: delete_all desc mentions pinned exception
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_26_clipboard_delete_all_desc_mentions_pinned() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let del_all = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete_all")
            .unwrap();
        assert!(
            del_all
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("pinned")
        );
    }

    #[test]
    fn cat27_26_clipboard_delete_multiple_desc_mentions_filter() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let del_multi = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete_multiple")
            .unwrap();
        assert!(
            del_multi
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("search")
        );
    }

    #[test]
    fn cat27_26_clipboard_delete_shortcut_is_ctrl_x() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let del = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete")
            .unwrap();
        assert_eq!(del.shortcut.as_deref(), Some("⌃X"));
    }

    // ─────────────────────────────────────────────
    // 27. Note switcher: preview with time uses separator
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_27_note_switcher_preview_with_time_has_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "3m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.contains(" · "));
        assert!(desc.contains("Hello world"));
        assert!(desc.contains("3m ago"));
    }

    #[test]
    fn cat27_27_note_switcher_long_preview_truncated() {
        let long_preview = "a".repeat(80);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 80,
            is_current: false,
            is_pinned: false,
            preview: long_preview,
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.contains("…"));
    }

    #[test]
    fn cat27_27_note_switcher_exactly_60_chars_no_truncation() {
        let exact = "b".repeat(60);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 60,
            is_current: false,
            is_pinned: false,
            preview: exact,
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(!desc.contains("…"));
    }

    // ─────────────────────────────────────────────
    // 28. ScriptInfo: with_frecency builder chain
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_28_with_frecency_sets_is_suggested() {
        let script = ScriptInfo::new("test", "/p.ts").with_frecency(true, Some("/p".into()));
        assert!(script.is_suggested);
        assert_eq!(script.frecency_path, Some("/p".into()));
    }

    #[test]
    fn cat27_28_with_frecency_false_not_suggested() {
        let script = ScriptInfo::new("test", "/p.ts").with_frecency(false, None);
        assert!(!script.is_suggested);
        assert!(script.frecency_path.is_none());
    }

    #[test]
    fn cat27_28_with_frecency_preserves_other_fields() {
        let script = ScriptInfo::new("test", "/p.ts").with_frecency(true, None);
        assert!(script.is_script);
        assert_eq!(script.action_verb, "Run");
        assert_eq!(script.name, "test");
    }

    // ─────────────────────────────────────────────
    // 29. CommandBarConfig: anchor positions
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_29_default_config_anchor_bottom() {
        let cfg = CommandBarConfig::default();
        assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Bottom);
    }

    #[test]
    fn cat27_29_ai_style_anchor_top() {
        let cfg = CommandBarConfig::ai_style();
        assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Top);
    }

    #[test]
    fn cat27_29_main_menu_anchor_bottom() {
        let cfg = CommandBarConfig::main_menu_style();
        assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Bottom);
    }

    #[test]
    fn cat27_29_notes_style_anchor_top() {
        let cfg = CommandBarConfig::notes_style();
        assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Top);
    }

    // --- merged from part_04.rs ---

    // ─────────────────────────────────────────────
    // 30. Cross-context: all action IDs are non-empty
    // ─────────────────────────────────────────────

    #[test]
    fn cat27_30_script_action_ids_non_empty() {
        let script = ScriptInfo::new("test", "/p.ts");
        let actions = get_script_context_actions(&script);
        for a in &actions {
            assert!(!a.id.is_empty(), "action ID should not be empty");
        }
    }

    #[test]
    fn cat27_30_clipboard_action_ids_non_empty() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for a in &actions {
            assert!(!a.id.is_empty());
        }
    }

    #[test]
    fn cat27_30_ai_action_ids_non_empty() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(!a.id.is_empty());
        }
    }

    #[test]
    fn cat27_30_path_action_ids_non_empty() {
        let path = PathInfo {
            name: "f".into(),
            path: "/f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path);
        for a in &actions {
            assert!(!a.id.is_empty());
        }
    }

    #[test]
    fn cat27_30_file_action_ids_non_empty() {
        let file = FileInfo {
            name: "f.txt".into(),
            path: "/f.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file);
        for a in &actions {
            assert!(!a.id.is_empty());
        }
    }

}

mod from_dialog_builtin_action_validation_tests_28 {
    // --- merged from part_01.rs ---
    //! Batch 28: Builtin action validation tests
    //!
    //! 118 tests across 30 categories validating built-in action window dialog behaviors.

    use super::builders::*;
    use super::command_bar::CommandBarConfig;
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // =============================================================================
    // Category 1: Scriptlet context — copy_content shortcut and description
    // =============================================================================

    #[test]
    fn cat28_01_scriptlet_copy_content_shortcut() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn cat28_01_scriptlet_copy_content_description() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(
            cc.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("content")
        );
    }

    #[test]
    fn cat28_01_scriptlet_copy_content_title() {
        let script = ScriptInfo::scriptlet("My Snippet", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(cc.title, "Copy Content");
    }

    #[test]
    fn cat28_01_scriptlet_copy_content_has_action_false() {
        let script = ScriptInfo::scriptlet("X", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(!cc.has_action);
    }

    // =============================================================================
    // Category 2: Scriptlet context — alias actions share shortcut with script context
    // =============================================================================

    #[test]
    fn cat28_02_scriptlet_add_alias_shortcut() {
        let script = ScriptInfo::scriptlet("S", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let aa = actions.iter().find(|a| a.id == "add_alias").unwrap();
        assert_eq!(aa.shortcut.as_deref(), Some("⌘⇧A"));
    }

    #[test]
    fn cat28_02_scriptlet_update_alias_when_alias_present() {
        let script = ScriptInfo::scriptlet("S", "/p.md", None, Some("al".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "update_alias"));
        assert!(!actions.iter().any(|a| a.id == "add_alias"));
    }

    #[test]
    fn cat28_02_scriptlet_remove_alias_shortcut() {
        let script = ScriptInfo::scriptlet("S", "/p.md", None, Some("al".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let ra = actions.iter().find(|a| a.id == "remove_alias").unwrap();
        assert_eq!(ra.shortcut.as_deref(), Some("⌘⌥A"));
    }

    #[test]
    fn cat28_02_scriptlet_update_alias_shortcut() {
        let script = ScriptInfo::scriptlet("S", "/p.md", None, Some("al".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let ua = actions.iter().find(|a| a.id == "update_alias").unwrap();
        assert_eq!(ua.shortcut.as_deref(), Some("⌘⇧A"));
    }

    // =============================================================================
    // Category 3: Script context — copy_content and copy_path shortcuts
    // =============================================================================

    #[test]
    fn cat28_03_script_copy_content_shortcut() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn cat28_03_script_copy_path_shortcut() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
        assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
    }

    #[test]
    fn cat28_03_script_edit_shortcut() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let es = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(es.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn cat28_03_script_reveal_shortcut() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let rv = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
        assert_eq!(rv.shortcut.as_deref(), Some("⌘⇧F"));
    }

    // =============================================================================
    // Category 4: Script context — view_logs details
    // =============================================================================

    #[test]
    fn cat28_04_script_view_logs_shortcut() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let vl = actions.iter().find(|a| a.id == "view_logs").unwrap();
        assert_eq!(vl.shortcut.as_deref(), Some("⌘L"));
    }

    #[test]
    fn cat28_04_script_view_logs_title() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let vl = actions.iter().find(|a| a.id == "view_logs").unwrap();
        assert_eq!(vl.title, "Show Logs");
    }

    #[test]
    fn cat28_04_script_view_logs_description() {
        let script = ScriptInfo::new("my-script", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let vl = actions.iter().find(|a| a.id == "view_logs").unwrap();
        assert!(
            vl.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("log")
        );
    }

    #[test]
    fn cat28_04_view_logs_absent_for_builtin() {
        let builtin = ScriptInfo::builtin("App Launcher");
        let actions = get_script_context_actions(&builtin);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // =============================================================================
    // Category 5: Clipboard — pinned entry produces clipboard_unpin
    // =============================================================================

    #[test]
    fn cat28_05_pinned_entry_has_unpin() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
    }

    #[test]
    fn cat28_05_unpinned_entry_has_pin() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
    }

    #[test]
    fn cat28_05_pin_unpin_same_shortcut() {
        let pinned = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let unpinned = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let a1 = get_clipboard_history_context_actions(&pinned);
        let a2 = get_clipboard_history_context_actions(&unpinned);
        let s1 = a1
            .iter()
            .find(|a| a.id == "clip:clipboard_unpin")
            .unwrap()
            .shortcut
            .as_deref();
        let s2 = a2
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap()
            .shortcut
            .as_deref();
        assert_eq!(s1, s2);
        assert_eq!(s1, Some("⇧⌘P"));
    }

    // =============================================================================
    // Category 6: Clipboard — save_snippet title and description
    // =============================================================================

    #[test]
    fn cat28_06_save_snippet_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ss = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_snippet")
            .unwrap();
        assert_eq!(ss.title, "Save Text as Snippet");
    }

    #[test]
    fn cat28_06_save_snippet_description_mentions_scriptlet() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ss = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_snippet")
            .unwrap();
        assert!(
            ss.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("scriptlet")
        );
    }

    #[test]
    fn cat28_06_save_file_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let sf = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_file")
            .unwrap();
        assert_eq!(sf.title, "Save as File...");
    }

    // =============================================================================
    // Category 7: Clipboard — clipboard_copy description mentions "without pasting"
    // =============================================================================

    #[test]
    fn cat28_07_clipboard_copy_desc_without_pasting() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let cc = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_copy")
            .unwrap();
        assert!(
            cc.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("without pasting")
        );
    }

    #[test]
    fn cat28_07_clipboard_paste_desc_mentions_paste() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let cp = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert!(
            cp.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("paste")
        );
    }

    #[test]
    fn cat28_07_clipboard_share_desc_mentions_share() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let cs = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert!(
            cs.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("share")
        );
    }

    // =============================================================================
    // Category 8: File context — title includes quoted file name
    // =============================================================================

    #[test]
    fn cat28_08_file_open_title_quotes_name() {
        let file_info = FileInfo {
            path: "/Users/test/readme.txt".into(),
            name: "readme.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.title.contains("\"readme.txt\""));
    }

    #[test]
    fn cat28_08_dir_open_title_quotes_name() {
        let file_info = FileInfo {
            path: "/Users/test/Documents".into(),
            name: "Documents".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file_info);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(open.title.contains("\"Documents\""));
    }

    #[test]
    fn cat28_08_file_open_desc_says_application() {
        let file_info = FileInfo {
            path: "/Users/test/readme.txt".into(),
            name: "readme.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("default app")
        );
    }

    #[test]
    fn cat28_08_dir_open_desc_says_folder() {
        let file_info = FileInfo {
            path: "/Users/test/Documents".into(),
            name: "Documents".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file_info);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("folder")
        );
    }

    // =============================================================================
    // Category 9: File context — reveal_in_finder description
    // =============================================================================

    #[test]
    fn cat28_09_file_reveal_desc_mentions_finder() {
        let file_info = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let rv = actions
            .iter()
            .find(|a| a.id == "file:reveal_in_finder")
            .unwrap();
        assert!(
            rv.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("finder")
        );
    }

    // --- merged from part_02.rs ---

    #[test]
    fn cat28_09_file_copy_path_desc_mentions_clipboard() {
        let file_info = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert!(
            cp.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    #[test]
    fn cat28_09_file_copy_filename_desc_mentions_filename() {
        let file_info = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(
            cf.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("filename")
        );
    }

    // =============================================================================
    // Category 10: Path context — move_to_trash description dynamic for dir vs file
    // =============================================================================

    #[test]
    fn cat28_10_path_trash_desc_dir_says_folder() {
        let path_info = PathInfo {
            name: "MyDir".into(),
            path: "/test/MyDir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(
            trash
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("folder")
        );
    }

    #[test]
    fn cat28_10_path_trash_desc_file_says_file() {
        let path_info = PathInfo {
            name: "doc.txt".into(),
            path: "/test/doc.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(
            trash
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("file")
        );
    }

    #[test]
    fn cat28_10_path_trash_shortcut() {
        let path_info = PathInfo {
            name: "doc.txt".into(),
            path: "/test/doc.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert_eq!(trash.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn cat28_10_path_trash_title() {
        let path_info = PathInfo {
            name: "doc.txt".into(),
            path: "/test/doc.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert_eq!(trash.title, "Move to Trash");
    }

    // =============================================================================
    // Category 11: Path context — select_file / open_directory title includes quoted name
    // =============================================================================

    #[test]
    fn cat28_11_path_select_file_title_quotes_name() {
        let path_info = PathInfo {
            name: "report.pdf".into(),
            path: "/test/report.pdf".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let sel = actions.iter().find(|a| a.id == "file:select_file").unwrap();
        assert!(sel.title.contains("\"report.pdf\""));
    }

    #[test]
    fn cat28_11_path_open_dir_title_quotes_name() {
        let path_info = PathInfo {
            name: "Projects".into(),
            path: "/test/Projects".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        let od = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(od.title.contains("\"Projects\""));
    }

    #[test]
    fn cat28_11_path_select_desc_says_submit() {
        let path_info = PathInfo {
            name: "file.txt".into(),
            path: "/test/file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let sel = actions.iter().find(|a| a.id == "file:select_file").unwrap();
        assert!(
            sel.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("selects this file")
        );
    }

    #[test]
    fn cat28_11_path_open_dir_desc_says_navigate() {
        let path_info = PathInfo {
            name: "Projects".into(),
            path: "/test/Projects".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        let od = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(
            od.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("opens this directory")
        );
    }

    // =============================================================================
    // Category 12: AI command bar — toggle_shortcuts_help details
    // =============================================================================

    #[test]
    fn cat28_12_ai_toggle_shortcuts_shortcut() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.shortcut.as_deref(), Some("⌘/"));
    }

    #[test]
    fn cat28_12_ai_toggle_shortcuts_icon() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.icon, Some(IconName::Star));
    }

    #[test]
    fn cat28_12_ai_toggle_shortcuts_section() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.section.as_deref(), Some("Help"));
    }

    #[test]
    fn cat28_12_ai_toggle_shortcuts_title() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.title, "Keyboard Shortcuts");
    }

    // =============================================================================
    // Category 13: AI command bar — new_chat details
    // =============================================================================

    #[test]
    fn cat28_13_ai_new_chat_shortcut() {
        let actions = get_ai_command_bar_actions();
        let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert_eq!(nc.shortcut.as_deref(), Some("⌘N"));
    }

    #[test]
    fn cat28_13_ai_new_chat_icon() {
        let actions = get_ai_command_bar_actions();
        let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert_eq!(nc.icon, Some(IconName::Plus));
    }

    #[test]
    fn cat28_13_ai_new_chat_section() {
        let actions = get_ai_command_bar_actions();
        let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert_eq!(nc.section.as_deref(), Some("Actions"));
    }

    #[test]
    fn cat28_13_ai_new_chat_desc_mentions_new() {
        let actions = get_ai_command_bar_actions();
        let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert!(
            nc.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("new")
        );
    }

    // =============================================================================
    // Category 14: AI command bar — delete_chat details
    // =============================================================================

    #[test]
    fn cat28_14_ai_delete_chat_shortcut() {
        let actions = get_ai_command_bar_actions();
        let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert_eq!(dc.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn cat28_14_ai_delete_chat_icon() {
        let actions = get_ai_command_bar_actions();
        let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert_eq!(dc.icon, Some(IconName::Trash));
    }

    #[test]
    fn cat28_14_ai_delete_chat_section() {
        let actions = get_ai_command_bar_actions();
        let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert_eq!(dc.section.as_deref(), Some("Actions"));
    }

    #[test]
    fn cat28_14_ai_delete_chat_desc_mentions_delete() {
        let actions = get_ai_command_bar_actions();
        let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert!(
            dc.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("delete")
        );
    }

    // =============================================================================
    // Category 15: Chat context — continue_in_chat shortcut
    // =============================================================================

    #[test]
    fn cat28_15_chat_continue_in_chat_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let cic = actions
            .iter()
            .find(|a| a.id == "chat:continue_in_chat")
            .unwrap();
        assert_eq!(cic.shortcut.as_deref(), Some("⌘↵"));
    }

    #[test]
    fn cat28_15_chat_continue_in_chat_desc_mentions_harness() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let cic = actions
            .iter()
            .find(|a| a.id == "chat:continue_in_chat")
            .unwrap();
        assert!(
            cic.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("harness")
        );
    }

    #[test]
    fn cat28_15_chat_continue_always_present() {
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
        assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
    }

    // =============================================================================
    // Category 16: Chat context — clear_conversation details
    // =============================================================================

    #[test]
    fn cat28_16_chat_clear_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let clr = actions
            .iter()
            .find(|a| a.id == "chat:clear_conversation")
            .unwrap();
        assert_eq!(clr.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn cat28_16_chat_clear_absent_when_no_messages() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn cat28_16_chat_clear_desc_mentions_clear() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let clr = actions
            .iter()
            .find(|a| a.id == "chat:clear_conversation")
            .unwrap();
        assert!(
            clr.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clear")
        );
    }

    #[test]
    fn cat28_16_chat_copy_response_absent_when_no_response() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
    }

    // =============================================================================
    // Category 17: Notes — export section icon
    // =============================================================================

    // --- merged from part_03.rs ---

    // =============================================================================
    // Category 18: Notes — browse_notes details
    // =============================================================================

    // =============================================================================
    // Category 19: Notes full mode action count
    // =============================================================================

    // =============================================================================
    // Category 20: Note switcher — pinned note icon and section
    // =============================================================================

    #[test]
    fn cat28_20_note_switcher_pinned_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Pinned Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: true,
            preview: "Some content".into(),
            relative_time: "1d ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn cat28_20_note_switcher_pinned_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Pinned Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: true,
            preview: "Some content".into(),
            relative_time: "1d ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn cat28_20_note_switcher_regular_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Regular Note".into(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "test".into(),
            relative_time: "2h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    #[test]
    fn cat28_20_note_switcher_regular_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Regular Note".into(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "test".into(),
            relative_time: "2h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Recent"));
    }

    // =============================================================================
    // Category 21: Note switcher — current note icon and title prefix
    // =============================================================================

    #[test]
    fn cat28_21_note_switcher_current_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "xyz".into(),
            title: "Current Note".into(),
            char_count: 200,
            is_current: true,
            is_pinned: false,
            preview: "body".into(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn cat28_21_note_switcher_current_bullet_prefix() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "xyz".into(),
            title: "Current Note".into(),
            char_count: 200,
            is_current: true,
            is_pinned: false,
            preview: "body".into(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(actions[0].title.starts_with("• "));
    }

    #[test]
    fn cat28_21_note_switcher_non_current_no_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "xyz".into(),
            title: "Other Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "test".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(!actions[0].title.starts_with("• "));
    }

    #[test]
    fn cat28_21_pinned_trumps_current_for_icon() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "xyz".into(),
            title: "Both".into(),
            char_count: 100,
            is_current: true,
            is_pinned: true,
            preview: "test".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    // =============================================================================
    // Category 22: New chat — model description is provider_display_name
    // =============================================================================

    #[test]
    fn cat28_22_new_chat_model_description() {
        let models = vec![NewChatModelInfo {
            model_id: "claude-3-opus".into(),
            display_name: "Claude 3 Opus".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        let model_action = actions
            .iter()
            .find(|a| a.id == "model_anthropic::claude-3-opus")
            .unwrap();
        assert_eq!(model_action.description.as_deref(), Some("Uses Anthropic"));
    }

    #[test]
    fn cat28_22_new_chat_model_icon() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        let model_action = actions
            .iter()
            .find(|a| a.id == "model_openai::gpt4")
            .unwrap();
        assert_eq!(model_action.icon, Some(IconName::Settings));
    }

    #[test]
    fn cat28_22_new_chat_model_section() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        let model_action = actions
            .iter()
            .find(|a| a.id == "model_openai::gpt4")
            .unwrap();
        assert_eq!(model_action.section.as_deref(), Some("Models"));
    }

    #[test]
    fn cat28_22_new_chat_model_title_is_display_name() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        let model_action = actions
            .iter()
            .find(|a| a.id == "model_openai::gpt4")
            .unwrap();
        assert_eq!(model_action.title, "GPT-4");
    }

    // =============================================================================
    // Category 23: New chat — preset description is None
    // =============================================================================

    #[test]
    fn cat28_23_new_chat_preset_description_none() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        let preset = actions.iter().find(|a| a.id == "preset_general").unwrap();
        assert_eq!(preset.description.as_deref(), Some("Uses General preset"));
    }

    #[test]
    fn cat28_23_new_chat_preset_icon_preserved() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        let preset = actions.iter().find(|a| a.id == "preset_code").unwrap();
        assert_eq!(preset.icon, Some(IconName::Code));
    }

    #[test]
    fn cat28_23_new_chat_preset_section() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        let preset = actions.iter().find(|a| a.id == "preset_general").unwrap();
        assert_eq!(preset.section.as_deref(), Some("Presets"));
    }

    #[test]
    fn cat28_23_new_chat_preset_title() {
        let presets = vec![NewChatPresetInfo {
            id: "writer".into(),
            name: "Writer".into(),
            icon: IconName::File,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        let preset = actions.iter().find(|a| a.id == "preset_writer").unwrap();
        assert_eq!(preset.title, "Writer");
    }

    // =============================================================================
    // Category 24: format_shortcut_hint (builders.rs version) — simple transforms
    // =============================================================================

    #[test]
    fn cat28_24_builders_format_hint_cmd_c() {
        assert_eq!(super::builders::to_deeplink_name("cmd+c"), "cmd-c");
    }

    #[test]
    fn cat28_24_to_deeplink_name_basic() {
        assert_eq!(to_deeplink_name("My Script"), "my-script");
    }

    #[test]
    fn cat28_24_to_deeplink_name_underscores() {
        assert_eq!(to_deeplink_name("hello_world"), "hello-world");
    }

    #[test]
    fn cat28_24_to_deeplink_name_empty() {
        assert_eq!(to_deeplink_name(""), "_unnamed");
    }

    // =============================================================================
    // Category 25: Action with_shortcut_opt: None vs Some
    // =============================================================================

    #[test]
    fn cat28_25_with_shortcut_opt_some() {
        let action = Action::new("a", "A", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘A".into()));
        assert_eq!(action.shortcut.as_deref(), Some("⌘A"));
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘a"));
    }

    #[test]
    fn cat28_25_with_shortcut_opt_none() {
        let action =
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
        assert!(action.shortcut.is_none());
        assert!(action.shortcut_lower.is_none());
    }

    #[test]
    fn cat28_25_with_shortcut_sets_lower() {
        let action =
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘⇧c"));
    }

    #[test]
    fn cat28_25_action_new_no_shortcut_lower() {
        let action = Action::new("a", "A", None, ActionCategory::ScriptContext);
        assert!(action.shortcut_lower.is_none());
    }

    // =============================================================================
    // Category 26: Action with_icon and with_section
    // =============================================================================

    #[test]
    fn cat28_26_with_icon_sets_field() {
        let action =
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_icon(IconName::Copy);
        assert_eq!(action.icon, Some(IconName::Copy));
    }

    #[test]
    fn cat28_26_action_new_no_icon() {
        let action = Action::new("a", "A", None, ActionCategory::ScriptContext);
        assert!(action.icon.is_none());
    }

    #[test]
    fn cat28_26_with_section_sets_field() {
        let action =
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("My Section");
        assert_eq!(action.section.as_deref(), Some("My Section"));
    }

    #[test]
    fn cat28_26_action_new_no_section() {
        let action = Action::new("a", "A", None, ActionCategory::ScriptContext);
        assert!(action.section.is_none());
    }

    // =============================================================================
    // Category 27: Action cached lowercase fields
    // =============================================================================

    #[test]
    fn cat28_27_title_lower_computed() {
        let action = Action::new("a", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "hello world");
    }

    #[test]
    fn cat28_27_description_lower_computed() {
        let action = Action::new(
            "a",
            "A",
            Some("Some Description".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(
            action.description_lower.as_deref(),
            Some("some description")
        );
    }

    // --- merged from part_04.rs ---

    #[test]
    fn cat28_27_description_lower_none_when_no_desc() {
        let action = Action::new("a", "A", None, ActionCategory::ScriptContext);
        assert!(action.description_lower.is_none());
    }

    #[test]
    fn cat28_27_shortcut_lower_after_with_shortcut() {
        let action =
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧D");
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘⇧d"));
    }

    // =============================================================================
    // Category 28: CommandBarConfig presets — dialog_config fields
    // =============================================================================

    #[test]
    fn cat28_28_main_menu_search_top() {
        let cfg = CommandBarConfig::main_menu_style();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
    }

    #[test]
    fn cat28_28_ai_style_search_top() {
        let cfg = CommandBarConfig::ai_style();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
    }

    #[test]
    fn cat28_28_no_search_hidden() {
        let cfg = CommandBarConfig::no_search();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Hidden);
    }

    #[test]
    fn cat28_28_notes_style_search_top() {
        let cfg = CommandBarConfig::notes_style();
        assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
    }

    // =============================================================================
    // Category 29: Cross-context — action ID uniqueness within each context
    // =============================================================================

    #[test]
    fn cat28_29_script_ids_unique() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Duplicate IDs found: {:?}", ids);
    }

    #[test]
    fn cat28_29_clipboard_ids_unique() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Duplicate IDs found: {:?}", ids);
    }

    #[test]
    fn cat28_29_ai_ids_unique() {
        let actions = get_ai_command_bar_actions();
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Duplicate IDs found: {:?}", ids);
    }

    #[test]
    fn cat28_29_path_ids_unique() {
        let path_info = PathInfo {
            name: "test".into(),
            path: "/test".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Duplicate IDs found: {:?}", ids);
    }

    // =============================================================================
    // Category 30: Cross-context — all contexts produce non-empty title and id
    // =============================================================================

    #[test]
    fn cat28_30_script_actions_non_empty_titles() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        for a in &actions {
            assert!(!a.id.is_empty(), "Action ID must not be empty");
            assert!(!a.title.is_empty(), "Action title must not be empty");
        }
    }

    #[test]
    fn cat28_30_clipboard_actions_non_empty_titles() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for a in &actions {
            assert!(!a.id.is_empty(), "Action ID must not be empty");
            assert!(!a.title.is_empty(), "Action title must not be empty");
        }
    }

    #[test]
    fn cat28_30_file_actions_non_empty_titles() {
        let file_info = FileInfo {
            path: "/test/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        for a in &actions {
            assert!(!a.id.is_empty(), "Action ID must not be empty");
            assert!(!a.title.is_empty(), "Action title must not be empty");
        }
    }

    #[test]
    fn cat28_30_ai_actions_non_empty_titles() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(!a.id.is_empty(), "Action ID must not be empty");
            assert!(!a.title.is_empty(), "Action title must not be empty");
        }
    }

    #[test]
    fn cat28_30_path_actions_non_empty_titles() {
        let path_info = PathInfo {
            name: "test".into(),
            path: "/test".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        for a in &actions {
            assert!(!a.id.is_empty(), "Action ID must not be empty");
            assert!(!a.title.is_empty(), "Action title must not be empty");
        }
    }
}

mod from_dialog_builtin_action_validation_tests_29 {
    // --- merged from part_01.rs ---
    //! Batch 29: Builtin action validation tests
    //!
    //! 115 tests across 30 categories validating built-in action window dialog behaviors.

    use super::builders::*;
    use super::command_bar::CommandBarConfig;
    use super::dialog::{GroupedActionItem, build_grouped_items_static, coerce_action_selection};
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // =============================================================================
    // Category 1: Note switcher — empty notes produces helpful placeholder action
    // =============================================================================

    #[test]
    fn cat29_01_note_switcher_empty_produces_placeholder() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn cat29_01_note_switcher_empty_placeholder_id() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].id, "no_notes");
    }

    #[test]
    fn cat29_01_note_switcher_empty_placeholder_title() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].title, "No notes yet");
    }

    #[test]
    fn cat29_01_note_switcher_empty_placeholder_desc_mentions_cmd_n() {
        let actions = get_note_switcher_actions(&[]);
        assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
    }

    #[test]
    fn cat29_01_note_switcher_empty_placeholder_icon_plus() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].icon, Some(IconName::Plus));
    }

    // =============================================================================
    // Category 2: Note switcher — section is "Notes" for placeholder, else Pinned/Recent
    // =============================================================================

    #[test]
    fn cat29_02_note_switcher_empty_section_notes() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].section.as_deref(), Some("Notes"));
    }

    #[test]
    fn cat29_02_note_switcher_pinned_section() {
        let note = NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Pinned Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: true,
            preview: "some text".into(),
            relative_time: "1h ago".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn cat29_02_note_switcher_unpinned_section() {
        let note = NoteSwitcherNoteInfo {
            id: "def".into(),
            title: "Regular Note".into(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "content".into(),
            relative_time: "2d ago".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert_eq!(actions[0].section.as_deref(), Some("Recent"));
    }

    // =============================================================================
    // Category 3: Note switcher — note ID format is "note_{uuid}"
    // =============================================================================

    #[test]
    fn cat29_03_note_switcher_id_format() {
        let note = NoteSwitcherNoteInfo {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            title: "Test".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert_eq!(actions[0].id, "note_550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn cat29_03_note_switcher_id_starts_with_note_prefix() {
        let note = NoteSwitcherNoteInfo {
            id: "xyz".into(),
            title: "T".into(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert!(actions[0].id.starts_with("note_"));
    }

    // =============================================================================
    // Category 4: Clipboard — text entry action count on macOS vs all platforms
    // =============================================================================

    #[test]
    fn cat29_04_clipboard_text_action_count_cross_platform() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "Hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // Cross-platform: paste, copy, keep_open, share, attach_to_ai, pin, keep_in_today, save_snippet, save_file, delete, delete_multiple, delete_all = 12
        // macOS adds: quick_look = +1 = 13
        #[cfg(target_os = "macos")]
        assert_eq!(actions.len(), 13);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(actions.len(), 12);
    }

    #[test]
    fn cat29_04_clipboard_image_action_count_cross_platform() {
        let entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "Image".into(),
            image_dimensions: Some((640, 480)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // Image vs text: +ocr, -save_snippet (snippet is text-only)
        // macOS additionally adds: open_with, annotate_cleanshot, upload_cleanshot (quick_look is
        // on both text and image), so macOS image = text(13) - 1 + 4 = 16
        #[cfg(target_os = "macos")]
        assert_eq!(actions.len(), 16);
        #[cfg(not(target_os = "macos"))]
        assert_eq!(actions.len(), 12);
    }

    // =============================================================================
    // Category 5: Clipboard — attach_to_ai shortcut and section
    // =============================================================================

    #[test]
    fn cat29_05_clipboard_attach_to_ai_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ai = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert_eq!(ai.shortcut.as_deref(), Some("⌃⌘A"));
    }

    #[test]
    fn cat29_05_clipboard_attach_to_ai_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ai = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert_eq!(ai.title, "Attach to Agent Chat");
    }

    #[test]
    fn cat29_05_clipboard_attach_to_ai_desc_mentions_ai() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ai = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert!(
            ai.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent chat")
        );
    }

    // =============================================================================
    // Category 6: Clipboard — share action present for both text and image
    // =============================================================================

    #[test]
    fn cat29_06_clipboard_share_present_for_text() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "abc".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_share"));
    }

    #[test]
    fn cat29_06_clipboard_share_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_share"));
    }

    #[test]
    fn cat29_06_clipboard_share_shortcut_same_for_both() {
        let text_entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let img_entry = ClipboardEntryInfo {
            id: "2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "i".into(),
            image_dimensions: Some((10, 10)),
            frontmost_app_name: None,
        };
        let text_actions = get_clipboard_history_context_actions(&text_entry);
        let img_actions = get_clipboard_history_context_actions(&img_entry);
        let ts = text_actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        let is = img_actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert_eq!(ts.shortcut, is.shortcut);
    }

    // =============================================================================
    // Category 7: Script context — with_all constructor preserves every field
    // =============================================================================

    #[test]
    fn cat29_07_with_all_preserves_name() {
        let s = ScriptInfo::with_all(
            "Foo",
            "/p",
            true,
            "Launch",
            Some("cmd+f".into()),
            Some("f".into()),
        );
        assert_eq!(s.name, "Foo");
    }

    #[test]
    fn cat29_07_with_all_preserves_action_verb() {
        let s = ScriptInfo::with_all("Foo", "/p", true, "Launch", None, None);
        assert_eq!(s.action_verb, "Launch");
    }

    #[test]
    fn cat29_07_with_all_preserves_is_script() {
        let s = ScriptInfo::with_all("Foo", "/p", false, "Run", None, None);
        assert!(!s.is_script);
    }

    #[test]
    fn cat29_07_with_all_run_title_uses_verb_and_name() {
        let s = ScriptInfo::with_all("My Tool", "/p", true, "Execute", None, None);
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Execute");
    }

    // =============================================================================
    // Category 8: Script context — agent has edit_script with "Agent" title but no view_logs
    // =============================================================================

    #[test]
    fn cat29_08_agent_edit_title_says_agent() {
        let mut s = ScriptInfo::new("My Agent", "/p.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn cat29_08_agent_no_view_logs() {
        let mut s = ScriptInfo::new("My Agent", "/p.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    #[test]
    fn cat29_08_agent_has_copy_content() {
        let mut s = ScriptInfo::new("My Agent", "/p.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "copy_content"));
    }

    #[test]
    fn cat29_08_agent_has_reveal_in_finder() {
        let mut s = ScriptInfo::new("My Agent", "/p.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
    }

    // =============================================================================
    // Category 9: Notes command bar — format action details
    // =============================================================================

    // =============================================================================
    // Category 10: Notes command bar — new_note always present with correct details
    // =============================================================================

    // --- merged from part_02.rs ---

    // =============================================================================
    // Category 11: AI command bar — copy_chat details
    // =============================================================================

    #[test]
    fn cat29_11_ai_copy_chat_shortcut() {
        let actions = get_ai_command_bar_actions();
        let cc = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌥⇧⌘C"));
    }

    #[test]
    fn cat29_11_ai_copy_chat_icon() {
        let actions = get_ai_command_bar_actions();
        let cc = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(cc.icon, Some(IconName::Copy));
    }

    #[test]
    fn cat29_11_ai_copy_chat_section() {
        let actions = get_ai_command_bar_actions();
        let cc = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(cc.section.as_deref(), Some("Response"));
    }

    #[test]
    fn cat29_11_ai_copy_chat_desc_mentions_conversation() {
        let actions = get_ai_command_bar_actions();
        let cc = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert!(
            cc.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("conversation")
        );
    }

    // =============================================================================
    // Category 12: AI command bar — copy_last_code details
    // =============================================================================

    #[test]
    fn cat29_12_ai_copy_last_code_shortcut() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(clc.shortcut.as_deref(), Some("⌥⌘C"));
    }

    #[test]
    fn cat29_12_ai_copy_last_code_icon() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(clc.icon, Some(IconName::Code));
    }

    #[test]
    fn cat29_12_ai_copy_last_code_section() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(clc.section.as_deref(), Some("Response"));
    }

    #[test]
    fn cat29_12_ai_copy_last_code_desc_mentions_code() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert!(
            clc.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("code")
        );
    }

    // =============================================================================
    // Category 13: AI command bar — copy_response in command bar vs chat context
    // =============================================================================

    #[test]
    fn cat29_13_ai_command_bar_copy_response_shortcut() {
        let actions = get_ai_command_bar_actions();
        let cr = actions
            .iter()
            .find(|a| a.id == "chat:copy_response")
            .unwrap();
        assert_eq!(cr.shortcut.as_deref(), Some("⇧⌘C"));
    }

    #[test]
    fn cat29_13_chat_context_copy_response_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        let cr = actions
            .iter()
            .find(|a| a.id == "chat:copy_response")
            .unwrap();
        assert_eq!(cr.shortcut.as_deref(), Some("⌘C"));
    }

    #[test]
    fn cat29_13_ai_vs_chat_copy_response_different_shortcuts() {
        let ai_actions = get_ai_command_bar_actions();
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        // copy_response is in root context actions, not the model picker
        let actions = get_chat_context_actions(&info);
        let ai_cr = ai_actions
            .iter()
            .find(|a| a.id == "chat:copy_response")
            .unwrap();
        let chat_cr = actions
            .iter()
            .find(|a| a.id == "chat:copy_response")
            .unwrap();
        assert_ne!(ai_cr.shortcut, chat_cr.shortcut);
    }

    // =============================================================================
    // Category 14: Chat context — model ID format is "select_model_{id}"
    // =============================================================================

    #[test]
    fn cat29_14_chat_model_id_format() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "claude-3-opus".into(),
                display_name: "Claude 3 Opus".into(),
                provider: "Anthropic".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        assert!(
            picker
                .iter()
                .any(|a| a.id == "chat:select_model_claude-3-opus")
        );
    }

    #[test]
    fn cat29_14_chat_model_title_is_display_name() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-4")
            .unwrap();
        assert_eq!(m.title, "GPT-4");
    }

    #[test]
    fn cat29_14_chat_model_description_via_provider() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-4")
            .unwrap();
        assert_eq!(m.description.as_deref(), Some("Uses OpenAI"));
    }

    // =============================================================================
    // Category 15: File context — open_file title format
    // =============================================================================

    #[test]
    fn cat29_15_file_open_title_quotes_name() {
        let fi = FileInfo {
            path: "/test/doc.pdf".into(),
            name: "doc.pdf".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.title.contains("\"doc.pdf\""));
    }

    #[test]
    fn cat29_15_file_dir_open_title_quotes_name() {
        let fi = FileInfo {
            path: "/test/Documents".into(),
            name: "Documents".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(open.title.contains("\"Documents\""));
    }

    #[test]
    fn cat29_15_file_open_desc_says_default_application() {
        let fi = FileInfo {
            path: "/test/image.png".into(),
            name: "image.png".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("default app")
        );
    }

    #[test]
    fn cat29_15_file_dir_open_desc_says_folder() {
        let fi = FileInfo {
            path: "/test/Docs".into(),
            name: "Docs".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&fi);
        let open = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(
            open.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("folder")
        );
    }

    // =============================================================================
    // Category 16: Path context — select_file vs open_directory description wording
    // =============================================================================

    #[test]
    fn cat29_16_path_select_file_desc_says_submit() {
        let pi = PathInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        let sel = actions.iter().find(|a| a.id == "file:select_file").unwrap();
        assert!(
            sel.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("selects this file")
        );
    }

    #[test]
    fn cat29_16_path_open_directory_desc_says_navigate() {
        let pi = PathInfo {
            path: "/test/folder".into(),
            name: "folder".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        let od = actions
            .iter()
            .find(|a| a.id == "file:open_directory")
            .unwrap();
        assert!(
            od.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("opens this directory")
        );
    }

    #[test]
    fn cat29_16_path_file_has_no_open_directory() {
        let pi = PathInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert!(!actions.iter().any(|a| a.id == "file:open_directory"));
    }

    #[test]
    fn cat29_16_path_dir_has_no_select_file() {
        let pi = PathInfo {
            path: "/test/folder".into(),
            name: "folder".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        assert!(!actions.iter().any(|a| a.id == "file:select_file"));
    }

    // =============================================================================
    // Category 17: to_deeplink_name — preserves numbers and lowercase letters
    // =============================================================================

    #[test]
    fn cat29_17_deeplink_lowercase_preserved() {
        assert_eq!(to_deeplink_name("hello"), "hello");
    }

    #[test]
    fn cat29_17_deeplink_numbers_preserved() {
        assert_eq!(to_deeplink_name("test123"), "test123");
    }

    #[test]
    fn cat29_17_deeplink_mixed_case_lowered() {
        assert_eq!(to_deeplink_name("HelloWorld"), "helloworld");
    }

    #[test]
    fn cat29_17_deeplink_spaces_to_hyphens() {
        assert_eq!(to_deeplink_name("my script name"), "my-script-name");
    }

    // =============================================================================
    // Category 18: format_shortcut_hint (dialog.rs) — combined modifier+key combos
    // =============================================================================

    #[test]
    fn cat29_18_format_hint_cmd_shift_k() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("cmd+shift+k"),
            "⌘⇧K"
        );
    }

    #[test]
    fn cat29_18_format_hint_ctrl_alt_delete() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("ctrl+alt+delete"),
            "⌃⌥⌫"
        );
    }

    #[test]
    fn cat29_18_format_hint_meta_alias() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("meta+c"),
            "⌘C"
        );
    }

    #[test]
    fn cat29_18_format_hint_option_space() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("option+space"),
            "⌥␣"
        );
    }

    #[test]
    fn cat29_18_format_hint_single_enter() {
        assert_eq!(
            super::dialog::ActionsDialog::format_shortcut_hint("enter"),
            "↵"
        );
    }

    // =============================================================================
    // Category 19: parse_shortcut_keycaps — multi-symbol shortcut strings
    // =============================================================================

    #[test]
    fn cat29_19_parse_keycaps_cmd_enter() {
        let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘↵");
        assert_eq!(keycaps, vec!["⌘", "↵"]);
    }

    #[test]
    fn cat29_19_parse_keycaps_all_modifiers_plus_key() {
        let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌃⌥⇧⌘K");
        assert_eq!(keycaps, vec!["⌃", "⌥", "⇧", "⌘", "K"]);
    }

    #[test]
    fn cat29_19_parse_keycaps_space_symbol() {
        let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("␣");
        assert_eq!(keycaps, vec!["␣"]);
    }

    #[test]
    fn cat29_19_parse_keycaps_arrows() {
        let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("↑↓←→");
        assert_eq!(keycaps, vec!["↑", "↓", "←", "→"]);
    }

    // =============================================================================
    // Category 20: score_action — description bonus adds to prefix score
    // =============================================================================

    #[test]
    fn cat29_20_score_prefix_plus_desc_bonus() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            Some("Edit the script in your editor".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = super::dialog::ActionsDialog::score_action(&action, "edit");
        // prefix(100) + desc bonus(15) = 115
        assert!(score >= 115);
    }

    // --- merged from part_03.rs ---

    #[test]
    fn cat29_20_score_prefix_plus_shortcut_bonus() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            None,
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        // search for "e" — prefix match on title "edit script" (100) + shortcut "⌘e" contains "e" (10)
        let score = super::dialog::ActionsDialog::score_action(&action, "e");
        assert!(score >= 110);
    }

    #[test]
    fn cat29_20_score_all_three_bonuses() {
        let action = Action::new(
            "edit_script",
            "Edit Script",
            Some("Edit the file".to_string()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        // "e" => title prefix(100) + desc contains(15) + shortcut contains(10)
        let score = super::dialog::ActionsDialog::score_action(&action, "e");
        assert!(score >= 125);
    }

    // =============================================================================
    // Category 21: fuzzy_match — case sensitivity and edge cases
    // =============================================================================

    #[test]
    fn cat29_21_fuzzy_match_exact() {
        assert!(super::dialog::ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn cat29_21_fuzzy_match_subsequence() {
        assert!(super::dialog::ActionsDialog::fuzzy_match(
            "hello world",
            "hwd"
        ));
    }

    #[test]
    fn cat29_21_fuzzy_match_no_match() {
        assert!(!super::dialog::ActionsDialog::fuzzy_match("hello", "xyz"));
    }

    #[test]
    fn cat29_21_fuzzy_match_empty_needle_matches() {
        assert!(super::dialog::ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn cat29_21_fuzzy_match_needle_longer_fails() {
        assert!(!super::dialog::ActionsDialog::fuzzy_match("hi", "hello"));
    }

    // =============================================================================
    // Category 22: build_grouped_items_static — section headers with Headers style
    // =============================================================================

    #[test]
    fn cat29_22_grouped_items_headers_adds_section_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Group1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Group2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should have: Header("Group1"), Item(0), Header("Group2"), Item(1)
        assert_eq!(grouped.len(), 4);
        assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
        assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
        assert!(matches!(grouped[2], GroupedActionItem::SectionHeader(_)));
        assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
    }

    #[test]
    fn cat29_22_grouped_items_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Group1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Group2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Should have just items, no headers
        assert_eq!(grouped.len(), 2);
        assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
    }

    #[test]
    fn cat29_22_grouped_items_same_section_one_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Same"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Same"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should have: Header("Same"), Item(0), Item(1) — only one header
        assert_eq!(grouped.len(), 3);
        assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
    }

    #[test]
    fn cat29_22_grouped_items_empty_returns_empty() {
        let actions: Vec<Action> = vec![];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // =============================================================================
    // Category 23: coerce_action_selection — header skipping behavior
    // =============================================================================

    #[test]
    fn cat29_23_coerce_on_item_stays() {
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn cat29_23_coerce_on_header_jumps_down() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn cat29_23_coerce_trailing_header_jumps_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn cat29_23_coerce_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn cat29_23_coerce_empty_returns_none() {
        let rows: Vec<GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // =============================================================================
    // Category 24: CommandBarConfig — all presets preserve close defaults
    // =============================================================================

    #[test]
    fn cat29_24_ai_style_close_on_select() {
        let config = CommandBarConfig::ai_style();
        assert!(config.close_on_select);
    }

    #[test]
    fn cat29_24_main_menu_close_on_escape() {
        let config = CommandBarConfig::main_menu_style();
        assert!(config.close_on_escape);
    }

    #[test]
    fn cat29_24_no_search_close_on_click_outside() {
        let config = CommandBarConfig::no_search();
        assert!(config.close_on_click_outside);
    }

    #[test]
    fn cat29_24_notes_style_close_defaults() {
        let config = CommandBarConfig::notes_style();
        assert!(config.close_on_select);
        assert!(config.close_on_escape);
        assert!(config.close_on_click_outside);
    }

    // =============================================================================
    // Category 25: CommandBarConfig — show_icons and show_footer combinations
    // =============================================================================

    #[test]
    fn cat29_25_ai_style_has_icons_and_footer() {
        let config = CommandBarConfig::ai_style();
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn cat29_25_main_menu_no_icons_no_footer() {
        let config = CommandBarConfig::main_menu_style();
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn cat29_25_notes_style_has_icons_and_footer() {
        let config = CommandBarConfig::notes_style();
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn cat29_25_no_search_no_icons_no_footer() {
        let config = CommandBarConfig::no_search();
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    // =============================================================================
    // Category 26: New chat — empty inputs produce empty actions
    // =============================================================================

    #[test]
    fn cat29_26_new_chat_all_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn cat29_26_new_chat_only_last_used() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "P".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn cat29_26_new_chat_only_presets() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    #[test]
    fn cat29_26_new_chat_only_models() {
        let models = vec![NewChatModelInfo {
            model_id: "m2".into(),
            display_name: "Model 2".into(),
            provider: "P".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    // =============================================================================
    // Category 27: Scriptlet context with_custom — reset_ranking is always last
    // =============================================================================

    #[test]
    fn cat29_27_scriptlet_frecency_reset_ranking_last() {
        let script =
            ScriptInfo::scriptlet("S", "/p.md", None, None).with_frecency(true, Some("s".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let last = actions.last().unwrap();
        assert_eq!(last.id, "reset_ranking");
    }

    #[test]
    fn cat29_27_script_frecency_reset_ranking_last() {
        let script = ScriptInfo::new("S", "/p.ts").with_frecency(true, Some("s".into()));
        let actions = get_script_context_actions(&script);
        let last = actions.last().unwrap();
        assert_eq!(last.id, "reset_ranking");
    }

    #[test]
    fn cat29_27_builtin_frecency_reset_ranking_last() {
        let script = ScriptInfo::builtin("CH").with_frecency(true, Some("ch".into()));
        let actions = get_script_context_actions(&script);
        let last = actions.last().unwrap();
        assert_eq!(last.id, "reset_ranking");
    }

    // =============================================================================
    // Category 28: Cross-context — all built-in actions have ActionCategory::ScriptContext
    // =============================================================================

    #[test]
    fn cat29_28_script_all_script_context() {
        let script = ScriptInfo::new("S", "/p.ts");
        let actions = get_script_context_actions(&script);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "action {} has wrong category",
                a.id
            );
        }
    }

    #[test]
    fn cat29_28_clipboard_all_script_context() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "action {} has wrong category",
                a.id
            );
        }
    }

    #[test]
    fn cat29_28_ai_all_script_context() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "action {} has wrong category",
                a.id
            );
        }
    }

    #[test]
    fn cat29_28_path_all_script_context() {
        let pi = PathInfo {
            path: "/p".into(),
            name: "p".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "action {} has wrong category",
                a.id
            );
        }
    }

    // =============================================================================
    // Category 29: Cross-context — first action ID is always the primary action
    // =============================================================================

    #[test]
    fn cat29_29_script_first_is_run_script() {
        let script = ScriptInfo::new("S", "/p.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn cat29_29_clipboard_first_is_paste() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
    }

    #[test]
    fn cat29_29_path_file_first_is_select_file() {
        let pi = PathInfo {
            path: "/p/f.txt".into(),
            name: "f.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn cat29_29_path_dir_first_is_open_directory() {
        let pi = PathInfo {
            path: "/p/dir".into(),
            name: "dir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn cat29_29_file_first_is_open() {
        let fi = FileInfo {
            path: "/p/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        assert_eq!(actions[0].id, "file:open_file");
    }

    // =============================================================================
    // Category 30: Action builder — chaining preserves all fields correctly
    // =============================================================================

    #[test]
    fn cat29_30_action_new_defaults() {
        let a = Action::new(
            "id",
            "Title",
            Some("Desc".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(a.id, "id");
        assert_eq!(a.title, "Title");
        assert_eq!(a.description.as_deref(), Some("Desc"));
        assert!(!a.has_action);
        assert!(a.shortcut.is_none());
        assert!(a.icon.is_none());
        assert!(a.section.is_none());
        assert!(a.value.is_none());
    }

    // --- merged from part_04.rs ---

    #[test]
    fn cat29_30_action_full_chain() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘X")
            .with_icon(IconName::Trash)
            .with_section("Danger");
        assert_eq!(a.shortcut.as_deref(), Some("⌘X"));
        assert_eq!(a.icon, Some(IconName::Trash));
        assert_eq!(a.section.as_deref(), Some("Danger"));
    }

    #[test]
    fn cat29_30_action_title_lower_computed() {
        let a = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "hello world");
    }

    #[test]
    fn cat29_30_action_description_lower_computed() {
        let a = Action::new(
            "id",
            "T",
            Some("FoO BaR".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(a.description_lower.as_deref(), Some("foo bar"));
    }

    #[test]
    fn cat29_30_action_shortcut_lower_computed() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        assert_eq!(a.shortcut_lower.as_deref(), Some("⌘e"));
    }
}

mod from_dialog_builtin_action_validation_tests_30 {
    // --- merged from part_01.rs ---
    //! Batch 30: Builtin action validation tests
    //!
    //! 30 categories validating random built-in action behaviors across
    //! script, clipboard, file, path, AI, notes, chat, and new-chat contexts.

    use crate::actions::builders::{
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
        get_chat_model_picker_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom, to_deeplink_name,
    };
    use crate::actions::command_bar::CommandBarConfig;
    use crate::actions::dialog::{
        ActionsDialog, build_grouped_items_static, coerce_action_selection,
    };
    use crate::actions::types::{
        Action, ActionCategory, AnchorPosition, SearchPosition, SectionStyle,
    };
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // ---------------------------------------------------------------------------
    // 1. Script context: copy_content description wording is consistent
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_script_copy_content_desc_says_entire_file() {
        let script = crate::actions::types::ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(
            cc.description.as_ref().unwrap().contains("entire file"),
            "copy_content desc should mention 'entire file', got: {:?}",
            cc.description
        );
    }

    #[test]
    fn batch30_scriptlet_copy_content_desc_says_entire_file() {
        let script = crate::actions::types::ScriptInfo::scriptlet("x", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(cc.description.as_ref().unwrap().contains("entire file"));
    }

    #[test]
    fn batch30_agent_copy_content_desc_says_entire_file() {
        let mut script = crate::actions::types::ScriptInfo::new("agent", "/p/agent.ts");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(cc.description.as_ref().unwrap().contains("entire file"));
    }

    #[test]
    fn batch30_builtin_has_no_copy_content() {
        let script = crate::actions::types::ScriptInfo::builtin("Settings");
        let actions = get_script_context_actions(&script);
        assert!(!actions.iter().any(|a| a.id == "copy_content"));
    }

    // ---------------------------------------------------------------------------
    // 2. Clipboard: image-only actions absent for text entries
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_clipboard_text_no_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hi".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn batch30_clipboard_text_no_open_with() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
    }

    #[test]
    fn batch30_clipboard_text_no_annotate_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            !actions
                .iter()
                .any(|a| a.id == "clip:clipboard_annotate_cleanshot")
        );
    }

    #[test]
    fn batch30_clipboard_text_no_upload_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            !actions
                .iter()
                .any(|a| a.id == "clip:clipboard_upload_cleanshot")
        );
    }

    // ---------------------------------------------------------------------------
    // 3. Clipboard: image entry has OCR and macOS image actions
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_clipboard_image_has_ocr() {
        let entry = ClipboardEntryInfo {
            id: "i".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch30_clipboard_image_has_open_with_macos() {
        let entry = ClipboardEntryInfo {
            id: "i".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch30_clipboard_image_has_annotate_cleanshot_macos() {
        let entry = ClipboardEntryInfo {
            id: "i".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_annotate_cleanshot")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch30_clipboard_image_annotate_cleanshot_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "i".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let a = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_annotate_cleanshot")
            .unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⇧⌘A"));
    }

    // ---------------------------------------------------------------------------
    // 4. File context: directory has no quick_look on macOS
    // ---------------------------------------------------------------------------
    #[cfg(target_os = "macos")]
    #[test]
    fn batch30_file_dir_no_quick_look() {
        let info = FileInfo {
            path: "/tmp/dir".into(),
            name: "dir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&info);
        assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch30_file_regular_has_quick_look() {
        let info = FileInfo {
            path: "/tmp/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:quick_look"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn batch30_file_quick_look_shortcut_is_cmd_y() {
        let info = FileInfo {
            path: "/tmp/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        let ql = actions.iter().find(|a| a.id == "file:quick_look").unwrap();
        assert_eq!(ql.shortcut.as_deref(), Some("⌘Y"));
    }

    #[test]
    fn batch30_file_dir_has_open_directory() {
        let info = FileInfo {
            path: "/tmp/dir".into(),
            name: "dir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:open_directory"));
        assert!(!actions.iter().any(|a| a.id == "file:open_file"));
    }

    // ---------------------------------------------------------------------------
    // 5. Path context: total action count for file vs dir
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_path_file_has_7_actions() {
        let info = PathInfo {
            path: "/tmp/f.txt".into(),
            name: "f.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        assert_eq!(
            actions.len(),
            7,
            "Path file should have 7 actions: select_file, copy_path, open_in_finder, open_in_editor, open_in_quick_terminal, copy_filename, move_to_trash"
        );
    }

    #[test]
    fn batch30_path_dir_has_7_actions() {
        let info = PathInfo {
            path: "/tmp/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&info);
        assert_eq!(
            actions.len(),
            7,
            "Path dir should have 7 actions: open_directory, copy_path, open_in_finder, open_in_editor, open_in_quick_terminal, copy_filename, move_to_trash"
        );
    }

    #[test]
    fn batch30_path_file_first_action_is_select_file() {
        let info = PathInfo {
            path: "/tmp/f.txt".into(),
            name: "f.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn batch30_path_dir_first_action_is_open_directory() {
        let info = PathInfo {
            path: "/tmp/d".into(),
            name: "d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    // ---------------------------------------------------------------------------
    // 6. Path context: open_in_quick_terminal ships without a shortcut
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_path_open_in_quick_terminal_shortcut() {
        let info = PathInfo {
            path: "/tmp/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        let t = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        assert_eq!(t.shortcut.as_deref(), None);
    }

    #[test]
    fn batch30_path_open_in_quick_terminal_desc_mentions_terminal() {
        let info = PathInfo {
            path: "/tmp/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        let t = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        assert!(
            t.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("terminal")
        );
    }

    #[test]
    fn batch30_path_open_in_finder_shortcut() {
        let info = PathInfo {
            path: "/tmp/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        let f = actions
            .iter()
            .find(|a| a.id == "file:open_in_finder")
            .unwrap();
        assert_eq!(f.shortcut.as_deref(), Some("⌘⇧F"));
    }

    #[test]
    fn batch30_path_copy_path_shortcut() {
        let info = PathInfo {
            path: "/tmp/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
    }

    // ---------------------------------------------------------------------------
    // 7. AI command bar: all 21 actions have unique IDs
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_ai_bar_12_actions() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    #[test]
    fn batch30_ai_bar_all_ids_unique() {
        let actions = get_ai_command_bar_actions();
        let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 38, "All 38 AI bar action IDs must be unique");
    }

    #[test]
    fn batch30_ai_bar_all_have_section() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(
                a.section.is_some(),
                "AI bar action '{}' should have a section",
                a.id
            );
        }
    }

    #[test]
    fn batch30_ai_bar_all_have_icon() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(
                a.icon.is_some(),
                "AI bar action '{}' should have an icon",
                a.id
            );
        }
    }

    // ---------------------------------------------------------------------------
    // 8. AI command bar: branch_from_last has no shortcut
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_ai_bar_branch_from_last_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let b = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert!(b.shortcut.is_none());
    }

    #[test]
    fn batch30_ai_bar_change_model_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(cm.shortcut.is_none());
    }

    #[test]
    fn batch30_ai_bar_branch_from_last_icon_arrowright() {
        let actions = get_ai_command_bar_actions();
        let b = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert_eq!(b.icon, Some(IconName::ArrowRight));
    }

    #[test]
    fn batch30_ai_bar_change_model_icon_settings() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert_eq!(cm.icon, Some(IconName::Settings));
    }

    // --- merged from part_02.rs ---

    // ---------------------------------------------------------------------------
    // 9. Chat context: current model gets ✓ suffix
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_chat_current_model_has_check() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(m.title.contains("✓"), "Current model title should have ✓");
    }

    #[test]
    fn batch30_chat_non_current_model_no_check() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".into()),
            available_models: vec![ChatModelInfo {
                id: "claude".into(),
                display_name: "Claude".into(),
                provider: "Anthropic".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m = picker
            .iter()
            .find(|a| a.id == "chat:select_model_claude")
            .unwrap();
        assert!(!m.title.contains("✓"));
    }

    #[test]
    fn batch30_chat_no_current_model_no_check() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(!m.title.contains("✓"));
    }

    #[test]
    fn batch30_chat_model_desc_says_via_provider() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "c3".into(),
                display_name: "Claude 3".into(),
                provider: "Anthropic".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let m = picker
            .iter()
            .find(|a| a.id == "chat:select_model_c3")
            .unwrap();
        assert_eq!(m.description.as_deref(), Some("Uses Anthropic"));
    }

    // ---------------------------------------------------------------------------
    // 10. Notes command bar: new_note always present
    // ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------
    // 11. Notes command bar: full mode action count
    // ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------
    // 12. Note switcher: pinned note gets StarFilled icon
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_note_switcher_pinned_icon_star_filled() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Pinned Note".into(),
            char_count: 50,
            is_current: false,
            is_pinned: true,
            preview: "Some preview".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn batch30_note_switcher_pinned_section() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "P".into(),
            char_count: 10,
            is_current: false,
            is_pinned: true,
            preview: "x".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn batch30_note_switcher_pinned_and_current_icon_is_star() {
        // pinned trumps current for icon
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Both".into(),
            char_count: 10,
            is_current: true,
            is_pinned: true,
            preview: "x".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    #[test]
    fn batch30_note_switcher_regular_icon_file() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Regular".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "x".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    // ---------------------------------------------------------------------------
    // 13. Note switcher: preview truncation boundary at 60 chars
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_note_switcher_60_chars_no_truncation() {
        let preview = "a".repeat(60);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 60,
            is_current: false,
            is_pinned: false,
            preview,
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(!desc.contains("…"), "Exactly 60 chars should not truncate");
    }

    #[test]
    fn batch30_note_switcher_61_chars_truncated() {
        let preview = "a".repeat(61);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 61,
            is_current: false,
            is_pinned: false,
            preview,
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.contains("…"), "61 chars should truncate with …");
    }

    #[test]
    fn batch30_note_switcher_short_preview_no_truncation() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: "hello".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(!desc.contains("…"));
        assert!(desc.contains("hello"));
    }

    // ---------------------------------------------------------------------------
    // 14. Note switcher: empty preview falls back to char count
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_note_switcher_empty_preview_empty_time_shows_chars() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
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
    fn batch30_note_switcher_empty_preview_with_time_shows_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "3d ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert_eq!(desc, "3d ago");
    }

    #[test]
    fn batch30_note_switcher_singular_char() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
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
    fn batch30_note_switcher_zero_chars() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "T".into(),
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

    // ---------------------------------------------------------------------------
    // 15. New chat: empty inputs produce empty results
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_new_chat_all_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn batch30_new_chat_only_models() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    #[test]
    fn batch30_new_chat_only_presets() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    #[test]
    fn batch30_new_chat_only_last_used() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "M1".into(),
            provider: "p".into(),
            provider_display_name: "Prov".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    // ---------------------------------------------------------------------------
    // 16. New chat: section ordering is last_used → presets → models
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_new_chat_section_ordering() {
        let last_used = vec![NewChatModelInfo {
            model_id: "lu".into(),
            display_name: "LU".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "gen".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m".into(),
            display_name: "M".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
        assert_eq!(actions[1].section.as_deref(), Some("Presets"));
        assert_eq!(actions[2].section.as_deref(), Some("Models"));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn batch30_new_chat_preset_desc_is_none() {
        let presets = vec![NewChatPresetInfo {
            id: "gen".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses General preset")
        );
    }

    #[test]
    fn batch30_new_chat_last_used_desc_is_provider() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m".into(),
            display_name: "M".into(),
            provider: "p".into(),
            provider_display_name: "MyProvider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].description.as_deref(), Some("Uses MyProvider"));
    }

    #[test]
    fn batch30_new_chat_model_desc_is_provider() {
        let models = vec![NewChatModelInfo {
            model_id: "m".into(),
            display_name: "M".into(),
            provider: "p".into(),
            provider_display_name: "ProvDisplay".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].description.as_deref(), Some("Uses ProvDisplay"));
    }

    // ---------------------------------------------------------------------------
    // 17. to_deeplink_name: various edge cases
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_deeplink_name_unicode_preserved() {
        // non-ASCII characters are percent-encoded
        let result = to_deeplink_name("café");
        assert_eq!(result, "caf%C3%A9");
    }

    #[test]
    fn batch30_deeplink_name_all_special_chars() {
        let result = to_deeplink_name("!@#$%^");
        assert_eq!(result, "_unnamed");
    }

    #[test]
    fn batch30_deeplink_name_mixed_case_lowered() {
        let result = to_deeplink_name("MyScript");
        assert_eq!(result, "myscript");
    }

    #[test]
    fn batch30_deeplink_name_numbers_preserved() {
        let result = to_deeplink_name("test123");
        assert_eq!(result, "test123");
    }

    // ---------------------------------------------------------------------------
    // 18. Script context: action verb propagates to run_script title
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_script_verb_run_default() {
        let script = crate::actions::types::ScriptInfo::new("foo", "/p/foo.ts");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Run");
    }

    #[test]
    fn batch30_script_verb_launch() {
        let script = crate::actions::types::ScriptInfo::with_action_verb(
            "Safari",
            "/Applications/Safari.app",
            false,
            "Launch",
        );
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Launch");
    }

    #[test]
    fn batch30_script_verb_switch_to() {
        let script = crate::actions::types::ScriptInfo::with_action_verb(
            "Preview",
            "window:1",
            false,
            "Switch to",
        );
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Switch To");
    }

    #[test]
    fn batch30_script_verb_desc_uses_verb() {
        let script = crate::actions::types::ScriptInfo::with_action_verb("X", "/p", false, "Open");
        let actions = get_script_context_actions(&script);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.description.as_deref(), Some("Open this item"));
    }

    // ---------------------------------------------------------------------------
    // 19. Script context: deeplink URL in copy_deeplink description
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_deeplink_desc_contains_url() {
        let script = crate::actions::types::ScriptInfo::new("My Cool Script", "/p.ts");
        let actions = get_script_context_actions(&script);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.title, "Share");
        assert_eq!(
            dl.description.as_deref(),
            Some("Copy a portable Script Kit share link to clipboard")
        );
    }

    #[test]
    fn batch30_deeplink_shortcut_is_cmd_shift_d() {
        let script = crate::actions::types::ScriptInfo::new("X", "/p.ts");
        let actions = get_script_context_actions(&script);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.shortcut.as_deref(), Some("⌘⇧D"));
    }

    #[test]
    fn batch30_deeplink_desc_for_builtin() {
        let script = crate::actions::types::ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&script);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert!(
            dl.description
                .as_ref()
                .unwrap()
                .contains("scriptkit://run/clipboard-history")
        );
    }

    // ---------------------------------------------------------------------------
    // 20. CommandBarConfig: notes_style matches expected fields
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_command_bar_notes_style_search_top() {
        let cfg = CommandBarConfig::notes_style();
        assert!(matches!(
            cfg.dialog_config.search_position,
            SearchPosition::Top
        ));
    }

    #[test]
    fn batch30_command_bar_notes_style_section_separators() {
        let cfg = CommandBarConfig::notes_style();
        assert!(matches!(
            cfg.dialog_config.section_style,
            SectionStyle::Headers
        ));
    }

    #[test]
    fn batch30_command_bar_notes_style_anchor_top() {
        let cfg = CommandBarConfig::notes_style();
        assert!(matches!(cfg.dialog_config.anchor, AnchorPosition::Top));
    }

    #[test]
    fn batch30_command_bar_notes_style_show_icons_and_footer() {
        let cfg = CommandBarConfig::notes_style();
        assert!(cfg.dialog_config.show_icons);
        assert!(!cfg.dialog_config.show_footer);
    }

    // ---------------------------------------------------------------------------
    // 21. parse_shortcut_keycaps: modifier+letter combos
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_parse_keycaps_cmd_c() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘C");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    #[test]
    fn batch30_parse_keycaps_cmd_shift_a() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧A");
        assert_eq!(caps, vec!["⌘", "⇧", "A"]);
    }

    #[test]
    fn batch30_parse_keycaps_enter_alone() {
        let caps = ActionsDialog::parse_shortcut_keycaps("↵");
        assert_eq!(caps, vec!["↵"]);
    }

    #[test]
    fn batch30_parse_keycaps_all_modifiers_plus_key() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌃⌥⇧⌘K");
        assert_eq!(caps, vec!["⌃", "⌥", "⇧", "⌘", "K"]);
    }

    // ---------------------------------------------------------------------------
    // 22. score_action: various match scenarios
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_score_prefix_match_gte_100() {
        let action = Action::new(
            "e",
            "Edit Script",
            Some("Open in $EDITOR".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(score >= 100, "Prefix match should be ≥100, got {}", score);
    }

    #[test]
    fn batch30_score_contains_match_50_to_99() {
        let action = Action::new("c", "Copy Edit Path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(
            (50..100).contains(&score),
            "Contains match should be 50..99, got {}",
            score
        );
    }

    #[test]
    fn batch30_score_no_match_is_zero() {
        let action = Action::new("x", "Run Script", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "zzz");
        assert_eq!(score, 0);
    }

    #[test]
    fn batch30_score_empty_search_is_prefix() {
        let action = Action::new("x", "Hello", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        assert!(score >= 100, "Empty search is prefix match, got {}", score);
    }

    // ---------------------------------------------------------------------------
    // 23. fuzzy_match: edge cases
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_fuzzy_exact_match() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn batch30_fuzzy_subsequence() {
        assert!(ActionsDialog::fuzzy_match("hello world", "hwo"));
    }

    #[test]
    fn batch30_fuzzy_no_match() {
        assert!(!ActionsDialog::fuzzy_match("abc", "xyz"));
    }

    #[test]
    fn batch30_fuzzy_needle_longer_than_haystack() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abcdef"));
    }

    #[test]
    fn batch30_fuzzy_empty_needle() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    // ---------------------------------------------------------------------------
    // 24. build_grouped_items_static: Headers vs Separators behavior
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_grouped_headers_adds_section_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered = vec![0, 1];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should be: Header("S1"), Item(0), Header("S2"), Item(1) = 4 items
        assert_eq!(grouped.len(), 4);
    }

    #[test]
    fn batch30_grouped_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered = vec![0, 1];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Should be: Item(0), Item(1) = 2 items (no headers)
        assert_eq!(grouped.len(), 2);
    }

    #[test]
    fn batch30_grouped_empty_returns_empty() {
        let grouped = build_grouped_items_static(&[], &[], SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    #[test]
    fn batch30_grouped_same_section_one_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S"),
        ];
        let filtered = vec![0, 1];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should be: Header("S"), Item(0), Item(1) = 3 items
        assert_eq!(grouped.len(), 3);
    }

    // ---------------------------------------------------------------------------
    // 25. coerce_action_selection: header skipping
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_coerce_on_item_stays() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn batch30_coerce_on_header_jumps_down() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn batch30_coerce_trailing_header_jumps_up() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn batch30_coerce_all_headers_none() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::SectionHeader("H1".into()),
            GroupedActionItem::SectionHeader("H2".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn batch30_coerce_empty_none() {
        assert_eq!(coerce_action_selection(&[], 0), None);
    }

    // ---------------------------------------------------------------------------
    // 26. Clipboard: destructive actions ordering invariant
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_clipboard_destructive_always_last_three() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
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
    fn batch30_clipboard_image_destructive_also_last_three() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "i".into(),
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

    #[test]
    fn batch30_clipboard_delete_all_desc_mentions_pinned() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let da = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete_all")
            .unwrap();
        assert!(
            da.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("pinned")
        );
    }

    // ---------------------------------------------------------------------------
    // 27. Script context: agent has specific action set
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_agent_has_edit_script_title_edit_agent() {
        let mut script = crate::actions::types::ScriptInfo::new("a", "/p");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn batch30_agent_has_no_view_logs() {
        let mut script = crate::actions::types::ScriptInfo::new("a", "/p");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // --- merged from part_04.rs ---

    #[test]
    fn batch30_agent_has_reveal_in_finder() {
        let mut script = crate::actions::types::ScriptInfo::new("a", "/p");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
    }

    #[test]
    fn batch30_agent_has_copy_path() {
        let mut script = crate::actions::types::ScriptInfo::new("a", "/p");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "copy_path"));
    }

    // ---------------------------------------------------------------------------
    // 28. Action builder: cached lowercase fields
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_action_title_lower_precomputed() {
        let action = Action::new("x", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "hello world");
    }

    #[test]
    fn batch30_action_description_lower_precomputed() {
        let action = Action::new(
            "x",
            "T",
            Some("Open in $EDITOR".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower.as_deref(), Some("open in $editor"));
    }

    #[test]
    fn batch30_action_shortcut_lower_after_with_shortcut() {
        let action =
            Action::new("x", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘⇧c"));
    }

    #[test]
    fn batch30_action_no_shortcut_lower_is_none() {
        let action = Action::new("x", "T", None, ActionCategory::ScriptContext);
        assert!(action.shortcut_lower.is_none());
    }

    // ---------------------------------------------------------------------------
    // 29. Action builder: with_icon and with_section
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_action_with_icon_sets_field() {
        let action =
            Action::new("x", "T", None, ActionCategory::ScriptContext).with_icon(IconName::Star);
        assert_eq!(action.icon, Some(IconName::Star));
    }

    #[test]
    fn batch30_action_new_no_icon() {
        let action = Action::new("x", "T", None, ActionCategory::ScriptContext);
        assert!(action.icon.is_none());
    }

    #[test]
    fn batch30_action_with_section_sets_field() {
        let action =
            Action::new("x", "T", None, ActionCategory::ScriptContext).with_section("MySection");
        assert_eq!(action.section.as_deref(), Some("MySection"));
    }

    #[test]
    fn batch30_action_new_no_section() {
        let action = Action::new("x", "T", None, ActionCategory::ScriptContext);
        assert!(action.section.is_none());
    }

    // ---------------------------------------------------------------------------
    // 30. Cross-context: all built-in actions have has_action=false
    // ---------------------------------------------------------------------------
    #[test]
    fn batch30_cross_context_script_actions_has_action_false() {
        let script = crate::actions::types::ScriptInfo::new("s", "/p.ts");
        let actions = get_script_context_actions(&script);
        for a in &actions {
            assert!(
                !a.has_action,
                "Script action '{}' should have has_action=false",
                a.id
            );
        }
    }

    #[test]
    fn batch30_cross_context_clipboard_actions_has_action_false() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for a in &actions {
            assert!(
                !a.has_action,
                "Clipboard action '{}' should have has_action=false",
                a.id
            );
        }
    }

    #[test]
    fn batch30_cross_context_file_actions_has_action_false() {
        let info = FileInfo {
            path: "/f.txt".into(),
            name: "f.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        for a in &actions {
            assert!(
                !a.has_action,
                "File action '{}' should have has_action=false",
                a.id
            );
        }
    }

    #[test]
    fn batch30_cross_context_path_actions_has_action_false() {
        let info = PathInfo {
            path: "/f".into(),
            name: "f".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&info);
        for a in &actions {
            assert!(
                !a.has_action,
                "Path action '{}' should have has_action=false",
                a.id
            );
        }
    }

    #[test]
    fn batch30_cross_context_ai_bar_actions_has_action_false() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(
                !a.has_action,
                "AI bar action '{}' should have has_action=false",
                a.id
            );
        }
    }

}
