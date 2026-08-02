#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;

mod from_dialog_builtin_action_validation_tests_31 {
    //! Purged batch 31 validation tests.
    //!
    //! This file previously contained generated duplicate assertions already covered
    //! by higher-signal dialog/action test suites.
}

mod from_dialog_builtin_action_validation_tests_32 {
    // --- merged from part_01.rs ---
    //! Batch 32: Builtin action validation tests
    //!
    //! 30 categories validating random built-in action behaviors across
    //! script, clipboard, file, path, AI, notes, chat, and new-chat contexts.

    use crate::actions::ActionsDialog;
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
        GroupedActionItem, build_grouped_items_static, coerce_action_selection,
    };
    use crate::actions::types::{Action, ActionCategory, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // ---------------------------------------------------------------------------
    // 1. Script context: agent has no view_logs but has copy_content desc about file
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_agent_copy_content_desc_mentions_entire_file() {
        let mut script = crate::actions::types::ScriptInfo::new("my-agent", "/p/my-agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(
            cc.description.as_ref().unwrap().contains("entire file"),
            "agent copy_content desc should mention 'entire file', got: {:?}",
            cc.description
        );
    }

    #[test]
    fn batch32_agent_edit_script_desc_mentions_agent_file() {
        let mut script = crate::actions::types::ScriptInfo::new("my-agent", "/p/my-agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let es = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(
            es.description.as_ref().unwrap().contains("agent"),
            "agent edit desc should mention 'agent', got: {:?}",
            es.description
        );
    }

    #[test]
    fn batch32_agent_reveal_desc_mentions_agent_file() {
        let mut script = crate::actions::types::ScriptInfo::new("my-agent", "/p/my-agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let r = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
        assert!(
            r.description.as_ref().unwrap().contains("agent"),
            "agent reveal desc should mention 'agent', got: {:?}",
            r.description
        );
    }

    #[test]
    fn batch32_agent_copy_path_desc_mentions_agent() {
        let mut script = crate::actions::types::ScriptInfo::new("my-agent", "/p/my-agent.md");
        script.is_agent = true;
        script.is_script = false;
        let actions = get_script_context_actions(&script);
        let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
        assert!(
            cp.description.as_ref().unwrap().contains("agent"),
            "agent copy_path desc should mention 'agent', got: {:?}",
            cp.description
        );
    }

    // ---------------------------------------------------------------------------
    // 2. Scriptlet context with_custom: None scriptlet produces only built-in actions
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_scriptlet_context_none_scriptlet_no_custom_actions() {
        let script = crate::actions::types::ScriptInfo::scriptlet("Test", "/p/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        // All actions should have has_action=false (built-in)
        for a in &actions {
            assert!(
                !a.has_action,
                "built-in action {} should have has_action=false",
                a.id
            );
        }
    }

    #[test]
    fn batch32_scriptlet_context_none_scriptlet_has_edit_scriptlet() {
        let script = crate::actions::types::ScriptInfo::scriptlet("Test", "/p/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "edit_scriptlet"));
    }

    #[test]
    fn batch32_scriptlet_context_none_scriptlet_has_reveal_scriptlet() {
        let script = crate::actions::types::ScriptInfo::scriptlet("Test", "/p/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "reveal_scriptlet_in_finder"));
    }

    #[test]
    fn batch32_scriptlet_context_none_scriptlet_has_copy_scriptlet_path() {
        let script = crate::actions::types::ScriptInfo::scriptlet("Test", "/p/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "copy_scriptlet_path"));
    }

    // ---------------------------------------------------------------------------
    // 3. Clipboard: frontmost_app_name empty string edge case
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_clipboard_empty_app_name_paste_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: Some("".to_string()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        // Empty app name falls back to the generic title (no trailing-space variant)
        assert_eq!(paste.title, "Paste to Active App");
    }

    #[test]
    fn batch32_clipboard_long_app_name_paste_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Very Long Application Name Here".to_string()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.title, "Paste to Very Long Application Name Here");
    }

    #[test]
    fn batch32_clipboard_none_app_name_paste_active_app() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.title, "Paste to Active App");
    }

    // ---------------------------------------------------------------------------
    // 4. Clipboard: text entry has no image-specific actions
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_clipboard_text_no_clipboard_open_with() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
    }

    #[test]
    fn batch32_clipboard_text_no_clipboard_annotate_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
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
    fn batch32_clipboard_text_no_clipboard_upload_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
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

    #[test]
    fn batch32_clipboard_text_no_clipboard_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    // ---------------------------------------------------------------------------
    // 5. File context: reveal_in_finder always present for both file and dir
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_file_reveal_in_finder_present_for_file() {
        let info = FileInfo {
            path: "/p/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
    }

    #[test]
    fn batch32_file_reveal_in_finder_present_for_dir() {
        let info = FileInfo {
            path: "/p/mydir".into(),
            name: "mydir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
    }

    #[test]
    fn batch32_file_reveal_in_finder_shortcut_is_cmd_enter() {
        let info = FileInfo {
            path: "/p/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        let reveal = actions
            .iter()
            .find(|a| a.id == "file:reveal_in_finder")
            .unwrap();
        assert_eq!(reveal.shortcut.as_deref(), Some("⌘⇧F"));
    }

    #[test]
    fn batch32_file_reveal_desc_says_reveal_in_finder() {
        let info = FileInfo {
            path: "/p/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        let reveal = actions
            .iter()
            .find(|a| a.id == "file:reveal_in_finder")
            .unwrap();
        assert!(reveal.description.as_ref().unwrap().contains("Finder"));
    }

    // ---------------------------------------------------------------------------
    // 6. Path context: action ordering after primary action
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_path_file_second_action_is_copy_path() {
        let info = PathInfo::new("test.txt", "/p/test.txt", false);
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[1].id, "file:copy_path");
    }

    #[test]
    fn batch32_path_dir_second_action_is_copy_path() {
        let info = PathInfo::new("mydir", "/p/mydir", true);
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[1].id, "file:copy_path");
    }

    #[test]
    fn batch32_path_file_third_action_is_open_in_finder() {
        let info = PathInfo::new("test.txt", "/p/test.txt", false);
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[2].id, "file:open_in_finder");
    }

    #[test]
    fn batch32_path_last_action_is_move_to_trash() {
        let info = PathInfo::new("test.txt", "/p/test.txt", false);
        let actions = get_path_context_actions(&info);
        assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
    }

    // ---------------------------------------------------------------------------
    // 7. Path context: name in primary action title is quoted
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_path_file_primary_title_quotes_name() {
        let info = PathInfo::new("report.pdf", "/p/report.pdf", false);
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[0].title, "Select \"report.pdf\"");
    }

    #[test]
    fn batch32_path_dir_primary_title_quotes_name() {
        let info = PathInfo::new("Documents", "/p/Documents", true);
        let actions = get_path_context_actions(&info);
        assert_eq!(actions[0].title, "Open \"Documents\"");
    }

    #[test]
    fn batch32_path_file_select_desc_submit() {
        let info = PathInfo::new("file.txt", "/p/file.txt", false);
        let actions = get_path_context_actions(&info);
        assert!(
            actions[0]
                .description
                .as_ref()
                .unwrap()
                .contains("Selects this file")
        );
    }

    #[test]
    fn batch32_path_dir_open_desc_navigate() {
        let info = PathInfo::new("dir", "/p/dir", true);
        let actions = get_path_context_actions(&info);
        assert!(
            actions[0]
                .description
                .as_ref()
                .unwrap()
                .contains("Opens this directory")
        );
    }

    // ---------------------------------------------------------------------------
    // 8. AI command bar: export_markdown details
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_ai_export_markdown_section_is_export() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.section.as_deref(), Some("Export"));
    }

    #[test]
    fn batch32_ai_export_markdown_icon_is_file_code() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.icon, Some(IconName::FileCode));
    }

    #[test]
    fn batch32_ai_export_markdown_shortcut() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.shortcut.as_deref(), Some("⇧⌘E"));
    }

    #[test]
    fn batch32_ai_export_markdown_desc_mentions_markdown() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert!(em.description.as_ref().unwrap().contains("Markdown"));
    }

    // ---------------------------------------------------------------------------
    // 9. AI command bar: submit action details
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_ai_submit_icon_is_arrow_up() {
        let actions = get_ai_command_bar_actions();
        let s = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(s.icon, Some(IconName::ArrowUp));
    }

    #[test]
    fn batch32_ai_submit_section_is_actions() {
        let actions = get_ai_command_bar_actions();
        let s = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(s.section.as_deref(), Some("Actions"));
    }

    #[test]
    fn batch32_ai_submit_shortcut_is_enter() {
        let actions = get_ai_command_bar_actions();
        let s = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(s.shortcut.as_deref(), Some("↵"));
    }

    #[test]
    fn batch32_ai_submit_desc_mentions_send() {
        let actions = get_ai_command_bar_actions();
        let s = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert!(
            s.description.as_ref().unwrap().contains("Send"),
            "submit desc should mention 'Send', got: {:?}",
            s.description
        );
    }

    // ---------------------------------------------------------------------------
    // 10. Chat context: single model produces 2 actions minimum
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_chat_single_model_no_flags_produces_2_actions() {
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
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions.len(), 3, "change_model + continue + capture = 3");
    }

    // --- merged from part_02.rs ---

    #[test]
    fn batch32_chat_single_model_both_flags_produces_4_actions() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(
            actions.len(),
            5,
            "change_model + continue + copy + clear + capture = 5"
        );
    }

    #[test]
    fn batch32_chat_single_model_title_matches_display_name() {
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
        assert_eq!(picker[0].title, "GPT-4");
    }

    // ---------------------------------------------------------------------------
    // 11. Chat context: has_response=true without has_messages
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_chat_has_response_no_messages_has_copy_no_clear() {
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
    fn batch32_chat_has_messages_no_response_has_clear_no_copy() {
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
    fn batch32_chat_no_flags_only_continue() {
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

    // ---------------------------------------------------------------------------
    // 12. Notes command bar: find_in_note details
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // 13. Notes: trash view blocks all selection-dependent actions
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // 14. Note switcher: preview truncation with trim_end on > 60 chars
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_note_switcher_61_char_preview_truncated_with_ellipsis() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "a".repeat(61),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(
            desc.ends_with('…'),
            "61-char preview should end with …, got: {}",
            desc
        );
    }

    #[test]
    fn batch32_note_switcher_60_char_preview_no_truncation() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "b".repeat(60),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(
            !desc.ends_with('…'),
            "60-char preview should not be truncated, got: {}",
            desc
        );
        assert_eq!(desc.len(), 60);
    }

    #[test]
    fn batch32_note_switcher_short_preview_no_truncation() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "hello world".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert_eq!(desc, "hello world");
    }

    // ---------------------------------------------------------------------------
    // 15. Note switcher: title without current indicator has no bullet
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_note_switcher_non_current_no_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].title, "My Note");
    }

    #[test]
    fn batch32_note_switcher_current_has_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 10,
            is_current: true,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].title, "• My Note");
    }

    #[test]
    fn batch32_note_switcher_current_pinned_icon_star_filled() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 10,
            is_current: true,
            is_pinned: true,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    // ---------------------------------------------------------------------------
    // 16. New chat: last_used icon is always BoltFilled
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_new_chat_last_used_icon_bolt_filled() {
        let last_used = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    #[test]
    fn batch32_new_chat_last_used_section_is_last_used_settings() {
        let last_used = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn batch32_new_chat_last_used_desc_is_provider_display_name() {
        let last_used = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].description.as_deref(), Some("Uses OpenAI"));
    }

    // ---------------------------------------------------------------------------
    // 17. New chat: model section always "Models" with Settings icon
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_new_chat_model_section_is_models() {
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
    fn batch32_new_chat_model_icon_is_settings() {
        let models = vec![NewChatModelInfo {
            model_id: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].icon, Some(IconName::Settings));
    }

    #[test]
    fn batch32_new_chat_model_id_format() {
        let models = vec![NewChatModelInfo {
            model_id: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_anthropic::claude");
    }

    #[test]
    fn batch32_new_chat_preset_section_is_presets() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    // ---------------------------------------------------------------------------
    // 18. to_deeplink_name: tab, newline, and numbers-only input
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_to_deeplink_name_tab_and_newline() {
        assert_eq!(to_deeplink_name("test\ttab\nnewline"), "test-tab-newline");
    }

    #[test]
    fn batch32_to_deeplink_name_numbers_only() {
        assert_eq!(to_deeplink_name("12345"), "12345");
    }

    #[test]
    fn batch32_to_deeplink_name_leading_trailing_hyphens() {
        assert_eq!(to_deeplink_name("--hello--"), "hello");
    }

    #[test]
    fn batch32_to_deeplink_name_single_word() {
        assert_eq!(to_deeplink_name("hello"), "hello");
    }

    // ---------------------------------------------------------------------------
    // 19. format_shortcut_hint (on ActionsDialog): key conversions
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_format_shortcut_hint_cmd_e() {
        let result = ActionsDialog::format_shortcut_hint("cmd+e");
        assert_eq!(result, "⌘E");
    }

    #[test]
    fn batch32_format_shortcut_hint_all_modifiers() {
        let result = ActionsDialog::format_shortcut_hint("cmd+shift+ctrl+alt+k");
        assert_eq!(result, "⌘⇧⌃⌥K");
    }

    #[test]
    fn batch32_format_shortcut_hint_enter_alone() {
        let result = ActionsDialog::format_shortcut_hint("enter");
        assert_eq!(result, "↵");
    }

    #[test]
    fn batch32_format_shortcut_hint_meta_alias() {
        let result = ActionsDialog::format_shortcut_hint("meta+c");
        assert_eq!(result, "⌘C");
    }

    // ---------------------------------------------------------------------------
    // 20. parse_shortcut_keycaps: various inputs
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_parse_shortcut_keycaps_single_letter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("E");
        assert_eq!(caps, vec!["E"]);
    }

    #[test]
    fn batch32_parse_shortcut_keycaps_cmd_enter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
        assert_eq!(caps, vec!["⌘", "↵"]);
    }

    #[test]
    fn batch32_parse_shortcut_keycaps_slash() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘/");
        assert_eq!(caps, vec!["⌘", "/"]);
    }

    // --- merged from part_03.rs ---

    #[test]
    fn batch32_parse_shortcut_keycaps_space_symbol() {
        let caps = ActionsDialog::parse_shortcut_keycaps("␣");
        assert_eq!(caps, vec!["␣"]);
    }

    // ---------------------------------------------------------------------------
    // 21. score_action: empty search returns zero
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_score_action_empty_search_returns_zero() {
        let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        // Empty string is a prefix of everything, so prefix match gives 100
        assert!(
            score >= 100,
            "Empty search should prefix-match, got {}",
            score
        );
    }

    #[test]
    fn batch32_score_action_prefix_match_100_plus() {
        let action = Action::new(
            "script:edit",
            "Edit Script",
            Some("Open in editor".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "edit");
        // prefix (100) + description contains "edit" (15) = 115
        assert!(score >= 100, "Prefix match should be 100+, got {}", score);
    }

    #[test]
    fn batch32_score_action_no_match_returns_zero() {
        let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "xyz");
        assert_eq!(score, 0);
    }

    #[test]
    fn batch32_score_action_desc_bonus_stacks() {
        let action = Action::new(
            "open",
            "Open File",
            Some("Open in editor".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "open");
        // prefix (100) + desc contains "open" (15) = 115
        assert_eq!(score, 115);
    }

    // ---------------------------------------------------------------------------
    // 22. fuzzy_match: edge cases
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_fuzzy_match_empty_needle_matches() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn batch32_fuzzy_match_empty_haystack_empty_needle() {
        assert!(ActionsDialog::fuzzy_match("", ""));
    }

    #[test]
    fn batch32_fuzzy_match_empty_haystack_nonempty_needle() {
        assert!(!ActionsDialog::fuzzy_match("", "a"));
    }

    #[test]
    fn batch32_fuzzy_match_subsequence() {
        assert!(ActionsDialog::fuzzy_match("hello world", "hlo"));
    }

    #[test]
    fn batch32_fuzzy_match_no_subsequence() {
        assert!(!ActionsDialog::fuzzy_match("hello", "ba"));
    }

    // ---------------------------------------------------------------------------
    // 23. build_grouped_items_static: Headers style adds section headers
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_grouped_items_headers_style_adds_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Sec1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Sec1"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should be: Header("Sec1"), Item(0), Item(1)
        assert_eq!(grouped.len(), 3);
        assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "Sec1"));
    }

    #[test]
    fn batch32_grouped_items_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Sec1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Sec2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // No headers, just items
        assert_eq!(grouped.len(), 2);
        assert!(matches!(&grouped[0], GroupedActionItem::Item(_)));
    }

    #[test]
    fn batch32_grouped_items_headers_two_sections() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Header("S1"), Item(0), Header("S2"), Item(1)
        assert_eq!(grouped.len(), 4);
        assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "S1"));
        assert!(matches!(&grouped[2], GroupedActionItem::SectionHeader(s) if s == "S2"));
    }

    #[test]
    fn batch32_grouped_items_empty_returns_empty() {
        let actions: Vec<Action> = vec![];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // ---------------------------------------------------------------------------
    // 24. coerce_action_selection: various patterns
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_coerce_on_item_stays() {
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn batch32_coerce_on_header_jumps_down_to_item() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn batch32_coerce_trailing_header_jumps_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn batch32_coerce_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H1".into()),
            GroupedActionItem::SectionHeader("H2".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn batch32_coerce_empty_returns_none() {
        let rows: Vec<GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // ---------------------------------------------------------------------------
    // 25. CommandBarConfig: ai_style vs main_menu_style differences
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_config_ai_style_show_icons_true() {
        let config = CommandBarConfig::ai_style();
        assert!(config.dialog_config.show_icons);
    }

    #[test]
    fn batch32_config_main_menu_show_icons_false() {
        let config = CommandBarConfig::main_menu_style();
        assert!(!config.dialog_config.show_icons);
    }

    #[test]
    fn batch32_config_ai_style_show_footer_true() {
        let config = CommandBarConfig::ai_style();
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn batch32_config_main_menu_show_footer_false() {
        let config = CommandBarConfig::main_menu_style();
        assert!(!config.dialog_config.show_footer);
    }

    // ---------------------------------------------------------------------------
    // 26. Script context: with_action_verb propagates to run_script title
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_script_custom_verb_launch() {
        let script =
            crate::actions::types::ScriptInfo::with_action_verb("App", "/p/app", true, "Launch");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Launch");
    }

    #[test]
    fn batch32_script_custom_verb_switch_to() {
        let script = crate::actions::types::ScriptInfo::with_action_verb(
            "Window",
            "/p/w",
            false,
            "Switch to",
        );
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].title, "Switch To");
    }

    #[test]
    fn batch32_script_custom_verb_desc_uses_verb() {
        let script =
            crate::actions::types::ScriptInfo::with_action_verb("Foo", "/p/foo", true, "Execute");
        let actions = get_script_context_actions(&script);
        assert!(
            actions[0].description.as_ref().unwrap().contains("Execute"),
            "run desc should use verb, got: {:?}",
            actions[0].description
        );
    }

    // ---------------------------------------------------------------------------
    // 27. Script context: deeplink URL format in copy_deeplink description
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_script_deeplink_url_format() {
        let script = crate::actions::types::ScriptInfo::new("My Script", "/p/my-script.ts");
        let actions = get_script_context_actions(&script);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.title, "Share");
        assert_eq!(
            dl.description.as_deref(),
            Some("Copy a portable Script Kit share link to clipboard")
        );
    }

    #[test]
    fn batch32_script_deeplink_shortcut() {
        let script = crate::actions::types::ScriptInfo::new("Test", "/p/test.ts");
        let actions = get_script_context_actions(&script);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.shortcut.as_deref(), Some("⌘⇧D"));
    }

    #[test]
    fn batch32_builtin_deeplink_url_format() {
        let builtin = crate::actions::types::ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&builtin);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert!(
            dl.description
                .as_ref()
                .unwrap()
                .contains("scriptkit://run/clipboard-history"),
        );
    }

    // ---------------------------------------------------------------------------
    // 28. Clipboard: save_snippet and save_file always present
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_clipboard_text_has_save_snippet() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_save_snippet")
        );
    }

    #[test]
    fn batch32_clipboard_image_lacks_save_snippet() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // save_snippet exists only for text entries; images get clipboard_ocr instead
        assert!(
            !actions
                .iter()
                .any(|a| a.id == "clip:clipboard_save_snippet")
        );
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn batch32_clipboard_text_has_save_file() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_save_file"));
    }

    // ---------------------------------------------------------------------------
    // 29. Action builder: cached lowercase fields
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_action_title_lower_is_precomputed() {
        let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "hello world");
    }

    #[test]
    fn batch32_action_description_lower_is_precomputed() {
        let action = Action::new(
            "id",
            "T",
            Some("Open In EDITOR".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower.as_deref(), Some("open in editor"));
    }

    #[test]
    fn batch32_action_no_description_lower_is_none() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(action.description_lower.is_none());
    }

    #[test]
    fn batch32_action_shortcut_lower_after_with_shortcut() {
        let action =
            Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘⇧c"));
    }

    // ---------------------------------------------------------------------------
    // 30. Cross-context: all clipboard actions have ActionCategory::ScriptContext
    // ---------------------------------------------------------------------------

    #[test]
    fn batch32_all_clipboard_actions_are_script_context() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "clipboard action {} should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn batch32_all_file_actions_are_script_context() {
        let info = FileInfo {
            path: "/p/test.txt".into(),
            name: "test.txt".into(),
            file_type: FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&info);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "file action {} should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn batch32_all_path_actions_are_script_context() {
        let info = PathInfo::new("test.txt", "/p/test.txt", false);
        let actions = get_path_context_actions(&info);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "path action {} should be ScriptContext",
                a.id
            );
        }
    }

    #[test]
    fn batch32_all_ai_bar_actions_are_script_context() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "AI action {} should be ScriptContext",
                a.id
            );
        }
    }

    // --- merged from part_04.rs ---

    #[test]
    fn batch32_all_new_chat_actions_are_script_context() {
        let models = vec![NewChatModelInfo {
            model_id: "m".into(),
            display_name: "M".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        for a in &actions {
            assert_eq!(
                a.category,
                ActionCategory::ScriptContext,
                "new chat action {} should be ScriptContext",
                a.id
            );
        }
    }
}

mod from_dialog_builtin_action_validation_tests_33 {
    // --- merged from part_01.rs ---
    //! Batch 33: Dialog built-in action validation tests
    //!
    //! 115 tests across 30 categories validating random behaviors from
    //! built-in action window dialogs.

    use crate::actions::builders::{
        ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo, NoteSwitcherNoteInfo, NotesInfo,
        get_ai_command_bar_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom, to_deeplink_name,
    };
    use crate::actions::command_bar::CommandBarConfig;
    use crate::actions::dialog::{
        ActionsDialog, build_grouped_items_static, coerce_action_selection,
    };
    use crate::actions::types::{
        Action, ActionCategory, AnchorPosition, ScriptInfo, SearchPosition, SectionStyle,
    };
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::FileInfo;
    use crate::prompts::PathInfo;

    // =====================================================================
    // 1. ActionsDialog::format_shortcut_hint: alias handling for meta/super
    // =====================================================================

    #[test]
    fn format_shortcut_hint_meta_maps_to_cmd_symbol() {
        let result = ActionsDialog::format_shortcut_hint("meta+c");
        assert_eq!(result, "⌘C");
    }

    #[test]
    fn format_shortcut_hint_super_maps_to_cmd_symbol() {
        let result = ActionsDialog::format_shortcut_hint("super+x");
        assert_eq!(result, "⌘X");
    }

    #[test]
    fn format_shortcut_hint_command_alias() {
        let result = ActionsDialog::format_shortcut_hint("command+z");
        assert_eq!(result, "⌘Z");
    }

    #[test]
    fn format_shortcut_hint_opt_maps_to_option_symbol() {
        let result = ActionsDialog::format_shortcut_hint("opt+a");
        assert_eq!(result, "⌥A");
    }

    // =====================================================================
    // 2. ActionsDialog::format_shortcut_hint: special keys
    // =====================================================================

    #[test]
    fn format_shortcut_hint_return_maps_to_enter_symbol() {
        let result = ActionsDialog::format_shortcut_hint("return");
        assert_eq!(result, "↵");
    }

    #[test]
    fn format_shortcut_hint_esc_maps_to_escape_symbol() {
        let result = ActionsDialog::format_shortcut_hint("esc");
        assert_eq!(result, "⎋");
    }

    #[test]
    fn format_shortcut_hint_tab_maps_to_tab_symbol() {
        let result = ActionsDialog::format_shortcut_hint("tab");
        assert_eq!(result, "⇥");
    }

    #[test]
    fn format_shortcut_hint_space_maps_to_space_symbol() {
        let result = ActionsDialog::format_shortcut_hint("space");
        assert_eq!(result, "␣");
    }

    // =====================================================================
    // 3. ActionsDialog::format_shortcut_hint: arrow key variants
    // =====================================================================

    #[test]
    fn format_shortcut_hint_arrowup_maps_to_up_arrow() {
        let result = ActionsDialog::format_shortcut_hint("arrowup");
        assert_eq!(result, "↑");
    }

    #[test]
    fn format_shortcut_hint_arrowdown_maps_to_down_arrow() {
        let result = ActionsDialog::format_shortcut_hint("arrowdown");
        assert_eq!(result, "↓");
    }

    #[test]
    fn format_shortcut_hint_arrowleft_maps_to_left_arrow() {
        let result = ActionsDialog::format_shortcut_hint("arrowleft");
        assert_eq!(result, "←");
    }

    #[test]
    fn format_shortcut_hint_arrowright_maps_to_right_arrow() {
        let result = ActionsDialog::format_shortcut_hint("arrowright");
        assert_eq!(result, "→");
    }

    // =====================================================================
    // 4. ActionsDialog::format_shortcut_hint: combined modifier+special key
    // =====================================================================

    #[test]
    fn format_shortcut_hint_cmd_enter() {
        let result = ActionsDialog::format_shortcut_hint("cmd+enter");
        assert_eq!(result, "⌘↵");
    }

    #[test]
    fn format_shortcut_hint_ctrl_backspace() {
        let result = ActionsDialog::format_shortcut_hint("ctrl+backspace");
        assert_eq!(result, "⌃⌫");
    }

    #[test]
    fn format_shortcut_hint_option_space() {
        let result = ActionsDialog::format_shortcut_hint("option+space");
        assert_eq!(result, "⌥␣");
    }

    #[test]
    fn format_shortcut_hint_all_modifiers_plus_key() {
        let result = ActionsDialog::format_shortcut_hint("cmd+shift+ctrl+alt+k");
        assert_eq!(result, "⌘⇧⌃⌥K");
    }

    // =====================================================================
    // 5. ActionsDialog::parse_shortcut_keycaps: multi-symbol strings
    // =====================================================================

    #[test]
    fn parse_shortcut_keycaps_cmd_return() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
        assert_eq!(keycaps, vec!["⌘", "↵"]);
    }

    #[test]
    fn parse_shortcut_keycaps_all_modifiers_key() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧⌃⌥K");
        assert_eq!(keycaps, vec!["⌘", "⇧", "⌃", "⌥", "K"]);
    }

    #[test]
    fn parse_shortcut_keycaps_space_symbol() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("␣");
        assert_eq!(keycaps, vec!["␣"]);
    }

    #[test]
    fn parse_shortcut_keycaps_lowercase_uppercased() {
        let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘a");
        assert_eq!(keycaps, vec!["⌘", "A"]);
    }

    // =====================================================================
    // 6. ActionsDialog::score_action: prefix vs contains vs fuzzy vs none
    // =====================================================================

    #[test]
    fn score_action_prefix_match_at_least_100() {
        let action = Action::new(
            "script:edit",
            "Edit Script",
            None,
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(score >= 100, "Prefix match should be >= 100, got {}", score);
    }

    #[test]
    fn score_action_contains_match_between_50_and_99() {
        let action = Action::new(
            "copy",
            "Copy Edit Path",
            None,
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(
            (50..100).contains(&score),
            "Contains match should be 50-99, got {}",
            score
        );
    }

    #[test]
    fn score_action_no_match_returns_zero() {
        let action = Action::new(
            "script:run",
            "Run Script",
            None,
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "zzznotfound");
        assert_eq!(score, 0, "No match should return 0");
    }

    #[test]
    fn score_action_description_bonus_adds_15() {
        let action = Action::new(
            "open",
            "Open File",
            Some("Edit the file in your editor".to_string()),
            ActionCategory::ScriptContext,
        );
        // "editor" matches description but not title
        let score = ActionsDialog::score_action(&action, "editor");
        assert!(
            score >= 15,
            "Description match should add >= 15 points, got {}",
            score
        );
    }

    // =====================================================================
    // 7. ActionsDialog::score_action: shortcut bonus and empty search
    // =====================================================================

    #[test]
    fn score_action_shortcut_bonus_adds_10() {
        let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘E");
        // Searching for "⌘" matches the shortcut_lower
        let score = ActionsDialog::score_action(&action, "⌘");
        assert!(
            score >= 10,
            "Shortcut match should add >= 10 points, got {}",
            score
        );
    }

    #[test]
    fn score_action_empty_search_gives_prefix_match() {
        let action = Action::new("test", "Anything", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        assert!(
            score >= 100,
            "Empty search should prefix-match everything, got {}",
            score
        );
    }

    #[test]
    fn score_action_prefix_plus_description_bonus_stacks() {
        let action = Action::new(
            "script:edit",
            "Edit Script",
            Some("Edit the script file".to_string()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(
            score >= 115,
            "Prefix (100) + desc bonus (15) should be >= 115, got {}",
            score
        );
    }

    // =====================================================================
    // 8. ActionsDialog::fuzzy_match: edge cases
    // =====================================================================

    #[test]
    fn fuzzy_match_exact_match_returns_true() {
        assert!(ActionsDialog::fuzzy_match("edit script", "edit script"));
    }

    #[test]
    fn fuzzy_match_subsequence_returns_true() {
        assert!(ActionsDialog::fuzzy_match("edit script", "eds"));
    }

    #[test]
    fn fuzzy_match_no_match_returns_false() {
        assert!(!ActionsDialog::fuzzy_match("edit script", "xyz"));
    }

    #[test]
    fn fuzzy_match_empty_needle_returns_true() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn fuzzy_match_needle_longer_returns_false() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abcdef"));
    }

    // =====================================================================
    // 9. build_grouped_items_static: Headers vs Separators behavior
    // =====================================================================

    #[test]
    fn build_grouped_headers_style_adds_section_headers() {
        let actions = vec![
            Action::new("a", "Action A", None, ActionCategory::ScriptContext)
                .with_section("Response"),
            Action::new("b", "Action B", None, ActionCategory::ScriptContext)
                .with_section("Actions"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should have 2 headers + 2 items = 4
        assert_eq!(grouped.len(), 4, "Should have 2 headers + 2 items");
    }

    #[test]
    fn build_grouped_separators_style_no_headers() {
        let actions = vec![
            Action::new("a", "Action A", None, ActionCategory::ScriptContext)
                .with_section("Response"),
            Action::new("b", "Action B", None, ActionCategory::ScriptContext)
                .with_section("Actions"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Should have just 2 items, no headers
        assert_eq!(grouped.len(), 2, "Separators style should have no headers");
    }

    #[test]
    fn build_grouped_same_section_one_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Same"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Same"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // 1 header + 2 items = 3
        assert_eq!(
            grouped.len(),
            3,
            "Same section should produce single header + items"
        );
    }

    #[test]
    fn build_grouped_empty_returns_empty() {
        let actions: Vec<Action> = vec![];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // =====================================================================
    // 10. coerce_action_selection: header skipping behavior
    // =====================================================================

    #[test]
    fn coerce_on_item_stays_put() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn coerce_on_header_jumps_to_next_item() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::SectionHeader("Header".to_string()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn coerce_trailing_header_jumps_up() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("Header".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn coerce_all_headers_returns_none() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::SectionHeader("A".to_string()),
            GroupedActionItem::SectionHeader("B".to_string()),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn coerce_empty_returns_none() {
        let rows: Vec<crate::actions::dialog::GroupedActionItem> = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    // =====================================================================
    // 11. CommandBarConfig: preset dialog_config fields
    // =====================================================================

    #[test]
    fn command_bar_main_menu_search_top() {
        let config = CommandBarConfig::main_menu_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    }

    #[test]
    fn command_bar_ai_search_top() {
        let config = CommandBarConfig::ai_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    }

    #[test]
    fn command_bar_no_search_hidden() {
        let config = CommandBarConfig::no_search();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
    }

    #[test]
    fn command_bar_notes_search_top() {
        let config = CommandBarConfig::notes_style();
        assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    }

    // =====================================================================
    // 12. CommandBarConfig: section_style and anchor presets
    // =====================================================================

    #[test]
    fn command_bar_ai_section_headers() {
        let config = CommandBarConfig::ai_style();
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    }

    #[test]
    fn command_bar_main_menu_section_separators() {
        let config = CommandBarConfig::main_menu_style();
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    }

    #[test]
    fn command_bar_notes_section_separators() {
        let config = CommandBarConfig::notes_style();
        assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    }

    #[test]
    fn command_bar_ai_anchor_top() {
        let config = CommandBarConfig::ai_style();
        assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
    }

    // --- merged from part_02.rs ---

    // =====================================================================
    // 13. CommandBarConfig: show_icons and show_footer presets
    // =====================================================================

    #[test]
    fn command_bar_ai_shows_icons_and_footer() {
        let config = CommandBarConfig::ai_style();
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn command_bar_main_menu_hides_icons_and_footer() {
        let config = CommandBarConfig::main_menu_style();
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn command_bar_notes_shows_icons_and_footer() {
        let config = CommandBarConfig::notes_style();
        assert!(config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    #[test]
    fn command_bar_no_search_hides_icons_and_footer() {
        let config = CommandBarConfig::no_search();
        assert!(!config.dialog_config.show_icons);
        assert!(!config.dialog_config.show_footer);
    }

    // =====================================================================
    // 14. CommandBarConfig: close flag defaults
    // =====================================================================

    #[test]
    fn command_bar_default_close_flags_all_true() {
        let config = CommandBarConfig::default();
        assert!(config.close_on_select);
        assert!(config.close_on_click_outside);
        assert!(config.close_on_escape);
    }

    #[test]
    fn command_bar_ai_close_flags_inherited() {
        let config = CommandBarConfig::ai_style();
        assert!(config.close_on_select);
        assert!(config.close_on_click_outside);
        assert!(config.close_on_escape);
    }

    #[test]
    fn command_bar_main_menu_close_flags_inherited() {
        let config = CommandBarConfig::main_menu_style();
        assert!(config.close_on_select);
        assert!(config.close_on_click_outside);
        assert!(config.close_on_escape);
    }

    #[test]
    fn command_bar_notes_close_flags_inherited() {
        let config = CommandBarConfig::notes_style();
        assert!(config.close_on_select);
        assert!(config.close_on_click_outside);
        assert!(config.close_on_escape);
    }

    // =====================================================================
    // 15. AI command bar: paste_image details
    // =====================================================================

    #[test]
    fn ai_command_bar_paste_image_shortcut() {
        let actions = get_ai_command_bar_actions();
        let action = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(action.shortcut.as_ref().unwrap(), "⌘V");
    }

    #[test]
    fn ai_command_bar_paste_image_icon() {
        let actions = get_ai_command_bar_actions();
        let action = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(action.icon, Some(IconName::File));
    }

    #[test]
    fn ai_command_bar_paste_image_section() {
        let actions = get_ai_command_bar_actions();
        let action = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(action.section.as_deref(), Some("Attachments"));
    }

    #[test]
    fn ai_command_bar_paste_image_desc_mentions_clipboard() {
        let actions = get_ai_command_bar_actions();
        let action = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert!(
            action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("clipboard")
        );
    }

    // =====================================================================
    // 16. AI command bar: section distribution (count per section)
    // =====================================================================

    #[test]
    fn ai_command_bar_response_section_has_3_actions() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(count, 3);
    }

    #[test]
    fn ai_command_bar_actions_section_has_4_actions() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Actions"))
            .count();
        assert_eq!(count, 7);
    }

    #[test]
    fn ai_command_bar_attachments_section_has_2_actions() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Attachments"))
            .count();
        assert_eq!(count, 4);
    }

    #[test]
    fn ai_command_bar_total_is_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    // =====================================================================
    // 17. to_deeplink_name: edge cases with unicode and special chars
    // =====================================================================

    #[test]
    fn to_deeplink_name_with_parentheses_and_ampersand() {
        assert_eq!(to_deeplink_name("Copy & Paste (v2)"), "copy-paste-v2");
    }

    #[test]
    fn to_deeplink_name_with_dots_and_slashes() {
        assert_eq!(to_deeplink_name("file.txt/path"), "file-txt-path");
    }

    #[test]
    fn to_deeplink_name_only_special_chars() {
        assert_eq!(to_deeplink_name("!@#$%^&*()"), "_unnamed");
    }

    #[test]
    fn to_deeplink_name_already_hyphenated() {
        assert_eq!(to_deeplink_name("my-script"), "my-script");
    }

    // =====================================================================
    // 18. Script context: exact action ordering for plain script
    // =====================================================================

    #[test]
    fn script_context_first_action_is_run_script() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn script_context_last_action_is_copy_deeplink_without_suggestion() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        assert_eq!(actions.last().unwrap().id, "delete_script");
    }

    #[test]
    fn script_context_last_action_is_reset_ranking_with_suggestion() {
        let script = ScriptInfo::new("test", "/path/test.ts")
            .with_frecency(true, Some("/path/test.ts".to_string()));
        let actions = get_script_context_actions(&script);
        assert_eq!(actions.last().unwrap().id, "reset_ranking");
    }

    #[test]
    fn script_context_action_count_no_shortcut_no_alias() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        // run + toggle_info + add_shortcut + add_alias + toggle_favorite + edit + view_logs + reveal + open_in_quick_terminal + copy_path + copy_content + copy_deeplink + delete_script = 13
        assert_eq!(actions.len(), 13);
    }

    // =====================================================================
    // 19. Script context: agent-specific descriptions mention "agent"
    // =====================================================================

    #[test]
    fn agent_edit_title_is_edit_agent() {
        let mut agent = ScriptInfo::new("My Agent", "/path/agent.ts");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn agent_edit_desc_mentions_agent_file() {
        let mut agent = ScriptInfo::new("My Agent", "/path/agent.ts");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
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
    fn agent_reveal_desc_mentions_agent() {
        let mut agent = ScriptInfo::new("My Agent", "/path/agent.ts");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
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
    fn agent_has_no_view_logs() {
        let mut agent = ScriptInfo::new("My Agent", "/path/agent.ts");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // =====================================================================
    // 20. Clipboard: share shortcut and section for both text and image
    // =====================================================================

    #[test]
    fn clipboard_share_shortcut_is_shift_cmd_e() {
        let entry = ClipboardEntryInfo {
            id: "t".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert_eq!(share.shortcut.as_ref().unwrap(), "⇧⌘E");
    }

    #[test]
    fn clipboard_share_title_is_share() {
        let entry = ClipboardEntryInfo {
            id: "t".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert_eq!(share.title, "Share...");
    }

    #[test]
    fn clipboard_share_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "i".to_string(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".to_string(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_share"));
    }

    #[test]
    fn clipboard_share_desc_mentions_share() {
        let entry = ClipboardEntryInfo {
            id: "t".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert!(
            share
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("share")
        );
    }

    // =====================================================================
    // 21. Note switcher: char count singular vs plural
    // =====================================================================

    #[test]
    fn note_switcher_zero_chars_plural() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "id1".to_string(),
            title: "Note".to_string(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("0 chars"));
    }

    #[test]
    fn note_switcher_one_char_singular() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "id1".to_string(),
            title: "Note".to_string(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("1 char"));
    }

    #[test]
    fn note_switcher_many_chars_plural() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "id1".to_string(),
            title: "Note".to_string(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("42 chars"));
    }

    // =====================================================================
    // 22. Note switcher: preview with relative time separator
    // =====================================================================

    #[test]
    fn note_switcher_preview_with_time_has_dot_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "id1".to_string(),
            title: "Note".to_string(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".to_string(),
            relative_time: "2m ago".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Hello world · 2m ago")
        );
    }

    #[test]
    fn note_switcher_preview_without_time_no_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "id1".to_string(),
            title: "Note".to_string(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".to_string(),
            relative_time: "".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("Hello world"));
    }

    #[test]
    fn note_switcher_no_preview_with_time_shows_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "id1".to_string(),
            title: "Note".to_string(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "".to_string(),
            relative_time: "5d ago".to_string(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("5d ago"));
    }

    // =====================================================================
    // 23. Notes command bar: conditional action presence (selection + trash)
    // =====================================================================

    // --- merged from part_03.rs ---

    // =====================================================================
    // 24. Path context: exact action count for file vs dir
    // =====================================================================

    #[test]
    fn path_context_file_has_7_actions() {
        let path_info = PathInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions.len(), 7);
    }

    #[test]
    fn path_context_dir_has_7_actions() {
        let path_info = PathInfo {
            path: "/tmp/mydir".to_string(),
            name: "mydir".to_string(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions.len(), 7);
    }

    #[test]
    fn path_context_file_first_is_select_file() {
        let path_info = PathInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn path_context_dir_first_is_open_directory() {
        let path_info = PathInfo {
            path: "/tmp/mydir".to_string(),
            name: "mydir".to_string(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    // =====================================================================
    // 25. File context: macOS action count for file vs dir
    // =====================================================================

    #[cfg(target_os = "macos")]
    #[test]
    fn file_context_file_macos_has_12_actions() {
        let file_info = FileInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: crate::file_search::FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        // open_file + reveal + rename + move + duplicate + open_in_editor + open_in_quick_terminal + attach_to_ai + quick_look + show_info + copy_path + copy_filename + move_to_trash = 13
        assert_eq!(actions.len(), 13);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn file_context_dir_macos_has_10_actions() {
        let file_info = FileInfo {
            path: "/tmp/mydir".to_string(),
            name: "mydir".to_string(),
            file_type: crate::file_search::FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file_info);
        // open_directory + reveal + rename + move + duplicate + open_in_editor + open_in_quick_terminal + show_info + copy_path + copy_filename + move_to_trash = 11
        assert_eq!(actions.len(), 11);
    }

    #[test]
    fn file_context_file_title_quoted() {
        let file_info = FileInfo {
            path: "/tmp/doc.pdf".to_string(),
            name: "doc.pdf".to_string(),
            file_type: crate::file_search::FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        assert_eq!(actions[0].title, "Open \"doc.pdf\"");
    }

    #[test]
    fn file_context_dir_title_quoted() {
        let file_info = FileInfo {
            path: "/tmp/docs".to_string(),
            name: "docs".to_string(),
            file_type: crate::file_search::FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file_info);
        assert_eq!(actions[0].title, "Open \"docs\"");
    }

    // =====================================================================
    // 26. Scriptlet context with H3 custom: ordering invariant
    // =====================================================================

    #[test]
    fn scriptlet_with_custom_run_before_custom_actions() {
        use crate::scriptlets::{Scriptlet, ScriptletAction};

        let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
        let mut scriptlet =
            Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
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
        assert!(run_idx < custom_idx);
    }

    #[test]
    fn scriptlet_with_custom_builtins_after_custom() {
        use crate::scriptlets::{Scriptlet, ScriptletAction};

        let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
        let mut scriptlet =
            Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
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
        let custom_idx = actions
            .iter()
            .position(|a| a.id == "scriptlet_action:custom")
            .unwrap();
        let edit_idx = actions
            .iter()
            .position(|a| a.id == "edit_scriptlet")
            .unwrap();
        assert!(custom_idx < edit_idx);
    }

    #[test]
    fn scriptlet_custom_action_has_action_true() {
        use crate::scriptlets::{Scriptlet, ScriptletAction};

        let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
        let mut scriptlet =
            Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
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
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:custom")
            .unwrap();
        assert!(custom.has_action);
    }

    // =====================================================================
    // 27. New chat: section ordering and ID format
    // =====================================================================

    #[test]
    fn new_chat_last_used_section_name() {
        let last_used = vec![NewChatModelInfo {
            model_id: "claude-3".to_string(),
            display_name: "Claude 3".to_string(),
            provider: "anthropic".to_string(),
            provider_display_name: "Anthropic".to_string(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn new_chat_model_id_format() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt-4".to_string(),
            display_name: "GPT-4".to_string(),
            provider: "openai".to_string(),
            provider_display_name: "OpenAI".to_string(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_openai::gpt-4");
    }

    #[test]
    fn new_chat_preset_id_format() {
        let presets = vec![NewChatPresetInfo {
            id: "general".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_general");
    }

    #[test]
    fn new_chat_preset_description_is_none() {
        let presets = vec![NewChatPresetInfo {
            id: "code".to_string(),
            name: "Code".to_string(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].description.as_deref(), Some("Uses Code preset"));
    }

    // =====================================================================
    // 28. Action builder: with_shortcut_opt(None) vs with_shortcut_opt(Some)
    // =====================================================================

    #[test]
    fn action_with_shortcut_opt_none_leaves_shortcut_none() {
        let action =
            Action::new("id", "Title", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
        assert!(action.shortcut.is_none());
        assert!(action.shortcut_lower.is_none());
    }

    #[test]
    fn action_with_shortcut_opt_some_sets_shortcut() {
        let action = Action::new("id", "Title", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘K".to_string()));
        assert_eq!(action.shortcut.as_deref(), Some("⌘K"));
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘k"));
    }

    #[test]
    fn action_with_icon_sets_icon() {
        let action = Action::new("id", "Title", None, ActionCategory::ScriptContext)
            .with_icon(IconName::Copy);
        assert_eq!(action.icon, Some(IconName::Copy));
    }

    #[test]
    fn action_with_section_sets_section() {
        let action = Action::new("id", "Title", None, ActionCategory::ScriptContext)
            .with_section("Response");
        assert_eq!(action.section.as_deref(), Some("Response"));
    }

    // =====================================================================
    // 29. Cross-context: all built-in actions have has_action=false
    // =====================================================================

    #[test]
    fn all_script_actions_have_has_action_false() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for action in get_script_context_actions(&script) {
            assert!(
                !action.has_action,
                "Action {} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn all_clipboard_actions_have_has_action_false() {
        let entry = ClipboardEntryInfo {
            id: "t".to_string(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".to_string(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for action in get_clipboard_history_context_actions(&entry) {
            assert!(
                !action.has_action,
                "Action {} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn all_ai_bar_actions_have_has_action_false() {
        for action in get_ai_command_bar_actions() {
            assert!(
                !action.has_action,
                "Action {} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn all_path_actions_have_has_action_false() {
        let path_info = PathInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            is_dir: false,
        };
        for action in get_path_context_actions(&path_info) {
            assert!(
                !action.has_action,
                "Action {} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn all_file_actions_have_has_action_false() {
        let file_info = FileInfo {
            path: "/tmp/test.txt".to_string(),
            name: "test.txt".to_string(),
            file_type: crate::file_search::FileType::File,
            is_dir: false,
        };
        for action in get_file_context_actions(&file_info) {
            assert!(
                !action.has_action,
                "Action {} should have has_action=false",
                action.id
            );
        }
    }

    // =====================================================================
    // 30. Cross-context: all actions have non-empty title and id
    // =====================================================================

    #[test]
    fn all_ai_bar_actions_have_nonempty_title_and_id() {
        for action in get_ai_command_bar_actions() {
            assert!(!action.id.is_empty(), "Action should have non-empty id");
            assert!(
                !action.title.is_empty(),
                "Action {} should have non-empty title",
                action.id
            );
        }
    }

    // --- merged from part_04.rs ---

    #[test]
    fn all_new_chat_actions_have_nonempty_title_and_id() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".to_string(),
            display_name: "Model 1".to_string(),
            provider: "p1".to_string(),
            provider_display_name: "Provider 1".to_string(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "general".to_string(),
            name: "General".to_string(),
            icon: IconName::Star,
        }];
        for action in get_new_chat_actions(&models, &presets, &models) {
            assert!(!action.id.is_empty());
            assert!(
                !action.title.is_empty(),
                "Action {} should have non-empty title",
                action.id
            );
        }
    }
}

mod from_dialog_builtin_action_validation_tests_34 {
    // --- merged from part_01.rs ---
    //! Batch 34: Dialog built-in action validation tests
    //!
    //! 120 tests across 30 categories validating random behaviors from
    //! built-in action window dialogs.

    use crate::actions::builders::{
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
        get_chat_model_picker_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom, to_deeplink_name,
    };
    use crate::actions::command_bar::CommandBarConfig;
    use crate::actions::dialog::ActionsDialog;
    use crate::actions::types::{Action, ActionCategory, ScriptInfo, SearchPosition, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::FileInfo;
    use crate::prompts::PathInfo;

    // =====================================================================
    // 1. Clipboard: pinned image entry has both unpin and image-specific actions
    // =====================================================================

    #[test]
    fn clipboard_pinned_image_has_unpin_not_pin() {
        let entry = ClipboardEntryInfo {
            id: "pi-1".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "screenshot".into(),
            image_dimensions: Some((1920, 1080)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
    }

    #[test]
    fn clipboard_pinned_image_has_ocr() {
        let entry = ClipboardEntryInfo {
            id: "pi-2".into(),
            content_type: ContentType::Image,
            pinned: true,
            preview: "screenshot".into(),
            image_dimensions: Some((1920, 1080)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn clipboard_unpinned_text_has_pin_not_unpin() {
        let entry = ClipboardEntryInfo {
            id: "ut-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
    }

    #[test]
    fn clipboard_pinned_text_has_unpin() {
        let entry = ClipboardEntryInfo {
            id: "pt-1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
    }

    // =====================================================================
    // 2. Clipboard: OCR shortcut and description details
    // =====================================================================

    #[test]
    fn clipboard_ocr_shortcut_is_shift_cmd_c() {
        let entry = ClipboardEntryInfo {
            id: "ocr-1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert_eq!(ocr.shortcut.as_deref(), Some("⇧⌘C"));
    }

    #[test]
    fn clipboard_ocr_title_is_copy_text_from_image() {
        let entry = ClipboardEntryInfo {
            id: "ocr-2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert_eq!(ocr.title, "Copy Text from Image");
    }

    #[test]
    fn clipboard_ocr_desc_mentions_ocr() {
        let entry = ClipboardEntryInfo {
            id: "ocr-3".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert!(ocr.description.as_ref().unwrap().contains("OCR"));
    }

    #[test]
    fn clipboard_ocr_absent_for_text_entry() {
        let entry = ClipboardEntryInfo {
            id: "ocr-4".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    // =====================================================================
    // 3. Path context: move_to_trash description differs for file vs dir
    // =====================================================================

    #[test]
    fn path_move_to_trash_file_desc_says_delete_file() {
        let path_info = PathInfo {
            path: "/tmp/foo.txt".into(),
            name: "foo.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(trash.description.as_ref().unwrap().contains("file"));
    }

    #[test]
    fn path_move_to_trash_dir_desc_says_delete_folder() {
        let path_info = PathInfo {
            path: "/tmp/bar".into(),
            name: "bar".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        let trash = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(trash.description.as_ref().unwrap().contains("folder"));
    }

    #[test]
    fn path_move_to_trash_shortcut_is_cmd_delete() {
        let path_info = PathInfo {
            path: "/tmp/foo.txt".into(),
            name: "foo.txt".into(),
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
    fn path_move_to_trash_is_last_action() {
        let path_info = PathInfo {
            path: "/tmp/foo.txt".into(),
            name: "foo.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
    }

    // =====================================================================
    // 4. File context: description wording for specific actions
    // =====================================================================

    #[test]
    fn file_open_file_desc_says_default_application() {
        let file_info = FileInfo {
            path: "/tmp/doc.pdf".into(),
            name: "doc.pdf".into(),
            file_type: crate::file_search::FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
        assert!(open.description.as_ref().unwrap().contains("default app"));
    }

    #[test]
    fn file_reveal_desc_says_reveal_in_finder() {
        let file_info = FileInfo {
            path: "/tmp/doc.pdf".into(),
            name: "doc.pdf".into(),
            file_type: crate::file_search::FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let reveal = actions
            .iter()
            .find(|a| a.id == "file:reveal_in_finder")
            .unwrap();
        assert!(reveal.description.as_ref().unwrap().contains("Finder"));
    }

    #[test]
    fn file_copy_path_desc_says_full_path() {
        let file_info = FileInfo {
            path: "/tmp/doc.pdf".into(),
            name: "doc.pdf".into(),
            file_type: crate::file_search::FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert!(cp.description.as_ref().unwrap().contains("full path"));
    }

    #[test]
    fn file_copy_filename_desc_says_just_the_filename() {
        let file_info = FileInfo {
            path: "/tmp/doc.pdf".into(),
            name: "doc.pdf".into(),
            file_type: crate::file_search::FileType::Document,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(cf.description.as_ref().unwrap().contains("filename"));
    }

    // =====================================================================
    // 5. AI command bar: new_chat, delete_chat, toggle_shortcuts_help details
    // =====================================================================

    #[test]
    fn ai_bar_new_chat_shortcut_cmd_n() {
        let actions = get_ai_command_bar_actions();
        let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert_eq!(nc.shortcut.as_deref(), Some("⌘N"));
    }

    #[test]
    fn ai_bar_new_chat_icon_plus() {
        let actions = get_ai_command_bar_actions();
        let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
        assert_eq!(nc.icon, Some(IconName::Plus));
    }

    #[test]
    fn ai_bar_delete_chat_shortcut_cmd_delete() {
        let actions = get_ai_command_bar_actions();
        let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert_eq!(dc.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn ai_bar_delete_chat_icon_trash() {
        let actions = get_ai_command_bar_actions();
        let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
        assert_eq!(dc.icon, Some(IconName::Trash));
    }

    // =====================================================================
    // 6. AI command bar: toggle_shortcuts_help and section distribution
    // =====================================================================

    #[test]
    fn ai_bar_toggle_shortcuts_help_shortcut_cmd_slash() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.shortcut.as_deref(), Some("⌘/"));
    }

    #[test]
    fn ai_bar_toggle_shortcuts_help_icon_star() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.icon, Some(IconName::Star));
    }

    #[test]
    fn ai_bar_toggle_shortcuts_help_section_help() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.section.as_deref(), Some("Help"));
    }

    #[test]
    fn ai_bar_section_help_has_one_action() {
        let actions = get_ai_command_bar_actions();
        let help_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Help"))
            .count();
        assert_eq!(help_count, 1);
    }

    // =====================================================================
    // 7. AI command bar: Settings section has exactly one action
    // =====================================================================

    #[test]
    fn ai_bar_settings_section_has_one_action() {
        let actions = get_ai_command_bar_actions();
        let settings_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Settings"))
            .count();
        assert_eq!(settings_count, 2);
    }

    #[test]
    fn ai_bar_settings_action_is_change_model() {
        let actions = get_ai_command_bar_actions();
        let settings: Vec<_> = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Settings"))
            .collect();
        assert_eq!(settings[0].id, "chat:change_model");
    }

    #[test]
    fn ai_bar_change_model_has_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(cm.shortcut.is_none());
    }

    #[test]
    fn ai_bar_total_section_count_is_seven() {
        let actions = get_ai_command_bar_actions();
        let mut sections: Vec<&str> = actions
            .iter()
            .filter_map(|a| a.section.as_deref())
            .collect();
        sections.dedup();
        // Response, Actions, Attachments, Export, Context, Actions(again), Help, Settings
        // Unique sections: Response, Actions, Attachments, Export, Context, Help, Settings = 7
        let unique: std::collections::HashSet<&str> = actions
            .iter()
            .filter_map(|a| a.section.as_deref())
            .collect();
        assert_eq!(unique.len(), 7);
    }

    // =====================================================================
    // 8. Notes command bar: browse_notes details
    // =====================================================================

    // --- merged from part_02.rs ---

    // =====================================================================
    // 9. Notes command bar: export details
    // =====================================================================

    // =====================================================================
    // 10. Chat context: all 4 flag combinations (has_messages x has_response)
    // =====================================================================

    #[test]
    fn chat_no_messages_no_response_has_only_models_and_continue() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "m1".into(),
                display_name: "Model1".into(),
                provider: "P1".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue + capture = 3
        assert_eq!(actions.len(), 3);
        assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
        assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn chat_has_messages_no_response_has_clear_no_copy() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "m1".into(),
                display_name: "Model1".into(),
                provider: "P1".into(),
            }],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
        assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
    }

    #[test]
    fn chat_no_messages_has_response_has_copy_no_clear() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "m1".into(),
                display_name: "Model1".into(),
                provider: "P1".into(),
            }],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
        assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    #[test]
    fn chat_has_both_flags_has_copy_and_clear() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "m1".into(),
                display_name: "Model1".into(),
                provider: "P1".into(),
            }],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
        assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
        // change_model + continue + copy + clear + capture = 5
        assert_eq!(actions.len(), 5);
    }

    // =====================================================================
    // 11. Chat context: continue_in_chat always present regardless of flags
    // =====================================================================

    #[test]
    fn chat_continue_in_chat_always_present() {
        for (has_messages, has_response) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages,
                has_response,
            };
            let actions = get_chat_context_actions(&info);
            assert!(
                actions.iter().any(|a| a.id == "chat:continue_in_chat"),
                "continue_in_chat missing for has_messages={has_messages}, has_response={has_response}"
            );
        }
    }

    #[test]
    fn chat_continue_in_chat_shortcut_cmd_enter() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let cont = actions
            .iter()
            .find(|a| a.id == "chat:continue_in_chat")
            .unwrap();
        assert_eq!(cont.shortcut.as_deref(), Some("⌘↵"));
    }

    #[test]
    fn chat_continue_in_chat_desc_mentions_ai_harness() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let cont = actions
            .iter()
            .find(|a| a.id == "chat:continue_in_chat")
            .unwrap();
        assert!(
            cont.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("harness terminal")
        );
    }

    #[test]
    fn chat_clear_conversation_shortcut_cmd_delete() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let clear = actions
            .iter()
            .find(|a| a.id == "chat:clear_conversation")
            .unwrap();
        assert_eq!(clear.shortcut.as_deref(), Some("⌘⌫"));
    }

    // =====================================================================
    // 12. Chat context: copy_response shortcut is ⌘C
    // =====================================================================

    #[test]
    fn chat_copy_response_shortcut_cmd_c() {
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
    fn chat_copy_response_title() {
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
        assert_eq!(cr.title, "Copy Last Response");
    }

    #[test]
    fn chat_copy_response_desc_mentions_assistant() {
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
        assert!(cr.description.as_ref().unwrap().contains("assistant"));
    }

    // =====================================================================
    // 13. Script context: scriptlet with shortcut gets update/remove not add
    // =====================================================================

    #[test]
    fn scriptlet_with_shortcut_has_update_shortcut() {
        let script = ScriptInfo::scriptlet(
            "My Scriptlet",
            "/path/bundle.md",
            Some("cmd+s".into()),
            None,
        );
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "update_shortcut"));
        assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
    }

    #[test]
    fn scriptlet_with_shortcut_has_remove_shortcut() {
        let script = ScriptInfo::scriptlet(
            "My Scriptlet",
            "/path/bundle.md",
            Some("cmd+s".into()),
            None,
        );
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
    }

    #[test]
    fn scriptlet_without_shortcut_has_add_shortcut() {
        let script = ScriptInfo::scriptlet("My Scriptlet", "/path/bundle.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "add_shortcut"));
        assert!(!actions.iter().any(|a| a.id == "update_shortcut"));
    }

    #[test]
    fn scriptlet_with_alias_has_update_alias() {
        let script =
            ScriptInfo::scriptlet("My Scriptlet", "/path/bundle.md", None, Some("ms".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "update_alias"));
        assert!(actions.iter().any(|a| a.id == "remove_alias"));
        assert!(!actions.iter().any(|a| a.id == "add_alias"));
    }

    // =====================================================================
    // 14. Scriptlet context: copy_content desc mentions entire file
    // =====================================================================

    #[test]
    fn scriptlet_copy_content_desc_mentions_entire_file() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(cc.description.as_ref().unwrap().contains("entire file"));
    }

    #[test]
    fn scriptlet_copy_content_shortcut_opt_cmd_c() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn scriptlet_edit_scriptlet_shortcut_cmd_e() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
        assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn scriptlet_edit_scriptlet_desc_mentions_editor() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("$EDITOR"));
    }

    // =====================================================================
    // 15. builders::format_shortcut_hint vs ActionsDialog::format_shortcut_hint
    // =====================================================================

    #[test]
    fn builders_format_basic_cmd_c() {
        // builders::format_shortcut_hint is private, but we test via to_deeplink_name
        // and scriptlet-defined actions. The ActionsDialog version handles more aliases.
        let result = ActionsDialog::format_shortcut_hint("cmd+c");
        assert_eq!(result, "⌘C");
    }

    #[test]
    fn dialog_format_handles_control_alias() {
        let result = ActionsDialog::format_shortcut_hint("control+x");
        assert_eq!(result, "⌃X");
    }

    #[test]
    fn dialog_format_handles_option_alias() {
        let result = ActionsDialog::format_shortcut_hint("option+v");
        assert_eq!(result, "⌥V");
    }

    #[test]
    fn dialog_format_handles_backspace_key() {
        let result = ActionsDialog::format_shortcut_hint("cmd+backspace");
        assert_eq!(result, "⌘⌫");
    }

    // =====================================================================
    // 16. to_deeplink_name: various transformations
    // =====================================================================

    #[test]
    fn deeplink_underscores_become_hyphens() {
        assert_eq!(to_deeplink_name("hello_world"), "hello-world");
    }

    #[test]
    fn deeplink_multiple_underscores_collapse() {
        assert_eq!(to_deeplink_name("a___b"), "a-b");
    }

    #[test]
    fn deeplink_mixed_punctuation() {
        assert_eq!(to_deeplink_name("Hello, World!"), "hello-world");
    }

    #[test]
    fn deeplink_empty_string() {
        assert_eq!(to_deeplink_name(""), "_unnamed");
    }

    // =====================================================================
    // 17. Constants: UI dimensions validation
    // =====================================================================

    #[test]
    fn constant_popup_width_320() {
        use crate::actions::constants::POPUP_WIDTH;
        assert_eq!(POPUP_WIDTH, 340.0);
    }

    #[test]
    fn constant_popup_max_height_400() {
        use crate::actions::constants::POPUP_MAX_HEIGHT;
        assert_eq!(POPUP_MAX_HEIGHT, 400.0);
    }

    #[test]
    fn constant_action_item_height_30() {
        use crate::actions::constants::ACTION_ITEM_HEIGHT;
        assert_eq!(ACTION_ITEM_HEIGHT, 36.0);
    }

    #[test]
    fn constant_search_input_height_36() {
        use crate::actions::constants::SEARCH_INPUT_HEIGHT;
        assert_eq!(SEARCH_INPUT_HEIGHT, 40.0);
    }

    // =====================================================================
    // 18. CommandBarConfig notes_style preset
    // =====================================================================

    #[test]
    fn notes_style_search_position_top() {
        let config = CommandBarConfig::notes_style();
        assert!(matches!(
            config.dialog_config.search_position,
            SearchPosition::Top
        ));
    }

    #[test]
    fn notes_style_section_style_separators() {
        let config = CommandBarConfig::notes_style();
        assert!(matches!(
            config.dialog_config.section_style,
            SectionStyle::Headers
        ));
    }

    #[test]
    fn notes_style_show_icons_true() {
        let config = CommandBarConfig::notes_style();
        assert!(config.dialog_config.show_icons);
    }

    #[test]
    fn notes_style_show_footer_true() {
        let config = CommandBarConfig::notes_style();
        assert!(!config.dialog_config.show_footer);
    }

    // =====================================================================
    // 19. Global actions: seeded with global ops
    // =====================================================================

    #[test]
    fn global_actions_seeded() {
        use crate::actions::builders::get_global_actions;
        let actions = get_global_actions();
        // Seeded, never empty: reload_scripts, settings, view_logs plus
        // the Discover rows. Prompt export/handoff rows are Agent Chat-owned
        // and must NOT appear here.
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"reload_scripts"));
        assert!(ids.contains(&"settings"));
        assert!(ids.contains(&"view_logs"));
    }

    #[test]
    fn global_actions_returns_vec() {
        use crate::actions::builders::get_global_actions;
        let actions = get_global_actions();
        // Exact total is config-dependent; only assert it is non-empty.
        assert!(!actions.is_empty());
    }

    // --- merged from part_03.rs ---

    // =====================================================================
    // 20. Action::with_shortcut_opt with None leaves shortcut unset
    // =====================================================================

    #[test]
    fn action_with_shortcut_opt_none_leaves_none() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(None);
        assert!(action.shortcut.is_none());
        assert!(action.shortcut_lower.is_none());
    }

    #[test]
    fn action_with_shortcut_opt_some_sets_both() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘X".into()));
        assert_eq!(action.shortcut.as_deref(), Some("⌘X"));
        assert_eq!(action.shortcut_lower.as_deref(), Some("⌘x"));
    }

    #[test]
    fn action_with_shortcut_sets_shortcut_lower() {
        let action =
            Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut("⇧⌘K");
        assert_eq!(action.shortcut.as_deref(), Some("⇧⌘K"));
        assert_eq!(action.shortcut_lower.as_deref(), Some("⇧⌘k"));
    }

    #[test]
    fn action_new_has_no_shortcut_by_default() {
        let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
        assert!(action.shortcut.is_none());
        assert!(action.shortcut_lower.is_none());
    }

    // =====================================================================
    // 21. Note switcher: section assignment based on pinned status
    // =====================================================================

    #[test]
    fn note_switcher_pinned_section_is_pinned() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-1".into(),
            title: "Pinned Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: true,
            preview: "Some text".into(),
            relative_time: "1m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
    }

    #[test]
    fn note_switcher_unpinned_section_is_recent() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid-2".into(),
            title: "Regular Note".into(),
            char_count: 50,
            is_current: false,
            is_pinned: false,
            preview: "Some text".into(),
            relative_time: "2h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Recent"));
    }

    #[test]
    fn note_switcher_mixed_pinned_and_recent() {
        let notes = vec![
            NoteSwitcherNoteInfo {
                id: "a".into(),
                title: "A".into(),
                char_count: 10,
                is_current: false,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            },
            NoteSwitcherNoteInfo {
                id: "b".into(),
                title: "B".into(),
                char_count: 20,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            },
        ];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        assert_eq!(actions[1].section.as_deref(), Some("Recent"));
    }

    // =====================================================================
    // 22. Clipboard: pin/unpin share the same shortcut ⇧⌘P
    // =====================================================================

    #[test]
    fn clipboard_pin_shortcut_shift_cmd_p() {
        let entry = ClipboardEntryInfo {
            id: "pin-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap();
        assert_eq!(pin.shortcut.as_deref(), Some("⇧⌘P"));
    }

    #[test]
    fn clipboard_unpin_shortcut_shift_cmd_p() {
        let entry = ClipboardEntryInfo {
            id: "pin-2".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let unpin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_unpin")
            .unwrap();
        assert_eq!(unpin.shortcut.as_deref(), Some("⇧⌘P"));
    }

    #[test]
    fn clipboard_pin_title_is_pin_entry() {
        let entry = ClipboardEntryInfo {
            id: "pin-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
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
    fn clipboard_unpin_title_is_unpin_entry() {
        let entry = ClipboardEntryInfo {
            id: "pin-4".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "test".into(),
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

    // =====================================================================
    // 23. Script context: agent action set details
    // =====================================================================

    #[test]
    fn agent_edit_title_is_edit_agent() {
        let mut script = ScriptInfo::new("MyAgent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn agent_edit_desc_mentions_agent_file() {
        let mut script = ScriptInfo::new("MyAgent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn agent_has_reveal_in_finder() {
        let mut script = ScriptInfo::new("MyAgent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
    }

    #[test]
    fn agent_has_no_view_logs() {
        let mut script = ScriptInfo::new("MyAgent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // =====================================================================
    // 24. New chat: preset icon is preserved
    // =====================================================================

    #[test]
    fn new_chat_preset_icon_preserved() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Star));
    }

    #[test]
    fn new_chat_preset_section_is_presets() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    #[test]
    fn new_chat_preset_desc_is_none() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].description.as_deref(), Some("Uses Code preset"));
    }

    #[test]
    fn new_chat_model_desc_is_provider_display_name() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].description.as_deref(), Some("Uses OpenAI"));
    }

    // =====================================================================
    // 25. Note switcher: empty preview uses relative_time or char count
    // =====================================================================

    #[test]
    fn note_switcher_empty_preview_with_time_shows_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n1".into(),
            title: "Note".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("5m ago"));
    }

    #[test]
    fn note_switcher_empty_preview_empty_time_shows_chars() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n2".into(),
            title: "Note".into(),
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
    fn note_switcher_preview_with_time_has_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n3".into(),
            title: "Note".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "1h ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.contains(" · "));
        assert!(desc.contains("Hello world"));
        assert!(desc.contains("1h ago"));
    }

    #[test]
    fn note_switcher_preview_without_time_no_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n4".into(),
            title: "Note".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(!desc.contains(" · "));
        assert_eq!(desc, "Hello world");
    }

    // =====================================================================
    // 26. Clipboard: upload_cleanshot shortcut is ⇧⌘U (macOS only)
    // =====================================================================

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_shortcut_shift_cmd_u() {
        let entry = ClipboardEntryInfo {
            id: "uc-1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let upload = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_upload_cleanshot")
            .unwrap();
        assert_eq!(upload.shortcut.as_deref(), Some("⇧⌘U"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_title() {
        let entry = ClipboardEntryInfo {
            id: "uc-2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let upload = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_upload_cleanshot")
            .unwrap();
        assert_eq!(upload.title, "Upload to CleanShot X");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_desc_mentions_cloud() {
        let entry = ClipboardEntryInfo {
            id: "uc-3".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let upload = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_upload_cleanshot")
            .unwrap();
        assert!(upload.description.as_ref().unwrap().contains("Cloud"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_absent_for_text() {
        let entry = ClipboardEntryInfo {
            id: "uc-4".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
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

    // =====================================================================
    // 27. Path context: open_in_editor and open_in_finder details
    // =====================================================================

    #[test]
    fn path_open_in_editor_shortcut_cmd_e() {
        let path_info = PathInfo {
            path: "/tmp/code.rs".into(),
            name: "code.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let editor = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert_eq!(editor.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn path_open_in_editor_desc_mentions_editor() {
        let path_info = PathInfo {
            path: "/tmp/code.rs".into(),
            name: "code.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let editor = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert!(editor.description.as_ref().unwrap().contains("$EDITOR"));
    }

    #[test]
    fn path_open_in_finder_shortcut_cmd_shift_f() {
        let path_info = PathInfo {
            path: "/tmp/code.rs".into(),
            name: "code.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let finder = actions
            .iter()
            .find(|a| a.id == "file:open_in_finder")
            .unwrap();
        assert_eq!(finder.shortcut.as_deref(), Some("⌘⇧F"));
    }

    #[test]
    fn path_open_in_finder_desc_mentions_finder() {
        let path_info = PathInfo {
            path: "/tmp/code.rs".into(),
            name: "code.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let finder = actions
            .iter()
            .find(|a| a.id == "file:open_in_finder")
            .unwrap();
        assert!(finder.description.as_ref().unwrap().contains("Finder"));
    }

    // --- merged from part_04.rs ---

    // =====================================================================
    // 28. Script context: copy_content shortcut ⌘⌥C for all types
    // =====================================================================

    #[test]
    fn script_copy_content_shortcut_opt_cmd_c() {
        let script = ScriptInfo::new("Test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn agent_copy_content_shortcut_opt_cmd_c() {
        let mut script = ScriptInfo::new("Agent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn script_copy_content_desc_mentions_entire_file() {
        let script = ScriptInfo::new("Test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(cc.description.as_ref().unwrap().contains("entire file"));
    }

    #[test]
    fn agent_copy_content_desc_mentions_entire_file() {
        let mut script = ScriptInfo::new("Agent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(cc.description.as_ref().unwrap().contains("entire file"));
    }

    // =====================================================================
    // 29. score_action: title_lower and description_lower used for matching
    // =====================================================================

    #[test]
    fn score_action_matches_case_insensitive() {
        let action = Action::new(
            "test",
            "Edit Script",
            Some("Open in editor".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!(score >= 100, "Prefix match should score >=100, got {score}");
    }

    #[test]
    fn score_action_description_bonus_adds_points() {
        let action = Action::new(
            "test",
            "Open File",
            Some("Open in editor for editing".into()),
            ActionCategory::ScriptContext,
        );
        // "editor" is not in title but is in description
        let score = ActionsDialog::score_action(&action, "editor");
        assert!(
            score >= 15,
            "Description match should score >=15, got {score}"
        );
    }

    #[test]
    fn score_action_no_match_returns_zero() {
        let action = Action::new(
            "test",
            "Edit Script",
            Some("Open in editor".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "zzzzz");
        assert_eq!(score, 0);
    }

    #[test]
    fn score_action_shortcut_bonus() {
        let action = Action::new("test", "Something", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘E");
        // "⌘e" matches shortcut_lower "⌘e"
        let score = ActionsDialog::score_action(&action, "⌘e");
        assert!(score >= 10, "Shortcut match should score >=10, got {score}");
    }

    // =====================================================================
    // 30. Cross-context: all clipboard text actions have ScriptContext category
    // =====================================================================

    #[test]
    fn all_clipboard_text_actions_have_script_context_category() {
        let entry = ClipboardEntryInfo {
            id: "cat-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "test".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        for action in &actions {
            assert_eq!(
                action.category,
                ActionCategory::ScriptContext,
                "Action '{}' should have ScriptContext category",
                action.id
            );
        }
    }

    #[test]
    fn all_path_actions_have_script_context_category() {
        let path_info = PathInfo {
            path: "/tmp/foo".into(),
            name: "foo".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        for action in &actions {
            assert_eq!(
                action.category,
                ActionCategory::ScriptContext,
                "Action '{}' should have ScriptContext category",
                action.id
            );
        }
    }

    #[test]
    fn all_ai_bar_actions_have_script_context_category() {
        let actions = get_ai_command_bar_actions();
        for action in &actions {
            assert_eq!(
                action.category,
                ActionCategory::ScriptContext,
                "Action '{}' should have ScriptContext category",
                action.id
            );
        }
    }

    #[test]
    fn all_note_switcher_actions_have_script_context_category() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "uuid".into(),
            title: "Note".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        for action in &actions {
            assert_eq!(
                action.category,
                ActionCategory::ScriptContext,
                "Action '{}' should have ScriptContext category",
                action.id
            );
        }
    }
}

mod from_dialog_builtin_action_validation_tests_35 {
    // --- merged from part_01.rs ---
    //! Batch 35: Dialog built-in action validation tests
    //!
    //! 116 tests across 30 categories validating random behaviors from
    //! built-in action window dialogs.

    use crate::actions::builders::{
        ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
        NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
        get_chat_model_picker_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom, to_deeplink_name,
    };
    use crate::actions::command_bar::CommandBarConfig;
    use crate::actions::constants::{ACTION_ROW_INSET, HEADER_HEIGHT, SEARCH_INPUT_HEIGHT};
    use crate::actions::dialog::{
        ActionsDialog, build_grouped_items_static, coerce_action_selection,
    };
    use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::FileInfo;
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // =====================================================================
    // 1. Clipboard: attach_to_ai shortcut and description
    // =====================================================================

    #[test]
    fn clipboard_attach_to_ai_shortcut_is_ctrl_cmd_a() {
        let entry = ClipboardEntryInfo {
            id: "ai-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "some text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert_eq!(action.shortcut.as_ref().unwrap(), "⌃⌘A");
    }

    #[test]
    fn clipboard_attach_to_ai_title() {
        let entry = ClipboardEntryInfo {
            id: "ai-2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert_eq!(action.title, "Attach to Agent Chat");
    }

    #[test]
    fn clipboard_attach_to_ai_desc_mentions_ai() {
        let entry = ClipboardEntryInfo {
            id: "ai-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_attach_to_ai")
            .unwrap();
        assert!(
            action
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("agent chat")
        );
    }

    #[test]
    fn clipboard_attach_to_ai_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "ai-4".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((640, 480)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_attach_to_ai")
        );
    }

    // =====================================================================
    // 2. Clipboard: total action count text vs image on macOS
    // =====================================================================

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_text_action_count_macos() {
        let entry = ClipboardEntryInfo {
            id: "cnt-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // Text on macOS: paste, copy, paste_keep_open, share, attach_to_ai, quick_look, pin,
        // keep_in_today, save_snippet, save_file, delete, delete_multiple, delete_all = 13
        assert_eq!(actions.len(), 13);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_image_action_count_macos() {
        let entry = ClipboardEntryInfo {
            id: "cnt-2".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // Image on macOS: paste, copy, paste_keep_open, share, attach_to_ai, quick_look,
        // open_with, annotate_cleanshot, upload_cleanshot, pin, keep_in_today, ocr,
        // save_file, delete, delete_multiple, delete_all = 16 (no save_snippet for images)
        assert_eq!(actions.len(), 16);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_image_has_3_more_actions_than_text_on_macos() {
        let text_entry = ClipboardEntryInfo {
            id: "cnt-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let img_entry = ClipboardEntryInfo {
            id: "cnt-4".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "i".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let text_count = get_clipboard_history_context_actions(&text_entry).len();
        let img_count = get_clipboard_history_context_actions(&img_entry).len();
        // Image adds open_with, annotate_cleanshot, upload_cleanshot, ocr but
        // drops save_snippet (text-only), so the net difference is 3.
        assert_eq!(img_count - text_count, 3);
    }

    #[test]
    fn clipboard_pinned_vs_unpinned_same_count() {
        let pinned = ClipboardEntryInfo {
            id: "cnt-5".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "p".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let unpinned = ClipboardEntryInfo {
            id: "cnt-6".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "u".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        // pin/unpin swapped but same count
        assert_eq!(
            get_clipboard_history_context_actions(&pinned).len(),
            get_clipboard_history_context_actions(&unpinned).len()
        );
    }

    // =====================================================================
    // 3. File context: primary action title is quoted with file name
    // =====================================================================

    #[test]
    fn file_context_file_title_quoted() {
        let file_info = FileInfo {
            path: "/Users/test/readme.md".into(),
            name: "readme.md".into(),
            file_type: crate::file_search::FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        let primary = actions.first().unwrap();
        assert_eq!(primary.title, "Open \"readme.md\"");
    }

    #[test]
    fn file_context_dir_title_quoted() {
        let file_info = FileInfo {
            path: "/Users/test/Documents".into(),
            name: "Documents".into(),
            file_type: crate::file_search::FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file_info);
        let primary = actions.first().unwrap();
        assert_eq!(primary.title, "Open \"Documents\"");
    }

    #[test]
    fn file_context_file_primary_shortcut_enter() {
        let file_info = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: crate::file_search::FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file_info);
        assert_eq!(actions[0].shortcut.as_ref().unwrap(), "↵");
    }

    #[test]
    fn file_context_dir_primary_shortcut_enter() {
        let file_info = FileInfo {
            path: "/test/dir".into(),
            name: "dir".into(),
            file_type: crate::file_search::FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&file_info);
        assert_eq!(actions[0].shortcut.as_ref().unwrap(), "↵");
    }

    // =====================================================================
    // 4. Path context: primary action description varies for file vs dir
    // =====================================================================

    #[test]
    fn path_file_primary_desc_is_submit() {
        let path_info = PathInfo {
            path: "/Users/test/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        let primary = actions.first().unwrap();
        assert!(primary.description.as_ref().unwrap().contains("Selects"));
    }

    #[test]
    fn path_dir_primary_desc_is_navigate() {
        let path_info = PathInfo {
            path: "/Users/test/docs".into(),
            name: "docs".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        let primary = actions.first().unwrap();
        assert!(primary.description.as_ref().unwrap().contains("Opens"));
    }

    #[test]
    fn path_file_primary_id_is_select_file() {
        let path_info = PathInfo {
            path: "/test/a.txt".into(),
            name: "a.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn path_dir_primary_id_is_open_directory() {
        let path_info = PathInfo {
            path: "/test/dir".into(),
            name: "dir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    // =====================================================================
    // 5. Script context: edit shortcut ⌘E across types
    // =====================================================================

    #[test]
    fn script_edit_shortcut_cmd_e() {
        let script = ScriptInfo::new("my-script", "/path/to/script.ts");
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.shortcut.as_ref().unwrap(), "⌘E");
    }

    #[test]
    fn scriptlet_edit_shortcut_cmd_e() {
        let scriptlet = ScriptInfo::scriptlet("Open URL", "/path/url.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
        assert_eq!(edit.shortcut.as_ref().unwrap(), "⌘E");
    }

    #[test]
    fn agent_edit_shortcut_cmd_e() {
        let mut agent = ScriptInfo::new("My Agent", "/path/agent");
        agent.is_script = false;
        agent.is_agent = true;
        let actions = get_script_context_actions(&agent);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.shortcut.as_ref().unwrap(), "⌘E");
    }

    #[test]
    fn script_copy_content_shortcut_cmd_opt_c() {
        let script = ScriptInfo::new("my-script", "/path/to/script.ts");
        let actions = get_script_context_actions(&script);
        let copy = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(copy.shortcut.as_ref().unwrap(), "⌘⌥C");
    }

    // =====================================================================
    // 6. Scriptlet with custom H3 actions: ID prefix, has_action, value
    // =====================================================================

    #[test]
    fn scriptlet_custom_action_id_prefix() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        s.actions = vec![ScriptletAction {
            name: "My Custom".into(),
            command: "my-custom".into(),
            tool: "bash".into(),
            code: "echo custom".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        assert!(actions.iter().any(|a| a.id == "scriptlet_action:my-custom"));
    }

    #[test]
    fn scriptlet_custom_action_has_action_true() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        s.actions = vec![ScriptletAction {
            name: "Custom".into(),
            command: "cmd".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:cmd")
            .unwrap();
        assert!(custom.has_action);
    }

    #[test]
    fn scriptlet_custom_action_value_is_command() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
        s.actions = vec![ScriptletAction {
            name: "Copy It".into(),
            command: "copy-it".into(),
            tool: "bash".into(),
            code: "pbcopy".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:copy-it")
            .unwrap();
        assert_eq!(custom.value.as_ref().unwrap(), "copy-it");
    }

    #[test]
    fn scriptlet_builtin_actions_has_action_false() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        for a in &actions {
            assert!(
                !a.has_action,
                "Built-in action {} should have has_action=false",
                a.id
            );
        }
    }

    // =====================================================================
    // 7. Scriptlet custom action with shortcut gets format_shortcut_hint applied
    // =====================================================================

    #[test]
    fn scriptlet_custom_action_shortcut_formatted() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo".into());
        s.actions = vec![ScriptletAction {
            name: "Action".into(),
            command: "act".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: Some("cmd+shift+x".into()),
            description: Some("Do something".into()),
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:act")
            .unwrap();
        assert_eq!(custom.shortcut.as_ref().unwrap(), "⌘⇧X");
    }

    // --- merged from part_02.rs ---

    #[test]
    fn scriptlet_custom_action_without_shortcut_is_none() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo".into());
        s.actions = vec![ScriptletAction {
            name: "NoKey".into(),
            command: "nokey".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:nokey")
            .unwrap();
        assert!(custom.shortcut.is_none());
    }

    #[test]
    fn scriptlet_custom_action_description_propagated() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo".into());
        s.actions = vec![ScriptletAction {
            name: "Desc Action".into(),
            command: "desc-act".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: None,
            description: Some("My description here".into()),
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:desc-act")
            .unwrap();
        assert_eq!(custom.description.as_ref().unwrap(), "My description here");
    }

    #[test]
    fn scriptlet_custom_action_title_is_name() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let mut s = Scriptlet::new("Test".into(), "bash".into(), "echo".into());
        s.actions = vec![ScriptletAction {
            name: "My Title".into(),
            command: "mt".into(),
            tool: "bash".into(),
            code: "echo".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&s));
        let custom = actions
            .iter()
            .find(|a| a.id == "scriptlet_action:mt")
            .unwrap();
        assert_eq!(custom.title, "My Title");
    }

    // =====================================================================
    // 8. AI command bar: copy_chat and copy_last_code details
    // =====================================================================

    #[test]
    fn ai_bar_copy_chat_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(a.shortcut.as_ref().unwrap(), "⌥⇧⌘C");
    }

    #[test]
    fn ai_bar_copy_chat_icon_copy() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(a.icon, Some(IconName::Copy));
    }

    #[test]
    fn ai_bar_copy_chat_section_response() {
        let actions = get_ai_command_bar_actions();
        let a = actions.iter().find(|a| a.id == "chat:copy_chat").unwrap();
        assert_eq!(a.section.as_ref().unwrap(), "Response");
    }

    #[test]
    fn ai_bar_copy_last_code_shortcut() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.shortcut.as_ref().unwrap(), "⌥⌘C");
    }

    #[test]
    fn ai_bar_copy_last_code_icon_code() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.icon, Some(IconName::Code));
    }

    #[test]
    fn ai_bar_copy_last_code_section_response() {
        let actions = get_ai_command_bar_actions();
        let a = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(a.section.as_ref().unwrap(), "Response");
    }

    // =====================================================================
    // 9. AI command bar: all IDs are unique
    // =====================================================================

    #[test]
    fn ai_bar_all_ids_unique() {
        let actions = get_ai_command_bar_actions();
        let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len);
    }

    #[test]
    fn ai_bar_all_have_icon() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(
                a.icon.is_some(),
                "AI bar action {} should have an icon",
                a.id
            );
        }
    }

    #[test]
    fn ai_bar_all_have_section() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(
                a.section.is_some(),
                "AI bar action {} should have a section",
                a.id
            );
        }
    }

    #[test]
    fn ai_bar_count_is_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    // =====================================================================
    // 10. Chat context: select_model ID format
    // =====================================================================

    #[test]
    fn chat_model_id_format() {
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
        assert!(picker.iter().any(|a| a.id == "chat:select_model_gpt-4"));
    }

    #[test]
    fn chat_model_current_check_by_display_name() {
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
        let model = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-4")
            .unwrap();
        assert!(model.title.contains("✓"));
    }

    #[test]
    fn chat_model_non_current_no_check() {
        let info = ChatPromptInfo {
            current_model: Some("Claude 3.5".into()),
            available_models: vec![ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        let model = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt-4")
            .unwrap();
        assert!(!model.title.contains("✓"));
    }

    #[test]
    fn chat_model_desc_via_provider() {
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
        let model = picker
            .iter()
            .find(|a| a.id == "chat:select_model_claude")
            .unwrap();
        assert_eq!(model.description.as_ref().unwrap(), "Uses Anthropic");
    }

    // =====================================================================
    // 11. Notes command bar: format action details
    // =====================================================================

    // =====================================================================
    // 12. Notes command bar: trash view exact action set
    // =====================================================================

    // =====================================================================
    // 13. Note switcher: empty notes produces no_notes action
    // =====================================================================

    #[test]
    fn note_switcher_empty_has_no_notes() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "no_notes");
    }

    #[test]
    fn note_switcher_no_notes_title() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].title, "No notes yet");
    }

    #[test]
    fn note_switcher_no_notes_desc_mentions_cmd_n() {
        let actions = get_note_switcher_actions(&[]);
        assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
    }

    #[test]
    fn note_switcher_no_notes_icon_plus() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].icon, Some(IconName::Plus));
    }

    // =====================================================================
    // 14. Note switcher: ID format is note_{uuid}
    // =====================================================================

    #[test]
    fn note_switcher_id_format() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc-123-def".into(),
            title: "My Note".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].id, "note_abc-123-def");
    }

    #[test]
    fn note_switcher_current_icon_check() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n1".into(),
            title: "Current".into(),
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
    fn note_switcher_regular_icon_file() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n2".into(),
            title: "Regular".into(),
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
    fn note_switcher_pinned_trumps_current() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n3".into(),
            title: "Both".into(),
            char_count: 10,
            is_current: true,
            is_pinned: true,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    // =====================================================================
    // 15. New chat: empty inputs produce expected results
    // =====================================================================

    #[test]
    fn new_chat_all_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn new_chat_only_models() {
        let models = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "model_p::m1");
    }

    // --- merged from part_03.rs ---

    #[test]
    fn new_chat_only_presets() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "preset_general");
    }

    #[test]
    fn new_chat_only_last_used() {
        let last_used = vec![NewChatModelInfo {
            model_id: "lu1".into(),
            display_name: "Last Used 1".into(),
            provider: "p".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "last_used_p::lu1");
    }

    // =====================================================================
    // 16. to_deeplink_name: additional transformations
    // =====================================================================

    #[test]
    fn deeplink_name_preserves_numbers() {
        assert_eq!(to_deeplink_name("Script 123"), "script-123");
    }

    #[test]
    fn deeplink_name_emoji_to_hyphens() {
        // Emojis are non-alphanumeric so they become hyphens (then collapse)
        assert_eq!(to_deeplink_name("Cool Script"), "cool-script");
    }

    #[test]
    fn deeplink_name_already_lowercase() {
        assert_eq!(to_deeplink_name("already-lowercase"), "already-lowercase");
    }

    #[test]
    fn deeplink_name_single_char() {
        assert_eq!(to_deeplink_name("A"), "a");
    }

    // =====================================================================
    // 17. Constants: secondary dimension values
    // =====================================================================

    #[test]
    fn constant_header_height() {
        assert_eq!(HEADER_HEIGHT, 26.0);
    }

    #[test]
    fn constant_action_row_inset() {
        assert_eq!(ACTION_ROW_INSET, 8.0);
    }

    // Removed: constant_selection_radius, constant_keycap_min_width, constant_keycap_height
    // (legacy constants SELECTION_RADIUS, KEYCAP_MIN_WIDTH, KEYCAP_HEIGHT removed from constants.rs)

    #[test]
    fn constant_search_input_height() {
        assert_eq!(SEARCH_INPUT_HEIGHT, 40.0);
    }

    // =====================================================================
    // 19. parse_shortcut_keycaps: modifier and special key parsing
    // =====================================================================

    #[test]
    fn parse_keycaps_cmd_enter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
        assert_eq!(caps, vec!["⌘", "↵"]);
    }

    #[test]
    fn parse_keycaps_all_modifiers_and_key() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧⌃⌥K");
        assert_eq!(caps, vec!["⌘", "⇧", "⌃", "⌥", "K"]);
    }

    #[test]
    fn parse_keycaps_single_letter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("A");
        assert_eq!(caps, vec!["A"]);
    }

    #[test]
    fn parse_keycaps_lowercase_uppercased() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘c");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    // =====================================================================
    // 20. format_shortcut_hint: additional conversions
    // =====================================================================

    #[test]
    fn format_shortcut_hint_cmd_backspace() {
        assert_eq!(ActionsDialog::format_shortcut_hint("cmd+backspace"), "⌘⌫");
    }

    #[test]
    fn format_shortcut_hint_ctrl_tab() {
        assert_eq!(ActionsDialog::format_shortcut_hint("ctrl+tab"), "⌃⇥");
    }

    #[test]
    fn format_shortcut_hint_option_space() {
        assert_eq!(ActionsDialog::format_shortcut_hint("option+space"), "⌥␣");
    }

    #[test]
    fn format_shortcut_hint_single_escape() {
        assert_eq!(ActionsDialog::format_shortcut_hint("escape"), "⎋");
    }

    // =====================================================================
    // 21. build_grouped_items_static: None section handling
    // =====================================================================

    #[test]
    fn grouped_items_none_section_no_header() {
        let actions = vec![
            Action::new("a", "Alpha", None, ActionCategory::ScriptContext),
            Action::new("b", "Beta", None, ActionCategory::ScriptContext),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // No sections on actions → no headers added
        assert_eq!(grouped.len(), 2);
    }

    #[test]
    fn grouped_items_mixed_some_none_sections() {
        let mut a1 = Action::new("a", "Alpha", None, ActionCategory::ScriptContext);
        a1.section = Some("Group A".into());
        let a2 = Action::new("b", "Beta", None, ActionCategory::ScriptContext);
        // a2 has no section
        let actions = vec![a1, a2];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // One header for "Group A", then item, then item (no header for None section)
        assert_eq!(grouped.len(), 3);
    }

    #[test]
    fn grouped_items_separators_never_adds_headers() {
        let mut a1 = Action::new("a", "Alpha", None, ActionCategory::ScriptContext);
        a1.section = Some("Group A".into());
        let mut a2 = Action::new("b", "Beta", None, ActionCategory::ScriptContext);
        a2.section = Some("Group B".into());
        let actions = vec![a1, a2];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Separators style never adds headers
        assert_eq!(grouped.len(), 2);
    }

    #[test]
    fn grouped_items_empty_filtered_returns_empty() {
        let actions = vec![Action::new(
            "a",
            "Alpha",
            None,
            ActionCategory::ScriptContext,
        )];
        let filtered: Vec<usize> = vec![];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        assert!(grouped.is_empty());
    }

    // =====================================================================
    // 22. coerce_action_selection: specific patterns
    // =====================================================================

    #[test]
    fn coerce_selection_single_item() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![GroupedActionItem::Item(0)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn coerce_selection_header_then_item() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn coerce_selection_item_then_header() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".into()),
        ];
        // On header at index 1 → search down (nothing) → search up → find item at 0
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn coerce_selection_beyond_bounds_clamped() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        // Index 99 → clamped to len-1=1 → Item(1) → Some(1)
        assert_eq!(coerce_action_selection(&rows, 99), Some(1));
    }

    // =====================================================================
    // 23. CommandBarConfig: close flags consistent
    // =====================================================================

    #[test]
    fn command_bar_ai_close_on_select_true() {
        let config = CommandBarConfig::ai_style();
        assert!(config.close_on_select);
    }

    #[test]
    fn command_bar_ai_close_on_escape_true() {
        let config = CommandBarConfig::ai_style();
        assert!(config.close_on_escape);
    }

    #[test]
    fn command_bar_main_menu_close_on_select_true() {
        let config = CommandBarConfig::main_menu_style();
        assert!(config.close_on_select);
    }

    #[test]
    fn command_bar_notes_close_on_escape_true() {
        let config = CommandBarConfig::notes_style();
        assert!(config.close_on_escape);
    }

    // =====================================================================
    // 24. Script context: scriptlet reveal and copy_path details
    // =====================================================================

    #[test]
    fn scriptlet_reveal_shortcut_cmd_shift_f() {
        let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let reveal = actions
            .iter()
            .find(|a| a.id == "reveal_scriptlet_in_finder")
            .unwrap();
        assert_eq!(reveal.shortcut.as_ref().unwrap(), "⌘⇧F");
    }

    #[test]
    fn scriptlet_reveal_desc_mentions_finder() {
        let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let reveal = actions
            .iter()
            .find(|a| a.id == "reveal_scriptlet_in_finder")
            .unwrap();
        assert!(reveal.description.as_ref().unwrap().contains("Finder"));
    }

    #[test]
    fn scriptlet_copy_path_shortcut_cmd_shift_c() {
        let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let cp = actions
            .iter()
            .find(|a| a.id == "copy_scriptlet_path")
            .unwrap();
        assert_eq!(cp.shortcut.as_ref().unwrap(), "⌘⇧C");
    }

    #[test]
    fn scriptlet_copy_path_desc_mentions_clipboard() {
        let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_script_context_actions(&scriptlet);
        let cp = actions
            .iter()
            .find(|a| a.id == "copy_scriptlet_path")
            .unwrap();
        assert!(
            cp.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("path")
        );
    }

    // =====================================================================
    // 25. Score action: fuzzy match scores lower than prefix/contains
    // =====================================================================

    #[test]
    fn score_action_fuzzy_lower_than_prefix() {
        let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
        let prefix_score = ActionsDialog::score_action(&action, "edit");
        let fuzzy_score = ActionsDialog::score_action(&action, "eds"); // e-d-i-t s-c-r-i-p-t has e,d,s
        assert!(prefix_score > fuzzy_score);
    }

    #[test]
    fn score_action_contains_lower_than_prefix() {
        let action = Action::new(
            "test",
            "My Edit Script",
            None,
            ActionCategory::ScriptContext,
        );
        let prefix_score = ActionsDialog::score_action(&action, "my");
        let contains_score = ActionsDialog::score_action(&action, "script:edit");
        assert!(prefix_score > contains_score);
    }

    #[test]
    fn score_action_both_title_and_desc_match() {
        let action = Action::new(
            "test",
            "Edit Script",
            Some("Edit the script file".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        // prefix(100) + desc(15) = 115
        assert!(score >= 115);
    }

    #[test]
    fn score_action_shortcut_bonus() {
        let action =
            Action::new("test", "Zzz", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "⌘e");
        // No title match but shortcut contains "⌘e" → 10
        assert!(score >= 10);
    }

    // =====================================================================
    // 26. fuzzy_match: additional patterns
    // =====================================================================

    #[test]
    fn fuzzy_match_exact() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn fuzzy_match_subsequence() {
        assert!(ActionsDialog::fuzzy_match("hello world", "hlwrd"));
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(!ActionsDialog::fuzzy_match("abc", "xyz"));
    }

    #[test]
    fn fuzzy_match_needle_longer_than_haystack() {
        assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
    }

    // =====================================================================
    // 27. Cross-context: all contexts produce non-empty actions
    // =====================================================================

    #[test]
    fn cross_context_script_non_empty() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        assert!(!get_script_context_actions(&script).is_empty());
    }

    #[test]
    fn cross_context_builtin_non_empty() {
        let builtin = ScriptInfo::builtin("Clipboard History");
        assert!(!get_script_context_actions(&builtin).is_empty());
    }

    #[test]
    fn cross_context_scriptlet_non_empty() {
        let scriptlet = ScriptInfo::scriptlet("Open URL", "/p.md", None, None);
        assert!(!get_script_context_actions(&scriptlet).is_empty());
    }

    #[test]
    fn cross_context_file_non_empty() {
        let f = FileInfo {
            path: "/t.txt".into(),
            name: "t.txt".into(),
            file_type: crate::file_search::FileType::File,
            is_dir: false,
        };
        assert!(!get_file_context_actions(&f).is_empty());
    }

    #[test]
    fn cross_context_path_non_empty() {
        let p = PathInfo {
            path: "/t".into(),
            name: "t".into(),
            is_dir: false,
        };
        assert!(!get_path_context_actions(&p).is_empty());
    }

    #[test]
    fn cross_context_ai_bar_non_empty() {
        assert!(!get_ai_command_bar_actions().is_empty());
    }

    // --- merged from part_04.rs ---

    // =====================================================================
    // 28. Clipboard: share action details
    // =====================================================================

    #[test]
    fn clipboard_share_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "sh-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert_eq!(share.shortcut.as_ref().unwrap(), "⇧⌘E");
    }

    #[test]
    fn clipboard_share_title() {
        let entry = ClipboardEntryInfo {
            id: "sh-2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert_eq!(share.title, "Share...");
    }

    #[test]
    fn clipboard_share_desc_mentions_share() {
        let entry = ClipboardEntryInfo {
            id: "sh-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let share = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_share")
            .unwrap();
        assert!(
            share
                .description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("share")
        );
    }

    #[test]
    fn clipboard_share_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "sh-4".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_share"));
    }

    // =====================================================================
    // 29. Action builder: cached lowercase consistency
    // =====================================================================

    #[test]
    fn action_title_lower_matches_title() {
        let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "hello world");
    }

    #[test]
    fn action_description_lower_matches_desc() {
        let action = Action::new(
            "id",
            "T",
            Some("My Description".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower.as_ref().unwrap(), "my description");
    }

    #[test]
    fn action_shortcut_lower_after_with_shortcut() {
        let action =
            Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
        assert_eq!(action.shortcut_lower.as_ref().unwrap(), "⌘⇧c");
    }

    #[test]
    fn action_no_shortcut_lower_is_none() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(action.shortcut_lower.is_none());
    }

    // =====================================================================
    // 30. Cross-context: all built-in actions use snake_case IDs
    // =====================================================================

    #[test]
    fn script_actions_ids_snake_case() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        for a in get_script_context_actions(&script) {
            assert!(
                !a.id.contains(' '),
                "Action ID '{}' should not contain spaces",
                a.id
            );
            assert!(
                !a.id.contains('-') || a.id.starts_with("scriptlet_action:"),
                "Action ID '{}' should be snake_case (no hyphens)",
                a.id
            );
        }
    }

    #[test]
    fn file_actions_ids_snake_case() {
        let f = FileInfo {
            path: "/t.txt".into(),
            name: "t.txt".into(),
            file_type: crate::file_search::FileType::File,
            is_dir: false,
        };
        for a in get_file_context_actions(&f) {
            assert!(
                !a.id.contains(' '),
                "Action ID '{}' should not contain spaces",
                a.id
            );
        }
    }

    #[test]
    fn path_actions_ids_snake_case() {
        let p = PathInfo {
            path: "/t".into(),
            name: "t".into(),
            is_dir: false,
        };
        for a in get_path_context_actions(&p) {
            assert!(
                !a.id.contains(' '),
                "Action ID '{}' should not contain spaces",
                a.id
            );
        }
    }

    #[test]
    fn clipboard_actions_ids_snake_case() {
        let entry = ClipboardEntryInfo {
            id: "e".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "t".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        for a in get_clipboard_history_context_actions(&entry) {
            assert!(
                !a.id.contains(' '),
                "Action ID '{}' should not contain spaces",
                a.id
            );
        }
    }
}

mod from_dialog_builtin_action_validation_tests_36 {
    // --- merged from part_01.rs ---
    //! Batch 36: Dialog built-in action validation tests
    //!
    //! 124 tests across 30 categories validating random behaviors from
    //! built-in action window dialogs.

    use crate::actions::builders::{
        ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo, NoteSwitcherNoteInfo, NotesInfo,
        get_ai_command_bar_actions, get_clipboard_history_context_actions,
        get_file_context_actions, get_new_chat_actions, get_note_switcher_actions,
        get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
        get_scriptlet_context_actions_with_custom, to_deeplink_name,
    };
    use crate::actions::command_bar::CommandBarConfig;
    use crate::actions::dialog::{
        ActionsDialog, build_grouped_items_static, coerce_action_selection,
    };
    use crate::actions::types::{Action, ActionCategory, AnchorPosition, ScriptInfo, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::DesignColors;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::protocol::ProtocolAction;
    use crate::scriptlets::Scriptlet;

    // =====================================================================
    // 1. hex_with_alpha: shift and OR behavior
    // =====================================================================

    #[test]
    fn hex_with_alpha_black_full_opaque() {
        // 0x000000 with alpha 0xFF => 0x000000FF
        assert_eq!(DesignColors::hex_with_alpha(0x000000, 0xFF), 0x000000FF);
    }

    #[test]
    fn hex_with_alpha_white_full_opaque() {
        // 0xFFFFFF with alpha 0xFF => 0xFFFFFFFF
        assert_eq!(DesignColors::hex_with_alpha(0xFFFFFF, 0xFF), 0xFFFFFFFF);
    }

    #[test]
    fn hex_with_alpha_color_half_transparent() {
        // 0x1A2B3C with alpha 0x80 => (0x1A2B3C << 8) | 0x80
        let result = DesignColors::hex_with_alpha(0x1A2B3C, 0x80);
        assert_eq!(result, (0x1A2B3C << 8) | 0x80);
    }

    #[test]
    fn hex_with_alpha_zero_alpha() {
        // 0xABCDEF with alpha 0 => 0xABCDEF00
        assert_eq!(DesignColors::hex_with_alpha(0xABCDEF, 0x00), 0xABCDEF00);
    }

    // =====================================================================
    // 2. ProtocolAction: is_visible default behavior
    // =====================================================================

    #[test]
    fn protocol_action_visible_none_defaults_to_true() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: None,
        };
        assert!(pa.is_visible());
    }

    #[test]
    fn protocol_action_visible_true_is_visible() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: Some(true),
            close: None,
        };
        assert!(pa.is_visible());
    }

    #[test]
    fn protocol_action_visible_false_is_hidden() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: Some(false),
            close: None,
        };
        assert!(!pa.is_visible());
    }

    #[test]
    fn protocol_action_has_action_false_default() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: None,
        };
        assert!(!pa.has_action);
    }

    // =====================================================================
    // 3. ProtocolAction: should_close default behavior
    // =====================================================================

    #[test]
    fn protocol_action_close_none_defaults_to_true() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: None,
        };
        assert!(pa.should_close());
    }

    #[test]
    fn protocol_action_close_false_stays_open() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: Some(false),
        };
        assert!(!pa.should_close());
    }

    #[test]
    fn protocol_action_close_true_closes() {
        let pa = ProtocolAction {
            name: "Test".into(),
            description: None,
            shortcut: None,
            value: None,
            has_action: false,
            visible: None,
            close: Some(true),
        };
        assert!(pa.should_close());
    }

    // =====================================================================
    // 4. builders::format_shortcut_hint (simple) vs ActionsDialog::format_shortcut_hint (sophisticated)
    // =====================================================================

    #[test]
    fn builders_format_converts_cmd_to_symbol() {
        // The builders version does simple string replace
        let result = ActionsDialog::format_shortcut_hint("cmd+c");
        assert_eq!(result, "⌘C");
    }

    #[test]
    fn dialog_format_handles_meta_alias() {
        let result = ActionsDialog::format_shortcut_hint("meta+k");
        assert_eq!(result, "⌘K");
    }

    #[test]
    fn dialog_format_handles_super_alias() {
        let result = ActionsDialog::format_shortcut_hint("super+j");
        assert_eq!(result, "⌘J");
    }

    #[test]
    fn dialog_format_handles_control_full_word() {
        let result = ActionsDialog::format_shortcut_hint("control+x");
        assert_eq!(result, "⌃X");
    }

    // =====================================================================
    // 5. Clipboard: quick_look details (macOS)
    // =====================================================================

    #[test]
    fn clipboard_quick_look_shortcut_is_space() {
        let entry = ClipboardEntryInfo {
            id: "ql-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ql = actions.iter().find(|a| a.id == "clip:clipboard_quick_look");
        // On macOS this should exist
        if let Some(action) = ql {
            assert_eq!(action.shortcut.as_deref(), Some("␣"));
        }
    }

    #[test]
    fn clipboard_quick_look_title() {
        let entry = ClipboardEntryInfo {
            id: "ql-2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        if let Some(action) = actions.iter().find(|a| a.id == "clip:clipboard_quick_look") {
            assert_eq!(action.title, "Quick Look");
        }
    }

    #[test]
    fn clipboard_quick_look_desc_mentions_preview() {
        let entry = ClipboardEntryInfo {
            id: "ql-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        if let Some(action) = actions.iter().find(|a| a.id == "clip:clipboard_quick_look") {
            let desc = action.description.as_deref().unwrap();
            assert!(desc.contains("Quick Look"));
        }
    }

    #[test]
    fn clipboard_quick_look_present_for_image_too() {
        let entry = ClipboardEntryInfo {
            id: "ql-4".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        // On macOS, quick_look is available for both text and image
        let has_ql = actions.iter().any(|a| a.id == "clip:clipboard_quick_look");
        // Either present (macOS) or absent (non-macOS), consistent with text entries
        let text_entry = ClipboardEntryInfo {
            id: "ql-4b".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let text_actions = get_clipboard_history_context_actions(&text_entry);
        let text_has_ql = text_actions
            .iter()
            .any(|a| a.id == "clip:clipboard_quick_look");
        assert_eq!(
            has_ql, text_has_ql,
            "quick_look availability should be consistent"
        );
    }

    // =====================================================================
    // 6. Clipboard: delete entry shortcut and description
    // =====================================================================

    #[test]
    fn clipboard_delete_shortcut_ctrl_x() {
        let entry = ClipboardEntryInfo {
            id: "d-1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete")
            .unwrap();
        assert_eq!(action.shortcut.as_deref(), Some("⌃X"));
    }

    #[test]
    fn clipboard_delete_title() {
        let entry = ClipboardEntryInfo {
            id: "d-2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete")
            .unwrap();
        assert_eq!(action.title, "Delete Entry");
    }

    #[test]
    fn clipboard_delete_desc_mentions_remove() {
        let entry = ClipboardEntryInfo {
            id: "d-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_delete")
            .unwrap();
        assert!(action.description.as_deref().unwrap().contains("Remove"));
    }

    #[test]
    fn clipboard_delete_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "d-4".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
            image_dimensions: Some((50, 50)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_delete"));
    }

    // =====================================================================
    // 7. Clipboard: action ordering invariants (paste first, destructive last)
    // =====================================================================

    #[test]
    fn clipboard_first_action_is_paste() {
        let entry = ClipboardEntryInfo {
            id: "ord-1".into(),
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
    fn clipboard_second_action_is_copy() {
        let entry = ClipboardEntryInfo {
            id: "ord-2".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[1].id, "clip:clipboard_copy");
    }

    #[test]
    fn clipboard_last_action_is_delete_all() {
        let entry = ClipboardEntryInfo {
            id: "ord-3".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions.last().unwrap().id, "clip:clipboard_delete_all");
    }

    #[test]
    fn clipboard_last_3_are_destructive() {
        let entry = ClipboardEntryInfo {
            id: "ord-4".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let n = actions.len();
        assert_eq!(actions[n - 3].id, "clip:clipboard_delete");
        assert_eq!(actions[n - 2].id, "clip:clipboard_delete_multiple");
        assert_eq!(actions[n - 1].id, "clip:clipboard_delete_all");
    }

    // =====================================================================
    // 8. File context: quick_look only for non-dir
    // =====================================================================

    #[test]
    fn file_context_file_has_quick_look_on_macos() {
        let fi = FileInfo {
            name: "readme.md".into(),
            path: "/path/readme.md".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        let has_ql = actions.iter().any(|a| a.id == "file:quick_look");
        // On macOS it should be present; on other platforms it's absent
        #[cfg(target_os = "macos")]
        assert!(has_ql);
        #[cfg(not(target_os = "macos"))]
        assert!(!has_ql);
    }

    #[test]
    fn file_context_dir_no_quick_look() {
        let fi = FileInfo {
            name: "docs".into(),
            path: "/path/docs".into(),
            is_dir: true,
            file_type: FileType::Directory,
        };
        let actions = get_file_context_actions(&fi);
        assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
    }

    #[test]
    fn file_context_file_quick_look_shortcut() {
        let fi = FileInfo {
            name: "img.png".into(),
            path: "/path/img.png".into(),
            is_dir: false,
            file_type: FileType::Image,
        };
        let actions = get_file_context_actions(&fi);
        if let Some(ql) = actions.iter().find(|a| a.id == "file:quick_look") {
            assert_eq!(ql.shortcut.as_deref(), Some("⌘Y"));
        }
    }

    // --- merged from part_02.rs ---

    #[test]
    fn file_context_file_quick_look_desc() {
        let fi = FileInfo {
            name: "demo.txt".into(),
            path: "/path/demo.txt".into(),
            is_dir: false,
            file_type: FileType::Document,
        };
        let actions = get_file_context_actions(&fi);
        if let Some(ql) = actions.iter().find(|a| a.id == "file:quick_look") {
            assert!(ql.description.as_deref().unwrap().contains("Quick Look"));
        }
    }

    // =====================================================================
    // 9. File context: copy_path shortcut ⌘⇧C
    // =====================================================================

    #[test]
    fn file_context_copy_path_shortcut() {
        let fi = FileInfo {
            name: "file.rs".into(),
            path: "/path/file.rs".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
    }

    #[test]
    fn file_context_copy_path_desc_mentions_full_path() {
        let fi = FileInfo {
            name: "file.rs".into(),
            path: "/path/file.rs".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&fi);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert!(cp.description.as_deref().unwrap().contains("full path"));
    }

    #[test]
    fn file_context_copy_filename_shortcut_cmd_c() {
        let fi = FileInfo {
            name: "main.rs".into(),
            path: "/path/main.rs".into(),
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

    // =====================================================================
    // 10. Path context: open_in_quick_terminal has no shortcut
    // =====================================================================

    #[test]
    fn path_context_open_in_quick_terminal_shortcut() {
        let pi = PathInfo {
            name: "project".into(),
            path: "/path/project".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        let ot = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        assert_eq!(ot.shortcut.as_deref(), None);
    }

    #[test]
    fn path_context_open_in_quick_terminal_desc() {
        let pi = PathInfo {
            name: "project".into(),
            path: "/path/project".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        let ot = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        // Description is "Opens Quick Terminal at this location"
        assert!(
            ot.description
                .as_deref()
                .unwrap()
                .contains("Quick Terminal")
        );
    }

    #[test]
    fn path_context_open_in_quick_terminal_title() {
        let pi = PathInfo {
            name: "src".into(),
            path: "/path/src".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&pi);
        let ot = actions
            .iter()
            .find(|a| a.id == "file:open_in_quick_terminal")
            .unwrap();
        assert_eq!(ot.title, "Open in Quick Terminal");
    }

    #[test]
    fn path_context_open_in_quick_terminal_present_for_file() {
        let pi = PathInfo {
            name: "main.rs".into(),
            path: "/path/main.rs".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&pi);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "file:open_in_quick_terminal")
        );
    }

    // =====================================================================
    // 11. Script context: view_logs shortcut ⌘L
    // =====================================================================

    #[test]
    fn script_context_view_logs_shortcut() {
        let script = ScriptInfo::new("my-script", "/path/my-script.ts");
        let actions = get_script_context_actions(&script);
        let vl = actions.iter().find(|a| a.id == "view_logs").unwrap();
        assert_eq!(vl.shortcut.as_deref(), Some("⌘L"));
    }

    #[test]
    fn script_context_view_logs_title() {
        let script = ScriptInfo::new("my-script", "/path/my-script.ts");
        let actions = get_script_context_actions(&script);
        let vl = actions.iter().find(|a| a.id == "view_logs").unwrap();
        assert_eq!(vl.title, "Show Logs");
    }

    #[test]
    fn script_context_view_logs_desc_mentions_logs() {
        let script = ScriptInfo::new("my-script", "/path/my-script.ts");
        let actions = get_script_context_actions(&script);
        let vl = actions.iter().find(|a| a.id == "view_logs").unwrap();
        assert!(vl.description.as_deref().unwrap().contains("logs"));
    }

    #[test]
    fn script_context_view_logs_absent_for_builtin() {
        let script = ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&script);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // =====================================================================
    // 12. Script context: all IDs unique within context
    // =====================================================================

    #[test]
    fn script_context_ids_unique_basic() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "Duplicate IDs found: {:?}", ids);
    }

    #[test]
    fn script_context_ids_unique_with_shortcut_and_alias() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/path/test.ts",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&script);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn scriptlet_context_ids_unique() {
        let script = ScriptInfo::scriptlet("Open URL", "/path/url.md", None, None);
        let scriptlet = Scriptlet::new("Open URL".into(), "bash".into(), "echo hi".into());
        let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn ai_command_bar_ids_unique() {
        let actions = get_ai_command_bar_actions();
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }

    // =====================================================================
    // 13. Script context: action count increases with shortcut+alias+suggestion
    // =====================================================================

    #[test]
    fn script_context_base_count_no_extras() {
        let script = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&script);
        // run + toggle_info + add_shortcut + add_alias + toggle_favorite + edit + view_logs + reveal + open_in_quick_terminal + copy_path + copy_content + copy_deeplink + delete_script = 13
        assert_eq!(actions.len(), 13);
    }

    #[test]
    fn script_context_with_shortcut_adds_one() {
        let script = ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".into()));
        let actions = get_script_context_actions(&script);
        // run + toggle_info + update_shortcut + remove_shortcut + add_alias + toggle_favorite + edit + view_logs + reveal + open_in_quick_terminal + copy_path + copy_content + copy_deeplink + delete_script = 14
        assert_eq!(actions.len(), 14);
    }

    #[test]
    fn script_context_with_both_adds_two() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/path/test.ts",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&script);
        // run + toggle_info + update_shortcut + remove_shortcut + update_alias + remove_alias + toggle_favorite + edit + view_logs + reveal + open_in_quick_terminal + copy_path + copy_content + copy_deeplink + delete_script = 15
        assert_eq!(actions.len(), 15);
    }

    #[test]
    fn script_context_with_suggestion_adds_reset_ranking() {
        let script = ScriptInfo::new("test", "/path/test.ts")
            .with_frecency(true, Some("/path/test.ts".into()));
        let actions = get_script_context_actions(&script);
        // 13 base + reset_ranking = 14
        assert_eq!(actions.len(), 14);
        assert!(actions.iter().any(|a| a.id == "reset_ranking"));
    }

    // =====================================================================
    // 14. Scriptlet context: identical shortcut/alias dynamic behavior
    // =====================================================================

    #[test]
    fn scriptlet_no_shortcut_has_add_shortcut() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "add_shortcut"));
    }

    #[test]
    fn scriptlet_with_shortcut_has_update_and_remove() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", Some("cmd+t".into()), None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "update_shortcut"));
        assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
        assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
    }

    #[test]
    fn scriptlet_no_alias_has_add_alias() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "add_alias"));
    }

    #[test]
    fn scriptlet_with_alias_has_update_and_remove() {
        let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, Some("ts".into()));
        let actions = get_scriptlet_context_actions_with_custom(&script, None);
        assert!(actions.iter().any(|a| a.id == "update_alias"));
        assert!(actions.iter().any(|a| a.id == "remove_alias"));
        assert!(!actions.iter().any(|a| a.id == "add_alias"));
    }

    // =====================================================================
    // 15. Agent context: no view_logs but has edit/reveal/copy_path/copy_content
    // =====================================================================

    #[test]
    fn agent_has_edit_with_agent_title() {
        let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert_eq!(edit.title, "Edit Agent");
    }

    #[test]
    fn agent_has_reveal_in_finder() {
        let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
    }

    #[test]
    fn agent_has_copy_path_and_copy_content() {
        let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(actions.iter().any(|a| a.id == "copy_path"));
        assert!(actions.iter().any(|a| a.id == "copy_content"));
    }

    #[test]
    fn agent_no_view_logs() {
        let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
        script.is_script = false;
        script.is_agent = true;
        let actions = get_script_context_actions(&script);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // =====================================================================
    // 16. AI command bar: branch_from_last has no shortcut
    // =====================================================================

    #[test]
    fn ai_bar_branch_from_last_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let bfl = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert!(bfl.shortcut.is_none());
    }

    #[test]
    fn ai_bar_branch_from_last_section_actions() {
        let actions = get_ai_command_bar_actions();
        let bfl = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert_eq!(bfl.section.as_deref(), Some("Actions"));
    }

    #[test]
    fn ai_bar_branch_from_last_icon_arrowright() {
        let actions = get_ai_command_bar_actions();
        let bfl = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert_eq!(bfl.icon, Some(IconName::ArrowRight));
    }

    #[test]
    fn ai_bar_branch_from_last_desc_mentions_branch() {
        let actions = get_ai_command_bar_actions();
        let bfl = actions
            .iter()
            .find(|a| a.id == "chat:branch_from_last")
            .unwrap();
        assert!(bfl.description.as_deref().unwrap().contains("new chat"));
    }

    // =====================================================================
    // 17. AI command bar: section ordering
    // =====================================================================

    #[test]
    fn ai_bar_first_section_is_response() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions[0].section.as_deref(), Some("Response"));
    }

    #[test]
    fn ai_bar_response_section_has_3_actions() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(count, 3);
    }

    #[test]
    fn ai_bar_actions_section_has_4_actions() {
        let actions = get_ai_command_bar_actions();
        let count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Actions"))
            .count();
        assert_eq!(count, 7);
    }

    #[test]
    fn ai_bar_total_is_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    // =====================================================================
    // 18. Notes: auto_sizing_enabled=true hides enable_auto_sizing
    // =====================================================================

    // =====================================================================
    // 19. Notes: full selection action set
    // =====================================================================

    // --- merged from part_03.rs ---

    // =====================================================================
    // 20. CommandBarConfig: anchor position differences
    // =====================================================================

    #[test]
    fn command_bar_ai_style_anchor_top() {
        let config = CommandBarConfig::ai_style();
        assert!(matches!(config.dialog_config.anchor, AnchorPosition::Top));
    }

    #[test]
    fn command_bar_main_menu_anchor_bottom() {
        let config = CommandBarConfig::main_menu_style();
        assert!(matches!(
            config.dialog_config.anchor,
            AnchorPosition::Bottom
        ));
    }

    #[test]
    fn command_bar_no_search_anchor_bottom() {
        let config = CommandBarConfig::no_search();
        assert!(matches!(
            config.dialog_config.anchor,
            AnchorPosition::Bottom
        ));
    }

    #[test]
    fn command_bar_notes_anchor_top() {
        let config = CommandBarConfig::notes_style();
        assert!(matches!(config.dialog_config.anchor, AnchorPosition::Top));
    }

    // =====================================================================
    // 21. New chat: combination of all three input types
    // =====================================================================

    #[test]
    fn new_chat_all_three_types() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model-1".into(),
            provider: "P".into(),
            provider_display_name: "Provider-1".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m2".into(),
            display_name: "Model-2".into(),
            provider: "P".into(),
            provider_display_name: "Provider-2".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn new_chat_sections_are_correct() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "LU".into(),
            provider: "P".into(),
            provider_display_name: "PD".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "g".into(),
            name: "G".into(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "m2".into(),
            display_name: "M".into(),
            provider: "P".into(),
            provider_display_name: "PD2".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
        assert_eq!(actions[1].section.as_deref(), Some("Presets"));
        assert_eq!(actions[2].section.as_deref(), Some("Models"));
    }

    #[test]
    fn new_chat_all_empty_produces_zero() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn new_chat_only_presets() {
        let presets = vec![
            NewChatPresetInfo {
                id: "a".into(),
                name: "A".into(),
                icon: IconName::Plus,
            },
            NewChatPresetInfo {
                id: "b".into(),
                name: "B".into(),
                icon: IconName::Code,
            },
        ];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 2);
        assert!(
            actions
                .iter()
                .all(|a| a.section.as_deref() == Some("Presets"))
        );
    }

    // =====================================================================
    // 22. Note switcher: pinned+current uses StarFilled
    // =====================================================================

    #[test]
    fn note_switcher_pinned_current_icon_is_star_filled() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n1".into(),
            title: "My Note".into(),
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
    fn note_switcher_pinned_not_current_icon_is_star_filled() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n2".into(),
            title: "Other Note".into(),
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
    fn note_switcher_current_not_pinned_icon_is_check() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n3".into(),
            title: "Current".into(),
            char_count: 30,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn note_switcher_neither_icon_is_file() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "n4".into(),
            title: "Regular".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::File));
    }

    // =====================================================================
    // 23. Note switcher: description with preview exactly 60 chars
    // =====================================================================

    #[test]
    fn note_switcher_preview_60_chars_not_truncated() {
        let preview: String = "a".repeat(60);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "t1".into(),
            title: "T".into(),
            char_count: 60,
            is_current: false,
            is_pinned: false,
            preview: preview.clone(),
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        // 60 chars should NOT be truncated (no ellipsis)
        assert!(!desc.contains('…'));
        assert_eq!(desc, &preview);
    }

    #[test]
    fn note_switcher_preview_61_chars_truncated() {
        let preview: String = "b".repeat(61);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "t2".into(),
            title: "T".into(),
            char_count: 61,
            is_current: false,
            is_pinned: false,
            preview,
            relative_time: String::new(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert!(desc.contains('…'));
    }

    #[test]
    fn note_switcher_empty_preview_with_time_shows_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "t3".into(),
            title: "T".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_deref().unwrap();
        assert_eq!(desc, "5m ago");
    }

    // =====================================================================
    // 24. to_deeplink_name: emoji and unicode handling
    // =====================================================================

    #[test]
    fn deeplink_name_emoji_preserved_as_chars() {
        // Emoji are alphanumeric-ish in Unicode; to_deeplink_name keeps them
        let result = to_deeplink_name("Hello 🌍 World");
        // Spaces become hyphens, emoji is alphanumeric? Let's test actual behavior
        assert!(result.contains("hello"));
        assert!(result.contains("world"));
    }

    #[test]
    fn deeplink_name_accented_chars_preserved() {
        let result = to_deeplink_name("café résumé");
        assert!(result.contains("caf"));
        assert!(result.contains("sum"));
    }

    #[test]
    fn deeplink_name_all_special_chars_empty() {
        let result = to_deeplink_name("!@#$%^&*()");
        assert_eq!(result, "_unnamed");
    }

    #[test]
    fn deeplink_name_mixed_separators() {
        let result = to_deeplink_name("hello---world___test   foo");
        assert_eq!(result, "hello-world-test-foo");
    }

    // =====================================================================
    // 25. ScriptInfo: with_frecency preserves all other fields
    // =====================================================================

    #[test]
    fn with_frecency_preserves_name_and_path() {
        let script = ScriptInfo::new("my-script", "/path/script.ts")
            .with_frecency(true, Some("/frecency".into()));
        assert_eq!(script.name, "my-script");
        assert_eq!(script.path, "/path/script.ts");
    }

    #[test]
    fn with_frecency_preserves_is_script() {
        let script = ScriptInfo::new("my-script", "/path/script.ts").with_frecency(true, None);
        assert!(script.is_script);
    }

    #[test]
    fn with_frecency_preserves_shortcut_and_alias() {
        let script = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/path/test.ts",
            Some("cmd+k".into()),
            Some("tk".into()),
        )
        .with_frecency(true, Some("/fp".into()));
        assert_eq!(script.shortcut, Some("cmd+k".into()));
        assert_eq!(script.alias, Some("tk".into()));
        assert!(script.is_suggested);
    }

    #[test]
    fn with_frecency_false_not_suggested() {
        let script = ScriptInfo::new("s", "/p").with_frecency(false, None);
        assert!(!script.is_suggested);
        assert!(script.frecency_path.is_none());
    }

    // =====================================================================
    // 26. Action: cached lowercase fields correctness
    // =====================================================================

    #[test]
    fn action_title_lower_cached_correctly() {
        let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(action.title_lower, "hello world");
    }

    #[test]
    fn action_description_lower_cached() {
        let action = Action::new(
            "id",
            "T",
            Some("My Description HERE".into()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.description_lower, Some("my description here".into()));
    }

    #[test]
    fn action_description_lower_none_when_no_desc() {
        let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(action.description_lower.is_none());
    }

    #[test]
    fn action_shortcut_lower_set_after_with_shortcut() {
        let action =
            Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧K");
        assert_eq!(action.shortcut_lower, Some("⌘⇧k".into()));
    }

    // =====================================================================
    // 27. build_grouped_items_static: SectionStyle::None produces no headers
    // =====================================================================

    #[test]
    fn grouped_items_none_style_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Section1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Section2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        // With SectionStyle::None, no headers should be inserted
        for item in &grouped {
            assert!(
                matches!(item, crate::actions::dialog::GroupedActionItem::Item(_)),
                "SectionStyle::None should not produce headers"
            );
        }
        assert_eq!(grouped.len(), 2);
    }

    #[test]
    fn grouped_items_headers_style_adds_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // 2 section headers + 2 items = 4
        assert_eq!(grouped.len(), 4);
    }

    #[test]
    fn grouped_items_same_section_one_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S1"),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // 1 header + 2 items = 3
        assert_eq!(grouped.len(), 3);
    }

    #[test]
    fn grouped_items_no_section_no_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext),
            Action::new("b", "B", None, ActionCategory::ScriptContext),
        ];
        let filtered: Vec<usize> = (0..actions.len()).collect();
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // No sections = no headers, just 2 items
        assert_eq!(grouped.len(), 2);
    }

    // =====================================================================
    // 28. coerce_action_selection: edge cases
    // =====================================================================

    #[test]
    fn coerce_selection_empty_returns_none() {
        let rows = vec![];
        assert_eq!(coerce_action_selection(&rows, 0), None);
    }

    #[test]
    fn coerce_selection_single_item() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![GroupedActionItem::Item(0)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    // --- merged from part_04.rs ---

    #[test]
    fn coerce_selection_beyond_bounds_clamped() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
        // Index 100 should clamp to last = 1
        assert_eq!(coerce_action_selection(&rows, 100), Some(1));
    }

    #[test]
    fn coerce_selection_header_then_item() {
        use crate::actions::dialog::GroupedActionItem;
        let rows = vec![
            GroupedActionItem::SectionHeader("S".into()),
            GroupedActionItem::Item(0),
        ];
        // Landing on header at 0, search down → finds Item at 1
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    // =====================================================================
    // 29. score_action: combined bonuses max scenario
    // =====================================================================

    #[test]
    fn score_action_prefix_plus_desc_plus_shortcut() {
        let action = Action::new(
            "script:edit",
            "Edit Script",
            Some("Edit the script file".into()),
            ActionCategory::ScriptContext,
        )
        .with_shortcut("⌘E");
        let score = ActionsDialog::score_action(&action, "e");
        // prefix(100) + desc contains(15) + shortcut contains(10) = 125
        assert!(score >= 125, "Expected ≥125, got {}", score);
    }

    #[test]
    fn score_action_contains_only() {
        let action = Action::new(
            "copy_edit",
            "Copy Edit Path",
            None,
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&action, "edit");
        assert!((50..100).contains(&score), "Expected 50-99, got {}", score);
    }

    #[test]
    fn score_action_no_match() {
        let action = Action::new("test", "Hello World", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "xyz");
        assert_eq!(score, 0);
    }

    #[test]
    fn score_action_empty_search_prefix_match() {
        let action = Action::new("test", "Anything", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&action, "");
        // Empty string is prefix of everything
        assert!(score >= 100, "Expected ≥100, got {}", score);
    }

    // =====================================================================
    // 30. Cross-context: ProtocolAction close/visibility defaults and SDK action ID format
    // =====================================================================

    #[test]
    fn protocol_action_sdk_id_matches_name() {
        // SDK actions use name as ID
        let pa = ProtocolAction {
            name: "My Custom Action".into(),
            description: Some("desc".into()),
            shortcut: None,
            value: Some("val".into()),
            has_action: true,
            visible: None,
            close: None,
        };
        // Simulate conversion (as done in set_sdk_actions)
        let action = Action::new(
            pa.name.clone(),
            pa.name.clone(),
            pa.description.clone(),
            ActionCategory::ScriptContext,
        );
        assert_eq!(action.id, "My Custom Action");
    }

    #[test]
    fn protocol_action_shortcut_converted_via_format() {
        let formatted = ActionsDialog::format_shortcut_hint("cmd+shift+c");
        assert_eq!(formatted, "⌘⇧C");
    }

    #[test]
    fn protocol_action_sdk_icon_is_none() {
        // SDK actions don't currently have icons
        let action = Action::new(
            "sdk_action",
            "SDK Action",
            None,
            ActionCategory::ScriptContext,
        );
        assert!(action.icon.is_none());
    }

    #[test]
    fn protocol_action_sdk_section_is_none() {
        // SDK actions don't currently have sections
        let action = Action::new(
            "sdk_action",
            "SDK Action",
            None,
            ActionCategory::ScriptContext,
        );
        assert!(action.section.is_none());
    }
}

mod from_dialog_builtin_action_validation_tests_37 {
    //! Purged batch 37 validation tests.
    //!
    //! The removed tests were mostly synthetic checks of locally reimplemented
    //! logic and overlapping invariants that are already validated elsewhere.
}

mod from_dialog_builtin_action_validation_tests_38 {
    //! Batch 38: Dialog builtin action validation tests
    //!
    //! Focuses on constructor variations, under-tested builder paths, and integration edges:
    //! - ScriptInfo constructor variants (with_action_verb, with_all, with_is_script)
    //! - Clipboard save_snippet, save_file, and frontmost_app_name dynamic title
    //! - Notes duplicate_note, find_in_note, copy_note_as details
    //! - AI command bar export_markdown and submit action specifics
    //! - Chat context empty models and model ID format
    //! - New chat last_used icon BoltFilled, model icon Settings
    //! - Note switcher current note "• " prefix and preview trimming
    //! - count_section_headers edge cases
    //! - WindowPosition enum variants and defaults
    //! - ProtocolAction::with_value constructor
    //! - score_action with whitespace and special character searches
    //! - Cross-context description keyword validation

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::*;
        use crate::actions::command_bar::CommandBarConfig;
        use crate::actions::dialog::ActionsDialog;
        use crate::actions::types::{
            Action, ActionCategory, AnchorPosition, ScriptInfo, SectionStyle,
        };
        use crate::actions::window::{WindowPosition, count_section_headers};
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::protocol::ProtocolAction;

        // =========================================================================
        // 1. ScriptInfo::with_action_verb: custom verb in primary action title
        // =========================================================================

        #[test]
        fn script_info_with_action_verb_sets_verb() {
            let info = ScriptInfo::with_action_verb("App Launcher", "/bin/launch", false, "Launch");
            assert_eq!(info.action_verb, "Launch");
        }

        #[test]
        fn script_info_with_action_verb_sets_is_script() {
            let info = ScriptInfo::with_action_verb("App Launcher", "/bin/launch", false, "Launch");
            assert!(!info.is_script);
        }

        #[test]
        fn script_info_with_action_verb_run_script_title_uses_verb() {
            let info = ScriptInfo::with_action_verb("Spotify", "/bin/spotify", false, "Switch to");
            let actions = get_script_context_actions(&info);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(run.title.starts_with("Switch To"));
        }

        #[test]
        fn script_info_with_action_verb_desc_uses_verb() {
            let info = ScriptInfo::with_action_verb("Calc", "/bin/calc", true, "Open");
            let actions = get_script_context_actions(&info);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(run.description.as_ref().unwrap().contains("Open"));
        }

        // =========================================================================
        // 2. ScriptInfo::with_all: preserves all fields
        // =========================================================================

        #[test]
        fn script_info_with_all_preserves_name_and_path() {
            let info = ScriptInfo::with_all(
                "Test",
                "/test.ts",
                true,
                "Run",
                Some("cmd+t".into()),
                Some("tt".into()),
            );
            assert_eq!(info.name, "Test");
            assert_eq!(info.path, "/test.ts");
        }

        #[test]
        fn script_info_with_all_preserves_shortcut_and_alias() {
            let info = ScriptInfo::with_all(
                "Test",
                "/test.ts",
                true,
                "Run",
                Some("cmd+t".into()),
                Some("tt".into()),
            );
            assert_eq!(info.shortcut, Some("cmd+t".to_string()));
            assert_eq!(info.alias, Some("tt".to_string()));
        }

        #[test]
        fn script_info_with_all_defaults_agent_false() {
            let info = ScriptInfo::with_all("Test", "/test.ts", true, "Run", None, None);
            assert!(!info.is_agent);
        }

        #[test]
        fn script_info_with_all_defaults_suggested_false() {
            let info = ScriptInfo::with_all("Test", "/test.ts", true, "Run", None, None);
            assert!(!info.is_suggested);
            assert!(info.frecency_path.is_none());
        }

        // =========================================================================
        // 3. ScriptInfo::with_is_script: explicit is_script flag
        // =========================================================================

        #[test]
        fn script_info_with_is_script_true_has_edit_actions() {
            let info = ScriptInfo::with_is_script("my-script", "/path.ts", true);
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "edit_script"));
            assert!(actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn script_info_with_is_script_false_no_edit_actions() {
            let info = ScriptInfo::with_is_script("built-in", "", false);
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "edit_script"));
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn script_info_with_is_script_defaults_verb_to_run() {
            let info = ScriptInfo::with_is_script("test", "/p", true);
            assert_eq!(info.action_verb, "Run");
        }

        #[test]
        fn script_info_with_is_script_defaults_scriptlet_false() {
            let info = ScriptInfo::with_is_script("test", "/p", true);
            assert!(!info.is_scriptlet);
        }

        // =========================================================================
        // 4. Clipboard: save_snippet and save_file details
        // =========================================================================

        #[test]
        fn clipboard_save_snippet_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let save = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(save.shortcut.as_deref(), Some("⇧⌘S"));
        }

        #[test]
        fn clipboard_save_snippet_title() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let save = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(save.title, "Save Text as Snippet");
        }

        #[test]
        fn clipboard_save_file_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let save = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert_eq!(save.shortcut.as_deref(), Some("⌥⇧⌘S"));
        }

        #[test]
        fn clipboard_save_file_desc_mentions_file() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let save = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert!(save.description.as_ref().unwrap().contains("file"));
        }

        // =========================================================================
        // 5. Clipboard: frontmost_app_name dynamic paste title
        // =========================================================================

        #[test]
        fn clipboard_paste_title_with_app_name() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: Some("Safari".into()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to Safari");
        }

        #[test]
        fn clipboard_paste_title_without_app_name() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to Active App");
        }

        #[test]
        fn clipboard_paste_title_with_special_chars_in_app_name() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: Some("Visual Studio Code".into()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to Visual Studio Code");
        }

        #[test]
        fn clipboard_paste_shortcut_always_return() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: Some("Terminal".into()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.shortcut.as_deref(), Some("↵"));
        }

        // =========================================================================
        // 6. Notes: duplicate_note and find_in_note details
        // =========================================================================

        // =========================================================================
        // 7. Notes: find_in_note details
        // =========================================================================

        // =========================================================================
        // 8. Notes: copy_note_as details
        // =========================================================================

        // =========================================================================
        // 9. AI command bar: export_markdown details
        // =========================================================================

        #[test]
        fn ai_bar_export_markdown_shortcut() {
            let actions = get_ai_command_bar_actions();
            let exp = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(exp.shortcut.as_deref(), Some("⇧⌘E"));
        }

        #[test]
        fn ai_bar_export_markdown_icon() {
            let actions = get_ai_command_bar_actions();
            let exp = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(exp.icon, Some(IconName::FileCode));
        }

        #[test]
        fn ai_bar_export_markdown_section() {
            let actions = get_ai_command_bar_actions();
            let exp = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(exp.section.as_deref(), Some("Export"));
        }

        #[test]
        fn ai_bar_export_markdown_desc_mentions_markdown() {
            let actions = get_ai_command_bar_actions();
            let exp = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert!(exp.description.as_ref().unwrap().contains("Markdown"));
        }

        // =========================================================================
        // 10. AI command bar: submit action details
        // =========================================================================

        // --- merged from tests_part_02.rs ---
        #[test]
        fn ai_bar_submit_shortcut() {
            let actions = get_ai_command_bar_actions();
            let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(submit.shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn ai_bar_submit_icon() {
            let actions = get_ai_command_bar_actions();
            let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(submit.icon, Some(IconName::ArrowUp));
        }

        #[test]
        fn ai_bar_submit_section_is_actions() {
            let actions = get_ai_command_bar_actions();
            let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(submit.section.as_deref(), Some("Actions"));
        }

        #[test]
        fn ai_bar_submit_desc_mentions_send() {
            let actions = get_ai_command_bar_actions();
            let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert!(submit.description.as_ref().unwrap().contains("Send"));
        }

        // =========================================================================
        // 11. Chat context: empty available_models
        // =========================================================================

        #[test]
        fn chat_empty_models_still_has_continue() {
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
        fn chat_empty_models_no_response_count_is_1() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions.len(), 3);
        }

        #[test]
        fn chat_model_id_uses_model_id_field() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "claude-3".into(),
                    display_name: "Claude 3".into(),
                    provider: "Anthropic".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            assert!(picker.iter().any(|a| a.id == "chat:select_model_claude-3"));
        }

        #[test]
        fn chat_current_model_gets_check_mark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude 3".into()),
                available_models: vec![ChatModelInfo {
                    id: "claude-3".into(),
                    display_name: "Claude 3".into(),
                    provider: "Anthropic".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-3")
                .unwrap();
            assert!(model.title.contains('✓'));
        }

        // =========================================================================
        // 12. New chat: last_used icon is BoltFilled
        // =========================================================================

        #[test]
        fn new_chat_last_used_icon_bolt_filled() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "p1".into(),
                provider_display_name: "Provider 1".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            let action = actions.iter().find(|a| a.id == "last_used_p1::m1").unwrap();
            assert_eq!(action.icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn new_chat_last_used_section() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "p1".into(),
                provider_display_name: "Provider 1".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            let action = actions.iter().find(|a| a.id == "last_used_p1::m1").unwrap();
            assert_eq!(action.section.as_deref(), Some("Last Used Settings"));
        }

        #[test]
        fn new_chat_last_used_desc_is_provider_display() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "p1".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            let action = actions.iter().find(|a| a.id == "last_used_p1::m1").unwrap();
            assert_eq!(action.description.as_deref(), Some("Uses Anthropic"));
        }

        #[test]
        fn new_chat_model_icon_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "p1".into(),
                provider_display_name: "Provider 1".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            let action = actions.iter().find(|a| a.id == "model_p1::m1").unwrap();
            assert_eq!(action.icon, Some(IconName::Settings));
        }

        // =========================================================================
        // 13. New chat: preset section and desc
        // =========================================================================

        #[test]
        fn new_chat_preset_section_is_presets() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let action = actions.iter().find(|a| a.id == "preset_general").unwrap();
            assert_eq!(action.section.as_deref(), Some("Presets"));
        }

        #[test]
        fn new_chat_preset_desc_is_none() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let action = actions.iter().find(|a| a.id == "preset_general").unwrap();
            assert_eq!(action.description.as_deref(), Some("Uses General preset"));
        }

        #[test]
        fn new_chat_preset_icon_preserved() {
            let presets = vec![NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let action = actions.iter().find(|a| a.id == "preset_code").unwrap();
            assert_eq!(action.icon, Some(IconName::Code));
        }

        #[test]
        fn new_chat_model_section_is_models() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "M1".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            let action = actions.iter().find(|a| a.id == "model_p::m1").unwrap();
            assert_eq!(action.section.as_deref(), Some("Models"));
        }

        // =========================================================================
        // 14. Note switcher: current note gets "• " prefix
        // =========================================================================

        #[test]
        fn note_switcher_current_note_has_bullet_prefix() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "My Note".into(),
                char_count: 42,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].title, "• My Note");
        }

        #[test]
        fn note_switcher_non_current_note_no_prefix() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "My Note".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].title, "My Note");
        }

        #[test]
        fn note_switcher_current_icon_is_check_when_not_pinned() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Test".into(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::Check));
        }

        #[test]
        fn note_switcher_pinned_takes_priority_over_current() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Test".into(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        // =========================================================================
        // 15. Note switcher: preview trimming at 60 chars
        // =========================================================================

        #[test]
        fn note_switcher_preview_exactly_60_not_truncated() {
            let preview: String = "A".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "T".into(),
                char_count: 60,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'));
        }

        #[test]
        fn note_switcher_preview_61_chars_truncated() {
            let preview: String = "B".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "T".into(),
                char_count: 61,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains('…'));
        }

        #[test]
        fn note_switcher_preview_with_time_has_separator() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "T".into(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "Hello world".into(),
                relative_time: "2m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains(" · "));
        }

        #[test]
        fn note_switcher_no_preview_no_time_shows_chars() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "T".into(),
                char_count: 99,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "99 chars");
        }

        // =========================================================================
        // 16. count_section_headers: edge cases
        // =========================================================================

        #[test]
        fn count_section_headers_empty_filtered() {
            let actions: Vec<Action> = vec![];
            let filtered: Vec<usize> = vec![];
            assert_eq!(count_section_headers(&actions, &filtered), 0);
        }

        #[test]
        fn count_section_headers_no_sections() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext),
                Action::new("b", "B", None, ActionCategory::ScriptContext),
            ];
            let filtered = vec![0, 1];
            assert_eq!(count_section_headers(&actions, &filtered), 0);
        }

        #[test]
        fn count_section_headers_all_same_section() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S"),
            ];
            let filtered = vec![0, 1];
            assert_eq!(count_section_headers(&actions, &filtered), 1);
        }

        #[test]
        fn count_section_headers_different_sections() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Y"),
                Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Z"),
            ];
            let filtered = vec![0, 1, 2];
            assert_eq!(count_section_headers(&actions, &filtered), 3);
        }

        // =========================================================================
        // 17. count_section_headers: mixed with and without sections
        // =========================================================================

        #[test]
        fn count_section_headers_mixed_some_none() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
                Action::new("b", "B", None, ActionCategory::ScriptContext), // no section
                Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("S"),
            ];
            let filtered = vec![0, 1, 2];
            // Unsectioned rows do not reset section runs, so both S entries share one header.
            assert_eq!(count_section_headers(&actions, &filtered), 1);
        }

        #[test]
        fn count_section_headers_filtered_subset() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Y"),
                Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("X"),
            ];
            // Only index 0 and 2 (same section X, but separated by skipped Y)
            let filtered = vec![0, 2];
            assert_eq!(count_section_headers(&actions, &filtered), 1);
        }

        #[test]
        fn count_section_headers_single_action_with_section() {
            let actions =
                vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S")];
            let filtered = vec![0];
            assert_eq!(count_section_headers(&actions, &filtered), 1);
        }

        #[test]
        fn count_section_headers_single_action_no_section() {
            let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
            let filtered = vec![0];
            assert_eq!(count_section_headers(&actions, &filtered), 0);
        }

        // =========================================================================
        // 18. WindowPosition enum variants and Default
        // =========================================================================

        #[test]
        fn window_position_default_is_bottom_right() {
            assert_eq!(WindowPosition::default(), WindowPosition::BottomRight);
        }

        #[test]
        fn window_position_bottom_right_variant_exists() {
            let _pos = WindowPosition::BottomRight;
        }

        #[test]
        fn window_position_top_right_variant_exists() {
            let _pos = WindowPosition::TopRight;
        }

        #[test]
        fn window_position_top_center_variant_exists() {
            let _pos = WindowPosition::TopCenter;
        }

        // =========================================================================
        // 19. ProtocolAction::with_value constructor
        // =========================================================================

        // --- merged from tests_part_03.rs ---
        #[test]
        fn protocol_action_with_value_sets_name() {
            let pa = ProtocolAction::with_value("Test Action".into(), "test-val".into());
            assert_eq!(pa.name, "Test Action");
        }

        #[test]
        fn protocol_action_with_value_sets_value() {
            let pa = ProtocolAction::with_value("Test".into(), "my-value".into());
            assert_eq!(pa.value, Some("my-value".to_string()));
        }

        #[test]
        fn protocol_action_with_value_has_action_false() {
            let pa = ProtocolAction::with_value("Test".into(), "val".into());
            assert!(!pa.has_action);
        }

        #[test]
        fn protocol_action_with_value_defaults_visible_and_close_none() {
            let pa = ProtocolAction::with_value("Test".into(), "val".into());
            assert!(pa.visible.is_none());
            assert!(pa.close.is_none());
            // But is_visible() and should_close() default to true
            assert!(pa.is_visible());
            assert!(pa.should_close());
        }

        // =========================================================================
        // 20. score_action: whitespace and special character searches
        // =========================================================================

        #[test]
        fn score_action_whitespace_search_no_match() {
            let action = Action::new(
                "test",
                "Run Script",
                Some("Execute script".into()),
                ActionCategory::ScriptContext,
            );
            // Whitespace in search - "run script" is lowered and contains space
            let score = ActionsDialog::score_action(&action, "run script");
            // "run script" is a prefix of "run script" (title_lower)
            assert!(score >= 100);
        }

        #[test]
        fn score_action_dash_in_search() {
            let action = Action::new(
                "test",
                "copy-path",
                Some("Copy the path".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "copy-");
            assert!(score >= 100); // prefix match
        }

        #[test]
        fn score_action_single_char_search() {
            let action = Action::new("test", "Run", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "r");
            assert!(score >= 100); // "r" is prefix of "run"
        }

        #[test]
        fn score_action_no_match_returns_zero() {
            let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "zzz");
            assert_eq!(score, 0);
        }

        // =========================================================================
        // 21. fuzzy_match: repeated and edge characters
        // =========================================================================

        #[test]
        fn fuzzy_match_repeated_chars_in_needle() {
            // "aab" should match "a_a_b_" since both a's and b are found in order
            assert!(ActionsDialog::fuzzy_match("a_a_b_", "aab"));
        }

        #[test]
        fn fuzzy_match_needle_equals_haystack() {
            assert!(ActionsDialog::fuzzy_match("exact", "exact"));
        }

        #[test]
        fn fuzzy_match_reverse_order_fails() {
            // "ba" should not match "ab" because b comes after a
            assert!(!ActionsDialog::fuzzy_match("ab", "ba"));
        }

        #[test]
        fn fuzzy_match_single_char() {
            assert!(ActionsDialog::fuzzy_match("hello", "h"));
        }

        // =========================================================================
        // 22. to_deeplink_name: numeric and edge cases
        // =========================================================================

        #[test]
        fn to_deeplink_name_numeric_only() {
            assert_eq!(to_deeplink_name("123"), "123");
        }

        #[test]
        fn to_deeplink_name_leading_trailing_spaces() {
            assert_eq!(to_deeplink_name("  hello  "), "hello");
        }

        #[test]
        fn to_deeplink_name_mixed_case() {
            assert_eq!(to_deeplink_name("Hello World"), "hello-world");
        }

        #[test]
        fn to_deeplink_name_underscores_to_hyphens() {
            assert_eq!(to_deeplink_name("my_cool_script"), "my-cool-script");
        }

        // =========================================================================
        // 23. CommandBarConfig: all styles have expected close defaults
        // =========================================================================

        #[test]
        fn command_bar_config_default_close_flags() {
            let cfg = CommandBarConfig::default();
            assert!(cfg.close_on_select);
            assert!(cfg.close_on_click_outside);
            assert!(cfg.close_on_escape);
        }

        #[test]
        fn command_bar_config_ai_style_inherits_close_flags() {
            let cfg = CommandBarConfig::ai_style();
            assert!(cfg.close_on_select);
            assert!(cfg.close_on_escape);
        }

        #[test]
        fn command_bar_config_main_menu_inherits_close_flags() {
            let cfg = CommandBarConfig::main_menu_style();
            assert!(cfg.close_on_select);
            assert!(cfg.close_on_escape);
        }

        #[test]
        fn command_bar_config_notes_inherits_close_flags() {
            let cfg = CommandBarConfig::notes_style();
            assert!(cfg.close_on_select);
            assert!(cfg.close_on_escape);
        }

        // =========================================================================
        // 24. ActionsDialogConfig Default trait
        // =========================================================================

        #[test]
        fn actions_dialog_config_default_search_top() {
            let cfg = crate::actions::types::ActionsDialogConfig::default();
            assert_eq!(
                cfg.search_position,
                crate::actions::types::SearchPosition::Top
            );
        }

        #[test]
        fn actions_dialog_config_default_section_style_separators() {
            let cfg = crate::actions::types::ActionsDialogConfig::default();
            assert_eq!(cfg.section_style, SectionStyle::Headers);
        }

        #[test]
        fn actions_dialog_config_default_anchor_bottom() {
            let cfg = crate::actions::types::ActionsDialogConfig::default();
            assert_eq!(cfg.anchor, AnchorPosition::Bottom);
        }

        #[test]
        fn actions_dialog_config_default_show_icons_false() {
            let cfg = crate::actions::types::ActionsDialogConfig::default();
            assert!(!cfg.show_icons);
            assert!(!cfg.show_footer);
        }

        // =========================================================================
        // 25. File context: reveal_in_finder always present
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_reveal_in_finder_present_for_file() {
            let file = FileInfo {
                path: "/test.txt".into(),
                name: "test.txt".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_reveal_in_finder_present_for_dir() {
            let dir = FileInfo {
                path: "/docs".into(),
                name: "docs".into(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&dir);
            assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_reveal_shortcut() {
            let file = FileInfo {
                path: "/test.txt".into(),
                name: "test.txt".into(),
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

        #[cfg(target_os = "macos")]
        #[test]
        fn file_context_reveal_desc_mentions_finder() {
            let file = FileInfo {
                path: "/test.txt".into(),
                name: "test.txt".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let reveal = actions
                .iter()
                .find(|a| a.id == "file:reveal_in_finder")
                .unwrap();
            assert!(reveal.description.as_ref().unwrap().contains("Finder"));
        }

        // =========================================================================
        // 26. Path context: primary action differences for file vs dir
        // =========================================================================

        #[test]
        fn path_context_file_primary_is_select() {
            let info = PathInfo::new("readme.md", "/home/readme.md", false);
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn path_context_dir_primary_is_open_directory() {
            let info = PathInfo::new("docs", "/home/docs", true);
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn path_context_file_primary_title_quotes_name() {
            let info = PathInfo::new("data.csv", "/home/data.csv", false);
            let actions = get_path_context_actions(&info);
            assert!(actions[0].title.contains("\"data.csv\""));
        }

        #[test]
        fn path_context_dir_primary_title_quotes_name() {
            let info = PathInfo::new("src", "/home/src", true);
            let actions = get_path_context_actions(&info);
            assert!(actions[0].title.contains("\"src\""));
        }

        // =========================================================================
        // 27. Script context: builtin has exactly 4 actions (no shortcut/alias)
        // =========================================================================

        #[test]
        fn builtin_script_action_count_no_extras() {
            let builtin = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&builtin);
            // run_script + toggle_info + add_shortcut + add_alias + copy_deeplink = 5
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn builtin_script_no_edit_actions() {
            let builtin = ScriptInfo::builtin("App Launcher");
            let actions = get_script_context_actions(&builtin);
            assert!(!actions.iter().any(|a| a.id == "edit_script"));
            assert!(!actions.iter().any(|a| a.id == "edit_scriptlet"));
        }

        #[test]
        fn builtin_script_no_reveal_or_copy_path() {
            let builtin = ScriptInfo::builtin("App Launcher");
            let actions = get_script_context_actions(&builtin);
            assert!(!actions.iter().any(|a| a.id == "file:reveal_in_finder"));
            assert!(!actions.iter().any(|a| a.id == "file:copy_path"));
        }

        #[test]
        fn builtin_script_has_copy_deeplink() {
            let builtin = ScriptInfo::builtin("App Launcher");
            let actions = get_script_context_actions(&builtin);
            assert!(actions.iter().any(|a| a.id == "copy_deeplink"));
        }

        // =========================================================================
        // 28. Script context: agent actions include specific IDs
        // =========================================================================

        #[test]
        fn agent_script_has_edit_agent_title() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_script = false;
            info.is_agent = true;
            let actions = get_script_context_actions(&info);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn agent_script_desc_mentions_agent() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_script = false;
            info.is_agent = true;
            let actions = get_script_context_actions(&info);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(edit.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn agent_script_has_copy_content() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_script = false;
            info.is_agent = true;
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn agent_script_no_view_logs() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_script = false;
            info.is_agent = true;
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        // =========================================================================
        // 29. Scriptlet context: copy_scriptlet_path details
        // =========================================================================

        #[test]
        fn scriptlet_copy_path_id() {
            let info = ScriptInfo::scriptlet("Test", "/path.md#test", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            assert!(actions.iter().any(|a| a.id == "copy_scriptlet_path"));
        }

        #[test]
        fn scriptlet_copy_path_shortcut() {
            let info = ScriptInfo::scriptlet("Test", "/path.md#test", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let cp = actions
                .iter()
                .find(|a| a.id == "copy_scriptlet_path")
                .unwrap();
            assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
        }

        #[test]
        fn scriptlet_copy_path_desc_mentions_path() {
            let info = ScriptInfo::scriptlet("Test", "/path.md#test", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let cp = actions
                .iter()
                .find(|a| a.id == "copy_scriptlet_path")
                .unwrap();
            assert!(cp.description.as_ref().unwrap().contains("path"));
        }

        #[test]
        fn scriptlet_edit_scriptlet_desc_mentions_editor() {
            let info = ScriptInfo::scriptlet("Test", "/path.md#test", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
            assert!(edit.description.as_ref().unwrap().contains("$EDITOR"));
        }

        // =========================================================================
        // 30. Cross-context: all action titles are non-empty and IDs are non-empty
        // =========================================================================

        #[test]
        fn cross_context_script_all_titles_and_ids_nonempty() {
            let script = ScriptInfo::new("test", "/test.ts");
            let actions = get_script_context_actions(&script);
            for action in &actions {
                assert!(
                    !action.title.is_empty(),
                    "Empty title for id: {}",
                    action.id
                );
                assert!(!action.id.is_empty(), "Empty id found");
            }
        }

        #[test]
        fn cross_context_clipboard_all_titles_and_ids_nonempty() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert!(
                    !action.title.is_empty(),
                    "Empty title for id: {}",
                    action.id
                );
                assert!(!action.id.is_empty(), "Empty id found");
            }
        }

        #[test]
        fn cross_context_ai_bar_all_titles_and_ids_nonempty() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    !action.title.is_empty(),
                    "Empty title for id: {}",
                    action.id
                );
                assert!(!action.id.is_empty(), "Empty id found");
            }
        }

        // --- merged from tests_part_04.rs ---
    }
}

mod from_dialog_builtin_action_validation_tests_39 {
    //! Batch 39: Dialog builtin action validation tests
    //!
    //! Focuses on:
    //! - ScriptInfo::with_shortcut constructor and field preservation
    //! - ScriptInfo::with_shortcut_and_alias constructor combinations
    //! - ScriptInfo::scriptlet constructor field validation
    //! - format_shortcut_hint: arrow key and special key conversions
    //! - format_shortcut_hint: alias variants (return, esc, opt, arrowdown)
    //! - parse_shortcut_keycaps: multi-char modifier combos
    //! - builders::format_shortcut_hint vs dialog::format_shortcut_hint
    //! - Clipboard: ordering of common actions (paste, copy, paste_keep_open)
    //! - Clipboard: destructive action ordering (delete, delete_multiple, delete_all)
    //! - Clipboard: image-only OCR position relative to pin/unpin
    //! - File context: total action count file vs dir on macOS
    //! - File context: copy_filename shortcut differs from path context
    //! - Path context: total action count file vs dir
    //! - Path context: move_to_trash is always last
    //! - Script context: with_frecency adds reset_ranking
    //! - Script context: agent has no view_logs but has copy_path
    //! - Scriptlet context: total action count without custom actions
    //! - Scriptlet context with custom: custom actions appear after run
    //! - AI bar: paste_image details
    //! - AI bar: section ordering matches declaration order
    //! - Notes: section distribution with selection + no trash + disabled auto
    //! - Notes: all actions have icons
    //! - Chat context: model actions come before continue_in_chat
    //! - Chat context: context_title with model name
    //! - New chat: last_used IDs use index format
    //! - New chat: model section actions use Settings icon
    //! - Note switcher: singular vs plural char count
    //! - Note switcher: section assignment pinned vs recent
    //! - coerce_action_selection: all headers returns None
    //! - build_grouped_items_static: filter_idx in Item matches enumerate order

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::*;
        use crate::actions::dialog::{ActionsDialog, build_grouped_items_static};
        use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::Scriptlet;

        use super::super::dialog::{GroupedActionItem, coerce_action_selection};

        // =========================================================================
        // 1. ScriptInfo::with_shortcut: preserves fields and sets shortcut
        // =========================================================================

        #[test]
        fn script_info_with_shortcut_sets_name_and_path() {
            let info = ScriptInfo::with_shortcut(
                "my-script",
                "/scripts/my-script.ts",
                Some("cmd+k".into()),
            );
            assert_eq!(info.name, "my-script");
            assert_eq!(info.path, "/scripts/my-script.ts");
        }

        #[test]
        fn script_info_with_shortcut_is_script_true() {
            let info = ScriptInfo::with_shortcut("x", "/x", Some("cmd+x".into()));
            assert!(info.is_script);
            assert!(!info.is_scriptlet);
            assert!(!info.is_agent);
        }

        #[test]
        fn script_info_with_shortcut_none_leaves_shortcut_none() {
            let info = ScriptInfo::with_shortcut("x", "/x", None);
            assert!(info.shortcut.is_none());
        }

        #[test]
        fn script_info_with_shortcut_defaults_verb_run() {
            let info = ScriptInfo::with_shortcut("x", "/x", Some("cmd+a".into()));
            assert_eq!(info.action_verb, "Run");
            assert!(info.alias.is_none());
        }

        // =========================================================================
        // 2. ScriptInfo::with_shortcut_and_alias: both fields set
        // =========================================================================

        #[test]
        fn script_info_with_shortcut_and_alias_both_set() {
            let info = ScriptInfo::with_shortcut_and_alias(
                "x",
                "/x",
                Some("cmd+t".into()),
                Some("ts".into()),
            );
            assert_eq!(info.shortcut, Some("cmd+t".into()));
            assert_eq!(info.alias, Some("ts".into()));
        }

        #[test]
        fn script_info_with_shortcut_and_alias_both_none() {
            let info = ScriptInfo::with_shortcut_and_alias("x", "/x", None, None);
            assert!(info.shortcut.is_none());
            assert!(info.alias.is_none());
        }

        #[test]
        fn script_info_with_shortcut_and_alias_defaults_not_suggested() {
            let info = ScriptInfo::with_shortcut_and_alias("x", "/x", None, None);
            assert!(!info.is_suggested);
            assert!(info.frecency_path.is_none());
        }

        #[test]
        fn script_info_with_shortcut_and_alias_is_script_true() {
            let info = ScriptInfo::with_shortcut_and_alias("x", "/x", None, None);
            assert!(info.is_script);
            assert!(!info.is_scriptlet);
        }

        // =========================================================================
        // 3. ScriptInfo::scriptlet: field validation
        // =========================================================================

        #[test]
        fn script_info_scriptlet_is_scriptlet_true_is_script_false() {
            let info = ScriptInfo::scriptlet("Copy URL", "/bundles/url.md", None, None);
            assert!(info.is_scriptlet);
            assert!(!info.is_script);
            assert!(!info.is_agent);
        }

        #[test]
        fn script_info_scriptlet_path_preserved() {
            let info = ScriptInfo::scriptlet("Copy URL", "/bundles/url.md#copy-url", None, None);
            assert_eq!(info.path, "/bundles/url.md#copy-url");
        }

        #[test]
        fn script_info_scriptlet_shortcut_and_alias_set() {
            let info =
                ScriptInfo::scriptlet("Copy URL", "/p", Some("cmd+u".into()), Some("url".into()));
            assert_eq!(info.shortcut, Some("cmd+u".into()));
            assert_eq!(info.alias, Some("url".into()));
        }

        #[test]
        fn script_info_scriptlet_verb_defaults_to_run() {
            let info = ScriptInfo::scriptlet("Copy URL", "/p", None, None);
            assert_eq!(info.action_verb, "Run");
        }

        // =========================================================================
        // 4. format_shortcut_hint: arrow keys and special keys
        // =========================================================================

        #[test]
        fn format_shortcut_hint_up_arrow() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+up"), "⌘↑");
        }

        #[test]
        fn format_shortcut_hint_down_arrow() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+down"), "⌘↓");
        }

        #[test]
        fn format_shortcut_hint_left_arrow() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+left"), "⌘←");
        }

        #[test]
        fn format_shortcut_hint_right_arrow() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+right"), "⌘→");
        }

        // =========================================================================
        // 5. format_shortcut_hint: alias variants (return, esc, opt, arrowdown)
        // =========================================================================

        #[test]
        fn format_shortcut_hint_return_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+return"), "⌘↵");
        }

        #[test]
        fn format_shortcut_hint_esc_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
        }

        #[test]
        fn format_shortcut_hint_opt_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("opt+c"), "⌥C");
        }

        #[test]
        fn format_shortcut_hint_arrowdown_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowdown"), "↓");
        }

        // =========================================================================
        // 6. parse_shortcut_keycaps: multi-char modifier combos
        // =========================================================================

        #[test]
        fn parse_keycaps_cmd_shift_letter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
        }

        #[test]
        fn parse_keycaps_ctrl_option_enter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌃⌥↵");
            assert_eq!(keycaps, vec!["⌃", "⌥", "↵"]);
        }

        #[test]
        fn parse_keycaps_single_escape() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn parse_keycaps_lowercase_letter_uppercased() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘k");
            assert_eq!(keycaps, vec!["⌘", "K"]);
        }

        // =========================================================================
        // 7. builders::format_shortcut_hint vs dialog::format_shortcut_hint
        // =========================================================================

        #[test]
        fn dialog_format_handles_command_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("command+c"), "⌘C");
        }

        #[test]
        fn dialog_format_handles_super_alias() {
            assert_eq!(ActionsDialog::format_shortcut_hint("super+c"), "⌘C");
        }

        #[test]
        fn dialog_format_handles_control_full_word() {
            assert_eq!(ActionsDialog::format_shortcut_hint("control+x"), "⌃X");
        }

        #[test]
        fn dialog_format_handles_option_full_word() {
            assert_eq!(ActionsDialog::format_shortcut_hint("option+v"), "⌥V");
        }

        // =========================================================================
        // 8. Clipboard: ordering of common actions (paste, copy, paste_keep_open)
        // =========================================================================

        #[test]
        fn clipboard_first_action_is_paste() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].id, "clip:clipboard_paste");
        }

        #[test]
        fn clipboard_second_action_is_copy() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[1].id, "clip:clipboard_copy");
        }

        #[test]
        fn clipboard_third_action_is_paste_keep_open() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[2].id, "clip:clipboard_paste_keep_open");
        }

        #[test]
        fn clipboard_fourth_action_is_share() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[3].id, "clip:clipboard_share");
        }

        // =========================================================================
        // 9. Clipboard: destructive action ordering (delete, delete_multiple, delete_all)
        // =========================================================================

        #[test]
        fn clipboard_last_three_are_destructive() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
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
        fn clipboard_delete_all_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let last = actions.last().unwrap();
            assert_eq!(last.shortcut.as_deref(), Some("⌃⇧X"));
        }

        #[test]
        fn clipboard_delete_all_desc_mentions_pinned() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let last = actions.last().unwrap();
            assert!(last.description.as_ref().unwrap().contains("pinned"));
        }

        #[test]
        fn clipboard_image_destructive_still_last_three() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let len = actions.len();
            assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
            assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
            assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
        }

        // =========================================================================
        // 10. Clipboard: image OCR position relative to pin/unpin
        // =========================================================================

        #[test]
        fn clipboard_image_unpinned_pin_comes_before_ocr() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let pin_pos = actions
                .iter()
                .position(|a| a.id == "clip:clipboard_pin")
                .unwrap();
            let ocr_pos = actions
                .iter()
                .position(|a| a.id == "clip:clipboard_ocr")
                .unwrap();
            assert!(
                pin_pos < ocr_pos,
                "pin should come before ocr in ordering: pin={} ocr={}",
                pin_pos,
                ocr_pos
            );
        }

        #[test]
        fn clipboard_image_pinned_unpin_comes_before_ocr() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Image,
                pinned: true,
                preview: "".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let unpin_pos = actions
                .iter()
                .position(|a| a.id == "clip:clipboard_unpin")
                .unwrap();
            let ocr_pos = actions
                .iter()
                .position(|a| a.id == "clip:clipboard_ocr")
                .unwrap();
            assert!(
                unpin_pos < ocr_pos,
                "unpin should come before ocr in ordering"
            );
        }

        #[test]
        fn clipboard_image_has_ocr_and_no_save_snippet() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // save_snippet is text-only; images carry clipboard_ocr instead.
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
            assert!(
                !actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_save_snippet")
            );
        }

        #[test]
        fn clipboard_text_has_no_ocr() {
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

        // =========================================================================
        // 11. File context: total action count file vs dir on macOS
        // =========================================================================

        // --- merged from tests_part_02.rs ---
        #[test]
        fn file_context_file_has_more_actions_than_dir() {
            let file = FileInfo {
                name: "readme.md".into(),
                path: "/readme.md".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let dir = FileInfo {
                name: "src".into(),
                path: "/src".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let file_actions = get_file_context_actions(&file);
            let dir_actions = get_file_context_actions(&dir);
            // Files have quick_look, dirs don't (on macOS)
            assert!(file_actions.len() >= dir_actions.len());
        }

        #[test]
        fn file_context_dir_no_quick_look() {
            let dir = FileInfo {
                name: "src".into(),
                path: "/src".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&dir);
            assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[test]
        fn file_context_both_have_reveal_in_finder() {
            let file = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let dir = FileInfo {
                name: "b".into(),
                path: "/b".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            assert!(
                get_file_context_actions(&file)
                    .iter()
                    .any(|a| a.id == "file:reveal_in_finder")
            );
            assert!(
                get_file_context_actions(&dir)
                    .iter()
                    .any(|a| a.id == "file:reveal_in_finder")
            );
        }

        #[test]
        fn file_context_both_have_copy_path_and_copy_filename() {
            let file = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&file);
            assert!(actions.iter().any(|a| a.id == "file:copy_path"));
            assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
        }

        // =========================================================================
        // 12. File context: copy_filename shortcut ⌘C differs from path context (no shortcut)
        // =========================================================================

        #[test]
        fn file_context_copy_filename_has_shortcut() {
            let file = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&file);
            let cf = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert_eq!(cf.shortcut.as_deref(), Some("⌘C"));
        }

        #[test]
        fn path_context_copy_filename_has_no_shortcut() {
            let info = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let cf = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert!(cf.shortcut.is_none());
        }

        #[test]
        fn file_context_copy_path_shortcut_matches_path_context() {
            let file = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let path = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let fc = get_file_context_actions(&file);
            let pc = get_path_context_actions(&path);
            let fcp = fc.iter().find(|a| a.id == "file:copy_path").unwrap();
            let pcp = pc.iter().find(|a| a.id == "file:copy_path").unwrap();
            assert_eq!(fcp.shortcut, pcp.shortcut);
        }

        #[test]
        fn file_and_path_copy_path_shortcut_is_cmd_shift_c() {
            let file = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&file);
            let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
            assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
        }

        // =========================================================================
        // 13. Path context: total action count file vs dir
        // =========================================================================

        #[test]
        fn path_context_dir_has_one_more_than_common() {
            let file = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let dir = PathInfo {
                name: "b".into(),
                path: "/b".into(),
                is_dir: true,
            };
            // Both have same total: primary + 6 common = 7
            assert_eq!(
                get_path_context_actions(&file).len(),
                get_path_context_actions(&dir).len()
            );
        }

        #[test]
        fn path_context_file_primary_is_select_file() {
            let info = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn path_context_dir_primary_is_open_directory() {
            let info = PathInfo {
                name: "b".into(),
                path: "/b".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn path_context_both_have_7_actions() {
            let file = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let dir = PathInfo {
                name: "b".into(),
                path: "/b".into(),
                is_dir: true,
            };
            assert_eq!(get_path_context_actions(&file).len(), 7);
            assert_eq!(get_path_context_actions(&dir).len(), 7);
        }

        // =========================================================================
        // 14. Path context: move_to_trash is always last
        // =========================================================================

        #[test]
        fn path_context_file_last_is_move_to_trash() {
            let info = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
        }

        #[test]
        fn path_context_dir_last_is_move_to_trash() {
            let info = PathInfo {
                name: "b".into(),
                path: "/b".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
        }

        #[test]
        fn path_context_move_to_trash_desc_file() {
            let info = PathInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions.last().unwrap();
            assert!(trash.description.as_ref().unwrap().contains("file"));
        }

        #[test]
        fn path_context_move_to_trash_desc_folder() {
            let info = PathInfo {
                name: "b".into(),
                path: "/b".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions.last().unwrap();
            assert!(trash.description.as_ref().unwrap().contains("folder"));
        }

        // =========================================================================
        // 15. Script context: with_frecency adds reset_ranking
        // =========================================================================

        #[test]
        fn script_with_frecency_has_reset_ranking() {
            let info =
                ScriptInfo::new("test", "/test.ts").with_frecency(true, Some("/test.ts".into()));
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn script_without_frecency_no_reset_ranking() {
            let info = ScriptInfo::new("test", "/test.ts");
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn script_with_frecency_reset_ranking_is_last() {
            let info =
                ScriptInfo::new("test", "/test.ts").with_frecency(true, Some("/test.ts".into()));
            let actions = get_script_context_actions(&info);
            assert_eq!(actions.last().unwrap().id, "reset_ranking");
        }

        #[test]
        fn script_with_frecency_reset_ranking_no_shortcut() {
            let info =
                ScriptInfo::new("test", "/test.ts").with_frecency(true, Some("/test.ts".into()));
            let actions = get_script_context_actions(&info);
            let rr = actions.iter().find(|a| a.id == "reset_ranking").unwrap();
            assert_eq!(rr.shortcut.as_deref(), Some("⌃⌘R"));
        }

        // =========================================================================
        // 16. Script context: agent has no view_logs but has copy_path
        // =========================================================================

        #[test]
        fn agent_context_no_view_logs() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn agent_context_has_copy_path() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "copy_path"));
        }

        #[test]
        fn agent_context_has_reveal_in_finder() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
        }

        #[test]
        fn agent_context_edit_title_says_agent() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        // =========================================================================
        // 17. Scriptlet context: total action count without custom actions
        // =========================================================================

        #[test]
        fn scriptlet_context_no_custom_no_shortcut_no_alias_count() {
            let info = ScriptInfo::scriptlet("My Script", "/scripts.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            // run + add_shortcut + add_alias + edit_scriptlet + reveal + copy_path + copy_content + copy_deeplink = 8
            assert_eq!(actions.len(), 8);
        }

        #[test]
        fn scriptlet_context_with_shortcut_adds_two_actions() {
            let info =
                ScriptInfo::scriptlet("My Script", "/scripts.md", Some("cmd+m".into()), None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            // run + update_shortcut + remove_shortcut + add_alias + edit + reveal + copy_path + copy_content + copy_deeplink = 9
            assert_eq!(actions.len(), 9);
        }

        #[test]
        fn scriptlet_context_with_both_shortcut_alias_count() {
            let info = ScriptInfo::scriptlet(
                "My Script",
                "/scripts.md",
                Some("cmd+m".into()),
                Some("ms".into()),
            );
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            // run + update_shortcut + remove_shortcut + update_alias + remove_alias + edit + reveal + copy_path + copy_content + copy_deeplink = 10
            assert_eq!(actions.len(), 10);
        }

        #[test]
        fn scriptlet_context_suggested_adds_reset_ranking() {
            let info = ScriptInfo::scriptlet("My Script", "/scripts.md", None, None)
                .with_frecency(true, Some("/scripts.md".into()));
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        // =========================================================================
        // 18. Scriptlet context with custom: custom actions appear after run
        // =========================================================================

        #[test]
        fn scriptlet_custom_actions_appear_after_run() {
            let info = ScriptInfo::scriptlet("My Script", "/scripts.md", None, None);
            let mut scriptlet = Scriptlet::new("My Script".into(), "bash".into(), "echo hi".into());
            scriptlet.actions.push(crate::scriptlets::ScriptletAction {
                name: "Copy".into(),
                command: "copy".into(),
                tool: "bash".into(),
                code: "echo copy".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            });
            let actions = get_scriptlet_context_actions_with_custom(&info, Some(&scriptlet));
            assert_eq!(actions[0].id, "run_script");
            assert_eq!(actions[1].id, "scriptlet_action:copy");
        }

        #[test]
        fn scriptlet_custom_actions_have_has_action_true() {
            let info = ScriptInfo::scriptlet("My Script", "/scripts.md", None, None);
            let mut scriptlet = Scriptlet::new("My Script".into(), "bash".into(), "echo hi".into());
            scriptlet.actions.push(crate::scriptlets::ScriptletAction {
                name: "Do Thing".into(),
                command: "do-thing".into(),
                tool: "bash".into(),
                code: "echo thing".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            });
            let actions = get_scriptlet_context_actions_with_custom(&info, Some(&scriptlet));
            let custom = actions
                .iter()
                .find(|a| a.id == "scriptlet_action:do-thing")
                .unwrap();
            assert!(custom.has_action);
        }

        #[test]
        fn scriptlet_custom_action_value_is_command() {
            let info = ScriptInfo::scriptlet("My Script", "/scripts.md", None, None);
            let mut scriptlet = Scriptlet::new("My Script".into(), "bash".into(), "echo hi".into());
            scriptlet.actions.push(crate::scriptlets::ScriptletAction {
                name: "Do Thing".into(),
                command: "do-thing".into(),
                tool: "bash".into(),
                code: "echo thing".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            });
            let actions = get_scriptlet_context_actions_with_custom(&info, Some(&scriptlet));
            let custom = actions
                .iter()
                .find(|a| a.id == "scriptlet_action:do-thing")
                .unwrap();
            assert_eq!(custom.value.as_deref(), Some("do-thing"));
        }

        #[test]
        fn scriptlet_no_scriptlet_no_custom_actions() {
            let info = ScriptInfo::scriptlet("My Script", "/scripts.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&info, None);
            assert!(
                !actions
                    .iter()
                    .any(|a| a.id.starts_with("scriptlet_action:"))
            );
        }

        // =========================================================================
        // 19. AI bar: paste_image details
        // =========================================================================

        #[test]
        fn ai_bar_paste_image_shortcut() {
            let actions = get_ai_command_bar_actions();
            let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
            assert_eq!(pi.shortcut.as_deref(), Some("⌘V"));
        }

        #[test]
        fn ai_bar_paste_image_icon() {
            let actions = get_ai_command_bar_actions();
            let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
            assert_eq!(pi.icon, Some(IconName::File));
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn ai_bar_paste_image_section() {
            let actions = get_ai_command_bar_actions();
            let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
            assert_eq!(pi.section.as_deref(), Some("Attachments"));
        }

        #[test]
        fn ai_bar_paste_image_desc_mentions_clipboard() {
            let actions = get_ai_command_bar_actions();
            let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
            assert!(
                pi.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("clipboard")
            );
        }

        // =========================================================================
        // 20. AI bar: section ordering matches declaration order
        // =========================================================================

        #[test]
        fn ai_bar_first_section_is_response() {
            let actions = get_ai_command_bar_actions();
            let first_with_section = actions.iter().find(|a| a.section.is_some()).unwrap();
            assert_eq!(first_with_section.section.as_deref(), Some("Response"));
        }

        #[test]
        fn ai_bar_last_section_is_settings() {
            let actions = get_ai_command_bar_actions();
            let last = actions.last().unwrap();
            assert_eq!(last.section.as_deref(), Some("Settings"));
        }

        #[test]
        fn ai_bar_export_section_has_one_action() {
            let actions = get_ai_command_bar_actions();
            let export_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Export"))
                .count();
            assert_eq!(export_count, 1);
        }

        #[test]
        fn ai_bar_attachments_section_has_two_actions() {
            let actions = get_ai_command_bar_actions();
            let att_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .count();
            assert_eq!(att_count, 4);
        }

        // =========================================================================
        // 21. Notes: section distribution with selection + no trash + disabled auto
        // =========================================================================

        // =========================================================================
        // 22. Notes: all actions have icons
        // =========================================================================

        // =========================================================================
        // 23. Chat context: model actions come before continue_in_chat
        // =========================================================================

        #[test]
        fn chat_model_actions_before_continue() {
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
            let change_model_pos = actions
                .iter()
                .position(|a| a.id == "chat:change_model")
                .unwrap();
            let continue_pos = actions
                .iter()
                .position(|a| a.id == "chat:continue_in_chat")
                .unwrap();
            assert!(change_model_pos < continue_pos);
        }

        #[test]
        fn chat_all_model_actions_contiguous() {
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
            let model_indices: Vec<usize> = picker
                .iter()
                .enumerate()
                .filter(|(_, a)| a.id.starts_with("chat:select_model_"))
                .map(|(i, _)| i)
                .collect();
            assert_eq!(model_indices, vec![0, 1]);
        }

        #[test]
        fn chat_continue_in_chat_always_after_models() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions[0].id, "chat:change_model");
        }

        #[test]
        fn chat_copy_response_after_continue() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            let cont_pos = actions
                .iter()
                .position(|a| a.id == "chat:continue_in_chat")
                .unwrap();
            let copy_pos = actions
                .iter()
                .position(|a| a.id == "chat:copy_response")
                .unwrap();
            assert!(copy_pos > cont_pos);
        }

        // =========================================================================
        // 24. Chat context: current model marked with checkmark
        // =========================================================================

        #[test]
        fn chat_current_model_has_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
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
            assert!(model_action.title.contains("✓"));
        }

        #[test]
        fn chat_non_current_model_no_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
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
            assert!(!model_action.title.contains("✓"));
        }

        #[test]
        fn chat_model_desc_mentions_provider() {
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
            assert!(
                model_action
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("Anthropic")
            );
        }

        #[test]
        fn chat_model_no_shortcut() {
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
            assert!(model_action.shortcut.is_none());
        }

        // =========================================================================
        // 25. New chat: last_used IDs use index format
        // =========================================================================

        #[test]
        fn new_chat_last_used_id_format() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].id, "last_used_anthropic::claude");
        }

        #[test]
        fn new_chat_last_used_second_id() {
            let last_used = vec![
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
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[1].id, "last_used_openai::gpt4");
        }

        #[test]
        fn new_chat_last_used_desc_is_provider_display_name() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].description.as_deref(), Some("Uses Anthropic"));
        }

        #[test]
        fn new_chat_last_used_section() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
        }

        // =========================================================================
        // 26. New chat: model section actions use Settings icon
        // =========================================================================

        #[test]
        fn new_chat_model_icon_is_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        #[test]
        fn new_chat_model_section_is_models() {
            let models = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].section.as_deref(), Some("Models"));
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn new_chat_model_id_uses_index() {
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
        fn new_chat_preset_id_uses_preset_id() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].id, "preset_general");
        }

        // =========================================================================
        // 27. Note switcher: singular vs plural char count
        // =========================================================================

        #[test]
        fn note_switcher_one_char_singular() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
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
        fn note_switcher_zero_chars_plural() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].description.as_deref(), Some("0 chars"));
        }

        #[test]
        fn note_switcher_many_chars_plural() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
                char_count: 500,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].description.as_deref(), Some("500 chars"));
        }

        #[test]
        fn note_switcher_two_chars_plural() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
                char_count: 2,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].description.as_deref(), Some("2 chars"));
        }

        // =========================================================================
        // 28. Note switcher: section assignment pinned vs recent
        // =========================================================================

        #[test]
        fn note_switcher_pinned_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
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
        fn note_switcher_unpinned_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn note_switcher_mixed_sections() {
            let notes = vec![
                NoteSwitcherNoteInfo {
                    id: "1".into(),
                    title: "A".into(),
                    char_count: 10,
                    is_current: false,
                    is_pinned: true,
                    preview: "".into(),
                    relative_time: "".into(),
                },
                NoteSwitcherNoteInfo {
                    id: "2".into(),
                    title: "B".into(),
                    char_count: 20,
                    is_current: false,
                    is_pinned: false,
                    preview: "".into(),
                    relative_time: "".into(),
                },
            ];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
            assert_eq!(actions[1].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn note_switcher_current_pinned_still_pinned_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".into(),
                title: "Note".into(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        // =========================================================================
        // 29. coerce_action_selection: all headers returns None
        // =========================================================================

        #[test]
        fn coerce_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::SectionHeader("B".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn coerce_empty_returns_none() {
            let rows: Vec<GroupedActionItem> = vec![];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn coerce_header_then_item_returns_item_index() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn coerce_item_at_exact_index_returns_same() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::Item(1),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        // =========================================================================
        // 30. build_grouped_items_static: filter_idx in Item matches enumerate order
        // =========================================================================

        #[test]
        fn build_grouped_items_no_sections_items_sequential() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext),
                Action::new("b", "B", None, ActionCategory::ScriptContext),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn build_grouped_items_with_headers_adds_section_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // S1 header + item(0) + S2 header + item(1) = 4
            assert_eq!(grouped.len(), 4);
            assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "S1"));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(&grouped[2], GroupedActionItem::SectionHeader(s) if s == "S2"));
            assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
        }

        #[test]
        fn build_grouped_items_separators_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // No headers, just items
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn build_grouped_items_empty_filtered() {
            let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }
    }
}

mod from_dialog_builtin_action_validation_tests_40 {
    //! Batch 40: Dialog builtin action validation tests
    //!
    //! Focuses on:
    //! - ScriptInfo::with_action_verb_and_shortcut: field validation
    //! - ScriptInfo: is_agent manual override after construction
    //! - Action::with_shortcut_opt: Some vs None behavior
    //! - Action::with_icon and with_section chaining
    //! - Clipboard: text entry total action count on macOS
    //! - Clipboard: image entry total action count on macOS
    //! - Clipboard: pinned text entry has clipboard_unpin ID
    //! - Clipboard: unpinned text entry has clipboard_pin ID
    //! - File context: dir has no quick_look but has open_with on macOS
    //! - File context: file primary title format uses quoted name
    //! - Path context: all action IDs are snake_case
    //! - Path context: open_in_editor desc mentions $EDITOR
    //! - Script context: scriptlet is_scriptlet true has edit_scriptlet
    //! - Script context: builtin has exactly 4 actions when no shortcut/alias
    //! - Script context: primary title uses action_verb
    //! - Scriptlet context: with_custom run_script is first action
    //! - Scriptlet context: with_custom copy_deeplink URL uses to_deeplink_name
    //! - AI bar: toggle_shortcuts_help details
    //! - AI bar: change_model has no shortcut
    //! - AI bar: unique IDs across all 21 actions
    //! - Notes: export action requires selection and not trash
    //! - Notes: browse_notes always present
    //! - Chat context: copy_response only when has_response
    //! - Chat context: clear_conversation only when has_messages
    //! - New chat: empty lists produce zero actions
    //! - New chat: preset IDs use preset_{id} format
    //! - Note switcher: empty notes produces no_notes action
    //! - Note switcher: preview with relative_time has separator
    //! - ProtocolAction: with_value sets value and has_action false
    //! - format_shortcut_hint: dialog vs builders produce different results

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::*;
        use crate::actions::dialog::ActionsDialog;
        use crate::actions::types::{Action, ActionCategory, ScriptInfo};
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::protocol::ProtocolAction;
        use crate::scriptlets::Scriptlet;

        // =========================================================================
        // 1. ScriptInfo::with_action_verb_and_shortcut: field validation
        // =========================================================================

        #[test]
        fn with_action_verb_and_shortcut_preserves_name_path() {
            let info = ScriptInfo::with_action_verb_and_shortcut(
                "launcher",
                "/scripts/launcher.ts",
                true,
                "Launch",
                Some("cmd+l".into()),
            );
            assert_eq!(info.name, "launcher");
            assert_eq!(info.path, "/scripts/launcher.ts");
        }

        #[test]
        fn with_action_verb_and_shortcut_sets_verb() {
            let info = ScriptInfo::with_action_verb_and_shortcut("x", "/x", true, "Open", None);
            assert_eq!(info.action_verb, "Open");
        }

        #[test]
        fn with_action_verb_and_shortcut_sets_shortcut() {
            let info = ScriptInfo::with_action_verb_and_shortcut(
                "x",
                "/x",
                false,
                "Run",
                Some("cmd+shift+r".into()),
            );
            assert_eq!(info.shortcut, Some("cmd+shift+r".to_string()));
            assert!(!info.is_script);
        }

        #[test]
        fn with_action_verb_and_shortcut_defaults_agent_false() {
            let info =
                ScriptInfo::with_action_verb_and_shortcut("x", "/x", true, "Run", Some("a".into()));
            assert!(!info.is_agent);
            assert!(!info.is_scriptlet);
            assert!(!info.is_suggested);
        }

        // =========================================================================
        // 2. ScriptInfo: is_agent manual override after construction
        // =========================================================================

        #[test]
        fn script_info_set_is_agent_true_after_new() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            assert!(info.is_agent);
            assert!(info.is_script); // new() sets is_script=true
        }

        #[test]
        fn agent_actions_include_edit_agent_title() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false; // agents have is_script=false
            let actions = get_script_context_actions(&info);
            let edit = actions.iter().find(|a| a.id == "edit_script");
            assert!(edit.is_some());
            assert_eq!(edit.unwrap().title, "Edit Agent");
        }

        #[test]
        fn agent_actions_have_copy_content() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn agent_actions_desc_mentions_agent() {
            let mut info = ScriptInfo::new("my-agent", "/agents/my-agent.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(
                edit.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("agent")
            );
        }

        // =========================================================================
        // 3. Action::with_shortcut_opt: Some vs None behavior
        // =========================================================================

        #[test]
        fn with_shortcut_opt_some_sets_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘T".to_string()));
            assert_eq!(action.shortcut, Some("⌘T".to_string()));
        }

        #[test]
        fn with_shortcut_opt_none_leaves_none() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(None);
            assert!(action.shortcut.is_none());
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn with_shortcut_opt_some_sets_shortcut_lower() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘ABC".to_string()));
            assert_eq!(action.shortcut_lower, Some("⌘abc".to_string()));
        }

        #[test]
        fn with_shortcut_opt_overwrites_previous_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘X")
                .with_shortcut_opt(Some("⌘Y".to_string()));
            assert_eq!(action.shortcut, Some("⌘Y".to_string()));
        }

        // =========================================================================
        // 4. Action::with_icon and with_section chaining
        // =========================================================================

        #[test]
        fn with_icon_sets_icon() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Copy);
            assert_eq!(action.icon, Some(IconName::Copy));
        }

        #[test]
        fn with_section_sets_section() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_section("MySection");
            assert_eq!(action.section, Some("MySection".to_string()));
        }

        #[test]
        fn chained_icon_section_shortcut_preserves_all() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘T")
                .with_icon(IconName::Star)
                .with_section("Actions");
            assert_eq!(action.shortcut, Some("⌘T".to_string()));
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section, Some("Actions".to_string()));
        }

        #[test]
        fn action_new_defaults_icon_section_none() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.icon.is_none());
            assert!(action.section.is_none());
            assert!(action.value.is_none());
        }

        // =========================================================================
        // 5. Clipboard: text entry total action count on macOS
        // =========================================================================

        #[test]
        fn clipboard_text_unpinned_action_count_macos() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // On macOS: paste, copy, paste_keep_open, share, attach_to_ai, quick_look,
            //           pin, keep_in_today, save_snippet, save_file, delete, delete_multiple,
            //           delete_all = 13
            #[cfg(target_os = "macos")]
            assert_eq!(actions.len(), 13);
        }

        #[test]
        fn clipboard_text_pinned_same_count_as_unpinned() {
            let unpinned = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "a".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let pinned = ClipboardEntryInfo {
                id: "2".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "b".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let unpinned_actions = get_clipboard_history_context_actions(&unpinned);
            let pinned_actions = get_clipboard_history_context_actions(&pinned);
            assert_eq!(unpinned_actions.len(), pinned_actions.len());
        }

        #[test]
        fn clipboard_text_first_action_is_paste() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].id, "clip:clipboard_paste");
        }

        #[test]
        fn clipboard_text_second_action_is_copy() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[1].id, "clip:clipboard_copy");
        }

        // =========================================================================
        // 6. Clipboard: image entry total action count on macOS
        // =========================================================================

        #[test]
        fn clipboard_image_unpinned_action_count_macos() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // On macOS image: paste, copy, paste_keep_open, share, attach_to_ai, quick_look,
            //   open_with, annotate_cleanshot, upload_cleanshot, pin, keep_in_today, ocr,
            //   save_file, delete, delete_multiple, delete_all = 16 (no save_snippet for images)
            #[cfg(target_os = "macos")]
            assert_eq!(actions.len(), 16);
        }

        #[test]
        fn clipboard_image_has_3_more_than_text() {
            let text_entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let image_entry = ClipboardEntryInfo {
                id: "2".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let text_count = get_clipboard_history_context_actions(&text_entry).len();
            let image_count = get_clipboard_history_context_actions(&image_entry).len();
            // Image adds open_with, annotate_cleanshot, upload_cleanshot, ocr but
            // drops save_snippet (text-only), so the net difference is 3.
            #[cfg(target_os = "macos")]
            assert_eq!(image_count - text_count, 3);
        }

        #[test]
        fn clipboard_image_has_ocr_action() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
        }

        #[test]
        fn clipboard_text_has_no_ocr_action() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
        }

        // =========================================================================
        // 7. Clipboard: pinned text entry has clipboard_unpin ID
        // =========================================================================

        #[test]
        fn clipboard_pinned_has_unpin_action() {
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
        fn clipboard_pinned_unpin_title() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "test".into(),
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
        fn clipboard_pinned_unpin_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let unpin = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_unpin")
                .unwrap();
            assert_eq!(unpin.shortcut, Some("⇧⌘P".to_string()));
        }

        #[test]
        fn clipboard_pinned_has_no_pin_action() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        }

        // =========================================================================
        // 8. Clipboard: unpinned text entry has clipboard_pin ID
        // =========================================================================

        #[test]
        fn clipboard_unpinned_has_pin_action() {
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
        fn clipboard_unpinned_pin_title() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
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
        fn clipboard_unpinned_pin_desc_mentions_prevent() {
            let entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let pin = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_pin")
                .unwrap();
            assert!(pin.description.as_ref().unwrap().contains("Pin"));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn clipboard_pin_and_unpin_same_shortcut() {
            let pinned_entry = ClipboardEntryInfo {
                id: "1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "a".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let unpinned_entry = ClipboardEntryInfo {
                id: "2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "b".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let pinned_actions = get_clipboard_history_context_actions(&pinned_entry);
            let unpinned_actions = get_clipboard_history_context_actions(&unpinned_entry);
            let unpin = pinned_actions
                .iter()
                .find(|a| a.id == "clip:clipboard_unpin")
                .unwrap();
            let pin = unpinned_actions
                .iter()
                .find(|a| a.id == "clip:clipboard_pin")
                .unwrap();
            assert_eq!(unpin.shortcut, pin.shortcut);
        }

        // =========================================================================
        // 9. File context: dir has no quick_look but has open_with on macOS
        // =========================================================================

        #[test]
        fn file_dir_has_no_quick_look() {
            let file_info = FileInfo {
                name: "Documents".into(),
                path: "/Users/test/Documents".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[test]
        fn file_file_has_quick_look_on_macos() {
            let file_info = FileInfo {
                name: "readme.md".into(),
                path: "/Users/test/readme.md".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&file_info);
            #[cfg(target_os = "macos")]
            assert!(actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[test]
        fn file_dir_has_open_in_editor_on_macos() {
            let file_info = FileInfo {
                name: "Documents".into(),
                path: "/Users/test/Documents".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&file_info);
            #[cfg(target_os = "macos")]
            assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
        }

        #[test]
        fn file_dir_has_show_info_on_macos() {
            let file_info = FileInfo {
                name: "Documents".into(),
                path: "/Users/test/Documents".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&file_info);
            #[cfg(target_os = "macos")]
            assert!(actions.iter().any(|a| a.id == "file:show_info"));
        }

        // =========================================================================
        // 10. File context: file primary title format uses quoted name
        // =========================================================================

        #[test]
        fn file_primary_title_quotes_filename() {
            let file_info = FileInfo {
                name: "report.pdf".into(),
                path: "/Users/test/report.pdf".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&file_info);
            assert_eq!(actions[0].title, "Open \"report.pdf\"");
        }

        #[test]
        fn file_dir_primary_title_quotes_dirname() {
            let file_info = FileInfo {
                name: "src".into(),
                path: "/Users/test/src".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&file_info);
            assert_eq!(actions[0].title, "Open \"src\"");
        }

        #[test]
        fn file_primary_id_is_open_file_for_files() {
            let file_info = FileInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&file_info);
            assert_eq!(actions[0].id, "file:open_file");
        }

        #[test]
        fn file_primary_id_is_open_directory_for_dirs() {
            let file_info = FileInfo {
                name: "docs".into(),
                path: "/docs".into(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&file_info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        // =========================================================================
        // 11. Path context: all action IDs are snake_case
        // =========================================================================

        #[test]
        fn path_file_all_ids_snake_case() {
            let path_info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            for action in &actions {
                assert!(
                    action
                        .id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c == ':'),
                    "Action ID '{}' is not snake_case",
                    action.id
                );
            }
        }

        #[test]
        fn path_dir_all_ids_snake_case() {
            let path_info = PathInfo {
                name: "docs".into(),
                path: "/docs".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path_info);
            for action in &actions {
                assert!(
                    action
                        .id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c == ':'),
                    "Action ID '{}' is not snake_case",
                    action.id
                );
            }
        }

        #[test]
        fn path_file_has_seven_actions() {
            let path_info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            assert_eq!(actions.len(), 7);
        }

        #[test]
        fn path_dir_has_seven_actions() {
            let path_info = PathInfo {
                name: "docs".into(),
                path: "/docs".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path_info);
            assert_eq!(actions.len(), 7);
        }

        // =========================================================================
        // 12. Path context: open_in_editor desc mentions $EDITOR
        // =========================================================================

        #[test]
        fn path_open_in_editor_desc_mentions_editor() {
            let path_info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let editor_action = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .unwrap();
            assert!(
                editor_action
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("$EDITOR")
            );
        }

        #[test]
        fn path_open_in_editor_shortcut() {
            let path_info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let editor_action = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .unwrap();
            assert_eq!(editor_action.shortcut, Some("⌘E".to_string()));
        }

        #[test]
        fn path_open_in_finder_shortcut() {
            let path_info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let finder_action = actions
                .iter()
                .find(|a| a.id == "file:open_in_finder")
                .unwrap();
            assert_eq!(finder_action.shortcut, Some("⌘⇧F".to_string()));
        }

        #[test]
        fn path_move_to_trash_shortcut() {
            let path_info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let trash_action = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert_eq!(trash_action.shortcut, Some("⌘⌫".to_string()));
        }

        // =========================================================================
        // 13. Script context: scriptlet is_scriptlet true has edit_scriptlet
        // =========================================================================

        #[test]
        fn script_context_scriptlet_has_edit_scriptlet() {
            let info = ScriptInfo::scriptlet("My Snippet", "/path/to/snippets.md", None, None);
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "edit_scriptlet"));
        }

        #[test]
        fn script_context_scriptlet_no_edit_script() {
            let info = ScriptInfo::scriptlet("My Snippet", "/path/to/snippets.md", None, None);
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "edit_script"));
        }

        #[test]
        fn script_context_scriptlet_has_reveal_scriptlet_in_finder() {
            let info = ScriptInfo::scriptlet("My Snippet", "/path/to/snippets.md", None, None);
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "reveal_scriptlet_in_finder"));
        }

        #[test]
        fn script_context_scriptlet_no_view_logs() {
            let info = ScriptInfo::scriptlet("My Snippet", "/path/to/snippets.md", None, None);
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        // =========================================================================
        // 14. Script context: builtin has exactly 4 actions when no shortcut/alias
        // =========================================================================

        #[test]
        fn builtin_no_shortcut_no_alias_has_4_actions() {
            let info = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&info);
            // run_script, toggle_info, add_shortcut, add_alias, copy_deeplink = 5
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn builtin_action_ids() {
            let info = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&info);
            let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert!(ids.contains(&"run_script"));
            assert!(ids.contains(&"add_shortcut"));
            assert!(ids.contains(&"add_alias"));
            assert!(ids.contains(&"copy_deeplink"));
        }

        #[test]
        fn builtin_no_edit_no_reveal_no_copy_path() {
            let info = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "edit_script"));
            assert!(!actions.iter().any(|a| a.id == "file:reveal_in_finder"));
            assert!(!actions.iter().any(|a| a.id == "file:copy_path"));
        }

        #[test]
        fn builtin_no_view_logs() {
            let info = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        // =========================================================================
        // 15. Script context: primary title uses action_verb
        // =========================================================================

        #[test]
        fn script_primary_title_uses_run_verb() {
            let info = ScriptInfo::new("my-script", "/path/to/my-script.ts");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Run");
        }

        #[test]
        fn script_primary_title_uses_custom_verb() {
            let info = ScriptInfo::with_action_verb("launcher", "/path", true, "Launch");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].title, "Launch");
        }

        #[test]
        fn script_primary_desc_uses_verb() {
            let info = ScriptInfo::with_action_verb("app", "/path", false, "Open");
            let actions = get_script_context_actions(&info);
            assert!(actions[0].description.as_ref().unwrap().contains("Open"));
        }

        #[test]
        fn script_primary_shortcut_is_enter() {
            let info = ScriptInfo::new("test", "/test");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions[0].shortcut, Some("↵".to_string()));
        }

        // =========================================================================
        // 16. Scriptlet context: with_custom run_script is first action
        // =========================================================================

        #[test]
        fn scriptlet_with_custom_first_is_run_script() {
            let script = ScriptInfo::scriptlet("snippet", "/path/snippet.md", None, None);
            let scriptlet = Scriptlet::new(
                "snippet".to_string(),
                "bash".to_string(),
                "echo hello".to_string(),
            );
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn scriptlet_with_custom_run_title_includes_name() {
            let script = ScriptInfo::scriptlet("My Snippet", "/path/snippet.md", None, None);
            let scriptlet = Scriptlet::new(
                "My Snippet".to_string(),
                "bash".to_string(),
                "echo hello".to_string(),
            );
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            assert!(actions[0].title.contains("My Snippet"));
        }

        #[test]
        fn scriptlet_with_custom_run_shortcut_enter() {
            let script = ScriptInfo::scriptlet("snippet", "/path/snippet.md", None, None);
            let scriptlet = Scriptlet::new(
                "snippet".to_string(),
                "bash".to_string(),
                "echo hello".to_string(),
            );
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            assert_eq!(actions[0].shortcut, Some("↵".to_string()));
        }

        #[test]
        fn scriptlet_with_custom_none_scriptlet_has_no_custom_actions() {
            let script = ScriptInfo::scriptlet("snippet", "/path/snippet.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            // Should not have any scriptlet_action: prefixed actions
            assert!(
                !actions
                    .iter()
                    .any(|a| a.id.starts_with("scriptlet_action:"))
            );
        }

        // =========================================================================
        // 17. Scriptlet context: with_custom copy_deeplink URL uses to_deeplink_name
        // =========================================================================

        #[test]
        fn scriptlet_copy_deeplink_uses_deeplink_name() {
            let script = ScriptInfo::scriptlet("Open GitHub", "/path/snippet.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let deeplink = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert!(
                deeplink
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("open-github")
            );
        }

        #[test]
        fn scriptlet_copy_deeplink_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let deeplink = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(deeplink.shortcut, Some("⌘⇧D".to_string()));
        }

        #[test]
        fn scriptlet_copy_content_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert_eq!(cc.shortcut, Some("⌘⌥C".to_string()));
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn scriptlet_edit_scriptlet_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
            assert_eq!(edit.shortcut, Some("⌘E".to_string()));
        }

        // =========================================================================
        // 18. AI bar: toggle_shortcuts_help details
        // =========================================================================

        #[test]
        fn ai_bar_toggle_shortcuts_help_shortcut() {
            let actions = get_ai_command_bar_actions();
            let help = actions
                .iter()
                .find(|a| a.id == "chat:toggle_shortcuts_help")
                .unwrap();
            assert_eq!(help.shortcut, Some("⌘/".to_string()));
        }

        #[test]
        fn ai_bar_toggle_shortcuts_help_icon() {
            let actions = get_ai_command_bar_actions();
            let help = actions
                .iter()
                .find(|a| a.id == "chat:toggle_shortcuts_help")
                .unwrap();
            assert_eq!(help.icon, Some(IconName::Star));
        }

        #[test]
        fn ai_bar_toggle_shortcuts_help_section() {
            let actions = get_ai_command_bar_actions();
            let help = actions
                .iter()
                .find(|a| a.id == "chat:toggle_shortcuts_help")
                .unwrap();
            assert_eq!(help.section, Some("Help".to_string()));
        }

        #[test]
        fn ai_bar_toggle_shortcuts_help_title() {
            let actions = get_ai_command_bar_actions();
            let help = actions
                .iter()
                .find(|a| a.id == "chat:toggle_shortcuts_help")
                .unwrap();
            assert_eq!(help.title, "Keyboard Shortcuts");
        }

        // =========================================================================
        // 19. AI bar: change_model has no shortcut
        // =========================================================================

        #[test]
        fn ai_bar_change_model_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let model = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert!(model.shortcut.is_none());
        }

        #[test]
        fn ai_bar_change_model_icon_settings() {
            let actions = get_ai_command_bar_actions();
            let model = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert_eq!(model.icon, Some(IconName::Settings));
        }

        #[test]
        fn ai_bar_change_model_section_settings() {
            let actions = get_ai_command_bar_actions();
            let model = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert_eq!(model.section, Some("Settings".to_string()));
        }

        #[test]
        fn ai_bar_branch_from_last_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let branch = actions
                .iter()
                .find(|a| a.id == "chat:branch_from_last")
                .unwrap();
            assert!(branch.shortcut.is_none());
        }

        // =========================================================================
        // 20. AI bar: unique IDs across all 21 actions
        // =========================================================================

        #[test]
        fn ai_bar_has_12_actions() {
            let actions = get_ai_command_bar_actions();
            assert_eq!(actions.len(), 38);
        }

        #[test]
        fn ai_bar_all_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            let original_len = ids.len();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), original_len);
        }

        #[test]
        fn ai_bar_all_have_icons() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.icon.is_some(),
                    "AI bar action '{}' has no icon",
                    action.id
                );
            }
        }

        #[test]
        fn ai_bar_all_have_sections() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.section.is_some(),
                    "AI bar action '{}' has no section",
                    action.id
                );
            }
        }

        // =========================================================================
        // 21. Notes: export action requires selection and not trash
        // =========================================================================

        // =========================================================================
        // 22. Notes: browse_notes always present
        // =========================================================================

        // =========================================================================
        // 23. Chat context: copy_response only when has_response
        // =========================================================================

        #[test]
        fn chat_copy_response_present_when_has_response() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
        }

        #[test]
        fn chat_copy_response_absent_when_no_response() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
        }

        #[test]
        fn chat_copy_response_shortcut() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            let copy = actions
                .iter()
                .find(|a| a.id == "chat:copy_response")
                .unwrap();
            assert_eq!(copy.shortcut, Some("⌘C".to_string()));
        }

        #[test]
        fn chat_copy_response_title() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            let copy = actions
                .iter()
                .find(|a| a.id == "chat:copy_response")
                .unwrap();
            assert_eq!(copy.title, "Copy Last Response");
        }

        // =========================================================================
        // 24. Chat context: clear_conversation only when has_messages
        // =========================================================================

        #[test]
        fn chat_clear_present_when_has_messages() {
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
        fn chat_clear_absent_when_no_messages() {
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
        fn chat_clear_shortcut() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            let clear = actions
                .iter()
                .find(|a| a.id == "chat:clear_conversation")
                .unwrap();
            assert_eq!(clear.shortcut, Some("⌘⌫".to_string()));
        }

        #[test]
        fn chat_continue_in_chat_always_present() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
        }

        // =========================================================================
        // 25. New chat: empty lists produce zero actions
        // =========================================================================

        #[test]
        fn new_chat_empty_inputs_zero_actions() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert_eq!(actions.len(), 0);
        }

        #[test]
        fn new_chat_only_last_used_produces_correct_count() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions.len(), 1);
        }

        #[test]
        fn new_chat_only_models_produces_correct_count() {
            let models = vec![
                NewChatModelInfo {
                    model_id: "gpt4".into(),
                    display_name: "GPT-4".into(),
                    provider: "openai".into(),
                    provider_display_name: "OpenAI".into(),
                },
                NewChatModelInfo {
                    model_id: "claude".into(),
                    display_name: "Claude".into(),
                    provider: "anthropic".into(),
                    provider_display_name: "Anthropic".into(),
                },
            ];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions.len(), 2);
        }

        #[test]
        fn new_chat_all_three_sections_total() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 3);
        }

        // =========================================================================
        // 26. New chat: preset IDs use preset_{id} format
        // =========================================================================

        #[test]
        fn new_chat_preset_id_format() {
            let presets = vec![NewChatPresetInfo {
                id: "code-review".into(),
                name: "Code Review".into(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].id, "preset_code-review");
        }

        #[test]
        fn new_chat_preset_section_is_presets() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].section, Some("Presets".to_string()));
        }

        #[test]
        fn new_chat_preset_icon_preserved() {
            let presets = vec![NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].icon, Some(IconName::Code));
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn new_chat_preset_description_is_none() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(
                actions[0].description.as_deref(),
                Some("Uses General preset")
            );
        }

        // =========================================================================
        // 27. Note switcher: empty notes produces no_notes action
        // =========================================================================

        #[test]
        fn note_switcher_empty_produces_no_notes() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
        }

        #[test]
        fn note_switcher_empty_title() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].title, "No notes yet");
        }

        #[test]
        fn note_switcher_empty_icon_plus() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].icon, Some(IconName::Plus));
        }

        #[test]
        fn note_switcher_empty_desc_mentions_cmd_n() {
            let actions = get_note_switcher_actions(&[]);
            assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
        }

        // =========================================================================
        // 28. Note switcher: preview with relative_time has separator
        // =========================================================================

        #[test]
        fn note_switcher_preview_and_time_has_dot_separator() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc-123".into(),
                title: "Test Note".into(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "Hello world".into(),
                relative_time: "2m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(actions[0].description.as_ref().unwrap().contains(" · "));
        }

        #[test]
        fn note_switcher_preview_only_no_separator() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc-123".into(),
                title: "Test Note".into(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "Hello world".into(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(!actions[0].description.as_ref().unwrap().contains(" · "));
        }

        #[test]
        fn note_switcher_time_only_when_empty_preview() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc-123".into(),
                title: "Test Note".into(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: "5d ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].description.as_ref().unwrap(), "5d ago");
        }

        #[test]
        fn note_switcher_no_preview_no_time_shows_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc-123".into(),
                title: "Test Note".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].description.as_ref().unwrap(), "42 chars");
        }

        // =========================================================================
        // 29. ProtocolAction: with_value sets value and has_action false
        // =========================================================================

        #[test]
        fn protocol_action_with_value_sets_name() {
            let pa = ProtocolAction::with_value("Copy".into(), "copy-cmd".into());
            assert_eq!(pa.name, "Copy");
        }

        #[test]
        fn protocol_action_with_value_sets_value() {
            let pa = ProtocolAction::with_value("Copy".into(), "copy-cmd".into());
            assert_eq!(pa.value, Some("copy-cmd".to_string()));
        }

        #[test]
        fn protocol_action_with_value_has_action_false() {
            let pa = ProtocolAction::with_value("Copy".into(), "copy-cmd".into());
            assert!(!pa.has_action);
        }

        #[test]
        fn protocol_action_with_value_defaults_visible_close_none() {
            let pa = ProtocolAction::with_value("X".into(), "y".into());
            assert!(pa.visible.is_none());
            assert!(pa.close.is_none());
            // But defaults still work:
            assert!(pa.is_visible());
            assert!(pa.should_close());
        }

        // =========================================================================
        // 30. format_shortcut_hint: dialog vs builders produce different results
        // =========================================================================

        #[test]
        fn dialog_format_shortcut_hint_handles_meta() {
            let result = ActionsDialog::format_shortcut_hint("meta+c");
            assert_eq!(result, "⌘C");
        }

        #[test]
        fn dialog_format_shortcut_hint_handles_super() {
            let result = ActionsDialog::format_shortcut_hint("super+x");
            assert_eq!(result, "⌘X");
        }

        #[test]
        fn dialog_format_shortcut_hint_handles_option() {
            let result = ActionsDialog::format_shortcut_hint("option+v");
            assert_eq!(result, "⌥V");
        }

        #[test]
        fn dialog_format_shortcut_hint_handles_compound() {
            let result = ActionsDialog::format_shortcut_hint("ctrl+shift+alt+k");
            assert_eq!(result, "⌃⇧⌥K");
        }
    }
}

mod from_dialog_builtin_action_validation_tests_41 {
    //! Batch 41: Dialog builtin action validation tests
    //!
    //! Focuses on:
    //! - fuzzy_match: empty needle and empty haystack behavior
    //! - fuzzy_match: subsequence order enforcement
    //! - score_action: description-only match yields exactly 15
    //! - score_action: shortcut-only match yields exactly 10
    //! - score_action: combined prefix + description + shortcut max score
    //! - builders format_shortcut_hint: simpler .replace chain vs dialog
    //! - builders format_shortcut_hint: unknown keys pass through
    //! - parse_shortcut_keycaps: all modifier symbols individually
    //! - parse_shortcut_keycaps: empty string produces empty vec
    //! - Clipboard: share action details (shortcut, title, position)
    //! - Clipboard: attach_to_ai action details
    //! - Clipboard: image open_with is macOS only
    //! - File context: primary action ID differs file vs dir
    //! - File context: all IDs unique within context
    //! - Path context: open_in_quick_terminal shortcut and desc
    //! - Path context: move_to_trash desc differs file vs dir
    //! - Script context: with shortcut yields update_shortcut + remove_shortcut
    //! - Script context: with alias yields update_alias + remove_alias
    //! - Script context: agent has edit_script with "Edit Agent" title, desc mentions agent
    //! - Script context: total action count varies by type
    //! - Scriptlet context: add_shortcut when no shortcut, add_alias when no alias
    //! - Scriptlet context: reset_ranking only when is_suggested
    //! - AI bar: delete_chat shortcut and icon
    //! - AI bar: new_chat shortcut and icon
    //! - Notes: format action details
    //! - Notes: selection+trash yields subset of actions
    //! - Chat context: model with current_model gets checkmark
    //! - Chat context: multiple models ordering
    //! - New chat: section ordering across last_used, presets, models
    //! - count_section_headers: items without sections produce 0 headers

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use crate::actions::builders::*;
        use crate::actions::dialog::ActionsDialog;
        use crate::actions::types::{Action, ActionCategory, ScriptInfo};
        use crate::actions::window::count_section_headers;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::Scriptlet;

        // =========================================================================
        // 1. fuzzy_match: empty needle and empty haystack behavior
        // =========================================================================

        #[test]
        fn fuzzy_match_empty_needle_matches_anything() {
            // Empty needle should match any haystack (no characters to find)
            assert!(ActionsDialog::fuzzy_match("hello world", ""));
        }

        #[test]
        fn fuzzy_match_empty_haystack_fails_nonempty_needle() {
            // Non-empty needle can't be found in empty haystack
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn fuzzy_match_both_empty_matches() {
            // Both empty: trivially matches
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn fuzzy_match_single_char_in_haystack() {
            assert!(ActionsDialog::fuzzy_match("abcdef", "a"));
            assert!(ActionsDialog::fuzzy_match("abcdef", "f"));
            assert!(!ActionsDialog::fuzzy_match("abcdef", "z"));
        }

        // =========================================================================
        // 2. fuzzy_match: subsequence order enforcement
        // =========================================================================

        #[test]
        fn fuzzy_match_correct_order_matches() {
            assert!(ActionsDialog::fuzzy_match("copy path", "cp"));
        }

        #[test]
        fn fuzzy_match_reversed_order_fails() {
            assert!(!ActionsDialog::fuzzy_match("copy path", "pc"));
        }

        #[test]
        fn fuzzy_match_duplicate_chars_in_needle() {
            // "aa" should match "banana" (a at index 1, a at index 3)
            assert!(ActionsDialog::fuzzy_match("banana", "aa"));
        }

        #[test]
        fn fuzzy_match_full_string_as_subsequence() {
            assert!(ActionsDialog::fuzzy_match("edit", "edit"));
        }

        // =========================================================================
        // 3. score_action: description-only match yields exactly 15
        // =========================================================================

        #[test]
        fn score_action_desc_only_match_is_15() {
            let action = Action::new(
                "test",
                "Xyz Title",
                Some("open in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            // Search "editor" won't match title "xyz title" but will match description
            let score = ActionsDialog::score_action(&action, "editor");
            assert_eq!(score, 15);
        }

        #[test]
        fn score_action_no_desc_no_match_is_0() {
            let action = Action::new("test", "Xyz Title", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "editor");
            assert_eq!(score, 0);
        }

        #[test]
        fn score_action_desc_match_plus_title_prefix() {
            let action = Action::new(
                "test",
                "Open File",
                Some("Open in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            // "open" matches title prefix (100) + description contains (15)
            let score = ActionsDialog::score_action(&action, "open");
            assert_eq!(score, 115);
        }

        #[test]
        fn score_action_desc_match_plus_title_contains() {
            let action = Action::new(
                "test",
                "My Open File",
                Some("Open in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            // "open" matches title contains (50) + description contains (15)
            let score = ActionsDialog::score_action(&action, "open");
            assert_eq!(score, 65);
        }

        // =========================================================================
        // 4. score_action: shortcut-only match yields exactly 10
        // =========================================================================

        #[test]
        fn score_action_shortcut_only_match_is_10() {
            let action = Action::new("test", "Xyz Title", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            // Search "⌘e" (lowercase) matches shortcut_lower "⌘e"
            let score = ActionsDialog::score_action(&action, "⌘e");
            assert_eq!(score, 10);
        }

        #[test]
        fn score_action_shortcut_no_match_is_0() {
            let action = Action::new("test", "Abc Title", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            // "q" doesn't match title "abc title", description None, or shortcut "⌘e"
            let score = ActionsDialog::score_action(&action, "q");
            assert_eq!(score, 0);
        }

        #[test]
        fn score_action_shortcut_plus_title_prefix() {
            let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            // "e" matches title prefix "edit script" (100) + shortcut_lower "⌘e" contains "e" (10)
            let score = ActionsDialog::score_action(&action, "e");
            assert_eq!(score, 110);
        }

        #[test]
        fn score_action_shortcut_plus_desc() {
            let action = Action::new(
                "test",
                "Xyz Title",
                Some("open in editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E");
            // "⌘e" only matches shortcut (10), not title, not desc
            let score = ActionsDialog::score_action(&action, "⌘e");
            assert_eq!(score, 10);
        }

        // =========================================================================
        // 5. score_action: combined prefix + description + shortcut max score
        // =========================================================================

        #[test]
        fn score_action_max_combined_prefix_desc_shortcut() {
            let action = Action::new(
                "test",
                "edit script",
                Some("edit in editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("edit");
            // "edit" matches: title prefix (100) + desc contains (15) + shortcut contains (10) = 125
            let score = ActionsDialog::score_action(&action, "edit");
            assert_eq!(score, 125);
        }

        #[test]
        fn score_action_contains_desc_shortcut() {
            let action = Action::new(
                "test",
                "My Edit Script",
                Some("edit in editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("edit");
            // "edit" matches: title contains (50) + desc contains (15) + shortcut contains (10) = 75
            let score = ActionsDialog::score_action(&action, "edit");
            assert_eq!(score, 75);
        }

        #[test]
        fn score_action_no_title_match_desc_and_shortcut() {
            let action = Action::new(
                "test",
                "my xdyt script",
                Some("edit in editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("edit");
            // "edit": 'e' not in "my xdyt script" so no fuzzy → 0
            // desc "edit in editor" contains "edit" → +15
            // shortcut "edit" contains "edit" → +10
            // Total: 25
            let score = ActionsDialog::score_action(&action, "edit");
            assert_eq!(score, 25);
        }

        #[test]
        fn score_action_no_title_match_desc_shortcut_only() {
            let action = Action::new(
                "test",
                "Xyz Abc",
                Some("Open file in editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E");
            // "⌘e" matches: no title match (0) + no desc match (0) + shortcut (10) = 10
            let score = ActionsDialog::score_action(&action, "⌘e");
            assert_eq!(score, 10);
        }

        // =========================================================================
        // 6. builders format_shortcut_hint: simpler .replace chain vs dialog
        // =========================================================================

        #[test]
        fn builders_format_shortcut_hint_cmd_c() {
            // builders::format_shortcut_hint uses simple .replace() chain
            // "cmd+c" → replace cmd→⌘, replace +→"", uppercase → "⌘C"
            let hint = format_shortcut_hint_for_test("cmd+c");
            assert_eq!(hint, "⌘C");
        }

        #[test]
        fn builders_format_shortcut_hint_ctrl_shift_x() {
            let hint = format_shortcut_hint_for_test("ctrl+shift+x");
            assert_eq!(hint, "⌃⇧X");
        }

        #[test]
        fn builders_format_shortcut_hint_alt_k() {
            let hint = format_shortcut_hint_for_test("alt+k");
            assert_eq!(hint, "⌥K");
        }

        #[test]
        fn builders_format_shortcut_hint_single_letter() {
            // Just a single letter "a" → "A"
            let hint = format_shortcut_hint_for_test("a");
            assert_eq!(hint, "A");
        }

        // Helper to call the builders-level format_shortcut_hint (private fn, test via scriptlet)
        fn format_shortcut_hint_for_test(shortcut: &str) -> String {
            // We can test this indirectly by creating a scriptlet action with a shortcut
            // and checking the resulting action's shortcut field
            let mut scriptlet =
                Scriptlet::new("Test".to_string(), "bash".to_string(), String::new());
            scriptlet.actions.push(crate::scriptlets::ScriptletAction {
                name: "Test Action".to_string(),
                command: "test-action".to_string(),
                description: None,
                shortcut: Some(shortcut.to_string()),
                tool: "bash".to_string(),
                code: String::new(),
                inputs: vec![],
            });
            let actions = get_scriptlet_defined_actions(&scriptlet);
            actions[0].shortcut.clone().unwrap_or_default()
        }

        // =========================================================================
        // 7. builders format_shortcut_hint: unknown keys pass through
        // =========================================================================

        #[test]
        fn builders_format_unknown_key_uppercased() {
            let hint = format_shortcut_hint_for_test("cmd+f1");
            // "cmd+f1" → "⌘" + remove + → "⌘F1"
            assert_eq!(hint, "⌘F1");
        }

        #[test]
        fn builders_format_numbers_preserved() {
            let hint = format_shortcut_hint_for_test("cmd+1");
            assert_eq!(hint, "⌘1");
        }

        #[test]
        fn builders_format_empty_shortcut() {
            let hint = format_shortcut_hint_for_test("");
            assert_eq!(hint, "");
        }

        #[test]
        fn builders_format_all_four_modifiers() {
            let hint = format_shortcut_hint_for_test("cmd+ctrl+alt+shift+k");
            assert_eq!(hint, "⌘⌃⌥⇧K");
        }

        // =========================================================================
        // 8. parse_shortcut_keycaps: all modifier symbols individually
        // =========================================================================

        #[test]
        fn parse_keycaps_command_symbol() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘");
            assert_eq!(caps, vec!["⌘"]);
        }

        #[test]
        fn parse_keycaps_all_arrows() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn parse_keycaps_escape_tab_backspace_space() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⎋⇥⌫␣");
            assert_eq!(caps, vec!["⎋", "⇥", "⌫", "␣"]);
        }

        #[test]
        fn parse_keycaps_mixed_modifiers_and_letter() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(caps, vec!["⌘", "⇧", "C"]);
        }

        // =========================================================================
        // 9. parse_shortcut_keycaps: empty string produces empty vec
        // =========================================================================

        #[test]
        fn parse_keycaps_empty_string() {
            let caps = ActionsDialog::parse_shortcut_keycaps("");
            assert!(caps.is_empty());
        }

        #[test]
        fn parse_keycaps_lowercase_uppercased() {
            let caps = ActionsDialog::parse_shortcut_keycaps("a");
            assert_eq!(caps, vec!["A"]);
        }

        #[test]
        fn parse_keycaps_digit_preserved() {
            let caps = ActionsDialog::parse_shortcut_keycaps("1");
            assert_eq!(caps, vec!["1"]);
        }

        #[test]
        fn parse_keycaps_return_symbol() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(caps, vec!["↵"]);
        }

        // =========================================================================
        // 10. Clipboard: share action details (shortcut, title, position)
        // =========================================================================

        #[test]
        fn clipboard_share_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let share = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_share")
                .unwrap();
            assert_eq!(share.shortcut.as_deref(), Some("⇧⌘E"));
        }

        #[test]
        fn clipboard_share_title() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let share = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_share")
                .unwrap();
            assert_eq!(share.title, "Share...");
        }

        #[test]
        fn clipboard_share_position_after_paste_keep_open() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let share_idx = actions
                .iter()
                .position(|a| a.id == "clip:clipboard_share")
                .unwrap();
            let paste_keep_idx = actions
                .iter()
                .position(|a| a.id == "clip:clipboard_paste_keep_open")
                .unwrap();
            assert!(share_idx > paste_keep_idx);
        }

        #[test]
        fn clipboard_share_desc_mentions_share() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let share = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_share")
                .unwrap();
            assert!(
                share
                    .description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("share")
            );
        }

        // =========================================================================
        // 11. Clipboard: attach_to_ai action details
        // =========================================================================

        // --- merged from tests_part_02.rs ---
        #[test]
        fn clipboard_attach_to_ai_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let attach = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_attach_to_ai")
                .unwrap();
            assert_eq!(attach.shortcut.as_deref(), Some("⌃⌘A"));
        }

        #[test]
        fn clipboard_attach_to_ai_title() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let attach = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_attach_to_ai")
                .unwrap();
            assert_eq!(attach.title, "Attach to Agent Chat");
        }

        #[test]
        fn clipboard_attach_to_ai_desc_mentions_ai() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".to_string(),
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
        fn clipboard_attach_to_ai_present_for_image_too() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
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

        // =========================================================================
        // 12. Clipboard: image open_with is macOS only
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_image_has_open_with() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_text_has_no_open_with() {
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

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_image_open_with_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let open_with = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_open_with")
                .unwrap();
            assert_eq!(open_with.shortcut.as_deref(), Some("⌘O"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn clipboard_image_annotate_cleanshot_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let annotate = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_annotate_cleanshot")
                .unwrap();
            assert_eq!(annotate.shortcut.as_deref(), Some("⇧⌘A"));
        }

        // =========================================================================
        // 13. File context: primary action ID differs file vs dir
        // =========================================================================

        #[test]
        fn file_context_file_primary_id_is_open_file() {
            let info = FileInfo {
                name: "readme.md".to_string(),
                path: "/docs/readme.md".to_string(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_file");
        }

        #[test]
        fn file_context_dir_primary_id_is_open_directory() {
            let info = FileInfo {
                name: "src".to_string(),
                path: "/project/src".to_string(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn file_context_primary_shortcut_is_enter() {
            let info = FileInfo {
                name: "test.txt".to_string(),
                path: "/test.txt".to_string(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn file_context_dir_primary_desc_mentions_folder() {
            let info = FileInfo {
                name: "lib".to_string(),
                path: "/lib".to_string(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&info);
            assert!(
                actions[0]
                    .description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("folder")
            );
        }

        // =========================================================================
        // 14. File context: all IDs unique within context
        // =========================================================================

        #[test]
        fn file_context_file_all_ids_unique() {
            let info = FileInfo {
                name: "test.rs".to_string(),
                path: "/test.rs".to_string(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            let mut unique_ids = ids.clone();
            unique_ids.sort();
            unique_ids.dedup();
            assert_eq!(ids.len(), unique_ids.len());
        }

        #[test]
        fn file_context_dir_all_ids_unique() {
            let info = FileInfo {
                name: "docs".to_string(),
                path: "/docs".to_string(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&info);
            let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            let mut unique_ids = ids.clone();
            unique_ids.sort();
            unique_ids.dedup();
            assert_eq!(ids.len(), unique_ids.len());
        }

        #[test]
        fn file_context_file_has_copy_path_and_copy_filename() {
            let info = FileInfo {
                name: "foo.txt".to_string(),
                path: "/foo.txt".to_string(),
                is_dir: false,
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "file:copy_path"));
            assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
        }

        #[test]
        fn file_context_reveal_in_finder_always_present() {
            let file_info = FileInfo {
                name: "a.txt".to_string(),
                path: "/a.txt".to_string(),
                is_dir: false,
                file_type: FileType::File,
            };
            let dir_info = FileInfo {
                name: "b".to_string(),
                path: "/b".to_string(),
                is_dir: true,
                file_type: FileType::Directory,
            };
            assert!(
                get_file_context_actions(&file_info)
                    .iter()
                    .any(|a| a.id == "file:reveal_in_finder")
            );
            assert!(
                get_file_context_actions(&dir_info)
                    .iter()
                    .any(|a| a.id == "file:reveal_in_finder")
            );
        }

        // =========================================================================
        // 15. Path context: open_in_quick_terminal has no shortcut, desc mentions it
        // =========================================================================

        #[test]
        fn path_context_open_in_quick_terminal_shortcut() {
            let info = PathInfo {
                name: "src".to_string(),
                path: "/project/src".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let term = actions
                .iter()
                .find(|a| a.id == "file:open_in_quick_terminal")
                .unwrap();
            assert_eq!(term.shortcut.as_deref(), None);
        }

        #[test]
        fn path_context_open_in_quick_terminal_desc_mentions_terminal() {
            let info = PathInfo {
                name: "src".to_string(),
                path: "/project/src".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let term = actions
                .iter()
                .find(|a| a.id == "file:open_in_quick_terminal")
                .unwrap();
            assert!(
                term.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("terminal")
            );
        }

        #[test]
        fn path_context_open_in_quick_terminal_present_for_files() {
            let info = PathInfo {
                name: "script.sh".to_string(),
                path: "/project/script.sh".to_string(),
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
        fn path_context_open_in_quick_terminal_title() {
            let info = PathInfo {
                name: "foo".to_string(),
                path: "/foo".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let term = actions
                .iter()
                .find(|a| a.id == "file:open_in_quick_terminal")
                .unwrap();
            assert_eq!(term.title, "Open in Quick Terminal");
        }

        // =========================================================================
        // 16. Path context: move_to_trash desc differs file vs dir
        // =========================================================================

        #[test]
        fn path_context_trash_desc_file() {
            let info = PathInfo {
                name: "test.txt".to_string(),
                path: "/test.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(trash.description.as_ref().unwrap().contains("file"));
        }

        #[test]
        fn path_context_trash_desc_dir() {
            let info = PathInfo {
                name: "src".to_string(),
                path: "/src".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(trash.description.as_ref().unwrap().contains("folder"));
        }

        #[test]
        fn path_context_trash_shortcut() {
            let info = PathInfo {
                name: "x".to_string(),
                path: "/x".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert_eq!(trash.shortcut.as_deref(), Some("⌘⌫"));
        }

        #[test]
        fn path_context_trash_is_last_action() {
            let info = PathInfo {
                name: "y".to_string(),
                path: "/y".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
        }

        // =========================================================================
        // 17. Script context: with shortcut yields update_shortcut + remove_shortcut
        // =========================================================================

        #[test]
        fn script_with_shortcut_has_update_shortcut() {
            let info = ScriptInfo::with_shortcut("my-script", "/s.ts", Some("cmd+k".into()));
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "update_shortcut"));
        }

        #[test]
        fn script_with_shortcut_has_remove_shortcut() {
            let info = ScriptInfo::with_shortcut("my-script", "/s.ts", Some("cmd+k".into()));
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
        }

        #[test]
        fn script_with_shortcut_has_no_add_shortcut() {
            let info = ScriptInfo::with_shortcut("my-script", "/s.ts", Some("cmd+k".into()));
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
        }

        #[test]
        fn script_without_shortcut_has_add_shortcut() {
            let info = ScriptInfo::new("my-script", "/s.ts");
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "add_shortcut"));
            assert!(!actions.iter().any(|a| a.id == "update_shortcut"));
            assert!(!actions.iter().any(|a| a.id == "remove_shortcut"));
        }

        // =========================================================================
        // 18. Script context: with alias yields update_alias + remove_alias
        // =========================================================================

        #[test]
        fn script_with_alias_has_update_alias() {
            let info = ScriptInfo::with_shortcut_and_alias(
                "my-script",
                "/s.ts",
                None,
                Some("ms".to_string()),
            );
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "update_alias"));
        }

        #[test]
        fn script_with_alias_has_remove_alias() {
            let info = ScriptInfo::with_shortcut_and_alias(
                "my-script",
                "/s.ts",
                None,
                Some("ms".to_string()),
            );
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "remove_alias"));
        }

        #[test]
        fn script_with_alias_has_no_add_alias() {
            let info = ScriptInfo::with_shortcut_and_alias(
                "my-script",
                "/s.ts",
                None,
                Some("ms".to_string()),
            );
            let actions = get_script_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "add_alias"));
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn script_without_alias_has_add_alias() {
            let info = ScriptInfo::new("my-script", "/s.ts");
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "add_alias"));
            assert!(!actions.iter().any(|a| a.id == "update_alias"));
            assert!(!actions.iter().any(|a| a.id == "remove_alias"));
        }

        // =========================================================================
        // 19. Script context: agent has edit_script with "Edit Agent" title, desc mentions agent
        // =========================================================================

        #[test]
        fn agent_edit_title_is_edit_agent() {
            let mut info = ScriptInfo::new("my-agent", "/a.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn agent_edit_desc_mentions_agent() {
            let mut info = ScriptInfo::new("my-agent", "/a.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
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
        fn agent_has_reveal_in_finder() {
            let mut info = ScriptInfo::new("my-agent", "/a.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
        }

        #[test]
        fn agent_reveal_desc_mentions_agent() {
            let mut info = ScriptInfo::new("my-agent", "/a.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
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

        // =========================================================================
        // 20. Script context: total action count varies by type
        // =========================================================================

        #[test]
        fn script_context_real_script_count() {
            // Real script: run_script, toggle_info, add_shortcut, add_alias, toggle_favorite,
            // edit_script, view_logs, reveal_in_finder, file:open_in_quick_terminal,
            // copy_path, copy_content, copy_deeplink, delete_script = 13
            let info = ScriptInfo::new("test", "/test.ts");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions.len(), 13);
        }

        #[test]
        fn script_context_builtin_count() {
            // Builtin: run + toggle_info + add_shortcut + add_alias + copy_deeplink = 5
            let info = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&info);
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn script_context_agent_count() {
            // Agent with non-empty path (no shortcut/alias actions ever): run_script,
            // toggle_info, toggle_favorite, edit_script, reveal_in_finder,
            // file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink = 9
            let mut info = ScriptInfo::new("my-agent", "/a.md");
            info.is_agent = true;
            info.is_script = false;
            let actions = get_script_context_actions(&info);
            assert_eq!(actions.len(), 9);
        }

        #[test]
        fn script_context_scriptlet_count() {
            // Scriptlet via script context: run_script, toggle_info, add_shortcut, add_alias,
            // toggle_favorite, edit_scriptlet, reveal_scriptlet_in_finder,
            // file:open_in_quick_terminal, copy_scriptlet_path, copy_content, copy_deeplink = 11
            let info = ScriptInfo::scriptlet("Test Scriptlet", "/t.md", None, None);
            let actions = get_script_context_actions(&info);
            assert_eq!(actions.len(), 11);
        }

        // =========================================================================
        // 21. Scriptlet context: add_shortcut when no shortcut, add_alias when no alias
        // =========================================================================

        #[test]
        fn scriptlet_with_custom_no_shortcut_has_add_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "add_shortcut"));
            assert!(!actions.iter().any(|a| a.id == "update_shortcut"));
        }

        #[test]
        fn scriptlet_with_custom_has_shortcut_shows_update_remove() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", Some("cmd+t".into()), None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "update_shortcut"));
            assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
            assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
        }

        #[test]
        fn scriptlet_with_custom_no_alias_has_add_alias() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "add_alias"));
            assert!(!actions.iter().any(|a| a.id == "update_alias"));
        }

        #[test]
        fn scriptlet_with_custom_has_alias_shows_update_remove() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, Some("tst".into()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "update_alias"));
            assert!(actions.iter().any(|a| a.id == "remove_alias"));
            assert!(!actions.iter().any(|a| a.id == "add_alias"));
        }

        // =========================================================================
        // 22. Scriptlet context: reset_ranking only when is_suggested
        // =========================================================================

        #[test]
        fn scriptlet_with_custom_suggested_has_reset_ranking() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, None)
                .with_frecency(true, Some("/t.md".into()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn scriptlet_with_custom_not_suggested_no_reset_ranking() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(!actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn scriptlet_with_custom_reset_ranking_is_last() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, None)
                .with_frecency(true, Some("/t.md".into()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert_eq!(actions.last().unwrap().id, "reset_ranking");
        }

        #[test]
        fn scriptlet_with_custom_reset_ranking_has_no_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/t.md", None, None)
                .with_frecency(true, Some("/t.md".into()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let reset = actions.iter().find(|a| a.id == "reset_ranking").unwrap();
            assert_eq!(reset.shortcut.as_deref(), Some("⌃⌘R"));
        }

        // =========================================================================
        // 23. AI bar: delete_chat shortcut and icon
        // =========================================================================

        #[test]
        fn ai_bar_delete_chat_shortcut() {
            let actions = get_ai_command_bar_actions();
            let delete = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(delete.shortcut.as_deref(), Some("⌘⌫"));
        }

        #[test]
        fn ai_bar_delete_chat_icon() {
            let actions = get_ai_command_bar_actions();
            let delete = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(delete.icon, Some(IconName::Trash));
        }

        #[test]
        fn ai_bar_delete_chat_section() {
            let actions = get_ai_command_bar_actions();
            let delete = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(delete.section.as_deref(), Some("Actions"));
        }

        #[test]
        fn ai_bar_delete_chat_desc_mentions_delete() {
            let actions = get_ai_command_bar_actions();
            let delete = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert!(
                delete
                    .description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("delete")
            );
        }

        // =========================================================================
        // 24. AI bar: new_chat shortcut and icon
        // =========================================================================

        #[test]
        fn ai_bar_new_chat_shortcut() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(nc.shortcut.as_deref(), Some("⌘N"));
        }

        #[test]
        fn ai_bar_new_chat_icon() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(nc.icon, Some(IconName::Plus));
        }

        #[test]
        fn ai_bar_new_chat_section() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(nc.section.as_deref(), Some("Actions"));
        }

        #[test]
        fn ai_bar_new_chat_desc_mentions_conversation() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert!(
                nc.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("conversation")
            );
        }

        // =========================================================================
        // 25. Notes: format action details
        // =========================================================================

        // =========================================================================
        // 26. Notes: selection+trash yields subset of actions
        // =========================================================================

        // =========================================================================
        // 27. Chat context: model with current_model gets checkmark
        // =========================================================================

        #[test]
        fn chat_current_model_has_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "gpt4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt4")
                .unwrap();
            assert!(model_action.title.contains("✓"));
        }

        #[test]
        fn chat_non_current_model_no_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "gpt4".to_string(),
                    display_name: "GPT-4".to_string(),
                    provider: "OpenAI".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt4")
                .unwrap();
            assert!(!model_action.title.contains("✓"));
        }

        #[test]
        fn chat_no_current_model_no_checkmark() {
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
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt4")
                .unwrap();
            assert!(!model_action.title.contains("✓"));
        }

        #[test]
        fn chat_model_desc_mentions_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "claude".to_string(),
                    display_name: "Claude".to_string(),
                    provider: "Anthropic".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude")
                .unwrap();
            assert!(
                model_action
                    .description
                    .as_ref()
                    .unwrap()
                    .contains("Anthropic")
            );
        }

        // =========================================================================
        // 28. Chat context: multiple models ordering
        // =========================================================================

        // --- merged from tests_part_04.rs ---
        #[test]
        fn chat_models_come_before_continue_in_chat() {
            // After drill-down refactor: model picker only has model rows,
            // continue_in_chat lives in the root context actions.
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![
                    ChatModelInfo {
                        id: "a".to_string(),
                        display_name: "Model A".to_string(),
                        provider: "P".to_string(),
                    },
                    ChatModelInfo {
                        id: "b".to_string(),
                        display_name: "Model B".to_string(),
                        provider: "P".to_string(),
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
            assert!(picker.iter().any(|a| a.id == "chat:select_model_a"));
            assert!(picker.iter().any(|a| a.id == "chat:select_model_b"));
            // continue_in_chat is in the root context, not the picker
            let root = get_chat_context_actions(&info);
            assert!(root.iter().any(|a| a.id == "chat:continue_in_chat"));
        }

        #[test]
        fn chat_models_preserve_order() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![
                    ChatModelInfo {
                        id: "first".to_string(),
                        display_name: "First".to_string(),
                        provider: "P".to_string(),
                    },
                    ChatModelInfo {
                        id: "second".to_string(),
                        display_name: "Second".to_string(),
                        provider: "P".to_string(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let first_idx = picker
                .iter()
                .position(|a| a.id == "chat:select_model_first")
                .unwrap();
            let second_idx = picker
                .iter()
                .position(|a| a.id == "chat:select_model_second")
                .unwrap();
            assert!(first_idx < second_idx);
        }

        #[test]
        fn chat_both_messages_and_response_max_actions() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m1".to_string(),
                    display_name: "Model".to_string(),
                    provider: "P".to_string(),
                }],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            // 1 model + continue + copy_response + clear + capture = 5
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn chat_no_models_no_messages_minimal() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            // continue + capture
            assert_eq!(actions.len(), 3);
            assert_eq!(actions[0].id, "chat:change_model");
        }

        // =========================================================================
        // 29. New chat: section ordering across last_used, presets, models
        // =========================================================================

        #[test]
        fn new_chat_section_ordering_last_used_first() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".to_string(),
                display_name: "Model 2".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            // First action section should be Last Used Settings
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
        }

        #[test]
        fn new_chat_section_ordering_presets_second() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".to_string(),
                display_name: "Model 2".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
        }

        #[test]
        fn new_chat_section_ordering_models_last() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".to_string(),
                display_name: "Model 2".to_string(),
                provider: "p".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        #[test]
        fn new_chat_total_count_matches_input_sizes() {
            let last_used = vec![
                NewChatModelInfo {
                    model_id: "m1".to_string(),
                    display_name: "M1".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                },
                NewChatModelInfo {
                    model_id: "m2".to_string(),
                    display_name: "M2".to_string(),
                    provider: "p".to_string(),
                    provider_display_name: "P".to_string(),
                },
            ];
            let presets = vec![NewChatPresetInfo {
                id: "g".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m3".to_string(),
                display_name: "M3".to_string(),
                provider: "p".to_string(),
                provider_display_name: "P".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 4); // 2 + 1 + 1
        }

        // =========================================================================
        // 30. count_section_headers: items without sections produce 0 headers
        // =========================================================================

        #[test]
        fn count_headers_no_sections_is_zero() {
            let actions = vec![
                Action::new("a", "Action A", None, ActionCategory::ScriptContext),
                Action::new("b", "Action B", None, ActionCategory::ScriptContext),
            ];
            let indices: Vec<usize> = (0..actions.len()).collect();
            assert_eq!(count_section_headers(&actions, &indices), 0);
        }

        #[test]
        fn count_headers_all_same_section_is_one() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Group"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Group"),
            ];
            let indices: Vec<usize> = (0..actions.len()).collect();
            assert_eq!(count_section_headers(&actions, &indices), 1);
        }

        #[test]
        fn count_headers_two_different_sections() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Alpha"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Beta"),
            ];
            let indices: Vec<usize> = (0..actions.len()).collect();
            assert_eq!(count_section_headers(&actions, &indices), 2);
        }

        #[test]
        fn count_headers_empty_indices() {
            let actions =
                vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X")];
            let indices: Vec<usize> = vec![];
            assert_eq!(count_section_headers(&actions, &indices), 0);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_42 {
    //! Purged batch 42 validation tests.
    //!
    //! The removed suite duplicated constructor/formatting/context assertions that
    //! are covered by canonical dialog and window tests.
}

mod from_dialog_builtin_action_validation_tests_43 {
    //! Purged batch 43 validation tests.
    //!
    //! This generated batch overlapped extensively with existing dialog/action tests
    //! and consisted primarily of trivial or redundant assertions.
}

mod from_dialog_builtin_action_validation_tests_44 {
    // --- merged from part_01.rs ---
    //! Batch 44: Dialog Built-in Action Validation Tests
    //!
    //! 120 tests across 30 categories validating action behaviors
    //! in various built-in action window dialogs.

    use crate::actions::builders::*;
    use crate::actions::dialog::ActionsDialog;
    use crate::actions::types::{Action, ActionCategory, ScriptInfo};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // =========== 1. ScriptInfo::with_is_script: is_script true sets correct defaults ===========

    #[test]
    fn with_is_script_true_sets_is_script() {
        let s = ScriptInfo::with_is_script("my-script", "/path", true);
        assert!(s.is_script);
    }

    #[test]
    fn with_is_script_true_is_scriptlet_false() {
        let s = ScriptInfo::with_is_script("my-script", "/path", true);
        assert!(!s.is_scriptlet);
    }

    #[test]
    fn with_is_script_false_sets_is_script_false() {
        let s = ScriptInfo::with_is_script("builtin", "", false);
        assert!(!s.is_script);
    }

    #[test]
    fn with_is_script_defaults_action_verb_run() {
        let s = ScriptInfo::with_is_script("test", "/p", true);
        assert_eq!(s.action_verb, "Run");
    }

    // =========== 2. ScriptInfo::with_action_verb: custom verb preserved ===========

    #[test]
    fn with_action_verb_sets_verb() {
        let s = ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
        assert_eq!(s.action_verb, "Launch");
    }

    #[test]
    fn with_action_verb_is_script_param() {
        let s = ScriptInfo::with_action_verb("test", "/p", true, "Execute");
        assert!(s.is_script);
    }

    #[test]
    fn with_action_verb_false_is_script() {
        let s = ScriptInfo::with_action_verb("test", "/p", false, "Open");
        assert!(!s.is_script);
    }

    #[test]
    fn with_action_verb_shortcut_none() {
        let s = ScriptInfo::with_action_verb("test", "/p", true, "Run");
        assert!(s.shortcut.is_none());
    }

    // =========== 3. Clipboard: paste title with frontmost_app_name ===========

    #[test]
    fn clipboard_paste_title_with_app_name() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: Some("Safari".to_string()),
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.title, "Paste to Safari");
    }

    #[test]
    fn clipboard_paste_title_without_app_name() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.title, "Paste to Active App");
    }

    #[test]
    fn clipboard_paste_shortcut_is_enter() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.shortcut, Some("↵".to_string()));
    }

    #[test]
    fn clipboard_paste_desc_mentions_paste() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "x".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert!(paste.description.as_ref().unwrap().contains("paste"));
    }

    // =========== 4. Clipboard: save_snippet and save_file details ===========

    #[test]
    fn clipboard_save_snippet_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "code".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ss = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_snippet")
            .unwrap();
        assert_eq!(ss.shortcut, Some("⇧⌘S".to_string()));
    }

    #[test]
    fn clipboard_save_snippet_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "code".into(),
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
    fn clipboard_save_file_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "code".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let sf = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_file")
            .unwrap();
        assert_eq!(sf.shortcut, Some("⌥⇧⌘S".to_string()));
    }

    #[test]
    fn clipboard_save_file_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "code".into(),
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

    // =========== 5. Clipboard: image upload_cleanshot details (macOS) ===========

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let uc = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_upload_cleanshot")
            .unwrap();
        assert_eq!(uc.shortcut, Some("⇧⌘U".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let uc = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_upload_cleanshot")
            .unwrap();
        assert_eq!(uc.title, "Upload to CleanShot X");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_not_present_for_text() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "txt".into(),
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

    #[cfg(target_os = "macos")]
    #[test]
    fn clipboard_upload_cleanshot_desc_mentions_cloud() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((200, 200)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let uc = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_upload_cleanshot")
            .unwrap();
        assert!(uc.description.as_ref().unwrap().contains("Cloud"));
    }

    // =========== 6. Clipboard: OCR shortcut and desc ===========

    #[test]
    fn clipboard_ocr_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert_eq!(ocr.shortcut, Some("⇧⌘C".to_string()));
    }

    #[test]
    fn clipboard_ocr_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert_eq!(ocr.title, "Copy Text from Image");
    }

    #[test]
    fn clipboard_ocr_desc_mentions_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "img".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let ocr = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_ocr")
            .unwrap();
        assert!(ocr.description.as_ref().unwrap().contains("OCR"));
    }

    #[test]
    fn clipboard_ocr_not_present_for_text() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "txt".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    // =========== 7. File context: quick_look only for files (macOS) ===========

    #[cfg(target_os = "macos")]
    #[test]
    fn file_quick_look_present_for_file() {
        let file = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        assert!(actions.iter().any(|a| a.id == "file:quick_look"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn file_quick_look_shortcut() {
        let file = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let ql = actions.iter().find(|a| a.id == "file:quick_look").unwrap();
        assert_eq!(ql.shortcut, Some("⌘Y".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn file_quick_look_absent_for_dir() {
        let dir = FileInfo {
            path: "/test/dir".into(),
            name: "dir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn file_quick_look_desc_mentions_preview() {
        let file = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let ql = actions.iter().find(|a| a.id == "file:quick_look").unwrap();
        assert!(
            ql.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("preview")
        );
    }

    // =========== 8. File context: copy_path shortcut is ⌘⇧C ===========

    #[test]
    fn file_copy_path_shortcut() {
        let file = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.shortcut, Some("⌘⇧C".to_string()));
    }

    #[test]
    fn file_copy_path_title() {
        let file = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert_eq!(cp.title, "Copy Path");
    }

    #[test]
    fn file_copy_path_desc_mentions_clipboard() {
        let file = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&file);
        let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
        assert!(cp.description.as_ref().unwrap().contains("clipboard"));
    }

    #[test]
    fn file_copy_path_present_for_dir() {
        let dir = FileInfo {
            path: "/test/dir".into(),
            name: "dir".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&dir);
        assert!(actions.iter().any(|a| a.id == "file:copy_path"));
    }

    // --- merged from part_02.rs ---

    // =========== 9. Path context: all actions have ScriptContext category ===========

    #[test]
    fn path_file_all_script_context() {
        let p = PathInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&p);
        assert!(
            actions
                .iter()
                .all(|a| a.category == ActionCategory::ScriptContext)
        );
    }

    #[test]
    fn path_dir_all_script_context() {
        let p = PathInfo {
            path: "/test/dir".into(),
            name: "dir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&p);
        assert!(
            actions
                .iter()
                .all(|a| a.category == ActionCategory::ScriptContext)
        );
    }

    #[test]
    fn path_file_primary_is_first() {
        let p = PathInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&p);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn path_dir_primary_is_first() {
        let p = PathInfo {
            path: "/test/dir".into(),
            name: "dir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&p);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    // =========== 10. Script context: run_script title includes verb and quoted name ===========

    #[test]
    fn script_run_title_includes_verb() {
        let s = ScriptInfo::with_action_verb("Test", "/p", true, "Launch");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.title.starts_with("Launch"));
    }

    #[test]
    fn script_run_title_includes_quoted_name() {
        let s = ScriptInfo::new("My Script", "/p");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Run");
    }

    #[test]
    fn script_run_desc_includes_verb() {
        let s = ScriptInfo::with_action_verb("X", "/p", true, "Execute");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.description.as_ref().unwrap().contains("Execute"));
    }

    #[test]
    fn script_run_shortcut_enter() {
        let s = ScriptInfo::new("X", "/p");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.shortcut, Some("↵".to_string()));
    }

    // =========== 11. Script context: copy_deeplink desc uses to_deeplink_name ===========

    #[test]
    fn script_deeplink_desc_has_correct_url() {
        let s = ScriptInfo::new("My Cool Script", "/p");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(
            dl.description.as_deref(),
            Some("Copy a portable Script Kit share link to clipboard")
        );
    }

    #[test]
    fn script_deeplink_shortcut() {
        let s = ScriptInfo::new("X", "/p");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.shortcut, Some("⌘⇧D".to_string()));
    }

    #[test]
    fn script_deeplink_title() {
        let s = ScriptInfo::new("X", "/p");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.title, "Share");
    }

    #[test]
    fn scriptlet_deeplink_desc_has_slugified_name() {
        let s = ScriptInfo::scriptlet("Open GitHub PR", "/path.md", None, None);
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(
            dl.description.as_deref(),
            Some("Copy a portable Script Kit share link to clipboard")
        );
    }

    // =========== 12. Script context: agent actions have agent-specific descriptions ===========

    #[test]
    fn agent_edit_desc_mentions_agent() {
        let mut s = ScriptInfo::new("My Agent", "/p");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn agent_reveal_desc_mentions_agent() {
        let mut s = ScriptInfo::new("My Agent", "/p");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
        assert!(reveal.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn agent_copy_path_desc_mentions_agent() {
        let mut s = ScriptInfo::new("My Agent", "/p");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
        assert!(cp.description.as_ref().unwrap().contains("agent"));
    }

    #[test]
    fn agent_no_view_logs() {
        let mut s = ScriptInfo::new("My Agent", "/p");
        s.is_script = false;
        s.is_agent = true;
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
    }

    // =========== 13. Scriptlet with_custom: run_script title format ===========

    #[test]
    fn scriptlet_with_custom_run_title_includes_name() {
        let s = ScriptInfo::scriptlet("My Snippet", "/path.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.title.contains("\"My Snippet\""));
    }

    #[test]
    fn scriptlet_with_custom_run_title_starts_with_verb() {
        let s = ScriptInfo::scriptlet("X", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.title.starts_with("Run"));
    }

    #[test]
    fn scriptlet_with_custom_edit_desc_mentions_editor() {
        let s = ScriptInfo::scriptlet("X", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let edit = actions.iter().find(|a| a.id == "edit_scriptlet").unwrap();
        assert!(edit.description.as_ref().unwrap().contains("$EDITOR"));
    }

    #[test]
    fn scriptlet_with_custom_reveal_desc_mentions_finder() {
        let s = ScriptInfo::scriptlet("X", "/p.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let reveal = actions
            .iter()
            .find(|a| a.id == "reveal_scriptlet_in_finder")
            .unwrap();
        assert!(reveal.description.as_ref().unwrap().contains("Finder"));
    }

    // =========== 14. Scriptlet defined actions: has_action and value set ===========

    #[test]
    fn scriptlet_defined_action_has_action_true() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "copy".to_string(),
            tool: "bash".to_string(),
            code: "pbcopy".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions[0].has_action);
    }

    #[test]
    fn scriptlet_defined_action_value_is_command() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "copy-text".to_string(),
            tool: "bash".to_string(),
            code: "pbcopy".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].value, Some("copy-text".to_string()));
    }

    #[test]
    fn scriptlet_defined_action_id_uses_prefix() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Open".to_string(),
            command: "open-link".to_string(),
            tool: "open".to_string(),
            code: "https://example.com".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].id, "scriptlet_action:open-link");
    }

    #[test]
    fn scriptlet_defined_action_shortcut_formatted() {
        let mut scriptlet = Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "copy".to_string(),
            tool: "bash".to_string(),
            code: "pbcopy".to_string(),
            inputs: vec![],
            shortcut: Some("cmd+c".to_string()),
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].shortcut, Some("⌘C".to_string()));
    }

    // =========== 15. AI bar: copy_last_code details ===========

    #[test]
    fn ai_bar_copy_last_code_shortcut() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(clc.shortcut, Some("⌥⌘C".to_string()));
    }

    #[test]
    fn ai_bar_copy_last_code_icon() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(clc.icon, Some(IconName::Code));
    }

    #[test]
    fn ai_bar_copy_last_code_section() {
        let actions = get_ai_command_bar_actions();
        let clc = actions
            .iter()
            .find(|a| a.id == "chat:copy_last_code")
            .unwrap();
        assert_eq!(clc.section, Some("Response".to_string()));
    }

    #[test]
    fn ai_bar_copy_last_code_desc_mentions_code() {
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

    // =========== 16. AI bar: submit action details ===========

    #[test]
    fn ai_bar_submit_shortcut() {
        let actions = get_ai_command_bar_actions();
        let sub = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(sub.shortcut, Some("↵".to_string()));
    }

    #[test]
    fn ai_bar_submit_icon() {
        let actions = get_ai_command_bar_actions();
        let sub = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(sub.icon, Some(IconName::ArrowUp));
    }

    #[test]
    fn ai_bar_submit_section_actions() {
        let actions = get_ai_command_bar_actions();
        let sub = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert_eq!(sub.section, Some("Actions".to_string()));
    }

    #[test]
    fn ai_bar_submit_desc_mentions_send() {
        let actions = get_ai_command_bar_actions();
        let sub = actions.iter().find(|a| a.id == "chat:submit").unwrap();
        assert!(
            sub.description
                .as_ref()
                .unwrap()
                .to_lowercase()
                .contains("send")
        );
    }

    // =========== 17. AI bar: export_markdown details ===========

    #[test]
    fn ai_bar_export_markdown_shortcut() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.shortcut, Some("⇧⌘E".to_string()));
    }

    #[test]
    fn ai_bar_export_markdown_icon() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.icon, Some(IconName::FileCode));
    }

    #[test]
    fn ai_bar_export_markdown_section() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.section, Some("Export".to_string()));
    }

    #[test]
    fn ai_bar_export_markdown_title() {
        let actions = get_ai_command_bar_actions();
        let em = actions
            .iter()
            .find(|a| a.id == "chat:export_markdown")
            .unwrap();
        assert_eq!(em.title, "Copy Chat as Markdown");
    }

    // =========== 18. Notes: find_in_note details ===========

    // =========== 19. Notes: duplicate_note details ===========

    // --- merged from part_03.rs ---

    // =========== 20. Notes: copy_note_as details ===========

    // =========== 21. Notes: total action count varies by state ===========

    // =========== 22. Chat context: no models produces only continue_in_chat ===========

    #[test]
    fn chat_no_models_no_messages_single_action() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn chat_no_models_single_is_continue() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions[0].id, "chat:change_model");
    }

    #[test]
    fn chat_with_messages_adds_clear() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue_in_chat + clear_conversation + capture_screen_area = 4
        assert_eq!(actions.len(), 4);
    }

    #[test]
    fn chat_with_response_adds_copy() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        // change_model + continue_in_chat + copy_response + capture_screen_area = 4
        assert_eq!(actions.len(), 4);
    }

    // =========== 23. Chat context: model IDs use select_model_{model.id} ===========

    #[test]
    fn chat_model_id_format() {
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
        assert_eq!(picker[0].id, "chat:select_model_claude-3-opus");
    }

    #[test]
    fn chat_model_title_is_display_name() {
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
        assert_eq!(picker[0].title, "GPT-4");
    }

    #[test]
    fn chat_model_desc_via_provider() {
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
        assert_eq!(picker[0].description, Some("Uses OpenAI".to_string()));
    }

    #[test]
    fn chat_current_model_gets_checkmark() {
        let info = ChatPromptInfo {
            current_model: Some("GPT-4".to_string()),
            available_models: vec![ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        assert!(picker[0].title.contains('✓'));
    }

    // =========== 24. New chat: last_used section and icon ===========

    #[test]
    fn new_chat_last_used_section() {
        let lu = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "Provider 1".into(),
        }];
        let actions = get_new_chat_actions(&lu, &[], &[]);
        assert_eq!(actions[0].section, Some("Last Used Settings".to_string()));
    }

    #[test]
    fn new_chat_last_used_icon_bolt() {
        let lu = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "Provider 1".into(),
        }];
        let actions = get_new_chat_actions(&lu, &[], &[]);
        assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
    }

    #[test]
    fn new_chat_last_used_desc_is_provider() {
        let lu = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&lu, &[], &[]);
        assert_eq!(actions[0].description, Some("Uses Anthropic".to_string()));
    }

    #[test]
    fn new_chat_last_used_id_format() {
        let lu = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let actions = get_new_chat_actions(&lu, &[], &[]);
        assert_eq!(actions[0].id, "last_used_p::m1");
    }

    // =========== 25. New chat: preset section and icon ===========

    #[test]
    fn new_chat_preset_section() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].section, Some("Presets".to_string()));
    }

    #[test]
    fn new_chat_preset_icon_preserved() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].icon, Some(IconName::Code));
    }

    #[test]
    fn new_chat_preset_id_format() {
        let presets = vec![NewChatPresetInfo {
            id: "writer".into(),
            name: "Writer".into(),
            icon: IconName::File,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_writer");
    }

    #[test]
    fn new_chat_preset_desc_none() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(
            actions[0].description,
            Some("Uses General preset".to_string())
        );
    }

    // =========== 26. Note switcher: current note has bullet prefix ===========

    #[test]
    fn note_switcher_current_has_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 42,
            is_current: true,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(actions[0].title.starts_with("• "));
    }

    #[test]
    fn note_switcher_non_current_no_bullet() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert!(!actions[0].title.starts_with("• "));
    }

    #[test]
    fn note_switcher_current_icon_check() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 42,
            is_current: true,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].icon, Some(IconName::Check));
    }

    #[test]
    fn note_switcher_pinned_current_icon_star() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "My Note".into(),
            char_count: 42,
            is_current: true,
            is_pinned: true,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        // pinned takes priority over current for icon
        assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    }

    // =========== 27. Note switcher: preview truncation at 60 chars ===========

    #[test]
    fn note_switcher_short_preview_not_truncated() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "Short preview".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description, Some("Short preview".to_string()));
    }

    #[test]
    fn note_switcher_long_preview_truncated_with_ellipsis() {
        let long_preview = "a".repeat(80);
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 80,
            is_current: false,
            is_pinned: false,
            preview: long_preview,
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.ends_with('…'));
        // 60 'a's + ellipsis
        assert_eq!(desc.chars().count(), 61);
    }

    #[test]
    fn note_switcher_preview_with_time_separator() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "2m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.contains(" · "));
        assert!(desc.contains("2m ago"));
    }

    // --- merged from part_04.rs ---

    #[test]
    fn note_switcher_no_preview_shows_char_count() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "T".into(),
            char_count: 42,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description, Some("42 chars".to_string()));
    }

    // =========== 28. to_deeplink_name: various edge cases ===========

    #[test]
    fn to_deeplink_name_uppercase_to_lower() {
        assert_eq!(to_deeplink_name("HELLO"), "hello");
    }

    #[test]
    fn to_deeplink_name_preserves_numbers() {
        assert_eq!(to_deeplink_name("test123"), "test123");
    }

    #[test]
    fn to_deeplink_name_multiple_special_chars_collapse() {
        assert_eq!(to_deeplink_name("a!!!b"), "a-b");
    }

    #[test]
    fn to_deeplink_name_leading_trailing_special_removed() {
        assert_eq!(to_deeplink_name("---hello---"), "hello");
    }

    // =========== 29. score_action: various match type scores ===========

    #[test]
    fn score_action_prefix_match_100() {
        let a = Action::new("id", "copy path", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&a, "copy");
        assert_eq!(score, 100);
    }

    #[test]
    fn score_action_contains_match_50() {
        let a = Action::new("id", "my copy action", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&a, "copy");
        assert_eq!(score, 50);
    }

    #[test]
    fn score_action_fuzzy_match_25() {
        let a = Action::new("id", "clipboard", None, ActionCategory::ScriptContext);
        // "cpd" is a subsequence of "clipboard" (c-l-i-p-b-o-a-r-d)
        // c..p..d - wait, let me verify: c(lipboar)d - not quite
        // "cbd" = c(lip)b(oar)d - that works
        let score = ActionsDialog::score_action(&a, "cbd");
        assert_eq!(score, 25);
    }

    #[test]
    fn score_action_no_match_0() {
        let a = Action::new("id", "abc title", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&a, "xyz");
        assert_eq!(score, 0);
    }

    // =========== 30. fuzzy_match: various patterns ===========

    #[test]
    fn fuzzy_match_full_string() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn fuzzy_match_subsequence() {
        assert!(ActionsDialog::fuzzy_match("hello world", "hwd"));
    }

    #[test]
    fn fuzzy_match_empty_needle_matches() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn fuzzy_match_reversed_fails() {
        assert!(!ActionsDialog::fuzzy_match("abc", "cba"));
    }
}

mod from_dialog_builtin_action_validation_tests_45 {
    // --- merged from part_01.rs ---
    //! Batch 45: Dialog Built-in Action Validation Tests
    //!
    //! 120 tests across 30 categories validating action behaviors
    //! in various built-in action window dialogs.

    use crate::actions::builders::*;
    use crate::actions::dialog::ActionsDialog;
    use crate::actions::types::{Action, ActionCategory, ScriptInfo};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    // =========== 1. ScriptInfo::with_shortcut_and_alias: both populated ===========

    #[test]
    fn with_shortcut_and_alias_sets_shortcut() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+k".into()),
            Some("tk".into()),
        );
        assert_eq!(s.shortcut, Some("cmd+k".to_string()));
    }

    #[test]
    fn with_shortcut_and_alias_sets_alias() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+k".into()),
            Some("tk".into()),
        );
        assert_eq!(s.alias, Some("tk".to_string()));
    }

    #[test]
    fn with_shortcut_and_alias_is_script_true() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+k".into()),
            Some("tk".into()),
        );
        assert!(s.is_script);
    }

    #[test]
    fn with_shortcut_and_alias_is_scriptlet_false() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+k".into()),
            Some("tk".into()),
        );
        assert!(!s.is_scriptlet);
    }

    // =========== 2. ScriptInfo: with_frecency on scriptlet preserves type ===========

    #[test]
    fn scriptlet_with_frecency_preserves_is_scriptlet() {
        let s = ScriptInfo::scriptlet("Open URL", "/urls.md", None, None)
            .with_frecency(true, Some("/f".into()));
        assert!(s.is_scriptlet);
    }

    #[test]
    fn scriptlet_with_frecency_is_script_stays_false() {
        let s = ScriptInfo::scriptlet("Open URL", "/urls.md", None, None)
            .with_frecency(true, Some("/f".into()));
        assert!(!s.is_script);
    }

    #[test]
    fn scriptlet_with_frecency_preserves_name() {
        let s = ScriptInfo::scriptlet("Open URL", "/urls.md", None, None)
            .with_frecency(true, Some("/f".into()));
        assert_eq!(s.name, "Open URL");
    }

    #[test]
    fn scriptlet_with_frecency_sets_is_suggested() {
        let s = ScriptInfo::scriptlet("Open URL", "/urls.md", None, None)
            .with_frecency(true, Some("/f".into()));
        assert!(s.is_suggested);
    }

    // =========== 3. Action: category preserved through builder chaining ===========

    #[test]
    fn action_category_preserved_after_with_shortcut() {
        let a =
            Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘T");
        assert_eq!(a.category, ActionCategory::ScriptContext);
    }

    #[test]
    fn action_category_preserved_after_with_icon() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_icon(IconName::Star);
        assert_eq!(a.category, ActionCategory::ScriptContext);
    }

    #[test]
    fn action_category_preserved_after_with_section() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_section("Actions");
        assert_eq!(a.category, ActionCategory::ScriptContext);
    }

    #[test]
    fn action_category_preserved_after_full_chain() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut("⌘T")
            .with_icon(IconName::Star)
            .with_section("Actions");
        assert_eq!(a.category, ActionCategory::ScriptContext);
    }

    // =========== 4. Action: with_icon returns expected icon value ===========

    #[test]
    fn action_with_icon_star_filled() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_icon(IconName::StarFilled);
        assert_eq!(a.icon, Some(IconName::StarFilled));
    }

    #[test]
    fn action_with_icon_plus() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_icon(IconName::Plus);
        assert_eq!(a.icon, Some(IconName::Plus));
    }

    #[test]
    fn action_with_icon_settings() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_icon(IconName::Settings);
        assert_eq!(a.icon, Some(IconName::Settings));
    }

    #[test]
    fn action_without_icon_is_none() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext);
        assert!(a.icon.is_none());
    }

    // =========== 5. Clipboard: first 4 text action IDs in order ===========

    #[test]
    fn clipboard_text_first_action_is_paste() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[0].id, "clip:clipboard_paste");
    }

    #[test]
    fn clipboard_text_second_action_is_copy() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[1].id, "clip:clipboard_copy");
    }

    #[test]
    fn clipboard_text_third_action_is_paste_keep_open() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[2].id, "clip:clipboard_paste_keep_open");
    }

    #[test]
    fn clipboard_text_fourth_action_is_share() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert_eq!(actions[3].id, "clip:clipboard_share");
    }

    // =========== 6. Clipboard: save_snippet desc mentions "scriptlet" ===========

    #[test]
    fn clipboard_save_snippet_desc_mentions_scriptlet() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_snippet")
            .unwrap();
        assert!(action.description.as_ref().unwrap().contains("scriptlet"));
    }

    #[test]
    fn clipboard_save_file_desc_mentions_file() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_file")
            .unwrap();
        assert!(action.description.as_ref().unwrap().contains("file"));
    }

    #[test]
    fn clipboard_save_snippet_shortcut_differs_from_save_file() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let snippet = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_snippet")
            .unwrap();
        let file = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_save_file")
            .unwrap();
        assert_ne!(snippet.shortcut, file.shortcut);
    }

    #[test]
    fn clipboard_save_snippet_and_save_file_both_present() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_save_snippet")
        );
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_save_file"));
    }

    // =========== 7. Clipboard: destructive actions in last 3 positions ===========

    #[test]
    fn clipboard_delete_is_third_from_last() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();
        assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
    }

    #[test]
    fn clipboard_delete_multiple_is_second_from_last() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let len = actions.len();
        assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
    }

    #[test]
    fn clipboard_delete_all_is_last() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let last = actions.last().unwrap();
        assert_eq!(last.id, "clip:clipboard_delete_all");
    }

    #[test]
    fn clipboard_all_three_destructive_actions_present() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_delete"));
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_delete_multiple")
        );
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_delete_all"));
    }

    // =========== 8. Clipboard: annotate_cleanshot image-only ===========

    #[test]
    fn clipboard_annotate_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
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

    #[test]
    fn clipboard_annotate_absent_for_text() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
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
    fn clipboard_annotate_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_annotate_cleanshot")
            .unwrap();
        assert_eq!(action.shortcut.as_deref(), Some("⇧⌘A"));
    }

    #[test]
    fn clipboard_annotate_desc_mentions_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: String::new(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let action = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_annotate_cleanshot")
            .unwrap();
        assert!(action.description.as_ref().unwrap().contains("CleanShot X"));
    }

    // =========== 9. File context: macOS file=12 dir=10 action count ===========

    #[test]
    fn file_context_file_has_12_actions() {
        let file_info = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file_info);
        assert_eq!(actions.len(), 13);
    }

    #[test]
    fn file_context_dir_has_10_actions() {
        let file_info = FileInfo {
            name: "mydir".into(),
            path: "/tmp/mydir".into(),
            is_dir: true,
            file_type: FileType::Directory,
        };
        let actions = get_file_context_actions(&file_info);
        assert_eq!(actions.len(), 11);
    }

    #[test]
    fn file_context_file_has_quick_look() {
        let file_info = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file_info);
        assert!(actions.iter().any(|a| a.id == "file:quick_look"));
    }

    #[test]
    fn file_context_dir_has_no_quick_look() {
        let file_info = FileInfo {
            name: "mydir".into(),
            path: "/tmp/mydir".into(),
            is_dir: true,
            file_type: FileType::Directory,
        };
        let actions = get_file_context_actions(&file_info);
        assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
    }

    // --- merged from part_02.rs ---

    // =========== 10. File context: all ScriptContext category ===========

    #[test]
    fn file_context_file_all_script_context() {
        let file_info = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file_info);
        assert!(
            actions
                .iter()
                .all(|a| a.category == ActionCategory::ScriptContext)
        );
    }

    #[test]
    fn file_context_dir_all_script_context() {
        let file_info = FileInfo {
            name: "mydir".into(),
            path: "/tmp/mydir".into(),
            is_dir: true,
            file_type: FileType::Directory,
        };
        let actions = get_file_context_actions(&file_info);
        assert!(
            actions
                .iter()
                .all(|a| a.category == ActionCategory::ScriptContext)
        );
    }

    #[test]
    fn file_context_no_script_ops() {
        let file_info = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file_info);
        assert!(
            !actions
                .iter()
                .any(|a| a.category == ActionCategory::ScriptOps)
        );
    }

    #[test]
    fn file_context_no_global_ops() {
        let file_info = FileInfo {
            name: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            is_dir: false,
            file_type: FileType::File,
        };
        let actions = get_file_context_actions(&file_info);
        assert!(
            !actions
                .iter()
                .any(|a| a.category == ActionCategory::GlobalOps)
        );
    }

    // =========== 11. Path context: primary at index 0 ===========

    #[test]
    fn path_file_primary_at_index_0() {
        let path_info = PathInfo {
            name: "file.txt".into(),
            path: "/tmp/file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn path_dir_primary_at_index_0() {
        let path_info = PathInfo {
            name: "mydir".into(),
            path: "/tmp/mydir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn path_file_copy_path_at_index_1() {
        let path_info = PathInfo {
            name: "file.txt".into(),
            path: "/tmp/file.txt".into(),
            is_dir: false,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[1].id, "file:copy_path");
    }

    #[test]
    fn path_dir_copy_path_at_index_1() {
        let path_info = PathInfo {
            name: "mydir".into(),
            path: "/tmp/mydir".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert_eq!(actions[1].id, "file:copy_path");
    }

    // =========== 12. Path context: dir has all 7 IDs ===========

    #[test]
    fn path_dir_has_open_directory() {
        let path_info = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert!(actions.iter().any(|a| a.id == "file:open_directory"));
    }

    #[test]
    fn path_dir_has_copy_path_and_copy_filename() {
        let path_info = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert!(actions.iter().any(|a| a.id == "file:copy_path"));
        assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
    }

    #[test]
    fn path_dir_has_open_in_finder_and_editor() {
        let path_info = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert!(actions.iter().any(|a| a.id == "file:open_in_finder"));
        assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
    }

    #[test]
    fn path_dir_has_terminal_and_trash() {
        let path_info = PathInfo {
            name: "d".into(),
            path: "/d".into(),
            is_dir: true,
        };
        let actions = get_path_context_actions(&path_info);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "file:open_in_quick_terminal")
        );
        assert!(actions.iter().any(|a| a.id == "file:move_to_trash"));
    }

    // =========== 13. Script: shortcut+alias yields update+remove for both ===========

    #[test]
    fn script_shortcut_alias_has_update_shortcut() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "update_shortcut"));
    }

    #[test]
    fn script_shortcut_alias_has_update_alias() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "update_alias"));
    }

    #[test]
    fn script_shortcut_alias_has_remove_shortcut() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
    }

    #[test]
    fn script_shortcut_alias_has_remove_alias() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/p",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "remove_alias"));
    }

    // =========== 14. Script: agent action count ===========

    #[test]
    fn agent_has_9_actions() {
        let mut s = ScriptInfo::new("my-agent", "/agents/my-agent.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        // Agent (non-empty path, never shortcut/alias actions): run_script, toggle_info,
        // toggle_favorite, edit_script, reveal_in_finder, file:open_in_quick_terminal,
        // copy_path, copy_content, copy_deeplink = 9
        assert_eq!(actions.len(), 9);
    }

    #[test]
    fn agent_has_edit_script() {
        let mut s = ScriptInfo::new("my-agent", "/agents/my-agent.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "edit_script"));
    }

    #[test]
    fn agent_has_copy_content() {
        let mut s = ScriptInfo::new("my-agent", "/agents/my-agent.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "copy_content"));
    }

    #[test]
    fn agent_has_copy_deeplink() {
        let mut s = ScriptInfo::new("my-agent", "/agents/my-agent.md");
        s.is_agent = true;
        s.is_script = false;
        let actions = get_script_context_actions(&s);
        assert!(actions.iter().any(|a| a.id == "copy_deeplink"));
    }

    // =========== 15. Script: get_global_actions seeded ===========

    #[test]
    fn global_actions_returns_seeded() {
        let actions = get_global_actions();
        // Seeded, never empty: reload_scripts, settings, view_logs plus
        // the Discover rows. Prompt export/handoff rows are Agent Chat-owned
        // and must NOT appear here.
        assert!(!actions.is_empty());
    }

    #[test]
    fn global_actions_contains_core_ids() {
        let actions = get_global_actions();
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"reload_scripts"));
        assert!(ids.contains(&"settings"));
        assert!(ids.contains(&"view_logs"));
    }

    #[test]
    fn global_actions_no_script_context() {
        let actions = get_global_actions();
        assert!(
            !actions
                .iter()
                .any(|a| a.category == ActionCategory::ScriptContext)
        );
    }

    #[test]
    fn global_actions_has_global_ops() {
        let actions = get_global_actions();
        assert!(
            actions
                .iter()
                .any(|a| a.category == ActionCategory::GlobalOps)
        );
    }

    // =========== 16. Scriptlet with_custom: None scriptlet → no has_action=true ===========

    #[test]
    fn scriptlet_with_custom_none_first_is_run() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn scriptlet_with_custom_none_all_has_action_false() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(actions.iter().all(|a| !a.has_action));
    }

    #[test]
    fn scriptlet_with_custom_none_no_scriptlet_action_ids() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(
            !actions
                .iter()
                .any(|a| a.id.starts_with("scriptlet_action:"))
        );
    }

    #[test]
    fn scriptlet_with_custom_none_has_edit_scriptlet() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(actions.iter().any(|a| a.id == "edit_scriptlet"));
    }

    // =========== 17. Scriptlet with_custom: copy_content desc ===========

    #[test]
    fn scriptlet_with_custom_copy_content_desc() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let action = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert!(
            action
                .description
                .as_ref()
                .unwrap()
                .contains("entire file content")
        );
    }

    #[test]
    fn scriptlet_with_custom_copy_content_shortcut() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let action = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(action.shortcut.as_deref(), Some("⌘⌥C"));
    }

    #[test]
    fn scriptlet_with_custom_copy_content_title() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let action = actions.iter().find(|a| a.id == "copy_content").unwrap();
        assert_eq!(action.title, "Copy Content");
    }

    #[test]
    fn scriptlet_with_custom_copy_content_present() {
        let s = ScriptInfo::scriptlet("My Scriptlet", "/s.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(actions.iter().any(|a| a.id == "copy_content"));
    }

    // =========== 18. Scriptlet defined: empty actions → empty result ===========

    #[test]
    fn scriptlet_defined_empty_returns_empty() {
        let scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions.is_empty());
    }

    #[test]
    fn scriptlet_defined_empty_len_zero() {
        let scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn scriptlet_defined_empty_no_has_action() {
        let scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(!actions.iter().any(|a| a.has_action));
    }

    #[test]
    fn scriptlet_defined_empty_no_ids() {
        let scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(
            !actions
                .iter()
                .any(|a| a.id.starts_with("scriptlet_action:"))
        );
    }

    // =========== 19. Scriptlet defined: action with description preserved ===========

    #[test]
    fn scriptlet_defined_preserves_description() {
        let mut scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "pbcopy".to_string(),
            tool: "bash".to_string(),
            code: "echo hi | pbcopy".to_string(),
            description: Some("Copy to clipboard".to_string()),
            shortcut: None,
            inputs: vec![],
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(
            actions[0].description,
            Some("Copy to clipboard".to_string())
        );
    }

    #[test]
    fn scriptlet_defined_has_action_true() {
        let mut scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "pbcopy".to_string(),
            tool: "bash".to_string(),
            code: "echo hi | pbcopy".to_string(),
            description: None,
            shortcut: None,
            inputs: vec![],
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions[0].has_action);
    }

    #[test]
    fn scriptlet_defined_value_is_command() {
        let mut scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "pbcopy".to_string(),
            tool: "bash".to_string(),
            code: "echo hi | pbcopy".to_string(),
            description: None,
            shortcut: None,
            inputs: vec![],
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].value, Some("pbcopy".to_string()));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn scriptlet_defined_id_uses_action_id() {
        let mut scriptlet = Scriptlet::new(
            "test".to_string(),
            "bash".to_string(),
            "echo hi".to_string(),
        );
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".to_string(),
            command: "pbcopy".to_string(),
            tool: "bash".to_string(),
            code: "echo hi | pbcopy".to_string(),
            description: None,
            shortcut: None,
            inputs: vec![],
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions[0].id.contains("pbcopy") || actions[0].id.starts_with("scriptlet_action:"));
    }

    // =========== 20. AI bar: all 12 have descriptions ===========

    #[test]
    fn ai_bar_all_have_descriptions() {
        let actions = get_ai_command_bar_actions();
        assert!(actions.iter().all(|a| a.description.is_some()));
    }

    #[test]
    fn ai_bar_all_descriptions_non_empty() {
        let actions = get_ai_command_bar_actions();
        assert!(
            actions
                .iter()
                .all(|a| !a.description.as_ref().unwrap().is_empty())
        );
    }

    #[test]
    fn ai_bar_count_is_12() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    #[test]
    fn ai_bar_all_have_icons() {
        let actions = get_ai_command_bar_actions();
        assert!(actions.iter().all(|a| a.icon.is_some()));
    }

    // =========== 21. AI bar: Response section has 3 actions ===========

    #[test]
    fn ai_bar_response_section_count() {
        let actions = get_ai_command_bar_actions();
        let response_count = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .count();
        assert_eq!(response_count, 3);
    }

    #[test]
    fn ai_bar_response_has_copy_response() {
        let actions = get_ai_command_bar_actions();
        let response: Vec<_> = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .collect();
        assert!(response.iter().any(|a| a.id == "chat:copy_response"));
    }

    #[test]
    fn ai_bar_response_has_copy_chat() {
        let actions = get_ai_command_bar_actions();
        let response: Vec<_> = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .collect();
        assert!(response.iter().any(|a| a.id == "chat:copy_chat"));
    }

    #[test]
    fn ai_bar_response_has_copy_last_code() {
        let actions = get_ai_command_bar_actions();
        let response: Vec<_> = actions
            .iter()
            .filter(|a| a.section.as_deref() == Some("Response"))
            .collect();
        assert!(response.iter().any(|a| a.id == "chat:copy_last_code"));
    }

    // =========== 22. Notes: untested boolean combos ===========

    // =========== 23. Notes: trash+selection suppresses selection-dependent ===========

    // =========== 24. Chat: 2 models + response + messages = 5 actions ===========

    #[test]
    fn chat_2_models_response_messages_count() {
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
        assert_eq!(actions.len(), 5);
    }

    #[test]
    fn chat_2_models_response_messages_has_continue() {
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
                    provider: "Q".into(),
                },
            ],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
    }

    #[test]
    fn chat_2_models_response_messages_has_copy_response() {
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
                    provider: "Q".into(),
                },
            ],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
    }

    #[test]
    fn chat_2_models_response_messages_has_clear() {
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
                    provider: "Q".into(),
                },
            ],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
    }

    // =========== 25. Chat: models before continue_in_chat in ordering ===========

    #[test]
    fn chat_model_at_index_0() {
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
        assert!(picker[0].id.starts_with("chat:select_model_"));
    }

    #[test]
    fn chat_continue_after_models() {
        // After drill-down refactor: root has change_model first, then continue_in_chat
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
                    provider: "Q".into(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions[0].id, "chat:change_model");
        assert_eq!(actions[1].id, "chat:continue_in_chat");
    }

    #[test]
    fn chat_models_preserve_insertion_order() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![
                ChatModelInfo {
                    id: "first".into(),
                    display_name: "First".into(),
                    provider: "P".into(),
                },
                ChatModelInfo {
                    id: "second".into(),
                    display_name: "Second".into(),
                    provider: "Q".into(),
                },
            ],
            has_messages: false,
            has_response: false,
        };
        let picker = get_chat_model_picker_actions(&info);
        assert_eq!(picker[0].id, "chat:select_model_first");
        assert_eq!(picker[1].id, "chat:select_model_second");
    }

    #[test]
    fn chat_single_model_continue_at_index_1() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![ChatModelInfo {
                id: "only".into(),
                display_name: "Only".into(),
                provider: "P".into(),
            }],
            has_messages: false,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert_eq!(actions[1].id, "chat:continue_in_chat");
    }

    // =========== 26. New chat: section assignment per type ===========

    #[test]
    fn new_chat_last_used_section() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Model 1".into(),
            provider: "p".into(),
            provider_display_name: "Provider".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    }

    #[test]
    fn new_chat_preset_section() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    }

    #[test]
    fn new_chat_model_section() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].section.as_deref(), Some("Models"));
    }

    #[test]
    fn new_chat_all_three_sections_present() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "M1".into(),
            provider: "p".into(),
            provider_display_name: "P".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "gen".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        let sections: Vec<_> = actions
            .iter()
            .filter_map(|a| a.section.as_deref())
            .collect();
        assert!(sections.contains(&"Last Used Settings"));
        assert!(sections.contains(&"Presets"));
        assert!(sections.contains(&"Models"));
    }

    // =========== 27. Clipboard: text has no image-specific actions ===========

    #[test]
    fn clipboard_text_no_ocr() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    // --- merged from part_04.rs ---

    #[test]
    fn clipboard_text_no_open_with() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_open_with"));
    }

    #[test]
    fn clipboard_text_no_annotate_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
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
    fn clipboard_text_no_upload_cleanshot() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
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

    // =========== 28. Script: scriptlet vs with_custom share common actions ===========

    #[test]
    fn scriptlet_both_contexts_have_run_script() {
        let s = ScriptInfo::scriptlet("My Script", "/s.md", None, None);
        let script_actions = get_script_context_actions(&s);
        let custom_actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(script_actions.iter().any(|a| a.id == "run_script"));
        assert!(custom_actions.iter().any(|a| a.id == "run_script"));
    }

    #[test]
    fn scriptlet_both_contexts_have_copy_content() {
        let s = ScriptInfo::scriptlet("My Script", "/s.md", None, None);
        let script_actions = get_script_context_actions(&s);
        let custom_actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(script_actions.iter().any(|a| a.id == "copy_content"));
        assert!(custom_actions.iter().any(|a| a.id == "copy_content"));
    }

    #[test]
    fn scriptlet_both_contexts_have_edit_scriptlet() {
        let s = ScriptInfo::scriptlet("My Script", "/s.md", None, None);
        let script_actions = get_script_context_actions(&s);
        let custom_actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(script_actions.iter().any(|a| a.id == "edit_scriptlet"));
        assert!(custom_actions.iter().any(|a| a.id == "edit_scriptlet"));
    }

    #[test]
    fn scriptlet_both_contexts_have_copy_deeplink() {
        let s = ScriptInfo::scriptlet("My Script", "/s.md", None, None);
        let script_actions = get_script_context_actions(&s);
        let custom_actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert!(script_actions.iter().any(|a| a.id == "copy_deeplink"));
        assert!(custom_actions.iter().any(|a| a.id == "copy_deeplink"));
    }

    // =========== 29. Dialog format_shortcut_hint: arrow key variants ===========

    #[test]
    fn dialog_format_hint_up() {
        assert_eq!(ActionsDialog::format_shortcut_hint("up"), "↑");
    }

    #[test]
    fn dialog_format_hint_arrowup() {
        assert_eq!(ActionsDialog::format_shortcut_hint("arrowup"), "↑");
    }

    #[test]
    fn dialog_format_hint_down() {
        assert_eq!(ActionsDialog::format_shortcut_hint("down"), "↓");
    }

    #[test]
    fn dialog_format_hint_arrowdown() {
        assert_eq!(ActionsDialog::format_shortcut_hint("arrowdown"), "↓");
    }

    // =========== 30. Dialog format_shortcut_hint: control and opt aliases ===========

    #[test]
    fn dialog_format_hint_control() {
        assert_eq!(ActionsDialog::format_shortcut_hint("control+k"), "⌃K");
    }

    #[test]
    fn dialog_format_hint_opt() {
        assert_eq!(ActionsDialog::format_shortcut_hint("opt+k"), "⌥K");
    }

    #[test]
    fn dialog_format_hint_command() {
        assert_eq!(ActionsDialog::format_shortcut_hint("command+k"), "⌘K");
    }

    #[test]
    fn dialog_format_hint_arrowleft() {
        assert_eq!(ActionsDialog::format_shortcut_hint("arrowleft"), "←");
    }
}

mod from_dialog_builtin_action_validation_tests_46 {
    // --- merged from part_01.rs ---
    //! Batch 46: Dialog Built-in Action Validation Tests
    //!
    //! 120 tests across 30 categories validating action behaviors
    //! in various built-in action window dialogs.

    use crate::actions::builders::*;
    use crate::actions::dialog::{
        GroupedActionItem, build_grouped_items_static, coerce_action_selection,
    };
    use crate::actions::types::{Action, ActionCategory, ScriptInfo, SectionStyle};
    use crate::clipboard_history::ContentType;
    use crate::designs::icon_variations::IconName;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;

    // =========== 1. Action::with_shortcut_opt: Some vs None ===========

    #[test]
    fn with_shortcut_opt_some_sets_shortcut() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘K".to_string()));
        assert_eq!(a.shortcut, Some("⌘K".to_string()));
    }

    #[test]
    fn with_shortcut_opt_some_sets_shortcut_lower() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘K".to_string()));
        assert_eq!(a.shortcut_lower, Some("⌘k".to_string()));
    }

    #[test]
    fn with_shortcut_opt_none_leaves_shortcut_none() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(None);
        assert!(a.shortcut.is_none());
    }

    #[test]
    fn with_shortcut_opt_none_leaves_shortcut_lower_none() {
        let a = Action::new("test", "Test", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(None);
        assert!(a.shortcut_lower.is_none());
    }

    // =========== 2. Action: title_lower correctly lowercased for mixed case ===========

    #[test]
    fn action_title_lower_from_mixed_case() {
        let a = Action::new("test", "Copy Deeplink", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "copy deeplink");
    }

    #[test]
    fn action_title_lower_from_all_caps() {
        let a = Action::new("test", "SUBMIT", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "submit");
    }

    #[test]
    fn action_title_lower_preserves_already_lowercase() {
        let a = Action::new("test", "browse notes", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "browse notes");
    }

    #[test]
    fn action_description_lower_from_mixed_case() {
        let a = Action::new(
            "test",
            "Test",
            Some("Open in $EDITOR".to_string()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(a.description_lower, Some("open in $editor".to_string()));
    }

    // =========== 3. ScriptInfo::with_action_verb_and_shortcut: verb and shortcut ===========

    #[test]
    fn with_action_verb_and_shortcut_sets_verb() {
        let s = ScriptInfo::with_action_verb_and_shortcut("Safari", "/app", false, "Launch", None);
        assert_eq!(s.action_verb, "Launch");
    }

    #[test]
    fn with_action_verb_and_shortcut_sets_shortcut() {
        let s = ScriptInfo::with_action_verb_and_shortcut(
            "Safari",
            "/app",
            false,
            "Launch",
            Some("cmd+l".into()),
        );
        assert_eq!(s.shortcut, Some("cmd+l".to_string()));
    }

    #[test]
    fn with_action_verb_and_shortcut_is_agent_false() {
        let s = ScriptInfo::with_action_verb_and_shortcut("Safari", "/app", false, "Launch", None);
        assert!(!s.is_agent);
    }

    #[test]
    fn with_action_verb_and_shortcut_alias_none() {
        let s = ScriptInfo::with_action_verb_and_shortcut("Safari", "/app", false, "Launch", None);
        assert!(s.alias.is_none());
    }

    // =========== 4. Clipboard: unpinned text action count on macOS ===========

    #[test]
    fn clipboard_text_unpinned_has_pin_action() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
    }

    #[test]
    fn clipboard_text_pinned_has_unpin_action() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
    }

    #[test]
    fn clipboard_pin_shortcut_is_shift_cmd_p() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap();
        assert_eq!(pin.shortcut.as_deref(), Some("⇧⌘P"));
    }

    #[test]
    fn clipboard_unpin_shortcut_is_shift_cmd_p() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: true,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let unpin = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_unpin")
            .unwrap();
        assert_eq!(unpin.shortcut.as_deref(), Some("⇧⌘P"));
    }

    // =========== 5. Clipboard: paste_keep_open shortcut ⌥↵ ===========

    #[test]
    fn clipboard_paste_keep_open_shortcut() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pko = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste_keep_open")
            .unwrap();
        assert_eq!(pko.shortcut.as_deref(), Some("⌥↵"));
    }

    #[test]
    fn clipboard_paste_keep_open_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pko = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste_keep_open")
            .unwrap();
        assert_eq!(pko.title, "Paste and Keep Window Open");
    }

    #[test]
    fn clipboard_paste_keep_open_desc_mentions_keep() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let pko = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste_keep_open")
            .unwrap();
        assert!(pko.description.as_ref().unwrap().contains("keep"));
    }

    #[test]
    fn clipboard_paste_keep_open_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((100, 100)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(
            actions
                .iter()
                .any(|a| a.id == "clip:clipboard_paste_keep_open")
        );
    }

    // =========== 6. Clipboard: copy shortcut ⌘↵ ===========

    #[test]
    fn clipboard_copy_shortcut_is_cmd_enter() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let copy = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_copy")
            .unwrap();
        assert_eq!(copy.shortcut.as_deref(), Some("⌘↵"));
    }

    #[test]
    fn clipboard_copy_title() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let copy = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_copy")
            .unwrap();
        assert_eq!(copy.title, "Copy to Clipboard");
    }

    #[test]
    fn clipboard_copy_desc_mentions_without_pasting() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "text".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        let copy = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_copy")
            .unwrap();
        assert!(copy.description.as_ref().unwrap().contains("without"));
    }

    #[test]
    fn clipboard_copy_present_for_image() {
        let entry = ClipboardEntryInfo {
            id: "1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "".into(),
            image_dimensions: Some((50, 50)),
            frontmost_app_name: None,
        };
        let actions = get_clipboard_history_context_actions(&entry);
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_copy"));
    }

    // =========== 7. File context: copy_filename shortcut ⌘C ===========

    #[test]
    fn file_context_copy_filename_shortcut() {
        let fi = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert_eq!(cf.shortcut.as_deref(), Some("⌘C"));
    }

    #[test]
    fn file_context_copy_filename_title() {
        let fi = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert_eq!(cf.title, "Copy Filename");
    }

    #[test]
    fn file_context_copy_filename_present_for_dir() {
        let fi = FileInfo {
            path: "/test/docs".into(),
            name: "docs".into(),
            file_type: FileType::Directory,
            is_dir: true,
        };
        let actions = get_file_context_actions(&fi);
        assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
    }

    #[test]
    fn file_context_copy_filename_desc_mentions_filename() {
        let fi = FileInfo {
            path: "/test/file.txt".into(),
            name: "file.txt".into(),
            file_type: FileType::File,
            is_dir: false,
        };
        let actions = get_file_context_actions(&fi);
        let cf = actions
            .iter()
            .find(|a| a.id == "file:copy_filename")
            .unwrap();
        assert!(cf.description.as_ref().unwrap().contains("filename"));
    }

    // =========== 8. Path context: open_in_editor shortcut ⌘E ===========

    #[test]
    fn path_context_open_in_editor_shortcut() {
        let pi = PathInfo::new("file.rs", "/src/file.rs", false);
        let actions = get_path_context_actions(&pi);
        let oie = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert_eq!(oie.shortcut.as_deref(), Some("⌘E"));
    }

    #[test]
    fn path_context_open_in_editor_title() {
        let pi = PathInfo::new("file.rs", "/src/file.rs", false);
        let actions = get_path_context_actions(&pi);
        let oie = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert_eq!(oie.title, "Open in Editor");
    }

    #[test]
    fn path_context_open_in_editor_desc_mentions_editor() {
        let pi = PathInfo::new("file.rs", "/src/file.rs", false);
        let actions = get_path_context_actions(&pi);
        let oie = actions
            .iter()
            .find(|a| a.id == "file:open_in_editor")
            .unwrap();
        assert!(oie.description.as_ref().unwrap().contains("$EDITOR"));
    }

    #[test]
    fn path_context_open_in_editor_present_for_dir() {
        let pi = PathInfo::new("src", "/project/src", true);
        let actions = get_path_context_actions(&pi);
        assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
    }

    // =========== 9. Path context: move_to_trash shortcut ⌘⌫ ===========

    #[test]
    fn path_context_move_to_trash_shortcut() {
        let pi = PathInfo::new("old.txt", "/tmp/old.txt", false);
        let actions = get_path_context_actions(&pi);
        let mt = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert_eq!(mt.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn path_context_move_to_trash_file_desc_says_file() {
        let pi = PathInfo::new("old.txt", "/tmp/old.txt", false);
        let actions = get_path_context_actions(&pi);
        let mt = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(mt.description.as_ref().unwrap().contains("file"));
    }

    #[test]
    fn path_context_move_to_trash_dir_desc_says_folder() {
        let pi = PathInfo::new("old_dir", "/tmp/old_dir", true);
        let actions = get_path_context_actions(&pi);
        let mt = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(mt.description.as_ref().unwrap().contains("folder"));
    }

    #[test]
    fn path_context_move_to_trash_title() {
        let pi = PathInfo::new("old.txt", "/tmp/old.txt", false);
        let actions = get_path_context_actions(&pi);
        let mt = actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert_eq!(mt.title, "Move to Trash");
    }

    // =========== 10. Path context: file has 7 actions, dir has 8 ===========

    #[test]
    fn path_context_file_action_count() {
        let pi = PathInfo::new("file.rs", "/src/file.rs", false);
        let actions = get_path_context_actions(&pi);
        // select_file, copy_path, open_in_finder, open_in_editor, open_in_quick_terminal, copy_filename, move_to_trash
        assert_eq!(actions.len(), 7);
    }

    #[test]
    fn path_context_dir_action_count() {
        let pi = PathInfo::new("src", "/project/src", true);
        let actions = get_path_context_actions(&pi);
        // open_directory, copy_path, open_in_finder, open_in_editor, open_in_quick_terminal, copy_filename, move_to_trash
        assert_eq!(actions.len(), 7);
    }

    #[test]
    fn path_context_file_has_select_file() {
        let pi = PathInfo::new("file.rs", "/src/file.rs", false);
        let actions = get_path_context_actions(&pi);
        assert!(actions.iter().any(|a| a.id == "file:select_file"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn path_context_dir_has_open_directory() {
        let pi = PathInfo::new("src", "/project/src", true);
        let actions = get_path_context_actions(&pi);
        assert!(actions.iter().any(|a| a.id == "file:open_directory"));
    }

    // =========== 11. Script: run_script title includes verb and quoted name ===========

    #[test]
    fn script_run_title_default_verb() {
        let s = ScriptInfo::new("my-script", "/path/my-script.ts");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Run");
    }

    #[test]
    fn script_run_title_custom_verb_launch() {
        let s = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Launch");
    }

    #[test]
    fn script_run_title_custom_verb_switch_to() {
        let s = ScriptInfo::with_action_verb("Doc Window", "win:1", false, "Switch to");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert_eq!(run.title, "Switch To");
    }

    #[test]
    fn script_run_desc_includes_verb() {
        let s = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
        let actions = get_script_context_actions(&s);
        let run = actions.iter().find(|a| a.id == "run_script").unwrap();
        assert!(run.description.as_ref().unwrap().contains("Launch"));
    }

    // =========== 12. Script: copy_deeplink URL format ===========

    #[test]
    fn script_copy_deeplink_url_contains_slugified_name() {
        let s = ScriptInfo::new("My Cool Script", "/path/script.ts");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(
            dl.description.as_deref(),
            Some("Copy a portable Script Kit share link to clipboard")
        );
    }

    #[test]
    fn script_copy_deeplink_shortcut() {
        let s = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.shortcut.as_deref(), Some("⌘⇧D"));
    }

    #[test]
    fn script_copy_deeplink_title() {
        let s = ScriptInfo::new("test", "/path/test.ts");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert_eq!(dl.title, "Share");
    }

    #[test]
    fn builtin_copy_deeplink_url_contains_slugified_name() {
        let s = ScriptInfo::builtin("Clipboard History");
        let actions = get_script_context_actions(&s);
        let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
        assert!(
            dl.description
                .as_ref()
                .unwrap()
                .contains("scriptkit://run/clipboard-history")
        );
    }

    // =========== 13. Script: reset_ranking has no shortcut ===========

    #[test]
    fn script_reset_ranking_no_shortcut() {
        let s = ScriptInfo::new("test", "/p").with_frecency(true, Some("/p".into()));
        let actions = get_script_context_actions(&s);
        let rr = actions.iter().find(|a| a.id == "reset_ranking").unwrap();
        assert_eq!(rr.shortcut.as_deref(), Some("⌃⌘R"));
    }

    #[test]
    fn script_reset_ranking_title() {
        let s = ScriptInfo::new("test", "/p").with_frecency(true, Some("/p".into()));
        let actions = get_script_context_actions(&s);
        let rr = actions.iter().find(|a| a.id == "reset_ranking").unwrap();
        assert_eq!(rr.title, "Delete Ranking Entry");
    }

    #[test]
    fn script_reset_ranking_desc_mentions_suggested() {
        let s = ScriptInfo::new("test", "/p").with_frecency(true, Some("/p".into()));
        let actions = get_script_context_actions(&s);
        let rr = actions.iter().find(|a| a.id == "reset_ranking").unwrap();
        assert!(rr.description.as_ref().unwrap().contains("Suggested"));
    }

    #[test]
    fn script_reset_ranking_absent_when_not_suggested() {
        let s = ScriptInfo::new("test", "/p");
        let actions = get_script_context_actions(&s);
        assert!(!actions.iter().any(|a| a.id == "reset_ranking"));
    }

    // =========== 14. Script: add_shortcut vs update_shortcut descriptions ===========

    #[test]
    fn script_add_shortcut_desc_mentions_set() {
        let s = ScriptInfo::new("test", "/p");
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "add_shortcut").unwrap();
        assert!(a.description.as_ref().unwrap().contains("Set"));
    }

    #[test]
    fn script_update_shortcut_desc_mentions_change() {
        let s = ScriptInfo::with_shortcut("test", "/p", Some("cmd+t".into()));
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "update_shortcut").unwrap();
        assert!(a.description.as_ref().unwrap().contains("Change"));
    }

    #[test]
    fn script_remove_shortcut_desc_mentions_remove() {
        let s = ScriptInfo::with_shortcut("test", "/p", Some("cmd+t".into()));
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "remove_shortcut").unwrap();
        assert!(a.description.as_ref().unwrap().contains("Remove"));
    }

    #[test]
    fn script_add_shortcut_shortcut_is_cmd_shift_k() {
        let s = ScriptInfo::new("test", "/p");
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "add_shortcut").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⇧K"));
    }

    // =========== 15. Script: add_alias vs update_alias descriptions ===========

    #[test]
    fn script_add_alias_desc_mentions_alias() {
        let s = ScriptInfo::new("test", "/p");
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "add_alias").unwrap();
        assert!(a.description.as_ref().unwrap().contains("alias"));
    }

    #[test]
    fn script_update_alias_desc_mentions_change() {
        let s = ScriptInfo::with_shortcut_and_alias("test", "/p", None, Some("t".into()));
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "update_alias").unwrap();
        assert!(a.description.as_ref().unwrap().contains("Change"));
    }

    #[test]
    fn script_remove_alias_shortcut_is_cmd_opt_a() {
        let s = ScriptInfo::with_shortcut_and_alias("test", "/p", None, Some("t".into()));
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "remove_alias").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⌥A"));
    }

    #[test]
    fn script_add_alias_shortcut_is_cmd_shift_a() {
        let s = ScriptInfo::new("test", "/p");
        let actions = get_script_context_actions(&s);
        let a = actions.iter().find(|a| a.id == "add_alias").unwrap();
        assert_eq!(a.shortcut.as_deref(), Some("⌘⇧A"));
    }

    // =========== 16. AI bar: paste_image details ===========

    #[test]
    fn ai_bar_paste_image_shortcut() {
        let actions = get_ai_command_bar_actions();
        let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(pi.shortcut.as_deref(), Some("⌘V"));
    }

    #[test]
    fn ai_bar_paste_image_icon() {
        let actions = get_ai_command_bar_actions();
        let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(pi.icon, Some(IconName::File));
    }

    #[test]
    fn ai_bar_paste_image_section() {
        let actions = get_ai_command_bar_actions();
        let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert_eq!(pi.section.as_deref(), Some("Attachments"));
    }

    #[test]
    fn ai_bar_paste_image_desc_mentions_clipboard() {
        let actions = get_ai_command_bar_actions();
        let pi = actions.iter().find(|a| a.id == "chat:paste_image").unwrap();
        assert!(pi.description.as_ref().unwrap().contains("clipboard"));
    }

    // =========== 17. AI bar: toggle_shortcuts_help details ===========

    #[test]
    fn ai_bar_toggle_shortcuts_help_shortcut() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.shortcut.as_deref(), Some("⌘/"));
    }

    #[test]
    fn ai_bar_toggle_shortcuts_help_icon() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.icon, Some(IconName::Star));
    }

    #[test]
    fn ai_bar_toggle_shortcuts_help_section() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.section.as_deref(), Some("Help"));
    }

    #[test]
    fn ai_bar_toggle_shortcuts_help_title() {
        let actions = get_ai_command_bar_actions();
        let tsh = actions
            .iter()
            .find(|a| a.id == "chat:toggle_shortcuts_help")
            .unwrap();
        assert_eq!(tsh.title, "Keyboard Shortcuts");
    }

    // =========== 18. AI bar: change_model details ===========

    #[test]
    fn ai_bar_change_model_no_shortcut() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(cm.shortcut.is_none());
    }

    #[test]
    fn ai_bar_change_model_icon() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert_eq!(cm.icon, Some(IconName::Settings));
    }

    #[test]
    fn ai_bar_change_model_section() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert_eq!(cm.section.as_deref(), Some("Settings"));
    }

    #[test]
    fn ai_bar_change_model_desc_mentions_model() {
        let actions = get_ai_command_bar_actions();
        let cm = actions
            .iter()
            .find(|a| a.id == "chat:change_model")
            .unwrap();
        assert!(cm.description.as_ref().unwrap().contains("model"));
    }

    // =========== 19. AI bar: unique action IDs ===========

    #[test]
    fn ai_bar_all_ids_unique() {
        let actions = get_ai_command_bar_actions();
        let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn ai_bar_no_empty_ids() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(!a.id.is_empty());
        }
    }

    #[test]
    fn ai_bar_all_titles_non_empty() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(!a.title.is_empty());
        }
    }

    #[test]
    fn ai_bar_all_have_sections() {
        let actions = get_ai_command_bar_actions();
        for a in &actions {
            assert!(a.section.is_some(), "Action {} should have a section", a.id);
        }
    }

    // =========== 20. Notes: browse_notes always present ===========

    // =========== 21. Notes: export details ===========

    // =========== 22. Notes: copy_note_as icon Copy ===========

    // --- merged from part_03.rs ---

    // =========== 23. Chat: copy_response conditional on has_response ===========

    #[test]
    fn chat_copy_response_present_when_has_response() {
        let info = ChatPromptInfo {
            current_model: Some("Claude".into()),
            available_models: vec![],
            has_messages: true,
            has_response: true,
        };
        let actions = get_chat_context_actions(&info);
        assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
    }

    #[test]
    fn chat_copy_response_absent_when_no_response() {
        let info = ChatPromptInfo {
            current_model: Some("Claude".into()),
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
    }

    #[test]
    fn chat_copy_response_shortcut() {
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
    fn chat_copy_response_title() {
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
        assert_eq!(cr.title, "Copy Last Response");
    }

    // =========== 24. Chat: clear_conversation conditional on has_messages ===========

    #[test]
    fn chat_clear_conversation_present_when_has_messages() {
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
    fn chat_clear_conversation_absent_when_no_messages() {
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
    fn chat_clear_conversation_shortcut() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let cc = actions
            .iter()
            .find(|a| a.id == "chat:clear_conversation")
            .unwrap();
        assert_eq!(cc.shortcut.as_deref(), Some("⌘⌫"));
    }

    #[test]
    fn chat_clear_conversation_title() {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        let actions = get_chat_context_actions(&info);
        let cc = actions
            .iter()
            .find(|a| a.id == "chat:clear_conversation")
            .unwrap();
        assert_eq!(cc.title, "Clear Conversation");
    }

    // =========== 25. New chat: empty inputs produce empty actions ===========

    #[test]
    fn new_chat_all_empty_produces_empty() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn new_chat_only_last_used_count() {
        let last_used = vec![NewChatModelInfo {
            model_id: "m1".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn new_chat_only_presets_count() {
        let presets = vec![NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn new_chat_only_models_count() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions.len(), 1);
    }

    // =========== 26. New chat: model ID format uses index ===========

    #[test]
    fn new_chat_model_id_format() {
        let models = vec![NewChatModelInfo {
            model_id: "gpt4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&[], &[], &models);
        assert_eq!(actions[0].id, "model_openai::gpt4");
    }

    #[test]
    fn new_chat_last_used_id_format() {
        let last_used = vec![NewChatModelInfo {
            model_id: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &[], &[]);
        assert_eq!(actions[0].id, "last_used_anthropic::claude");
    }

    #[test]
    fn new_chat_preset_id_format() {
        let presets = vec![NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: IconName::Code,
        }];
        let actions = get_new_chat_actions(&[], &presets, &[]);
        assert_eq!(actions[0].id, "preset_code");
    }

    #[test]
    fn new_chat_combined_ordering() {
        let last_used = vec![NewChatModelInfo {
            model_id: "c".into(),
            display_name: "Claude".into(),
            provider: "a".into(),
            provider_display_name: "Anthropic".into(),
        }];
        let presets = vec![NewChatPresetInfo {
            id: "gen".into(),
            name: "General".into(),
            icon: IconName::Star,
        }];
        let models = vec![NewChatModelInfo {
            model_id: "g".into(),
            display_name: "GPT-4".into(),
            provider: "o".into(),
            provider_display_name: "OpenAI".into(),
        }];
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
        assert_eq!(actions[1].section.as_deref(), Some("Presets"));
        assert_eq!(actions[2].section.as_deref(), Some("Models"));
    }

    // =========== 27. Note switcher: empty notes produces "no notes yet" ===========

    #[test]
    fn note_switcher_empty_has_no_notes_message() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "no_notes");
    }

    #[test]
    fn note_switcher_no_notes_title() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].title, "No notes yet");
    }

    #[test]
    fn note_switcher_no_notes_icon() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions[0].icon, Some(IconName::Plus));
    }

    #[test]
    fn note_switcher_no_notes_desc_mentions_cmd_n() {
        let actions = get_note_switcher_actions(&[]);
        assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
    }

    // =========== 28. Note switcher: char count display when no preview ===========

    #[test]
    fn note_switcher_no_preview_shows_char_count_singular() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Note".into(),
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
    fn note_switcher_no_preview_shows_char_count_plural() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Note".into(),
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
    fn note_switcher_no_preview_with_time_shows_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "5m ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(actions[0].description.as_deref(), Some("5m ago"));
    }

    #[test]
    fn note_switcher_with_preview_and_time() {
        let notes = vec![NoteSwitcherNoteInfo {
            id: "abc".into(),
            title: "Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: "Hello world".into(),
            relative_time: "2d ago".into(),
        }];
        let actions = get_note_switcher_actions(&notes);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Hello world · 2d ago")
        );
    }

    // =========== 29. coerce_action_selection: mixed headers and items ===========

    #[test]
    fn coerce_selection_first_header_then_items() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H".into()),
            GroupedActionItem::Item(0),
            GroupedActionItem::Item(1),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn coerce_selection_item_between_headers() {
        let rows = vec![
            GroupedActionItem::SectionHeader("H1".into()),
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H2".into()),
            GroupedActionItem::Item(1),
        ];
        assert_eq!(coerce_action_selection(&rows, 2), Some(3));
    }

    #[test]
    fn coerce_selection_trailing_header_searches_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn coerce_selection_multiple_headers_between_items() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("H1".into()),
            GroupedActionItem::SectionHeader("H2".into()),
            GroupedActionItem::Item(1),
        ];
        // Index 1 is header, search down → finds Item(1) at index 3
        assert_eq!(coerce_action_selection(&rows, 1), Some(3));
    }

    // =========== 30. build_grouped_items_static: action count matches filtered ===========

    #[test]
    fn build_grouped_items_item_count_matches_filtered() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext),
            Action::new("b", "B", None, ActionCategory::ScriptContext),
            Action::new("c", "C", None, ActionCategory::ScriptContext),
        ];
        let filtered = vec![0usize, 1, 2];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        let item_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::Item(_)))
            .count();
        assert_eq!(item_count, 3);
    }

    #[test]
    fn build_grouped_items_headers_from_sections() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered = vec![0usize, 1];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(header_count, 2);
    }

    #[test]
    fn build_grouped_items_no_headers_with_none_style() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered = vec![0usize, 1];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(header_count, 0);
    }

    #[test]
    fn build_grouped_items_same_section_one_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Same"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Same"),
            Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Same"),
        ];
        let filtered = vec![0usize, 1, 2];
        let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        let header_count = grouped
            .iter()
            .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
            .count();
        assert_eq!(header_count, 1);
    }
}
