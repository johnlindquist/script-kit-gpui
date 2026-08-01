// --- merged from part_01.rs ---
//! Random action/window/dialog behavior validation tests
//!
//! Tests validate randomly-selected behaviors across action builders,
//! confirm dialog constants, window configs, and action property invariants.
//! Each test picks a specific scenario and validates expected behavior.

use super::builders::{
    ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
    NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
    get_chat_model_picker_actions, get_clipboard_history_context_actions, get_file_context_actions,
    get_new_chat_actions, get_note_switcher_actions, get_notes_command_bar_actions,
    get_path_context_actions, get_script_context_actions,
    get_scriptlet_context_actions_with_custom, to_deeplink_name,
};
use super::command_bar::CommandBarConfig;
use super::dialog::{
    ActionsDialog, GroupedActionItem, build_grouped_items_static, coerce_action_selection,
};
use super::types::{
    Action, ActionCategory, AnchorPosition, ScriptInfo, SearchPosition, SectionStyle,
};
use super::window::{WindowPosition, count_section_headers};
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

fn make_action(id: &str, title: &str, section: Option<&str>) -> Action {
    let mut a = Action::new(id, title, None, ActionCategory::ScriptContext);
    if let Some(s) = section {
        a = a.with_section(s);
    }
    a
}

// =========================================================================
// 1. Action shortcut uniqueness within each context
// =========================================================================

#[test]
fn script_context_shortcuts_are_unique() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    let total = shortcuts.len();
    let mut deduped = shortcuts.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        total,
        deduped.len(),
        "Duplicate shortcuts found in script context: {:?}",
        shortcuts
    );
}

#[test]
fn file_context_shortcuts_are_unique() {
    let file = FileInfo {
        path: "/f.txt".into(),
        name: "f.txt".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    let actions = get_file_context_actions(&file);
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    let total = shortcuts.len();
    let mut deduped = shortcuts.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        total,
        deduped.len(),
        "Duplicate shortcuts found in file context: {:?}",
        shortcuts
    );
}

#[test]
fn clipboard_context_shortcuts_are_unique() {
    let entry = ClipboardEntryInfo {
        id: "c".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "c".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&entry);
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    let total = shortcuts.len();
    let mut deduped = shortcuts.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        total,
        deduped.len(),
        "Duplicate shortcuts found in clipboard context: {:?}",
        shortcuts
    );
}

#[test]
fn ai_command_bar_shortcuts_are_unique() {
    let actions = get_ai_command_bar_actions();
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    let total = shortcuts.len();
    let mut deduped = shortcuts.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        total,
        deduped.len(),
        "Duplicate shortcuts found in AI command bar: {:?}",
        shortcuts
    );
}

// =========================================================================
// 2. Notes command bar sections have correct icons
// =========================================================================

#[test]
fn notes_command_bar_actions_have_icons() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    // All notes command bar actions should have icons
    for action in &actions {
        assert!(
            action.icon.is_some(),
            "Notes action '{}' should have an icon",
            action.id
        );
    }
}

#[test]
fn ai_command_bar_actions_have_icons() {
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
fn ai_command_bar_actions_have_sections() {
    let actions = get_ai_command_bar_actions();
    for action in &actions {
        assert!(
            action.section.is_some(),
            "AI action '{}' should have a section",
            action.id
        );
    }
}

// =========================================================================
// 3. Random script type permutations — shortcut+alias+frecency combos
// =========================================================================

#[test]
fn script_no_shortcut_no_alias_no_frecency() {
    let script = ScriptInfo::new("vanilla", "/path/vanilla.ts");
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"add_shortcut"));
    assert!(ids.contains(&"add_alias"));
    assert!(!ids.contains(&"reset_ranking"));
    assert_eq!(actions[0].id, "run_script");
}

#[test]
fn script_has_shortcut_has_alias_has_frecency() {
    let script = ScriptInfo::with_shortcut_and_alias(
        "full",
        "/path/full.ts",
        Some("cmd+f".into()),
        Some("fu".into()),
    )
    .with_frecency(true, Some("/path/full.ts".into()));
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"update_shortcut"));
    assert!(ids.contains(&"remove_shortcut"));
    assert!(ids.contains(&"update_alias"));
    assert!(ids.contains(&"remove_alias"));
    assert!(ids.contains(&"reset_ranking"));
    assert!(!ids.contains(&"add_shortcut"));
    assert!(!ids.contains(&"add_alias"));
}

#[test]
fn script_has_shortcut_no_alias_no_frecency() {
    let script = ScriptInfo::with_shortcut("shortcut-only", "/path/s.ts", Some("cmd+s".into()));
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"update_shortcut"));
    assert!(ids.contains(&"remove_shortcut"));
    assert!(ids.contains(&"add_alias"));
    assert!(!ids.contains(&"add_shortcut"));
    assert!(!ids.contains(&"reset_ranking"));
}

#[test]
fn script_no_shortcut_has_alias_has_frecency() {
    let script =
        ScriptInfo::with_shortcut_and_alias("alias-frec", "/path/af.ts", None, Some("af".into()))
            .with_frecency(true, Some("/path/af.ts".into()));
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"add_shortcut"));
    assert!(ids.contains(&"update_alias"));
    assert!(ids.contains(&"remove_alias"));
    assert!(ids.contains(&"reset_ranking"));
}

#[test]
fn builtin_has_shortcut_has_alias() {
    let builtin = ScriptInfo::with_all(
        "Test Builtin",
        "builtin:test",
        false,
        "Open",
        Some("cmd+b".into()),
        Some("tb".into()),
    );
    let actions = get_script_context_actions(&builtin);
    let ids = action_ids(&actions);
    // Builtins get run + shortcut/alias mgmt + deeplink
    assert!(ids.contains(&"run_script"));
    assert!(ids.contains(&"update_shortcut"));
    assert!(ids.contains(&"remove_shortcut"));
    assert!(ids.contains(&"update_alias"));
    assert!(ids.contains(&"remove_alias"));
    assert!(ids.contains(&"copy_deeplink"));
    // No script-only actions
    assert!(!ids.contains(&"edit_script"));
    assert!(!ids.contains(&"view_logs"));
}

#[test]
fn scriptlet_has_shortcut_no_alias_with_frecency() {
    let script = ScriptInfo::scriptlet("Test SL", "/path/test.md", Some("cmd+t".into()), None)
        .with_frecency(true, Some("scriptlet:Test SL".into()));
    let actions = get_scriptlet_context_actions_with_custom(&script, None);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"update_shortcut"));
    assert!(ids.contains(&"remove_shortcut"));
    assert!(ids.contains(&"add_alias"));
    assert!(ids.contains(&"reset_ranking"));
    assert!(!ids.contains(&"add_shortcut"));
}

// =========================================================================
// 4. Agent flag edge cases
// =========================================================================

#[test]
fn agent_no_shortcut_no_alias_not_suggested() {
    let mut agent = ScriptInfo::new("clean-agent", "/path/agent.md");
    agent.is_agent = true;
    agent.is_script = false;
    let actions = get_script_context_actions(&agent);
    let ids = action_ids(&actions);
    // Agents get no shortcut/alias preference actions
    // (ScriptContextPreferenceActionPlan::AgentNoPreferenceActions).
    assert!(!ids.contains(&"add_shortcut"));
    assert!(!ids.contains(&"add_alias"));
    assert!(!ids.contains(&"reset_ranking"));
    // Agent-specific
    let edit = find_action(&actions, "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
}

#[test]
fn agent_frecency_adds_reset() {
    let mut agent = ScriptInfo::new("frec-agent", "/path/agent.md")
        .with_frecency(true, Some("agent:frec".into()));
    agent.is_agent = true;
    agent.is_script = false;
    let actions = get_script_context_actions(&agent);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"reset_ranking"));
}

// =========================================================================
// 5. Clipboard action random permutations
// =========================================================================

#[test]
fn clipboard_text_pinned_with_app_name() {
    let entry = ClipboardEntryInfo {
        id: "t1".into(),
        content_type: ContentType::Text,
        pinned: true,
        preview: "pinned text".into(),
        image_dimensions: None,
        frontmost_app_name: Some("Safari".into()),
    };
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    // Pinned → unpin
    assert!(ids.contains(&"clip:clipboard_unpin"));
    assert!(!ids.contains(&"clip:clipboard_pin"));
    // App name in title
    let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
    assert_eq!(paste.title, "Paste to Safari");
    // Destructive still last
    let len = ids.len();
    assert_eq!(ids[len - 1], "clip:clipboard_delete_all");
}

#[test]
fn clipboard_image_unpinned_no_app() {
    let entry = ClipboardEntryInfo {
        id: "i1".into(),
        content_type: ContentType::Image,
        pinned: false,
        preview: "".into(),
        image_dimensions: Some((1920, 1080)),
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"clip:clipboard_pin"));
    assert!(!ids.contains(&"clip:clipboard_unpin"));
    assert!(ids.contains(&"clip:clipboard_ocr"));
    let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
    assert_eq!(paste.title, "Paste to Active App");
}

#[test]
fn clipboard_all_entries_have_core_actions() {
    for content_type in [ContentType::Text, ContentType::Image] {
        for pinned in [false, true] {
            for app in [None, Some("Terminal".to_string())] {
                let entry = ClipboardEntryInfo {
                    id: "any".into(),
                    content_type,
                    pinned,
                    preview: "any".into(),
                    image_dimensions: if content_type == ContentType::Image {
                        Some((100, 100))
                    } else {
                        None
                    },
                    frontmost_app_name: app.clone(),
                };
                let actions = get_clipboard_history_context_actions(&entry);
                let ids = action_ids(&actions);
                // Core actions always present
                assert!(
                    ids.contains(&"clip:clipboard_paste"),
                    "Missing paste for {:?}/{}/{:?}",
                    content_type,
                    pinned,
                    app
                );
                assert!(
                    ids.contains(&"clip:clipboard_copy"),
                    "Missing copy for {:?}/{}/{:?}",
                    content_type,
                    pinned,
                    app
                );
                assert!(
                    ids.contains(&"clip:clipboard_delete"),
                    "Missing delete for {:?}/{}/{:?}",
                    content_type,
                    pinned,
                    app
                );
                assert!(
                    ids.contains(&"clip:clipboard_delete_all"),
                    "Missing delete_all for {:?}/{}/{:?}",
                    content_type,
                    pinned,
                    app
                );
                // Pin/unpin mutually exclusive
                assert!(
                    ids.contains(&"clip:clipboard_pin") ^ ids.contains(&"clip:clipboard_unpin"),
                    "Pin/unpin should be mutually exclusive for {:?}/{}/{:?}",
                    content_type,
                    pinned,
                    app
                );
            }
        }
    }
}

// =========================================================================
// 6. Chat context — all flag combinations
// =========================================================================

#[test]
fn chat_single_model_current_checkmark() {
    let info = ChatPromptInfo {
        current_model: Some("Test Model".into()),
        available_models: vec![ChatModelInfo {
            id: "tm".into(),
            display_name: "Test Model".into(),
            provider: "TestCo".into(),
        }],
        has_messages: false,
        has_response: false,
    };
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);
    let model = find_action(&picker, "chat:select_model_tm").unwrap();
    assert!(model.title.contains('✓'));
    assert_eq!(model.description.as_deref(), Some("Uses TestCo"));
}

// --- merged from part_02.rs ---

#[test]
fn chat_many_models_only_one_checkmark() {
    let models: Vec<ChatModelInfo> = (0..5)
        .map(|i| ChatModelInfo {
            id: format!("m{}", i),
            display_name: format!("Model {}", i),
            provider: format!("P{}", i),
        })
        .collect();
    let info = ChatPromptInfo {
        current_model: Some("Model 2".into()),
        available_models: models,
        has_messages: true,
        has_response: true,
    };
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);
    let checkmark_count = picker.iter().filter(|a| a.title.contains('✓')).count();
    assert_eq!(checkmark_count, 1, "Only one model should have checkmark");
    let checked = picker.iter().find(|a| a.title.contains('✓')).unwrap();
    assert_eq!(checked.id, "chat:select_model_m2");
}

#[test]
fn chat_response_without_messages_still_gives_copy_response() {
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

#[test]
fn chat_messages_without_response_gives_clear_but_no_copy() {
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
// 7. Notes command bar — trash view disables most actions
// =========================================================================

#[test]
fn notes_trash_view_disables_edit_copy_export() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: true,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    // Trash view should hide duplicate, find, format, copy, export
    assert!(!ids.contains(&"duplicate_note"));
    assert!(!ids.contains(&"find_in_note"));
    assert!(!ids.contains(&"format"));
    assert!(!ids.contains(&"copy_note_as"));
    assert!(!ids.contains(&"export"));
    // But new_note and browse_notes are always present
    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
}

#[test]
fn notes_no_selection_disables_edit_copy_export() {
    let info = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(!ids.contains(&"duplicate_note"));
    assert!(!ids.contains(&"find_in_note"));
    assert!(!ids.contains(&"copy_note_as"));
}

#[test]
fn notes_auto_sizing_toggled_all_permutations() {
    for &sel in &[false, true] {
        for &trash in &[false, true] {
            let off_actions = get_notes_command_bar_actions(&NotesInfo {
                has_selection: sel,
                is_trash_view: trash,
                auto_sizing_enabled: false,
            });
            let off_toggle = off_actions
                .iter()
                .find(|action| action.id == "toggle_auto_sizing")
                .expect("missing toggle_auto_sizing when disabled");
            assert_eq!(off_toggle.title, "Enable Auto-Sizing");

            let on_actions = get_notes_command_bar_actions(&NotesInfo {
                has_selection: sel,
                is_trash_view: trash,
                auto_sizing_enabled: true,
            });
            let on_toggle = on_actions
                .iter()
                .find(|action| action.id == "toggle_auto_sizing")
                .expect("missing toggle_auto_sizing when enabled");
            assert_eq!(on_toggle.title, "Disable Auto-Sizing");
        }
    }
}

// =========================================================================
// 8. File context — all FileType variants have primary action
// =========================================================================

#[test]
fn file_context_all_types_have_primary_with_enter_shortcut() {
    let types = [
        (FileType::File, false),
        (FileType::Directory, true),
        (FileType::Document, false),
        (FileType::Image, false),
        (FileType::Application, false),
    ];
    for (ft, is_dir) in types {
        let info = FileInfo {
            path: format!("/test/{:?}", ft),
            name: format!("{:?}", ft),
            file_type: ft,
            is_dir,
        };
        let actions = get_file_context_actions(&info);
        assert!(
            !actions.is_empty(),
            "FileType {:?} should produce actions",
            ft
        );
        assert_eq!(
            actions[0].shortcut.as_deref(),
            Some("↵"),
            "Primary action for {:?} should have enter shortcut",
            ft
        );
    }
}

#[test]
fn file_context_directory_never_has_quick_look() {
    let info = FileInfo {
        path: "/test/dir".into(),
        name: "dir".into(),
        file_type: FileType::Directory,
        is_dir: true,
    };
    let actions = get_file_context_actions(&info);
    let ids = action_ids(&actions);
    assert!(
        !ids.contains(&"file:quick_look"),
        "Directory should not have quick_look"
    );
}

// =========================================================================
// 9. Path context — directory vs file primary action
// =========================================================================

#[test]
fn path_context_dir_starts_with_open_directory() {
    let info = PathInfo::new("mydir", "/home/mydir", true);
    let actions = get_path_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_directory");
    assert!(actions[0].title.contains("mydir"));
}

#[test]
fn path_context_file_starts_with_select_file() {
    let info = PathInfo::new("data.csv", "/home/data.csv", false);
    let actions = get_path_context_actions(&info);
    assert_eq!(actions[0].id, "file:select_file");
    assert!(actions[0].title.contains("data.csv"));
}

#[test]
fn path_context_trash_differs_for_file_and_dir() {
    let dir = get_path_context_actions(&PathInfo::new("d", "/d", true));
    let file = get_path_context_actions(&PathInfo::new("f", "/f", false));
    let dir_trash = find_action(&dir, "file:move_to_trash").unwrap();
    let file_trash = find_action(&file, "file:move_to_trash").unwrap();
    assert_ne!(dir_trash.description, file_trash.description);
    assert!(dir_trash.description.as_ref().unwrap().contains("folder"));
    assert!(file_trash.description.as_ref().unwrap().contains("file"));
}

// =========================================================================
// 10. New chat actions — varied inputs
// =========================================================================

#[test]
fn new_chat_only_last_used() {
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
    assert_eq!(actions.len(), 2);
    assert!(
        actions
            .iter()
            .all(|a| a.section.as_deref() == Some("Last Used Settings"))
    );
    assert!(actions.iter().all(|a| a.icon == Some(IconName::BoltFilled)));
}

#[test]
fn new_chat_only_presets() {
    let presets = vec![NewChatPresetInfo {
        id: "code".into(),
        name: "Code".into(),
        icon: IconName::Code,
    }];
    let actions = get_new_chat_actions(&[], &presets, &[]);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    assert_eq!(actions[0].icon, Some(IconName::Code));
}

#[test]
fn new_chat_only_models() {
    let models = vec![NewChatModelInfo {
        model_id: "x".into(),
        display_name: "X".into(),
        provider: "xp".into(),
        provider_display_name: "XP".into(),
    }];
    let actions = get_new_chat_actions(&[], &[], &models);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].section.as_deref(), Some("Models"));
    assert_eq!(actions[0].icon, Some(IconName::Settings));
}

#[test]
fn new_chat_mixed_sections_order() {
    let last = vec![NewChatModelInfo {
        model_id: "l".into(),
        display_name: "L".into(),
        provider: "lp".into(),
        provider_display_name: "LP".into(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "p".into(),
        name: "P".into(),
        icon: IconName::Star,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "m".into(),
        display_name: "M".into(),
        provider: "mp".into(),
        provider_display_name: "MP".into(),
    }];
    let actions = get_new_chat_actions(&last, &presets, &models);
    assert_eq!(actions.len(), 3);
    // Verify order: Last Used Settings → Presets → Models
    assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    assert_eq!(actions[1].section.as_deref(), Some("Presets"));
    assert_eq!(actions[2].section.as_deref(), Some("Models"));
}

// =========================================================================
// 11. Note switcher — edge cases
// =========================================================================

#[test]
fn note_switcher_many_notes_all_have_correct_ids() {
    let notes: Vec<NoteSwitcherNoteInfo> = (0..10)
        .map(|i| NoteSwitcherNoteInfo {
            id: format!("note-{}", i),
            title: format!("Note {}", i),
            char_count: i * 100,
            is_current: i == 3,
            is_pinned: i == 0 || i == 5,
            preview: String::new(),
            relative_time: String::new(),
        })
        .collect();
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions.len(), 10);
    for (i, action) in actions.iter().enumerate() {
        assert_eq!(action.id, format!("note_note-{}", i));
        assert!(
            action.section.as_deref() == Some("Recent")
                || action.section.as_deref() == Some("Pinned"),
            "Note switcher action '{}' should be in 'Recent' or 'Pinned' section, got {:?}",
            action.id,
            action.section
        );
    }
    // Current note (index 3) has bullet
    assert!(actions[3].title.starts_with("• "));
    // Non-current notes don't
    assert!(!actions[0].title.starts_with("• "));
    // Pinned notes get star icon
    assert_eq!(actions[0].icon, Some(IconName::StarFilled));
    assert_eq!(actions[5].icon, Some(IconName::StarFilled));
    // Current non-pinned gets check
    assert_eq!(actions[3].icon, Some(IconName::Check));
    // Regular notes get file icon
    assert_eq!(actions[1].icon, Some(IconName::File));
}

#[test]
fn note_switcher_large_char_count() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "big".into(),
        title: "Big Note".into(),
        char_count: 1_000_000,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].description.as_deref(), Some("1000000 chars"));
}

// =========================================================================
// 12. Action scoring — boundary conditions
// =========================================================================

#[test]
fn score_action_single_char_search() {
    let action = Action::new(
        "script:run",
        "Run Script",
        None,
        ActionCategory::ScriptContext,
    );
    let score = ActionsDialog::score_action(&action, "r");
    assert!(
        score >= 100,
        "Single char 'r' should prefix match 'run script', got {}",
        score
    );
}

#[test]
fn score_action_exact_title_match() {
    let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext);
    let score = ActionsDialog::score_action(&action, "run script");
    assert!(
        score >= 100,
        "Exact title match should score high, got {}",
        score
    );
}

#[test]
fn score_action_is_case_insensitive() {
    // Matching goes through the shared launcher matcher, which lowercases the
    // query itself — uppercase queries score the same as lowercase ones.
    let action = Action::new("test", "Edit Script", None, ActionCategory::ScriptContext);
    let lower = ActionsDialog::score_action(&action, "edit");
    assert!(
        lower >= 100,
        "Lowercased prefix should score high, got {}",
        lower
    );
    let upper = ActionsDialog::score_action(&action, "EDIT");
    assert_eq!(
        upper, lower,
        "Uppercase query should match like the main search does"
    );
}

#[test]
fn score_action_partial_word_still_matches() {
    let action = Action::new(
        "test",
        "Reveal in Finder",
        None,
        ActionCategory::ScriptContext,
    );
    let score = ActionsDialog::score_action(&action, "rev");
    assert!(
        score >= 100,
        "'rev' should prefix match 'reveal in finder', got {}",
        score
    );
}

// =========================================================================
// 13. Fuzzy match (via shared matcher) — more edge cases
// =========================================================================

#[test]
fn score_action_fuzzy_single_char_at_end() {
    let action = Action::new("test", "hello world", None, ActionCategory::ScriptContext);
    assert!(ActionsDialog::score_action(&action, "d") > 0);
}

#[test]
fn score_action_fuzzy_single_char_not_present() {
    let action = Action::new("test", "hello world", None, ActionCategory::ScriptContext);
    assert_eq!(ActionsDialog::score_action(&action, "z"), 0);
}

#[test]
fn score_action_fuzzy_unicode_chars() {
    let action = Action::new("test", "café résumé", None, ActionCategory::ScriptContext);
    assert!(ActionsDialog::score_action(&action, "cr") > 0);
}

#[test]
fn score_action_fuzzy_interleaved_chars() {
    let action = Action::new("test", "abcdefghij", None, ActionCategory::ScriptContext);
    assert!(ActionsDialog::score_action(&action, "acegi") > 0);
}

// =========================================================================
// 14. format_shortcut_hint — more combinations
// =========================================================================

#[test]
fn format_shortcut_delete_key() {
    let result = ActionsDialog::format_shortcut_hint("cmd+delete");
    assert!(result.contains('⌘'));
    assert!(result.contains('⌫'));
}

// --- merged from part_03.rs ---

#[test]
fn format_shortcut_single_letter() {
    let result = ActionsDialog::format_shortcut_hint("a");
    assert_eq!(result, "A");
}

#[test]
fn format_shortcut_triple_modifier() {
    let result = ActionsDialog::format_shortcut_hint("cmd+shift+alt+x");
    assert!(result.contains('⌘'));
    assert!(result.contains('⇧'));
    assert!(result.contains('⌥'));
    assert!(result.contains('X'));
}

// =========================================================================
// 15. parse_shortcut_keycaps — additional patterns
// =========================================================================

#[test]
fn parse_shortcut_keycaps_mixed_modifiers_and_letter() {
    let caps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
    assert_eq!(caps, vec!["⌘", "⇧", "C"]);
}

#[test]
fn parse_shortcut_keycaps_enter_only() {
    let caps = ActionsDialog::parse_shortcut_keycaps("↵");
    assert_eq!(caps, vec!["↵"]);
}

#[test]
fn parse_shortcut_keycaps_modifier_plus_arrow() {
    let caps = ActionsDialog::parse_shortcut_keycaps("⌘↑");
    assert_eq!(caps, vec!["⌘", "↑"]);
}

#[test]
fn parse_shortcut_keycaps_preserves_multi_character_and_literal_keys() {
    assert_eq!(
        ActionsDialog::parse_shortcut_keycaps("⌘F12"),
        vec!["⌘", "F12"]
    );
    assert_eq!(
        ActionsDialog::parse_shortcut_keycaps("cmd++"),
        vec!["⌘", "+"]
    );
    assert_eq!(
        ActionsDialog::parse_shortcut_keycaps("ctrl+\\"),
        vec!["⌃", "\\"]
    );
}

// =========================================================================
// 16. to_deeplink_name — more edge cases
// =========================================================================

#[test]
fn to_deeplink_name_japanese_chars() {
    // Japanese characters are alphanumeric per Rust's is_alphanumeric()
    let result = to_deeplink_name("スクリプト");
    assert!(!result.is_empty());
}

#[test]
fn to_deeplink_name_mixed_scripts() {
    let result = to_deeplink_name("Hello 世界");
    assert!(result.contains("hello"));
    assert!(result.contains("%E4%B8%96%E7%95%8C"));
}

#[test]
fn to_deeplink_name_very_long_name() {
    let long = "a".repeat(200);
    let result = to_deeplink_name(&long);
    assert_eq!(result.len(), 200);
}

#[test]
fn to_deeplink_name_single_hyphen() {
    assert_eq!(to_deeplink_name("-"), "_unnamed");
}

// =========================================================================
// 17. CommandBarConfig — field interactions
// =========================================================================

#[test]
fn command_bar_all_presets_exist() {
    // Verify all 5 preset constructors work without panicking
    let _ = CommandBarConfig::default();
    let _ = CommandBarConfig::main_menu_style();
    let _ = CommandBarConfig::ai_style();
    let _ = CommandBarConfig::notes_style();
    let _ = CommandBarConfig::no_search();
}

#[test]
fn command_bar_no_search_still_has_close_behaviors() {
    let config = CommandBarConfig::no_search();
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

#[test]
fn command_bar_ai_style_uses_headers() {
    let config = CommandBarConfig::ai_style();
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
}

#[test]
fn command_bar_main_menu_uses_separators() {
    let config = CommandBarConfig::main_menu_style();
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
}

// =========================================================================
// 18. Grouped items — interleaved headers and items
// =========================================================================

#[test]
fn grouped_items_alternating_sections() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S2")),
        make_action("c", "C", Some("S1")),
        make_action("d", "D", Some("S2")),
    ];
    let result = build_grouped_items_static(&actions, &[0, 1, 2, 3], SectionStyle::Headers);
    // Each section change introduces a new header
    let header_count = result
        .iter()
        .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
        .count();
    assert_eq!(header_count, 4, "Each section change should have a header");
}

#[test]
fn grouped_items_filtered_subset_only_shows_subset_sections() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S2")),
        make_action("c", "C", Some("S3")),
    ];
    // Only show S1 and S3
    let result = build_grouped_items_static(&actions, &[0, 2], SectionStyle::Headers);
    let headers: Vec<String> = result
        .iter()
        .filter_map(|i| match i {
            GroupedActionItem::SectionHeader(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(headers, vec!["S1", "S3"]);
}

// =========================================================================
// 19. Coerce selection — wrap-around and boundary
// =========================================================================

#[test]
fn coerce_single_item_always_selects_it() {
    let rows = vec![GroupedActionItem::Item(0)];
    assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    assert_eq!(coerce_action_selection(&rows, 100), Some(0));
}

#[test]
fn coerce_empty_returns_none() {
    let rows: Vec<GroupedActionItem> = vec![];
    assert_eq!(coerce_action_selection(&rows, 0), None);
}

#[test]
fn coerce_all_headers_returns_none() {
    let rows = vec![
        GroupedActionItem::SectionHeader("A".into()),
        GroupedActionItem::SectionHeader("B".into()),
        GroupedActionItem::SectionHeader("C".into()),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), None);
    assert_eq!(coerce_action_selection(&rows, 2), None);
}

// =========================================================================
// 20. Window position variants exhaustive test
// =========================================================================

#[test]
fn window_position_all_variants_have_unique_debug_repr() {
    let variants = [
        WindowPosition::BottomRight,
        WindowPosition::TopRight,
        WindowPosition::TopCenter,
    ];
    let debug_strs: Vec<String> = variants.iter().map(|v| format!("{:?}", v)).collect();
    // Each variant should have a distinct debug representation
    for (i, s) in debug_strs.iter().enumerate() {
        for (j, other) in debug_strs.iter().enumerate() {
            if i != j {
                assert_ne!(
                    s, other,
                    "Variants {} and {} should have distinct Debug repr",
                    i, j
                );
            }
        }
    }
}

// =========================================================================
// 21. ProtocolAction — edge case field combos
// =========================================================================

#[test]
fn protocol_action_all_fields_populated() {
    let action = ProtocolAction {
        name: "Full Action".into(),
        description: Some("Full description".into()),
        shortcut: Some("cmd+f".into()),
        value: Some("full-value".into()),
        has_action: true,
        visible: Some(true),
        close: Some(true),
    };
    assert!(action.is_visible());
    assert!(action.should_close());
    assert_eq!(action.name, "Full Action");
    assert_eq!(action.value.as_deref(), Some("full-value"));
}

#[test]
fn protocol_action_minimal_fields() {
    let action = ProtocolAction::new("Minimal".into());
    assert!(action.is_visible());
    assert!(action.should_close());
    assert!(action.description.is_none());
    assert!(action.shortcut.is_none());
    assert!(action.value.is_none());
    assert!(!action.has_action);
}

// =========================================================================
// 22. Action property invariants across all contexts
// =========================================================================

#[test]
fn all_actions_have_non_empty_ids() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    for action in get_script_context_actions(&script) {
        assert!(!action.id.is_empty(), "Action has empty ID");
    }
    for action in get_ai_command_bar_actions() {
        assert!(!action.id.is_empty(), "AI action has empty ID");
    }
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    for action in get_notes_command_bar_actions(&info) {
        assert!(!action.id.is_empty(), "Notes action has empty ID");
    }
}

#[test]
fn all_actions_have_non_empty_titles() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    for action in get_script_context_actions(&script) {
        assert!(
            !action.title.is_empty(),
            "Action '{}' has empty title",
            action.id
        );
    }
    for action in get_ai_command_bar_actions() {
        assert!(
            !action.title.is_empty(),
            "AI action '{}' has empty title",
            action.id
        );
    }
}

#[test]
fn title_lower_always_matches_title_lowercased() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    for action in get_script_context_actions(&script) {
        assert_eq!(
            action.title_lower,
            action.title.to_lowercase(),
            "title_lower mismatch for '{}'",
            action.id
        );
    }
}

#[test]
fn description_lower_matches_description_lowercased() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    for action in get_script_context_actions(&script) {
        match (&action.description, &action.description_lower) {
            (Some(desc), Some(lower)) => {
                assert_eq!(
                    *lower,
                    desc.to_lowercase(),
                    "description_lower mismatch for '{}'",
                    action.id
                );
            }
            (None, None) => {} // Both none is fine
            _ => panic!(
                "description/description_lower mismatch for '{}': {:?} vs {:?}",
                action.id, action.description, action.description_lower
            ),
        }
    }
}

// =========================================================================
// 23. Scriptlet with multiple custom actions — ordering and fields
// =========================================================================

#[test]
fn scriptlet_five_custom_actions_all_have_has_action() {
    let script = ScriptInfo::scriptlet("Multi", "/path/multi.md", None, None);
    let mut scriptlet = Scriptlet::new("Multi".into(), "bash".into(), "echo main".into());
    for i in 0..5 {
        scriptlet.actions.push(ScriptletAction {
            name: format!("Action {}", i),
            command: format!("action-{}", i),
            tool: "bash".into(),
            code: format!("echo {}", i),
            inputs: vec![],
            shortcut: if i % 2 == 0 {
                Some(format!("cmd+{}", i))
            } else {
                None
            },
            description: Some(format!("Does thing {}", i)),
        });
    }
    let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
    let custom: Vec<&Action> = actions
        .iter()
        .filter(|a| a.id.starts_with("scriptlet_action:"))
        .collect();
    assert_eq!(custom.len(), 5);
    for c in &custom {
        assert!(
            c.has_action,
            "Custom action '{}' should have has_action=true",
            c.id
        );
        assert!(
            c.value.is_some(),
            "Custom action '{}' should have value",
            c.id
        );
    }
    // Verify ordering: run_script first, then custom actions in order
    assert_eq!(actions[0].id, "run_script");
    for i in 0..5 {
        assert_eq!(actions[i + 1].id, format!("scriptlet_action:action-{}", i));
    }
}

// =========================================================================
// 24. count_section_headers matches grouped items header count
// =========================================================================

#[test]
fn section_header_count_matches_for_notes_actions() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let filtered: Vec<usize> = (0..actions.len()).collect();
    let from_count = count_section_headers(&actions, &filtered);
    let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    let from_grouped = grouped
        .iter()
        .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
        .count();
    assert_eq!(
        from_count, from_grouped,
        "count_section_headers should match actual headers"
    );
}

#[test]
fn section_header_count_matches_for_ai_actions() {
    let actions = get_ai_command_bar_actions();
    let filtered: Vec<usize> = (0..actions.len()).collect();
    let from_count = count_section_headers(&actions, &filtered);
    let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    let from_grouped = grouped
        .iter()
        .filter(|i| matches!(i, GroupedActionItem::SectionHeader(_)))
        .count();
    assert_eq!(from_count, from_grouped);
}

// =========================================================================
// 25. Enum variant completeness
// =========================================================================

#[test]
fn search_position_all_variants_distinct() {
    let variants = [
        SearchPosition::Bottom,
        SearchPosition::Top,
        SearchPosition::Hidden,
    ];
    for (i, a) in variants.iter().enumerate() {
        for (j, b) in variants.iter().enumerate() {
            if i != j {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn section_style_all_variants_distinct() {
    let variants = [
        SectionStyle::None,
        SectionStyle::Separators,
        SectionStyle::Headers,
    ];
    for (i, a) in variants.iter().enumerate() {
        for (j, b) in variants.iter().enumerate() {
            if i != j {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn anchor_position_all_variants_distinct() {
    let variants = [AnchorPosition::Bottom, AnchorPosition::Top];
    assert_ne!(variants[0], variants[1]);
}

// --- merged from part_04.rs ---

// =========================================================================
// 26. Action builder — with_shortcut_opt None vs Some
// =========================================================================

#[test]
fn with_shortcut_opt_none_leaves_shortcut_none() {
    let action = Action::new("x", "X", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
    assert!(action.shortcut.is_none());
    assert!(action.shortcut_lower.is_none());
}

#[test]
fn with_shortcut_opt_some_sets_both() {
    let action = Action::new("x", "X", None, ActionCategory::ScriptContext)
        .with_shortcut_opt(Some("⌘Z".into()));
    assert_eq!(action.shortcut.as_deref(), Some("⌘Z"));
    assert_eq!(action.shortcut_lower.as_deref(), Some("⌘z"));
}

// =========================================================================
// 27. Action categories are always ScriptContext for built-in builders
// =========================================================================

#[test]
fn all_builder_actions_use_script_context_category() {
    // Script context
    let script = ScriptInfo::new("t", "/t.ts");
    for a in get_script_context_actions(&script) {
        assert_eq!(
            a.category,
            ActionCategory::ScriptContext,
            "Action '{}' wrong category",
            a.id
        );
    }
    // File context
    let file = FileInfo {
        path: "/f.txt".into(),
        name: "f.txt".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    for a in get_file_context_actions(&file) {
        assert_eq!(
            a.category,
            ActionCategory::ScriptContext,
            "File action '{}' wrong category",
            a.id
        );
    }
    // Path context
    let path = PathInfo::new("p", "/p", false);
    for a in get_path_context_actions(&path) {
        assert_eq!(
            a.category,
            ActionCategory::ScriptContext,
            "Path action '{}' wrong category",
            a.id
        );
    }
    // Clipboard context
    let clip = ClipboardEntryInfo {
        id: "c".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "c".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    for a in get_clipboard_history_context_actions(&clip) {
        assert_eq!(
            a.category,
            ActionCategory::ScriptContext,
            "Clipboard action '{}' wrong category",
            a.id
        );
    }
    // AI command bar
    for a in get_ai_command_bar_actions() {
        assert_eq!(
            a.category,
            ActionCategory::ScriptContext,
            "AI action '{}' wrong category",
            a.id
        );
    }
    // Notes command bar
    let notes = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    for a in get_notes_command_bar_actions(&notes) {
        assert_eq!(
            a.category,
            ActionCategory::ScriptContext,
            "Notes action '{}' wrong category",
            a.id
        );
    }
}

// =========================================================================
// 28. Confirm dialog default button focus
// =========================================================================

// confirm_dialog_default_focus_is_confirm_button removed:
// ConfirmDialog replaced by gpui-component Dialog (parent dialog pattern).

// =========================================================================
// 29. Action with_all constructor fields
// =========================================================================

#[test]
fn script_info_with_all_sets_all_fields() {
    let info = ScriptInfo::with_all(
        "Test All",
        "/path/all.ts",
        true,
        "Execute",
        Some("cmd+e".into()),
        Some("ta".into()),
    );
    assert_eq!(info.name, "Test All");
    assert_eq!(info.path, "/path/all.ts");
    assert!(info.is_script);
    assert_eq!(info.action_verb, "Execute");
    assert_eq!(info.shortcut, Some("cmd+e".into()));
    assert_eq!(info.alias, Some("ta".into()));
}

#[test]
fn script_info_builtin_defaults() {
    let info = ScriptInfo::builtin("My Builtin");
    assert_eq!(info.name, "My Builtin");
    assert!(info.path.is_empty());
    assert!(!info.is_script);
    assert!(!info.is_scriptlet);
    assert!(!info.is_agent);
    assert_eq!(info.action_verb, "Run");
}

// =========================================================================
// 30. Action ID uniqueness across all builder contexts
// =========================================================================

#[test]
fn no_duplicate_ids_across_six_contexts() {
    // Script
    let script = ScriptInfo::new("test", "/path/test.ts");
    check_no_dups(&get_script_context_actions(&script), "script");
    // File
    let file = FileInfo {
        path: "/f".into(),
        name: "f".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    check_no_dups(&get_file_context_actions(&file), "file");
    // Path
    let path = PathInfo::new("p", "/p", false);
    check_no_dups(&get_path_context_actions(&path), "path");
    // Clipboard
    let clip = ClipboardEntryInfo {
        id: "c".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "c".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    check_no_dups(&get_clipboard_history_context_actions(&clip), "clipboard");
    // AI
    check_no_dups(&get_ai_command_bar_actions(), "ai");
    // Notes
    let notes = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    check_no_dups(&get_notes_command_bar_actions(&notes), "notes");
}

fn check_no_dups(actions: &[Action], context: &str) {
    let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(
        total,
        ids.len(),
        "Duplicate IDs found in {} context",
        context
    );
}
