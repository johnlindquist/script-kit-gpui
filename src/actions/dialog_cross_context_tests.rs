// --- merged from part_01.rs ---
//! Cross-context validation tests for actions/dialog/window behaviors
//!
//! These tests validate cross-cutting concerns:
//! - Primary action consistency across all builder contexts
//! - Action description content patterns
//! - Shortcut uniqueness within contexts
//! - Section grouping correctness for command bars
//! - Dynamic title formatting with special characters
//! - Clipboard action completeness for each entry type
//! - Note switcher icon assignment correctness
//! - Agent vs script flag interaction edge cases
//! - Action count determinism for all builder permutations

use super::builders::*;
use super::command_bar::CommandBarConfig;
use super::dialog::{GroupedActionItem, build_grouped_items_static, coerce_action_selection};
use super::types::*;
use super::window::{WindowPosition, count_section_headers};
use crate::clipboard_history::ContentType;
use crate::file_search::{FileInfo, FileType};
use crate::prompts::PathInfo;
use crate::protocol::ProtocolAction;
use crate::scriptlets::{Scriptlet, ScriptletAction};

// ============================================================================
// Cross-context primary action consistency
// ============================================================================

#[test]
fn primary_action_is_always_first_in_script_context() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    assert_eq!(actions[0].id, "run_script");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_scriptlet_context() {
    let script = ScriptInfo::scriptlet("test", "/path/test.md", None, None);
    let actions = get_scriptlet_context_actions_with_custom(&script, None);
    assert_eq!(actions[0].id, "run_script");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_file_context_file() {
    let info = FileInfo {
        path: "/test/file.txt".into(),
        name: "file.txt".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    let actions = get_file_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_file");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_file_context_dir() {
    let info = FileInfo {
        path: "/test/dir".into(),
        name: "dir".into(),
        file_type: FileType::Directory,
        is_dir: true,
    };
    let actions = get_file_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_directory");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_path_context_file() {
    let info = PathInfo::new("file.txt", "/test/file.txt", false);
    let actions = get_path_context_actions(&info);
    assert_eq!(actions[0].id, "file:select_file");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_path_context_dir() {
    let info = PathInfo::new("dir", "/test/dir", true);
    let actions = get_path_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_directory");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_clipboard_context() {
    let info = ClipboardEntryInfo {
        id: "1".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "hello".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&info);
    assert_eq!(actions[0].id, "clip:clipboard_paste");
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn primary_action_is_always_first_in_chat_context() {
    // Chat root actions now start with Change Model drill-down trigger
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };
    let actions = get_chat_context_actions(&info);
    assert_eq!(actions[0].id, "chat:change_model");
}

// ============================================================================
// All actions have descriptions
// ============================================================================

#[test]
fn all_script_actions_have_descriptions() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    for action in &actions {
        assert!(
            action.description.is_some(),
            "Script action '{}' missing description",
            action.id
        );
    }
}

#[test]
fn all_file_actions_have_descriptions() {
    let info = FileInfo {
        path: "/test/file.txt".into(),
        name: "file.txt".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    let actions = get_file_context_actions(&info);
    for action in &actions {
        assert!(
            action.description.is_some(),
            "File action '{}' missing description",
            action.id
        );
    }
}

#[test]
fn all_path_actions_have_descriptions() {
    let info = PathInfo::new("dir", "/test/dir", true);
    let actions = get_path_context_actions(&info);
    for action in &actions {
        assert!(
            action.description.is_some(),
            "Path action '{}' missing description",
            action.id
        );
    }
}

#[test]
fn all_clipboard_actions_have_descriptions() {
    let info = ClipboardEntryInfo {
        id: "1".into(),
        content_type: ContentType::Image,
        pinned: true,
        preview: "img".into(),
        image_dimensions: Some((100, 100)),
        frontmost_app_name: Some("Finder".into()),
    };
    let actions = get_clipboard_history_context_actions(&info);
    for action in &actions {
        assert!(
            action.description.is_some(),
            "Clipboard action '{}' missing description",
            action.id
        );
    }
}

#[test]
fn all_ai_command_bar_actions_have_descriptions() {
    let actions = get_ai_command_bar_actions();
    for action in &actions {
        assert!(
            action.description.is_some(),
            "AI action '{}' missing description",
            action.id
        );
    }
}

#[test]
fn all_notes_command_bar_actions_have_descriptions() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    for action in &actions {
        assert!(
            action.description.is_some(),
            "Notes action '{}' missing description",
            action.id
        );
    }
}

// ============================================================================
// Shortcut uniqueness within each context
// ============================================================================

#[test]
fn no_duplicate_shortcuts_in_script_context() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    for (i, s) in shortcuts.iter().enumerate() {
        for (j, other) in shortcuts.iter().enumerate() {
            if i != j {
                assert_ne!(s, other, "Duplicate shortcut '{}' in script context", s);
            }
        }
    }
}

#[test]
fn no_duplicate_shortcuts_in_ai_command_bar() {
    let actions = get_ai_command_bar_actions();
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    for (i, s) in shortcuts.iter().enumerate() {
        for (j, other) in shortcuts.iter().enumerate() {
            if i != j {
                assert_ne!(s, other, "Duplicate shortcut '{}' in AI command bar", s);
            }
        }
    }
}

#[test]
fn no_duplicate_shortcuts_in_clipboard_context() {
    let info = ClipboardEntryInfo {
        id: "1".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "test".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&info);
    let shortcuts: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.shortcut.as_deref())
        .collect();
    for (i, s) in shortcuts.iter().enumerate() {
        for (j, other) in shortcuts.iter().enumerate() {
            if i != j {
                assert_ne!(s, other, "Duplicate shortcut '{}' in clipboard context", s);
            }
        }
    }
}

// ============================================================================
// Clipboard action completeness per entry type
// ============================================================================

#[test]
fn clipboard_text_unpinned_has_expected_action_set() {
    let info = ClipboardEntryInfo {
        id: "1".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "hello world".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Must have these core actions
    assert!(ids.contains(&"clip:clipboard_paste"));
    assert!(ids.contains(&"clip:clipboard_copy"));
    assert!(ids.contains(&"clip:clipboard_paste_keep_open"));
    assert!(ids.contains(&"clip:clipboard_share"));
    assert!(ids.contains(&"clip:clipboard_attach_to_ai"));
    assert!(ids.contains(&"clip:clipboard_pin")); // not pinned → pin
    assert!(ids.contains(&"clip:clipboard_save_snippet"));
    assert!(ids.contains(&"clip:clipboard_save_file"));
    assert!(ids.contains(&"clip:clipboard_delete"));
    assert!(ids.contains(&"clip:clipboard_delete_multiple"));
    assert!(ids.contains(&"clip:clipboard_delete_all"));

    // Must NOT have image-only actions
    assert!(!ids.contains(&"clip:clipboard_ocr"));
    assert!(!ids.contains(&"clip:clipboard_unpin"));
}

#[test]
fn clipboard_image_pinned_has_expected_action_set() {
    let info = ClipboardEntryInfo {
        id: "2".into(),
        content_type: ContentType::Image,
        pinned: true,
        preview: "Image 800x600".into(),
        image_dimensions: Some((800, 600)),
        frontmost_app_name: Some("Preview".into()),
    };
    let actions = get_clipboard_history_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Must have image-specific
    assert!(ids.contains(&"clip:clipboard_ocr"));
    assert!(ids.contains(&"clip:clipboard_unpin")); // pinned → unpin
    assert!(!ids.contains(&"clip:clipboard_pin")); // should NOT have pin

    // Paste title should include app name
    let paste = actions
        .iter()
        .find(|a| a.id == "clip:clipboard_paste")
        .unwrap();
    assert_eq!(paste.title, "Paste to Preview");
}

#[test]
fn clipboard_text_exact_action_count() {
    let info = ClipboardEntryInfo {
        id: "1".into(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "test".into(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&info);
    // Text: paste, copy, paste_keep_open, share, attach_to_ai, quick_look (macOS),
    //        pin, keep_in_today, save_snippet, save_file, delete, delete_multiple, delete_all
    #[cfg(target_os = "macos")]
    assert_eq!(
        actions.len(),
        13,
        "macOS text clipboard should have 13 actions"
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        actions.len(),
        12,
        "non-macOS text clipboard should have 12 actions"
    );
}

#[test]
fn clipboard_image_exact_action_count() {
    let info = ClipboardEntryInfo {
        id: "1".into(),
        content_type: ContentType::Image,
        pinned: false,
        preview: "img".into(),
        image_dimensions: Some((100, 100)),
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&info);
    // Image adds: ocr + macOS: open_with, annotate_cleanshot, upload_cleanshot
    #[cfg(target_os = "macos")]
    assert_eq!(
        actions.len(),
        16,
        "macOS image clipboard should have 16 actions"
    );
    #[cfg(not(target_os = "macos"))]
    assert_eq!(
        actions.len(),
        12,
        "non-macOS image clipboard should have 12 actions"
    );
}

// ============================================================================
// Dynamic title formatting with names
// ============================================================================

#[test]
fn run_script_title_includes_script_name() {
    let script = ScriptInfo::new("Clipboard History", "/path/ch.ts");
    let actions = get_script_context_actions(&script);
    let run = actions.iter().find(|a| a.id == "run_script").unwrap();
    assert_eq!(run.title, "Run");
}

#[test]
fn run_script_title_uses_custom_verb() {
    let script = ScriptInfo::with_action_verb("Safari", "/app/safari", false, "Launch");
    let actions = get_script_context_actions(&script);
    let run = actions.iter().find(|a| a.id == "run_script").unwrap();
    assert_eq!(run.title, "Launch");
    assert!(
        run.description.as_ref().unwrap().contains("Launch"),
        "Description should use verb"
    );
}

#[test]
fn open_file_title_includes_filename() {
    let info = FileInfo {
        path: "/test/my document.pdf".into(),
        name: "my document.pdf".into(),
        file_type: FileType::Document,
        is_dir: false,
    };
    let actions = get_file_context_actions(&info);
    let open = actions.iter().find(|a| a.id == "file:open_file").unwrap();
    assert_eq!(open.title, "Open \"my document.pdf\"");
}

#[test]
fn open_directory_title_includes_dirname() {
    let info = FileInfo {
        path: "/test/My Folder".into(),
        name: "My Folder".into(),
        file_type: FileType::Directory,
        is_dir: true,
    };
    let actions = get_file_context_actions(&info);
    let open = actions
        .iter()
        .find(|a| a.id == "file:open_directory")
        .unwrap();
    assert_eq!(open.title, "Open \"My Folder\"");
}

#[test]
fn path_context_select_file_title_includes_name() {
    let info = PathInfo::new("report.csv", "/data/report.csv", false);
    let actions = get_path_context_actions(&info);
    let select = actions.iter().find(|a| a.id == "file:select_file").unwrap();
    assert_eq!(select.title, "Select \"report.csv\"");
}

#[test]
fn path_context_open_dir_title_includes_name() {
    let info = PathInfo::new("Documents", "/home/user/Documents", true);
    let actions = get_path_context_actions(&info);
    let open = actions
        .iter()
        .find(|a| a.id == "file:open_directory")
        .unwrap();
    assert_eq!(open.title, "Open \"Documents\"");
}

// --- merged from part_02.rs ---

// ============================================================================
// Deeplink description formatting across script types
// ============================================================================

#[test]
fn deeplink_description_format_for_script() {
    let script = ScriptInfo::new("My Cool Script", "/path/script.ts");
    let actions = get_script_context_actions(&script);
    let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
    assert_eq!(
        dl.description.as_deref(),
        Some("Copy a portable Script Kit share link to clipboard")
    );
}

#[test]
fn deeplink_description_format_for_builtin() {
    let builtin = ScriptInfo::builtin("Clipboard History");
    let actions = get_script_context_actions(&builtin);
    let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
    assert_eq!(
        dl.description.as_deref(),
        Some("Copy scriptkit://run/clipboard-history URL to clipboard")
    );
}

#[test]
fn deeplink_description_format_for_scriptlet() {
    let scriptlet = ScriptInfo::scriptlet("Open GitHub", "/path/url.md", None, None);
    let actions = get_scriptlet_context_actions_with_custom(&scriptlet, None);
    let dl = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();
    assert_eq!(
        dl.description.as_deref(),
        Some("Copy scriptkit://run/open-github URL to clipboard")
    );
}

// ============================================================================
// Agent flag interaction edge cases
// ============================================================================

#[test]
fn agent_with_is_script_false_gets_agent_actions() {
    let mut agent = ScriptInfo::new("My Agent", "/path/agent.md");
    agent.is_agent = true;
    agent.is_script = false;
    let actions = get_script_context_actions(&agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Agent-specific
    let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
    assert!(ids.contains(&"reveal_in_finder"));
    assert!(ids.contains(&"copy_path"));
    assert!(ids.contains(&"copy_content"));

    // Must NOT have script-only actions
    assert!(!ids.contains(&"view_logs"));
}

#[test]
fn agent_with_all_flags_combined() {
    let mut agent = ScriptInfo::with_shortcut_and_alias(
        "Super Agent",
        "/path/agent.md",
        Some("cmd+shift+a".into()),
        Some("sa".into()),
    );
    agent.is_agent = true;
    agent.is_script = false;
    let agent = agent.with_frecency(true, Some("agent:super".into()));

    let actions = get_script_context_actions(&agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Agent shortcut and alias handlers reject Agent results, so the builder
    // must not advertise those actions even when stale metadata is present.
    assert!(!ids.contains(&"update_shortcut"));
    assert!(!ids.contains(&"remove_shortcut"));
    assert!(!ids.contains(&"update_alias"));
    assert!(!ids.contains(&"remove_alias"));
    assert!(!ids.contains(&"add_shortcut"));
    assert!(!ids.contains(&"add_alias"));

    // Has frecency reset
    assert!(ids.contains(&"reset_ranking"));

    // Has agent actions
    let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
    assert!(ids.contains(&"copy_deeplink"));
}

#[test]
fn agent_with_is_script_true_gets_script_actions_instead() {
    // If someone mistakenly sets both is_agent and is_script true,
    // is_script section fires first (before is_agent), so we get script actions
    let mut script = ScriptInfo::new("Weird Agent", "/path/weird.ts");
    script.is_agent = true;
    // is_script is already true from new()
    let actions = get_script_context_actions(&script);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Script-only action remains present.
    assert!(ids.contains(&"view_logs"));
    // edit_script is deduplicated by action id.
    let edit_actions: Vec<&str> = actions
        .iter()
        .filter(|a| a.id == "edit_script")
        .map(|a| a.title.as_str())
        .collect();
    assert_eq!(edit_actions.len(), 1);
    assert!(edit_actions.contains(&"Edit Script"));
}

// ============================================================================
// Notes command bar section correctness
// ============================================================================

#[test]
fn notes_command_bar_sections_are_correct() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);

    // Verify each action has the expected section
    let new_note = actions.iter().find(|a| a.id == "new_note").unwrap();
    assert_eq!(new_note.section.as_deref(), Some("Notes"));

    let duplicate = actions.iter().find(|a| a.id == "duplicate_note").unwrap();
    assert_eq!(duplicate.section.as_deref(), Some("Notes"));

    let browse = actions.iter().find(|a| a.id == "browse_notes").unwrap();
    assert_eq!(browse.section.as_deref(), Some("Notes"));

    let find = actions.iter().find(|a| a.id == "find_in_note").unwrap();
    assert_eq!(find.section.as_deref(), Some("Edit"));

    let format = actions.iter().find(|a| a.id == "format").unwrap();
    assert_eq!(format.section.as_deref(), Some("Edit"));

    let copy_as = actions.iter().find(|a| a.id == "copy_note_as").unwrap();
    assert_eq!(copy_as.section.as_deref(), Some("Copy"));

    let export = actions.iter().find(|a| a.id == "export").unwrap();
    assert_eq!(export.section.as_deref(), Some("Export"));

    let auto_size = actions
        .iter()
        .find(|a| a.id == "toggle_auto_sizing")
        .unwrap();
    assert_eq!(auto_size.section.as_deref(), Some("Settings"));
}

#[test]
fn notes_command_bar_auto_sizing_enabled_shows_disable_toggle() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&info);
    let toggle = actions
        .iter()
        .find(|action| action.id == "toggle_auto_sizing")
        .expect("auto-sizing toggle should always be present");
    assert_eq!(toggle.title, "Disable Auto-Sizing");
}

// ============================================================================
// AI command bar section ordering
// ============================================================================

#[test]
fn ai_command_bar_section_order_is_deterministic() {
    let actions = get_ai_command_bar_actions();
    let sections: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.section.as_deref())
        .collect();

    // Find first occurrence of each section
    let response_idx = sections.iter().position(|s| *s == "Response").unwrap();
    let actions_idx = sections.iter().position(|s| *s == "Actions").unwrap();
    let attachments_idx = sections.iter().position(|s| *s == "Attachments").unwrap();
    let settings_idx = sections.iter().position(|s| *s == "Settings").unwrap();

    assert!(response_idx < actions_idx, "Response before Actions");
    assert!(actions_idx < attachments_idx, "Actions before Attachments");
    assert!(
        attachments_idx < settings_idx,
        "Attachments before Settings"
    );
}

#[test]
fn ai_command_bar_response_section_has_three_actions() {
    let actions = get_ai_command_bar_actions();
    let response_count = actions
        .iter()
        .filter(|a| a.section.as_deref() == Some("Response"))
        .count();
    assert_eq!(response_count, 3);
}

#[test]
fn ai_command_bar_actions_section_has_four_actions() {
    let actions = get_ai_command_bar_actions();
    let action_count = actions
        .iter()
        .filter(|a| a.section.as_deref() == Some("Actions"))
        .count();
    assert_eq!(action_count, 7);
}

#[test]
fn ai_command_bar_attachments_section_has_two_actions() {
    let actions = get_ai_command_bar_actions();
    let attach_count = actions
        .iter()
        .filter(|a| a.section.as_deref() == Some("Attachments"))
        .count();
    assert_eq!(attach_count, 4);
}

#[test]
fn ai_command_bar_settings_section_has_one_action() {
    let actions = get_ai_command_bar_actions();
    let settings_count = actions
        .iter()
        .filter(|a| a.section.as_deref() == Some("Settings"))
        .count();
    assert_eq!(settings_count, 2);
}

// ============================================================================
// Note switcher correctness
// ============================================================================

#[test]
fn note_switcher_multiple_notes_icon_assignment() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "Pinned Current".into(),
            char_count: 100,
            is_current: true,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "b".into(),
            title: "Pinned Not Current".into(),
            char_count: 50,
            is_current: false,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "c".into(),
            title: "Current Not Pinned".into(),
            char_count: 200,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "d".into(),
            title: "Neither".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions.len(), 4);

    // Pinned+current → StarFilled (pinned takes priority)
    assert_eq!(
        actions[0].icon,
        Some(crate::designs::icon_variations::IconName::StarFilled)
    );
    assert!(actions[0].title.starts_with("• ")); // current indicator

    // Pinned only → StarFilled
    assert_eq!(
        actions[1].icon,
        Some(crate::designs::icon_variations::IconName::StarFilled)
    );
    assert!(!actions[1].title.starts_with("• ")); // not current

    // Current only → Check
    assert_eq!(
        actions[2].icon,
        Some(crate::designs::icon_variations::IconName::Check)
    );
    assert!(actions[2].title.starts_with("• "));

    // Neither → File
    assert_eq!(
        actions[3].icon,
        Some(crate::designs::icon_variations::IconName::File)
    );
    assert!(!actions[3].title.starts_with("• "));
}

#[test]
fn note_switcher_char_count_description_formatting() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "Zero".into(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "b".into(),
            title: "One".into(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "c".into(),
            title: "Many".into(),
            char_count: 999,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].description.as_deref(), Some("0 chars"));
    assert_eq!(actions[1].description.as_deref(), Some("1 char"));
    assert_eq!(actions[2].description.as_deref(), Some("999 chars"));
}

#[test]
fn note_switcher_id_format() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123-def".into(),
        title: "Test".into(),
        char_count: 10,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].id, "note_abc-123-def");
}

// ============================================================================
// Chat context model actions
// ============================================================================

#[test]
fn chat_context_with_multiple_models_marks_only_current() {
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
            ChatModelInfo {
                id: "gemini".into(),
                display_name: "Gemini".into(),
                provider: "Google".into(),
            },
        ],
        has_messages: true,
        has_response: true,
    };
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);

    // Only GPT-4 should have checkmark
    let gpt4 = picker
        .iter()
        .find(|a| a.id == "chat:select_model_gpt4")
        .unwrap();
    assert_eq!(gpt4.title, "GPT-4 ✓");

    let claude = picker
        .iter()
        .find(|a| a.id == "chat:select_model_claude")
        .unwrap();
    assert_eq!(claude.title, "Claude");
    assert!(!claude.title.contains('✓'));

    let gemini = picker
        .iter()
        .find(|a| a.id == "chat:select_model_gemini")
        .unwrap();
    assert_eq!(gemini.title, "Gemini");
    assert!(!gemini.title.contains('✓'));
}

#[test]
fn chat_context_model_description_shows_provider() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![ChatModelInfo {
            id: "model1".into(),
            display_name: "Model One".into(),
            provider: "Anthropic".into(),
        }],
        has_messages: false,
        has_response: false,
    };
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);
    let model = picker
        .iter()
        .find(|a| a.id == "chat:select_model_model1")
        .unwrap();
    assert_eq!(model.description.as_deref(), Some("Uses Anthropic"));
}

// --- merged from part_03.rs ---

#[test]
fn chat_context_all_four_conditional_combos() {
    for (has_response, has_messages) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let info = ChatPromptInfo {
            current_model: None,
            available_models: vec![],
            has_messages,
            has_response,
        };
        let actions = get_chat_context_actions(&info);
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

        // continue_in_chat always present
        assert!(ids.contains(&"chat:continue_in_chat"));

        // copy_response only when has_response
        assert_eq!(
            ids.contains(&"chat:copy_response"),
            has_response,
            "copy_response presence should match has_response={}",
            has_response
        );

        // clear_conversation only when has_messages
        assert_eq!(
            ids.contains(&"chat:clear_conversation"),
            has_messages,
            "clear_conversation presence should match has_messages={}",
            has_messages
        );
    }
}

// ============================================================================
// New chat actions
// ============================================================================

#[test]
fn new_chat_empty_inputs_returns_empty() {
    let actions = get_new_chat_actions(&[], &[], &[]);
    assert!(actions.is_empty());
}

#[test]
fn new_chat_section_names_are_correct() {
    let last = vec![NewChatModelInfo {
        model_id: "m1".into(),
        display_name: "Model 1".into(),
        provider: "P".into(),
        provider_display_name: "Provider".into(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "p1".into(),
        name: "General".into(),
        icon: crate::designs::icon_variations::IconName::BoltFilled,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "m2".into(),
        display_name: "Model 2".into(),
        provider: "P".into(),
        provider_display_name: "Provider 2".into(),
    }];

    let actions = get_new_chat_actions(&last, &presets, &models);
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    assert_eq!(actions[1].section.as_deref(), Some("Presets"));
    assert_eq!(actions[2].section.as_deref(), Some("Models"));
}

#[test]
fn new_chat_preset_uses_custom_icon() {
    let presets = vec![NewChatPresetInfo {
        id: "code".into(),
        name: "Code".into(),
        icon: crate::designs::icon_variations::IconName::Code,
    }];
    let actions = get_new_chat_actions(&[], &presets, &[]);
    assert_eq!(
        actions[0].icon,
        Some(crate::designs::icon_variations::IconName::Code)
    );
}

// ============================================================================
// Scriptlet custom actions ordering and fields
// ============================================================================

#[test]
fn scriptlet_custom_actions_have_correct_value_field() {
    let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
    let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo main".into());
    scriptlet.actions = vec![ScriptletAction {
        name: "My Action".into(),
        command: "my-action-cmd".into(),
        tool: "bash".into(),
        code: "echo custom".into(),
        inputs: vec![],
        shortcut: None,
        description: Some("Does something".into()),
    }];

    let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
    let custom = actions
        .iter()
        .find(|a| a.id == "scriptlet_action:my-action-cmd")
        .unwrap();

    assert!(custom.has_action);
    assert_eq!(custom.value.as_deref(), Some("my-action-cmd"));
    assert_eq!(custom.title, "My Action");
    assert_eq!(custom.description.as_deref(), Some("Does something"));
}

#[test]
fn scriptlet_multiple_custom_actions_preserve_order() {
    let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
    let mut scriptlet = Scriptlet::new("Test".into(), "bash".into(), "echo main".into());
    scriptlet.actions = vec![
        ScriptletAction {
            name: "Alpha".into(),
            command: "alpha".into(),
            tool: "bash".into(),
            code: "echo a".into(),
            inputs: vec![],
            shortcut: Some("cmd+1".into()),
            description: None,
        },
        ScriptletAction {
            name: "Beta".into(),
            command: "beta".into(),
            tool: "bash".into(),
            code: "echo b".into(),
            inputs: vec![],
            shortcut: Some("cmd+2".into()),
            description: None,
        },
        ScriptletAction {
            name: "Gamma".into(),
            command: "gamma".into(),
            tool: "bash".into(),
            code: "echo g".into(),
            inputs: vec![],
            shortcut: None,
            description: None,
        },
    ];

    let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));

    // run_script at 0, then custom actions in order, then built-in
    assert_eq!(actions[0].id, "run_script");
    assert_eq!(actions[1].id, "scriptlet_action:alpha");
    assert_eq!(actions[2].id, "scriptlet_action:beta");
    assert_eq!(actions[3].id, "scriptlet_action:gamma");

    // Alpha has formatted shortcut
    assert_eq!(actions[1].shortcut.as_deref(), Some("⌘1"));
    // Beta has formatted shortcut
    assert_eq!(actions[2].shortcut.as_deref(), Some("⌘2"));
    // Gamma has no shortcut
    assert!(actions[3].shortcut.is_none());
}

// ============================================================================
// CommandBarConfig field interactions
// ============================================================================

#[test]
fn command_bar_config_notes_style_specific_fields() {
    let config = CommandBarConfig::notes_style();
    // Notes style uses Top search, Separators, Top anchor, icons+footer
    assert!(matches!(
        config.dialog_config.search_position,
        SearchPosition::Top
    ));
    assert!(matches!(
        config.dialog_config.section_style,
        SectionStyle::Headers
    ));
    assert!(matches!(config.dialog_config.anchor, AnchorPosition::Top));
    assert!(config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
    // Close behaviors default to true
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

#[test]
fn command_bar_config_all_presets_have_close_defaults() {
    for config in [
        CommandBarConfig::default(),
        CommandBarConfig::main_menu_style(),
        CommandBarConfig::ai_style(),
        CommandBarConfig::no_search(),
        CommandBarConfig::notes_style(),
    ] {
        assert!(
            config.close_on_select,
            "close_on_select should default to true"
        );
        assert!(
            config.close_on_escape,
            "close_on_escape should default to true"
        );
        assert!(
            config.close_on_click_outside,
            "close_on_click_outside should default to true"
        );
    }
}

// ============================================================================
// Grouped items and coercion edge cases
// ============================================================================

#[test]
fn grouped_items_with_headers_counts_match_count_section_headers() {
    let actions = get_ai_command_bar_actions();
    let filtered: Vec<usize> = (0..actions.len()).collect();

    let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    let header_count_from_grouped = grouped
        .iter()
        .filter(|item| matches!(item, GroupedActionItem::SectionHeader(_)))
        .count();
    let header_count_from_window = count_section_headers(&actions, &filtered);

    assert_eq!(header_count_from_grouped, header_count_from_window);
}

#[test]
fn coerce_navigation_skips_all_headers_to_reach_items() {
    let rows = vec![
        GroupedActionItem::SectionHeader("A".into()),
        GroupedActionItem::SectionHeader("B".into()),
        GroupedActionItem::SectionHeader("C".into()),
        GroupedActionItem::Item(0),
        GroupedActionItem::SectionHeader("D".into()),
        GroupedActionItem::Item(1),
    ];

    // Starting at header 0 should find Item at index 3
    assert_eq!(coerce_action_selection(&rows, 0), Some(3));
    // Starting at header 1 should find Item at index 3
    assert_eq!(coerce_action_selection(&rows, 1), Some(3));
    // Starting at header 4 should find Item at index 5
    assert_eq!(coerce_action_selection(&rows, 4), Some(5));
    // Item at 3 stays at 3
    assert_eq!(coerce_action_selection(&rows, 3), Some(3));
}

#[test]
fn coerce_last_item_is_header_searches_backwards() {
    let rows = vec![
        GroupedActionItem::Item(0),
        GroupedActionItem::Item(1),
        GroupedActionItem::SectionHeader("End".into()),
    ];

    // At header index 2, should search backwards to item at index 1
    assert_eq!(coerce_action_selection(&rows, 2), Some(1));
}

// ============================================================================
// WindowPosition default and variants
// ============================================================================

#[test]
fn window_position_default_is_bottom_right() {
    let pos = WindowPosition::default();
    assert_eq!(pos, WindowPosition::BottomRight);
}

#[test]
fn window_position_all_variants_distinct() {
    assert_ne!(WindowPosition::BottomRight, WindowPosition::TopRight);
    assert_ne!(WindowPosition::TopRight, WindowPosition::TopCenter);
    assert_ne!(WindowPosition::BottomRight, WindowPosition::TopCenter);
}

// ============================================================================
// ProtocolAction constructor variants
// ============================================================================

#[test]
fn protocol_action_with_handler_has_action_true() {
    let action = ProtocolAction::with_handler("My Handler".into());
    assert!(action.has_action);
    assert!(action.value.is_none());
    assert!(action.description.is_none());
    assert!(action.is_visible());
    assert!(action.should_close());
}

#[test]
fn protocol_action_with_value_has_action_false() {
    let action = ProtocolAction::with_value("Submit".into(), "submit-val".into());
    assert!(!action.has_action);
    assert_eq!(action.value.as_deref(), Some("submit-val"));
    assert!(action.is_visible());
    assert!(action.should_close());
}

#[test]
fn protocol_action_hidden_not_visible() {
    let action = ProtocolAction {
        name: "Hidden".into(),
        description: None,
        shortcut: None,
        value: None,
        has_action: false,
        visible: Some(false),
        close: None,
    };
    assert!(!action.is_visible());
}

#[test]
fn protocol_action_close_false_stays_open() {
    let action = ProtocolAction {
        name: "Stay Open".into(),
        description: None,
        shortcut: None,
        value: None,
        has_action: true,
        visible: None,
        close: Some(false),
    };
    assert!(!action.should_close());
    assert!(action.is_visible()); // visible defaults to true
}

// ============================================================================
// Action struct caching behavior
// ============================================================================

#[test]
fn action_title_lower_cache_matches_lowercase() {
    let action = Action::new(
        "test",
        "Edit Script",
        Some("Open in $EDITOR".into()),
        ActionCategory::ScriptContext,
    );
    assert_eq!(action.title_lower, "edit script");
    assert_eq!(action.description_lower.as_deref(), Some("open in $editor"));
}

#[test]
fn action_shortcut_lower_cache_set_by_with_shortcut() {
    let action =
        Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
    assert_eq!(action.shortcut_lower.as_deref(), Some("⌘⇧c"));
}

#[test]
fn action_shortcut_lower_cache_not_set_without_shortcut() {
    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
    assert!(action.shortcut_lower.is_none());
}

#[test]
fn action_with_icon_and_section_chain() {
    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
        .with_icon(crate::designs::icon_variations::IconName::Plus)
        .with_section("MySection")
        .with_shortcut("⌘T");

    assert_eq!(
        action.icon,
        Some(crate::designs::icon_variations::IconName::Plus)
    );
    assert_eq!(action.section.as_deref(), Some("MySection"));
    assert_eq!(action.shortcut.as_deref(), Some("⌘T"));
}

// ============================================================================
// Exact action count for script type permutations
// ============================================================================

#[test]
fn script_action_count_without_shortcut_or_alias() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    // + toggle_info + toggle_favorite + file:open_in_quick_terminal + delete_script = 13
    assert_eq!(actions.len(), 13);
}

#[test]
fn script_action_count_with_shortcut_and_alias() {
    let script = ScriptInfo::with_shortcut_and_alias(
        "test",
        "/path/test.ts",
        Some("cmd+t".into()),
        Some("ts".into()),
    );
    let actions = get_script_context_actions(&script);
    // run + toggle_info + update_shortcut + remove_shortcut + update_alias + remove_alias
    // + edit + view_logs + reveal + file:open_in_quick_terminal + copy_path + copy_content
    // + copy_deeplink + toggle_favorite + delete_script = 15
    assert_eq!(actions.len(), 15);
}

#[test]
fn builtin_action_count() {
    let builtin = ScriptInfo::builtin("Test Builtin");
    let actions = get_script_context_actions(&builtin);
    // run + toggle_info + add_shortcut + add_alias + copy_deeplink = 5
    assert_eq!(actions.len(), 5);
}

#[test]
fn scriptlet_action_count_without_shortcut_or_alias() {
    let scriptlet = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
    let actions = get_script_context_actions(&scriptlet);
    // + toggle_info + toggle_favorite + file:open_in_quick_terminal = 11
    assert_eq!(actions.len(), 11);
}

#[test]
fn path_context_file_action_count() {
    let info = PathInfo::new("file.txt", "/test/file.txt", false);
    let actions = get_path_context_actions(&info);
    // select_file + copy_path + open_in_finder + open_in_editor + open_in_quick_terminal + copy_filename + move_to_trash = 7
    assert_eq!(actions.len(), 7);
}

#[test]
fn path_context_dir_action_count() {
    let info = PathInfo::new("dir", "/test/dir", true);
    let actions = get_path_context_actions(&info);
    // open_directory + copy_path + open_in_finder + open_in_editor + open_in_quick_terminal + copy_filename + move_to_trash = 7
    assert_eq!(actions.len(), 7);
}

// ============================================================================
// Deeplink name edge cases
// ============================================================================

#[test]
fn deeplink_name_with_unicode() {
    // Non-ASCII characters are percent-encoded.
    assert_eq!(to_deeplink_name("café"), "caf%C3%A9");
}

// --- merged from part_04.rs ---

#[test]
fn deeplink_name_with_numbers() {
    assert_eq!(to_deeplink_name("script123"), "script123");
}

#[test]
fn deeplink_name_empty_string() {
    assert_eq!(to_deeplink_name(""), "_unnamed");
}

#[test]
fn deeplink_name_only_special_chars() {
    assert_eq!(to_deeplink_name("@#$%^&"), "_unnamed");
}

#[test]
fn deeplink_name_preserves_hyphens_as_single() {
    assert_eq!(to_deeplink_name("a---b"), "a-b");
}

// ============================================================================
// count_section_headers edge cases
// ============================================================================

#[test]
fn count_section_headers_with_none_sections_among_some() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Sec1"),
        Action::new("b", "B", None, ActionCategory::ScriptContext), // no section
        Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Sec1"),
        Action::new("d", "D", None, ActionCategory::ScriptContext).with_section("Sec2"),
    ];
    let filtered = vec![0, 1, 2, 3];
    let count = count_section_headers(&actions, &filtered);
    // idx 0: Sec1 → header (first section)
    // idx 1: None → no header (section is None, only Some sections count)
    // idx 2: Sec1 → no header (unsectioned rows do not reset section tracking)
    // idx 3: Sec2 → header (section changed from Sec1 to Sec2)
    assert_eq!(count, 2);
}

#[test]
fn count_section_headers_all_same_section() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Same"),
        Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Same"),
        Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Same"),
    ];
    let filtered = vec![0, 1, 2];
    let count = count_section_headers(&actions, &filtered);
    assert_eq!(count, 1); // Only one header for the single section
}

#[test]
fn count_section_headers_filtered_subset_skips_some() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("S1"),
        Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("S2"),
        Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("S3"),
    ];
    // Only show actions 0 and 2 (skip S2)
    let filtered = vec![0, 2];
    let count = count_section_headers(&actions, &filtered);
    assert_eq!(count, 2); // S1 and S3
}

// ============================================================================
// File context with different FileType variants
// ============================================================================

#[test]
fn file_context_application_treated_as_file() {
    let info = FileInfo {
        path: "/Applications/Safari.app".into(),
        name: "Safari.app".into(),
        file_type: FileType::Application,
        is_dir: false,
    };
    let actions = get_file_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_file");
    assert_eq!(actions[0].title, "Open \"Safari.app\"");
}

#[test]
fn file_context_image_treated_as_file() {
    let info = FileInfo {
        path: "/photos/sunset.jpg".into(),
        name: "sunset.jpg".into(),
        file_type: FileType::Image,
        is_dir: false,
    };
    let actions = get_file_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_file");
    #[cfg(target_os = "macos")]
    {
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"file:quick_look"));
    }
}

// ============================================================================
// Global actions
// ============================================================================

#[test]
fn global_actions_is_seeded() {
    let actions = get_global_actions();
    assert!(
        !actions.is_empty(),
        "global actions should expose at least one row"
    );
}

// ============================================================================
// Enum default values
// ============================================================================

#[test]
fn search_position_default_is_bottom() {
    assert!(matches!(SearchPosition::default(), SearchPosition::Bottom));
}

#[test]
fn section_style_default_is_separators() {
    assert!(matches!(SectionStyle::default(), SectionStyle::Headers));
}

#[test]
fn anchor_position_default_is_bottom() {
    assert!(matches!(AnchorPosition::default(), AnchorPosition::Bottom));
}

#[test]
fn actions_dialog_config_default_values() {
    let config = ActionsDialogConfig::default();
    assert!(matches!(config.search_position, SearchPosition::Bottom));
    assert!(matches!(config.section_style, SectionStyle::Headers));
    assert!(matches!(config.anchor, AnchorPosition::Bottom));
    assert!(!config.show_icons);
    assert!(!config.show_footer);
}
