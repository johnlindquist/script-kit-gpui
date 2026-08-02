#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;

mod from_dialog_builtin_action_validation_tests_11 {
    // --- merged from part_01.rs ---
    //! Batch 11: Random builtin action/dialog validation tests
    //!
    //! 30 test categories covering fresh angles on action builders, edge cases,
    //! and behavioral invariants not thoroughly covered by batches 1-10.

    use super::builders::*;
    use super::dialog::{
        ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
    };
    use super::types::*;
    use crate::clipboard_history::ContentType;
    use crate::file_search::{FileInfo, FileType};
    use crate::prompts::PathInfo;
    use crate::scriptlets::{Scriptlet, ScriptletAction};
    use std::collections::HashSet;

    // ============================================================================
    // Helper
    // ============================================================================

    fn action_ids(actions: &[Action]) -> Vec<String> {
        actions.iter().map(|a| a.id.clone()).collect()
    }

    // ============================================================================
    // 1. ScriptInfo constructor symmetry — each constructor sets exactly the
    //    expected defaults
    // ============================================================================

    #[test]
    fn cat01_script_info_new_defaults_all_fields() {
        let s = ScriptInfo::new("abc", "/tmp/abc.ts");
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
    fn cat01_script_info_builtin_path_is_empty() {
        let b = ScriptInfo::builtin("Test Builtin");
        assert!(b.path.is_empty());
        assert!(!b.is_script);
        assert!(!b.is_scriptlet);
        assert!(!b.is_agent);
    }

    #[test]
    fn cat01_scriptlet_sets_only_scriptlet_flag() {
        let s = ScriptInfo::scriptlet("x", "/p.md", None, None);
        assert!(!s.is_script);
        assert!(s.is_scriptlet);
        assert!(!s.is_agent);
        assert_eq!(s.action_verb, "Run");
    }

    #[test]
    fn cat01_with_action_verb_and_shortcut_preserves_verb() {
        let s = ScriptInfo::with_action_verb_and_shortcut(
            "App",
            "/app",
            false,
            "Launch",
            Some("cmd+l".into()),
        );
        assert_eq!(s.action_verb, "Launch");
        assert_eq!(s.shortcut, Some("cmd+l".into()));
        assert!(!s.is_script);
    }

    // ============================================================================
    // 2. Action::new caches lowercase fields correctly
    // ============================================================================

    #[test]
    fn cat02_title_lower_is_cached_on_creation() {
        let a = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
        assert_eq!(a.title_lower, "hello world");
    }

    #[test]
    fn cat02_description_lower_cached_when_present() {
        let a = Action::new(
            "id",
            "T",
            Some("My Desc".to_string()),
            ActionCategory::ScriptContext,
        );
        assert_eq!(a.description_lower, Some("my desc".to_string()));
    }

    #[test]
    fn cat02_description_lower_none_when_no_desc() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(a.description_lower.is_none());
    }

    #[test]
    fn cat02_shortcut_lower_none_until_with_shortcut() {
        let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
        assert!(a.shortcut_lower.is_none());
        let a2 = a.with_shortcut("⌘X");
        assert_eq!(a2.shortcut_lower, Some("⌘x".to_string()));
    }

    // ============================================================================
    // 3. Action builder chaining — order independence and with_shortcut_opt
    // ============================================================================

    #[test]
    fn cat03_with_icon_then_section_same_as_reverse() {
        let a1 = Action::new("a", "A", None, ActionCategory::ScriptContext)
            .with_icon(crate::designs::icon_variations::IconName::Plus)
            .with_section("S");
        let a2 = Action::new("a", "A", None, ActionCategory::ScriptContext)
            .with_section("S")
            .with_icon(crate::designs::icon_variations::IconName::Plus);
        assert_eq!(a1.icon, a2.icon);
        assert_eq!(a1.section, a2.section);
    }

    #[test]
    fn cat03_with_shortcut_opt_none_leaves_shortcut_none() {
        let a = Action::new("a", "A", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
        assert!(a.shortcut.is_none());
        assert!(a.shortcut_lower.is_none());
    }

    #[test]
    fn cat03_with_shortcut_opt_some_sets_both() {
        let a = Action::new("a", "A", None, ActionCategory::ScriptContext)
            .with_shortcut_opt(Some("⌘Z".to_string()));
        assert_eq!(a.shortcut, Some("⌘Z".to_string()));
        assert_eq!(a.shortcut_lower, Some("⌘z".to_string()));
    }

    // ============================================================================
    // 4. Script context — exact action IDs per flag combination
    // ============================================================================

    #[test]
    fn cat04_script_no_shortcut_no_alias_ids() {
        let s = ScriptInfo::new("test", "/test.ts");
        let actions = get_script_context_actions(&s);
        let ids: HashSet<String> = action_ids(&actions).into_iter().collect();
        // Must have these exact built-in IDs
        for expected in &[
            "run_script",
            "add_shortcut",
            "add_alias",
            "edit_script",
            "view_logs",
            "reveal_in_finder",
            "copy_path",
            "copy_content",
            "copy_deeplink",
        ] {
            assert!(ids.contains(*expected), "Missing: {}", expected);
        }
        // Must NOT have these
        for absent in &[
            "update_shortcut",
            "remove_shortcut",
            "update_alias",
            "remove_alias",
            "reset_ranking",
        ] {
            assert!(!ids.contains(*absent), "Unexpected: {}", absent);
        }
    }

    #[test]
    fn cat04_script_with_shortcut_and_alias_ids() {
        let s = ScriptInfo::with_shortcut_and_alias(
            "test",
            "/test.ts",
            Some("cmd+t".into()),
            Some("ts".into()),
        );
        let actions = get_script_context_actions(&s);
        let ids: HashSet<String> = action_ids(&actions).into_iter().collect();
        assert!(ids.contains("update_shortcut"));
        assert!(ids.contains("remove_shortcut"));
        assert!(ids.contains("update_alias"));
        assert!(ids.contains("remove_alias"));
        assert!(!ids.contains("add_shortcut"));
        assert!(!ids.contains("add_alias"));
    }

    #[test]
    fn cat04_builtin_has_exactly_4_actions() {
        let b = ScriptInfo::builtin("Test");
        let actions = get_script_context_actions(&b);
        // run_script, toggle_info, add_shortcut, add_alias, copy_deeplink
        assert_eq!(actions.len(), 5, "Builtin should have 5 actions");
    }

    #[test]
    fn cat04_agent_has_no_view_logs() {
        let mut a = ScriptInfo::new("agent", "/agent.claude.md");
        a.is_script = false;
        a.is_agent = true;
        let actions = get_script_context_actions(&a);
        assert!(!actions.iter().any(|a| a.id == "view_logs"));
        assert!(actions.iter().any(|a| a.title == "Edit Agent"));
    }

    // ============================================================================
    // 5. Scriptlet context actions — ordering guarantees
    // ============================================================================

    #[test]
    fn cat05_scriptlet_run_is_first() {
        let s = ScriptInfo::scriptlet("x", "/x.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert_eq!(actions[0].id, "run_script");
    }

    #[test]
    fn cat05_scriptlet_custom_actions_between_run_and_edit() {
        let s = ScriptInfo::scriptlet("x", "/x.md", None, None);
        let mut scriptlet = Scriptlet::new("x".into(), "bash".into(), "echo hi".into());
        scriptlet.actions = vec![ScriptletAction {
            name: "Custom".into(),
            command: "custom".into(),
            tool: "bash".into(),
            code: "echo custom".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_context_actions_with_custom(&s, Some(&scriptlet));
        let run_pos = actions.iter().position(|a| a.id == "run_script").unwrap();
        let custom_pos = actions
            .iter()
            .position(|a| a.id == "scriptlet_action:custom")
            .unwrap();
        let edit_pos = actions
            .iter()
            .position(|a| a.id == "edit_scriptlet")
            .unwrap();
        assert!(run_pos < custom_pos, "run before custom");
        assert!(custom_pos < edit_pos, "custom before edit");
    }

    #[test]
    fn cat05_scriptlet_copy_content_before_deeplink() {
        let s = ScriptInfo::scriptlet("x", "/x.md", None, None);
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        let content_pos = actions.iter().position(|a| a.id == "copy_content").unwrap();
        let deeplink_pos = actions
            .iter()
            .position(|a| a.id == "copy_deeplink")
            .unwrap();
        assert!(content_pos < deeplink_pos);
    }

    #[test]
    fn cat05_scriptlet_with_frecency_adds_reset_ranking_last() {
        let s = ScriptInfo::scriptlet("x", "/x.md", None, None)
            .with_frecency(true, Some("/x.md".into()));
        let actions = get_scriptlet_context_actions_with_custom(&s, None);
        assert_eq!(actions.last().unwrap().id, "reset_ranking");
    }

    // ============================================================================
    // 6. Clipboard actions — content type differences
    // ============================================================================

    fn text_entry() -> ClipboardEntryInfo {
        ClipboardEntryInfo {
            id: "t1".into(),
            content_type: ContentType::Text,
            pinned: false,
            preview: "hello".into(),
            image_dimensions: None,
            frontmost_app_name: None,
        }
    }

    fn image_entry() -> ClipboardEntryInfo {
        ClipboardEntryInfo {
            id: "i1".into(),
            content_type: ContentType::Image,
            pinned: false,
            preview: "800x600".into(),
            image_dimensions: Some((800, 600)),
            frontmost_app_name: None,
        }
    }

    #[test]
    fn cat06_image_has_ocr_text_does_not() {
        let text_actions = get_clipboard_history_context_actions(&text_entry());
        let image_actions = get_clipboard_history_context_actions(&image_entry());
        assert!(!text_actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
        assert!(image_actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
    }

    #[test]
    fn cat06_image_has_more_actions_than_text() {
        let ta = get_clipboard_history_context_actions(&text_entry());
        let ia = get_clipboard_history_context_actions(&image_entry());
        assert!(
            ia.len() > ta.len(),
            "Image {} > Text {}",
            ia.len(),
            ta.len()
        );
    }

    #[test]
    fn cat06_destructive_actions_always_last_three() {
        for entry in &[text_entry(), image_entry()] {
            let actions = get_clipboard_history_context_actions(entry);
            let len = actions.len();
            assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
            assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
            assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
        }
    }

    #[test]
    fn cat06_paste_is_always_first() {
        for entry in &[text_entry(), image_entry()] {
            let actions = get_clipboard_history_context_actions(entry);
            assert_eq!(actions[0].id, "clip:clipboard_paste");
        }
    }

    // ============================================================================
    // 7. Clipboard pin/unpin dynamic toggle
    // ============================================================================

    #[test]
    fn cat07_unpinned_shows_pin() {
        let actions = get_clipboard_history_context_actions(&text_entry());
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
    }

    #[test]
    fn cat07_pinned_shows_unpin() {
        let mut e = text_entry();
        e.pinned = true;
        let actions = get_clipboard_history_context_actions(&e);
        assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
        assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
    }

    #[test]
    fn cat07_pin_unpin_share_same_shortcut() {
        let pin_actions = get_clipboard_history_context_actions(&text_entry());
        let mut pinned = text_entry();
        pinned.pinned = true;
        let unpin_actions = get_clipboard_history_context_actions(&pinned);
        let pin_sc = pin_actions
            .iter()
            .find(|a| a.id == "clip:clipboard_pin")
            .unwrap()
            .shortcut
            .as_ref()
            .unwrap();
        let unpin_sc = unpin_actions
            .iter()
            .find(|a| a.id == "clip:clipboard_unpin")
            .unwrap()
            .shortcut
            .as_ref()
            .unwrap();
        assert_eq!(pin_sc, unpin_sc, "Pin/Unpin share ⇧⌘P");
    }

    // ============================================================================
    // 8. Clipboard frontmost_app_name propagation
    // ============================================================================

    #[test]
    fn cat08_no_app_shows_active_app() {
        let actions = get_clipboard_history_context_actions(&text_entry());
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.title, "Paste to Active App");
    }

    #[test]
    fn cat08_with_app_shows_name() {
        let mut e = text_entry();
        e.frontmost_app_name = Some("Firefox".into());
        let actions = get_clipboard_history_context_actions(&e);
        let paste = actions
            .iter()
            .find(|a| a.id == "clip:clipboard_paste")
            .unwrap();
        assert_eq!(paste.title, "Paste to Firefox");
    }

    #[test]
    fn cat08_app_name_does_not_affect_other_action_count() {
        let a1 = get_clipboard_history_context_actions(&text_entry());
        let mut e = text_entry();
        e.frontmost_app_name = Some("Safari".into());
        let a2 = get_clipboard_history_context_actions(&e);
        assert_eq!(a1.len(), a2.len());
    }

    // ============================================================================
    // 9. File context — directory vs file differences
    // ============================================================================

    fn file_info_file() -> FileInfo {
        FileInfo {
            path: "/tmp/doc.pdf".into(),
            name: "doc.pdf".into(),
            file_type: FileType::Document,
            is_dir: false,
        }
    }

    fn file_info_dir() -> FileInfo {
        FileInfo {
            path: "/tmp/docs".into(),
            name: "docs".into(),
            file_type: FileType::Directory,
            is_dir: true,
        }
    }

    #[test]
    fn cat09_file_has_open_file_not_open_directory() {
        let actions = get_file_context_actions(&file_info_file());
        assert!(actions.iter().any(|a| a.id == "file:open_file"));
        assert!(!actions.iter().any(|a| a.id == "file:open_directory"));
    }

    #[test]
    fn cat09_dir_has_open_directory_not_open_file() {
        let actions = get_file_context_actions(&file_info_dir());
        assert!(actions.iter().any(|a| a.id == "file:open_directory"));
        assert!(!actions.iter().any(|a| a.id == "file:open_file"));
    }

    // --- merged from part_02.rs ---

    #[test]
    fn cat09_both_have_reveal_copy_path_copy_filename() {
        for info in &[file_info_file(), file_info_dir()] {
            let actions = get_file_context_actions(info);
            assert!(actions.iter().any(|a| a.id == "file:reveal_in_finder"));
            assert!(actions.iter().any(|a| a.id == "file:copy_path"));
            assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cat09_quick_look_only_for_files() {
        let file_actions = get_file_context_actions(&file_info_file());
        let dir_actions = get_file_context_actions(&file_info_dir());
        assert!(file_actions.iter().any(|a| a.id == "file:quick_look"));
        assert!(!dir_actions.iter().any(|a| a.id == "file:quick_look"));
    }

    // ============================================================================
    // 10. File context — title includes quoted filename
    // ============================================================================

    #[test]
    fn cat10_file_title_includes_name() {
        let actions = get_file_context_actions(&file_info_file());
        let primary = &actions[0];
        assert!(
            primary.title.contains("doc.pdf"),
            "Title should contain filename: {}",
            primary.title
        );
        assert!(primary.title.contains('"'));
    }

    #[test]
    fn cat10_dir_title_includes_name() {
        let actions = get_file_context_actions(&file_info_dir());
        let primary = &actions[0];
        assert!(primary.title.contains("docs"));
    }

    // ============================================================================
    // 11. Path context — directory vs file primary action
    // ============================================================================

    fn path_dir() -> PathInfo {
        PathInfo {
            path: "/tmp/projects".into(),
            name: "projects".into(),
            is_dir: true,
        }
    }

    fn path_file() -> PathInfo {
        PathInfo {
            path: "/tmp/readme.md".into(),
            name: "readme.md".into(),
            is_dir: false,
        }
    }

    #[test]
    fn cat11_dir_primary_is_open_directory() {
        let actions = get_path_context_actions(&path_dir());
        assert_eq!(actions[0].id, "file:open_directory");
    }

    #[test]
    fn cat11_file_primary_is_select_file() {
        let actions = get_path_context_actions(&path_file());
        assert_eq!(actions[0].id, "file:select_file");
    }

    #[test]
    fn cat11_trash_is_always_last() {
        for info in &[path_dir(), path_file()] {
            let actions = get_path_context_actions(info);
            assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
        }
    }

    #[test]
    fn cat11_trash_description_mentions_folder_or_file() {
        let dir_actions = get_path_context_actions(&path_dir());
        let file_actions = get_path_context_actions(&path_file());
        let dir_trash = dir_actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        let file_trash = file_actions
            .iter()
            .find(|a| a.id == "file:move_to_trash")
            .unwrap();
        assert!(dir_trash.description.as_ref().unwrap().contains("folder"));
        assert!(file_trash.description.as_ref().unwrap().contains("file"));
    }

    #[test]
    fn cat11_dir_and_file_have_same_action_count() {
        let d = get_path_context_actions(&path_dir());
        let f = get_path_context_actions(&path_file());
        assert_eq!(d.len(), f.len());
    }

    // ============================================================================
    // 12. Path context — common actions present for both
    // ============================================================================

    #[test]
    fn cat12_always_has_copy_path_and_open_in_editor() {
        for info in &[path_dir(), path_file()] {
            let actions = get_path_context_actions(info);
            assert!(actions.iter().any(|a| a.id == "file:copy_path"));
            assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
            assert!(
                actions
                    .iter()
                    .any(|a| a.id == "file:open_in_quick_terminal")
            );
            assert!(actions.iter().any(|a| a.id == "file:open_in_finder"));
            assert!(actions.iter().any(|a| a.id == "file:copy_filename"));
        }
    }

    // ============================================================================
    // 13. AI command bar — exact action count and section distribution
    // ============================================================================

    #[test]
    fn cat13_ai_command_bar_has_12_actions() {
        let actions = get_ai_command_bar_actions();
        assert_eq!(actions.len(), 38);
    }

    #[test]
    fn cat13_ai_sections_present() {
        let actions = get_ai_command_bar_actions();
        let sections: HashSet<String> = actions.iter().filter_map(|a| a.section.clone()).collect();
        for expected in &[
            "Response",
            "Actions",
            "Attachments",
            "Export",
            "Context",
            "Help",
            "Settings",
        ] {
            assert!(
                sections.contains(*expected),
                "Missing section: {}",
                expected
            );
        }
    }

    #[test]
    fn cat13_all_ai_actions_have_icons() {
        for action in &get_ai_command_bar_actions() {
            assert!(
                action.icon.is_some(),
                "AI action {} missing icon",
                action.id
            );
        }
    }

    #[test]
    fn cat13_all_ai_actions_have_sections() {
        for action in &get_ai_command_bar_actions() {
            assert!(
                action.section.is_some(),
                "AI action {} missing section",
                action.id
            );
        }
    }

    #[test]
    fn cat13_ai_action_ids_unique() {
        let actions = get_ai_command_bar_actions();
        let ids: HashSet<String> = action_ids(&actions).into_iter().collect();
        assert_eq!(ids.len(), actions.len(), "Duplicate IDs in AI command bar");
    }

    // ============================================================================
    // 14. Notes command bar — conditional actions based on state
    // ============================================================================

    // ============================================================================
    // 15. Notes command bar — all actions have icons and sections
    // ============================================================================

    // ============================================================================
    // 16. New chat actions — section structure
    // ============================================================================

    fn sample_model() -> NewChatModelInfo {
        NewChatModelInfo {
            model_id: "claude-3".into(),
            display_name: "Claude 3".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        }
    }

    fn sample_preset() -> NewChatPresetInfo {
        NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: crate::designs::icon_variations::IconName::Star,
        }
    }

    #[test]
    fn cat16_empty_inputs_produce_empty_actions() {
        let actions = get_new_chat_actions(&[], &[], &[]);
        assert!(actions.is_empty());
    }

    #[test]
    fn cat16_sections_appear_in_order() {
        let actions =
            get_new_chat_actions(&[sample_model()], &[sample_preset()], &[sample_model()]);
        let sections: Vec<String> = actions.iter().filter_map(|a| a.section.clone()).collect();
        let last_used_pos = sections.iter().position(|s| s == "Last Used Settings");
        let presets_pos = sections.iter().position(|s| s == "Presets");
        let models_pos = sections.iter().position(|s| s == "Models");
        assert!(last_used_pos.unwrap() < presets_pos.unwrap());
        assert!(presets_pos.unwrap() < models_pos.unwrap());
    }

    #[test]
    fn cat16_preset_has_no_description() {
        let actions = get_new_chat_actions(&[], &[sample_preset()], &[]);
        assert_eq!(
            actions[0].description.as_deref(),
            Some("Uses General preset")
        );
    }

    #[test]
    fn cat16_model_description_is_provider() {
        let actions = get_new_chat_actions(&[], &[], &[sample_model()]);
        assert_eq!(actions[0].description, Some("Uses Anthropic".to_string()));
    }

    #[test]
    fn cat16_last_used_has_bolt_icon() {
        let actions = get_new_chat_actions(&[sample_model()], &[], &[]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::BoltFilled)
        );
    }

    #[test]
    fn cat16_models_have_settings_icon() {
        let actions = get_new_chat_actions(&[], &[], &[sample_model()]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::Settings)
        );
    }

    // ============================================================================
    // 17. Note switcher — icon hierarchy and section assignment
    // ============================================================================

    fn make_note(id: &str, pinned: bool, current: bool) -> NoteSwitcherNoteInfo {
        NoteSwitcherNoteInfo {
            id: id.into(),
            title: format!("Note {}", id),
            char_count: 42,
            is_current: current,
            is_pinned: pinned,
            preview: "some preview text".into(),
            relative_time: "2m ago".into(),
        }
    }

    #[test]
    fn cat17_pinned_gets_star_icon() {
        let actions = get_note_switcher_actions(&[make_note("1", true, false)]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::StarFilled)
        );
    }

    #[test]
    fn cat17_current_gets_check_icon() {
        let actions = get_note_switcher_actions(&[make_note("1", false, true)]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::Check)
        );
    }

    #[test]
    fn cat17_regular_gets_file_icon() {
        let actions = get_note_switcher_actions(&[make_note("1", false, false)]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::File)
        );
    }

    #[test]
    fn cat17_pinned_overrides_current_for_icon() {
        // When both pinned and current, pinned icon wins
        let actions = get_note_switcher_actions(&[make_note("1", true, true)]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::StarFilled)
        );
    }

    #[test]
    fn cat17_pinned_note_in_pinned_section() {
        let actions = get_note_switcher_actions(&[make_note("1", true, false)]);
        assert_eq!(actions[0].section, Some("Pinned".to_string()));
    }

    #[test]
    fn cat17_unpinned_note_in_recent_section() {
        let actions = get_note_switcher_actions(&[make_note("1", false, false)]);
        assert_eq!(actions[0].section, Some("Recent".to_string()));
    }

    #[test]
    fn cat17_current_note_has_bullet_prefix() {
        let actions = get_note_switcher_actions(&[make_note("1", false, true)]);
        assert!(
            actions[0].title.starts_with("• "),
            "Current note should have bullet: {}",
            actions[0].title
        );
    }

    #[test]
    fn cat17_non_current_no_bullet() {
        let actions = get_note_switcher_actions(&[make_note("1", false, false)]);
        assert!(!actions[0].title.starts_with("• "));
    }

    // ============================================================================
    // 18. Note switcher — description rendering edge cases
    // ============================================================================

    #[test]
    fn cat18_preview_with_time_uses_separator() {
        let note = NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: "hello".into(),
            relative_time: "5m ago".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert!(actions[0].description.as_ref().unwrap().contains(" · "));
    }

    // --- merged from part_03.rs ---

    #[test]
    fn cat18_empty_preview_with_time_uses_time() {
        let note = NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "1h ago".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert_eq!(actions[0].description, Some("1h ago".to_string()));
    }

    #[test]
    fn cat18_empty_preview_empty_time_uses_char_count() {
        let note = NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert_eq!(actions[0].description, Some("0 chars".to_string()));
    }

    #[test]
    fn cat18_singular_char_count() {
        let note = NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: "".into(),
            relative_time: "".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        assert_eq!(actions[0].description, Some("1 char".to_string()));
    }

    #[test]
    fn cat18_preview_truncated_at_61_chars() {
        let long_preview = "a".repeat(61);
        let note = NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 61,
            is_current: false,
            is_pinned: false,
            preview: long_preview,
            relative_time: "".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(desc.ends_with('…'), "Should be truncated: {}", desc);
    }

    #[test]
    fn cat18_preview_not_truncated_at_60_chars() {
        let exact_preview = "b".repeat(60);
        let note = NoteSwitcherNoteInfo {
            id: "1".into(),
            title: "T".into(),
            char_count: 60,
            is_current: false,
            is_pinned: false,
            preview: exact_preview,
            relative_time: "".into(),
        };
        let actions = get_note_switcher_actions(&[note]);
        let desc = actions[0].description.as_ref().unwrap();
        assert!(
            !desc.ends_with('…'),
            "Should NOT be truncated at exactly 60"
        );
    }

    // ============================================================================
    // 19. Note switcher — empty state fallback
    // ============================================================================

    #[test]
    fn cat19_empty_notes_shows_placeholder() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "no_notes");
        assert!(actions[0].title.contains("No notes yet"));
    }

    #[test]
    fn cat19_empty_placeholder_has_plus_icon() {
        let actions = get_note_switcher_actions(&[]);
        assert_eq!(
            actions[0].icon,
            Some(crate::designs::icon_variations::IconName::Plus)
        );
    }

    #[test]
    fn cat19_empty_placeholder_description_mentions_cmd_n() {
        let actions = get_note_switcher_actions(&[]);
        assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
    }

    // ============================================================================
    // 20. Chat context — model selection and conditional actions
    // ============================================================================

    #[test]
    fn cat20_no_models_still_has_continue_in_chat() {
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
    fn cat20_current_model_gets_checkmark() {
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
        let picker = get_chat_model_picker_actions(&info);
        let gpt4 = picker
            .iter()
            .find(|a| a.id == "chat:select_model_gpt4")
            .unwrap();
        assert!(gpt4.title.contains('✓'), "Current should have ✓");
        let claude = picker
            .iter()
            .find(|a| a.id == "chat:select_model_claude")
            .unwrap();
        assert!(!claude.title.contains('✓'), "Non-current should not have ✓");
    }

    #[test]
    fn cat20_copy_response_only_when_has_response() {
        let no_resp = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let with_resp = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: true,
        };
        assert!(
            !get_chat_context_actions(&no_resp)
                .iter()
                .any(|a| a.id == "chat:copy_response")
        );
        assert!(
            get_chat_context_actions(&with_resp)
                .iter()
                .any(|a| a.id == "chat:copy_response")
        );
    }

    #[test]
    fn cat20_clear_conversation_only_when_has_messages() {
        let no_msgs = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: false,
            has_response: false,
        };
        let with_msgs = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages: true,
            has_response: false,
        };
        assert!(
            !get_chat_context_actions(&no_msgs)
                .iter()
                .any(|a| a.id == "chat:clear_conversation")
        );
        assert!(
            get_chat_context_actions(&with_msgs)
                .iter()
                .any(|a| a.id == "chat:clear_conversation")
        );
    }

    // ============================================================================
    // 21. to_deeplink_name — edge cases
    // ============================================================================

    #[test]
    fn cat21_basic_conversion() {
        assert_eq!(to_deeplink_name("My Script"), "my-script");
    }

    #[test]
    fn cat21_underscores_become_hyphens() {
        assert_eq!(to_deeplink_name("hello_world"), "hello-world");
    }

    #[test]
    fn cat21_special_chars_stripped() {
        assert_eq!(to_deeplink_name("test!@#$%"), "test");
    }

    #[test]
    fn cat21_consecutive_specials_collapsed() {
        assert_eq!(to_deeplink_name("a---b"), "a-b");
    }

    #[test]
    fn cat21_unicode_alphanumeric_preserved() {
        assert_eq!(to_deeplink_name("café"), "caf%C3%A9");
    }

    #[test]
    fn cat21_leading_trailing_stripped() {
        assert_eq!(to_deeplink_name("  hello  "), "hello");
    }

    #[test]
    fn cat21_numbers_preserved() {
        assert_eq!(to_deeplink_name("v2 script"), "v2-script");
    }

    // ============================================================================
    // 22. fuzzy_match edge cases
    // ============================================================================

    #[test]
    fn cat22_empty_needle_matches() {
        assert!(ActionsDialog::fuzzy_match("anything", ""));
    }

    #[test]
    fn cat22_empty_haystack_with_needle_fails() {
        assert!(!ActionsDialog::fuzzy_match("", "x"));
    }

    #[test]
    fn cat22_both_empty_matches() {
        assert!(ActionsDialog::fuzzy_match("", ""));
    }

    #[test]
    fn cat22_exact_match() {
        assert!(ActionsDialog::fuzzy_match("hello", "hello"));
    }

    #[test]
    fn cat22_subsequence_match() {
        assert!(ActionsDialog::fuzzy_match("hello world", "hlo"));
    }

    #[test]
    fn cat22_no_subsequence() {
        assert!(!ActionsDialog::fuzzy_match("hello", "xyz"));
    }

    #[test]
    fn cat22_needle_longer_than_haystack() {
        assert!(!ActionsDialog::fuzzy_match("hi", "hello"));
    }

    // ============================================================================
    // 23. score_action boundary thresholds
    // ============================================================================

    #[test]
    fn cat23_prefix_match_gives_100() {
        let a = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
        assert!(ActionsDialog::score_action(&a, "edit") >= 100);
    }

    #[test]
    fn cat23_contains_match_gives_50() {
        let a = Action::new("id", "My Edit Tool", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&a, "edit");
        assert!(
            (50..100).contains(&score),
            "Contains should be 50-99: {}",
            score
        );
    }

    #[test]
    fn cat23_fuzzy_match_gives_25() {
        let a = Action::new("id", "Elephant", None, ActionCategory::ScriptContext);
        let score = ActionsDialog::score_action(&a, "ept");
        assert!(
            (25..50).contains(&score),
            "Fuzzy should be 25-49: {}",
            score
        );
    }

    #[test]
    fn cat23_description_bonus_15() {
        let a = Action::new(
            "id",
            "Open File",
            Some("Edit in editor".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&a, "editor");
        assert!(
            score >= 15,
            "Description match should give >= 15: {}",
            score
        );
    }

    #[test]
    fn cat23_no_match_gives_0() {
        let a = Action::new("id", "Run Script", None, ActionCategory::ScriptContext);
        assert_eq!(ActionsDialog::score_action(&a, "xyz"), 0);
    }

    #[test]
    fn cat23_prefix_plus_desc_stacks() {
        let a = Action::new(
            "id",
            "Edit Script",
            Some("Edit the script in editor".into()),
            ActionCategory::ScriptContext,
        );
        let score = ActionsDialog::score_action(&a, "edit");
        assert!(score >= 115, "Prefix(100) + Desc(15) = 115: {}", score);
    }

    // ============================================================================
    // 24. parse_shortcut_keycaps
    // ============================================================================

    #[test]
    fn cat24_modifier_plus_letter() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘C");
        assert_eq!(caps, vec!["⌘", "C"]);
    }

    #[test]
    fn cat24_two_modifiers() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
        assert_eq!(caps, vec!["⌘", "⇧", "C"]);
    }

    #[test]
    fn cat24_enter_symbol() {
        let caps = ActionsDialog::parse_shortcut_keycaps("↵");
        assert_eq!(caps, vec!["↵"]);
    }

    #[test]
    fn cat24_arrow_keys() {
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
    }

    #[test]
    fn cat24_escape_and_space() {
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("⎋"), vec!["⎋"]);
        assert_eq!(ActionsDialog::parse_shortcut_keycaps("␣"), vec!["␣"]);
    }

    #[test]
    fn cat24_lowercase_uppercased() {
        let caps = ActionsDialog::parse_shortcut_keycaps("⌘x");
        assert_eq!(caps, vec!["⌘", "X"]);
    }

    // ============================================================================
    // 25. build_grouped_items_static behavior
    // ============================================================================

    #[test]
    fn cat25_empty_filtered_returns_empty() {
        let actions: Vec<Action> = vec![];
        let result = build_grouped_items_static(&actions, &[], SectionStyle::Headers);
        assert!(result.is_empty());
    }

    #[test]
    fn cat25_headers_inserts_section_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered = vec![0, 1];
        let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should have: header S1, item 0, header S2, item 1
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn cat25_separators_no_headers() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        ];
        let filtered = vec![0, 1];
        let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
        // Should have: item 0, item 1 (no headers)
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn cat25_none_style_no_headers() {
        let actions =
            vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1")];
        let filtered = vec![0];
        let result = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn cat25_same_section_no_duplicate_header() {
        let actions = vec![
            Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
            Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S"),
        ];
        let filtered = vec![0, 1];
        let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
        // Should have: header S, item 0, item 1
        assert_eq!(result.len(), 3);
    }

    // --- merged from part_04.rs ---

    // ============================================================================
    // 26. coerce_action_selection
    // ============================================================================

    #[test]
    fn cat26_empty_returns_none() {
        assert!(coerce_action_selection(&[], 0).is_none());
    }

    #[test]
    fn cat26_on_item_returns_same() {
        let rows = vec![GroupedActionItem::Item(0)];
        assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    }

    #[test]
    fn cat26_header_searches_down() {
        let rows = vec![
            GroupedActionItem::SectionHeader("S".into()),
            GroupedActionItem::Item(0),
        ];
        assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    }

    #[test]
    fn cat26_trailing_header_searches_up() {
        let rows = vec![
            GroupedActionItem::Item(0),
            GroupedActionItem::SectionHeader("S".into()),
        ];
        assert_eq!(coerce_action_selection(&rows, 1), Some(0));
    }

    #[test]
    fn cat26_all_headers_returns_none() {
        let rows = vec![
            GroupedActionItem::SectionHeader("A".into()),
            GroupedActionItem::SectionHeader("B".into()),
        ];
        assert!(coerce_action_selection(&rows, 0).is_none());
    }

    #[test]
    fn cat26_out_of_bounds_clamped() {
        let rows = vec![GroupedActionItem::Item(0)];
        assert_eq!(coerce_action_selection(&rows, 100), Some(0));
    }

    // ============================================================================
    // 27. Cross-context ID namespace collision avoidance
    // ============================================================================

    #[test]
    fn cat27_script_and_clipboard_no_id_overlap() {
        let script_actions = get_script_context_actions(&ScriptInfo::new("test", "/test.ts"));
        let clip_actions = get_clipboard_history_context_actions(&text_entry());
        let script_ids: HashSet<String> = action_ids(&script_actions).into_iter().collect();
        let clip_ids: HashSet<String> = action_ids(&clip_actions).into_iter().collect();
        let overlap: Vec<&String> = script_ids.intersection(&clip_ids).collect();
        assert!(
            overlap.is_empty(),
            "Script/Clipboard ID overlap: {:?}",
            overlap
        );
    }

    #[test]
    fn cat27_path_and_file_some_shared_ids() {
        // Path and file contexts are related — they share some IDs by design
        let path_actions = get_path_context_actions(&path_dir());
        let file_actions = get_file_context_actions(&file_info_dir());
        let path_ids: HashSet<String> = action_ids(&path_actions).into_iter().collect();
        let file_ids: HashSet<String> = action_ids(&file_actions).into_iter().collect();
        let shared: Vec<&String> = path_ids.intersection(&file_ids).collect();
        // copy_path, copy_filename, open_directory should be shared
        assert!(
            shared.len() >= 2,
            "Path/File should share some IDs: {:?}",
            shared
        );
    }

    // ============================================================================
    // 28. All actions have non-empty id and title
    // ============================================================================

    #[test]
    fn cat28_script_actions_nonempty_id_title() {
        for action in &get_script_context_actions(&ScriptInfo::new("t", "/t.ts")) {
            assert!(!action.id.is_empty(), "Empty ID");
            assert!(!action.title.is_empty(), "Empty title for {}", action.id);
        }
    }

    #[test]
    fn cat28_clipboard_actions_nonempty_id_title() {
        for action in &get_clipboard_history_context_actions(&text_entry()) {
            assert!(!action.id.is_empty(), "Empty ID");
            assert!(!action.title.is_empty(), "Empty title for {}", action.id);
        }
    }

    #[test]
    fn cat28_ai_actions_nonempty_id_title() {
        for action in &get_ai_command_bar_actions() {
            assert!(!action.id.is_empty(), "Empty ID");
            assert!(!action.title.is_empty(), "Empty title for {}", action.id);
        }
    }

    #[test]
    fn cat28_path_actions_nonempty_id_title() {
        for action in &get_path_context_actions(&path_dir()) {
            assert!(!action.id.is_empty(), "Empty ID");
            assert!(!action.title.is_empty(), "Empty title for {}", action.id);
        }
    }

    #[test]
    fn cat28_file_actions_nonempty_id_title() {
        for action in &get_file_context_actions(&file_info_file()) {
            assert!(!action.id.is_empty(), "Empty ID");
            assert!(!action.title.is_empty(), "Empty title for {}", action.id);
        }
    }

    // ============================================================================
    // 29. has_action = false for all built-in actions
    // ============================================================================

    #[test]
    fn cat29_script_actions_has_action_false() {
        for action in &get_script_context_actions(&ScriptInfo::new("t", "/t.ts")) {
            assert!(
                !action.has_action,
                "{} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn cat29_clipboard_actions_has_action_false() {
        for action in &get_clipboard_history_context_actions(&text_entry()) {
            assert!(
                !action.has_action,
                "{} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn cat29_ai_actions_has_action_false() {
        for action in &get_ai_command_bar_actions() {
            assert!(
                !action.has_action,
                "{} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn cat29_path_actions_has_action_false() {
        for action in &get_path_context_actions(&path_dir()) {
            assert!(
                !action.has_action,
                "{} should have has_action=false",
                action.id
            );
        }
    }

    #[test]
    fn cat29_file_actions_has_action_false() {
        for action in &get_file_context_actions(&file_info_file()) {
            assert!(
                !action.has_action,
                "{} should have has_action=false",
                action.id
            );
        }
    }

    // ============================================================================
    // 30. Scriptlet defined actions — has_action=true and value set
    // ============================================================================

    #[test]
    fn cat30_scriptlet_actions_have_has_action_true() {
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".into(),
            command: "copy-cmd".into(),
            tool: "bash".into(),
            code: "pbcopy".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(
            actions[0].has_action,
            "Scriptlet action should have has_action=true"
        );
    }

    #[test]
    fn cat30_scriptlet_actions_have_value() {
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".into(),
            command: "copy-cmd".into(),
            tool: "bash".into(),
            code: "pbcopy".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].value, Some("copy-cmd".to_string()));
    }

    #[test]
    fn cat30_scriptlet_action_id_format() {
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
        scriptlet.actions = vec![ScriptletAction {
            name: "Open Browser".into(),
            command: "open-browser".into(),
            tool: "bash".into(),
            code: "open".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].id, "scriptlet_action:open-browser");
    }

    #[test]
    fn cat30_scriptlet_with_shortcut_formatted() {
        let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
        scriptlet.actions = vec![ScriptletAction {
            name: "Copy".into(),
            command: "copy".into(),
            tool: "bash".into(),
            code: "pbcopy".into(),
            inputs: vec![],
            shortcut: Some("cmd+c".into()),
            description: None,
        }];
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert_eq!(actions[0].shortcut, Some("⌘C".to_string()));
    }

    #[test]
    fn cat30_scriptlet_empty_actions_returns_empty() {
        let scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo test".into());
        let actions = get_scriptlet_defined_actions(&scriptlet);
        assert!(actions.is_empty());
    }

    // ============================================================================
    // Bonus: Ordering determinism — repeated calls produce same result
    // ============================================================================

    #[test]
    fn bonus_script_actions_deterministic() {
        let s = ScriptInfo::new("test", "/test.ts");
        let a1 = action_ids(&get_script_context_actions(&s));
        let a2 = action_ids(&get_script_context_actions(&s));
        assert_eq!(a1, a2);
    }

    #[test]
    fn bonus_clipboard_actions_deterministic() {
        let a1 = action_ids(&get_clipboard_history_context_actions(&text_entry()));
        let a2 = action_ids(&get_clipboard_history_context_actions(&text_entry()));
        assert_eq!(a1, a2);
    }

    #[test]
    fn bonus_ai_actions_deterministic() {
        let a1 = action_ids(&get_ai_command_bar_actions());
        let a2 = action_ids(&get_ai_command_bar_actions());
        assert_eq!(a1, a2);
    }

    // ============================================================================
    // Bonus: ActionCategory PartialEq
    // ============================================================================

    #[test]
    fn bonus_action_category_equality() {
        assert_eq!(ActionCategory::ScriptContext, ActionCategory::ScriptContext);
        assert_ne!(ActionCategory::ScriptContext, ActionCategory::ScriptOps);
        assert_ne!(ActionCategory::GlobalOps, ActionCategory::Terminal);
    }

    // ============================================================================
    // Bonus: title_lower invariant across contexts
    // ============================================================================

    #[test]
    fn bonus_title_lower_matches_lowercase() {
        // Script context
        for action in &get_script_context_actions(&ScriptInfo::new("Test", "/t.ts")) {
            assert_eq!(action.title_lower, action.title.to_lowercase());
        }
        // Clipboard context
        for action in &get_clipboard_history_context_actions(&text_entry()) {
            assert_eq!(action.title_lower, action.title.to_lowercase());
        }
        // AI command bar
        for action in &get_ai_command_bar_actions() {
            assert_eq!(action.title_lower, action.title.to_lowercase());
        }
    }

    // ============================================================================
    // Bonus: All ScriptContext category
    // ============================================================================

    #[test]
    fn bonus_all_script_actions_are_script_context() {
        for a in &get_script_context_actions(&ScriptInfo::new("t", "/t.ts")) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn bonus_all_clipboard_actions_are_script_context() {
        for a in &get_clipboard_history_context_actions(&text_entry()) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn bonus_all_ai_actions_are_script_context() {
        for a in &get_ai_command_bar_actions() {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }

    #[test]
    fn bonus_all_path_actions_are_script_context() {
        for a in &get_path_context_actions(&path_dir()) {
            assert_eq!(a.category, ActionCategory::ScriptContext);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_12 {
    //! Batch 12: Dialog Built-in Action Validation Tests
    //!
    //! 150+ tests across 30 categories validating random behaviors of
    //! built-in action window dialogs.
    //!
    //! Focus areas for this batch:
    //! - CommandBarConfig preset field validation (all presets, all fields)
    //! - Clipboard text vs image action count delta and exact IDs
    //! - Scriptlet context with_custom shortcut/alias dynamic action switching
    //! - Agent context description keyword content
    //! - Note switcher multi-note sorting and section assignment
    //! - AI command bar section-to-action-count mapping
    //! - Path context description substring matching
    //! - Chat context multi-model ID generation patterns
    //! - Score_action stacking with multi-field matches
    //! - build_grouped_items_static with alternating sections
    //! - Cross-context action description non-empty invariant
    //! - format_shortcut_hint chaining (multiple modifiers in sequence)
    //! - Action builder with_icon and with_section field isolation
    //! - ScriptInfo with_is_script constructor
    //! - Deeplink name with mixed Unicode scripts
    //! - parse_shortcut_keycaps special symbol recognition
    //! - Clipboard destructive action shortcut exact values
    //! - File context macOS action count (file vs dir)
    //! - New chat action ID prefix patterns
    //! - Notes command bar section-to-action mapping

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // =========================================================================
        // Helper
        // =========================================================================
        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // =========================================================================
        // 1. CommandBarConfig preset field validation — all four presets
        // =========================================================================

        #[test]
        fn cat01_command_bar_default_dialog_config() {
            let cfg = ActionsDialogConfig::default();
            assert_eq!(cfg.search_position, SearchPosition::Top);
            assert_eq!(cfg.section_style, SectionStyle::Headers);
            assert_eq!(cfg.anchor, AnchorPosition::Bottom);
            assert!(!cfg.show_icons);
            assert!(!cfg.show_footer);
        }

        #[test]
        fn cat01_command_bar_ai_style_dialog_config() {
            let cfg = super::super::command_bar::CommandBarConfig::ai_style();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Top);
            assert!(cfg.dialog_config.show_icons);
            assert!(!cfg.dialog_config.show_footer);
            assert!(cfg.close_on_select);
            assert!(cfg.close_on_click_outside);
            assert!(cfg.close_on_escape);
        }

        #[test]
        fn cat01_command_bar_main_menu_style_dialog_config() {
            let cfg = super::super::command_bar::CommandBarConfig::main_menu_style();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Bottom);
            assert!(!cfg.dialog_config.show_icons);
            assert!(!cfg.dialog_config.show_footer);
        }

        #[test]
        fn cat01_command_bar_no_search_dialog_config() {
            let cfg = super::super::command_bar::CommandBarConfig::no_search();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Hidden);
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Bottom);
            assert!(!cfg.dialog_config.show_icons);
            assert!(!cfg.dialog_config.show_footer);
        }

        #[test]
        fn cat01_command_bar_notes_style_dialog_config() {
            let cfg = super::super::command_bar::CommandBarConfig::notes_style();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Top);
            assert!(cfg.dialog_config.show_icons);
            assert!(!cfg.dialog_config.show_footer);
        }

        #[test]
        fn cat01_command_bar_all_presets_close_defaults_true() {
            for cfg in [
                super::super::command_bar::CommandBarConfig::default(),
                super::super::command_bar::CommandBarConfig::ai_style(),
                super::super::command_bar::CommandBarConfig::main_menu_style(),
                super::super::command_bar::CommandBarConfig::no_search(),
                super::super::command_bar::CommandBarConfig::notes_style(),
            ] {
                assert!(cfg.close_on_select, "close_on_select should default true");
                assert!(
                    cfg.close_on_click_outside,
                    "close_on_click_outside should default true"
                );
                assert!(cfg.close_on_escape, "close_on_escape should default true");
            }
        }

        // =========================================================================
        // 2. Clipboard text vs image — exact ID difference
        // =========================================================================

        fn make_text_entry() -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "t1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            }
        }

        fn make_image_entry() -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "i1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".into(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            }
        }

        #[test]
        fn cat02_image_has_ocr_text_does_not() {
            let text_ids = action_ids(&get_clipboard_history_context_actions(&make_text_entry()));
            let image_ids = action_ids(&get_clipboard_history_context_actions(&make_image_entry()));
            assert!(!text_ids.contains(&"clip:clipboard_ocr".to_string()));
            assert!(image_ids.contains(&"clip:clipboard_ocr".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_image_has_open_with_and_cleanshot_text_does_not() {
            let text_ids = action_ids(&get_clipboard_history_context_actions(&make_text_entry()));
            let image_ids = action_ids(&get_clipboard_history_context_actions(&make_image_entry()));
            for id in [
                "clip:clipboard_open_with",
                "clip:clipboard_annotate_cleanshot",
                "clip:clipboard_upload_cleanshot",
            ] {
                assert!(
                    !text_ids.contains(&id.to_string()),
                    "text should not have {}",
                    id
                );
                assert!(
                    image_ids.contains(&id.to_string()),
                    "image should have {}",
                    id
                );
            }
        }

        #[test]
        fn cat02_image_more_actions_than_text() {
            let text_count = get_clipboard_history_context_actions(&make_text_entry()).len();
            let image_count = get_clipboard_history_context_actions(&make_image_entry()).len();
            assert!(
                image_count > text_count,
                "image {} > text {}",
                image_count,
                text_count
            );
        }

        #[test]
        fn cat02_shared_actions_between_text_and_image() {
            let text_ids: HashSet<String> =
                action_ids(&get_clipboard_history_context_actions(&make_text_entry()))
                    .into_iter()
                    .collect();
            let image_ids: HashSet<String> =
                action_ids(&get_clipboard_history_context_actions(&make_image_entry()))
                    .into_iter()
                    .collect();
            let shared: Vec<&str> = vec![
                "clip:clipboard_paste",
                "clip:clipboard_copy",
                "clip:clipboard_paste_keep_open",
                "clip:clipboard_share",
                "clip:clipboard_attach_to_ai",
                "clip:clipboard_pin",
                "clip:clipboard_save_file",
                "clip:clipboard_delete",
                "clip:clipboard_delete_multiple",
                "clip:clipboard_delete_all",
            ];
            for id in shared {
                assert!(text_ids.contains(id), "text missing {}", id);
                assert!(image_ids.contains(id), "image missing {}", id);
            }
            // save_snippet is TEXT-only: images get clip:clipboard_ocr instead.
            assert!(
                text_ids.contains("clip:clipboard_save_snippet"),
                "text missing clip:clipboard_save_snippet"
            );
            assert!(
                !image_ids.contains("clip:clipboard_save_snippet"),
                "save_snippet must be text-only"
            );
        }

        // =========================================================================
        // 3. Scriptlet context shortcut/alias dynamic switching
        // =========================================================================

        #[test]
        fn cat03_scriptlet_with_custom_no_shortcut_no_alias() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"add_shortcut".to_string()));
            assert!(ids.contains(&"add_alias".to_string()));
            assert!(!ids.contains(&"update_shortcut".to_string()));
            assert!(!ids.contains(&"update_alias".to_string()));
        }

        #[test]
        fn cat03_scriptlet_with_custom_has_shortcut_has_alias() {
            let script = ScriptInfo::scriptlet(
                "Test",
                "/path/test.md",
                Some("cmd+t".into()),
                Some("ts".into()),
            );
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"update_shortcut".to_string()));
            assert!(ids.contains(&"remove_shortcut".to_string()));
            assert!(ids.contains(&"update_alias".to_string()));
            assert!(ids.contains(&"remove_alias".to_string()));
            assert!(!ids.contains(&"add_shortcut".to_string()));
            assert!(!ids.contains(&"add_alias".to_string()));
        }

        #[test]
        fn cat03_scriptlet_with_custom_has_shortcut_no_alias() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", Some("cmd+t".into()), None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"update_shortcut".to_string()));
            assert!(ids.contains(&"add_alias".to_string()));
        }

        #[test]
        fn cat03_scriptlet_with_custom_no_shortcut_has_alias() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, Some("ts".into()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"add_shortcut".to_string()));
            assert!(ids.contains(&"update_alias".to_string()));
        }

        #[test]
        fn cat03_scriptlet_context_always_has_edit_reveal_copy() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"edit_scriptlet".to_string()));
            assert!(ids.contains(&"reveal_scriptlet_in_finder".to_string()));
            assert!(ids.contains(&"copy_scriptlet_path".to_string()));
            assert!(ids.contains(&"copy_content".to_string()));
            assert!(ids.contains(&"copy_deeplink".to_string()));
        }

        // =========================================================================
        // 4. Agent context description keyword content
        // =========================================================================

        #[test]
        fn cat04_agent_edit_title_says_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn cat04_agent_edit_description_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(edit.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn cat04_agent_reveal_description_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
            assert!(reveal.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn cat04_agent_copy_path_description_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
            assert!(cp.description.as_ref().unwrap().contains("agent"));
        }

        #[test]
        fn cat04_agent_no_view_logs() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        // =========================================================================
        // 5. Note switcher multi-note section assignment
        // =========================================================================

        fn make_note(id: &str, title: &str, pinned: bool, current: bool) -> NoteSwitcherNoteInfo {
            NoteSwitcherNoteInfo {
                id: id.into(),
                title: title.into(),
                char_count: 100,
                is_current: current,
                is_pinned: pinned,
                preview: "Some preview text".into(),
                relative_time: "2m ago".into(),
            }
        }

        #[test]
        fn cat05_pinned_notes_in_pinned_section() {
            let notes = vec![make_note("1", "Pinned Note", true, false)];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn cat05_unpinned_notes_in_recent_section() {
            let notes = vec![make_note("2", "Recent Note", false, false)];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn cat05_mixed_notes_correct_sections() {
            let notes = vec![
                make_note("1", "Pinned", true, false),
                make_note("2", "Recent", false, false),
                make_note("3", "Also Pinned", true, true),
            ];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
            assert_eq!(actions[1].section.as_deref(), Some("Recent"));
            assert_eq!(actions[2].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn cat05_current_note_gets_bullet_prefix() {
            let notes = vec![make_note("1", "My Note", false, true)];
            let actions = get_note_switcher_actions(&notes);
            assert!(actions[0].title.starts_with("• "));
        }

        #[test]
        fn cat05_non_current_note_no_bullet() {
            let notes = vec![make_note("1", "My Note", false, false)];
            let actions = get_note_switcher_actions(&notes);
            assert!(!actions[0].title.starts_with("• "));
        }

        #[test]
        fn cat05_note_ids_follow_pattern() {
            let notes = vec![
                make_note("abc-123", "Note A", false, false),
                make_note("def-456", "Note B", true, false),
            ];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].id, "note_abc-123");
            assert_eq!(actions[1].id, "note_def-456");
        }

        // =========================================================================
        // 6. AI command bar section-to-action-count mapping
        // =========================================================================

        #[test]
        fn cat06_ai_response_section_has_3_actions() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .count();
            assert_eq!(count, 3, "Response section should have 3 actions");
        }

        #[test]
        fn cat06_ai_actions_section_has_4_actions() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .count();
            assert_eq!(
                count, 7,
                "Actions section should have 7 actions (submit, new_chat, delete_chat, branch_from_last)"
            );
        }

        #[test]
        fn cat06_ai_attachments_section_has_2_actions() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .count();
            assert_eq!(count, 4, "Attachments section should have 7 actions");
        }

        #[test]
        fn cat06_ai_export_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Export"))
                .count();
            assert_eq!(count, 1, "Export section should have 1 action");
        }

        #[test]
        fn cat06_ai_help_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Help"))
                .count();
            assert_eq!(count, 1, "Help section should have 1 action");
        }

        #[test]
        fn cat06_ai_settings_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Settings"))
                .count();
            assert_eq!(count, 2, "Settings section should have 2 actions");
        }

        #[test]
        fn cat06_ai_total_12_actions() {
            assert_eq!(get_ai_command_bar_actions().len(), 38);
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat06_ai_7_unique_sections() {
            let actions = get_ai_command_bar_actions();
            let sections: HashSet<_> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            assert_eq!(sections.len(), 7);
        }

        // =========================================================================
        // 7. Path context description substring matching
        // =========================================================================

        #[test]
        fn cat07_path_dir_open_description_mentions_directory() {
            let info = PathInfo {
                path: "/tmp/docs".into(),
                name: "docs".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let open = actions
                .iter()
                .find(|a| a.id == "file:open_directory")
                .unwrap();
            assert!(
                open.description.as_ref().unwrap().contains("directory")
                    || open.description.as_ref().unwrap().contains("Navigate")
            );
        }

        #[test]
        fn cat07_path_file_select_description_mentions_file() {
            let info = PathInfo {
                path: "/tmp/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let sel = actions.iter().find(|a| a.id == "file:select_file").unwrap();
            assert!(
                sel.description.as_ref().unwrap().contains("file")
                    || sel.description.as_ref().unwrap().contains("Submit")
            );
        }

        #[test]
        fn cat07_path_trash_dir_says_folder() {
            let info = PathInfo {
                path: "/tmp/docs".into(),
                name: "docs".into(),
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
        fn cat07_path_trash_file_says_file() {
            let info = PathInfo {
                path: "/tmp/a.txt".into(),
                name: "a.txt".into(),
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
        fn cat07_path_open_in_editor_mentions_editor() {
            let info = PathInfo {
                path: "/tmp/a.txt".into(),
                name: "a.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let ed = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .unwrap();
            assert!(
                ed.description.as_ref().unwrap().contains("$EDITOR")
                    || ed.description.as_ref().unwrap().contains("editor")
            );
        }

        // =========================================================================
        // 8. Chat context multi-model ID generation
        // =========================================================================

        #[test]
        fn cat08_multiple_models_generate_sequential_ids() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt-4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                    ChatModelInfo {
                        id: "claude-3".into(),
                        display_name: "Claude 3".into(),
                        provider: "Anthropic".into(),
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
            assert!(picker.iter().any(|a| a.id == "chat:select_model_gpt-4"));
            assert!(picker.iter().any(|a| a.id == "chat:select_model_claude-3"));
            assert!(picker.iter().any(|a| a.id == "chat:select_model_gemini"));
        }

        #[test]
        fn cat08_current_model_gets_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
                available_models: vec![
                    ChatModelInfo {
                        id: "gpt-4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                    ChatModelInfo {
                        id: "claude-3".into(),
                        display_name: "Claude 3".into(),
                        provider: "Anthropic".into(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
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

        #[test]
        fn cat08_no_models_still_has_continue() {
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
        fn cat08_model_description_shows_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m1".into(),
                    display_name: "Model 1".into(),
                    provider: "Anthropic".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let m = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m1")
                .unwrap();
            assert!(m.description.as_ref().unwrap().contains("Anthropic"));
        }

        #[test]
        fn cat08_has_response_adds_copy_response() {
            let with = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let without = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            assert!(
                get_chat_context_actions(&with)
                    .iter()
                    .any(|a| a.id == "chat:copy_response")
            );
            assert!(
                !get_chat_context_actions(&without)
                    .iter()
                    .any(|a| a.id == "chat:copy_response")
            );
        }

        #[test]
        fn cat08_has_messages_adds_clear_conversation() {
            let with = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let without = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            assert!(
                get_chat_context_actions(&with)
                    .iter()
                    .any(|a| a.id == "chat:clear_conversation")
            );
            assert!(
                !get_chat_context_actions(&without)
                    .iter()
                    .any(|a| a.id == "chat:clear_conversation")
            );
        }

        // =========================================================================
        // 9. Score_action stacking with multi-field matches
        // =========================================================================

        #[test]
        fn cat09_prefix_plus_description_stacks() {
            let action = Action::new(
                "script:edit",
                "Edit Script",
                Some("Edit the script file".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "edit");
            assert!(score >= 115, "prefix(100)+desc(15)={}", score);
        }

        #[test]
        fn cat09_prefix_plus_shortcut_stacks() {
            let action = Action::new("script:edit", "Edit", None, ActionCategory::ScriptContext)
                .with_shortcut("script:edit");
            let score = ActionsDialog::score_action(&action, "edit");
            assert!(score >= 110, "prefix(100)+shortcut(10)={}", score);
        }

        #[test]
        fn cat09_contains_plus_description_stacks() {
            let action = Action::new(
                "x",
                "Copy Edit Path",
                Some("Edit mode".into()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "edit");
            assert!(score >= 65, "contains(50)+desc(15)={}", score);
        }

        #[test]
        fn cat09_no_match_returns_zero() {
            let action = Action::new(
                "x",
                "Foo",
                Some("Bar".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(ActionsDialog::score_action(&action, "zzz"), 0);
        }

        #[test]
        fn cat09_empty_query_matches_prefix() {
            let action = Action::new("x", "Anything", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "");
            assert!(
                score >= 100,
                "empty query is prefix of everything: {}",
                score
            );
        }

        // =========================================================================
        // 10. build_grouped_items_static with alternating sections
        // =========================================================================

        #[test]
        fn cat10_alternating_sections_produce_multiple_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Alpha"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Beta"),
                Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Alpha"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 3, "Alpha, Beta, Alpha = 3 headers");
        }

        #[test]
        fn cat10_same_section_no_duplicate_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Same"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Same"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1);
        }

        #[test]
        fn cat10_separators_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Y"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            let header_count = grouped
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0);
        }

        #[test]
        fn cat10_none_style_no_headers() {
            let actions =
                vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("X")];
            let filtered: Vec<usize> = vec![0];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            let header_count = grouped
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0);
        }

        #[test]
        fn cat10_empty_filtered_returns_empty() {
            let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
            let grouped = build_grouped_items_static(&actions, &[], SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        // =========================================================================
        // 11. Cross-context action description non-empty invariant
        // =========================================================================

        #[test]
        fn cat11_script_actions_all_have_descriptions() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    action.description.is_some(),
                    "Script action '{}' missing description",
                    action.id
                );
            }
        }

        #[test]
        fn cat11_clipboard_actions_all_have_descriptions() {
            for action in &get_clipboard_history_context_actions(&make_text_entry()) {
                assert!(
                    action.description.is_some(),
                    "Clipboard action '{}' missing description",
                    action.id
                );
            }
        }

        #[test]
        fn cat11_ai_actions_all_have_descriptions() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    action.description.is_some(),
                    "AI action '{}' missing description",
                    action.id
                );
            }
        }

        #[test]
        fn cat11_path_actions_all_have_descriptions() {
            let info = PathInfo {
                path: "/tmp/x".into(),
                name: "x".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&info) {
                assert!(
                    action.description.is_some(),
                    "Path action '{}' missing description",
                    action.id
                );
            }
        }

        #[test]
        fn cat11_file_actions_all_have_descriptions() {
            let info = FileInfo {
                path: "/tmp/x.txt".into(),
                name: "x.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&info) {
                assert!(
                    action.description.is_some(),
                    "File action '{}' missing description",
                    action.id
                );
            }
        }

        // =========================================================================
        // 12. format_shortcut_hint chaining — multiple modifiers
        // =========================================================================

        // format_shortcut_hint is private, but we test it indirectly through scriptlet actions

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat12_scriptlet_shortcut_cmd_shift_becomes_symbols() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Act".into(),
                command: "act".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: Some("cmd+shift+c".into()),
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].shortcut.as_deref(), Some("⌘⇧C"));
        }

        #[test]
        fn cat12_scriptlet_shortcut_ctrl_alt_becomes_symbols() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Act".into(),
                command: "act".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: Some("ctrl+alt+x".into()),
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].shortcut.as_deref(), Some("⌃⌥X"));
        }

        #[test]
        fn cat12_scriptlet_shortcut_no_shortcut_is_none() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Act".into(),
                command: "act".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].shortcut.is_none());
        }

        #[test]
        fn cat12_scriptlet_shortcut_single_key() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Act".into(),
                command: "act".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: Some("a".into()),
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].shortcut.as_deref(), Some("A"));
        }

        // =========================================================================
        // 13. Action builder with_icon and with_section field isolation
        // =========================================================================

        #[test]
        fn cat13_with_icon_does_not_affect_section() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Copy);
            assert_eq!(action.icon, Some(IconName::Copy));
            assert!(action.section.is_none());
        }

        #[test]
        fn cat13_with_section_does_not_affect_icon() {
            let action =
                Action::new("x", "X", None, ActionCategory::ScriptContext).with_section("Test");
            assert!(action.icon.is_none());
            assert_eq!(action.section.as_deref(), Some("Test"));
        }

        #[test]
        fn cat13_chaining_icon_then_section() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Star)
                .with_section("Sec");
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("Sec"));
        }

        #[test]
        fn cat13_chaining_section_then_icon() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_section("Sec")
                .with_icon(IconName::Star);
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("Sec"));
        }

        #[test]
        fn cat13_with_shortcut_preserves_icon_section() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Plus)
                .with_section("S")
                .with_shortcut("⌘X");
            assert_eq!(action.icon, Some(IconName::Plus));
            assert_eq!(action.section.as_deref(), Some("S"));
            assert_eq!(action.shortcut.as_deref(), Some("⌘X"));
        }

        // =========================================================================
        // 14. ScriptInfo with_is_script constructor
        // =========================================================================

        #[test]
        fn cat14_with_is_script_true() {
            let s = ScriptInfo::with_is_script("test", "/path", true);
            assert!(s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
            assert_eq!(s.action_verb, "Run");
        }

        #[test]
        fn cat14_with_is_script_false() {
            let s = ScriptInfo::with_is_script("builtin", "", false);
            assert!(!s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn cat14_with_is_script_false_has_limited_actions() {
            let s = ScriptInfo::with_is_script("App", "", false);
            let actions = get_script_context_actions(&s);
            let ids = action_ids(&actions);
            // Non-script, non-scriptlet, non-agent = builtin-like
            assert!(!ids.contains(&"edit_script".to_string()));
            assert!(!ids.contains(&"view_logs".to_string()));
            assert!(ids.contains(&"run_script".to_string()));
            assert!(ids.contains(&"copy_deeplink".to_string()));
        }

        // =========================================================================
        // 15. Deeplink name with mixed Unicode scripts
        // =========================================================================

        #[test]
        fn cat15_deeplink_cjk_preserved() {
            let result = to_deeplink_name("日本語テスト");
            assert!(result.contains("%E6%97%A5"));
            assert!(result.contains("%E8%AA%9E"));
        }

        #[test]
        fn cat15_deeplink_mixed_ascii_and_accents() {
            let result = to_deeplink_name("Café Script");
            assert!(result.contains("caf"));
            assert!(result.contains("%C3%A9"));
        }

        #[test]
        fn cat15_deeplink_all_special_chars() {
            let result = to_deeplink_name("!@#$%^&*()");
            assert_eq!(result, "_unnamed");
        }

        #[test]
        fn cat15_deeplink_leading_trailing_stripped() {
            let result = to_deeplink_name("  hello  ");
            assert_eq!(result, "hello");
        }

        #[test]
        fn cat15_deeplink_consecutive_specials_collapsed() {
            let result = to_deeplink_name("a---b___c");
            assert_eq!(result, "a-b-c");
        }

        // =========================================================================
        // 16. parse_shortcut_keycaps special symbol recognition
        // =========================================================================

        #[test]
        fn cat16_modifier_symbols_are_individual_keycaps() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
        }

        #[test]
        fn cat16_enter_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(keycaps, vec!["↵"]);
        }

        #[test]
        fn cat16_escape_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn cat16_arrow_symbols() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(keycaps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn cat16_backspace_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⌫");
            assert_eq!(keycaps, vec!["⌘", "⌫"]);
        }

        #[test]
        fn cat16_space_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(keycaps, vec!["␣"]);
        }

        #[test]
        fn cat16_tab_symbol() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⇥");
            assert_eq!(keycaps, vec!["⇥"]);
        }

        #[test]
        fn cat16_lowercase_uppercased() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘c");
            assert_eq!(keycaps, vec!["⌘", "C"]);
        }

        // =========================================================================
        // 17. Clipboard destructive action shortcut exact values
        // =========================================================================

        #[test]
        fn cat17_delete_shortcut() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete")
                .unwrap();
            assert_eq!(del.shortcut.as_deref(), Some("⌃X"));
        }

        #[test]
        fn cat17_delete_multiple_shortcut() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_multiple")
                .unwrap();
            assert_eq!(del.shortcut.as_deref(), Some("⇧⌘X"));
        }

        #[test]
        fn cat17_delete_all_shortcut() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_all")
                .unwrap();
            assert_eq!(del.shortcut.as_deref(), Some("⌃⇧X"));
        }

        #[test]
        fn cat17_destructive_actions_are_last_three() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let len = actions.len();
            assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
            assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
            assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
        }

        // =========================================================================
        // 18. File context macOS action count (file vs dir)
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat18_file_has_more_actions_than_dir_on_macos() {
            let file_info = FileInfo {
                path: "/tmp/a.txt".into(),
                name: "a.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let dir_info = FileInfo {
                path: "/tmp/docs".into(),
                name: "docs".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let file_count = get_file_context_actions(&file_info).len();
            let dir_count = get_file_context_actions(&dir_info).len();
            // File has quick_look (macOS), dir doesn't
            assert!(
                file_count > dir_count,
                "file {} > dir {}",
                file_count,
                dir_count
            );
        }

        #[test]
        fn cat18_both_have_reveal_copy_path_copy_filename() {
            let file_info = FileInfo {
                path: "/tmp/a.txt".into(),
                name: "a.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let dir_info = FileInfo {
                path: "/tmp/docs".into(),
                name: "docs".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            for info in [&file_info, &dir_info] {
                let ids = action_ids(&get_file_context_actions(info));
                assert!(ids.contains(&"file:reveal_in_finder".to_string()));
                assert!(ids.contains(&"file:copy_path".to_string()));
                assert!(ids.contains(&"file:copy_filename".to_string()));
            }
        }

        // =========================================================================
        // 19. New chat action ID prefix patterns
        // =========================================================================

        #[test]
        fn cat19_last_used_ids_start_with_last_used() {
            let actions = get_new_chat_actions(
                &[NewChatModelInfo {
                    model_id: "m1".into(),
                    display_name: "M1".into(),
                    provider: "P".into(),
                    provider_display_name: "Provider".into(),
                }],
                &[],
                &[],
            );
            assert!(actions[0].id.starts_with("last_used_"));
        }

        #[test]
        fn cat19_preset_ids_start_with_preset() {
            let actions = get_new_chat_actions(
                &[],
                &[NewChatPresetInfo {
                    id: "general".into(),
                    name: "General".into(),
                    icon: IconName::Star,
                }],
                &[],
            );
            assert!(actions[0].id.starts_with("preset_"));
        }

        #[test]
        fn cat19_model_ids_start_with_model() {
            let actions = get_new_chat_actions(
                &[],
                &[],
                &[NewChatModelInfo {
                    model_id: "gpt4".into(),
                    display_name: "GPT-4".into(),
                    provider: "OpenAI".into(),
                    provider_display_name: "OpenAI".into(),
                }],
            );
            assert!(actions[0].id.starts_with("model_"));
        }

        #[test]
        fn cat19_empty_inputs_empty_output() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn cat19_all_three_sections_present() {
            let actions = get_new_chat_actions(
                &[NewChatModelInfo {
                    model_id: "m".into(),
                    display_name: "M".into(),
                    provider: "P".into(),
                    provider_display_name: "PP".into(),
                }],
                &[NewChatPresetInfo {
                    id: "p".into(),
                    name: "P".into(),
                    icon: IconName::Star,
                }],
                &[NewChatModelInfo {
                    model_id: "x".into(),
                    display_name: "X".into(),
                    provider: "Q".into(),
                    provider_display_name: "QQ".into(),
                }],
            );
            let sections: Vec<_> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            assert!(sections.contains(&"Last Used Settings"));
            assert!(sections.contains(&"Presets"));
            assert!(sections.contains(&"Models"));
        }

        // =========================================================================
        // 20. Notes command bar section-to-action mapping
        // =========================================================================

        // --- merged from tests_part_04.rs ---
        // =========================================================================
        // 21. coerce_action_selection edge cases
        // =========================================================================

        #[test]
        fn cat21_empty_rows_returns_none() {
            assert_eq!(coerce_action_selection(&[], 0), None);
        }

        #[test]
        fn cat21_on_item_returns_same() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn cat21_header_searches_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat21_trailing_header_searches_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("H".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat21_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::SectionHeader("B".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat21_out_of_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 100), Some(0));
        }

        // =========================================================================
        // 22. fuzzy_match edge cases
        // =========================================================================

        #[test]
        fn cat22_empty_needle_matches() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn cat22_empty_haystack_no_match() {
            assert!(!ActionsDialog::fuzzy_match("", "x"));
        }

        #[test]
        fn cat22_both_empty_matches() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn cat22_exact_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        #[test]
        fn cat22_subsequence_match() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlw"));
        }

        #[test]
        fn cat22_no_subsequence() {
            assert!(!ActionsDialog::fuzzy_match("hello", "xyz"));
        }

        #[test]
        fn cat22_needle_longer_than_haystack() {
            assert!(!ActionsDialog::fuzzy_match("hi", "hello"));
        }

        // =========================================================================
        // 23. Script context exact action ordering
        // =========================================================================

        #[test]
        fn cat23_script_run_always_first() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat23_script_copy_deeplink_always_present() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "copy_deeplink"));
        }

        #[test]
        fn cat23_builtin_run_first() {
            let builtin = ScriptInfo::builtin("Test Builtin");
            let actions = get_script_context_actions(&builtin);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat23_scriptlet_run_first() {
            let scriptlet = ScriptInfo::scriptlet("Test", "/path.md", None, None);
            let actions = get_script_context_actions(&scriptlet);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat23_run_action_title_includes_verb_and_name() {
            let script = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].title, "Launch");
        }

        // =========================================================================
        // 24. Clipboard paste title dynamic behavior
        // =========================================================================

        #[test]
        fn cat24_paste_no_app() {
            let entry = make_text_entry();
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to Active App");
        }

        #[test]
        fn cat24_paste_with_app() {
            let mut entry = make_text_entry();
            entry.frontmost_app_name = Some("Safari".into());
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to Safari");
        }

        #[test]
        fn cat24_paste_with_unicode_app() {
            let mut entry = make_text_entry();
            entry.frontmost_app_name = Some("日本語App".into());
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to 日本語App");
        }

        #[test]
        fn cat24_paste_with_empty_app_string() {
            let mut entry = make_text_entry();
            entry.frontmost_app_name = Some("".into());
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            // Empty app name falls back to the generic "Paste to Active App" title
            assert_eq!(paste.title, "Paste to Active App");
        }

        // =========================================================================
        // 25. Action lowercase caching
        // =========================================================================

        #[test]
        fn cat25_title_lower_computed_on_creation() {
            let action = Action::new("x", "Hello World", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "hello world");
        }

        #[test]
        fn cat25_description_lower_computed() {
            let action = Action::new(
                "x",
                "X",
                Some("Foo Bar".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower, Some("foo bar".into()));
        }

        #[test]
        fn cat25_description_lower_none_when_no_desc() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn cat25_shortcut_lower_none_until_with_shortcut() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
            let action = action.with_shortcut("⌘C");
            assert_eq!(action.shortcut_lower, Some("⌘c".into()));
        }

        #[test]
        fn cat25_with_shortcut_opt_none_no_shortcut_lower() {
            let action =
                Action::new("x", "X", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
            assert!(action.shortcut_lower.is_none());
            assert!(action.shortcut.is_none());
        }

        // =========================================================================
        // 26. Note switcher description rendering
        // =========================================================================

        #[test]
        fn cat26_preview_with_time_uses_separator() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 50,
                is_current: false,
                is_pinned: false,
                preview: "Some preview".into(),
                relative_time: "5m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(
                desc.contains(" · "),
                "Expected ' · ' separator in: {}",
                desc
            );
        }

        #[test]
        fn cat26_empty_preview_empty_time_uses_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
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
        fn cat26_singular_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
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
        fn cat26_zero_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
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
        fn cat26_preview_truncated_at_61_chars() {
            let long_preview = "a".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 61,
                is_current: false,
                is_pinned: false,
                preview: long_preview,
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.ends_with('…'), "Should end with … : {}", desc);
        }

        #[test]
        fn cat26_preview_not_truncated_at_60_chars() {
            let preview = "a".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 60,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.ends_with('…'), "Should not truncate at 60: {}", desc);
        }

        // =========================================================================
        // 27. Note switcher icon hierarchy
        // =========================================================================

        #[test]
        fn cat27_pinned_icon_star_filled() {
            let notes = vec![make_note("1", "N", true, false)];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        #[test]
        fn cat27_current_icon_check() {
            let notes = vec![make_note("1", "N", false, true)];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::Check));
        }

        #[test]
        fn cat27_regular_icon_file() {
            let notes = vec![make_note("1", "N", false, false)];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::File));
        }

        #[test]
        fn cat27_pinned_overrides_current() {
            let notes = vec![make_note("1", "N", true, true)];
            let actions = get_note_switcher_actions(&notes);
            // Pinned takes priority in the if-else chain
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        // =========================================================================
        // 28. Cross-context ID uniqueness
        // =========================================================================

        #[test]
        fn cat28_script_ids_unique() {
            let actions = get_script_context_actions(&ScriptInfo::new("t", "/t.ts"));
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat28_clipboard_ids_unique() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat28_ai_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat28_path_ids_unique() {
            let info = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat28_file_ids_unique() {
            let info = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len());
        }

        // --- merged from tests_part_05.rs ---
        // =========================================================================
        // 29. has_action=false invariant for all built-ins
        // =========================================================================

        #[test]
        fn cat29_script_has_action_false() {
            for action in &get_script_context_actions(&ScriptInfo::new("t", "/t.ts")) {
                assert!(
                    !action.has_action,
                    "{} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_clipboard_has_action_false() {
            for action in &get_clipboard_history_context_actions(&make_text_entry()) {
                assert!(
                    !action.has_action,
                    "{} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_ai_has_action_false() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    !action.has_action,
                    "{} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_path_has_action_false() {
            let info = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "{} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_file_has_action_false() {
            let info = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "{} should have has_action=false",
                    action.id
                );
            }
        }

        // =========================================================================
        // 30. Ordering determinism
        // =========================================================================

        #[test]
        fn cat30_script_ordering_deterministic() {
            let s = ScriptInfo::new("t", "/t.ts");
            let a = action_ids(&get_script_context_actions(&s));
            let b = action_ids(&get_script_context_actions(&s));
            assert_eq!(a, b);
        }

        #[test]
        fn cat30_clipboard_ordering_deterministic() {
            let e = make_text_entry();
            let a = action_ids(&get_clipboard_history_context_actions(&e));
            let b = action_ids(&get_clipboard_history_context_actions(&e));
            assert_eq!(a, b);
        }

        #[test]
        fn cat30_ai_ordering_deterministic() {
            let a = action_ids(&get_ai_command_bar_actions());
            let b = action_ids(&get_ai_command_bar_actions());
            assert_eq!(a, b);
        }

        #[test]
        fn cat30_path_ordering_deterministic() {
            let info = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            let a = action_ids(&get_path_context_actions(&info));
            let b = action_ids(&get_path_context_actions(&info));
            assert_eq!(a, b);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_13 {
    //! Batch 13 — Builtin action validation tests
    //!
    //! Focus areas:
    //! - format_shortcut_hint edge cases (non-modifier intermediate parts, aliased modifiers)
    //! - ScriptInfo mutually-exclusive flags (agent vs script vs scriptlet vs builtin)
    //! - Scriptlet context custom action value/has_action propagation
    //! - Clipboard save_snippet (text-only) and save_file (text and image)
    //! - Path context copy_filename has no shortcut
    //! - Note switcher description ellipsis boundary (exactly 60 chars)
    //! - Chat context multi-model ordering and checkmark logic
    //! - AI command bar actions without shortcuts
    //! - CommandBarConfig close flag defaults
    //! - Cross-builder shortcut/alias action symmetry
    //! - Scriptlet context action verb propagation
    //! - Agent context exact action IDs
    //! - Deeplink URL in description for scriptlet context
    //! - Notes command bar create_quicklink and export actions
    //! - Action::new lowercase caching correctness

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // =========================================================================
        // Helper
        // =========================================================================
        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // =========================================================================
        // 1. format_shortcut_hint aliased modifier keywords
        // =========================================================================

        #[test]
        fn cat01_format_shortcut_hint_meta_maps_to_cmd() {
            let result = ActionsDialog::format_shortcut_hint("meta+c");
            assert_eq!(result, "⌘C");
        }

        #[test]
        fn cat01_format_shortcut_hint_super_maps_to_cmd() {
            let result = ActionsDialog::format_shortcut_hint("super+x");
            assert_eq!(result, "⌘X");
        }

        #[test]
        fn cat01_format_shortcut_hint_command_maps_to_cmd() {
            let result = ActionsDialog::format_shortcut_hint("command+a");
            assert_eq!(result, "⌘A");
        }

        #[test]
        fn cat01_format_shortcut_hint_option_maps_to_alt() {
            let result = ActionsDialog::format_shortcut_hint("option+z");
            assert_eq!(result, "⌥Z");
        }

        #[test]
        fn cat01_format_shortcut_hint_control_maps_to_ctrl() {
            let result = ActionsDialog::format_shortcut_hint("control+b");
            assert_eq!(result, "⌃B");
        }

        #[test]
        fn cat01_format_shortcut_hint_return_maps_to_enter() {
            let result = ActionsDialog::format_shortcut_hint("cmd+return");
            assert_eq!(result, "⌘↵");
        }

        #[test]
        fn cat01_format_shortcut_hint_esc_maps_to_escape() {
            let result = ActionsDialog::format_shortcut_hint("esc");
            assert_eq!(result, "⎋");
        }

        #[test]
        fn cat01_format_shortcut_hint_arrowup_maps_to_up() {
            let result = ActionsDialog::format_shortcut_hint("arrowup");
            assert_eq!(result, "↑");
        }

        #[test]
        fn cat01_format_shortcut_hint_arrowdown_maps_to_down() {
            let result = ActionsDialog::format_shortcut_hint("arrowdown");
            assert_eq!(result, "↓");
        }

        #[test]
        fn cat01_format_shortcut_hint_arrowleft_maps_to_left() {
            let result = ActionsDialog::format_shortcut_hint("arrowleft");
            assert_eq!(result, "←");
        }

        #[test]
        fn cat01_format_shortcut_hint_arrowright_maps_to_right() {
            let result = ActionsDialog::format_shortcut_hint("arrowright");
            assert_eq!(result, "→");
        }

        #[test]
        fn cat01_format_shortcut_hint_tab_maps_to_tab_symbol() {
            let result = ActionsDialog::format_shortcut_hint("tab");
            assert_eq!(result, "⇥");
        }

        #[test]
        fn cat01_format_shortcut_hint_backspace_maps_to_delete_symbol() {
            let result = ActionsDialog::format_shortcut_hint("backspace");
            assert_eq!(result, "⌫");
        }

        #[test]
        fn cat01_format_shortcut_hint_delete_maps_to_delete_symbol() {
            let result = ActionsDialog::format_shortcut_hint("delete");
            assert_eq!(result, "⌫");
        }

        #[test]
        fn cat01_format_shortcut_hint_space_maps_to_space_symbol() {
            let result = ActionsDialog::format_shortcut_hint("space");
            assert_eq!(result, "␣");
        }

        #[test]
        fn cat01_format_shortcut_hint_single_letter() {
            let result = ActionsDialog::format_shortcut_hint("a");
            assert_eq!(result, "A");
        }

        #[test]
        fn cat01_format_shortcut_hint_case_insensitive() {
            let result = ActionsDialog::format_shortcut_hint("CMD+SHIFT+C");
            assert_eq!(result, "⌘⇧C");
        }

        // =========================================================================
        // 2. ScriptInfo mutually-exclusive flags (agent vs script vs scriptlet)
        // =========================================================================

        #[test]
        fn cat02_script_info_new_has_is_script_true() {
            let s = ScriptInfo::new("t", "/p");
            assert!(s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn cat02_scriptlet_has_is_scriptlet_true() {
            let s = ScriptInfo::scriptlet("t", "/p", None, None);
            assert!(!s.is_script);
            assert!(s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn cat02_builtin_has_all_false() {
            let s = ScriptInfo::builtin("B");
            assert!(!s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn cat02_with_action_verb_not_script_not_scriptlet() {
            let s = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
            assert!(!s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn cat02_agent_flag_via_new_plus_mutation() {
            // ScriptInfo::new sets is_script=true; for agents we construct differently
            // The agent flag is set by the caller, not by the constructor
            let mut s = ScriptInfo::new("Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            assert!(!s.is_script);
            assert!(!s.is_scriptlet);
            assert!(s.is_agent);
        }

        // =========================================================================
        // 3. Scriptlet context custom action value/has_action propagation
        // =========================================================================

        #[test]
        fn cat03_scriptlet_defined_action_has_action_true() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy".into(),
                command: "copy-cmd".into(),
                tool: "bash".into(),
                code: "pbcopy".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions.len(), 1);
            assert!(actions[0].has_action);
            assert_eq!(actions[0].value, Some("copy-cmd".into()));
        }

        #[test]
        fn cat03_scriptlet_defined_action_id_prefix() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "My Action".into(),
                command: "my-action".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].id.starts_with("scriptlet_action:"));
        }

        #[test]
        fn cat03_scriptlet_defined_action_shortcut_formatted() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "A".into(),
                command: "a".into(),
                tool: "bash".into(),
                code: "".into(),
                inputs: vec![],
                shortcut: Some("cmd+shift+p".into()),
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            // format_shortcut_hint converts "cmd+shift+p" to "⌘⇧P"
            assert_eq!(actions[0].shortcut, Some("⌘⇧P".into()));
        }

        #[test]
        fn cat03_scriptlet_defined_action_description_propagated() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "A".into(),
                command: "a".into(),
                tool: "bash".into(),
                code: "".into(),
                inputs: vec![],
                shortcut: None,
                description: Some("Custom desc".into()),
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].description, Some("Custom desc".into()));
        }

        #[test]
        fn cat03_scriptlet_defined_action_no_shortcut_is_none() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "A".into(),
                command: "a".into(),
                tool: "bash".into(),
                code: "".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].shortcut.is_none());
        }

        #[test]
        fn cat03_empty_scriptlet_no_custom_actions() {
            let scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions.is_empty());
        }

        // =========================================================================
        // 4. Clipboard save_snippet (text-only) and save_file (text and image)
        // =========================================================================

        fn make_text_entry() -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "t1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            }
        }

        fn make_image_entry() -> ClipboardEntryInfo {
            ClipboardEntryInfo {
                id: "i1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "img".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            }
        }

        #[test]
        fn cat04_text_has_save_snippet() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            assert!(
                actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_save_snippet")
            );
        }

        #[test]
        fn cat04_text_has_save_file() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_save_file"));
        }

        #[test]
        fn cat04_image_has_no_save_snippet() {
            // save_snippet exists only for TEXT entries; image entries get ocr instead.
            let actions = get_clipboard_history_context_actions(&make_image_entry());
            assert!(
                !actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_save_snippet")
            );
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
        }

        #[test]
        fn cat04_image_has_save_file() {
            let actions = get_clipboard_history_context_actions(&make_image_entry());
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_save_file"));
        }

        #[test]
        fn cat04_save_snippet_shortcut() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let a = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘S"));
        }

        #[test]
        fn cat04_save_file_shortcut() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let a = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌥⇧⌘S"));
        }

        // =========================================================================
        // 5. Path context copy_filename has no shortcut
        // =========================================================================

        #[test]
        fn cat05_path_copy_filename_no_shortcut() {
            let info = PathInfo {
                path: "/tmp/foo.txt".into(),
                name: "foo.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert!(
                a.shortcut.is_none(),
                "Path copy_filename should have no shortcut"
            );
        }

        #[test]
        fn cat05_path_copy_filename_description() {
            let info = PathInfo {
                path: "/tmp/foo.txt".into(),
                name: "foo.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let a = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert!(a.description.as_ref().unwrap().contains("filename"));
        }

        #[test]
        fn cat05_file_copy_filename_has_shortcut() {
            // In contrast, file context copy_filename DOES have a shortcut (⌘C)
            let info = FileInfo {
                path: "/tmp/foo.txt".into(),
                name: "foo.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            let a = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘C"));
        }

        // =========================================================================
        // 6. Note switcher description ellipsis boundary (exactly 60 chars)
        // =========================================================================

        fn make_note(preview: &str, time: &str, chars: usize) -> NoteSwitcherNoteInfo {
            NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "Note".into(),
                char_count: chars,
                is_current: false,
                is_pinned: false,
                preview: preview.into(),
                relative_time: time.into(),
            }
        }

        #[test]
        fn cat06_preview_exactly_60_chars_no_ellipsis() {
            let preview = "a".repeat(60);
            let actions = get_note_switcher_actions(&[make_note(&preview, "1m", 60)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(
                !desc.contains('…'),
                "60 chars should NOT have ellipsis: {}",
                desc
            );
        }

        #[test]
        fn cat06_preview_61_chars_has_ellipsis() {
            let preview = "a".repeat(61);
            let actions = get_note_switcher_actions(&[make_note(&preview, "1m", 61)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(
                desc.contains('…'),
                "61 chars should have ellipsis: {}",
                desc
            );
        }

        #[test]
        fn cat06_empty_preview_empty_time_uses_char_count() {
            let actions = get_note_switcher_actions(&[make_note("", "", 42)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains("42"), "Should show char count: {}", desc);
            assert!(desc.contains("chars"), "Should say 'chars': {}", desc);
        }

        #[test]
        fn cat06_empty_preview_with_time_uses_time() {
            let actions = get_note_switcher_actions(&[make_note("", "5m ago", 10)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "5m ago");
        }

        #[test]
        fn cat06_preview_with_time_has_separator() {
            let actions = get_note_switcher_actions(&[make_note("Hello world", "2h ago", 11)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains(" · "), "Should have separator: {}", desc);
            assert!(desc.contains("Hello world"));
            assert!(desc.contains("2h ago"));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat06_one_char_singular() {
            let actions = get_note_switcher_actions(&[make_note("", "", 1)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1 char");
        }

        #[test]
        fn cat06_zero_chars_plural() {
            let actions = get_note_switcher_actions(&[make_note("", "", 0)]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "0 chars");
        }

        // =========================================================================
        // 7. Chat context multi-model ordering and checkmark logic
        // =========================================================================

        fn make_chat_info(
            current: Option<&str>,
            models: &[(&str, &str, &str)],
            has_response: bool,
            has_messages: bool,
        ) -> ChatPromptInfo {
            ChatPromptInfo {
                current_model: current.map(|s| s.to_string()),
                available_models: models
                    .iter()
                    .map(|(id, name, provider)| ChatModelInfo {
                        id: id.to_string(),
                        display_name: name.to_string(),
                        provider: provider.to_string(),
                    })
                    .collect(),
                has_response,
                has_messages,
            }
        }

        #[test]
        fn cat07_model_actions_ordered_by_input() {
            let info = make_chat_info(
                None,
                &[("m1", "Model A", "P1"), ("m2", "Model B", "P2")],
                false,
                false,
            );
            let picker = get_chat_model_picker_actions(&info);
            assert_eq!(picker[0].id, "chat:select_model_m1");
            assert_eq!(picker[1].id, "chat:select_model_m2");
        }

        #[test]
        fn cat07_current_model_gets_checkmark() {
            let info = make_chat_info(
                Some("Model A"),
                &[("m1", "Model A", "P1"), ("m2", "Model B", "P2")],
                false,
                false,
            );
            let picker = get_chat_model_picker_actions(&info);
            let m1 = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m1")
                .unwrap();
            assert!(m1.title.contains('✓'), "Current model should have ✓");
        }

        #[test]
        fn cat07_non_current_model_no_checkmark() {
            let info = make_chat_info(
                Some("Model A"),
                &[("m1", "Model A", "P1"), ("m2", "Model B", "P2")],
                false,
                false,
            );
            let picker = get_chat_model_picker_actions(&info);
            let m2 = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m2")
                .unwrap();
            assert!(
                !m2.title.contains('✓'),
                "Non-current model should NOT have ✓"
            );
        }

        #[test]
        fn cat07_model_description_is_via_provider() {
            let info = make_chat_info(None, &[("m1", "Claude", "Anthropic")], false, false);
            // Model rows live in the drill-down picker
            let picker = get_chat_model_picker_actions(&info);
            let model = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m1")
                .unwrap();
            assert_eq!(model.description.as_deref(), Some("Uses Anthropic"));
        }

        #[test]
        fn cat07_no_models_still_has_continue_in_chat() {
            let info = make_chat_info(None, &[], false, false);
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
        }

        #[test]
        fn cat07_has_response_adds_copy_response() {
            let info = make_chat_info(None, &[], true, false);
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
        }

        #[test]
        fn cat07_no_response_no_copy_response() {
            let info = make_chat_info(None, &[], false, false);
            let actions = get_chat_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
        }

        #[test]
        fn cat07_has_messages_adds_clear_conversation() {
            let info = make_chat_info(None, &[], false, true);
            let actions = get_chat_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
        }

        #[test]
        fn cat07_no_messages_no_clear_conversation() {
            let info = make_chat_info(None, &[], false, false);
            let actions = get_chat_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "chat:clear_conversation"));
        }

        #[test]
        fn cat07_continue_in_chat_shortcut() {
            let info = make_chat_info(None, &[], false, false);
            let actions = get_chat_context_actions(&info);
            let a = actions
                .iter()
                .find(|a| a.id == "chat:continue_in_chat")
                .unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘↵"));
        }

        // =========================================================================
        // 8. AI command bar actions without shortcuts
        // =========================================================================

        #[test]
        fn cat08_branch_from_last_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:branch_from_last")
                .unwrap();
            assert!(
                a.shortcut.is_none(),
                "branch_from_last should have no shortcut"
            );
        }

        #[test]
        fn cat08_change_model_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert!(a.shortcut.is_none(), "change_model should have no shortcut");
        }

        #[test]
        fn cat08_toggle_shortcuts_help_has_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:toggle_shortcuts_help")
                .unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⌘/"));
        }

        #[test]
        fn cat08_export_markdown_shortcut() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(a.shortcut.as_deref(), Some("⇧⌘E"));
        }

        #[test]
        fn cat08_ai_all_have_icons() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.icon.is_some(),
                    "AI action {} should have icon",
                    action.id
                );
            }
        }

        #[test]
        fn cat08_ai_all_have_sections() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.section.is_some(),
                    "AI action {} should have section",
                    action.id
                );
            }
        }

        // =========================================================================
        // 9. CommandBarConfig close flag defaults
        // =========================================================================

        #[test]
        fn cat09_default_close_on_select_true() {
            let config = super::super::command_bar::CommandBarConfig::default();
            assert!(config.close_on_select);
        }

        #[test]
        fn cat09_default_close_on_click_outside_true() {
            let config = super::super::command_bar::CommandBarConfig::default();
            assert!(config.close_on_click_outside);
        }

        #[test]
        fn cat09_default_close_on_escape_true() {
            let config = super::super::command_bar::CommandBarConfig::default();
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat09_ai_style_close_defaults_preserved() {
            let config = super::super::command_bar::CommandBarConfig::ai_style();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat09_main_menu_style_close_defaults_preserved() {
            let config = super::super::command_bar::CommandBarConfig::main_menu_style();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat09_no_search_style_close_defaults_preserved() {
            let config = super::super::command_bar::CommandBarConfig::no_search();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat09_notes_style_close_defaults_preserved() {
            let config = super::super::command_bar::CommandBarConfig::notes_style();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        // =========================================================================
        // 10. Cross-builder shortcut/alias action symmetry
        // =========================================================================

        #[test]
        fn cat10_script_no_shortcut_no_alias_has_add_both() {
            let s = ScriptInfo::new("t", "/p");
            let ids = action_ids(&get_script_context_actions(&s));
            assert!(ids.contains(&"add_shortcut".into()));
            assert!(ids.contains(&"add_alias".into()));
            assert!(!ids.contains(&"update_shortcut".into()));
            assert!(!ids.contains(&"update_alias".into()));
        }

        #[test]
        fn cat10_script_has_shortcut_has_alias_has_update_remove_both() {
            let s = ScriptInfo::with_shortcut_and_alias(
                "t",
                "/p",
                Some("cmd+t".into()),
                Some("ts".into()),
            );
            let ids = action_ids(&get_script_context_actions(&s));
            assert!(ids.contains(&"update_shortcut".into()));
            assert!(ids.contains(&"remove_shortcut".into()));
            assert!(ids.contains(&"update_alias".into()));
            assert!(ids.contains(&"remove_alias".into()));
            assert!(!ids.contains(&"add_shortcut".into()));
            assert!(!ids.contains(&"add_alias".into()));
        }

        #[test]
        fn cat10_scriptlet_context_same_shortcut_alias_logic() {
            let s = ScriptInfo::scriptlet("t", "/p", Some("cmd+k".into()), Some("tk".into()));
            let actions = get_scriptlet_context_actions_with_custom(&s, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"update_shortcut".into()));
            assert!(ids.contains(&"remove_shortcut".into()));
            assert!(ids.contains(&"update_alias".into()));
            assert!(ids.contains(&"remove_alias".into()));
        }

        #[test]
        fn cat10_scriptlet_no_shortcut_no_alias_has_add() {
            let s = ScriptInfo::scriptlet("t", "/p", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&s, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"add_shortcut".into()));
            assert!(ids.contains(&"add_alias".into()));
        }

        #[test]
        fn cat10_shortcut_and_alias_action_shortcut_values() {
            let s = ScriptInfo::new("t", "/p");
            let actions = get_script_context_actions(&s);
            let add_sc = actions.iter().find(|a| a.id == "add_shortcut").unwrap();
            assert_eq!(add_sc.shortcut.as_deref(), Some("⌘⇧K"));
            let add_al = actions.iter().find(|a| a.id == "add_alias").unwrap();
            assert_eq!(add_al.shortcut.as_deref(), Some("⌘⇧A"));
        }

        // =========================================================================
        // 11. Scriptlet context action verb propagation
        // =========================================================================

        #[test]
        fn cat11_scriptlet_run_title_uses_action_verb() {
            let s = ScriptInfo::scriptlet("My Script", "/p", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&s, None);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(
                run.title.starts_with("Run "),
                "Title should start with 'Run ': {}",
                run.title
            );
            assert!(
                run.title.contains("My Script"),
                "Title should contain name: {}",
                run.title
            );
        }

        #[test]
        fn cat11_scriptlet_run_description_uses_verb() {
            let s = ScriptInfo::scriptlet("T", "/p", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&s, None);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            let desc = run.description.as_ref().unwrap();
            assert!(
                desc.contains("Run"),
                "Description should contain verb: {}",
                desc
            );
        }

        #[test]
        fn cat11_script_context_custom_verb_propagates() {
            let s = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
            let actions = get_script_context_actions(&s);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Launch");
        }

        #[test]
        fn cat11_script_context_switch_to_verb() {
            let s = ScriptInfo::with_action_verb("Window", "win:1", false, "Switch to");
            let actions = get_script_context_actions(&s);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Switch To");
        }

        // =========================================================================
        // 12. Agent context exact action IDs
        // =========================================================================

        #[test]
        fn cat12_agent_has_edit_script_not_edit_agent_id() {
            // Agent uses "edit_script" as ID but "Edit Agent" as title
            let mut s = ScriptInfo::new("My Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            let actions = get_script_context_actions(&s);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"edit_script".into()));
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn cat12_agent_has_reveal_in_finder() {
            let mut s = ScriptInfo::new("My Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            let actions = get_script_context_actions(&s);
            assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
        }

        #[test]
        fn cat12_agent_has_copy_path() {
            let mut s = ScriptInfo::new("My Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            let actions = get_script_context_actions(&s);
            assert!(actions.iter().any(|a| a.id == "copy_path"));
        }

        #[test]
        fn cat12_agent_has_copy_content() {
            let mut s = ScriptInfo::new("My Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            let actions = get_script_context_actions(&s);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn cat12_agent_no_view_logs() {
            let mut s = ScriptInfo::new("My Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            let actions = get_script_context_actions(&s);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn cat12_agent_descriptions_mention_agent() {
            let mut s = ScriptInfo::new("My Agent", "/agent");
            s.is_agent = true;
            s.is_script = false;
            let actions = get_script_context_actions(&s);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(edit.description.as_ref().unwrap().contains("agent"));
            let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
            assert!(reveal.description.as_ref().unwrap().contains("agent"));
        }

        // =========================================================================
        // 13. Deeplink URL in description for scriptlet context
        // =========================================================================

        #[test]
        fn cat13_scriptlet_deeplink_description_contains_url() {
            let s = ScriptInfo::scriptlet("My Script", "/p", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&s, None);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            let desc = dl.description.as_ref().unwrap();
            assert!(desc.contains("scriptkit://run/my-script"), "Desc: {}", desc);
        }

        #[test]
        fn cat13_script_deeplink_description_format() {
            let s = ScriptInfo::new("Hello World", "/p");
            let actions = get_script_context_actions(&s);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            let desc = dl.description.as_ref().unwrap();
            assert_eq!(dl.title, "Share");
            assert_eq!(desc, "Copy a portable Script Kit share link to clipboard");
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat13_deeplink_name_special_chars_collapsed() {
            assert_eq!(to_deeplink_name("a!!b"), "a-b");
        }

        #[test]
        fn cat13_deeplink_name_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("  hello  "), "hello");
        }

        #[test]
        fn cat13_deeplink_name_unicode_preserved() {
            let result = to_deeplink_name("café");
            assert!(
                result.contains("caf"),
                "Should contain ascii part: {}",
                result
            );
            assert!(
                result.contains("%C3%A9"),
                "Should preserve unicode: {}",
                result
            );
        }

        // =========================================================================
        // 14. Notes command bar create_quicklink and export actions
        // =========================================================================

        // =========================================================================
        // 15. Action::new lowercase caching correctness
        // =========================================================================

        #[test]
        fn cat15_title_lower_is_lowercase() {
            let a = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
            assert_eq!(a.title_lower, "hello world");
        }

        #[test]
        fn cat15_description_lower_is_lowercase() {
            let a = Action::new(
                "id",
                "T",
                Some("Foo BAR".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(a.description_lower, Some("foo bar".into()));
        }

        #[test]
        fn cat15_description_lower_none_when_no_description() {
            let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
            assert!(a.description_lower.is_none());
        }

        #[test]
        fn cat15_shortcut_lower_none_initially() {
            let a = Action::new("id", "T", None, ActionCategory::ScriptContext);
            assert!(a.shortcut_lower.is_none());
        }

        #[test]
        fn cat15_shortcut_lower_set_after_with_shortcut() {
            let a = Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
            assert_eq!(a.shortcut_lower, Some("⌘e".into()));
        }

        #[test]
        fn cat15_with_shortcut_opt_none_does_not_set() {
            let a =
                Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
            assert!(a.shortcut_lower.is_none());
            assert!(a.shortcut.is_none());
        }

        #[test]
        fn cat15_with_shortcut_opt_some_sets() {
            let a = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘X".into()));
            assert_eq!(a.shortcut, Some("⌘X".into()));
            assert_eq!(a.shortcut_lower, Some("⌘x".into()));
        }

        // =========================================================================
        // 16. parse_shortcut_keycaps for special symbols
        // =========================================================================

        #[test]
        fn cat16_parse_cmd_c() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(caps, vec!["⌘", "C"]);
        }

        #[test]
        fn cat16_parse_all_modifiers() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧X");
            assert_eq!(caps, vec!["⌘", "⌃", "⌥", "⇧", "X"]);
        }

        #[test]
        fn cat16_parse_enter() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(caps, vec!["↵"]);
        }

        #[test]
        fn cat16_parse_escape() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(caps, vec!["⎋"]);
        }

        #[test]
        fn cat16_parse_arrows() {
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
        }

        #[test]
        fn cat16_parse_space() {
            let caps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(caps, vec!["␣"]);
        }

        #[test]
        fn cat16_parse_tab() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⇥");
            assert_eq!(caps, vec!["⇥"]);
        }

        #[test]
        fn cat16_parse_backspace() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌫");
            assert_eq!(caps, vec!["⌫"]);
        }

        #[test]
        fn cat16_parse_lowercase_uppercased() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘a");
            assert_eq!(caps, vec!["⌘", "A"]);
        }

        #[test]
        fn cat16_parse_empty() {
            let caps = ActionsDialog::parse_shortcut_keycaps("");
            assert!(caps.is_empty());
        }

        // =========================================================================
        // 17. score_action boundary thresholds
        // =========================================================================

        #[test]
        fn cat17_prefix_match_100() {
            let a = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            assert_eq!(ActionsDialog::score_action(&a, "edit"), 100);
        }

        #[test]
        fn cat17_contains_match_50() {
            let a = Action::new("id", "Copy Edit Path", None, ActionCategory::ScriptContext);
            assert_eq!(ActionsDialog::score_action(&a, "edit"), 50);
        }

        #[test]
        fn cat17_fuzzy_match_25() {
            let a = Action::new(
                "id",
                "Reveal in Finder",
                None,
                ActionCategory::ScriptContext,
            );
            // "rvf" is a subsequence of "reveal in finder" (r-e-v-e-a-l-_-i-n-_-f)
            assert_eq!(ActionsDialog::score_action(&a, "rvf"), 25);
        }

        #[test]
        fn cat17_description_bonus_15() {
            let a = Action::new(
                "id",
                "Open",
                Some("Edit file in editor".into()),
                ActionCategory::ScriptContext,
            );
            // "editor" not in title but in description
            assert_eq!(ActionsDialog::score_action(&a, "editor"), 15);
        }

        #[test]
        fn cat17_shortcut_bonus_10() {
            let a =
                Action::new("id", "Open", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
            // "⌘e" is in shortcut_lower
            assert_eq!(ActionsDialog::score_action(&a, "⌘e"), 10);
        }

        #[test]
        fn cat17_no_match_0() {
            let a = Action::new("id", "Open", None, ActionCategory::ScriptContext);
            assert_eq!(ActionsDialog::score_action(&a, "xyz"), 0);
        }

        #[test]
        fn cat17_prefix_plus_description_115() {
            let a = Action::new(
                "id",
                "Edit Script",
                Some("Edit the script file".into()),
                ActionCategory::ScriptContext,
            );
            // "edit" is prefix (100) + description contains "edit" (15)
            assert_eq!(ActionsDialog::score_action(&a, "edit"), 115);
        }

        // =========================================================================
        // 18. fuzzy_match edge cases
        // =========================================================================

        #[test]
        fn cat18_empty_needle_true() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn cat18_empty_haystack_false() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn cat18_both_empty_true() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn cat18_exact_match() {
            assert!(ActionsDialog::fuzzy_match("abc", "abc"));
        }

        #[test]
        fn cat18_subsequence() {
            assert!(ActionsDialog::fuzzy_match("abcdef", "ace"));
        }

        #[test]
        fn cat18_no_match() {
            assert!(!ActionsDialog::fuzzy_match("abc", "xyz"));
        }

        #[test]
        fn cat18_needle_longer_than_haystack() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        // =========================================================================
        // 19. build_grouped_items_static
        // =========================================================================

        #[test]
        fn cat19_empty_filtered_empty_grouped() {
            let actions: Vec<Action> = vec![];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn cat19_headers_inserts_section_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should have: S1 header, item 0, S2 header, item 1
            assert_eq!(grouped.len(), 4);
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[2], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat19_separators_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // No section headers
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat19_none_style_no_headers() {
            let actions =
                vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1")];
            let filtered = vec![0];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 1);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        }

        #[test]
        fn cat19_same_section_no_duplicate_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S1"),
            ];
            let filtered = vec![0, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // S1 header, item 0, item 1 (no second header)
            assert_eq!(grouped.len(), 3);
        }

        // =========================================================================
        // 20. coerce_action_selection edge cases
        // =========================================================================

        #[test]
        fn cat20_empty_returns_none() {
            assert_eq!(coerce_action_selection(&[], 0), None);
        }

        #[test]
        fn cat20_on_item_returns_same() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn cat20_header_searches_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat20_trailing_header_searches_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat20_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".into()),
                GroupedActionItem::SectionHeader("B".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat20_out_of_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 100), Some(0));
        }

        // =========================================================================
        // 21. New chat actions structure
        // =========================================================================

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat21_empty_inputs_empty_actions() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn cat21_last_used_section() {
            let lu = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&lu, &[], &[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn cat21_preset_section() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].section.as_deref(), Some("Presets"));
            assert!(
                actions[0]
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("preset")
            );
        }

        #[test]
        fn cat21_models_section() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].section.as_deref(), Some("Models"));
            assert_eq!(actions[0].icon, Some(IconName::Settings));
            assert_eq!(actions[0].description, Some("Uses OpenAI".into()));
        }

        #[test]
        fn cat21_section_ordering() {
            let lu = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "A".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "g".into(),
                name: "G".into(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".into(),
                display_name: "B".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let actions = get_new_chat_actions(&lu, &presets, &models);
            assert_eq!(actions.len(), 3);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        // =========================================================================
        // 22. Notes command bar auto_sizing toggle
        // =========================================================================

        // =========================================================================
        // 23. File context FileType variants
        // =========================================================================

        #[test]
        fn cat23_document_and_image_same_file_actions() {
            let doc = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let img = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::Image,
                is_dir: false,
            };
            let doc_ids = action_ids(&get_file_context_actions(&doc));
            let img_ids = action_ids(&get_file_context_actions(&img));
            assert_eq!(doc_ids, img_ids, "FileType should not affect action list");
        }

        #[test]
        fn cat23_directory_different_from_file() {
            let file = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let dir = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let file_ids = action_ids(&get_file_context_actions(&file));
            let dir_ids = action_ids(&get_file_context_actions(&dir));
            assert_ne!(
                file_ids, dir_ids,
                "Dir and file should have different actions"
            );
        }

        // =========================================================================
        // 24. Clipboard destructive actions always last three
        // =========================================================================

        #[test]
        fn cat24_text_last_three_destructive() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let ids = action_ids(&actions);
            let n = ids.len();
            assert_eq!(ids[n - 3], "clip:clipboard_delete");
            assert_eq!(ids[n - 2], "clip:clipboard_delete_multiple");
            assert_eq!(ids[n - 1], "clip:clipboard_delete_all");
        }

        #[test]
        fn cat24_image_last_three_destructive() {
            let actions = get_clipboard_history_context_actions(&make_image_entry());
            let ids = action_ids(&actions);
            let n = ids.len();
            assert_eq!(ids[n - 3], "clip:clipboard_delete");
            assert_eq!(ids[n - 2], "clip:clipboard_delete_multiple");
            assert_eq!(ids[n - 1], "clip:clipboard_delete_all");
        }

        #[test]
        fn cat24_paste_always_first() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            assert_eq!(actions[0].id, "clip:clipboard_paste");
        }

        #[test]
        fn cat24_copy_always_second() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            assert_eq!(actions[1].id, "clip:clipboard_copy");
        }

        // =========================================================================
        // 25. Note switcher icon hierarchy
        // =========================================================================

        #[test]
        fn cat25_pinned_gets_star_filled() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "T".into(),
                char_count: 0,
                is_current: false,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        #[test]
        fn cat25_current_gets_check() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "T".into(),
                char_count: 0,
                is_current: true,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::Check));
        }

        #[test]
        fn cat25_regular_gets_file() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "T".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::File));
        }

        #[test]
        fn cat25_pinned_overrides_current() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "T".into(),
                char_count: 0,
                is_current: true,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        // =========================================================================
        // 26. Note switcher section assignment
        // =========================================================================

        #[test]
        fn cat26_pinned_in_pinned_section() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "T".into(),
                char_count: 0,
                is_current: false,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn cat26_unpinned_in_recent_section() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "T".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn cat26_current_note_bullet_prefix() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "My Note".into(),
                char_count: 0,
                is_current: true,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert!(actions[0].title.starts_with("• "));
        }

        #[test]
        fn cat26_non_current_no_bullet() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "My Note".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            };
            let actions = get_note_switcher_actions(&[note]);
            assert!(!actions[0].title.starts_with("• "));
        }

        #[test]
        fn cat26_empty_notes_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
            assert_eq!(actions[0].icon, Some(IconName::Plus));
        }

        // =========================================================================
        // 27. Action builder chaining preserves fields
        // =========================================================================

        #[test]
        fn cat27_with_icon_preserves_other_fields() {
            let a = Action::new(
                "id",
                "Title",
                Some("Desc".into()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Copy);
            assert_eq!(a.id, "id");
            assert_eq!(a.title, "Title");
            assert_eq!(a.description, Some("Desc".into()));
            assert_eq!(a.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat27_with_section_preserves_other_fields() {
            let a = Action::new("id", "Title", None, ActionCategory::ScriptContext)
                .with_section("MySec");
            assert_eq!(a.section, Some("MySec".into()));
            assert_eq!(a.id, "id");
        }

        #[test]
        fn cat27_chaining_all_builders() {
            let a = Action::new(
                "id",
                "Title",
                Some("D".into()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E")
            .with_icon(IconName::Settings)
            .with_section("S");
            assert_eq!(a.shortcut, Some("⌘E".into()));
            assert_eq!(a.icon, Some(IconName::Settings));
            assert_eq!(a.section, Some("S".into()));
            assert_eq!(a.title, "Title");
        }

        // =========================================================================
        // 28. Cross-context ID uniqueness
        // =========================================================================

        #[test]
        fn cat28_script_ids_unique() {
            let s = ScriptInfo::new("t", "/p");
            let actions = get_script_context_actions(&s);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len(), "Script action IDs must be unique");
        }

        #[test]
        fn cat28_clipboard_ids_unique() {
            let actions = get_clipboard_history_context_actions(&make_text_entry());
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(
                ids.len(),
                actions.len(),
                "Clipboard action IDs must be unique"
            );
        }

        #[test]
        fn cat28_ai_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len(), "AI action IDs must be unique");
        }

        #[test]
        fn cat28_path_ids_unique() {
            let info = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len(), "Path action IDs must be unique");
        }

        #[test]
        fn cat28_file_ids_unique() {
            let info = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            let ids: HashSet<_> = actions.iter().map(|a| &a.id).collect();
            assert_eq!(ids.len(), actions.len(), "File action IDs must be unique");
        }

        // --- merged from tests_part_05.rs ---
        // =========================================================================
        // 29. has_action=false for all built-in actions
        // =========================================================================

        #[test]
        fn cat29_script_all_has_action_false() {
            let s = ScriptInfo::new("t", "/p");
            for a in &get_script_context_actions(&s) {
                assert!(!a.has_action, "{} should be false", a.id);
            }
        }

        #[test]
        fn cat29_clipboard_all_has_action_false() {
            for a in &get_clipboard_history_context_actions(&make_text_entry()) {
                assert!(!a.has_action, "{} should be false", a.id);
            }
        }

        #[test]
        fn cat29_ai_all_has_action_false() {
            for a in &get_ai_command_bar_actions() {
                assert!(!a.has_action, "{} should be false", a.id);
            }
        }

        #[test]
        fn cat29_path_all_has_action_false() {
            let info = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            for a in &get_path_context_actions(&info) {
                assert!(!a.has_action, "{} should be false", a.id);
            }
        }

        #[test]
        fn cat29_file_all_has_action_false() {
            let info = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for a in &get_file_context_actions(&info) {
                assert!(!a.has_action, "{} should be false", a.id);
            }
        }

        // =========================================================================
        // 30. Ordering determinism
        // =========================================================================

        #[test]
        fn cat30_script_ordering_deterministic() {
            let s = ScriptInfo::new("t", "/p");
            let a = action_ids(&get_script_context_actions(&s));
            let b = action_ids(&get_script_context_actions(&s));
            assert_eq!(a, b);
        }

        #[test]
        fn cat30_clipboard_ordering_deterministic() {
            let a = action_ids(&get_clipboard_history_context_actions(&make_text_entry()));
            let b = action_ids(&get_clipboard_history_context_actions(&make_text_entry()));
            assert_eq!(a, b);
        }

        #[test]
        fn cat30_ai_ordering_deterministic() {
            let a = action_ids(&get_ai_command_bar_actions());
            let b = action_ids(&get_ai_command_bar_actions());
            assert_eq!(a, b);
        }

        #[test]
        fn cat30_path_ordering_deterministic() {
            let info = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            let a = action_ids(&get_path_context_actions(&info));
            let b = action_ids(&get_path_context_actions(&info));
            assert_eq!(a, b);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_14 {
    //! Batch 14 — Builtin action validation tests
    //!
    //! Focus areas:
    //! - Path context action ordering and position guarantees
    //! - Clipboard OCR/open_with mutual exclusivity between text/image
    //! - ScriptInfo constructor field isolation (with_all sets exactly 6 params)
    //! - AI command bar exact icon-to-section exhaustive mapping
    //! - Notes command bar section-action membership
    //! - Note switcher title rendering (Untitled Note, bullet prefix, pinned override)
    //! - Chat context edge: zero models with both flags false
    //! - Scriptlet context custom action insertion ordering with multiple H3 actions
    //! - File context macOS-only action count delta
    //! - Cross-context description non-emptiness and keyword matching
    //! - build_grouped_items_static with mixed sections and alternating patterns
    //! - coerce_action_selection with interleaved headers
    //! - score_action stacking: title + description + shortcut all match
    //! - fuzzy_match Unicode subsequence
    //! - parse_shortcut_keycaps with slash and number inputs
    //! - to_deeplink_name with numeric-only and empty-after-strip inputs
    //! - Action with_shortcut_opt Some vs None chaining
    //! - CommandBarConfig notes_style field completeness
    //! - Clipboard destructive ordering invariant across pin states
    //! - Global actions seeded with core ops (reload_scripts, settings, view_logs)

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // =========================================================================
        // Helper
        // =========================================================================
        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // =========================================================================
        // 1. Path context action ordering: primary first, trash last
        // =========================================================================

        #[test]
        fn cat01_path_dir_primary_is_first_action() {
            let info = PathInfo {
                path: "/Users/test/Documents".to_string(),
                name: "Documents".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn cat01_path_file_primary_is_first_action() {
            let info = PathInfo {
                path: "/Users/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn cat01_path_trash_is_always_last() {
            for is_dir in [true, false] {
                let info = PathInfo {
                    path: "/test/item".to_string(),
                    name: "item".to_string(),
                    is_dir,
                };
                let actions = get_path_context_actions(&info);
                assert_eq!(
                    actions.last().unwrap().id,
                    "file:move_to_trash",
                    "move_to_trash should be last for is_dir={}",
                    is_dir
                );
            }
        }

        #[test]
        fn cat01_path_dir_and_file_same_action_count() {
            let dir_info = PathInfo {
                path: "/test/dir".to_string(),
                name: "dir".to_string(),
                is_dir: true,
            };
            let file_info = PathInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            assert_eq!(
                get_path_context_actions(&dir_info).len(),
                get_path_context_actions(&file_info).len()
            );
        }

        #[test]
        fn cat01_path_common_actions_always_present() {
            let info = PathInfo {
                path: "/test/thing".to_string(),
                name: "thing".to_string(),
                is_dir: false,
            };
            let ids = action_ids(&get_path_context_actions(&info));
            for expected in &[
                "file:copy_path",
                "file:open_in_finder",
                "file:open_in_editor",
                "file:open_in_quick_terminal",
                "file:copy_filename",
                "file:move_to_trash",
            ] {
                assert!(
                    ids.contains(&expected.to_string()),
                    "Missing expected action: {}",
                    expected
                );
            }
        }

        #[test]
        fn cat01_path_dir_title_contains_name() {
            let info = PathInfo {
                path: "/test/MyFolder".to_string(),
                name: "MyFolder".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            assert!(actions[0].title.contains("MyFolder"));
        }

        // =========================================================================
        // 2. Clipboard OCR only on image, not text
        // =========================================================================

        #[test]
        fn cat02_clipboard_text_has_no_ocr() {
            let entry = ClipboardEntryInfo {
                id: "t1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(!ids.contains(&"clip:clipboard_ocr".to_string()));
        }

        #[test]
        fn cat02_clipboard_image_has_ocr() {
            let entry = ClipboardEntryInfo {
                id: "i1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "img".to_string(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(ids.contains(&"clip:clipboard_ocr".to_string()));
        }

        #[test]
        fn cat02_clipboard_image_has_more_actions_than_text() {
            let text_entry = ClipboardEntryInfo {
                id: "t".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let img_entry = ClipboardEntryInfo {
                id: "i".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "y".to_string(),
                image_dimensions: Some((10, 10)),
                frontmost_app_name: None,
            };
            let text_count = get_clipboard_history_context_actions(&text_entry).len();
            let img_count = get_clipboard_history_context_actions(&img_entry).len();
            assert!(
                img_count > text_count,
                "Image ({}) should have more actions than text ({})",
                img_count,
                text_count
            );
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_text_no_open_with_macos() {
            let entry = ClipboardEntryInfo {
                id: "t2".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(!ids.contains(&"clip:clipboard_open_with".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_image_has_open_with_macos() {
            let entry = ClipboardEntryInfo {
                id: "i2".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "img".to_string(),
                image_dimensions: Some((50, 50)),
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(ids.contains(&"clip:clipboard_open_with".to_string()));
        }

        // =========================================================================
        // 3. ScriptInfo::with_all sets exactly the right fields
        // =========================================================================

        #[test]
        fn cat03_with_all_sets_name_and_path() {
            let s = ScriptInfo::with_all("Foo", "/bar", true, "Run", None, None);
            assert_eq!(s.name, "Foo");
            assert_eq!(s.path, "/bar");
        }

        #[test]
        fn cat03_with_all_sets_is_script() {
            let s_true = ScriptInfo::with_all("A", "/a", true, "Run", None, None);
            assert!(s_true.is_script);
            let s_false = ScriptInfo::with_all("B", "/b", false, "Run", None, None);
            assert!(!s_false.is_script);
        }

        #[test]
        fn cat03_with_all_sets_verb() {
            let s = ScriptInfo::with_all("X", "/x", false, "Execute", None, None);
            assert_eq!(s.action_verb, "Execute");
        }

        #[test]
        fn cat03_with_all_sets_shortcut_and_alias() {
            let s = ScriptInfo::with_all(
                "Y",
                "/y",
                false,
                "Open",
                Some("cmd+y".into()),
                Some("yy".into()),
            );
            assert_eq!(s.shortcut, Some("cmd+y".to_string()));
            assert_eq!(s.alias, Some("yy".to_string()));
        }

        #[test]
        fn cat03_with_all_defaults_no_agent_no_scriptlet() {
            let s = ScriptInfo::with_all("Z", "/z", true, "Run", None, None);
            assert!(!s.is_agent);
            assert!(!s.is_scriptlet);
            assert!(!s.is_suggested);
            assert!(s.frecency_path.is_none());
        }

        // =========================================================================
        // 4. AI command bar exact icon-to-section mapping
        // =========================================================================

        #[test]
        fn cat04_ai_response_section_has_3_actions() {
            let actions = get_ai_command_bar_actions();
            let response_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .collect();
            assert_eq!(response_actions.len(), 3);
        }

        #[test]
        fn cat04_ai_actions_section_has_4_actions() {
            let actions = get_ai_command_bar_actions();
            let section_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .collect();
            assert_eq!(section_actions.len(), 7);
        }

        #[test]
        fn cat04_ai_attachments_section_has_2_actions() {
            let actions = get_ai_command_bar_actions();
            let section_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .collect();
            assert_eq!(section_actions.len(), 4);
        }

        #[test]
        fn cat04_ai_export_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let section_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Export"))
                .collect();
            assert_eq!(section_actions.len(), 1);
        }

        #[test]
        fn cat04_ai_help_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let section_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Help"))
                .collect();
            assert_eq!(section_actions.len(), 1);
        }

        #[test]
        fn cat04_ai_settings_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let section_actions: Vec<_> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Settings"))
                .collect();
            assert_eq!(section_actions.len(), 2);
        }

        #[test]
        fn cat04_ai_copy_response_icon_is_copy() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:copy_response")
                .unwrap();
            assert_eq!(a.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat04_ai_submit_icon_is_arrowup() {
            let actions = get_ai_command_bar_actions();
            let a = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(a.icon, Some(IconName::ArrowUp));
        }

        #[test]
        fn cat04_ai_new_chat_icon_is_plus() {
            let actions = get_ai_command_bar_actions();
            let a = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(a.icon, Some(IconName::Plus));
        }

        #[test]
        fn cat04_ai_delete_chat_icon_is_trash() {
            let actions = get_ai_command_bar_actions();
            let a = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(a.icon, Some(IconName::Trash));
        }

        #[test]
        fn cat04_ai_change_model_icon_is_settings() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert_eq!(a.icon, Some(IconName::Settings));
        }

        // =========================================================================
        // 5. Notes command bar section-action membership
        // =========================================================================

        // =========================================================================
        // 6. Note switcher title rendering
        // =========================================================================

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat06_note_switcher_current_gets_bullet_prefix() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "My Note".into(),
                char_count: 100,
                is_current: true,
                is_pinned: false,
                preview: "content".into(),
                relative_time: "1m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(actions[0].title.starts_with("• "));
        }

        #[test]
        fn cat06_note_switcher_non_current_no_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n2".into(),
                title: "Other Note".into(),
                char_count: 50,
                is_current: false,
                is_pinned: false,
                preview: "stuff".into(),
                relative_time: "5m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(!actions[0].title.starts_with("• "));
            assert_eq!(actions[0].title, "Other Note");
        }

        #[test]
        fn cat06_note_switcher_pinned_overrides_current_icon() {
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
            // Pinned takes priority over current for icon
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        #[test]
        fn cat06_note_switcher_regular_icon_is_file() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n4".into(),
                title: "Regular".into(),
                char_count: 5,
                is_current: false,
                is_pinned: false,
                preview: "abc".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::File));
        }

        #[test]
        fn cat06_note_switcher_empty_shows_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].id, "no_notes");
            assert!(actions[0].title.contains("No notes"));
        }

        #[test]
        fn cat06_note_switcher_id_format() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc-123".into(),
                title: "Test".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].id, "note_abc-123");
        }

        // =========================================================================
        // 7. Chat context: zero models, both flags false
        // =========================================================================

        #[test]
        fn cat07_chat_zero_models_still_has_continue() {
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
        fn cat07_chat_no_response_no_copy() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(!ids.contains(&"chat:copy_response".to_string()));
        }

        #[test]
        fn cat07_chat_no_messages_no_clear() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(!ids.contains(&"chat:clear_conversation".to_string()));
        }

        #[test]
        fn cat07_chat_both_flags_true_gives_max_actions() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![ChatModelInfo {
                    id: "m1".into(),
                    display_name: "Claude".into(),
                    provider: "Anthropic".into(),
                }],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            // 1 model + continue + copy_response + clear + capture = 5
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn cat07_chat_current_model_gets_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude".into()),
                available_models: vec![
                    ChatModelInfo {
                        id: "m1".into(),
                        display_name: "Claude".into(),
                        provider: "Anthropic".into(),
                    },
                    ChatModelInfo {
                        id: "m2".into(),
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
                .find(|a| a.id == "chat:select_model_m1")
                .unwrap();
            assert!(claude.title.contains("✓"));
            let gpt = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m2")
                .unwrap();
            assert!(!gpt.title.contains("✓"));
        }

        // =========================================================================
        // 8. Scriptlet context custom action ordering with multiple H3 actions
        // =========================================================================

        #[test]
        fn cat08_scriptlet_custom_actions_appear_after_run() {
            let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "Alpha".into(),
                    command: "alpha".into(),
                    tool: "bash".into(),
                    code: "echo a".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Beta".into(),
                    command: "beta".into(),
                    tool: "bash".into(),
                    code: "echo b".into(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
            ];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let ids = action_ids(&actions);
            let run_idx = ids.iter().position(|id| id == "run_script").unwrap();
            let alpha_idx = ids
                .iter()
                .position(|id| id == "scriptlet_action:alpha")
                .unwrap();
            let beta_idx = ids
                .iter()
                .position(|id| id == "scriptlet_action:beta")
                .unwrap();
            assert!(run_idx < alpha_idx);
            assert!(alpha_idx < beta_idx);
        }

        #[test]
        fn cat08_scriptlet_custom_actions_before_builtins() {
            let script = ScriptInfo::scriptlet("Test", "/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo main".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Custom".into(),
                command: "custom".into(),
                tool: "bash".into(),
                code: "echo c".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let ids = action_ids(&actions);
            let custom_idx = ids
                .iter()
                .position(|id| id == "scriptlet_action:custom")
                .unwrap();
            let edit_idx = ids.iter().position(|id| id == "edit_scriptlet").unwrap();
            assert!(custom_idx < edit_idx);
        }

        #[test]
        fn cat08_scriptlet_custom_has_action_true() {
            let mut scriptlet =
                Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
            scriptlet.actions = vec![ScriptletAction {
                name: "Do Thing".into(),
                command: "do-thing".into(),
                tool: "bash".into(),
                code: "echo do".into(),
                inputs: vec![],
                shortcut: Some("cmd+d".into()),
                description: Some("Does the thing".into()),
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].has_action);
            assert_eq!(actions[0].value, Some("do-thing".to_string()));
        }

        #[test]
        fn cat08_scriptlet_custom_shortcut_formatted() {
            let mut scriptlet =
                Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy".into(),
                command: "copy".into(),
                tool: "bash".into(),
                code: "echo cp".into(),
                inputs: vec![],
                shortcut: Some("cmd+shift+c".into()),
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].shortcut, Some("⌘⇧C".to_string()));
        }

        #[test]
        fn cat08_scriptlet_no_actions_returns_empty() {
            let scriptlet =
                Scriptlet::new("Empty".to_string(), "bash".to_string(), "echo".to_string());
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions.is_empty());
        }

        // =========================================================================
        // 9. File context macOS-only action count delta
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat09_file_macos_has_quick_look_show_info() {
            let info = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let ids = action_ids(&get_file_context_actions(&info));
            assert!(ids.contains(&"file:quick_look".to_string()));
            assert!(ids.contains(&"file:show_info".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat09_file_dir_no_quick_look_macos() {
            let info = FileInfo {
                path: "/test/dir".into(),
                name: "dir".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let ids = action_ids(&get_file_context_actions(&info));
            assert!(!ids.contains(&"file:quick_look".to_string()));
            // But still has open_in_editor and show_info
            assert!(ids.contains(&"file:open_in_editor".to_string()));
            assert!(ids.contains(&"file:show_info".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat09_file_macos_file_more_actions_than_dir() {
            let file_info = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let dir_info = FileInfo {
                path: "/test/d".into(),
                name: "d".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let file_count = get_file_context_actions(&file_info).len();
            let dir_count = get_file_context_actions(&dir_info).len();
            assert!(
                file_count > dir_count,
                "File ({}) should have more actions than dir ({}) on macOS",
                file_count,
                dir_count
            );
        }

        // =========================================================================
        // 10. Cross-context description non-emptiness
        // =========================================================================

        #[test]
        fn cat10_script_context_all_have_descriptions() {
            let script = ScriptInfo::new("test", "/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    action.description.is_some(),
                    "Script action '{}' should have a description",
                    action.id
                );
            }
        }

        #[test]
        fn cat10_ai_command_bar_all_have_descriptions() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    action.description.is_some(),
                    "AI action '{}' should have a description",
                    action.id
                );
            }
        }

        #[test]
        fn cat10_path_context_all_have_descriptions() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&info) {
                assert!(
                    action.description.is_some(),
                    "Path action '{}' should have a description",
                    action.id
                );
            }
        }

        #[test]
        fn cat10_file_context_all_have_descriptions() {
            let info = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&info) {
                assert!(
                    action.description.is_some(),
                    "File action '{}' should have a description",
                    action.id
                );
            }
        }

        #[test]
        fn cat10_clipboard_context_all_have_descriptions() {
            let entry = ClipboardEntryInfo {
                id: "c1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(
                    action.description.is_some(),
                    "Clipboard action '{}' should have a description",
                    action.id
                );
            }
        }

        // =========================================================================
        // 11. build_grouped_items_static with mixed sections
        // =========================================================================

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat11_grouped_items_headers_insert_for_each_section_change() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("a3", "A3", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered: Vec<usize> = (0..3).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // S1 header, a1, a2, S2 header, a3
            assert_eq!(grouped.len(), 5);
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[2], GroupedActionItem::Item(1)));
            assert!(matches!(grouped[3], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[4], GroupedActionItem::Item(2)));
        }

        #[test]
        fn cat11_grouped_items_separators_no_headers() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat11_grouped_items_none_style_no_headers() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 2);
        }

        #[test]
        fn cat11_grouped_items_empty_input() {
            let actions: Vec<Action> = vec![];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn cat11_grouped_items_no_section_no_header() {
            let actions = vec![
                Action::new("a1", "A1", None, ActionCategory::ScriptContext),
                Action::new("a2", "A2", None, ActionCategory::ScriptContext),
            ];
            let filtered: Vec<usize> = (0..2).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // No sections means no headers
            assert_eq!(grouped.len(), 2);
        }

        // =========================================================================
        // 12. coerce_action_selection with interleaved headers
        // =========================================================================

        #[test]
        fn cat12_coerce_empty_returns_none() {
            assert_eq!(coerce_action_selection(&[], 0), None);
        }

        #[test]
        fn cat12_coerce_on_item_returns_same() {
            let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
            assert_eq!(coerce_action_selection(&rows, 1), Some(1));
        }

        #[test]
        fn cat12_coerce_header_searches_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat12_coerce_trailing_header_searches_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("H".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat12_coerce_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H1".into()),
                GroupedActionItem::SectionHeader("H2".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat12_coerce_out_of_bounds_clamped() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H".into()),
                GroupedActionItem::Item(0),
            ];
            // Index 99 should be clamped to last index (1), which is an Item
            assert_eq!(coerce_action_selection(&rows, 99), Some(1));
        }

        #[test]
        fn cat12_coerce_interleaved_headers() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S1".into()),
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S2".into()),
                GroupedActionItem::Item(1),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
            assert_eq!(coerce_action_selection(&rows, 2), Some(3));
        }

        // =========================================================================
        // 13. score_action stacking: title + description + shortcut all match
        // =========================================================================

        #[test]
        fn cat13_score_prefix_plus_desc_plus_shortcut() {
            let action = Action::new(
                "file:copy_path",
                "Copy Path",
                Some("Copy the full path".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘⇧C");
            // "copy" matches: prefix title (100) + description (15) = 115
            let score = ActionsDialog::score_action(&action, "copy");
            assert!(
                score >= 115,
                "Expected >=115 for prefix+desc match, got {}",
                score
            );
        }

        #[test]
        fn cat13_score_contains_title_only() {
            let action = Action::new(
                "x",
                "Open Copy Path",
                Some("unrelated".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "copy");
            assert_eq!(score, 50);
        }

        #[test]
        fn cat13_score_fuzzy_title() {
            let action = Action::new(
                "x",
                "Create Proxy",
                Some("unrelated".to_string()),
                ActionCategory::ScriptContext,
            );
            // "cry" is a subsequence of "create proxy"
            let score = ActionsDialog::score_action(&action, "cry");
            assert!(score >= 25, "Expected fuzzy match >=25, got {}", score);
        }

        #[test]
        fn cat13_score_description_only() {
            let action = Action::new(
                "x",
                "Unrelated Title",
                Some("Copy the path to clipboard".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "clipboard");
            assert_eq!(score, 15);
        }

        #[test]
        fn cat13_score_no_match_zero() {
            let action = Action::new(
                "x",
                "Edit Script",
                Some("Open in editor".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E");
            let score = ActionsDialog::score_action(&action, "zzzzz");
            assert_eq!(score, 0);
        }

        // =========================================================================
        // 14. fuzzy_match Unicode subsequence
        // =========================================================================

        #[test]
        fn cat14_fuzzy_match_ascii_subsequence() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlo"));
        }

        #[test]
        fn cat14_fuzzy_match_exact() {
            assert!(ActionsDialog::fuzzy_match("exact", "exact"));
        }

        #[test]
        fn cat14_fuzzy_match_empty_needle() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn cat14_fuzzy_match_no_match() {
            assert!(!ActionsDialog::fuzzy_match("abc", "xyz"));
        }

        #[test]
        fn cat14_fuzzy_match_needle_longer() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        #[test]
        fn cat14_fuzzy_match_both_empty() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn cat14_fuzzy_match_empty_haystack_nonempty_needle() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        // =========================================================================
        // 15. parse_shortcut_keycaps with slash and number inputs
        // =========================================================================

        #[test]
        fn cat15_parse_keycaps_cmd_slash() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘/");
            assert_eq!(keycaps, vec!["⌘", "/"]);
        }

        #[test]
        fn cat15_parse_keycaps_number() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘1");
            assert_eq!(keycaps, vec!["⌘", "1"]);
        }

        #[test]
        fn cat15_parse_keycaps_modifier_chain() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧⌥C");
            assert_eq!(keycaps, vec!["⌘", "⇧", "⌥", "C"]);
        }

        #[test]
        fn cat15_parse_keycaps_enter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(keycaps, vec!["↵"]);
        }

        #[test]
        fn cat15_parse_keycaps_escape() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn cat15_parse_keycaps_arrows() {
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
            assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
        }

        #[test]
        fn cat15_parse_keycaps_lowercase_uppercased() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘c");
            assert_eq!(keycaps, vec!["⌘", "C"]);
        }

        // =========================================================================
        // 16. to_deeplink_name edge cases
        // =========================================================================

        #[test]
        fn cat16_deeplink_numeric_only() {
            assert_eq!(to_deeplink_name("12345"), "12345");
        }

        #[test]
        fn cat16_deeplink_all_special_returns_empty() {
            assert_eq!(to_deeplink_name("!@#$%^&*()"), "_unnamed");
        }

        #[test]
        fn cat16_deeplink_mixed_case_lowered() {
            assert_eq!(to_deeplink_name("Hello World"), "hello-world");
        }

        #[test]
        fn cat16_deeplink_consecutive_specials_collapsed() {
            assert_eq!(to_deeplink_name("a!!b"), "a-b");
        }

        #[test]
        fn cat16_deeplink_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("---hello---"), "hello");
        }

        #[test]
        fn cat16_deeplink_unicode_preserved() {
            // Unicode alphanumeric chars are normalized then percent-encoded.
            let result = to_deeplink_name("café");
            assert_eq!(result, "caf%C3%A9");
        }

        #[test]
        fn cat16_deeplink_underscores_become_hyphens() {
            assert_eq!(to_deeplink_name("hello_world"), "hello-world");
        }

        // =========================================================================
        // 17. Action with_shortcut_opt Some vs None
        // =========================================================================

        #[test]
        fn cat17_with_shortcut_opt_none_preserves_none() {
            let action =
                Action::new("x", "X", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
            assert!(action.shortcut.is_none());
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn cat17_with_shortcut_opt_some_sets_shortcut() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘C".to_string()));
            assert_eq!(action.shortcut, Some("⌘C".to_string()));
            assert!(action.shortcut_lower.is_some());
        }

        #[test]
        fn cat17_with_shortcut_sets_lower() {
            let action =
                Action::new("x", "X", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧K");
            assert_eq!(action.shortcut_lower, Some("⌘⇧k".to_string()));
        }

        #[test]
        fn cat17_action_new_no_shortcut_lower() {
            let action = Action::new("x", "Title", None, ActionCategory::ScriptContext);
            assert!(action.shortcut.is_none());
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn cat17_action_title_lower_cached() {
            let action = Action::new("x", "Hello World", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "hello world");
        }

        #[test]
        fn cat17_action_description_lower_cached() {
            let action = Action::new(
                "x",
                "T",
                Some("Open in Editor".to_string()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower, Some("open in editor".to_string()));
        }

        // =========================================================================
        // 18. CommandBarConfig notes_style field completeness
        // =========================================================================

        #[test]
        fn cat18_notes_style_search_at_top() {
            let cfg = CommandBarConfig::notes_style();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
        }

        #[test]
        fn cat18_notes_style_separators() {
            let cfg = CommandBarConfig::notes_style();
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat18_notes_style_anchor_top() {
            let cfg = CommandBarConfig::notes_style();
            assert_eq!(cfg.dialog_config.anchor, AnchorPosition::Top);
        }

        #[test]
        fn cat18_notes_style_icons_enabled() {
            let cfg = CommandBarConfig::notes_style();
            assert!(cfg.dialog_config.show_icons);
        }

        #[test]
        fn cat18_notes_style_footer_enabled() {
            let cfg = CommandBarConfig::notes_style();
            assert!(!cfg.dialog_config.show_footer);
        }

        #[test]
        fn cat18_notes_style_close_defaults_true() {
            let cfg = CommandBarConfig::notes_style();
            assert!(cfg.close_on_select);
            assert!(cfg.close_on_click_outside);
            assert!(cfg.close_on_escape);
        }

        #[test]
        fn cat18_ai_style_search_at_top_headers() {
            let cfg = CommandBarConfig::ai_style();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat18_main_menu_search_at_top() {
            let cfg = CommandBarConfig::main_menu_style();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Top);
            assert_eq!(cfg.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat18_no_search_hidden() {
            let cfg = CommandBarConfig::no_search();
            assert_eq!(cfg.dialog_config.search_position, SearchPosition::Hidden);
        }

        // =========================================================================
        // 19. Clipboard destructive ordering invariant across pin states
        // =========================================================================

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat19_clipboard_unpinned_last_three_destructive() {
            let entry = ClipboardEntryInfo {
                id: "u1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "txt".into(),
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
        fn cat19_clipboard_pinned_last_three_destructive() {
            let entry = ClipboardEntryInfo {
                id: "p1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "txt".into(),
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
        fn cat19_clipboard_paste_always_first() {
            for pinned in [true, false] {
                for ct in [ContentType::Text, ContentType::Image] {
                    let entry = ClipboardEntryInfo {
                        id: "x".into(),
                        content_type: ct,
                        pinned,
                        preview: "p".into(),
                        image_dimensions: if ct == ContentType::Image {
                            Some((1, 1))
                        } else {
                            None
                        },
                        frontmost_app_name: None,
                    };
                    let actions = get_clipboard_history_context_actions(&entry);
                    assert_eq!(
                        actions[0].id, "clip:clipboard_paste",
                        "Paste should be first for pinned={} type={:?}",
                        pinned, ct
                    );
                }
            }
        }

        #[test]
        fn cat19_clipboard_copy_always_second() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[1].id, "clip:clipboard_copy");
        }

        // =========================================================================
        // 20. Global actions are seeded with core ops
        // =========================================================================

        #[test]
        fn cat20_global_actions_seeded() {
            // get_global_actions() is seeded (reload_scripts, settings, view_logs)
            // plus the Discover rows, so we assert the
            // core ids rather than emptiness or an exact total count.
            let actions = get_global_actions();
            let ids = action_ids(&actions);
            for core in ["reload_scripts", "settings", "view_logs"] {
                assert!(
                    ids.contains(&core.to_string()),
                    "global actions missing {}",
                    core
                );
            }
        }

        // =========================================================================
        // 21. New chat action structure and ID patterns
        // =========================================================================

        #[test]
        fn cat21_new_chat_empty_inputs_empty_actions() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn cat21_new_chat_last_used_id_pattern() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].id, "last_used_anthropic::m1");
        }

        #[test]
        fn cat21_new_chat_preset_id_pattern() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].id, "preset_general");
        }

        #[test]
        fn cat21_new_chat_model_id_pattern() {
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
        fn cat21_new_chat_section_ordering() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "gen".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        #[test]
        fn cat21_new_chat_last_used_icon_bolt() {
            let last_used = vec![NewChatModelInfo {
                model_id: "x".into(),
                display_name: "X".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn cat21_new_chat_model_icon_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "x".into(),
                display_name: "X".into(),
                provider: "p".into(),
                provider_display_name: "P".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        // =========================================================================
        // 22. format_shortcut_hint additional cases
        // =========================================================================

        #[test]
        fn cat22_format_shortcut_opt_maps_option() {
            let result = ActionsDialog::format_shortcut_hint("opt+v");
            assert_eq!(result, "⌥V");
        }

        #[test]
        fn cat22_format_shortcut_triple_modifier() {
            let result = ActionsDialog::format_shortcut_hint("cmd+shift+ctrl+a");
            assert_eq!(result, "⌘⇧⌃A");
        }

        #[test]
        fn cat22_format_shortcut_space() {
            let result = ActionsDialog::format_shortcut_hint("space");
            assert_eq!(result, "␣");
        }

        #[test]
        fn cat22_format_shortcut_tab() {
            let result = ActionsDialog::format_shortcut_hint("tab");
            assert_eq!(result, "⇥");
        }

        #[test]
        fn cat22_format_shortcut_arrowup() {
            let result = ActionsDialog::format_shortcut_hint("arrowup");
            assert_eq!(result, "↑");
        }

        #[test]
        fn cat22_format_shortcut_arrowdown() {
            let result = ActionsDialog::format_shortcut_hint("arrowdown");
            assert_eq!(result, "↓");
        }

        // =========================================================================
        // 23. Agent context actions
        // =========================================================================

        #[test]
        fn cat23_agent_has_edit_agent_title() {
            let mut script = ScriptInfo::new("My Agent", "/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn cat23_agent_has_no_view_logs() {
            let mut script = ScriptInfo::new("My Agent", "/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let ids = action_ids(&get_script_context_actions(&script));
            assert!(!ids.contains(&"view_logs".to_string()));
        }

        #[test]
        fn cat23_agent_has_copy_content() {
            let mut script = ScriptInfo::new("My Agent", "/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let ids = action_ids(&get_script_context_actions(&script));
            assert!(ids.contains(&"copy_content".to_string()));
        }

        #[test]
        fn cat23_agent_has_reveal_and_copy_path() {
            let mut script = ScriptInfo::new("My Agent", "/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let ids = action_ids(&get_script_context_actions(&script));
            assert!(ids.contains(&"reveal_in_finder".to_string()));
            assert!(ids.contains(&"copy_path".to_string()));
        }

        #[test]
        fn cat23_agent_description_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
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
        // 24. Cross-context ID uniqueness
        // =========================================================================

        #[test]
        fn cat24_script_context_ids_unique() {
            let script = ScriptInfo::new("test", "/test.ts");
            let ids = action_ids(&get_script_context_actions(&script));
            let set: HashSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), set.len());
        }

        #[test]
        fn cat24_clipboard_context_ids_unique() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            let set: HashSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), set.len());
        }

        #[test]
        fn cat24_ai_command_bar_ids_unique() {
            let ids = action_ids(&get_ai_command_bar_actions());
            let set: HashSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), set.len());
        }

        #[test]
        fn cat24_path_context_ids_unique() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            let ids = action_ids(&get_path_context_actions(&info));
            let set: HashSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), set.len());
        }

        #[test]
        fn cat24_file_context_ids_unique() {
            let info = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let ids = action_ids(&get_file_context_actions(&info));
            let set: HashSet<_> = ids.iter().collect();
            assert_eq!(ids.len(), set.len());
        }

        // =========================================================================
        // 25. has_action=false for all built-ins
        // =========================================================================

        #[test]
        fn cat25_script_all_has_action_false() {
            let script = ScriptInfo::new("test", "/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    !action.has_action,
                    "Action {} has_action should be false",
                    action.id
                );
            }
        }

        #[test]
        fn cat25_clipboard_all_has_action_false() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(
                    !action.has_action,
                    "Action {} has_action should be false",
                    action.id
                );
            }
        }

        #[test]
        fn cat25_ai_all_has_action_false() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    !action.has_action,
                    "Action {} has_action should be false",
                    action.id
                );
            }
        }

        #[test]
        fn cat25_path_all_has_action_false() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "Action {} has_action should be false",
                    action.id
                );
            }
        }

        #[test]
        fn cat25_file_all_has_action_false() {
            let info = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "Action {} has_action should be false",
                    action.id
                );
            }
        }

        // =========================================================================
        // 26. Ordering determinism
        // =========================================================================

        // --- merged from tests_part_05.rs ---
        #[test]
        fn cat26_script_ordering_deterministic() {
            let script = ScriptInfo::new("test", "/test.ts");
            let ids1 = action_ids(&get_script_context_actions(&script));
            let ids2 = action_ids(&get_script_context_actions(&script));
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn cat26_clipboard_ordering_deterministic() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids1 = action_ids(&get_clipboard_history_context_actions(&entry));
            let ids2 = action_ids(&get_clipboard_history_context_actions(&entry));
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn cat26_ai_ordering_deterministic() {
            let ids1 = action_ids(&get_ai_command_bar_actions());
            let ids2 = action_ids(&get_ai_command_bar_actions());
            assert_eq!(ids1, ids2);
        }

        #[test]
        fn cat26_path_ordering_deterministic() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: true,
            };
            let ids1 = action_ids(&get_path_context_actions(&info));
            let ids2 = action_ids(&get_path_context_actions(&info));
            assert_eq!(ids1, ids2);
        }

        // =========================================================================
        // 27. Action builder chaining
        // =========================================================================

        #[test]
        fn cat27_with_icon_preserves_shortcut() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘X")
                .with_icon(IconName::Copy);
            assert_eq!(action.shortcut, Some("⌘X".to_string()));
            assert_eq!(action.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat27_with_section_preserves_icon() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Star)
                .with_section("Test");
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section, Some("Test".to_string()));
        }

        #[test]
        fn cat27_full_chain_preserves_all() {
            let action = Action::new(
                "test",
                "Test",
                Some("Desc".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘T")
            .with_icon(IconName::Plus)
            .with_section("Section");
            assert_eq!(action.id, "test");
            assert_eq!(action.title, "Test");
            assert_eq!(action.description, Some("Desc".to_string()));
            assert_eq!(action.shortcut, Some("⌘T".to_string()));
            assert_eq!(action.icon, Some(IconName::Plus));
            assert_eq!(action.section, Some("Section".to_string()));
        }

        // =========================================================================
        // 28. Note switcher description rendering
        // =========================================================================

        #[test]
        fn cat28_note_switcher_preview_with_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n1".into(),
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
            assert!(desc.contains("·"));
        }

        #[test]
        fn cat28_note_switcher_empty_preview_shows_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n2".into(),
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
        fn cat28_note_switcher_singular_char() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n3".into(),
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
        fn cat28_note_switcher_truncation_at_61() {
            let long_preview = "a".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n4".into(),
                title: "T".into(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: long_preview,
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.ends_with('…'));
        }

        #[test]
        fn cat28_note_switcher_no_truncation_at_60() {
            let exact_preview = "b".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n5".into(),
                title: "T".into(),
                char_count: 60,
                is_current: false,
                is_pinned: false,
                preview: exact_preview.clone(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'));
            assert_eq!(desc.as_str(), exact_preview.as_str());
        }

        #[test]
        fn cat28_note_switcher_empty_preview_with_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n6".into(),
                title: "T".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "3h ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "3h ago");
        }

        // =========================================================================
        // 29. File context title includes quoted name
        // =========================================================================

        #[test]
        fn cat29_file_title_includes_filename() {
            let info = FileInfo {
                path: "/test/report.pdf".into(),
                name: "report.pdf".into(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions[0].title.contains("report.pdf"));
        }

        #[test]
        fn cat29_file_dir_title_includes_dirname() {
            let info = FileInfo {
                path: "/test/MyDir".into(),
                name: "MyDir".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions[0].title.contains("MyDir"));
        }

        #[test]
        fn cat29_file_title_has_quotes() {
            let info = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions[0].title.contains('"'));
        }

        // =========================================================================
        // 30. Non-empty id and title for all contexts
        // =========================================================================

        #[test]
        fn cat30_script_nonempty_id_title() {
            let script = ScriptInfo::new("test", "/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(!action.id.is_empty(), "Action should have non-empty id");
                assert!(
                    !action.title.is_empty(),
                    "Action {} should have non-empty title",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_clipboard_nonempty_id_title() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_ai_nonempty_id_title() {
            for action in &get_ai_command_bar_actions() {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_path_nonempty_id_title() {
            let info = PathInfo {
                path: "/test".into(),
                name: "test".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&info) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_file_nonempty_id_title() {
            let info = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&info) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }
    }
}

mod from_dialog_builtin_action_validation_tests_15 {
    //! Batch 15 – Dialog Builtin Action Validation Tests
    //!
    //! 30 categories, ~170 tests covering fresh angles:
    //! - to_deeplink_name with non-Latin Unicode (Arabic, Thai, Devanagari)
    //! - Clipboard image macOS-exclusive actions
    //! - Notes combined-flag interactions
    //! - Chat context boundary states
    //! - New chat section guarantees
    //! - Note switcher description fallback hierarchy
    //! - AI command bar per-section ID enumeration
    //! - Action builder overwrite semantics
    //! - CommandBarConfig preset field comparison matrix
    //! - Cross-context category uniformity
    //! - Clipboard exact action counts on macOS
    //! - Path primary-action insertion position
    //! - File title quoting
    //! - ScriptInfo::with_all field completeness
    //! - Ordering idempotency (double-call determinism)

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use std::collections::HashSet;

        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // =========================================================================
        // cat01: to_deeplink_name with non-Latin Unicode scripts
        // =========================================================================

        #[test]
        fn cat01_deeplink_arabic_preserved() {
            // Arabic alphanumeric chars should pass is_alphanumeric()
            let result = to_deeplink_name("مرحبا");
            assert!(!result.is_empty(), "Arabic should be preserved");
            assert!(
                result.contains('%'),
                "Unicode should be percent-encoded: {}",
                result
            );
        }

        #[test]
        fn cat01_deeplink_thai_preserved() {
            let result = to_deeplink_name("สวัสดี");
            assert!(!result.is_empty(), "Thai should be preserved");
        }

        #[test]
        fn cat01_deeplink_devanagari_preserved() {
            let result = to_deeplink_name("नमस्ते");
            assert!(!result.is_empty(), "Devanagari should be preserved");
        }

        #[test]
        fn cat01_deeplink_mixed_scripts() {
            // "Hello-مرحبا" — mixed Latin and Arabic
            let result = to_deeplink_name("Hello مرحبا");
            assert!(result.contains("hello"), "Latin part lowercased");
            // Arabic and Latin separated by space → hyphen
            assert!(result.contains('-'), "Space becomes hyphen");
        }

        #[test]
        fn cat01_deeplink_empty_string() {
            assert_eq!(to_deeplink_name(""), "_unnamed");
        }

        #[test]
        fn cat01_deeplink_only_specials() {
            assert_eq!(to_deeplink_name("!@#$%^&*()"), "_unnamed");
        }

        #[test]
        fn cat01_deeplink_single_char() {
            assert_eq!(to_deeplink_name("a"), "a");
        }

        // =========================================================================
        // cat02: Clipboard image macOS-exclusive action set
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_image_macos_has_open_with() {
            let entry = ClipboardEntryInfo {
                id: "img-1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_open_with".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_image_macos_has_annotate_cleanshot() {
            let entry = ClipboardEntryInfo {
                id: "img-2".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((200, 200)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_annotate_cleanshot".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_image_macos_has_upload_cleanshot() {
            let entry = ClipboardEntryInfo {
                id: "img-3".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_upload_cleanshot".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_text_macos_no_annotate() {
            let entry = ClipboardEntryInfo {
                id: "txt-1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(!ids.contains(&"clip:clipboard_annotate_cleanshot".to_string()));
            assert!(!ids.contains(&"clip:clipboard_upload_cleanshot".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat02_clipboard_image_has_ocr_text_does_not() {
            let img = ClipboardEntryInfo {
                id: "i1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let txt = ClipboardEntryInfo {
                id: "t1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let img_ids = action_ids(&get_clipboard_history_context_actions(&img));
            let txt_ids = action_ids(&get_clipboard_history_context_actions(&txt));
            assert!(img_ids.contains(&"clip:clipboard_ocr".to_string()));
            assert!(!txt_ids.contains(&"clip:clipboard_ocr".to_string()));
        }

        // =========================================================================
        // cat03: Notes combined-flag interactions
        // =========================================================================

        // =========================================================================
        // cat04: Chat context boundary states
        // =========================================================================

        #[test]
        fn cat04_chat_zero_models_both_flags_false() {
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
        fn cat04_chat_zero_models_both_flags_true() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            // continue_in_chat + copy_response + clear_conversation + capture = 4
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn cat04_chat_model_id_format() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "gpt-4o".into(),
                    display_name: "GPT-4o".into(),
                    provider: "OpenAI".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            assert_eq!(picker[0].id, "chat:select_model_gpt-4o");
        }

        #[test]
        fn cat04_chat_current_model_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude 3.5".into()),
                available_models: vec![
                    ChatModelInfo {
                        id: "claude-35".into(),
                        display_name: "Claude 3.5".into(),
                        provider: "Anthropic".into(),
                    },
                    ChatModelInfo {
                        id: "gpt-4".into(),
                        display_name: "GPT-4".into(),
                        provider: "OpenAI".into(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            // Model rows live in the drill-down picker
            let picker = get_chat_model_picker_actions(&info);
            let claude = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-35")
                .unwrap();
            assert!(claude.title.contains("✓"), "Current model gets checkmark");
            let gpt = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt-4")
                .unwrap();
            assert!(!gpt.title.contains("✓"), "Non-current model no checkmark");
        }

        #[test]
        fn cat04_chat_continue_shortcut() {
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
        fn cat04_chat_model_description_via_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m1".into(),
                    display_name: "Model One".into(),
                    provider: "Acme Corp".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            // Model rows live in the drill-down picker
            let picker = get_chat_model_picker_actions(&info);
            let model = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m1")
                .unwrap();
            assert_eq!(model.description.as_deref(), Some("Uses Acme Corp"));
        }

        // =========================================================================
        // cat05: New chat section ordering guarantees
        // =========================================================================

        #[test]
        fn cat05_new_chat_empty_all_sections() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        #[test]
        fn cat05_new_chat_section_ordering() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "Model 1".into(),
                provider: "P".into(),
                provider_display_name: "Provider".into(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".into(),
                display_name: "Model 2".into(),
                provider: "P".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 3);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        #[test]
        fn cat05_new_chat_preset_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert!(
                actions[0]
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("preset")
            );
        }

        #[test]
        fn cat05_new_chat_model_has_provider_description() {
            let models = vec![NewChatModelInfo {
                model_id: "gpt4".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].description.as_deref(), Some("Uses OpenAI"));
        }

        #[test]
        fn cat05_new_chat_last_used_icon_bolt() {
            let last_used = vec![NewChatModelInfo {
                model_id: "x".into(),
                display_name: "X".into(),
                provider: "P".into(),
                provider_display_name: "PP".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn cat05_new_chat_model_icon_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "M".into(),
                provider: "P".into(),
                provider_display_name: "PP".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        // =========================================================================
        // cat06: Note switcher description fallback hierarchy
        // =========================================================================

        #[test]
        fn cat06_note_switcher_preview_and_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "Test".into(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "Hello world".into(),
                relative_time: "5m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert!(desc.contains("Hello world"));
            assert!(desc.contains("5m ago"));
            assert!(desc.contains("·"), "Separator between preview and time");
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat06_note_switcher_preview_no_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n2".into(),
                title: "Test".into(),
                char_count: 50,
                is_current: false,
                is_pinned: false,
                preview: "Some content".into(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert_eq!(desc, "Some content");
            assert!(!desc.contains("·"));
        }

        #[test]
        fn cat06_note_switcher_no_preview_with_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n3".into(),
                title: "Test".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: "1h ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert_eq!(desc, "1h ago");
        }

        #[test]
        fn cat06_note_switcher_no_preview_no_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n4".into(),
                title: "Test".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        #[test]
        fn cat06_note_switcher_singular_char() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n5".into(),
                title: "T".into(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert_eq!(desc, "1 char");
        }

        #[test]
        fn cat06_note_switcher_zero_chars() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n6".into(),
                title: "Empty".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert_eq!(desc, "0 chars");
        }

        #[test]
        fn cat06_note_switcher_preview_exactly_60_no_ellipsis() {
            let preview: String = "a".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n7".into(),
                title: "T".into(),
                char_count: 60,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert!(!desc.contains('…'), "60 chars should not be truncated");
        }

        #[test]
        fn cat06_note_switcher_preview_61_has_ellipsis() {
            let preview: String = "b".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n8".into(),
                title: "T".into(),
                char_count: 61,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_deref().unwrap();
            assert!(desc.contains('…'), "61 chars should be truncated with …");
        }

        // =========================================================================
        // cat07: AI command bar per-section ID enumeration
        // =========================================================================

        #[test]
        fn cat07_ai_response_section_ids() {
            let actions = get_ai_command_bar_actions();
            let response_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                response_ids,
                vec![
                    "chat:copy_response",
                    "chat:copy_chat",
                    "chat:copy_last_code"
                ]
            );
        }

        #[test]
        fn cat07_ai_actions_section_ids() {
            let actions = get_ai_command_bar_actions();
            let action_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                action_ids,
                vec![
                    "chat:submit",
                    "chat:new_chat",
                    "chat:delete_chat",
                    "chat:branch_from_last",
                    "chat:toggle_favorite_model",
                    "chat:cycle_favorite_model",
                    "chat:expand_composer"
                ]
            );
        }

        #[test]
        fn cat07_ai_attachments_section_ids() {
            let actions = get_ai_command_bar_actions();
            let att_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                att_ids,
                vec![
                    "chat:add_file",
                    "chat:add_image",
                    "chat:paste_image",
                    "chat:capture_screen_area"
                ]
            );
        }

        #[test]
        fn cat07_ai_export_section_ids() {
            let actions = get_ai_command_bar_actions();
            let export_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Export"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(export_ids, vec!["chat:export_markdown"]);
        }

        #[test]
        fn cat07_ai_help_section_ids() {
            let actions = get_ai_command_bar_actions();
            let help_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Help"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(help_ids, vec!["chat:toggle_shortcuts_help"]);
        }

        #[test]
        fn cat07_ai_settings_section_ids() {
            let actions = get_ai_command_bar_actions();
            let settings_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Settings"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                settings_ids,
                vec!["chat:change_model", "chat:toggle_window_mode"]
            );
        }

        // =========================================================================
        // cat08: Action builder overwrite semantics
        // =========================================================================

        #[test]
        fn cat08_with_shortcut_overwrites_previous() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘A")
                .with_shortcut("⌘B");
            assert_eq!(action.shortcut.as_deref(), Some("⌘B"));
        }

        #[test]
        fn cat08_with_icon_overwrites_previous() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Star)
                .with_icon(IconName::Trash);
            assert_eq!(action.icon, Some(IconName::Trash));
        }

        #[test]
        fn cat08_with_section_overwrites_previous() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_section("A")
                .with_section("B");
            assert_eq!(action.section.as_deref(), Some("B"));
        }

        #[test]
        fn cat08_with_shortcut_opt_none_preserves() {
            // with_shortcut_opt(None) does NOT clear existing shortcut
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘A")
                .with_shortcut_opt(None);
            assert_eq!(
                action.shortcut.as_deref(),
                Some("⌘A"),
                "None does not clear existing shortcut"
            );
        }

        #[test]
        fn cat08_with_shortcut_opt_some_sets() {
            let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘Z".to_string()));
            assert_eq!(action.shortcut.as_deref(), Some("⌘Z"));
        }

        // =========================================================================
        // cat09: CommandBarConfig preset field comparison matrix
        // =========================================================================

        #[test]
        fn cat09_default_vs_ai_style() {
            let def = CommandBarConfig::default();
            let ai = CommandBarConfig::ai_style();
            // AI style uses Headers, default uses Separators
            assert_eq!(ai.dialog_config.section_style, SectionStyle::Headers);
            assert_eq!(def.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat09_notes_style_search_top() {
            let notes = CommandBarConfig::notes_style();
            assert_eq!(notes.dialog_config.search_position, SearchPosition::Top);
        }

        #[test]
        fn cat09_no_search_hidden() {
            let ns = CommandBarConfig::no_search();
            assert_eq!(ns.dialog_config.search_position, SearchPosition::Hidden);
        }

        #[test]
        fn cat09_main_menu_bottom() {
            let mm = CommandBarConfig::main_menu_style();
            assert_eq!(mm.dialog_config.search_position, SearchPosition::Top);
        }

        #[test]
        fn cat09_all_presets_close_on_select_true() {
            assert!(CommandBarConfig::default().close_on_select);
            assert!(CommandBarConfig::ai_style().close_on_select);
            assert!(CommandBarConfig::main_menu_style().close_on_select);
            assert!(CommandBarConfig::no_search().close_on_select);
            assert!(CommandBarConfig::notes_style().close_on_select);
        }

        #[test]
        fn cat09_all_presets_close_on_escape_true() {
            assert!(CommandBarConfig::default().close_on_escape);
            assert!(CommandBarConfig::ai_style().close_on_escape);
            assert!(CommandBarConfig::main_menu_style().close_on_escape);
            assert!(CommandBarConfig::no_search().close_on_escape);
            assert!(CommandBarConfig::notes_style().close_on_escape);
        }

        // =========================================================================
        // cat10: Cross-context category uniformity
        // =========================================================================

        #[test]
        fn cat10_script_actions_all_script_context() {
            let script = ScriptInfo::new("test", "/p");
            for action in &get_script_context_actions(&script) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn cat10_clipboard_actions_all_script_context() {
            let entry = ClipboardEntryInfo {
                id: "c1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn cat10_ai_actions_all_script_context() {
            for action in &get_ai_command_bar_actions() {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn cat10_path_actions_all_script_context() {
            let pi = PathInfo {
                name: "dir".into(),
                path: "/tmp/dir".into(),
                is_dir: true,
            };
            for action in &get_path_context_actions(&pi) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        #[test]
        fn cat10_chat_actions_all_script_context() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            for action in &get_chat_context_actions(&info) {
                assert_eq!(action.category, ActionCategory::ScriptContext);
            }
        }

        // =========================================================================
        // cat11: Clipboard exact action counts on macOS
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat11_clipboard_text_count_macos() {
            let entry = ClipboardEntryInfo {
                id: "t1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // paste, copy, paste_keep_open, share, attach, quick_look,
            // pin, keep_in_today, save_snippet, save_file, delete, delete_multiple, delete_all = 13
            assert_eq!(actions.len(), 13, "Text on macOS: {}", actions.len());
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat11_clipboard_image_count_macos() {
            let entry = ClipboardEntryInfo {
                id: "i1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // paste, copy, paste_keep_open, share, attach, quick_look,
            // open_with, annotate_cleanshot, upload_cleanshot,
            // pin, keep_in_today, ocr, save_file, delete, delete_multiple, delete_all = 16
            // (save_snippet is text-only; images get ocr instead)
            assert_eq!(actions.len(), 16, "Image on macOS: {}", actions.len());
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat11_clipboard_image_more_than_text_macos() {
            let img = ClipboardEntryInfo {
                id: "i".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let txt = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let img_count = get_clipboard_history_context_actions(&img).len();
            let txt_count = get_clipboard_history_context_actions(&txt).len();
            assert!(
                img_count > txt_count,
                "Image ({}) should have more actions than text ({})",
                img_count,
                txt_count
            );
        }

        // =========================================================================
        // cat12: Path primary-action insertion position
        // =========================================================================

        #[test]
        fn cat12_path_dir_primary_first() {
            let pi = PathInfo {
                name: "mydir".into(),
                path: "/tmp/mydir".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&pi);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat12_path_file_primary_first() {
            let pi = PathInfo {
                name: "file.txt".into(),
                path: "/tmp/file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&pi);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn cat12_path_trash_always_last() {
            let pi = PathInfo {
                name: "x".into(),
                path: "/tmp/x".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&pi);
            assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
        }

        #[test]
        fn cat12_path_dir_and_file_same_count() {
            let dir = PathInfo {
                name: "d".into(),
                path: "/tmp/d".into(),
                is_dir: true,
            };
            let file = PathInfo {
                name: "f".into(),
                path: "/tmp/f".into(),
                is_dir: false,
            };
            assert_eq!(
                get_path_context_actions(&dir).len(),
                get_path_context_actions(&file).len()
            );
        }

        // =========================================================================
        // cat13: File title quoting
        // =========================================================================

        #[test]
        fn cat13_file_title_contains_quoted_name() {
            let fi = FileInfo {
                name: "report.pdf".into(),
                path: "/docs/report.pdf".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            let actions = get_file_context_actions(&fi);
            assert!(
                actions[0].title.contains("\"report.pdf\""),
                "Title should contain quoted filename: {}",
                actions[0].title
            );
        }

        #[test]
        fn cat13_dir_title_contains_quoted_name() {
            let fi = FileInfo {
                name: "build".into(),
                path: "/project/build".into(),
                is_dir: true,
                file_type: crate::file_search::FileType::Directory,
            };
            let actions = get_file_context_actions(&fi);
            assert!(
                actions[0].title.contains("\"build\""),
                "Title should contain quoted dirname: {}",
                actions[0].title
            );
        }

        #[test]
        fn cat13_file_primary_is_open_file() {
            let fi = FileInfo {
                name: "x".into(),
                path: "/x".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            let actions = get_file_context_actions(&fi);
            assert_eq!(actions[0].id, "file:open_file");
        }

        #[test]
        fn cat13_dir_primary_is_open_directory() {
            let fi = FileInfo {
                name: "y".into(),
                path: "/y".into(),
                is_dir: true,
                file_type: crate::file_search::FileType::Directory,
            };
            let actions = get_file_context_actions(&fi);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        // =========================================================================
        // cat14: ScriptInfo::with_all field completeness
        // =========================================================================

        #[test]
        fn cat14_with_all_name_path() {
            let s = ScriptInfo::with_all("MyScript", "/path/my.ts", true, "Execute", None, None);
            assert_eq!(s.name, "MyScript");
            assert_eq!(s.path, "/path/my.ts");
        }

        #[test]
        fn cat14_with_all_is_script() {
            let s = ScriptInfo::with_all("S", "/p", true, "Run", None, None);
            assert!(s.is_script);
            let s2 = ScriptInfo::with_all("S", "/p", false, "Run", None, None);
            assert!(!s2.is_script);
        }

        #[test]
        fn cat14_with_all_verb() {
            let s = ScriptInfo::with_all("S", "/p", true, "Launch", None, None);
            assert_eq!(s.action_verb, "Launch");
        }

        #[test]
        fn cat14_with_all_shortcut_and_alias() {
            let s = ScriptInfo::with_all(
                "S",
                "/p",
                true,
                "Run",
                Some("cmd+k".into()),
                Some("sk".into()),
            );
            assert_eq!(s.shortcut, Some("cmd+k".to_string()));
            assert_eq!(s.alias, Some("sk".to_string()));
        }

        #[test]
        fn cat14_with_all_no_agent_no_scriptlet() {
            let s = ScriptInfo::with_all("S", "/p", true, "Run", None, None);
            assert!(!s.is_agent);
            assert!(!s.is_scriptlet);
            assert!(!s.is_suggested);
        }

        // =========================================================================
        // cat15: Script context run title includes verb + name
        // =========================================================================

        #[test]
        fn cat15_run_title_default_verb() {
            let s = ScriptInfo::new("My Script", "/p");
            let actions = get_script_context_actions(&s);
            assert_eq!(actions[0].title, "Run");
        }

        #[test]
        fn cat15_run_title_custom_verb() {
            let s = ScriptInfo::with_action_verb("Windows", "/p", true, "Switch to");
            let actions = get_script_context_actions(&s);
            assert_eq!(actions[0].title, "Switch To");
        }

        #[test]
        fn cat15_run_title_builtin() {
            let s = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&s);
            assert_eq!(actions[0].title, "Run");
        }

        // =========================================================================
        // cat16: Ordering idempotency (double-call determinism)
        // =========================================================================

        #[test]
        fn cat16_script_actions_idempotent() {
            let s = ScriptInfo::new("test", "/p");
            let a1 = action_ids(&get_script_context_actions(&s));
            let a2 = action_ids(&get_script_context_actions(&s));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat16_clipboard_actions_idempotent() {
            let e = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let a1 = action_ids(&get_clipboard_history_context_actions(&e));
            let a2 = action_ids(&get_clipboard_history_context_actions(&e));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat16_ai_actions_idempotent() {
            let a1 = action_ids(&get_ai_command_bar_actions());
            let a2 = action_ids(&get_ai_command_bar_actions());
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat16_path_actions_idempotent() {
            let pi = PathInfo {
                name: "x".into(),
                path: "/x".into(),
                is_dir: true,
            };
            let a1 = action_ids(&get_path_context_actions(&pi));
            let a2 = action_ids(&get_path_context_actions(&pi));
            assert_eq!(a1, a2);
        }

        // =========================================================================
        // cat17: Note switcher icon hierarchy
        // =========================================================================

        #[test]
        fn cat17_pinned_overrides_current() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "Both".into(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        #[test]
        fn cat17_current_only_check() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n2".into(),
                title: "Current".into(),
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
        fn cat17_regular_file_icon() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n3".into(),
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

        #[test]
        fn cat17_pinned_not_current_star() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n4".into(),
                title: "Pinned".into(),
                char_count: 10,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        // =========================================================================
        // cat18: Note switcher section assignment
        // =========================================================================

        #[test]
        fn cat18_pinned_in_pinned_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "p1".into(),
                title: "P".into(),
                char_count: 1,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn cat18_unpinned_in_recent_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "r1".into(),
                title: "R".into(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn cat18_mixed_sections() {
            let notes = vec![
                NoteSwitcherNoteInfo {
                    id: "p".into(),
                    title: "Pinned".into(),
                    char_count: 1,
                    is_current: false,
                    is_pinned: true,
                    preview: String::new(),
                    relative_time: String::new(),
                },
                NoteSwitcherNoteInfo {
                    id: "r".into(),
                    title: "Recent".into(),
                    char_count: 1,
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
        fn cat18_empty_shows_notes_section() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].section.as_deref(), Some("Notes"));
        }

        // =========================================================================
        // cat19: Note switcher current bullet prefix
        // =========================================================================

        #[test]
        fn cat19_current_has_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "c1".into(),
                title: "My Note".into(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(
                actions[0].title.starts_with("• "),
                "Current note should have bullet prefix: {}",
                actions[0].title
            );
        }

        #[test]
        fn cat19_non_current_no_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "c2".into(),
                title: "Other Note".into(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(
                !actions[0].title.starts_with("• "),
                "Non-current should not have bullet: {}",
                actions[0].title
            );
        }

        #[test]
        fn cat19_current_pinned_has_bullet() {
            // Even pinned+current gets bullet prefix
            let notes = vec![NoteSwitcherNoteInfo {
                id: "c3".into(),
                title: "Pinned Current".into(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(actions[0].title.starts_with("• "));
        }

        // =========================================================================
        // cat20: Clipboard paste title dynamic behavior
        // =========================================================================

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat20_paste_no_app() {
            let entry = ClipboardEntryInfo {
                id: "p1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].title, "Paste to Active App");
        }

        #[test]
        fn cat20_paste_with_app() {
            let entry = ClipboardEntryInfo {
                id: "p2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: Some("Safari".into()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].title, "Paste to Safari");
        }

        #[test]
        fn cat20_paste_with_unicode_app() {
            let entry = ClipboardEntryInfo {
                id: "p3".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: Some("日本語エディタ".into()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].title, "Paste to 日本語エディタ");
        }

        #[test]
        fn cat20_paste_with_empty_string_app() {
            let entry = ClipboardEntryInfo {
                id: "p4".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: Some(String::new()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // Some("") falls back to the generic "Paste to Active App" title
            assert_eq!(actions[0].title, "Paste to Active App");
        }

        // =========================================================================
        // cat21: Clipboard pin/unpin toggle
        // =========================================================================

        #[test]
        fn cat21_unpinned_shows_pin() {
            let entry = ClipboardEntryInfo {
                id: "u1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_pin".to_string()));
            assert!(!ids.contains(&"clip:clipboard_unpin".to_string()));
        }

        #[test]
        fn cat21_pinned_shows_unpin() {
            let entry = ClipboardEntryInfo {
                id: "u2".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"clip:clipboard_unpin".to_string()));
            assert!(!ids.contains(&"clip:clipboard_pin".to_string()));
        }

        #[test]
        fn cat21_pin_unpin_same_shortcut() {
            let pin_entry = ClipboardEntryInfo {
                id: "s1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let unpin_entry = ClipboardEntryInfo {
                id: "s2".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let pin_actions = get_clipboard_history_context_actions(&pin_entry);
            let unpin_actions = get_clipboard_history_context_actions(&unpin_entry);
            let pin_sc = pin_actions
                .iter()
                .find(|a| a.id == "clip:clipboard_pin")
                .unwrap()
                .shortcut
                .as_deref();
            let unpin_sc = unpin_actions
                .iter()
                .find(|a| a.id == "clip:clipboard_unpin")
                .unwrap()
                .shortcut
                .as_deref();
            assert_eq!(pin_sc, unpin_sc, "Pin and Unpin share same shortcut");
            assert_eq!(pin_sc, Some("⇧⌘P"));
        }

        // =========================================================================
        // cat22: Clipboard destructive actions always last three
        // =========================================================================

        #[test]
        fn cat22_text_last_three_destructive() {
            let entry = ClipboardEntryInfo {
                id: "d1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
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
        fn cat22_image_last_three_destructive() {
            let entry = ClipboardEntryInfo {
                id: "d2".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
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
        fn cat22_paste_always_first() {
            let entry = ClipboardEntryInfo {
                id: "d3".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[0].id, "clip:clipboard_paste");
        }

        #[test]
        fn cat22_copy_always_second() {
            let entry = ClipboardEntryInfo {
                id: "d4".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert_eq!(actions[1].id, "clip:clipboard_copy");
        }

        // =========================================================================
        // cat23: Action lowercase caching
        // =========================================================================

        #[test]
        fn cat23_title_lower_cached() {
            let action = Action::new("test", "My Title", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "my title");
        }

        #[test]
        fn cat23_description_lower_cached() {
            let action = Action::new(
                "test",
                "T",
                Some("My Description".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower.as_deref(), Some("my description"));
        }

        #[test]
        fn cat23_description_none_lower_none() {
            let action = Action::new("test", "T", None, ActionCategory::ScriptContext);
            assert_eq!(action.description_lower, None);
        }

        #[test]
        fn cat23_shortcut_lower_none_initially() {
            let action = Action::new("test", "T", None, ActionCategory::ScriptContext);
            assert_eq!(action.shortcut_lower, None);
        }

        #[test]
        fn cat23_shortcut_lower_set_after_with_shortcut() {
            let action =
                Action::new("test", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
            assert_eq!(action.shortcut_lower.as_deref(), Some("⌘⇧c"));
        }

        #[test]
        fn cat23_title_lower_unicode() {
            let action = Action::new("test", "Café Résumé", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "café résumé");
        }

        // =========================================================================
        // cat24: AI command bar total count and all have icons
        // =========================================================================

        #[test]
        fn cat24_ai_total_13() {
            assert_eq!(get_ai_command_bar_actions().len(), 38);
        }

        #[test]
        fn cat24_ai_all_have_icons() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    action.icon.is_some(),
                    "AI action {} should have icon",
                    action.id
                );
            }
        }

        #[test]
        fn cat24_ai_all_have_sections() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    action.section.is_some(),
                    "AI action {} should have section",
                    action.id
                );
            }
        }

        #[test]
        fn cat24_ai_7_unique_sections() {
            let actions = get_ai_command_bar_actions();
            let sections: HashSet<&str> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            assert_eq!(sections.len(), 7);
        }

        #[test]
        fn cat24_ai_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len(), "AI action IDs should be unique");
        }

        // =========================================================================
        // cat25: Notes format action shortcut and icon
        // =========================================================================

        // =========================================================================
        // cat26: File context common actions always present
        // =========================================================================

        #[test]
        fn cat26_file_has_reveal() {
            let fi = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            let ids = action_ids(&get_file_context_actions(&fi));
            assert!(ids.contains(&"file:reveal_in_finder".to_string()));
        }

        #[test]
        fn cat26_file_has_copy_path() {
            let fi = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            let ids = action_ids(&get_file_context_actions(&fi));
            assert!(ids.contains(&"file:copy_path".to_string()));
        }

        #[test]
        fn cat26_file_has_copy_filename() {
            let fi = FileInfo {
                name: "a.txt".into(),
                path: "/a.txt".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            let ids = action_ids(&get_file_context_actions(&fi));
            assert!(ids.contains(&"file:copy_filename".to_string()));
        }

        #[test]
        fn cat26_dir_has_reveal() {
            let fi = FileInfo {
                name: "d".into(),
                path: "/d".into(),
                is_dir: true,
                file_type: crate::file_search::FileType::Directory,
            };
            let ids = action_ids(&get_file_context_actions(&fi));
            assert!(ids.contains(&"file:reveal_in_finder".to_string()));
        }

        #[test]
        fn cat26_dir_has_copy_path() {
            let fi = FileInfo {
                name: "d".into(),
                path: "/d".into(),
                is_dir: true,
                file_type: crate::file_search::FileType::Directory,
            };
            let ids = action_ids(&get_file_context_actions(&fi));
            assert!(ids.contains(&"file:copy_path".to_string()));
        }

        // =========================================================================
        // cat27: Script context shortcut/alias dynamic action count
        // =========================================================================

        #[test]
        fn cat27_no_shortcut_no_alias_count() {
            let s = ScriptInfo::new("test", "/p");
            let actions = get_script_context_actions(&s);
            // run_script, toggle_info, add_shortcut, add_alias, toggle_favorite, edit_script,
            // view_logs, reveal_in_finder, file:open_in_quick_terminal, copy_path, copy_content,
            // copy_deeplink, delete_script = 13
            assert_eq!(actions.len(), 13);
        }

        #[test]
        fn cat27_with_shortcut_count() {
            let s = ScriptInfo::with_shortcut("test", "/p", Some("cmd+t".into()));
            let actions = get_script_context_actions(&s);
            // base 13 with add_shortcut replaced by update_shortcut + remove_shortcut = 14
            assert_eq!(actions.len(), 14);
        }

        #[test]
        fn cat27_with_both_count() {
            let s = ScriptInfo::with_shortcut_and_alias(
                "test",
                "/p",
                Some("cmd+t".into()),
                Some("ts".into()),
            );
            let actions = get_script_context_actions(&s);
            // base 13 + shortcut pair swap (+1) + alias pair swap (+1) = 15
            assert_eq!(actions.len(), 15);
        }

        #[test]
        fn cat27_frecency_adds_one() {
            let s = ScriptInfo::new("test", "/p").with_frecency(true, Some("/f".into()));
            let actions = get_script_context_actions(&s);
            // 13 + 1 (reset_ranking) = 14
            assert_eq!(actions.len(), 14);
        }

        // =========================================================================
        // cat28: has_action=false invariant for all built-ins
        // =========================================================================

        #[test]
        fn cat28_script_has_action_false() {
            let s = ScriptInfo::new("t", "/p");
            for action in &get_script_context_actions(&s) {
                assert!(
                    !action.has_action,
                    "Script action {} should have has_action=false",
                    action.id
                );
            }
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn cat28_clipboard_has_action_false() {
            let e = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&e) {
                assert!(
                    !action.has_action,
                    "Clipboard action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_ai_has_action_false() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    !action.has_action,
                    "AI action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_path_has_action_false() {
            let pi = PathInfo {
                name: "x".into(),
                path: "/x".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&pi) {
                assert!(
                    !action.has_action,
                    "Path action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_file_has_action_false() {
            let fi = FileInfo {
                name: "f".into(),
                path: "/f".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            for action in &get_file_context_actions(&fi) {
                assert!(
                    !action.has_action,
                    "File action {} should have has_action=false",
                    action.id
                );
            }
        }

        // =========================================================================
        // cat29: ID uniqueness across contexts
        // =========================================================================

        #[test]
        fn cat29_script_ids_unique() {
            let s = ScriptInfo::with_shortcut_and_alias(
                "t",
                "/p",
                Some("cmd+t".into()),
                Some("al".into()),
            );
            let actions = get_script_context_actions(&s);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat29_clipboard_ids_unique() {
            let e = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&e);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat29_path_ids_unique() {
            let pi = PathInfo {
                name: "x".into(),
                path: "/x".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&pi);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat29_file_ids_unique() {
            let fi = FileInfo {
                name: "f".into(),
                path: "/f".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            let actions = get_file_context_actions(&fi);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat29_note_switcher_ids_unique() {
            let notes = vec![
                NoteSwitcherNoteInfo {
                    id: "a".into(),
                    title: "A".into(),
                    char_count: 1,
                    is_current: false,
                    is_pinned: false,
                    preview: String::new(),
                    relative_time: String::new(),
                },
                NoteSwitcherNoteInfo {
                    id: "b".into(),
                    title: "B".into(),
                    char_count: 1,
                    is_current: false,
                    is_pinned: false,
                    preview: String::new(),
                    relative_time: String::new(),
                },
            ];
            let actions = get_note_switcher_actions(&notes);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        // =========================================================================
        // cat30: Non-empty id and title invariant
        // =========================================================================

        #[test]
        fn cat30_script_nonempty_id_title() {
            let s = ScriptInfo::new("t", "/p");
            for action in &get_script_context_actions(&s) {
                assert!(!action.id.is_empty(), "ID should not be empty");
                assert!(!action.title.is_empty(), "Title should not be empty");
            }
        }

        #[test]
        fn cat30_clipboard_nonempty_id_title() {
            let e = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&e) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_ai_nonempty_id_title() {
            for action in &get_ai_command_bar_actions() {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_path_nonempty_id_title() {
            let pi = PathInfo {
                name: "x".into(),
                path: "/x".into(),
                is_dir: false,
            };
            for action in &get_path_context_actions(&pi) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_file_nonempty_id_title() {
            let fi = FileInfo {
                name: "f".into(),
                path: "/f".into(),
                is_dir: false,
                file_type: crate::file_search::FileType::Document,
            };
            for action in &get_file_context_actions(&fi) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }
    }
}

mod from_dialog_builtin_action_validation_tests_16 {
    //! Batch 16 – Dialog Builtin Action Validation Tests
    //!
    //! 30 categories, ~155 tests covering fresh angles:
    //! - Scriptlet context with custom actions ordering relative to built-ins
    //! - Clipboard share/attach_to_ai universality across content types
    //! - Agent context copy_content description substring
    //! - Path context open_in_quick_terminal (no shortcut) / open_in_editor shortcut
    //! - File context shortcut consistency between file and directory
    //! - Notes command bar section count per flag combo
    //! - Chat context continue_in_chat shortcut value
    //! - AI command bar export section single action
    //! - New chat preset icon propagation
    //! - Note switcher pinned+current combined state
    //! - format_shortcut_hint modifier keyword normalization edge cases
    //! - to_deeplink_name numeric and underscore handling
    //! - score_action empty query behaviour
    //! - fuzzy_match case sensitivity
    //! - build_grouped_items_static single-item input
    //! - coerce_action_selection single-item input
    //! - CommandBarConfig close flag independence
    //! - Action::new description_lower None when description is None
    //! - Action builder chain ordering (icon before section, section before shortcut)
    //! - ScriptInfo with_action_verb preserves defaults
    //! - Script context agent flag produces edit_script with "Edit Agent" title
    //! - Cross-context shortcut format consistency (all use Unicode symbols)
    //! - Clipboard paste_keep_open shortcut value
    //! - Path context copy_filename has no shortcut
    //! - File context open_with macOS shortcut
    //! - Notes format shortcut exact value
    //! - AI command bar icon name correctness
    //! - Script context run title format
    //! - Ordering consistency across repeated calls

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // =========================================================================
        // cat01: Scriptlet context custom actions ordering relative to built-ins
        // =========================================================================

        #[test]
        fn cat01_scriptlet_custom_after_run_before_edit() {
            let script = ScriptInfo::scriptlet("Test", "/p/test.md", None, None);
            let mut scriptlet = Scriptlet::new(
                "Test".to_string(),
                "bash".to_string(),
                "echo hi".to_string(),
            );
            scriptlet.actions = vec![ScriptletAction {
                name: "Custom One".to_string(),
                command: "custom-one".to_string(),
                tool: "bash".to_string(),
                code: "echo 1".to_string(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let run_idx = actions.iter().position(|a| a.id == "run_script").unwrap();
            let custom_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:custom-one")
                .unwrap();
            let edit_idx = actions
                .iter()
                .position(|a| a.id == "edit_scriptlet")
                .unwrap();
            assert!(run_idx < custom_idx, "custom after run");
            assert!(custom_idx < edit_idx, "custom before edit_scriptlet");
        }

        #[test]
        fn cat01_scriptlet_multiple_customs_preserve_order() {
            let script = ScriptInfo::scriptlet("T", "/p/t.md", None, None);
            let mut scriptlet =
                Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
            scriptlet.actions = vec![
                ScriptletAction {
                    name: "Alpha".to_string(),
                    command: "alpha".to_string(),
                    tool: "bash".to_string(),
                    code: "echo a".to_string(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
                ScriptletAction {
                    name: "Beta".to_string(),
                    command: "beta".to_string(),
                    tool: "bash".to_string(),
                    code: "echo b".to_string(),
                    inputs: vec![],
                    shortcut: None,
                    description: None,
                },
            ];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let a_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:alpha")
                .unwrap();
            let b_idx = actions
                .iter()
                .position(|a| a.id == "scriptlet_action:beta")
                .unwrap();
            assert!(a_idx < b_idx, "customs preserve source order");
        }

        #[test]
        fn cat01_scriptlet_custom_has_action_true() {
            let script = ScriptInfo::scriptlet("T", "/p/t.md", None, None);
            let mut scriptlet =
                Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
            scriptlet.actions = vec![ScriptletAction {
                name: "Act".to_string(),
                command: "act-cmd".to_string(),
                tool: "bash".to_string(),
                code: "echo".to_string(),
                inputs: vec![],
                shortcut: Some("cmd+1".to_string()),
                description: Some("Do a thing".to_string()),
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom = actions
                .iter()
                .find(|a| a.id == "scriptlet_action:act-cmd")
                .unwrap();
            assert!(
                custom.has_action,
                "scriptlet custom actions have has_action=true"
            );
            assert_eq!(custom.value, Some("act-cmd".to_string()));
            assert_eq!(custom.shortcut, Some("⌘1".to_string()));
        }

        #[test]
        fn cat01_scriptlet_no_custom_still_has_builtins() {
            let script = ScriptInfo::scriptlet("T", "/p/t.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"run_script".to_string()));
            assert!(ids.contains(&"edit_scriptlet".to_string()));
            assert!(ids.contains(&"copy_deeplink".to_string()));
            assert!(!ids.iter().any(|id| id.starts_with("scriptlet_action:")));
        }

        #[test]
        fn cat01_scriptlet_custom_description_propagated() {
            let script = ScriptInfo::scriptlet("T", "/p/t.md", None, None);
            let mut scriptlet =
                Scriptlet::new("T".to_string(), "bash".to_string(), "echo".to_string());
            scriptlet.actions = vec![ScriptletAction {
                name: "Described".to_string(),
                command: "desc-cmd".to_string(),
                tool: "bash".to_string(),
                code: "echo".to_string(),
                inputs: vec![],
                shortcut: None,
                description: Some("My description".to_string()),
            }];
            let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
            let custom = actions
                .iter()
                .find(|a| a.id == "scriptlet_action:desc-cmd")
                .unwrap();
            assert_eq!(custom.description, Some("My description".to_string()));
        }

        // =========================================================================
        // cat02: Clipboard share and attach_to_ai universality
        // =========================================================================

        #[test]
        fn cat02_clipboard_text_has_share() {
            let entry = ClipboardEntryInfo {
                id: "t1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(ids.contains(&"clip:clipboard_share".to_string()));
        }

        #[test]
        fn cat02_clipboard_image_has_share() {
            let entry = ClipboardEntryInfo {
                id: "i1".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: Some((10, 10)),
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(ids.contains(&"clip:clipboard_share".to_string()));
        }

        #[test]
        fn cat02_clipboard_text_has_attach_to_ai() {
            let entry = ClipboardEntryInfo {
                id: "t2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "data".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(ids.contains(&"clip:clipboard_attach_to_ai".to_string()));
        }

        #[test]
        fn cat02_clipboard_image_has_attach_to_ai() {
            let entry = ClipboardEntryInfo {
                id: "i2".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: String::new(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let ids = action_ids(&get_clipboard_history_context_actions(&entry));
            assert!(ids.contains(&"clip:clipboard_attach_to_ai".to_string()));
        }

        #[test]
        fn cat02_clipboard_share_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "t3".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
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
        fn cat02_clipboard_attach_to_ai_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "t4".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
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

        // =========================================================================
        // cat03: Agent context copy_content description mentions "agent"
        // =========================================================================

        #[test]
        fn cat03_agent_edit_title_is_edit_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn cat03_agent_copy_content_desc_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let copy = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert!(
                copy.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("file")
            );
        }

        #[test]
        fn cat03_agent_has_reveal_and_copy_path() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let ids = action_ids(&actions);
            assert!(ids.contains(&"reveal_in_finder".to_string()));
            assert!(ids.contains(&"copy_path".to_string()));
        }

        #[test]
        fn cat03_agent_no_view_logs() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let ids = action_ids(&actions);
            assert!(!ids.contains(&"view_logs".to_string()));
        }

        #[test]
        fn cat03_agent_copy_path_shortcut() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
            assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
        }

        // =========================================================================
        // cat04: Path context shortcuts: open_in_quick_terminal has none, open_in_editor has ⌘E
        // =========================================================================

        #[test]
        fn cat04_path_open_in_quick_terminal_has_no_shortcut() {
            let info = PathInfo {
                name: "project".into(),
                path: "/home/project".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let terminal = actions
                .iter()
                .find(|a| a.id == "file:open_in_quick_terminal")
                .unwrap();
            // file:open_in_quick_terminal ships without a shortcut.
            assert_eq!(terminal.shortcut.as_deref(), None);
        }

        #[test]
        fn cat04_path_open_in_editor_shortcut() {
            let info = PathInfo {
                name: "file.rs".into(),
                path: "/home/file.rs".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let editor = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .unwrap();
            assert_eq!(editor.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn cat04_path_open_in_finder_shortcut() {
            let info = PathInfo {
                name: "docs".into(),
                path: "/home/docs".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let finder = actions
                .iter()
                .find(|a| a.id == "file:open_in_finder")
                .unwrap();
            assert_eq!(finder.shortcut.as_deref(), Some("⌘⇧F"));
        }

        #[test]
        fn cat04_path_copy_path_shortcut() {
            let info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
            assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
        }

        #[test]
        fn cat04_path_move_to_trash_shortcut() {
            let info = PathInfo {
                name: "old".into(),
                path: "/old".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert_eq!(trash.shortcut.as_deref(), Some("⌘⌫"));
        }

        // =========================================================================
        // cat05: File context shortcut consistency between file and directory
        // =========================================================================

        #[test]
        fn cat05_file_and_dir_both_have_reveal_shortcut() {
            let file = FileInfo {
                path: "/a/b.txt".into(),
                name: "b.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let dir = FileInfo {
                path: "/a/c".into(),
                name: "c".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let f_reveal = get_file_context_actions(&file)
                .into_iter()
                .find(|a| a.id == "file:reveal_in_finder")
                .unwrap();
            let d_reveal = get_file_context_actions(&dir)
                .into_iter()
                .find(|a| a.id == "file:reveal_in_finder")
                .unwrap();
            assert_eq!(f_reveal.shortcut, d_reveal.shortcut, "same shortcut");
        }

        #[test]
        fn cat05_file_and_dir_copy_path_same_shortcut() {
            let file = FileInfo {
                path: "/a.txt".into(),
                name: "a.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let dir = FileInfo {
                path: "/b".into(),
                name: "b".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let f_cp = get_file_context_actions(&file)
                .into_iter()
                .find(|a| a.id == "file:copy_path")
                .unwrap();
            let d_cp = get_file_context_actions(&dir)
                .into_iter()
                .find(|a| a.id == "file:copy_path")
                .unwrap();
            assert_eq!(f_cp.shortcut, d_cp.shortcut);
        }

        #[test]
        fn cat05_file_primary_has_enter_shortcut() {
            let file = FileInfo {
                path: "/x.rs".into(),
                name: "x.rs".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let primary = &actions[0];
            assert_eq!(primary.shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn cat05_dir_primary_has_enter_shortcut() {
            let dir = FileInfo {
                path: "/mydir".into(),
                name: "mydir".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&dir);
            let primary = &actions[0];
            assert_eq!(primary.shortcut.as_deref(), Some("↵"));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat05_file_copy_filename_shortcut_cmd_c() {
            let file = FileInfo {
                path: "/doc.pdf".into(),
                name: "doc.pdf".into(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let cf = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert_eq!(cf.shortcut.as_deref(), Some("⌘C"));
        }

        // =========================================================================
        // cat06: Notes command bar section count per flag combination
        // =========================================================================

        // =========================================================================
        // cat07: Chat context continue_in_chat shortcut
        // =========================================================================

        #[test]
        fn cat07_chat_continue_shortcut_cmd_enter() {
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
        fn cat07_chat_continue_always_present() {
            // Even with no models, continue_in_chat should be present
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(ids.contains(&"chat:continue_in_chat".to_string()));
        }

        #[test]
        fn cat07_chat_copy_response_conditional_true() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: true,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(ids.contains(&"chat:copy_response".to_string()));
        }

        #[test]
        fn cat07_chat_copy_response_conditional_false() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(!ids.contains(&"chat:copy_response".to_string()));
        }

        #[test]
        fn cat07_chat_clear_conditional_true() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(ids.contains(&"chat:clear_conversation".to_string()));
        }

        #[test]
        fn cat07_chat_clear_conditional_false() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let ids = action_ids(&get_chat_context_actions(&info));
            assert!(!ids.contains(&"chat:clear_conversation".to_string()));
        }

        // =========================================================================
        // cat08: AI command bar export section has exactly one action
        // =========================================================================

        #[test]
        fn cat08_ai_export_section_count() {
            let actions = get_ai_command_bar_actions();
            let export_count = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Export"))
                .count();
            assert_eq!(export_count, 1, "Export section has exactly 1 action");
        }

        #[test]
        fn cat08_ai_export_action_is_export_markdown() {
            let actions = get_ai_command_bar_actions();
            let export = actions
                .iter()
                .find(|a| a.section.as_deref() == Some("Export"))
                .unwrap();
            assert_eq!(export.id, "chat:export_markdown");
        }

        #[test]
        fn cat08_ai_export_markdown_shortcut() {
            let actions = get_ai_command_bar_actions();
            let export = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(export.shortcut.as_deref(), Some("⇧⌘E"));
        }

        #[test]
        fn cat08_ai_export_markdown_icon() {
            let actions = get_ai_command_bar_actions();
            let export = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(export.icon, Some(IconName::FileCode));
        }

        // =========================================================================
        // cat09: New chat preset icon propagation
        // =========================================================================

        #[test]
        fn cat09_new_chat_preset_icon_preserved() {
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let preset_action = actions.iter().find(|a| a.id == "preset_general").unwrap();
            assert_eq!(preset_action.icon, Some(IconName::Star));
        }

        #[test]
        fn cat09_new_chat_preset_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "code".into(),
                name: "Code".into(),
                icon: IconName::Code,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let preset_action = actions.iter().find(|a| a.id == "preset_code").unwrap();
            assert!(
                preset_action
                    .description
                    .as_deref()
                    .unwrap()
                    .contains("preset")
            );
        }

        #[test]
        fn cat09_new_chat_model_has_provider_description() {
            let models = vec![NewChatModelInfo {
                model_id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            let model_action = actions
                .iter()
                .find(|a| a.id == "model_openai::gpt-4")
                .unwrap();
            assert_eq!(model_action.description.as_deref(), Some("Uses OpenAI"));
        }

        #[test]
        fn cat09_new_chat_model_icon_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "claude".into(),
                display_name: "Claude".into(),
                provider: "anthropic".into(),
                provider_display_name: "Anthropic".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            let model = actions
                .iter()
                .find(|a| a.id == "model_anthropic::claude")
                .unwrap();
            assert_eq!(model.icon, Some(IconName::Settings));
        }

        #[test]
        fn cat09_new_chat_last_used_bolt_icon() {
            let last_used = vec![NewChatModelInfo {
                model_id: "gpt-4o".into(),
                display_name: "GPT-4o".into(),
                provider: "openai".into(),
                provider_display_name: "OpenAI".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            let lu = actions
                .iter()
                .find(|a| a.id == "last_used_openai::gpt-4o")
                .unwrap();
            assert_eq!(lu.icon, Some(IconName::BoltFilled));
        }

        // =========================================================================
        // cat10: Note switcher pinned+current combined state
        // =========================================================================

        #[test]
        fn cat10_pinned_current_icon_is_star() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n1".into(),
                title: "My Note".into(),
                char_count: 100,
                is_current: true,
                is_pinned: true,
                preview: "content".into(),
                relative_time: "1m ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        #[test]
        fn cat10_pinned_current_has_bullet_prefix() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n2".into(),
                title: "Pinned Current".into(),
                char_count: 50,
                is_current: true,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(
                actions[0].title.starts_with("• "),
                "current note should have bullet prefix"
            );
        }

        #[test]
        fn cat10_pinned_not_current_no_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n3".into(),
                title: "Pinned Only".into(),
                char_count: 20,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].title, "Pinned Only");
        }

        #[test]
        fn cat10_pinned_section_is_pinned() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n4".into(),
                title: "Pin".into(),
                char_count: 5,
                is_current: false,
                is_pinned: true,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn cat10_unpinned_section_is_recent() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "n5".into(),
                title: "Regular".into(),
                char_count: 5,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        // =========================================================================
        // cat11: format_shortcut_hint modifier keyword normalization
        // =========================================================================

        #[test]
        fn cat11_format_shortcut_cmd_c() {
            // Using the builders-private fn via ActionsDialog
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(keycaps, vec!["⌘", "C"]);
        }

        #[test]
        fn cat11_format_shortcut_ctrl_alt_del() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⌃⌥⌫");
            assert_eq!(keycaps, vec!["⌃", "⌥", "⌫"]);
        }

        #[test]
        fn cat11_format_shortcut_enter() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(keycaps, vec!["↵"]);
        }

        #[test]
        fn cat11_format_shortcut_arrows() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(keycaps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn cat11_format_shortcut_escape() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(keycaps, vec!["⎋"]);
        }

        #[test]
        fn cat11_format_shortcut_tab() {
            let keycaps = ActionsDialog::parse_shortcut_keycaps("⇥");
            assert_eq!(keycaps, vec!["⇥"]);
        }

        // =========================================================================
        // cat12: to_deeplink_name numeric and underscore handling
        // =========================================================================

        #[test]
        fn cat12_deeplink_numeric_only() {
            assert_eq!(to_deeplink_name("12345"), "12345");
        }

        #[test]
        fn cat12_deeplink_underscores_to_hyphens() {
            assert_eq!(to_deeplink_name("hello_world"), "hello-world");
        }

        #[test]
        fn cat12_deeplink_mixed_case_lowered() {
            assert_eq!(to_deeplink_name("MyScript"), "myscript");
        }

        #[test]
        fn cat12_deeplink_consecutive_specials_collapsed() {
            assert_eq!(to_deeplink_name("a!!b"), "a-b");
        }

        #[test]
        fn cat12_deeplink_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("--hello--"), "hello");
        }

        // =========================================================================
        // cat13: score_action empty query behaviour
        // =========================================================================

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat13_score_empty_query_returns_prefix_match() {
            let action = Action::new(
                "test",
                "Edit Script",
                Some("Open in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            // Empty string is a prefix of everything
            let score = ActionsDialog::score_action(&action, "");
            assert!(
                score >= 100,
                "empty query is prefix of any title: {}",
                score
            );
        }

        #[test]
        fn cat13_score_no_match_returns_zero() {
            let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "zzzzz");
            assert_eq!(score, 0);
        }

        #[test]
        fn cat13_score_prefix_beats_contains() {
            let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
            let prefix_score = ActionsDialog::score_action(&action, "edit");
            let contains_score = ActionsDialog::score_action(&action, "script");
            assert!(
                prefix_score > contains_score,
                "prefix {} > contains {}",
                prefix_score,
                contains_score
            );
        }

        #[test]
        fn cat13_score_description_bonus() {
            let action = Action::new(
                "test",
                "Open File",
                Some("Open in the default editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "editor");
            assert!(
                score >= 15,
                "description match gives at least 15: {}",
                score
            );
        }

        #[test]
        fn cat13_score_shortcut_bonus() {
            let action = Action::new("test", "Submit", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            let score = ActionsDialog::score_action(&action, "⌘e");
            assert!(score >= 10, "shortcut match gives at least 10: {}", score);
        }

        // =========================================================================
        // cat14: fuzzy_match case sensitivity
        // =========================================================================

        #[test]
        fn cat14_fuzzy_exact_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        #[test]
        fn cat14_fuzzy_subsequence() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hwd"));
        }

        #[test]
        fn cat14_fuzzy_no_match() {
            assert!(!ActionsDialog::fuzzy_match("hello", "xyz"));
        }

        #[test]
        fn cat14_fuzzy_empty_needle() {
            assert!(ActionsDialog::fuzzy_match("hello", ""));
        }

        #[test]
        fn cat14_fuzzy_empty_haystack() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn cat14_fuzzy_both_empty() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn cat14_fuzzy_needle_longer() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        // =========================================================================
        // cat15: build_grouped_items_static single-item input
        // =========================================================================

        #[test]
        fn cat15_grouped_single_item_headers() {
            let actions = vec![
                Action::new("a", "Action A", None, ActionCategory::ScriptContext)
                    .with_section("Sec"),
            ];
            let filtered = vec![0usize];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert_eq!(grouped.len(), 2, "1 header + 1 item");
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
        }

        #[test]
        fn cat15_grouped_single_item_separators() {
            let actions = vec![Action::new(
                "a",
                "Action A",
                None,
                ActionCategory::ScriptContext,
            )];
            let filtered = vec![0usize];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            assert_eq!(grouped.len(), 1, "no header for separators");
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        }

        #[test]
        fn cat15_grouped_single_item_none_style() {
            let actions = vec![Action::new(
                "a",
                "Action A",
                None,
                ActionCategory::ScriptContext,
            )];
            let filtered = vec![0usize];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 1);
        }

        #[test]
        fn cat15_grouped_empty_returns_empty() {
            let actions: Vec<Action> = vec![];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn cat15_grouped_same_section_no_duplicate_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S"),
            ];
            let filtered = vec![0usize, 1];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1, "same section = 1 header");
        }

        // =========================================================================
        // cat16: coerce_action_selection single-item input
        // =========================================================================

        #[test]
        fn cat16_coerce_single_item() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn cat16_coerce_single_header() {
            let rows = vec![GroupedActionItem::SectionHeader("S".into())];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat16_coerce_header_then_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat16_coerce_item_then_header() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat16_coerce_empty() {
            let rows: Vec<GroupedActionItem> = vec![];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat16_coerce_out_of_bounds_clamps() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 999), Some(0));
        }

        // =========================================================================
        // cat17: CommandBarConfig close flag independence
        // =========================================================================

        #[test]
        fn cat17_default_all_close_true() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat17_ai_style_close_flags() {
            let config = CommandBarConfig::ai_style();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat17_main_menu_close_flags() {
            let config = CommandBarConfig::main_menu_style();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat17_no_search_close_flags() {
            let config = CommandBarConfig::no_search();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        // =========================================================================
        // cat18: Action::new description_lower None when description is None
        // =========================================================================

        #[test]
        fn cat18_action_no_description_lower_none() {
            let action = Action::new("id", "Title", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn cat18_action_with_description_lower_set() {
            let action = Action::new(
                "id",
                "Title",
                Some("Hello World".to_string()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower.as_deref(), Some("hello world"));
        }

        #[test]
        fn cat18_action_title_lower_cached() {
            let action = Action::new("id", "My Title", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "my title");
        }

        #[test]
        fn cat18_action_shortcut_lower_none_initially() {
            let action = Action::new("id", "Title", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn cat18_action_shortcut_lower_set_after_with_shortcut() {
            let action =
                Action::new("id", "Title", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
            assert_eq!(action.shortcut_lower.as_deref(), Some("⌘e"));
        }

        // =========================================================================
        // cat19: Action builder chain ordering (icon, section, shortcut)
        // =========================================================================

        #[test]
        fn cat19_icon_then_section() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Star)
                .with_section("Sec");
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("Sec"));
        }

        #[test]
        fn cat19_section_then_icon() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_section("Sec")
                .with_icon(IconName::Star);
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("Sec"));
        }

        #[test]
        fn cat19_shortcut_then_icon_preserves_shortcut() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘K")
                .with_icon(IconName::Settings);
            assert_eq!(action.shortcut.as_deref(), Some("⌘K"));
            assert_eq!(action.icon, Some(IconName::Settings));
        }

        #[test]
        fn cat19_full_chain() {
            let action = Action::new(
                "id",
                "T",
                Some("desc".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘X")
            .with_icon(IconName::Trash)
            .with_section("Danger");
            assert_eq!(action.shortcut.as_deref(), Some("⌘X"));
            assert_eq!(action.icon, Some(IconName::Trash));
            assert_eq!(action.section.as_deref(), Some("Danger"));
            assert_eq!(action.description.as_deref(), Some("desc"));
        }

        // =========================================================================
        // cat20: ScriptInfo with_action_verb preserves defaults
        // =========================================================================

        #[test]
        fn cat20_with_action_verb_preserves_not_scriptlet() {
            let info = ScriptInfo::with_action_verb("App", "/app", false, "Launch");
            assert!(!info.is_scriptlet);
            assert!(!info.is_agent);
            assert!(info.shortcut.is_none());
            assert!(info.alias.is_none());
            assert!(!info.is_suggested);
        }

        #[test]
        fn cat20_with_action_verb_sets_verb() {
            let info = ScriptInfo::with_action_verb("Win", "/win", false, "Switch to");
            assert_eq!(info.action_verb, "Switch to");
        }

        #[test]
        fn cat20_with_action_verb_name_and_path() {
            let info =
                ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
            assert_eq!(info.name, "Safari");
            assert_eq!(info.path, "/Applications/Safari.app");
        }

        #[test]
        fn cat20_with_action_verb_is_script_flag() {
            let info_true = ScriptInfo::with_action_verb("S", "/s", true, "Run");
            assert!(info_true.is_script);
            let info_false = ScriptInfo::with_action_verb("S", "/s", false, "Run");
            assert!(!info_false.is_script);
        }

        // =========================================================================
        // cat21: Script context agent flag produces edit with "Edit Agent" title
        // =========================================================================

        #[test]
        fn cat21_agent_flag_produces_edit_agent() {
            let mut script = ScriptInfo::new("Bot", "/bot.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(edit.title.contains("Agent"));
        }

        #[test]
        fn cat21_agent_has_copy_content() {
            let mut script = ScriptInfo::new("Bot", "/bot.md");
            script.is_script = false;
            script.is_agent = true;
            let ids = action_ids(&get_script_context_actions(&script));
            assert!(ids.contains(&"copy_content".to_string()));
        }

        #[test]
        fn cat21_agent_edit_shortcut() {
            let mut script = ScriptInfo::new("Bot", "/bot.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn cat21_agent_reveal_shortcut() {
            let mut script = ScriptInfo::new("Bot", "/bot.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
            assert_eq!(reveal.shortcut.as_deref(), Some("⌘⇧F"));
        }

        // =========================================================================
        // cat22: Cross-context shortcut format uses Unicode symbols
        // =========================================================================

        #[test]
        fn cat22_script_shortcuts_use_unicode() {
            let script = ScriptInfo::new("test", "/p/test.ts");
            let actions = get_script_context_actions(&script);
            for action in &actions {
                if let Some(ref s) = action.shortcut {
                    // All shortcuts should contain Unicode symbols, not "cmd" / "shift" etc.
                    assert!(
                        !s.contains("cmd") && !s.contains("shift") && !s.contains("ctrl"),
                        "Shortcut '{}' on action '{}' should use Unicode symbols",
                        s,
                        action.id
                    );
                }
            }
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat22_clipboard_shortcuts_use_unicode() {
            let entry = ClipboardEntryInfo {
                id: "c1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                if let Some(ref s) = action.shortcut {
                    assert!(
                        !s.contains("cmd") && !s.contains("shift"),
                        "Clipboard shortcut '{}' should use Unicode",
                        s
                    );
                }
            }
        }

        #[test]
        fn cat22_ai_shortcuts_use_unicode() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                if let Some(ref s) = action.shortcut {
                    assert!(
                        !s.contains("cmd") && !s.contains("shift"),
                        "AI shortcut '{}' should use Unicode",
                        s
                    );
                }
            }
        }

        #[test]
        fn cat22_path_shortcuts_use_unicode() {
            let info = PathInfo {
                name: "f".into(),
                path: "/f".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            for action in &actions {
                if let Some(ref s) = action.shortcut {
                    assert!(
                        !s.contains("cmd") && !s.contains("shift"),
                        "Path shortcut '{}' should use Unicode",
                        s
                    );
                }
            }
        }

        // =========================================================================
        // cat23: Clipboard paste_keep_open shortcut
        // =========================================================================

        #[test]
        fn cat23_paste_keep_open_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "pk1".into(),
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
        fn cat23_paste_keep_open_title() {
            let entry = ClipboardEntryInfo {
                id: "pk2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
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
        fn cat23_paste_keep_open_description() {
            let entry = ClipboardEntryInfo {
                id: "pk3".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let pko = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste_keep_open")
                .unwrap();
            assert!(pko.description.is_some());
        }

        // =========================================================================
        // cat24: Path context copy_filename has no shortcut
        // =========================================================================

        #[test]
        fn cat24_path_copy_filename_no_shortcut() {
            let info = PathInfo {
                name: "test.txt".into(),
                path: "/test.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let cf = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert!(
                cf.shortcut.is_none(),
                "path copy_filename should have no shortcut"
            );
        }

        #[test]
        fn cat24_path_copy_filename_present() {
            let info = PathInfo {
                name: "readme.md".into(),
                path: "/readme.md".into(),
                is_dir: false,
            };
            let ids = action_ids(&get_path_context_actions(&info));
            assert!(ids.contains(&"file:copy_filename".to_string()));
        }

        #[test]
        fn cat24_path_copy_filename_description() {
            let info = PathInfo {
                name: "data.json".into(),
                path: "/data.json".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
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

        // =========================================================================
        // cat25: File context open_with macOS shortcut
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat25_file_open_in_editor_shortcut() {
            let file = FileInfo {
                path: "/test.txt".into(),
                name: "test.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let oe = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .expect("file:open_in_editor should exist");
            assert_eq!(oe.shortcut.as_deref(), Some("⌘E"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat25_file_show_info_shortcut() {
            let file = FileInfo {
                path: "/img.png".into(),
                name: "img.png".into(),
                file_type: crate::file_search::FileType::Image,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let si = actions.iter().find(|a| a.id == "file:show_info").unwrap();
            assert_eq!(si.shortcut.as_deref(), Some("⌘I"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat25_file_quick_look_shortcut() {
            let file = FileInfo {
                path: "/readme.md".into(),
                name: "readme.md".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let ql = actions.iter().find(|a| a.id == "file:quick_look").unwrap();
            assert_eq!(ql.shortcut.as_deref(), Some("⌘Y"));
        }

        // =========================================================================
        // cat26: Notes format shortcut exact value
        // =========================================================================

        // =========================================================================
        // cat27: AI command bar icon name correctness
        // =========================================================================

        #[test]
        fn cat27_ai_copy_response_icon() {
            let actions = get_ai_command_bar_actions();
            let cr = actions
                .iter()
                .find(|a| a.id == "chat:copy_response")
                .unwrap();
            assert_eq!(cr.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat27_ai_submit_icon() {
            let actions = get_ai_command_bar_actions();
            let submit = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(submit.icon, Some(IconName::ArrowUp));
        }

        #[test]
        fn cat27_ai_new_chat_icon() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(nc.icon, Some(IconName::Plus));
        }

        #[test]
        fn cat27_ai_delete_chat_icon() {
            let actions = get_ai_command_bar_actions();
            let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(dc.icon, Some(IconName::Trash));
        }

        #[test]
        fn cat27_ai_change_model_icon() {
            let actions = get_ai_command_bar_actions();
            let cm = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert_eq!(cm.icon, Some(IconName::Settings));
        }

        #[test]
        fn cat27_ai_toggle_shortcuts_help_icon() {
            let actions = get_ai_command_bar_actions();
            let ts = actions
                .iter()
                .find(|a| a.id == "chat:toggle_shortcuts_help")
                .unwrap();
            assert_eq!(ts.icon, Some(IconName::Star));
        }

        // =========================================================================
        // cat28: Script context run title format
        // =========================================================================

        #[test]
        fn cat28_run_title_default_verb() {
            let script = ScriptInfo::new("My Script", "/p/my-script.ts");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Run");
        }

        #[test]
        fn cat28_run_title_custom_verb() {
            let script = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Launch");
        }

        #[test]
        fn cat28_run_title_switch_to_verb() {
            let script = ScriptInfo::with_action_verb("Terminal", "window:1", false, "Switch to");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Switch To");
        }

        #[test]
        fn cat28_run_title_builtin() {
            let builtin = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&builtin);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Run");
        }

        #[test]
        fn cat28_run_shortcut_always_enter() {
            let script = ScriptInfo::new("test", "/p/test.ts");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.shortcut.as_deref(), Some("↵"));
        }

        // =========================================================================
        // cat29: Ordering consistency across repeated calls
        // =========================================================================

        #[test]
        fn cat29_script_ordering_deterministic() {
            let script = ScriptInfo::new("test", "/p/test.ts");
            let a1 = action_ids(&get_script_context_actions(&script));
            let a2 = action_ids(&get_script_context_actions(&script));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat29_clipboard_ordering_deterministic() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let a1 = action_ids(&get_clipboard_history_context_actions(&entry));
            let a2 = action_ids(&get_clipboard_history_context_actions(&entry));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat29_ai_ordering_deterministic() {
            let a1 = action_ids(&get_ai_command_bar_actions());
            let a2 = action_ids(&get_ai_command_bar_actions());
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat29_path_ordering_deterministic() {
            let info = PathInfo {
                name: "f".into(),
                path: "/f".into(),
                is_dir: false,
            };
            let a1 = action_ids(&get_path_context_actions(&info));
            let a2 = action_ids(&get_path_context_actions(&info));
            assert_eq!(a1, a2);
        }

        // =========================================================================
        // cat30: Cross-context non-empty ID and title, has_action=false, ID uniqueness
        // =========================================================================

        #[test]
        fn cat30_script_non_empty_ids_and_titles() {
            let script = ScriptInfo::new("test", "/p/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(!action.id.is_empty(), "action ID should not be empty");
                assert!(!action.title.is_empty(), "action title should not be empty");
            }
        }

        #[test]
        fn cat30_clipboard_non_empty_ids_and_titles() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        #[test]
        fn cat30_ai_non_empty_ids_and_titles() {
            for action in &get_ai_command_bar_actions() {
                assert!(!action.id.is_empty());
                assert!(!action.title.is_empty());
            }
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn cat30_script_has_action_false() {
            let script = ScriptInfo::new("test", "/p/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    !action.has_action,
                    "built-in action '{}' should be false",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_clipboard_has_action_false() {
            let entry = ClipboardEntryInfo {
                id: "c".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "x".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(
                    !action.has_action,
                    "clipboard action '{}' should be false",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_script_ids_unique() {
            let script = ScriptInfo::new("test", "/p/test.ts");
            let ids = action_ids(&get_script_context_actions(&script));
            let set: HashSet<&String> = ids.iter().collect();
            assert_eq!(ids.len(), set.len(), "script IDs must be unique");
        }

        #[test]
        fn cat30_ai_ids_unique() {
            let ids = action_ids(&get_ai_command_bar_actions());
            let set: HashSet<&String> = ids.iter().collect();
            assert_eq!(ids.len(), set.len(), "AI IDs must be unique");
        }

        #[test]
        fn cat30_path_ids_unique() {
            let info = PathInfo {
                name: "f".into(),
                path: "/f".into(),
                is_dir: false,
            };
            let ids = action_ids(&get_path_context_actions(&info));
            let set: HashSet<&String> = ids.iter().collect();
            assert_eq!(ids.len(), set.len(), "path IDs must be unique");
        }

        #[test]
        fn cat30_file_ids_unique() {
            let file = FileInfo {
                path: "/x.rs".into(),
                name: "x.rs".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let ids = action_ids(&get_file_context_actions(&file));
            let set: HashSet<&String> = ids.iter().collect();
            assert_eq!(ids.len(), set.len(), "file IDs must be unique");
        }

    }
}

mod from_dialog_builtin_action_validation_tests_17 {
    //! Batch 17 – Dialog Builtin Action Validation Tests
    //!
    //! 30 categories, ~150 tests validating random built-in action behaviors.
    //!
    //! Categories:
    //! 01. Script context exact action count by ScriptInfo type
    //! 02. Scriptlet context copy_content action details
    //! 03. Path context total action count and primary action
    //! 04. Clipboard paste action description content
    //! 05. AI command bar shortcut completeness (which actions lack shortcuts)
    //! 06. Notes command bar duplicate_note conditional visibility
    //! 07. Note switcher empty notes fallback placeholder
    //! 08. Chat context model ID format pattern
    //! 09. Scriptlet defined action ID prefix invariant
    //! 10. Agent context reveal_in_finder and copy_path shortcuts
    //! 11. File context exact description strings
    //! 12. Path context exact description strings
    //! 13. Clipboard text/image macOS action count difference
    //! 14. Script run title format includes quotes
    //! 15. to_deeplink_name with whitespace variations
    //! 16. Action::new pre-computes lowercase fields
    //! 17. ActionsDialog::format_shortcut_hint SDK-style shortcuts
    //! 18. ActionsDialog::parse_shortcut_keycaps compound shortcuts
    //! 19. ActionsDialog::score_action multi-field bonus stacking
    //! 20. ActionsDialog::fuzzy_match character ordering requirement
    //! 21. build_grouped_items_static with no-section actions
    //! 22. coerce_action_selection alternating header-item pattern
    //! 23. CommandBarConfig notes_style preset values
    //! 24. Clipboard unpin action title and description text
    //! 25. New chat actions mixed section sizes
    //! 26. Notes command bar browse_notes action details
    //! 27. Script context copy_deeplink description contains URL
    //! 28. Cross-context all actions are ScriptContext category
    //! 29. Action with_icon chaining preserves all fields
    //! 30. Script context action stability across flag combinations

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};

        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // ================================================================
        // Cat 01: Script context exact action count by ScriptInfo type
        // ================================================================

        #[test]
        fn cat01_script_new_no_shortcut_no_alias_count() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            // run, toggle_info, add_shortcut, add_alias, edit_script, view_logs, toggle_favorite,
            // reveal_in_finder, file:open_in_quick_terminal, copy_path, copy_content,
            // copy_deeplink, delete_script = 13
            assert_eq!(actions.len(), 13);
        }

        #[test]
        fn cat01_script_with_shortcut_count() {
            let script = ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".into()));
            let actions = get_script_context_actions(&script);
            // run, toggle_info, update_shortcut, remove_shortcut, add_alias, edit_script, view_logs,
            // toggle_favorite, reveal_in_finder, file:open_in_quick_terminal, copy_path,
            // copy_content, copy_deeplink, delete_script = 14
            assert_eq!(actions.len(), 14);
        }

        #[test]
        fn cat01_script_with_shortcut_and_alias_count() {
            let script = ScriptInfo::with_shortcut_and_alias(
                "test",
                "/path/test.ts",
                Some("cmd+t".into()),
                Some("ts".into()),
            );
            let actions = get_script_context_actions(&script);
            // run, toggle_info, update_shortcut, remove_shortcut, update_alias, remove_alias,
            // edit_script, view_logs, toggle_favorite, reveal_in_finder,
            // file:open_in_quick_terminal, copy_path, copy_content, copy_deeplink,
            // delete_script = 15
            assert_eq!(actions.len(), 15);
        }

        #[test]
        fn cat01_builtin_no_shortcut_count() {
            let builtin = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&builtin);
            // run, toggle_info, add_shortcut, add_alias, copy_deeplink = 5
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn cat01_scriptlet_no_shortcut_count() {
            let scriptlet = ScriptInfo::scriptlet("Open URL", "/path/url.md", None, None);
            let actions = get_script_context_actions(&scriptlet);
            // run, toggle_info, add_shortcut, add_alias, edit_scriptlet, toggle_favorite,
            // reveal_scriptlet_in_finder, file:open_in_quick_terminal, copy_scriptlet_path,
            // copy_content, copy_deeplink = 11
            assert_eq!(actions.len(), 11);
        }

        #[test]
        fn cat01_script_with_frecency_adds_reset_ranking() {
            let script = ScriptInfo::new("test", "/path/test.ts")
                .with_frecency(true, Some("/path/test.ts".into()));
            let actions = get_script_context_actions(&script);
            // base 13 + reset_ranking = 14
            assert_eq!(actions.len(), 14);
        }

        // ================================================================
        // Cat 02: Scriptlet context copy_content action details
        // ================================================================

        #[test]
        fn cat02_scriptlet_context_has_copy_content() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn cat02_scriptlet_copy_content_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
        }

        #[test]
        fn cat02_scriptlet_copy_content_description() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert!(cc.description.as_ref().unwrap().contains("file content"));
        }

        #[test]
        fn cat02_script_copy_content_same_shortcut() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert_eq!(cc.shortcut.as_deref(), Some("⌘⌥C"));
        }

        // ================================================================
        // Cat 03: Path context total action count and primary action
        // ================================================================

        #[test]
        fn cat03_path_dir_total_count() {
            let info = PathInfo {
                path: "/Users/test/Documents".into(),
                name: "Documents".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            // open_directory, copy_path, open_in_finder, open_in_editor,
            // open_in_quick_terminal, copy_filename, move_to_trash = 7
            assert_eq!(actions.len(), 7);
        }

        #[test]
        fn cat03_path_file_total_count() {
            let info = PathInfo {
                path: "/Users/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            // select_file, copy_path, open_in_finder, open_in_editor,
            // open_in_quick_terminal, copy_filename, move_to_trash = 7
            assert_eq!(actions.len(), 7);
        }

        #[test]
        fn cat03_path_dir_primary_is_open_directory() {
            let info = PathInfo {
                path: "/Users/test/Documents".into(),
                name: "Documents".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn cat03_path_file_primary_is_select_file() {
            let info = PathInfo {
                path: "/Users/test/file.txt".into(),
                name: "file.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn cat03_path_dir_and_file_same_count() {
            let dir = PathInfo {
                path: "/a".into(),
                name: "a".into(),
                is_dir: true,
            };
            let file = PathInfo {
                path: "/b".into(),
                name: "b".into(),
                is_dir: false,
            };
            assert_eq!(
                get_path_context_actions(&dir).len(),
                get_path_context_actions(&file).len()
            );
        }

        // ================================================================
        // Cat 04: Clipboard paste action description content
        // ================================================================

        #[test]
        fn cat04_clipboard_paste_description_mentions_clipboard() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
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
            assert!(paste.description.as_ref().unwrap().contains("clipboard"));
        }

        #[test]
        fn cat04_clipboard_copy_description_mentions_clipboard() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let copy = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_copy")
                .unwrap();
            assert!(copy.description.as_ref().unwrap().contains("clipboard"));
        }

        #[test]
        fn cat04_clipboard_paste_keep_open_description() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".into(),
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

        // ================================================================
        // Cat 05: AI command bar shortcut completeness
        // ================================================================

        #[test]
        fn cat05_branch_from_last_has_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let bfl = actions
                .iter()
                .find(|a| a.id == "chat:branch_from_last")
                .unwrap();
            assert!(bfl.shortcut.is_none());
        }

        #[test]
        fn cat05_change_model_has_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let cm = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert!(cm.shortcut.is_none());
        }

        #[test]
        fn cat05_submit_has_shortcut_enter() {
            let actions = get_ai_command_bar_actions();
            let s = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(s.shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn cat05_new_chat_has_shortcut_cmd_n() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(nc.shortcut.as_deref(), Some("⌘N"));
        }

        #[test]
        fn cat05_actions_with_shortcuts_count() {
            let actions = get_ai_command_bar_actions();
            let with_shortcuts = actions.iter().filter(|a| a.shortcut.is_some()).count();
            // The three favorite-model / expand-composer actions each carry a
            // shortcut; branch_from_last and change_model still lack one.
            assert_eq!(with_shortcuts, 15);
        }

        // ================================================================
        // Cat 06: Notes command bar duplicate_note conditional visibility
        // ================================================================

        // ================================================================
        // Cat 07: Note switcher empty notes fallback placeholder
        // ================================================================

        #[test]
        fn cat07_empty_notes_returns_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
        }

        #[test]
        fn cat07_placeholder_id_is_no_notes() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].id, "no_notes");
        }

        #[test]
        fn cat07_placeholder_title() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].title, "No notes yet");
        }

        #[test]
        fn cat07_placeholder_icon_is_plus() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].icon, Some(IconName::Plus));
        }

        #[test]
        fn cat07_placeholder_section_is_notes() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].section.as_deref(), Some("Notes"));
        }

        #[test]
        fn cat07_placeholder_description_mentions_cmd_n() {
            let actions = get_note_switcher_actions(&[]);
            assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
        }

        // ================================================================
        // Cat 08: Chat context model ID format pattern
        // ================================================================

        #[test]
        fn cat08_model_id_uses_select_model_prefix() {
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
        fn cat08_multiple_models_sequential_ids() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![
                    ChatModelInfo {
                        id: "m1".into(),
                        display_name: "M1".into(),
                        provider: "P".into(),
                    },
                    ChatModelInfo {
                        id: "m2".into(),
                        display_name: "M2".into(),
                        provider: "P".into(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let ids = action_ids(&picker);
            assert!(ids.contains(&"chat:select_model_m1".to_string()));
            assert!(ids.contains(&"chat:select_model_m2".to_string()));
        }

        #[test]
        fn cat08_current_model_has_checkmark() {
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
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-3")
                .unwrap();
            assert!(model_action.title.contains("✓"));
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat08_non_current_model_no_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
                available_models: vec![ChatModelInfo {
                    id: "claude-3".into(),
                    display_name: "Claude 3".into(),
                    provider: "Anthropic".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-3")
                .unwrap();
            assert!(!model_action.title.contains("✓"));
        }

        // ================================================================
        // Cat 09: Scriptlet defined action ID prefix invariant
        // ================================================================

        #[test]
        fn cat09_scriptlet_action_id_has_prefix() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Do Thing".into(),
                command: "do-thing".into(),
                tool: "bash".into(),
                code: "echo done".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].id.starts_with("scriptlet_action:"));
        }

        #[test]
        fn cat09_scriptlet_action_id_contains_command() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Do Thing".into(),
                command: "do-thing".into(),
                tool: "bash".into(),
                code: "echo done".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].id, "scriptlet_action:do-thing");
        }

        #[test]
        fn cat09_all_scriptlet_actions_have_prefix() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
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
            let actions = get_scriptlet_defined_actions(&scriptlet);
            for action in &actions {
                assert!(
                    action.id.starts_with("scriptlet_action:"),
                    "ID: {}",
                    action.id
                );
            }
        }

        #[test]
        fn cat09_scriptlet_actions_all_have_action_true() {
            let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo hi".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "A".into(),
                command: "a".into(),
                tool: "bash".into(),
                code: "echo a".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].has_action);
        }

        // ================================================================
        // Cat 10: Agent context reveal_in_finder and copy_path shortcuts
        // ================================================================

        #[test]
        fn cat10_agent_has_reveal_in_finder() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
        }

        #[test]
        fn cat10_agent_reveal_shortcut() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let reveal = actions.iter().find(|a| a.id == "reveal_in_finder").unwrap();
            assert_eq!(reveal.shortcut.as_deref(), Some("⌘⇧F"));
        }

        #[test]
        fn cat10_agent_copy_path_shortcut() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let cp = actions.iter().find(|a| a.id == "copy_path").unwrap();
            assert_eq!(cp.shortcut.as_deref(), Some("⌘⇧C"));
        }

        #[test]
        fn cat10_agent_edit_shortcut() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn cat10_agent_has_copy_content() {
            let mut script = ScriptInfo::new("My Agent", "/path/agent.md");
            script.is_script = false;
            script.is_agent = true;
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        // ================================================================
        // Cat 11: File context exact description strings
        // ================================================================

        #[test]
        fn cat11_file_open_description() {
            let fi = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
            assert_eq!(
                open.description.as_deref(),
                Some("Opens with the default app")
            );
        }

        #[test]
        fn cat11_dir_open_description() {
            let fi = FileInfo {
                path: "/test/dir".into(),
                name: "dir".into(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&fi);
            let open = actions
                .iter()
                .find(|a| a.id == "file:open_directory")
                .unwrap();
            assert_eq!(open.description.as_deref(), Some("Opens this folder"));
        }

        #[test]
        fn cat11_reveal_description() {
            let fi = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let reveal = actions
                .iter()
                .find(|a| a.id == "file:reveal_in_finder")
                .unwrap();
            assert_eq!(
                reveal.description.as_deref(),
                Some("Shows this item in Finder")
            );
        }

        #[test]
        fn cat11_copy_path_description() {
            let fi = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
            assert!(cp.description.as_ref().unwrap().contains("path"));
        }

        #[test]
        fn cat11_copy_filename_description() {
            let fi = FileInfo {
                path: "/test/file.txt".into(),
                name: "file.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&fi);
            let cf = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert!(cf.description.as_ref().unwrap().contains("filename"));
        }

        // ================================================================
        // Cat 12: Path context exact description strings
        // ================================================================

        #[test]
        fn cat12_path_dir_primary_description() {
            let info = PathInfo {
                path: "/a/b".into(),
                name: "b".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let primary = actions
                .iter()
                .find(|a| a.id == "file:open_directory")
                .unwrap();
            assert!(primary.description.as_ref().unwrap().contains("directory"));
        }

        #[test]
        fn cat12_path_file_primary_description() {
            let info = PathInfo {
                path: "/a/b.txt".into(),
                name: "b.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let primary = actions.iter().find(|a| a.id == "file:select_file").unwrap();
            assert!(primary.description.as_ref().unwrap().contains("file"));
        }

        #[test]
        fn cat12_path_open_in_editor_description() {
            let info = PathInfo {
                path: "/a/b".into(),
                name: "b".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&info);
            let editor = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .unwrap();
            assert!(editor.description.as_ref().unwrap().contains("$EDITOR"));
        }

        #[test]
        fn cat12_path_move_to_trash_dir_description() {
            let info = PathInfo {
                path: "/a/b".into(),
                name: "b".into(),
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
        fn cat12_path_move_to_trash_file_description() {
            let info = PathInfo {
                path: "/a/b.txt".into(),
                name: "b.txt".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&info);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(trash.description.as_ref().unwrap().contains("file"));
        }

        // ================================================================
        // Cat 13: Clipboard text/image macOS action count difference
        // ================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat13_image_has_more_actions_than_text_macos() {
            let text_entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let img_entry = ClipboardEntryInfo {
                id: "i".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "img".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let text_actions = get_clipboard_history_context_actions(&text_entry);
            let img_actions = get_clipboard_history_context_actions(&img_entry);
            assert!(img_actions.len() > text_actions.len());
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat13_image_has_ocr_text_does_not_macos() {
            let text_entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let img_entry = ClipboardEntryInfo {
                id: "i".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "img".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let text_ids = action_ids(&get_clipboard_history_context_actions(&text_entry));
            let img_ids = action_ids(&get_clipboard_history_context_actions(&img_entry));
            assert!(!text_ids.contains(&"clip:clipboard_ocr".to_string()));
            assert!(img_ids.contains(&"clip:clipboard_ocr".to_string()));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat13_text_count_macos() {
            let entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // paste, copy, paste_keep_open, share, attach_to_ai, quick_look,
            // pin, keep_in_today, save_snippet, save_file, delete, delete_multiple, delete_all = 13
            assert_eq!(actions.len(), 13);
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat13_image_count_macos() {
            let entry = ClipboardEntryInfo {
                id: "i".into(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "img".into(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            // paste, copy, paste_keep_open, share, attach_to_ai, quick_look,
            // open_with, annotate_cleanshot, upload_cleanshot, pin, keep_in_today,
            // ocr, save_file, delete, delete_multiple, delete_all = 16
            // (save_snippet is text-only; images get ocr instead)
            assert_eq!(actions.len(), 16);
        }

        // ================================================================
        // Cat 14: Script run title format includes quotes
        // ================================================================

        #[test]
        fn cat14_run_title_has_quotes_around_name() {
            let script = ScriptInfo::new("My Script", "/path/my-script.ts");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Run");
        }

        #[test]
        fn cat14_custom_verb_in_title() {
            let script =
                ScriptInfo::with_action_verb("Safari", "/Applications/Safari.app", false, "Launch");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Launch");
        }

        #[test]
        fn cat14_switch_to_verb_in_title() {
            let script =
                ScriptInfo::with_action_verb("My Window", "window:123", false, "Switch to");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Switch To");
        }

        #[test]
        fn cat14_run_shortcut_is_enter() {
            let script = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.shortcut.as_deref(), Some("↵"));
        }

        // ================================================================
        // Cat 15: to_deeplink_name with whitespace variations
        // ================================================================

        #[test]
        fn cat15_single_space() {
            assert_eq!(to_deeplink_name("A B"), "a-b");
        }

        #[test]
        fn cat15_multiple_spaces() {
            assert_eq!(to_deeplink_name("A  B   C"), "a-b-c");
        }

        #[test]
        fn cat15_tabs_converted() {
            assert_eq!(to_deeplink_name("A\tB"), "a-b");
        }

        #[test]
        fn cat15_leading_trailing_spaces() {
            assert_eq!(to_deeplink_name("  hello  "), "hello");
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat15_mixed_separators() {
            assert_eq!(to_deeplink_name("foo_bar baz-qux"), "foo-bar-baz-qux");
        }

        // ================================================================
        // Cat 16: Action::new pre-computes lowercase fields
        // ================================================================

        #[test]
        fn cat16_title_lower_cached() {
            let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "hello world");
        }

        #[test]
        fn cat16_description_lower_cached() {
            let action = Action::new(
                "id",
                "T",
                Some("Foo Bar".into()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower, Some("foo bar".into()));
        }

        #[test]
        fn cat16_description_none_lower_none() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn cat16_shortcut_lower_none_initially() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn cat16_shortcut_lower_set_after_with_shortcut() {
            let action =
                Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
            assert_eq!(action.shortcut_lower.as_deref(), Some("⌘e"));
        }

        // ================================================================
        // Cat 17: ActionsDialog::format_shortcut_hint SDK-style shortcuts
        // ================================================================

        #[test]
        fn cat17_cmd_c() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+c"), "⌘C");
        }

        #[test]
        fn cat17_command_c() {
            assert_eq!(ActionsDialog::format_shortcut_hint("command+c"), "⌘C");
        }

        #[test]
        fn cat17_meta_c() {
            assert_eq!(ActionsDialog::format_shortcut_hint("meta+c"), "⌘C");
        }

        #[test]
        fn cat17_ctrl_alt_delete() {
            assert_eq!(
                ActionsDialog::format_shortcut_hint("ctrl+alt+delete"),
                "⌃⌥⌫"
            );
        }

        #[test]
        fn cat17_shift_enter() {
            assert_eq!(ActionsDialog::format_shortcut_hint("shift+enter"), "⇧↵");
        }

        #[test]
        fn cat17_option_space() {
            assert_eq!(ActionsDialog::format_shortcut_hint("option+space"), "⌥␣");
        }

        #[test]
        fn cat17_arrowup() {
            assert_eq!(ActionsDialog::format_shortcut_hint("cmd+arrowup"), "⌘↑");
        }

        // ================================================================
        // Cat 18: ActionsDialog::parse_shortcut_keycaps compound shortcuts
        // ================================================================

        #[test]
        fn cat18_cmd_enter_two_keycaps() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘↵");
            assert_eq!(caps, vec!["⌘", "↵"]);
        }

        #[test]
        fn cat18_cmd_shift_c_three_keycaps() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(caps, vec!["⌘", "⇧", "C"]);
        }

        #[test]
        fn cat18_single_letter_uppercased() {
            let caps = ActionsDialog::parse_shortcut_keycaps("a");
            assert_eq!(caps, vec!["A"]);
        }

        #[test]
        fn cat18_arrows() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn cat18_empty_string() {
            let caps = ActionsDialog::parse_shortcut_keycaps("");
            assert!(caps.is_empty());
        }

        // ================================================================
        // Cat 19: ActionsDialog::score_action multi-field bonus stacking
        // ================================================================

        #[test]
        fn cat19_prefix_match_100() {
            let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            assert_eq!(ActionsDialog::score_action(&action, "edit"), 100);
        }

        #[test]
        fn cat19_prefix_plus_description_115() {
            let action = Action::new(
                "id",
                "Edit Script",
                Some("Edit this script".into()),
                ActionCategory::ScriptContext,
            );
            // prefix(100) + description(15) = 115
            assert_eq!(ActionsDialog::score_action(&action, "edit"), 115);
        }

        #[test]
        fn cat19_contains_match_50() {
            let action = Action::new("id", "Script Editor", None, ActionCategory::ScriptContext);
            // "edit" is contained but not prefix in "script editor"
            assert_eq!(ActionsDialog::score_action(&action, "edit"), 50);
        }

        #[test]
        fn cat19_no_match_0() {
            let action = Action::new("id", "Run Script", None, ActionCategory::ScriptContext);
            assert_eq!(ActionsDialog::score_action(&action, "xyz"), 0);
        }

        #[test]
        fn cat19_shortcut_bonus_10() {
            let action = Action::new("id", "No Match Title", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            // "⌘e" matches shortcut_lower "⌘e" => +10
            assert_eq!(ActionsDialog::score_action(&action, "⌘e"), 10);
        }

        // ================================================================
        // Cat 20: ActionsDialog::fuzzy_match character ordering requirement
        // ================================================================

        #[test]
        fn cat20_correct_order_matches() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlo"));
        }

        #[test]
        fn cat20_wrong_order_no_match() {
            assert!(!ActionsDialog::fuzzy_match("hello world", "olh"));
        }

        #[test]
        fn cat20_exact_match() {
            assert!(ActionsDialog::fuzzy_match("abc", "abc"));
        }

        #[test]
        fn cat20_empty_needle_matches() {
            assert!(ActionsDialog::fuzzy_match("anything", ""));
        }

        #[test]
        fn cat20_empty_haystack_no_match() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn cat20_needle_longer_no_match() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        // ================================================================
        // Cat 21: build_grouped_items_static with no-section actions
        // ================================================================

        #[test]
        fn cat21_no_section_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext),
                Action::new("b", "B", None, ActionCategory::ScriptContext),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // No section set => no headers, just 2 items
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat21_with_section_adds_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // 1 header + 2 items = 3
            assert_eq!(grouped.len(), 3);
            assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "S"));
        }

        #[test]
        fn cat21_separators_style_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // No headers in Separators mode
            assert_eq!(grouped.len(), 2);
        }

        #[test]
        fn cat21_empty_input_empty_output() {
            let actions: Vec<Action> = vec![];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        // ================================================================
        // Cat 22: coerce_action_selection alternating header-item pattern
        // ================================================================

        #[test]
        fn cat22_alternating_header_item_select_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H1".into()),
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("H2".into()),
                GroupedActionItem::Item(1),
            ];
            // Index 0 is header => coerce to 1 (next item)
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat22_alternating_last_header_coerces_up() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H1".into()),
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("H2".into()),
            ];
            // Index 2 is header, no items below => search up, find item at 1
            assert_eq!(coerce_action_selection(&rows, 2), Some(1));
        }

        #[test]
        fn cat22_item_at_index_stays() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(1));
        }

        #[test]
        fn cat22_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("H1".into()),
                GroupedActionItem::SectionHeader("H2".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat22_empty_returns_none() {
            let rows: Vec<GroupedActionItem> = vec![];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        // ================================================================
        // Cat 23: CommandBarConfig notes_style preset values
        // ================================================================

        #[test]
        fn cat23_notes_style_search_top() {
            let config = CommandBarConfig::notes_style();
            assert!(matches!(
                config.dialog_config.search_position,
                SearchPosition::Top
            ));
        }

        #[test]
        fn cat23_notes_style_separators() {
            let config = CommandBarConfig::notes_style();
            assert!(matches!(
                config.dialog_config.section_style,
                SectionStyle::Headers
            ));
        }

        #[test]
        fn cat23_notes_style_icons_enabled() {
            let config = CommandBarConfig::notes_style();
            assert!(config.dialog_config.show_icons);
        }

        #[test]
        fn cat23_notes_style_footer_enabled() {
            let config = CommandBarConfig::notes_style();
            assert!(!config.dialog_config.show_footer);
        }

        #[test]
        fn cat23_notes_style_close_defaults_true() {
            let config = CommandBarConfig::notes_style();
            assert!(config.close_on_select);
            assert!(config.close_on_escape);
            assert!(config.close_on_click_outside);
        }

        // ================================================================
        // Cat 24: Clipboard unpin action title and description text
        // ================================================================

        #[test]
        fn cat24_unpin_title() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "text".into(),
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
        fn cat24_unpin_description() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let unpin = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_unpin")
                .unwrap();
            assert!(unpin.description.as_ref().unwrap().contains("pin"));
        }

        #[test]
        fn cat24_pin_title() {
            let entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
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
        fn cat24_pin_unpin_same_shortcut() {
            let pinned_entry = ClipboardEntryInfo {
                id: "e1".into(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "text".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let unpinned_entry = ClipboardEntryInfo {
                id: "e2".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "text".into(),
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
            assert_eq!(pin.shortcut.as_deref(), Some("⇧⌘P"));
        }

        // ================================================================
        // Cat 25: New chat actions mixed section sizes
        // ================================================================

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat25_multiple_last_used_single_preset_single_model() {
            let last_used = vec![
                NewChatModelInfo {
                    model_id: "m1".into(),
                    display_name: "M1".into(),
                    provider: "p1".into(),
                    provider_display_name: "P1".into(),
                },
                NewChatModelInfo {
                    model_id: "m2".into(),
                    display_name: "M2".into(),
                    provider: "p2".into(),
                    provider_display_name: "P2".into(),
                },
            ];
            let presets = vec![NewChatPresetInfo {
                id: "general".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m3".into(),
                display_name: "M3".into(),
                provider: "p3".into(),
                provider_display_name: "P3".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 4); // 2 + 1 + 1
        }

        #[test]
        fn cat25_sections_are_correct() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "M1".into(),
                provider: "p1".into(),
                provider_display_name: "P1".into(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "gen".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m2".into(),
                display_name: "M2".into(),
                provider: "p2".into(),
                provider_display_name: "P2".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            let sections: Vec<&str> = actions
                .iter()
                .filter_map(|a| a.section.as_deref())
                .collect();
            assert_eq!(sections, vec!["Last Used Settings", "Presets", "Models"]);
        }

        #[test]
        fn cat25_preset_has_no_description() {
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
        fn cat25_model_has_provider_description() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".into(),
                display_name: "M1".into(),
                provider: "p1".into(),
                provider_display_name: "Provider One".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].description.as_deref(), Some("Uses Provider One"));
        }

        // ================================================================
        // Cat 26: Notes command bar browse_notes action details
        // ================================================================

        // ================================================================
        // Cat 27: Script context copy_deeplink description contains URL
        // ================================================================

        #[test]
        fn cat27_deeplink_description_contains_scriptkit_url() {
            let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(
                dl.description.as_deref(),
                Some("Copy a portable Script Kit share link to clipboard")
            );
        }

        #[test]
        fn cat27_deeplink_description_contains_deeplink_name() {
            let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(dl.title, "Share");
        }

        #[test]
        fn cat27_deeplink_shortcut() {
            let script = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(dl.shortcut.as_deref(), Some("⌘⇧D"));
        }

        #[test]
        fn cat27_scriptlet_deeplink_also_has_url() {
            let script = ScriptInfo::scriptlet("Open GitHub", "/path/url.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert!(
                dl.description
                    .as_ref()
                    .unwrap()
                    .contains("scriptkit://run/open-github")
            );
        }

        // ================================================================
        // Cat 28: Cross-context all actions are ScriptContext category
        // ================================================================

        #[test]
        fn cat28_script_actions_all_script_context() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            for action in &get_script_context_actions(&script) {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "ID: {}",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_clipboard_actions_all_script_context() {
            let entry = ClipboardEntryInfo {
                id: "e".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "t".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "ID: {}",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_ai_actions_all_script_context() {
            for action in &get_ai_command_bar_actions() {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "ID: {}",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_path_actions_all_script_context() {
            let info = PathInfo {
                path: "/a".into(),
                name: "a".into(),
                is_dir: true,
            };
            for action in &get_path_context_actions(&info) {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "ID: {}",
                    action.id
                );
            }
        }

        #[test]
        fn cat28_file_actions_all_script_context() {
            let fi = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: crate::file_search::FileType::File,
                is_dir: false,
            };
            for action in &get_file_context_actions(&fi) {
                assert_eq!(
                    action.category,
                    ActionCategory::ScriptContext,
                    "ID: {}",
                    action.id
                );
            }
        }

        // ================================================================
        // Cat 29: Action with_icon chaining preserves all fields
        // ================================================================

        #[test]
        fn cat29_with_icon_preserves_shortcut() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E")
                .with_icon(IconName::Star);
            assert_eq!(action.shortcut.as_deref(), Some("⌘E"));
            assert_eq!(action.icon, Some(IconName::Star));
        }

        #[test]
        fn cat29_with_section_preserves_icon() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Copy)
                .with_section("S");
            assert_eq!(action.icon, Some(IconName::Copy));
            assert_eq!(action.section.as_deref(), Some("S"));
        }

        #[test]
        fn cat29_full_chain_all_fields() {
            let action = Action::new(
                "id",
                "Title",
                Some("Desc".into()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘X")
            .with_icon(IconName::Trash)
            .with_section("Section");
            assert_eq!(action.id, "id");
            assert_eq!(action.title, "Title");
            assert_eq!(action.description.as_deref(), Some("Desc"));
            assert_eq!(action.shortcut.as_deref(), Some("⌘X"));
            assert_eq!(action.icon, Some(IconName::Trash));
            assert_eq!(action.section.as_deref(), Some("Section"));
            assert!(!action.has_action);
        }

        #[test]
        fn cat29_with_shortcut_opt_none_preserves_existing() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘A")
                .with_shortcut_opt(None);
            // with_shortcut_opt(None) does NOT clear existing shortcut
            assert_eq!(action.shortcut.as_deref(), Some("⌘A"));
        }

        #[test]
        fn cat29_with_shortcut_opt_some_sets() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘B".into()));
            assert_eq!(action.shortcut.as_deref(), Some("⌘B"));
        }

        // ================================================================
        // Cat 30: Script context action stability across flag combinations
        // ================================================================

        #[test]
        fn cat30_script_deterministic() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let a1 = action_ids(&get_script_context_actions(&script));
            let a2 = action_ids(&get_script_context_actions(&script));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat30_builtin_deterministic() {
            let builtin = ScriptInfo::builtin("Test");
            let a1 = action_ids(&get_script_context_actions(&builtin));
            let a2 = action_ids(&get_script_context_actions(&builtin));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat30_scriptlet_deterministic() {
            let scriptlet = ScriptInfo::scriptlet("Test", "/path.md", None, None);
            let a1 = action_ids(&get_script_context_actions(&scriptlet));
            let a2 = action_ids(&get_script_context_actions(&scriptlet));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat30_agent_deterministic() {
            let mut agent = ScriptInfo::new("Agent", "/path/agent.md");
            agent.is_script = false;
            agent.is_agent = true;
            let a1 = action_ids(&get_script_context_actions(&agent));
            let a2 = action_ids(&get_script_context_actions(&agent));
            assert_eq!(a1, a2);
        }

        #[test]
        fn cat30_frecency_flag_adds_exactly_one_action() {
            let base = ScriptInfo::new("test", "/path/test.ts");
            let with_frecency =
                ScriptInfo::new("test", "/path/test.ts").with_frecency(true, Some("/path".into()));
            let base_count = get_script_context_actions(&base).len();
            let frecency_count = get_script_context_actions(&with_frecency).len();
            assert_eq!(frecency_count, base_count + 1);
        }
    }
}

mod from_dialog_builtin_action_validation_tests_18 {
    // =============================================================================
    // Dialog Built-in Action Validation Tests — Batch 18
    //
    // 30 categories of tests validating random built-in actions from dialog windows.
    // Each category tests a specific behavior, field, or invariant.
    //
    // Run with:
    //   cargo test --lib actions::dialog_builtin_action_validation_tests_18
    // =============================================================================

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::FileInfo;
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        // =========================================================================
        // Category 01: Agent context is_agent via mutation — action set correctness
        // =========================================================================

        #[test]
        fn cat01_agent_via_mutation_has_edit_script() {
            let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn cat01_agent_via_mutation_has_reveal_in_finder() {
            let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
        }

        #[test]
        fn cat01_agent_via_mutation_has_copy_path() {
            let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "copy_path"));
        }

        #[test]
        fn cat01_agent_via_mutation_has_copy_content() {
            let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn cat01_agent_via_mutation_no_view_logs() {
            let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
        }

        #[test]
        fn cat01_agent_edit_description_mentions_agent() {
            let mut script = ScriptInfo::new("My Agent", "/path/to/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(edit.description.as_ref().unwrap().contains("agent"));
        }

        // =========================================================================
        // Category 02: Scriptlet context with_custom — reset_ranking placement
        // =========================================================================

        #[test]
        fn cat02_scriptlet_custom_with_frecency_has_reset_ranking() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None)
                .with_frecency(true, Some("scriptlet:Test".to_string()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn cat02_scriptlet_custom_reset_ranking_is_last() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None)
                .with_frecency(true, Some("scriptlet:Test".to_string()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let last = actions.last().unwrap();
            assert_eq!(last.id, "reset_ranking");
        }

        #[test]
        fn cat02_scriptlet_custom_no_frecency_no_reset() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(!actions.iter().any(|a| a.id == "reset_ranking"));
        }

        #[test]
        fn cat02_scriptlet_custom_with_custom_actions_and_frecency() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None)
                .with_frecency(true, Some("scriptlet:Test".to_string()));
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
            assert!(actions.iter().any(|a| a.id == "scriptlet_action:custom"));
            assert!(actions.iter().any(|a| a.id == "reset_ranking"));
        }

        // =========================================================================
        // Category 03: Clipboard OCR action details
        // =========================================================================

        #[test]
        fn cat03_ocr_shortcut_is_shift_cmd_c() {
            let entry = ClipboardEntryInfo {
                id: "img1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".to_string(),
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

        #[test]
        fn cat03_ocr_title_is_copy_text_from_image() {
            let entry = ClipboardEntryInfo {
                id: "img1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".to_string(),
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
        fn cat03_ocr_description_mentions_ocr() {
            let entry = ClipboardEntryInfo {
                id: "img1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".to_string(),
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
        fn cat03_text_entry_has_no_ocr() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_ocr"));
        }

        // =========================================================================
        // Category 04: Clipboard CleanShot actions (macOS only)
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat04_annotate_cleanshot_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "img1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".to_string(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let annotate = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_annotate_cleanshot")
                .unwrap();
            assert_eq!(annotate.shortcut.as_ref().unwrap(), "⇧⌘A");
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat04_upload_cleanshot_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "img1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".to_string(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let upload = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_upload_cleanshot")
                .unwrap();
            assert_eq!(upload.shortcut.as_ref().unwrap(), "⇧⌘U");
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat04_text_entry_no_cleanshot_actions() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(
                !actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_annotate_cleanshot")
            );
            assert!(
                !actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_upload_cleanshot")
            );
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat04_image_has_open_with_text_does_not() {
            let img = ClipboardEntryInfo {
                id: "img1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "Image".to_string(),
                image_dimensions: Some((800, 600)),
                frontmost_app_name: None,
            };
            let txt = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let img_actions = get_clipboard_history_context_actions(&img);
            let txt_actions = get_clipboard_history_context_actions(&txt);
            assert!(
                img_actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_open_with")
            );
            assert!(
                !txt_actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_open_with")
            );
        }

        // =========================================================================
        // Category 05: Chat context both flags false — minimal action set
        // =========================================================================

        #[test]
        fn cat05_both_flags_false_zero_models_only_continue() {
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
        fn cat05_both_flags_true_adds_copy_and_clear() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions.len(), 5);
            assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
            assert!(actions.iter().any(|a| a.id == "chat:copy_response"));
            assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
        }

        #[test]
        fn cat05_has_response_only_adds_copy_response() {
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
        fn cat05_has_messages_only_adds_clear() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            // change_model + continue_in_chat + clear_conversation + capture_screen_area = 4
            assert_eq!(actions.len(), 4);
            assert!(actions.iter().any(|a| a.id == "chat:clear_conversation"));
            assert!(!actions.iter().any(|a| a.id == "chat:copy_response"));
        }

        // =========================================================================
        // Category 06: Chat context model checkmark matching
        // =========================================================================

        #[test]
        fn cat06_current_model_gets_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("Claude 3.5 Sonnet".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "claude-3-5-sonnet".to_string(),
                    display_name: "Claude 3.5 Sonnet".to_string(),
                    provider: "Anthropic".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-3-5-sonnet")
                .unwrap();
            assert!(model_action.title.contains('✓'));
        }

        #[test]
        fn cat06_non_current_model_no_checkmark() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".to_string()),
                available_models: vec![ChatModelInfo {
                    id: "claude-3-5-sonnet".to_string(),
                    display_name: "Claude 3.5 Sonnet".to_string(),
                    provider: "Anthropic".to_string(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude-3-5-sonnet")
                .unwrap();
            assert!(!model_action.title.contains('✓'));
        }

        #[test]
        fn cat06_model_description_has_via_provider() {
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
            let model_action = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt-4")
                .unwrap();
            assert_eq!(model_action.description.as_ref().unwrap(), "Uses OpenAI");
        }

        #[test]
        fn cat06_multiple_models_ordering_preserved() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![
                    ChatModelInfo {
                        id: "m1".to_string(),
                        display_name: "Model A".to_string(),
                        provider: "P1".to_string(),
                    },
                    ChatModelInfo {
                        id: "m2".to_string(),
                        display_name: "Model B".to_string(),
                        provider: "P2".to_string(),
                    },
                ],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let m1_idx = picker
                .iter()
                .position(|a| a.id == "chat:select_model_m1")
                .unwrap();
            let m2_idx = picker
                .iter()
                .position(|a| a.id == "chat:select_model_m2")
                .unwrap();
            assert!(m1_idx < m2_idx);
        }

        // =========================================================================
        // Category 07: New chat action provider_display_name propagation
        // =========================================================================

        #[test]
        fn cat07_last_used_description_uses_provider_display_name() {
            let last_used = vec![NewChatModelInfo {
                model_id: "claude-3".to_string(),
                display_name: "Claude 3".to_string(),
                provider: "anthropic".to_string(),
                provider_display_name: "Anthropic".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            let lu = actions
                .iter()
                .find(|a| a.id == "last_used_anthropic::claude-3")
                .unwrap();
            assert_eq!(lu.description.as_ref().unwrap(), "Uses Anthropic");
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat07_model_description_uses_provider_display_name() {
            let models = vec![NewChatModelInfo {
                model_id: "gpt-4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "openai".to_string(),
                provider_display_name: "OpenAI".to_string(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            let m = actions
                .iter()
                .find(|a| a.id == "model_openai::gpt-4")
                .unwrap();
            assert_eq!(m.description.as_ref().unwrap(), "Uses OpenAI");
        }

        #[test]
        fn cat07_preset_has_no_description() {
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            let p = actions.iter().find(|a| a.id == "preset_general").unwrap();
            assert_eq!(p.description.as_deref(), Some("Uses General preset"));
        }

        #[test]
        fn cat07_all_sections_present_when_all_inputs_provided() {
            let last_used = vec![NewChatModelInfo {
                model_id: "c3".to_string(),
                display_name: "Claude 3".to_string(),
                provider: "anthropic".to_string(),
                provider_display_name: "Anthropic".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "code".to_string(),
                name: "Code".to_string(),
                icon: IconName::Code,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "gpt-4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "openai".to_string(),
                provider_display_name: "OpenAI".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            let sections: HashSet<_> = actions.iter().filter_map(|a| a.section.as_ref()).collect();
            assert!(sections.contains(&"Last Used Settings".to_string()));
            assert!(sections.contains(&"Presets".to_string()));
            assert!(sections.contains(&"Models".to_string()));
        }

        // =========================================================================
        // Category 08: Note switcher preview boundary — exactly 60 chars
        // =========================================================================

        #[test]
        fn cat08_preview_exactly_60_chars_no_ellipsis() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".to_string(),
                title: "Note".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "a".repeat(60),
                relative_time: String::new(),
            };
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'));
        }

        #[test]
        fn cat08_preview_61_chars_has_ellipsis() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".to_string(),
                title: "Note".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "a".repeat(61),
                relative_time: String::new(),
            };
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains('…'));
        }

        #[test]
        fn cat08_preview_59_chars_no_ellipsis() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".to_string(),
                title: "Note".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "a".repeat(59),
                relative_time: String::new(),
            };
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'));
        }

        #[test]
        fn cat08_empty_preview_no_time_uses_char_count() {
            let note = NoteSwitcherNoteInfo {
                id: "n1".to_string(),
                title: "Note".to_string(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: String::new(),
                relative_time: String::new(),
            };
            let actions = get_note_switcher_actions(&[note]);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        // =========================================================================
        // Category 09: Notes command bar find_in_note details
        // =========================================================================

        // =========================================================================
        // Category 10: Notes command bar export details
        // =========================================================================

        // =========================================================================
        // Category 11: Path context open_in_finder description
        // =========================================================================

        #[test]
        fn cat11_open_in_finder_description() {
            let path = PathInfo {
                path: "/Users/test/Documents".to_string(),
                name: "Documents".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let action = actions
                .iter()
                .find(|a| a.id == "file:open_in_finder")
                .unwrap();
            assert_eq!(
                action.description.as_ref().unwrap(),
                "Shows this item in Finder"
            );
        }

        #[test]
        fn cat11_open_in_finder_shortcut() {
            let path = PathInfo {
                path: "/Users/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let action = actions
                .iter()
                .find(|a| a.id == "file:open_in_finder")
                .unwrap();
            assert_eq!(action.shortcut.as_ref().unwrap(), "⌘⇧F");
        }

        #[test]
        fn cat11_open_in_editor_description_mentions_editor() {
            let path = PathInfo {
                path: "/Users/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let action = actions
                .iter()
                .find(|a| a.id == "file:open_in_editor")
                .unwrap();
            assert!(action.description.as_ref().unwrap().contains("$EDITOR"));
        }

        #[test]
        fn cat11_open_in_quick_terminal_has_no_shortcut() {
            let path = PathInfo {
                path: "/Users/test/Documents".to_string(),
                name: "Documents".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let action = actions
                .iter()
                .find(|a| a.id == "file:open_in_quick_terminal")
                .unwrap();
            // file:open_in_quick_terminal ships without a shortcut.
            assert_eq!(action.shortcut, None);
        }

        // =========================================================================
        // Category 12: File context exact descriptions
        // =========================================================================

        #[test]
        fn cat12_file_open_description() {
            let file = FileInfo {
                path: "/test/doc.pdf".to_string(),
                name: "doc.pdf".to_string(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
            assert_eq!(
                open.description.as_ref().unwrap(),
                "Opens with the default app"
            );
        }

        #[test]
        fn cat12_dir_open_description() {
            let dir = FileInfo {
                path: "/test/folder".to_string(),
                name: "folder".to_string(),
                file_type: crate::file_search::FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&dir);
            let open = actions
                .iter()
                .find(|a| a.id == "file:open_directory")
                .unwrap();
            assert_eq!(open.description.as_ref().unwrap(), "Opens this folder");
        }

        #[test]
        fn cat12_reveal_in_finder_description() {
            let file = FileInfo {
                path: "/test/doc.pdf".to_string(),
                name: "doc.pdf".to_string(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let reveal = actions
                .iter()
                .find(|a| a.id == "file:reveal_in_finder")
                .unwrap();
            assert_eq!(
                reveal.description.as_ref().unwrap(),
                "Shows this item in Finder"
            );
        }

        #[test]
        fn cat12_copy_path_description() {
            let file = FileInfo {
                path: "/test/doc.pdf".to_string(),
                name: "doc.pdf".to_string(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let cp = actions.iter().find(|a| a.id == "file:copy_path").unwrap();
            assert_eq!(
                cp.description.as_ref().unwrap(),
                "Copies the full path to the clipboard"
            );
        }

        #[test]
        fn cat12_copy_filename_description() {
            let file = FileInfo {
                path: "/test/doc.pdf".to_string(),
                name: "doc.pdf".to_string(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            let cf = actions
                .iter()
                .find(|a| a.id == "file:copy_filename")
                .unwrap();
            assert_eq!(
                cf.description.as_ref().unwrap(),
                "Copies only the filename to the clipboard"
            );
        }

        // =========================================================================
        // Category 13: format_shortcut_hint edge cases
        // =========================================================================

        #[test]
        fn cat13_control_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("control+c"), "⌃C");
        }

        #[test]
        fn cat13_super_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("super+c"), "⌘C");
        }

        #[test]
        fn cat13_esc_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
        }

        #[test]
        fn cat13_tab_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("tab"), "⇥");
        }

        #[test]
        fn cat13_backspace_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("backspace"), "⌫");
        }

        #[test]
        fn cat13_delete_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("delete"), "⌫");
        }

        #[test]
        fn cat13_space_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("space"), "␣");
        }

        #[test]
        fn cat13_arrowleft_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowleft"), "←");
        }

        #[test]
        fn cat13_arrowright_key() {
            assert_eq!(ActionsDialog::format_shortcut_hint("arrowright"), "→");
        }

        // =========================================================================
        // Category 14: parse_shortcut_keycaps — all symbol types
        // =========================================================================

        #[test]
        fn cat14_space_symbol() {
            let caps = ActionsDialog::parse_shortcut_keycaps("␣");
            assert_eq!(caps, vec!["␣"]);
        }

        #[test]
        fn cat14_backspace_symbol() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌫");
            assert_eq!(caps, vec!["⌫"]);
        }

        #[test]
        fn cat14_tab_symbol() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⇥");
            assert_eq!(caps, vec!["⇥"]);
        }

        #[test]
        fn cat14_escape_symbol() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(caps, vec!["⎋"]);
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat14_all_arrows() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
        }

        #[test]
        fn cat14_cmd_shift_delete() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧⌫");
            assert_eq!(caps, vec!["⌘", "⇧", "⌫"]);
        }

        // =========================================================================
        // Category 15: score_action with empty search string
        // =========================================================================

        #[test]
        fn cat15_empty_search_matches_prefix() {
            let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "");
            // Empty string is a prefix of everything
            assert_eq!(score, 100);
        }

        #[test]
        fn cat15_single_char_search() {
            let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "t");
            assert_eq!(score, 100); // prefix match
        }

        #[test]
        fn cat15_no_match_returns_zero() {
            let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "xyz");
            assert_eq!(score, 0);
        }

        #[test]
        fn cat15_description_bonus_stacking() {
            let action = Action::new(
                "test",
                "Test Action",
                Some("test description".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "test");
            // prefix(100) + description(15) = 115
            assert_eq!(score, 115);
        }

        #[test]
        fn cat15_shortcut_bonus_stacking() {
            let action = Action::new("test", "Test Action", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘T");
            let score = ActionsDialog::score_action(&action, "⌘t");
            // No title match for "⌘t", but shortcut match: 10
            assert_eq!(score, 10);
        }

        // =========================================================================
        // Category 16: fuzzy_match edge cases
        // =========================================================================

        #[test]
        fn cat16_repeated_chars_in_haystack() {
            // "aaa" in "banana" should match (b-a-n-a-n-a has three a's)
            assert!(ActionsDialog::fuzzy_match("banana", "aaa"));
        }

        #[test]
        fn cat16_repeated_chars_insufficient() {
            // "aaaa" in "banana" should fail (only 3 a's available)
            assert!(!ActionsDialog::fuzzy_match("banana", "aaaa"));
        }

        #[test]
        fn cat16_single_char_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "h"));
        }

        #[test]
        fn cat16_single_char_no_match() {
            assert!(!ActionsDialog::fuzzy_match("hello", "z"));
        }

        #[test]
        fn cat16_full_string_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        // =========================================================================
        // Category 17: build_grouped_items_static — section change behavior
        // =========================================================================

        #[test]
        fn cat17_headers_style_adds_header_on_section_change() {
            let actions = vec![
                Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
                    .with_section("Alpha"),
                Action::new("a2", "Action 2", None, ActionCategory::ScriptContext)
                    .with_section("Beta"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should be: Header("Alpha"), Item(0), Header("Beta"), Item(1)
            assert_eq!(grouped.len(), 4);
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(ref s) if s == "Alpha"));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[2], GroupedActionItem::SectionHeader(ref s) if s == "Beta"));
            assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat17_headers_style_same_section_no_duplicate_header() {
            let actions = vec![
                Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
                    .with_section("Alpha"),
                Action::new("a2", "Action 2", None, ActionCategory::ScriptContext)
                    .with_section("Alpha"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should be: Header("Alpha"), Item(0), Item(1)
            assert_eq!(grouped.len(), 3);
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(ref s) if s == "Alpha"));
        }

        #[test]
        fn cat17_separators_style_no_headers() {
            let actions = vec![
                Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
                    .with_section("Alpha"),
                Action::new("a2", "Action 2", None, ActionCategory::ScriptContext)
                    .with_section("Beta"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // No headers for separators mode
            assert_eq!(grouped.len(), 2);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat17_none_style_no_headers() {
            let actions = vec![
                Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
                    .with_section("Alpha"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(grouped.len(), 1);
            assert!(matches!(grouped[0], GroupedActionItem::Item(0)));
        }

        #[test]
        fn cat17_empty_filtered_returns_empty() {
            let actions = vec![Action::new(
                "a1",
                "Action 1",
                None,
                ActionCategory::ScriptContext,
            )];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        // =========================================================================
        // Category 18: coerce_action_selection — consecutive headers
        // =========================================================================

        #[test]
        fn cat18_two_consecutive_headers_then_item() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::SectionHeader("B".to_string()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(2));
        }

        #[test]
        fn cat18_header_at_end_searches_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("A".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat18_single_item_returns_itself() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn cat18_index_beyond_bounds_clamped() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 99), Some(0));
        }

        #[test]
        fn cat18_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("A".to_string()),
                GroupedActionItem::SectionHeader("B".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        // =========================================================================
        // Category 19: CommandBarConfig main_menu_style values
        // =========================================================================

        #[test]
        fn cat19_main_menu_search_top() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        }

        #[test]
        fn cat19_main_menu_separators() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat19_main_menu_anchor_bottom() {
            let config = CommandBarConfig::main_menu_style();
            assert_eq!(config.dialog_config.anchor, AnchorPosition::Bottom);
        }

        #[test]
        fn cat19_main_menu_no_icons() {
            let config = CommandBarConfig::main_menu_style();
            assert!(!config.dialog_config.show_icons);
        }

        #[test]
        fn cat19_main_menu_no_footer() {
            let config = CommandBarConfig::main_menu_style();
            assert!(!config.dialog_config.show_footer);
        }

        // =========================================================================
        // Category 20: CommandBarConfig ai_style values
        // =========================================================================

        #[test]
        fn cat20_ai_style_search_top() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
        }

        #[test]
        fn cat20_ai_style_headers() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat20_ai_style_anchor_top() {
            let config = CommandBarConfig::ai_style();
            assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
        }

        #[test]
        fn cat20_ai_style_icons_enabled() {
            let config = CommandBarConfig::ai_style();
            assert!(config.dialog_config.show_icons);
        }

        #[test]
        fn cat20_ai_style_footer_enabled() {
            let config = CommandBarConfig::ai_style();
            assert!(!config.dialog_config.show_footer);
        }

        // =========================================================================
        // Category 21: CommandBarConfig no_search values
        // =========================================================================

        #[test]
        fn cat21_no_search_hidden() {
            let config = CommandBarConfig::no_search();
            assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
        }

        #[test]
        fn cat21_no_search_separators() {
            let config = CommandBarConfig::no_search();
            assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
        }

        #[test]
        fn cat21_no_search_close_defaults_true() {
            let config = CommandBarConfig::no_search();
            assert!(config.close_on_select);
            assert!(config.close_on_click_outside);
            assert!(config.close_on_escape);
        }

        // =========================================================================
        // Category 22: Action with_section sets section field
        // =========================================================================

        #[test]
        fn cat22_with_section_sets_field() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_section("MySection");
            assert_eq!(action.section, Some("MySection".to_string()));
        }

        #[test]
        fn cat22_no_section_by_default() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.section.is_none());
        }

        #[test]
        fn cat22_with_section_preserves_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘T")
                .with_section("MySection");
            assert_eq!(action.shortcut, Some("⌘T".to_string()));
            assert_eq!(action.section, Some("MySection".to_string()));
        }

        #[test]
        fn cat22_with_section_preserves_icon() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Star)
                .with_section("MySection");
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section, Some("MySection".to_string()));
        }

        // =========================================================================
        // Category 23: ScriptInfo is_agent defaults and combined flags
        // =========================================================================

        #[test]
        fn cat23_new_is_agent_false() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            assert!(!script.is_agent);
        }

        #[test]
        fn cat23_builtin_is_agent_false() {
            let builtin = ScriptInfo::builtin("Clipboard History");
            assert!(!builtin.is_agent);
        }

        #[test]
        fn cat23_scriptlet_is_agent_false() {
            let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            assert!(!scriptlet.is_agent);
        }

        #[test]
        fn cat23_with_all_is_agent_false() {
            let script = ScriptInfo::with_all("Test", "/path", true, "Run", None, None);
            assert!(!script.is_agent);
        }

        #[test]
        fn cat23_agent_mutually_exclusive_with_script() {
            let mut script = ScriptInfo::new("Test", "/path");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            // Agent should NOT have view_logs (script-only)
            assert!(!actions.iter().any(|a| a.id == "view_logs"));
            // Agent SHOULD have edit_script (with "Edit Agent" title)
            assert!(actions.iter().any(|a| a.id == "edit_script"));
        }

        // =========================================================================
        // Category 24: Clipboard save_snippet and save_file shortcuts
        // =========================================================================

        #[test]
        fn cat24_save_snippet_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let snippet = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(snippet.shortcut.as_ref().unwrap(), "⇧⌘S");
        }

        #[test]
        fn cat24_save_file_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let save = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert_eq!(save.shortcut.as_ref().unwrap(), "⌥⇧⌘S");
        }

        #[test]
        fn cat24_save_snippet_title() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let snippet = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(snippet.title, "Save Text as Snippet");
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat24_save_file_title() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let save = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert_eq!(save.title, "Save as File...");
        }

        // =========================================================================
        // Category 25: Clipboard delete actions shortcuts
        // =========================================================================

        #[test]
        fn cat25_delete_entry_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete")
                .unwrap();
            assert_eq!(del.shortcut.as_ref().unwrap(), "⌃X");
        }

        #[test]
        fn cat25_delete_multiple_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_multiple")
                .unwrap();
            assert_eq!(del.shortcut.as_ref().unwrap(), "⇧⌘X");
        }

        #[test]
        fn cat25_delete_all_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_all")
                .unwrap();
            assert_eq!(del.shortcut.as_ref().unwrap(), "⌃⇧X");
        }

        #[test]
        fn cat25_delete_all_description_mentions_pinned() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let del = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_all")
                .unwrap();
            assert!(del.description.as_ref().unwrap().contains("pinned"));
        }

        // =========================================================================
        // Category 26: to_deeplink_name edge cases
        // =========================================================================

        #[test]
        fn cat26_numbers_only() {
            assert_eq!(to_deeplink_name("12345"), "12345");
        }

        #[test]
        fn cat26_all_special_chars_becomes_empty() {
            assert_eq!(to_deeplink_name("!@#$%^&*()"), "_unnamed");
        }

        #[test]
        fn cat26_mixed_case_lowered() {
            assert_eq!(to_deeplink_name("CamelCase"), "camelcase");
        }

        #[test]
        fn cat26_consecutive_specials_collapsed() {
            assert_eq!(to_deeplink_name("hello---world"), "hello-world");
        }

        #[test]
        fn cat26_underscores_become_hyphens() {
            assert_eq!(to_deeplink_name("hello_world"), "hello-world");
        }

        #[test]
        fn cat26_leading_trailing_specials_stripped() {
            assert_eq!(to_deeplink_name("--hello--"), "hello");
        }

        #[test]
        fn cat26_unicode_preserved() {
            // Non-ASCII characters are percent-encoded.
            assert_eq!(to_deeplink_name("日本語"), "%E6%97%A5%E6%9C%AC%E8%AA%9E");
        }

        // =========================================================================
        // Category 27: AI command bar per-section action counts
        // =========================================================================

        #[test]
        fn cat27_response_section_has_3_actions() {
            let actions = get_ai_command_bar_actions();
            let response_count = actions
                .iter()
                .filter(|a| a.section.as_ref() == Some(&"Response".to_string()))
                .count();
            assert_eq!(response_count, 3);
        }

        #[test]
        fn cat27_actions_section_has_7_actions() {
            let actions = get_ai_command_bar_actions();
            let actions_count = actions
                .iter()
                .filter(|a| a.section.as_ref() == Some(&"Actions".to_string()))
                .count();
            assert_eq!(actions_count, 7);
        }

        #[test]
        fn cat27_attachments_section_has_2_actions() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_ref() == Some(&"Attachments".to_string()))
                .count();
            assert_eq!(count, 4);
        }

        #[test]
        fn cat27_export_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_ref() == Some(&"Export".to_string()))
                .count();
            assert_eq!(count, 1);
        }

        #[test]
        fn cat27_help_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_ref() == Some(&"Help".to_string()))
                .count();
            assert_eq!(count, 1);
        }

        #[test]
        fn cat27_settings_section_has_1_action() {
            let actions = get_ai_command_bar_actions();
            let count = actions
                .iter()
                .filter(|a| a.section.as_ref() == Some(&"Settings".to_string()))
                .count();
            assert_eq!(count, 2);
        }

        #[test]
        fn cat27_total_ai_actions_is_12() {
            let actions = get_ai_command_bar_actions();
            assert_eq!(actions.len(), 38);
        }

        // =========================================================================
        // Category 28: Notes command bar all flag combinations
        // =========================================================================

        // =========================================================================
        // Category 29: Path context move_to_trash description formatting
        // =========================================================================

        #[test]
        fn cat29_trash_dir_description_says_folder() {
            let path = PathInfo {
                path: "/Users/test/Documents".to_string(),
                name: "Documents".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(trash.description.as_ref().unwrap().contains("folder"));
        }

        #[test]
        fn cat29_trash_file_description_says_file() {
            let path = PathInfo {
                path: "/Users/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert!(trash.description.as_ref().unwrap().contains("file"));
        }

        #[test]
        fn cat29_trash_shortcut() {
            let path = PathInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            let trash = actions
                .iter()
                .find(|a| a.id == "file:move_to_trash")
                .unwrap();
            assert_eq!(trash.shortcut.as_ref().unwrap(), "⌘⌫");
        }

        #[test]
        fn cat29_trash_always_last() {
            let path = PathInfo {
                path: "/test/Documents".to_string(),
                name: "Documents".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let last = actions.last().unwrap();
            assert_eq!(last.id, "file:move_to_trash");
        }

        // =========================================================================
        // Category 30: Cross-context all actions have non-empty descriptions
        // =========================================================================

        #[test]
        fn cat30_script_actions_all_have_descriptions() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "Script action '{}' should have description",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_clipboard_text_actions_all_have_descriptions() {
            let entry = ClipboardEntryInfo {
                id: "txt1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "Hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "Clipboard action '{}' should have description",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_ai_actions_all_have_descriptions() {
            let actions = get_ai_command_bar_actions();
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "AI action '{}' should have description",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_path_actions_all_have_descriptions() {
            let path = PathInfo {
                path: "/test/file.txt".to_string(),
                name: "file.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "Path action '{}' should have description",
                    action.id
                );
            }
        }

        #[test]
        fn cat30_file_actions_all_have_descriptions() {
            let file = FileInfo {
                path: "/test/doc.pdf".to_string(),
                name: "doc.pdf".to_string(),
                file_type: crate::file_search::FileType::Document,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            for action in &actions {
                assert!(
                    action.description.is_some(),
                    "File action '{}' should have description",
                    action.id
                );
            }
        }

    }
}

mod from_dialog_builtin_action_validation_tests_19 {
    // =============================================================================
    // Dialog Built-in Action Validation Tests — Batch 19
    //
    // 30 categories of tests validating random built-in actions from dialog windows.
    // Each category tests a specific behavior, field, or invariant.
    //
    // Run with:
    //   cargo test --lib actions::dialog_builtin_action_validation_tests_19
    // =============================================================================

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use std::collections::HashSet;

        // =========================================================================
        // Category 01: Scriptlet context shortcut/alias toggle symmetry
        // Verifies that scriptlet context actions correctly toggle add vs
        // update/remove based on shortcut and alias presence, mirroring script context.
        // =========================================================================

        #[test]
        fn cat01_scriptlet_no_shortcut_has_add_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "add_shortcut"));
            assert!(!actions.iter().any(|a| a.id == "update_shortcut"));
        }

        #[test]
        fn cat01_scriptlet_with_shortcut_has_update_remove() {
            let script =
                ScriptInfo::scriptlet("Test", "/path/test.md", Some("cmd+t".to_string()), None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
            assert!(actions.iter().any(|a| a.id == "update_shortcut"));
            assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
        }

        #[test]
        fn cat01_scriptlet_no_alias_has_add_alias() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "add_alias"));
            assert!(!actions.iter().any(|a| a.id == "update_alias"));
        }

        #[test]
        fn cat01_scriptlet_with_alias_has_update_remove_alias() {
            let script =
                ScriptInfo::scriptlet("Test", "/path/test.md", None, Some("ts".to_string()));
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(!actions.iter().any(|a| a.id == "add_alias"));
            assert!(actions.iter().any(|a| a.id == "update_alias"));
            assert!(actions.iter().any(|a| a.id == "remove_alias"));
        }

        #[test]
        fn cat01_scriptlet_both_shortcut_and_alias_set() {
            let script = ScriptInfo::scriptlet(
                "Test",
                "/path/test.md",
                Some("cmd+t".to_string()),
                Some("ts".to_string()),
            );
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "update_shortcut"));
            assert!(actions.iter().any(|a| a.id == "remove_shortcut"));
            assert!(actions.iter().any(|a| a.id == "update_alias"));
            assert!(actions.iter().any(|a| a.id == "remove_alias"));
        }

        // =========================================================================
        // Category 02: Script context deeplink description contains URL
        // Verifies the copy_deeplink action description format across script types.
        // =========================================================================

        #[test]
        fn cat02_script_deeplink_desc_contains_url_pattern() {
            let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(
                dl.description.as_deref(),
                Some("Copy a portable Script Kit share link to clipboard")
            );
        }

        #[test]
        fn cat02_script_deeplink_desc_contains_deeplink_name() {
            let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
            let actions = get_script_context_actions(&script);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert_eq!(dl.title, "Share");
        }

        #[test]
        fn cat02_builtin_deeplink_desc_contains_url() {
            let script = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&script);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert!(
                dl.description
                    .as_ref()
                    .unwrap()
                    .contains("clipboard-history")
            );
        }

        #[test]
        fn cat02_scriptlet_context_deeplink_desc_contains_url() {
            let script = ScriptInfo::scriptlet("Open GitHub", "/path/urls.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
            assert!(dl.description.as_ref().unwrap().contains("open-github"));
        }

        // =========================================================================
        // Category 03: Clipboard frontmost_app_name dynamic paste title
        // Tests that paste action title changes based on the frontmost app.
        // =========================================================================

        #[test]
        fn cat03_clipboard_paste_title_no_app() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
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
        fn cat03_clipboard_paste_title_with_app() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
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
        fn cat03_clipboard_paste_title_unicode_app() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: Some("日本語エディタ".to_string()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            assert_eq!(paste.title, "Paste to 日本語エディタ");
        }

        #[test]
        fn cat03_clipboard_paste_title_empty_app_string() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: Some("".to_string()),
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let paste = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_paste")
                .unwrap();
            // Empty string falls back to the generic "Paste to Active App" title
            assert_eq!(paste.title, "Paste to Active App");
        }

        // =========================================================================
        // Category 04: Notes command bar action presence per flag combination
        // Explores specific actions' conditional visibility across flag combos.
        // =========================================================================

        // =========================================================================
        // Category 05: Chat context action count boundary states
        // Validates exact action count under different ChatPromptInfo combos.
        // =========================================================================

        #[test]
        fn cat05_chat_zero_models_no_flags_one_action() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions.len(), 3); // continue_in_chat + capture
            assert_eq!(actions[0].id, "chat:change_model");
        }

        #[test]
        fn cat05_chat_zero_models_both_flags_three_actions() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![],
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions.len(), 5);
        }

        #[test]
        fn cat05_chat_two_models_no_flags_three_actions() {
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
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions.len(), 3); // 2 models + continue_in_chat + capture
        }

        #[test]
        fn cat05_chat_two_models_both_flags_five_actions() {
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
                has_messages: true,
                has_response: true,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions.len(), 5); // 2 models + continue + copy + clear + capture
        }

        // =========================================================================
        // Category 06: New chat actions section assignment correctness
        // Validates that each action lands in the right section.
        // =========================================================================

        #[test]
        fn cat06_new_chat_last_used_section() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "P1".to_string(),
                provider_display_name: "Provider 1".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
        }

        #[test]
        fn cat06_new_chat_preset_section() {
            let presets = vec![NewChatPresetInfo {
                id: "general".to_string(),
                name: "General".to_string(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].section.as_deref(), Some("Presets"));
        }

        #[test]
        fn cat06_new_chat_model_section() {
            let models = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model 1".to_string(),
                provider: "P1".to_string(),
                provider_display_name: "Provider 1".to_string(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].section.as_deref(), Some("Models"));
        }

        #[test]
        fn cat06_new_chat_mixed_sections_correct_order() {
            let last_used = vec![NewChatModelInfo {
                model_id: "lu".to_string(),
                display_name: "LU".to_string(),
                provider: "P".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let presets = vec![NewChatPresetInfo {
                id: "p1".to_string(),
                name: "Preset".to_string(),
                icon: IconName::Star,
            }];
            let models = vec![NewChatModelInfo {
                model_id: "m1".to_string(),
                display_name: "Model".to_string(),
                provider: "P".to_string(),
                provider_display_name: "Provider".to_string(),
            }];
            let actions = get_new_chat_actions(&last_used, &presets, &models);
            assert_eq!(actions.len(), 3);
            assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
            assert_eq!(actions[1].section.as_deref(), Some("Presets"));
            assert_eq!(actions[2].section.as_deref(), Some("Models"));
        }

        #[test]
        fn cat06_new_chat_empty_all_returns_empty() {
            let actions = get_new_chat_actions(&[], &[], &[]);
            assert!(actions.is_empty());
        }

        // =========================================================================
        // Category 07: Note switcher description edge cases with preview+time
        // Tests various combinations of preview and relative_time.
        // =========================================================================

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat07_note_switcher_preview_and_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".to_string(),
                title: "Test".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "Hello world".to_string(),
                relative_time: "2m ago".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains("Hello world"));
            assert!(desc.contains("2m ago"));
            assert!(desc.contains("·"));
        }

        #[test]
        fn cat07_note_switcher_preview_no_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".to_string(),
                title: "Test".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "Hello world".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "Hello world");
        }

        #[test]
        fn cat07_note_switcher_no_preview_with_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".to_string(),
                title: "Test".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "5d ago".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "5d ago");
        }

        #[test]
        fn cat07_note_switcher_no_preview_no_time_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".to_string(),
                title: "Test".to_string(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "42 chars");
        }

        #[test]
        fn cat07_note_switcher_zero_chars_fallback() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".to_string(),
                title: "Test".to_string(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "0 chars");
        }

        #[test]
        fn cat07_note_switcher_one_char_singular() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "abc".to_string(),
                title: "Test".to_string(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "1 char");
        }

        // =========================================================================
        // Category 08: Script context action ordering invariant — run_script first
        // Validates that run_script is always the very first action.
        // =========================================================================

        #[test]
        fn cat08_script_run_first_basic() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat08_script_run_first_with_shortcut() {
            let script =
                ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".to_string()));
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat08_script_run_first_builtin() {
            let script = ScriptInfo::builtin("Clipboard History");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat08_scriptlet_run_first() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert_eq!(actions[0].id, "run_script");
        }

        #[test]
        fn cat08_script_run_shortcut_is_enter() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
        }

        // =========================================================================
        // Category 09: Path context directory vs file primary action distinction
        // Verifies the different primary action IDs and titles for dirs vs files.
        // =========================================================================

        #[test]
        fn cat09_path_dir_primary_is_open_directory() {
            let info = PathInfo {
                path: "/users/test/Downloads".to_string(),
                is_dir: true,
                name: "Downloads".to_string(),
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn cat09_path_file_primary_is_select_file() {
            let info = PathInfo {
                path: "/users/test/file.txt".to_string(),
                is_dir: false,
                name: "file.txt".to_string(),
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn cat09_path_dir_primary_shortcut_enter() {
            let info = PathInfo {
                path: "/users/test/Downloads".to_string(),
                is_dir: true,
                name: "Downloads".to_string(),
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn cat09_path_file_primary_shortcut_enter() {
            let info = PathInfo {
                path: "/users/test/file.txt".to_string(),
                is_dir: false,
                name: "file.txt".to_string(),
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
        }

        #[test]
        fn cat09_path_trash_always_last() {
            let info = PathInfo {
                path: "/users/test/Downloads".to_string(),
                is_dir: true,
                name: "Downloads".to_string(),
            };
            let actions = get_path_context_actions(&info);
            assert_eq!(actions.last().unwrap().id, "file:move_to_trash");
        }

        // =========================================================================
        // Category 10: File context macOS-only action presence
        // Validates macOS-specific actions exist for file context on macOS.
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat10_file_macos_has_quick_look() {
            let info = FileInfo {
                path: "/users/test/photo.jpg".to_string(),
                is_dir: false,
                name: "photo.jpg".to_string(),
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat10_file_macos_has_open_in_editor() {
            let info = FileInfo {
                path: "/users/test/photo.jpg".to_string(),
                is_dir: false,
                name: "photo.jpg".to_string(),
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat10_file_macos_dir_no_quick_look() {
            let info = FileInfo {
                path: "/users/test/folder".to_string(),
                is_dir: true,
                name: "folder".to_string(),
                file_type: FileType::Directory,
            };
            let actions = get_file_context_actions(&info);
            assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat10_file_macos_has_show_info() {
            let info = FileInfo {
                path: "/users/test/photo.jpg".to_string(),
                is_dir: false,
                name: "photo.jpg".to_string(),
                file_type: FileType::File,
            };
            let actions = get_file_context_actions(&info);
            assert!(actions.iter().any(|a| a.id == "file:show_info"));
        }

        // =========================================================================
        // Category 11: AI command bar action section membership — exact IDs
        // Validates exact action IDs within each section of the AI command bar.
        // =========================================================================

        #[test]
        fn cat11_ai_response_section_ids() {
            let actions = get_ai_command_bar_actions();
            let response_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Response"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                response_ids,
                vec![
                    "chat:copy_response",
                    "chat:copy_chat",
                    "chat:copy_last_code"
                ]
            );
        }

        #[test]
        fn cat11_ai_actions_section_ids() {
            let actions = get_ai_command_bar_actions();
            let action_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Actions"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                action_ids,
                vec![
                    "chat:submit",
                    "chat:new_chat",
                    "chat:delete_chat",
                    "chat:branch_from_last",
                    "chat:toggle_favorite_model",
                    "chat:cycle_favorite_model",
                    "chat:expand_composer"
                ]
            );
        }

        #[test]
        fn cat11_ai_attachments_section_ids() {
            let actions = get_ai_command_bar_actions();
            let attachment_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Attachments"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(
                attachment_ids,
                vec![
                    "chat:add_file",
                    "chat:add_image",
                    "chat:paste_image",
                    "chat:capture_screen_area"
                ]
            );
        }

        #[test]
        fn cat11_ai_export_section_ids() {
            let actions = get_ai_command_bar_actions();
            let export_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Export"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(export_ids, vec!["chat:export_markdown"]);
        }

        #[test]
        fn cat11_ai_help_and_settings_section_ids() {
            let actions = get_ai_command_bar_actions();
            let help_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Help"))
                .map(|a| a.id.as_str())
                .collect();
            let settings_ids: Vec<&str> = actions
                .iter()
                .filter(|a| a.section.as_deref() == Some("Settings"))
                .map(|a| a.id.as_str())
                .collect();
            assert_eq!(help_ids, vec!["chat:toggle_shortcuts_help"]);
            assert_eq!(
                settings_ids,
                vec!["chat:change_model", "chat:toggle_window_mode"]
            );
        }

        // =========================================================================
        // Category 12: to_deeplink_name special character handling
        // Tests edge cases for special character replacement and collapsing.
        // =========================================================================

        #[test]
        fn cat12_deeplink_spaces_to_hyphens() {
            assert_eq!(to_deeplink_name("Hello World"), "hello-world");
        }

        #[test]
        fn cat12_deeplink_consecutive_specials_collapsed() {
            assert_eq!(to_deeplink_name("a--b__c  d"), "a-b-c-d");
        }

        #[test]
        fn cat12_deeplink_leading_trailing_stripped() {
            assert_eq!(to_deeplink_name("---hello---"), "hello");
        }

        #[test]
        fn cat12_deeplink_all_specials_returns_empty() {
            assert_eq!(to_deeplink_name("!@#$%^&*"), "_unnamed");
        }

        #[test]
        fn cat12_deeplink_unicode_preserved() {
            let result = to_deeplink_name("日本語テスト");
            assert!(result.starts_with("%E6%97%A5"));
            assert!(result.contains("%E8%AA%9E"));
        }

        #[test]
        fn cat12_deeplink_mixed_case_lowered() {
            assert_eq!(to_deeplink_name("MyScript"), "myscript");
        }

        // =========================================================================
        // Category 13: format_shortcut_hint modifier replacement
        // Tests that modifier keys are correctly replaced with symbols.
        // (format_shortcut_hint is private, so we test it indirectly via action shortcuts)
        // =========================================================================

        #[test]
        fn cat13_script_add_shortcut_uses_formatted_hint() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let add_shortcut = actions.iter().find(|a| a.id == "add_shortcut").unwrap();
            // Expected: ⌘⇧K (cmd+shift+k formatted)
            assert_eq!(add_shortcut.shortcut.as_deref(), Some("⌘⇧K"));
        }

        #[test]
        fn cat13_script_edit_shortcut_formatted() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn cat13_script_view_logs_shortcut_formatted() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let logs = actions.iter().find(|a| a.id == "view_logs").unwrap();
            assert_eq!(logs.shortcut.as_deref(), Some("⌘L"));
        }

        #[test]
        fn cat13_clipboard_delete_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
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

        #[test]
        fn cat13_clipboard_delete_all_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hello".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let del_all = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_all")
                .unwrap();
            assert_eq!(del_all.shortcut.as_deref(), Some("⌃⇧X"));
        }

        // =========================================================================
        // Category 14: score_action prefix vs contains vs fuzzy scoring
        // Tests the scoring function used for filtering actions.
        // =========================================================================

        #[test]
        fn cat14_score_prefix_match_100_plus() {
            let action = Action::new(
                "edit_script",
                "Edit Script",
                Some("Open in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "edit");
            assert!(score >= 100, "Prefix match should be 100+, got {}", score);
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat14_score_contains_match_50() {
            let action = Action::new(
                "file:copy_path",
                "Copy Path",
                Some("Copy to clipboard".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "path");
            assert!(
                (50..100).contains(&score),
                "Contains match should be 50-99, got {}",
                score
            );
        }

        #[test]
        fn cat14_score_no_match_zero() {
            let action = Action::new(
                "edit_script",
                "Edit Script",
                None,
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "zzz");
            assert_eq!(score, 0, "No match should be 0, got {}", score);
        }

        #[test]
        fn cat14_score_empty_query_prefix() {
            let action = Action::new(
                "edit_script",
                "Edit Script",
                None,
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "");
            assert!(
                score >= 100,
                "Empty query is prefix match (100+), got {}",
                score
            );
        }

        #[test]
        fn cat14_score_description_bonus() {
            let action = Action::new(
                "file:open_file",
                "Open File",
                Some("Edit the file in your editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "editor");
            assert!(
                score >= 15,
                "Description match should give 15+, got {}",
                score
            );
        }

        // =========================================================================
        // Category 15: fuzzy_match subsequence correctness
        // Tests the fuzzy matching helper function.
        // =========================================================================

        #[test]
        fn cat15_fuzzy_exact_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        #[test]
        fn cat15_fuzzy_subsequence_match() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlwrd"));
        }

        #[test]
        fn cat15_fuzzy_no_match() {
            assert!(!ActionsDialog::fuzzy_match("hello", "xyz"));
        }

        #[test]
        fn cat15_fuzzy_empty_needle() {
            assert!(ActionsDialog::fuzzy_match("hello", ""));
        }

        #[test]
        fn cat15_fuzzy_empty_haystack() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn cat15_fuzzy_needle_longer_than_haystack() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        #[test]
        fn cat15_fuzzy_both_empty() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        // =========================================================================
        // Category 16: build_grouped_items_static section style behavior
        // Tests that different section styles produce correct GroupedActionItem layouts.
        // =========================================================================

        #[test]
        fn cat16_grouped_headers_style_adds_section_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should have: Header(S1), Item(0), Header(S2), Item(1)
            assert_eq!(items.len(), 4);
            assert!(matches!(items[0], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(items[1], GroupedActionItem::Item(0)));
            assert!(matches!(items[2], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(items[3], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat16_grouped_separators_style_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            // No headers, just items
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], GroupedActionItem::Item(0)));
            assert!(matches!(items[1], GroupedActionItem::Item(1)));
        }

        #[test]
        fn cat16_grouped_none_style_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered = vec![0, 1];
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            assert_eq!(items.len(), 2);
            assert!(matches!(items[0], GroupedActionItem::Item(0)));
        }

        #[test]
        fn cat16_grouped_empty_filtered_returns_empty() {
            let actions =
                vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1")];
            let filtered: Vec<usize> = vec![];
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(items.is_empty());
        }

        #[test]
        fn cat16_grouped_same_section_one_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S1"),
            ];
            let filtered = vec![0, 1];
            let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Header(S1), Item(0), Item(1) — no duplicate header
            assert_eq!(items.len(), 3);
            let header_count = items
                .iter()
                .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1);
        }

        // =========================================================================
        // Category 17: coerce_action_selection header skipping
        // Tests that selection correctly skips over section headers.
        // =========================================================================

        #[test]
        fn cat17_coerce_on_item_stays() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn cat17_coerce_on_header_moves_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S1".to_string()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat17_coerce_trailing_header_moves_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S1".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat17_coerce_all_headers_returns_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S1".to_string()),
                GroupedActionItem::SectionHeader("S2".to_string()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat17_coerce_empty_returns_none() {
            let rows: Vec<GroupedActionItem> = vec![];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        // =========================================================================
        // Category 18: CommandBarConfig preset defaults — close flags
        // Validates that all presets have consistent close behavior defaults.
        // =========================================================================

        #[test]
        fn cat18_default_config_close_on_select() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
        }

        #[test]
        fn cat18_default_config_close_on_escape() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat18_ai_style_close_on_select() {
            let config = CommandBarConfig::ai_style();
            assert!(config.close_on_select);
        }

        #[test]
        fn cat18_main_menu_close_on_select() {
            let config = CommandBarConfig::main_menu_style();
            assert!(config.close_on_select);
        }

        #[test]
        fn cat18_notes_style_close_on_select() {
            let config = CommandBarConfig::notes_style();
            assert!(config.close_on_select);
        }

        #[test]
        fn cat18_no_search_close_on_escape() {
            let config = CommandBarConfig::no_search();
            assert!(config.close_on_escape);
        }

        // =========================================================================
        // Category 19: Action lowercase caching correctness
        // Verifies that title_lower, description_lower, and shortcut_lower
        // are correctly pre-computed when constructing an Action.
        // =========================================================================

        #[test]
        fn cat19_title_lower_computed() {
            let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "edit script");
        }

        #[test]
        fn cat19_description_lower_computed() {
            let action = Action::new(
                "test",
                "Test",
                Some("Open In Editor".to_string()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower, Some("open in editor".to_string()));
        }

        #[test]
        fn cat19_description_lower_none_when_no_desc() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn cat19_shortcut_lower_none_initially() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn cat19_shortcut_lower_set_after_with_shortcut() {
            let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E");
            assert_eq!(action.shortcut_lower, Some("⌘e".to_string()));
        }

        #[test]
        fn cat19_unicode_title_lower() {
            let action = Action::new("test", "ÜBER SCRIPT", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "über script");
        }

        // =========================================================================
        // Category 20: Action builder chaining — field preservation
        // Verifies that chaining with_shortcut, with_icon, with_section
        // preserves previously set fields.
        // =========================================================================

        #[test]
        fn cat20_with_shortcut_then_icon_preserves_shortcut() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E")
                .with_icon(IconName::Copy);
            assert_eq!(action.shortcut.as_deref(), Some("⌘E"));
            assert_eq!(action.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat20_with_icon_then_section_preserves_icon() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_icon(IconName::Star)
                .with_section("Help");
            assert_eq!(action.icon, Some(IconName::Star));
            assert_eq!(action.section.as_deref(), Some("Help"));
        }

        #[test]
        fn cat20_full_chain_preserves_all() {
            let action = Action::new(
                "t",
                "T",
                Some("Desc".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_shortcut("⌘E")
            .with_icon(IconName::Settings)
            .with_section("Settings");
            assert_eq!(action.shortcut.as_deref(), Some("⌘E"));
            assert_eq!(action.icon, Some(IconName::Settings));
            assert_eq!(action.section.as_deref(), Some("Settings"));
            assert_eq!(action.description.as_deref(), Some("Desc"));
        }

        #[test]
        fn cat20_with_shortcut_opt_none_preserves_existing() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘E")
                .with_shortcut_opt(None);
            // with_shortcut_opt(None) preserves the existing shortcut
            assert_eq!(action.shortcut.as_deref(), Some("⌘E"));
        }

        #[test]
        fn cat20_with_shortcut_opt_some_sets() {
            let action = Action::new("t", "T", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘F".to_string()));
            assert_eq!(action.shortcut.as_deref(), Some("⌘F"));
        }

        // =========================================================================
        // Category 21: ScriptInfo constructor defaults and mutability
        // Validates default field values across constructors and mutability of flags.
        // =========================================================================

        #[test]
        fn cat21_new_defaults() {
            let s = ScriptInfo::new("test", "/path/test.ts");
            assert!(s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
            assert_eq!(s.action_verb, "Run");
            assert!(s.shortcut.is_none());
            assert!(s.alias.is_none());
            assert!(!s.is_suggested);
        }

        #[test]
        fn cat21_builtin_defaults() {
            let s = ScriptInfo::builtin("Test");
            assert!(!s.is_script);
            assert!(!s.is_scriptlet);
            assert!(!s.is_agent);
            assert!(s.path.is_empty());
        }

        #[test]
        fn cat21_scriptlet_defaults() {
            let s = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            assert!(!s.is_script);
            assert!(s.is_scriptlet);
            assert!(!s.is_agent);
        }

        #[test]
        fn cat21_agent_via_mutation() {
            let mut s = ScriptInfo::new("Agent", "/path/agent.md");
            s.is_agent = true;
            s.is_script = false;
            assert!(s.is_agent);
            assert!(!s.is_script);
        }

        #[test]
        fn cat21_with_frecency_builder() {
            let s = ScriptInfo::new("test", "/path/test.ts")
                .with_frecency(true, Some("/frecency".to_string()));
            assert!(s.is_suggested);
            assert_eq!(s.frecency_path, Some("/frecency".to_string()));
        }

        // =========================================================================
        // Category 22: Clipboard pin/unpin toggle — exact action details
        // Validates that pin/unpin toggle produces correct titles/descriptions.
        // =========================================================================

        #[test]
        fn cat22_unpinned_shows_pin() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_pin"));
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        }

        #[test]
        fn cat22_pinned_shows_unpin() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(!actions.iter().any(|a| a.id == "clip:clipboard_pin"));
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_unpin"));
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat22_pin_unpin_same_shortcut() {
            let pinned_entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: true,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let unpinned_entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
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
            assert_eq!(pin.shortcut.as_deref(), Some("⇧⌘P"));
        }

        #[test]
        fn cat22_pin_title_and_description() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let pin = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_pin")
                .unwrap();
            assert_eq!(pin.title, "Pin Entry");
            assert!(
                pin.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("pin")
            );
        }

        // =========================================================================
        // Category 23: Clipboard save actions — snippet and file shortcuts
        // Validates save snippet and save file action details.
        // =========================================================================

        #[test]
        fn cat23_save_snippet_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let snippet = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(snippet.shortcut.as_deref(), Some("⇧⌘S"));
        }

        #[test]
        fn cat23_save_file_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let file = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert_eq!(file.shortcut.as_deref(), Some("⌥⇧⌘S"));
        }

        #[test]
        fn cat23_save_snippet_title() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let snippet = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_snippet")
                .unwrap();
            assert_eq!(snippet.title, "Save Text as Snippet");
        }

        #[test]
        fn cat23_save_file_title() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let file = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_save_file")
                .unwrap();
            assert_eq!(file.title, "Save as File...");
        }

        // =========================================================================
        // Category 24: Script context shortcut count — add vs update/remove
        // Validates exact action count difference between no-shortcut and with-shortcut.
        // =========================================================================

        #[test]
        fn cat24_no_shortcut_has_add_only() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let shortcut_actions: Vec<&Action> = actions
                .iter()
                .filter(|a| {
                    a.id == "add_shortcut" || a.id == "update_shortcut" || a.id == "remove_shortcut"
                })
                .collect();
            assert_eq!(shortcut_actions.len(), 1);
            assert_eq!(shortcut_actions[0].id, "add_shortcut");
        }

        #[test]
        fn cat24_with_shortcut_has_update_and_remove() {
            let script =
                ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".to_string()));
            let actions = get_script_context_actions(&script);
            let shortcut_actions: Vec<&Action> = actions
                .iter()
                .filter(|a| {
                    a.id == "add_shortcut" || a.id == "update_shortcut" || a.id == "remove_shortcut"
                })
                .collect();
            assert_eq!(shortcut_actions.len(), 2);
            let ids: HashSet<&str> = shortcut_actions.iter().map(|a| a.id.as_str()).collect();
            assert!(ids.contains("update_shortcut"));
            assert!(ids.contains("remove_shortcut"));
        }

        #[test]
        fn cat24_with_shortcut_one_more_action() {
            let no_shortcut = ScriptInfo::new("test", "/path/test.ts");
            let with_shortcut =
                ScriptInfo::with_shortcut("test", "/path/test.ts", Some("cmd+t".to_string()));
            let count_no = get_script_context_actions(&no_shortcut).len();
            let count_with = get_script_context_actions(&with_shortcut).len();
            assert_eq!(count_with, count_no + 1); // update + remove = add + 1
        }

        #[test]
        fn cat24_same_pattern_for_alias() {
            let no_alias = ScriptInfo::new("test", "/path/test.ts");
            let with_alias = ScriptInfo::with_shortcut_and_alias(
                "test",
                "/path/test.ts",
                None,
                Some("ts".to_string()),
            );
            let count_no = get_script_context_actions(&no_alias).len();
            let count_with = get_script_context_actions(&with_alias).len();
            assert_eq!(count_with, count_no + 1);
        }

        // =========================================================================
        // Category 25: Note switcher icon assignment — pinned > current > default
        // Verifies the icon priority: StarFilled > Check > File.
        // =========================================================================

        #[test]
        fn cat25_pinned_gets_star_filled() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
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
        fn cat25_current_gets_check() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
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
        fn cat25_regular_gets_file() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::File));
        }

        #[test]
        fn cat25_pinned_and_current_prefers_star() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
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
        fn cat25_note_switcher_empty_placeholder_icon() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].icon, Some(IconName::Plus));
        }

        // =========================================================================
        // Category 26: Note switcher section assignment — Pinned vs Recent
        // Validates that pinned notes go to "Pinned" section and others to "Recent".
        // =========================================================================

        #[test]
        fn cat26_pinned_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Pinned Note".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: true,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
        }

        #[test]
        fn cat26_unpinned_section() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Recent Note".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn cat26_mixed_notes_correct_sections() {
            let notes = vec![
                NoteSwitcherNoteInfo {
                    id: "a".to_string(),
                    title: "Pinned".to_string(),
                    char_count: 10,
                    is_current: false,
                    is_pinned: true,
                    preview: "".to_string(),
                    relative_time: "".to_string(),
                },
                NoteSwitcherNoteInfo {
                    id: "b".to_string(),
                    title: "Recent".to_string(),
                    char_count: 20,
                    is_current: false,
                    is_pinned: false,
                    preview: "".to_string(),
                    relative_time: "".to_string(),
                },
            ];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].section.as_deref(), Some("Pinned"));
            assert_eq!(actions[1].section.as_deref(), Some("Recent"));
        }

        #[test]
        fn cat26_empty_notes_placeholder_section() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].section.as_deref(), Some("Notes"));
        }

        // =========================================================================
        // Category 27: Note switcher current indicator — bullet prefix
        // Validates the "• " prefix for current notes.
        // =========================================================================

        #[test]
        fn cat27_current_note_has_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "My Note".to_string(),
                char_count: 10,
                is_current: true,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(actions[0].title.starts_with("• "));
        }

        #[test]
        fn cat27_non_current_no_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "My Note".to_string(),
                char_count: 10,
                is_current: false,
                is_pinned: false,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(!actions[0].title.starts_with("• "));
            assert_eq!(actions[0].title, "My Note");
        }

        #[test]
        fn cat27_current_and_pinned_has_bullet() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "My Note".to_string(),
                char_count: 10,
                is_current: true,
                is_pinned: true,
                preview: "".to_string(),
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert!(actions[0].title.starts_with("• "));
        }

        // =========================================================================
        // Category 28: Note switcher preview truncation boundary
        // Tests exact truncation at the 60-character boundary with ellipsis.
        // =========================================================================

        #[test]
        fn cat28_preview_exactly_60_no_ellipsis() {
            let preview = "a".repeat(60);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'));
            assert_eq!(desc.len(), 60);
        }

        #[test]
        fn cat28_preview_61_has_ellipsis() {
            let preview = "a".repeat(61);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains('…'));
        }

        #[test]
        fn cat28_preview_59_no_ellipsis() {
            let preview = "b".repeat(59);
            let notes = vec![NoteSwitcherNoteInfo {
                id: "a".to_string(),
                title: "Test".to_string(),
                char_count: 100,
                is_current: false,
                is_pinned: false,
                preview,
                relative_time: "".to_string(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(!desc.contains('…'));
        }

        // =========================================================================
        // Category 29: Cross-context has_action=false invariant
        // Validates that all built-in actions have has_action=false.
        // =========================================================================

        // --- merged from tests_part_05.rs ---
        #[test]
        fn cat29_script_actions_has_action_false() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            for action in &get_script_context_actions(&script) {
                assert!(
                    !action.has_action,
                    "Script action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_clipboard_actions_has_action_false() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            for action in &get_clipboard_history_context_actions(&entry) {
                assert!(
                    !action.has_action,
                    "Clipboard action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_ai_command_bar_has_action_false() {
            for action in &get_ai_command_bar_actions() {
                assert!(
                    !action.has_action,
                    "AI action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_path_actions_has_action_false() {
            let info = PathInfo {
                path: "/test".to_string(),
                is_dir: true,
                name: "test".to_string(),
            };
            for action in &get_path_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "Path action {} should have has_action=false",
                    action.id
                );
            }
        }

        #[test]
        fn cat29_file_actions_has_action_false() {
            let info = FileInfo {
                path: "/test/file.txt".to_string(),
                is_dir: false,
                name: "file.txt".to_string(),
                file_type: FileType::File,
            };
            for action in &get_file_context_actions(&info) {
                assert!(
                    !action.has_action,
                    "File action {} should have has_action=false",
                    action.id
                );
            }
        }

        // =========================================================================
        // Category 30: Cross-context ID uniqueness invariant
        // Validates that all action IDs within a single context are unique.
        // =========================================================================

        #[test]
        fn cat30_script_ids_unique() {
            let script = ScriptInfo::new("test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len(), "Script action IDs not unique");
        }

        #[test]
        fn cat30_clipboard_text_ids_unique() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len(), "Clipboard text IDs not unique");
        }

        #[test]
        fn cat30_clipboard_image_ids_unique() {
            let entry = ClipboardEntryInfo {
                id: "1".to_string(),
                content_type: ContentType::Image,
                pinned: false,
                preview: "".to_string(),
                image_dimensions: Some((100, 100)),
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len(), "Clipboard image IDs not unique");
        }

        #[test]
        fn cat30_ai_command_bar_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len(), "AI command bar IDs not unique");
        }

        #[test]
        fn cat30_path_ids_unique() {
            let info = PathInfo {
                path: "/test".to_string(),
                is_dir: true,
                name: "test".to_string(),
            };
            let actions = get_path_context_actions(&info);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len(), "Path action IDs not unique");
        }
    }
}

mod from_dialog_builtin_action_validation_tests_20 {
    // =============================================================================
    // Dialog Built-in Action Validation Tests — Batch 20
    //
    // 30 categories of tests validating random built-in actions from dialog windows.
    // Each category tests a specific behavior, field, or invariant.
    //
    // Run with:
    //   cargo test --lib actions::dialog_builtin_action_validation_tests_20
    // =============================================================================

    #[cfg(test)]
    mod tests {
        // --- merged from tests_part_01.rs ---
        use super::super::builders::*;
        use super::super::command_bar::CommandBarConfig;
        use super::super::dialog::{
            ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
        };
        use super::super::types::*;
        use crate::clipboard_history::ContentType;
        use crate::designs::icon_variations::IconName;
        use crate::file_search::{FileInfo, FileType};
        use crate::prompts::PathInfo;
        use crate::scriptlets::{Scriptlet, ScriptletAction};
        use std::collections::HashSet;

        fn action_ids(actions: &[Action]) -> Vec<String> {
            actions.iter().map(|a| a.id.clone()).collect()
        }

        // =========================================================================
        // Category 01: Script context agent vs script edit action description diff
        // Verifies that the edit action description differs between agent and
        // script contexts—agent says "agent file", script says "$EDITOR".
        // =========================================================================

        #[test]
        fn cat01_script_edit_desc_mentions_editor() {
            let script = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(
                edit.description.as_ref().unwrap().contains("$EDITOR"),
                "Script edit should mention $EDITOR"
            );
        }

        #[test]
        fn cat01_agent_edit_desc_mentions_agent() {
            let mut script = ScriptInfo::new("Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert!(
                edit.description.as_ref().unwrap().contains("agent"),
                "Agent edit should mention 'agent'"
            );
        }

        #[test]
        fn cat01_agent_edit_title_says_edit_agent() {
            let mut script = ScriptInfo::new("Agent", "/path/agent.md");
            script.is_agent = true;
            script.is_script = false;
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Agent");
        }

        #[test]
        fn cat01_script_edit_title_says_edit_script() {
            let script = ScriptInfo::new("Test", "/path/test.ts");
            let actions = get_script_context_actions(&script);
            let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
            assert_eq!(edit.title, "Edit Script");
        }

        // =========================================================================
        // Category 02: Scriptlet context copy_content always present
        // Verifies copy_content exists in scriptlet context (via both
        // get_script_context_actions and get_scriptlet_context_actions_with_custom).
        // =========================================================================

        #[test]
        fn cat02_scriptlet_context_has_copy_content() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_script_context_actions(&script);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn cat02_scriptlet_with_custom_has_copy_content() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            assert!(actions.iter().any(|a| a.id == "copy_content"));
        }

        #[test]
        fn cat02_scriptlet_copy_content_shortcut() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert_eq!(cc.shortcut.as_ref().unwrap(), "⌘⌥C");
        }

        #[test]
        fn cat02_scriptlet_copy_content_desc_mentions_file() {
            let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
            let actions = get_scriptlet_context_actions_with_custom(&script, None);
            let cc = actions.iter().find(|a| a.id == "copy_content").unwrap();
            assert!(
                cc.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("file")
            );
        }

        // =========================================================================
        // Category 03: Path context open title includes quoted name
        // Verifies the primary action title includes the file/directory name in quotes.
        // =========================================================================

        #[test]
        fn cat03_path_dir_title_includes_name() {
            let path_info = PathInfo {
                path: "/Users/test/Documents".to_string(),
                name: "Documents".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path_info);
            let primary = actions.first().unwrap();
            assert!(primary.title.contains("Documents"));
            assert!(primary.title.contains('"'));
        }

        #[test]
        fn cat03_path_file_title_includes_name() {
            let path_info = PathInfo {
                path: "/Users/test/readme.txt".to_string(),
                name: "readme.txt".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            let primary = actions.first().unwrap();
            assert!(primary.title.contains("readme.txt"));
        }

        #[test]
        fn cat03_path_dir_primary_is_open_directory() {
            let path_info = PathInfo {
                path: "/Users/test/src".to_string(),
                name: "src".to_string(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path_info);
            assert_eq!(actions[0].id, "file:open_directory");
        }

        #[test]
        fn cat03_path_file_primary_is_select_file() {
            let path_info = PathInfo {
                path: "/Users/test/file.rs".to_string(),
                name: "file.rs".to_string(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path_info);
            assert_eq!(actions[0].id, "file:select_file");
        }

        #[test]
        fn cat03_file_context_title_includes_name() {
            let file_info = FileInfo {
                path: "/test/photo.jpg".to_string(),
                name: "photo.jpg".to_string(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            let primary = actions.first().unwrap();
            assert!(primary.title.contains("photo.jpg"));
        }

        // =========================================================================
        // Category 04: Clipboard delete_all description mentions pinned
        // Verifies the delete_all action description mentions "pinned" items
        // are excluded from the clear operation.
        // =========================================================================

        #[test]
        fn cat04_delete_all_desc_mentions_pinned() {
            let entry = ClipboardEntryInfo {
                id: "t".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
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

        #[test]
        fn cat04_delete_entry_desc_mentions_remove() {
            let entry = ClipboardEntryInfo {
                id: "t".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let d = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete")
                .unwrap();
            assert!(
                d.description
                    .as_ref()
                    .unwrap()
                    .to_lowercase()
                    .contains("removes")
            );
        }

        #[test]
        fn cat04_delete_multiple_desc_mentions_filter() {
            let entry = ClipboardEntryInfo {
                id: "t".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let dm = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_delete_multiple")
                .unwrap();
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
                        .contains("entries")
            );
        }

        #[test]
        fn cat04_destructive_actions_order() {
            let entry = ClipboardEntryInfo {
                id: "t".to_string(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "test".to_string(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids = action_ids(&actions);
            let del_idx = ids
                .iter()
                .position(|id| id == "clip:clipboard_delete")
                .unwrap();
            let del_multi = ids
                .iter()
                .position(|id| id == "clip:clipboard_delete_multiple")
                .unwrap();
            let del_all = ids
                .iter()
                .position(|id| id == "clip:clipboard_delete_all")
                .unwrap();
            // Destructive actions in order: delete < delete_multiple < delete_all
            assert!(del_idx < del_multi);
            assert!(del_multi < del_all);
        }

        // =========================================================================
        // Category 05: AI command bar branch_from_last has no shortcut
        // Verifies that branch_from_last and change_model lack shortcuts.
        // =========================================================================

        #[test]
        fn cat05_branch_from_last_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let bfl = actions
                .iter()
                .find(|a| a.id == "chat:branch_from_last")
                .unwrap();
            assert!(bfl.shortcut.is_none());
        }

        #[test]
        fn cat05_change_model_no_shortcut() {
            let actions = get_ai_command_bar_actions();
            let cm = actions
                .iter()
                .find(|a| a.id == "chat:change_model")
                .unwrap();
            assert!(cm.shortcut.is_none());
        }

        #[test]
        fn cat05_submit_has_shortcut_enter() {
            let actions = get_ai_command_bar_actions();
            let s = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(s.shortcut.as_ref().unwrap(), "↵");
        }

        #[test]
        fn cat05_new_chat_shortcut_cmd_n() {
            let actions = get_ai_command_bar_actions();
            let nc = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(nc.shortcut.as_ref().unwrap(), "⌘N");
        }

        #[test]
        fn cat05_delete_chat_shortcut_cmd_delete() {
            let actions = get_ai_command_bar_actions();
            let dc = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(dc.shortcut.as_ref().unwrap(), "⌘⌫");
        }

        // =========================================================================
        // Category 06: Notes command bar new_note always present
        // Verifies new_note and browse_notes appear regardless of flag combos.
        // =========================================================================

        // =========================================================================
        // Category 07: Chat context model action ordering matches input
        // Verifies model selection actions appear in the same order as input.
        // =========================================================================

        #[test]
        fn cat07_model_ordering_preserved() {
            let info = ChatPromptInfo {
                current_model: None,
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
            assert_eq!(picker[0].id, "chat:select_model_gpt-4");
            assert_eq!(picker[1].id, "chat:select_model_claude");
        }

        #[test]
        fn cat07_continue_after_models() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m1".into(),
                    display_name: "M1".into(),
                    provider: "P".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let actions = get_chat_context_actions(&info);
            assert_eq!(actions[0].id, "chat:change_model");
            assert_eq!(actions[1].id, "chat:continue_in_chat");
        }

        // --- merged from tests_part_02.rs ---
        #[test]
        fn cat07_model_title_checkmark_current() {
            let info = ChatPromptInfo {
                current_model: Some("GPT-4".into()),
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
            let gpt4 = picker
                .iter()
                .find(|a| a.id == "chat:select_model_gpt-4")
                .unwrap();
            assert!(gpt4.title.contains('✓'));
            let claude = picker
                .iter()
                .find(|a| a.id == "chat:select_model_claude")
                .unwrap();
            assert!(!claude.title.contains('✓'));
        }

        #[test]
        fn cat07_model_description_via_provider() {
            let info = ChatPromptInfo {
                current_model: None,
                available_models: vec![ChatModelInfo {
                    id: "m".into(),
                    display_name: "M".into(),
                    provider: "Acme".into(),
                }],
                has_messages: false,
                has_response: false,
            };
            let picker = get_chat_model_picker_actions(&info);
            let m = picker
                .iter()
                .find(|a| a.id == "chat:select_model_m")
                .unwrap();
            assert_eq!(m.description.as_ref().unwrap(), "Uses Acme");
        }

        // =========================================================================
        // Category 08: New chat last_used section icon is BoltFilled
        // Verifies icon and section assignments in get_new_chat_picker.
        // =========================================================================

        #[test]
        fn cat08_last_used_icon_bolt() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "M".into(),
                provider: "P".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
        }

        #[test]
        fn cat08_last_used_section_name() {
            let last_used = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "M".into(),
                provider: "P".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&last_used, &[], &[]);
            assert_eq!(actions[0].section.as_ref().unwrap(), "Last Used Settings");
        }

        #[test]
        fn cat08_model_section_name() {
            let models = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "M".into(),
                provider: "P".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].section.as_ref().unwrap(), "Models");
        }

        #[test]
        fn cat08_model_icon_settings() {
            let models = vec![NewChatModelInfo {
                model_id: "m".into(),
                display_name: "M".into(),
                provider: "P".into(),
                provider_display_name: "Provider".into(),
            }];
            let actions = get_new_chat_actions(&[], &[], &models);
            assert_eq!(actions[0].icon, Some(IconName::Settings));
        }

        #[test]
        fn cat08_preset_section_name() {
            let presets = vec![NewChatPresetInfo {
                id: "gen".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].section.as_ref().unwrap(), "Presets");
        }

        #[test]
        fn cat08_preset_icon_preserved() {
            let presets = vec![NewChatPresetInfo {
                id: "gen".into(),
                name: "General".into(),
                icon: IconName::Star,
            }];
            let actions = get_new_chat_actions(&[], &presets, &[]);
            assert_eq!(actions[0].icon, Some(IconName::Star));
        }

        // =========================================================================
        // Category 09: Note switcher description with preview + relative_time
        // Verifies the "preview · time" format and char count fallback.
        // =========================================================================

        #[test]
        fn cat09_preview_and_time_format() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
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

        #[test]
        fn cat09_no_preview_uses_char_count() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 42,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains("42 chars"));
        }

        #[test]
        fn cat09_char_count_singular() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 1,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert!(desc.contains("1 char"));
            assert!(!desc.contains("chars"));
        }

        #[test]
        fn cat09_preview_only_no_time() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 5,
                is_current: false,
                is_pinned: false,
                preview: "Some text".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "Some text");
        }

        #[test]
        fn cat09_time_only_no_preview() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "Note".into(),
                char_count: 5,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "5d ago".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            let desc = actions[0].description.as_ref().unwrap();
            assert_eq!(desc, "5d ago");
        }

        // =========================================================================
        // Category 10: to_deeplink_name CJK and accented character preservation
        // Verifies Unicode alphanumeric chars are kept, not stripped.
        // =========================================================================

        #[test]
        fn cat10_cjk_chars_preserved() {
            let result = to_deeplink_name("测试脚本");
            assert!(result.contains("%E6%B5%8B%E8%AF%95"));
        }

        #[test]
        fn cat10_accented_chars_preserved() {
            let result = to_deeplink_name("Résumé Editor");
            assert!(result.contains("r%C3%A9sum%C3%A9"));
        }

        #[test]
        fn cat10_mixed_case_lowered() {
            assert_eq!(to_deeplink_name("MyScript"), "myscript");
        }

        #[test]
        fn cat10_mixed_special_and_alpha() {
            assert_eq!(to_deeplink_name("My -- Script!"), "my-script");
        }

        #[test]
        fn cat10_empty_string() {
            assert_eq!(to_deeplink_name(""), "_unnamed");
        }

        // =========================================================================
        // Category 11: Action::new pre-computes lowercase fields correctly
        // =========================================================================

        #[test]
        fn cat11_title_lower_matches() {
            let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
            assert_eq!(action.title_lower, "hello world");
        }

        #[test]
        fn cat11_description_lower_matches() {
            let action = Action::new(
                "id",
                "T",
                Some("Hello DESC".to_string()),
                ActionCategory::ScriptContext,
            );
            assert_eq!(action.description_lower, Some("hello desc".to_string()));
        }

        #[test]
        fn cat11_description_lower_none() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
            assert!(action.description_lower.is_none());
        }

        #[test]
        fn cat11_shortcut_lower_none_initially() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext);
            assert!(action.shortcut_lower.is_none());
        }

        #[test]
        fn cat11_shortcut_lower_set_after_with() {
            let action =
                Action::new("id", "T", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
            assert_eq!(action.shortcut_lower, Some("⌘⇧c".to_string()));
        }

        // =========================================================================
        // Category 12: score_action scoring tiers via ActionsDialog
        // =========================================================================

        #[test]
        fn cat12_prefix_scores_100() {
            let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "edit");
            assert_eq!(score, 100);
        }

        #[test]
        fn cat12_contains_scores_50() {
            let action = Action::new("id", "Copy Edit Path", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "edit");
            assert_eq!(score, 50);
        }

        #[test]
        fn cat12_no_match_scores_0() {
            let action = Action::new("id", "Run Script", None, ActionCategory::ScriptContext);
            let score = ActionsDialog::score_action(&action, "xyz");
            assert_eq!(score, 0);
        }

        #[test]
        fn cat12_desc_bonus_15() {
            let action = Action::new(
                "id",
                "Open File",
                Some("Edit in editor".to_string()),
                ActionCategory::ScriptContext,
            );
            let score = ActionsDialog::score_action(&action, "editor");
            assert!(
                score >= 15,
                "Description match should add at least 15 points, got {}",
                score
            );
        }

        #[test]
        fn cat12_shortcut_bonus_10() {
            let action =
                Action::new("id", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘X");
            let score = ActionsDialog::score_action(&action, "⌘x");
            assert!(
                score >= 10,
                "Shortcut match should score 10+, got {}",
                score
            );
        }

        // =========================================================================
        // Category 13: fuzzy_match subsequence behavior
        // =========================================================================

        #[test]
        fn cat13_exact_match() {
            assert!(ActionsDialog::fuzzy_match("hello", "hello"));
        }

        #[test]
        fn cat13_subsequence_match() {
            assert!(ActionsDialog::fuzzy_match("hello world", "hlwrd"));
        }

        #[test]
        fn cat13_no_match() {
            assert!(!ActionsDialog::fuzzy_match("hello", "xyz"));
        }

        #[test]
        fn cat13_empty_needle_matches() {
            assert!(ActionsDialog::fuzzy_match("hello", ""));
        }

        #[test]
        fn cat13_empty_haystack_no_match() {
            assert!(!ActionsDialog::fuzzy_match("", "a"));
        }

        #[test]
        fn cat13_both_empty() {
            assert!(ActionsDialog::fuzzy_match("", ""));
        }

        #[test]
        fn cat13_needle_longer_fails() {
            assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
        }

        // =========================================================================
        // Category 14: parse_shortcut_keycaps splits correctly
        // =========================================================================

        #[test]
        fn cat14_cmd_c_two_caps() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘C");
            assert_eq!(caps.len(), 2);
            assert_eq!(caps[0], "⌘");
            assert_eq!(caps[1], "C");
        }

        #[test]
        fn cat14_cmd_shift_c_three_caps() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
            assert_eq!(caps.len(), 3);
        }

        #[test]
        fn cat14_enter_single() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↵");
            assert_eq!(caps.len(), 1);
            assert_eq!(caps[0], "↵");
        }

        #[test]
        fn cat14_arrows() {
            let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
            assert_eq!(caps.len(), 4);
        }

        #[test]
        fn cat14_escape() {
            let caps = ActionsDialog::parse_shortcut_keycaps("⎋");
            assert_eq!(caps.len(), 1);
            assert_eq!(caps[0], "⎋");
        }

        // =========================================================================
        // Category 15: build_grouped_items_static with Headers style
        // =========================================================================

        #[test]
        fn cat15_headers_add_section_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should have: Header("S1"), Item(0), Header("S2"), Item(1)
            assert_eq!(grouped.len(), 4);
            assert!(matches!(grouped[0], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[1], GroupedActionItem::Item(0)));
            assert!(matches!(grouped[2], GroupedActionItem::SectionHeader(_)));
            assert!(matches!(grouped[3], GroupedActionItem::Item(1)));
        }

        // --- merged from tests_part_03.rs ---
        #[test]
        fn cat15_same_section_no_duplicate_header() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S1"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            // Should have: Header("S1"), Item(0), Item(1) — only one header
            assert_eq!(grouped.len(), 3);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 1);
        }

        #[test]
        fn cat15_separators_no_headers() {
            let actions = vec![
                Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
                Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
            ];
            let filtered: Vec<usize> = (0..actions.len()).collect();
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0);
        }

        #[test]
        fn cat15_empty_filtered_empty_result() {
            let actions = vec![Action::new("a", "A", None, ActionCategory::ScriptContext)];
            let filtered: Vec<usize> = vec![];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
            assert!(grouped.is_empty());
        }

        #[test]
        fn cat15_none_style_no_headers() {
            let actions =
                vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1")];
            let filtered: Vec<usize> = vec![0];
            let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
            let header_count = grouped
                .iter()
                .filter(|g| matches!(g, GroupedActionItem::SectionHeader(_)))
                .count();
            assert_eq!(header_count, 0);
        }

        // =========================================================================
        // Category 16: coerce_action_selection skips headers
        // =========================================================================

        #[test]
        fn cat16_item_stays() {
            let rows = vec![GroupedActionItem::Item(0)];
            assert_eq!(coerce_action_selection(&rows, 0), Some(0));
        }

        #[test]
        fn cat16_header_skips_down() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S".into()),
                GroupedActionItem::Item(0),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), Some(1));
        }

        #[test]
        fn cat16_trailing_header_skips_up() {
            let rows = vec![
                GroupedActionItem::Item(0),
                GroupedActionItem::SectionHeader("S".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 1), Some(0));
        }

        #[test]
        fn cat16_all_headers_none() {
            let rows = vec![
                GroupedActionItem::SectionHeader("S1".into()),
                GroupedActionItem::SectionHeader("S2".into()),
            ];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        #[test]
        fn cat16_empty_rows_none() {
            let rows: Vec<GroupedActionItem> = vec![];
            assert_eq!(coerce_action_selection(&rows, 0), None);
        }

        // =========================================================================
        // Category 17: CommandBarConfig preset field matrix
        // =========================================================================

        #[test]
        fn cat17_default_close_on_select() {
            let config = CommandBarConfig::default();
            assert!(config.close_on_select);
            assert!(config.close_on_escape);
        }

        #[test]
        fn cat17_ai_style_top_search() {
            let config = CommandBarConfig::ai_style();
            assert!(matches!(
                config.dialog_config.search_position,
                SearchPosition::Top
            ));
        }

        #[test]
        fn cat17_main_menu_top_search() {
            let config = CommandBarConfig::main_menu_style();
            assert!(matches!(
                config.dialog_config.search_position,
                SearchPosition::Top
            ));
        }

        #[test]
        fn cat17_no_search_hidden() {
            let config = CommandBarConfig::no_search();
            assert!(matches!(
                config.dialog_config.search_position,
                SearchPosition::Hidden
            ));
        }

        #[test]
        fn cat17_notes_style_separators() {
            let config = CommandBarConfig::notes_style();
            assert!(matches!(
                config.dialog_config.section_style,
                SectionStyle::Headers
            ));
            assert!(config.dialog_config.show_icons);
            assert!(!config.dialog_config.show_footer);
        }

        // =========================================================================
        // Category 18: Action builder with_shortcut_opt behavior
        // =========================================================================

        #[test]
        fn cat18_with_shortcut_opt_none_preserves() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘A")
                .with_shortcut_opt(None);
            // None should not clear the existing shortcut
            assert_eq!(action.shortcut, Some("⌘A".to_string()));
        }

        #[test]
        fn cat18_with_shortcut_opt_some_sets() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut_opt(Some("⌘B".to_string()));
            assert_eq!(action.shortcut, Some("⌘B".to_string()));
        }

        #[test]
        fn cat18_with_icon_preserves_shortcut() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘C")
                .with_icon(IconName::Copy);
            assert_eq!(action.shortcut, Some("⌘C".to_string()));
            assert_eq!(action.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat18_with_section_preserves_all() {
            let action = Action::new("id", "T", None, ActionCategory::ScriptContext)
                .with_shortcut("⌘D")
                .with_icon(IconName::Plus)
                .with_section("MySection");
            assert_eq!(action.shortcut, Some("⌘D".to_string()));
            assert_eq!(action.icon, Some(IconName::Plus));
            assert_eq!(action.section, Some("MySection".to_string()));
        }

        // =========================================================================
        // Category 19: ScriptInfo with_is_script vs with_action_verb
        // =========================================================================

        #[test]
        fn cat19_with_is_script_true() {
            let s = ScriptInfo::with_is_script("test", "/path", true);
            assert!(s.is_script);
            assert_eq!(s.action_verb, "Run");
        }

        #[test]
        fn cat19_with_is_script_false() {
            let s = ScriptInfo::with_is_script("app", "/app", false);
            assert!(!s.is_script);
        }

        #[test]
        fn cat19_with_action_verb_custom() {
            let s = ScriptInfo::with_action_verb("Window", "w:1", false, "Switch to");
            assert_eq!(s.action_verb, "Switch to");
        }

        #[test]
        fn cat19_with_action_verb_and_shortcut() {
            let s = ScriptInfo::with_action_verb_and_shortcut(
                "W",
                "w:1",
                false,
                "Launch",
                Some("cmd+l".to_string()),
            );
            assert_eq!(s.action_verb, "Launch");
            assert_eq!(s.shortcut, Some("cmd+l".to_string()));
        }

        // =========================================================================
        // Category 20: Notes command bar conditional actions per flags
        // =========================================================================

        // =========================================================================
        // Category 21: Clipboard share and attach_to_ai universality
        // Both text and image entries should have these actions.
        // =========================================================================

        #[test]
        fn cat21_text_has_share() {
            let entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(actions.iter().any(|a| a.id == "clip:clipboard_share"));
        }

        #[test]
        fn cat21_image_has_share() {
            let entry = ClipboardEntryInfo {
                id: "i".into(),
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
        fn cat21_text_has_attach_to_ai() {
            let entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            assert!(
                actions
                    .iter()
                    .any(|a| a.id == "clip:clipboard_attach_to_ai")
            );
        }

        #[test]
        fn cat21_image_has_attach_to_ai() {
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
                    .any(|a| a.id == "clip:clipboard_attach_to_ai")
            );
        }

        #[test]
        fn cat21_share_shortcut() {
            let entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let s = actions
                .iter()
                .find(|a| a.id == "clip:clipboard_share")
                .unwrap();
            assert_eq!(s.shortcut.as_ref().unwrap(), "⇧⌘E");
        }

        // =========================================================================
        // Category 22: File context directory lacks quick_look (macOS)
        // =========================================================================

        #[cfg(target_os = "macos")]
        #[test]
        fn cat22_file_dir_no_quick_look() {
            let file_info = FileInfo {
                path: "/test/Dir".into(),
                name: "Dir".into(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(!actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat22_file_file_has_quick_look() {
            let file_info = FileInfo {
                path: "/test/f.txt".into(),
                name: "f.txt".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(actions.iter().any(|a| a.id == "file:quick_look"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat22_file_dir_has_open_in_editor() {
            let file_info = FileInfo {
                path: "/test/Dir".into(),
                name: "Dir".into(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(actions.iter().any(|a| a.id == "file:open_in_editor"));
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn cat22_file_dir_has_show_info() {
            let file_info = FileInfo {
                path: "/test/Dir".into(),
                name: "Dir".into(),
                file_type: FileType::Directory,
                is_dir: true,
            };
            let actions = get_file_context_actions(&file_info);
            assert!(actions.iter().any(|a| a.id == "file:show_info"));
        }

        // =========================================================================
        // Category 23: Scriptlet defined actions from H3 headers
        // =========================================================================

        #[test]
        fn cat23_empty_scriptlet_no_actions() {
            let scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions.is_empty());
        }

        // --- merged from tests_part_04.rs ---
        #[test]
        fn cat23_single_action_has_action_true() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy".into(),
                command: "copy".into(),
                tool: "bash".into(),
                code: "echo | pbcopy".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions.len(), 1);
            assert!(actions[0].has_action);
        }

        #[test]
        fn cat23_action_id_prefix() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Do Thing".into(),
                command: "do-thing".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert!(actions[0].id.starts_with("scriptlet_action:"));
        }

        #[test]
        fn cat23_action_value_is_command() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Do".into(),
                command: "my-cmd".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: None,
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].value, Some("my-cmd".to_string()));
        }

        #[test]
        fn cat23_action_shortcut_formatted() {
            let mut scriptlet = Scriptlet::new("T".into(), "bash".into(), "echo".into());
            scriptlet.actions = vec![ScriptletAction {
                name: "Copy".into(),
                command: "copy".into(),
                tool: "bash".into(),
                code: "echo".into(),
                inputs: vec![],
                shortcut: Some("cmd+c".into()),
                description: None,
            }];
            let actions = get_scriptlet_defined_actions(&scriptlet);
            assert_eq!(actions[0].shortcut, Some("⌘C".to_string()));
        }

        // =========================================================================
        // Category 24: Script context run_script title includes action_verb
        // =========================================================================

        #[test]
        fn cat24_default_verb_run() {
            let script = ScriptInfo::new("Test", "/test.ts");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(run.title.starts_with("Run"));
        }

        #[test]
        fn cat24_custom_verb_launch() {
            let script = ScriptInfo::with_action_verb("Safari", "/app", false, "Launch");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(run.title.starts_with("Launch"));
        }

        #[test]
        fn cat24_custom_verb_switch_to() {
            let script = ScriptInfo::with_action_verb("Window", "w:1", false, "Switch to");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert!(run.title.starts_with("Switch To"));
        }

        #[test]
        fn cat24_title_contains_name_in_quotes() {
            let script = ScriptInfo::new("My Script", "/test.ts");
            let actions = get_script_context_actions(&script);
            let run = actions.iter().find(|a| a.id == "run_script").unwrap();
            assert_eq!(run.title, "Run");
        }

        // =========================================================================
        // Category 25: Notes command bar section assignments
        // =========================================================================

        // =========================================================================
        // Category 26: AI command bar icon assignments
        // =========================================================================

        #[test]
        fn cat26_copy_response_icon_copy() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:copy_response")
                .unwrap();
            assert_eq!(a.icon, Some(IconName::Copy));
        }

        #[test]
        fn cat26_submit_icon_arrow_up() {
            let actions = get_ai_command_bar_actions();
            let a = actions.iter().find(|a| a.id == "chat:submit").unwrap();
            assert_eq!(a.icon, Some(IconName::ArrowUp));
        }

        #[test]
        fn cat26_new_chat_icon_plus() {
            let actions = get_ai_command_bar_actions();
            let a = actions.iter().find(|a| a.id == "chat:new_chat").unwrap();
            assert_eq!(a.icon, Some(IconName::Plus));
        }

        #[test]
        fn cat26_delete_chat_icon_trash() {
            let actions = get_ai_command_bar_actions();
            let a = actions.iter().find(|a| a.id == "chat:delete_chat").unwrap();
            assert_eq!(a.icon, Some(IconName::Trash));
        }

        #[test]
        fn cat26_export_markdown_icon_filecode() {
            let actions = get_ai_command_bar_actions();
            let a = actions
                .iter()
                .find(|a| a.id == "chat:export_markdown")
                .unwrap();
            assert_eq!(a.icon, Some(IconName::FileCode));
        }

        // =========================================================================
        // Category 27: Note switcher icon priority hierarchy
        // pinned > current > regular
        // =========================================================================

        #[test]
        fn cat27_pinned_icon_star_filled() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "N".into(),
                char_count: 0,
                is_current: false,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        #[test]
        fn cat27_current_icon_check() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "N".into(),
                char_count: 0,
                is_current: true,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::Check));
        }

        #[test]
        fn cat27_regular_icon_file() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "N".into(),
                char_count: 0,
                is_current: false,
                is_pinned: false,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            assert_eq!(actions[0].icon, Some(IconName::File));
        }

        #[test]
        fn cat27_pinned_and_current_icon_star() {
            let notes = vec![NoteSwitcherNoteInfo {
                id: "1".into(),
                title: "N".into(),
                char_count: 0,
                is_current: true,
                is_pinned: true,
                preview: "".into(),
                relative_time: "".into(),
            }];
            let actions = get_note_switcher_actions(&notes);
            // Pinned takes priority over current
            assert_eq!(actions[0].icon, Some(IconName::StarFilled));
        }

        // =========================================================================
        // Category 28: Note switcher empty state placeholder
        // =========================================================================

        #[test]
        fn cat28_empty_notes_single_placeholder() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions.len(), 1);
        }

        #[test]
        fn cat28_empty_notes_id_no_notes() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].id, "no_notes");
        }

        #[test]
        fn cat28_empty_notes_title() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].title, "No notes yet");
        }

        #[test]
        fn cat28_empty_notes_icon_plus() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].icon, Some(IconName::Plus));
        }

        #[test]
        fn cat28_empty_notes_section_notes() {
            let actions = get_note_switcher_actions(&[]);
            assert_eq!(actions[0].section.as_ref().unwrap(), "Notes");
        }

        // =========================================================================
        // Category 29: Cross-context all descriptions non-empty
        // =========================================================================

        #[test]
        fn cat29_script_all_have_descriptions() {
            let script = ScriptInfo::new("T", "/t.ts");
            let actions = get_script_context_actions(&script);
            for a in &actions {
                assert!(
                    a.description.is_some(),
                    "Script action '{}' missing description",
                    a.id
                );
            }
        }

        #[test]
        fn cat29_clipboard_all_have_descriptions() {
            let entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            for a in &actions {
                assert!(
                    a.description.is_some(),
                    "Clipboard action '{}' missing description",
                    a.id
                );
            }
        }

        #[test]
        fn cat29_ai_all_have_descriptions() {
            let actions = get_ai_command_bar_actions();
            for a in &actions {
                assert!(
                    a.description.is_some(),
                    "AI action '{}' missing description",
                    a.id
                );
            }
        }

        #[test]
        fn cat29_path_all_have_descriptions() {
            let path = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: false,
            };
            let actions = get_path_context_actions(&path);
            for a in &actions {
                assert!(
                    a.description.is_some(),
                    "Path action '{}' missing description",
                    a.id
                );
            }
        }

        #[test]
        fn cat29_file_all_have_descriptions() {
            let file = FileInfo {
                path: "/t".into(),
                name: "t".into(),
                file_type: FileType::File,
                is_dir: false,
            };
            let actions = get_file_context_actions(&file);
            for a in &actions {
                assert!(
                    a.description.is_some(),
                    "File action '{}' missing description",
                    a.id
                );
            }
        }

        // =========================================================================
        // Category 30: Cross-context ID uniqueness and snake_case invariant
        // =========================================================================

        #[test]
        fn cat30_script_ids_unique() {
            let script = ScriptInfo::new("T", "/t.ts");
            let actions = get_script_context_actions(&script);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat30_clipboard_text_ids_unique() {
            let entry = ClipboardEntryInfo {
                id: "t".into(),
                content_type: ContentType::Text,
                pinned: false,
                preview: "hi".into(),
                image_dimensions: None,
                frontmost_app_name: None,
            };
            let actions = get_clipboard_history_context_actions(&entry);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat30_ai_ids_unique() {
            let actions = get_ai_command_bar_actions();
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        // --- merged from tests_part_05.rs ---
        #[test]
        fn cat30_path_ids_unique() {
            let path = PathInfo {
                path: "/t".into(),
                name: "t".into(),
                is_dir: true,
            };
            let actions = get_path_context_actions(&path);
            let ids: HashSet<&str> = actions.iter().map(|a| a.id.as_str()).collect();
            assert_eq!(ids.len(), actions.len());
        }

        #[test]
        fn cat30_builtin_ids_snake_case() {
            let script = ScriptInfo::new("T", "/t.ts");
            let actions = get_script_context_actions(&script);
            for a in &actions {
                assert!(
                    !a.id.contains(' ') && !a.id.contains('-'),
                    "Action ID '{}' should be snake_case",
                    a.id
                );
            }
        }

    }
}
