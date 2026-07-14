#[allow(unused_imports)]
use super::types::{Action, ActionCategory, ScriptInfo};
#[allow(unused_imports)]
use crate::actions::builders::*;

// --- merged from part_01.rs ---
// Tests for action builder functions.
//
// Validates that built-in action builders produce correct actions with
// expected IDs, titles, shortcuts, and conditional behavior across
// multiple contexts: file search, path prompt, clipboard history,
// chat prompt, notes command bar, and AI command bar.

use crate::clipboard_history::ContentType;
use crate::file_search::{FileInfo, FileType};
use crate::prompts::PathInfo;
use std::collections::HashSet;

// ============================================================
// Helper: extract action IDs from a Vec<Action>
// ============================================================

fn action_ids(actions: &[Action]) -> Vec<&str> {
    actions.iter().map(|a| a.id.as_str()).collect()
}

fn find_action<'a>(actions: &'a [Action], id: &str) -> Option<&'a Action> {
    actions.iter().find(|a| a.id == id)
}

// ============================================================
// 1. File context actions: files vs directories
// ============================================================

#[test]
fn file_context_directory_primary_action_is_open_directory() {
    let dir = FileInfo {
        path: "/Users/test/Documents".into(),
        name: "Documents".into(),
        file_type: FileType::Directory,
        is_dir: true,
    };
    let actions = get_file_context_actions(&dir);
    let ids = action_ids(&actions);

    assert_eq!(
        actions[0].id, "file:open_directory",
        "First action for a directory must be open_directory"
    );
    assert!(
        actions[0].title.contains("Documents"),
        "Title should include directory name"
    );
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));

    // Directories should NOT have quick_look
    assert!(
        !ids.contains(&"quick_look"),
        "Directories should not have Quick Look"
    );
}

#[test]
fn file_context_file_primary_action_is_open_file() {
    let file = FileInfo {
        path: "/Users/test/photo.png".into(),
        name: "photo.png".into(),
        file_type: FileType::Image,
        is_dir: false,
    };
    let actions = get_file_context_actions(&file);
    let ids = action_ids(&actions);

    assert_eq!(
        actions[0].id, "file:open_file",
        "First action for a file must be open_file"
    );
    assert!(
        actions[0].title.contains("photo.png"),
        "Title should include filename"
    );

    // Files SHOULD have quick_look on macOS
    #[cfg(target_os = "macos")]
    assert!(
        ids.contains(&"file:quick_look"),
        "Files should have Quick Look on macOS"
    );
}

#[test]
fn file_context_common_actions_present_for_both() {
    let file = FileInfo {
        path: "/Users/test/readme.md".into(),
        name: "readme.md".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    let dir = FileInfo {
        path: "/Users/test/src".into(),
        name: "src".into(),
        file_type: FileType::Directory,
        is_dir: true,
    };

    for info in &[&file, &dir] {
        let actions = get_file_context_actions(info);
        let ids = action_ids(&actions);
        assert!(
            ids.contains(&"file:reveal_in_finder"),
            "{} should have reveal_in_finder",
            info.name
        );
        assert!(
            ids.contains(&"file:copy_path"),
            "{} should have copy_path",
            info.name
        );
        assert!(
            ids.contains(&"file:copy_filename"),
            "{} should have copy_filename",
            info.name
        );
    }
}

#[test]
fn file_context_macos_specific_actions() {
    let file = FileInfo {
        path: "/Users/test/data.csv".into(),
        name: "data.csv".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    let actions = get_file_context_actions(&file);
    let ids = action_ids(&actions);

    #[cfg(target_os = "macos")]
    {
        assert!(ids.contains(&"file:open_in_editor"), "macOS should have Open in Editor");
        assert!(ids.contains(&"file:show_info"), "macOS should have Show Info");
    }

    #[cfg(not(target_os = "macos"))]
    {
        assert!(
            !ids.contains(&"file:open_in_editor"),
            "Non-macOS should not have Open in Editor"
        );
        assert!(
            !ids.contains(&"file:show_info"),
            "Non-macOS should not have Show Info"
        );
    }
}

#[test]
fn file_context_finder_labels_use_reveal_consistently() {
    let file = FileInfo {
        path: "/Users/test/data.csv".into(),
        name: "data.csv".into(),
        file_type: FileType::File,
        is_dir: false,
    };
    let actions = get_file_context_actions(&file);

    let reveal =
        find_action(&actions, "file:reveal_in_finder").expect("missing reveal_in_finder");
    assert_eq!(reveal.title, "Reveal in Finder");

    #[cfg(target_os = "macos")]
    {
        let show_info = find_action(&actions, "file:show_info").expect("missing show_info");
        assert_eq!(show_info.title, "Show Info");
    }
}

// ============================================================
// 2. Path context actions: files vs directories
// ============================================================

#[test]
fn path_context_directory_has_open_directory_as_primary() {
    let path = PathInfo::new("Projects", "/Users/test/Projects", true);
    let actions = get_path_context_actions(&path);

    assert_eq!(actions[0].id, "file:open_directory");
    assert!(actions[0].title.contains("Projects"));
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn path_context_file_has_select_file_as_primary() {
    let path = PathInfo::new("config.toml", "/Users/test/config.toml", false);
    let actions = get_path_context_actions(&path);

    assert_eq!(actions[0].id, "file:select_file");
    assert!(actions[0].title.contains("config.toml"));
    assert_eq!(actions[0].shortcut.as_deref(), Some("↵"));
}

#[test]
fn path_context_all_common_actions_present() {
    let path = PathInfo::new("file.txt", "/tmp/file.txt", false);
    let actions = get_path_context_actions(&path);
    let ids = action_ids(&actions);

    let expected = [
        "file:copy_path",
        "file:open_in_finder",
        "file:open_in_editor",
        "file:open_in_quick_terminal",
        "file:copy_filename",
        "file:move_to_trash",
    ];
    for &expected_id in &expected {
        assert!(
            ids.contains(&expected_id),
            "Path context should have {}",
            expected_id
        );
    }
}

#[test]
fn path_context_finder_label_uses_reveal() {
    let path = PathInfo::new("file.txt", "/tmp/file.txt", false);
    let actions = get_path_context_actions(&path);
    let reveal = find_action(&actions, "file:open_in_finder").expect("missing open_in_finder");
    assert_eq!(reveal.title, "Reveal in Finder");
}

#[test]
fn path_context_move_to_trash_description_matches_type() {
    let dir_path = PathInfo::new("build", "/tmp/build", true);
    let file_path = PathInfo::new("temp.log", "/tmp/temp.log", false);

    let dir_actions = get_path_context_actions(&dir_path);
    let file_actions = get_path_context_actions(&file_path);

    let dir_trash = find_action(&dir_actions, "file:move_to_trash").unwrap();
    let file_trash = find_action(&file_actions, "file:move_to_trash").unwrap();

    assert!(
        dir_trash.description.as_ref().unwrap().contains("folder"),
        "Directory trash description should mention 'folder'"
    );
    assert!(
        file_trash.description.as_ref().unwrap().contains("file"),
        "File trash description should mention 'file'"
    );
}

// ============================================================
// 3. Clipboard history actions: text vs image, pinned vs unpinned
// ============================================================

fn make_text_entry(pinned: bool) -> ClipboardEntryInfo {
    ClipboardEntryInfo {
        id: "entry-1".into(),
        content_type: ContentType::Text,
        pinned,
        preview: "Hello world".into(),
        image_dimensions: None,
        frontmost_app_name: Some("VS Code".into()),
    }
}

fn make_image_entry(pinned: bool) -> ClipboardEntryInfo {
    ClipboardEntryInfo {
        id: "entry-2".into(),
        content_type: ContentType::Image,
        pinned,
        preview: String::new(),
        image_dimensions: Some((800, 600)),
        frontmost_app_name: None,
    }
}

#[test]
fn clipboard_text_entry_has_core_actions() {
    let entry = make_text_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);

    let expected = [
        "clip:clipboard_paste",
        "clip:clipboard_copy",
        "clip:clipboard_paste_keep_open",
        "clip:clipboard_share",
        "clip:clipboard_attach_to_ai",
        "clip:clipboard_save_snippet",
        "clip:clipboard_save_file",
        "clip:clipboard_delete",
        "clip:clipboard_delete_multiple",
        "clip:clipboard_delete_all",
    ];
    for &expected_id in &expected {
        assert!(
            ids.contains(&expected_id),
            "Text entry should have {}",
            expected_id
        );
    }
}

#[test]
fn clipboard_paste_title_includes_app_name() {
    let entry = make_text_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let paste = find_action(&actions, "clip:clipboard_paste").unwrap();

    assert_eq!(paste.title, "Paste to VS Code");
}

#[test]
fn clipboard_paste_title_fallback_when_no_app() {
    let entry = make_image_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let paste = find_action(&actions, "clip:clipboard_paste").unwrap();

    assert_eq!(paste.title, "Paste to Active App");
}

#[test]
fn clipboard_image_entry_has_ocr_action() {
    let entry = make_image_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);

    assert!(
        ids.contains(&"clip:clipboard_ocr"),
        "Image entries should have OCR action"
    );

    // macOS image-specific actions
    #[cfg(target_os = "macos")]
    {
        assert!(
            ids.contains(&"clip:clipboard_open_with"),
            "macOS image should have Open With"
        );
        assert!(
            ids.contains(&"clip:clipboard_annotate_cleanshot"),
            "macOS image should have CleanShot annotate"
        );
        assert!(
            ids.contains(&"clip:clipboard_upload_cleanshot"),
            "macOS image should have CleanShot upload"
        );
    }
}

#[test]
fn clipboard_text_entry_lacks_image_specific_actions() {
    let entry = make_text_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);

    assert!(
        !ids.contains(&"clip:clipboard_ocr"),
        "Text entries should NOT have OCR"
    );
    assert!(
        !ids.contains(&"clip:clipboard_open_with"),
        "Text entries should NOT have Open With"
    );
    assert!(
        !ids.contains(&"clip:clipboard_annotate_cleanshot"),
        "Text entries should NOT have CleanShot annotate"
    );
}

#[test]
fn clipboard_pinned_entry_shows_unpin() {
    let entry = make_text_entry(true);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);

    assert!(
        ids.contains(&"clip:clipboard_unpin"),
        "Pinned entry should show Unpin"
    );
    assert!(
        !ids.contains(&"clip:clipboard_pin"),
        "Pinned entry should NOT show Pin"
    );
}

#[test]
fn clipboard_unpinned_entry_shows_pin() {
    let entry = make_text_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);

    assert!(
        ids.contains(&"clip:clipboard_pin"),
        "Unpinned entry should show Pin"
    );
    assert!(
        !ids.contains(&"clip:clipboard_unpin"),
        "Unpinned entry should NOT show Unpin"
    );
}


// --- merged from part_02.rs ---
// ============================================================
// 4. Chat context actions: model selection, conditional actions
// ============================================================

#[test]
fn chat_context_model_selection_marks_current() {
    let info = ChatPromptInfo {
        current_model: Some("Claude Sonnet".into()),
        available_models: vec![
            ChatModelInfo {
                id: "claude-sonnet".into(),
                display_name: "Claude Sonnet".into(),
                provider: "Anthropic".into(),
            },
            ChatModelInfo {
                id: "gpt-4".into(),
                display_name: "GPT-4".into(),
                provider: "OpenAI".into(),
            },
        ],
        has_messages: true,
        has_response: true,
    };
    let actions = get_chat_context_actions(&info);
    let change_model = find_action(&actions, "chat:change_model").unwrap();
    assert_eq!(change_model.title, "Change Model");
    assert_eq!(
        change_model.description.as_deref(),
        Some("Current: Claude Sonnet")
    );

    let picker = get_chat_model_picker_actions(&info);
    let sonnet = find_action(&picker, "chat:select_model_claude-sonnet").unwrap();
    assert!(
        sonnet.title.contains("✓"),
        "Current model should have checkmark"
    );

    let gpt4 = find_action(&picker, "chat:select_model_gpt-4").unwrap();
    assert!(
        !gpt4.title.contains("✓"),
        "Non-current model should not have checkmark"
    );
}

#[test]
fn chat_context_copy_response_only_when_has_response() {
    let with_response = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: true,
    };
    let actions = get_chat_context_actions(&with_response);
    let ids = action_ids(&actions);
    assert!(
        ids.contains(&"chat:copy_response"),
        "Should have copy_response when has_response=true"
    );

    let without_response = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };
    let actions = get_chat_context_actions(&without_response);
    let ids = action_ids(&actions);
    assert!(
        !ids.contains(&"chat:copy_response"),
        "Should NOT have copy_response when has_response=false"
    );
}

#[test]
fn chat_context_clear_only_when_has_messages() {
    let with_msgs = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: true,
        has_response: false,
    };
    let actions = get_chat_context_actions(&with_msgs);
    let ids = action_ids(&actions);
    assert!(
        ids.contains(&"chat:clear_conversation"),
        "Should have clear_conversation when has_messages=true"
    );

    let empty = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };
    let actions = get_chat_context_actions(&empty);
    let ids = action_ids(&actions);
    assert!(
        !ids.contains(&"chat:clear_conversation"),
        "Should NOT have clear_conversation when has_messages=false"
    );
}

#[test]
fn chat_context_continue_in_chat_always_present() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };
    let actions = get_chat_context_actions(&info);
    let ids = action_ids(&actions);
    assert!(
        ids.contains(&"chat:continue_in_chat"),
        "continue_in_chat should always be present"
    );
}

// ============================================================
// 5. to_deeplink_name conversion
// ============================================================

#[test]
fn deeplink_name_lowercase_and_hyphenated() {
    assert_eq!(to_deeplink_name("My Script"), "my-script");
    assert_eq!(to_deeplink_name("Clipboard History"), "clipboard-history");
    assert_eq!(to_deeplink_name("hello_world"), "hello-world");
}

#[test]
fn deeplink_name_strips_special_chars() {
    assert_eq!(to_deeplink_name("hello@world!"), "hello-world");
    assert_eq!(to_deeplink_name("  spaced  out  "), "spaced-out");
    assert_eq!(to_deeplink_name("---dashes---"), "dashes");
}

#[test]
fn deeplink_name_preserves_alphanumeric() {
    assert_eq!(to_deeplink_name("script123"), "script123");
    assert_eq!(to_deeplink_name("ABC"), "abc");
}

// ============================================================
// 6. Script context actions: sections + destructive ordering
// ============================================================

#[test]
fn script_context_actions_use_sectioned_grouping() {
    let script = ScriptInfo::new("my-script", "/path/to/my-script.ts");
    let actions = get_script_context_actions(&script);

    let run = find_action(&actions, "run_script").expect("missing run_script");
    assert_eq!(run.section.as_deref(), Some("Actions"));

    let add_shortcut = find_action(&actions, "add_shortcut").expect("missing add_shortcut");
    assert_eq!(add_shortcut.section.as_deref(), Some("Edit"));

    let add_alias = find_action(&actions, "add_alias").expect("missing add_alias");
    assert_eq!(add_alias.section.as_deref(), Some("Edit"));

    let edit_script = find_action(&actions, "edit_script").expect("missing edit_script");
    assert_eq!(edit_script.section.as_deref(), Some("Edit"));

    let copy_path = find_action(&actions, "copy_path").expect("missing copy_path");
    assert_eq!(copy_path.section.as_deref(), Some("Share"));

    let copy_deeplink = find_action(&actions, "copy_deeplink").expect("missing copy_deeplink");
    assert_eq!(copy_deeplink.section.as_deref(), Some("Share"));
}

#[test]
fn script_context_destructive_actions_are_last_and_marked() {
    let script = ScriptInfo::with_shortcut_and_alias(
        "my-script",
        "/path/to/my-script.ts",
        Some("cmd+shift+m".to_string()),
        Some("ms".to_string()),
    )
    .with_frecency(true, Some("my-script:/path/to/my-script.ts".to_string()));

    let actions = get_script_context_actions(&script);

    let first_destructive_index = actions
        .iter()
        .position(|a| a.section.as_deref() == Some("Destructive"))
        .expect("expected destructive section");

    for action in actions.iter().skip(first_destructive_index) {
        assert_eq!(
            action.section.as_deref(),
            Some("Destructive"),
            "all trailing actions should be in Destructive section"
        );
    }

    let remove_shortcut =
        find_action(&actions, "remove_shortcut").expect("missing remove_shortcut");
    assert_eq!(remove_shortcut.section.as_deref(), Some("Destructive"));
    assert_eq!(remove_shortcut.shortcut.as_deref(), Some("⌘⌥K"));

    let remove_alias = find_action(&actions, "remove_alias").expect("missing remove_alias");
    assert_eq!(remove_alias.section.as_deref(), Some("Destructive"));
    assert_eq!(remove_alias.shortcut.as_deref(), Some("⌘⌥A"));

    let reset_ranking = find_action(&actions, "reset_ranking").expect("missing reset_ranking");
    assert_eq!(reset_ranking.section.as_deref(), Some("Destructive"));
    assert!(
        reset_ranking.shortcut.is_some(),
        "destructive action should include keyboard hint"
    );
}

// ============================================================
// 7. AI command bar: sections, icons, completeness
// ============================================================

#[test]
fn ai_command_bar_has_all_expected_actions() {
    let actions = get_ai_command_bar_actions();
    let ids = action_ids(&actions);

    let expected = [
        "chat:copy_response",
        "chat:copy_chat",
        "chat:copy_last_code",
        "chat:submit",
        "chat:new_chat",
        "chat:delete_chat",
        "chat:add_file",
        "chat:paste_image",
        "chat:change_model",
    ];
    for &expected_id in &expected {
        assert!(
            ids.contains(&expected_id),
            "AI command bar should have {}",
            expected_id
        );
    }
}

#[test]
fn ai_command_bar_actions_have_sections() {
    let actions = get_ai_command_bar_actions();

    for action in &actions {
        assert!(
            action.section.is_some(),
            "AI command bar action '{}' should have a section",
            action.id
        );
    }

    // Verify correct section assignments
    let response_ids = ["chat:copy_response", "chat:copy_chat", "chat:copy_last_code"];
    for id in &response_ids {
        let a = find_action(&actions, id).unwrap();
        assert_eq!(
            a.section.as_deref(),
            Some("Response"),
            "{} should be in Response section",
            id
        );
    }

    let action_ids_list = ["chat:submit", "chat:new_chat", "chat:delete_chat"];
    for id in &action_ids_list {
        let a = find_action(&actions, id).unwrap();
        assert_eq!(
            a.section.as_deref(),
            Some("Actions"),
            "{} should be in Actions section",
            id
        );
    }
}

#[test]
fn ai_command_bar_actions_have_icons() {
    let actions = get_ai_command_bar_actions();

    for action in &actions {
        assert!(
            action.icon.is_some(),
            "AI command bar action '{}' should have an icon",
            action.id
        );
    }
}

// ============================================================
// 7. Notes command bar: conditional actions
// ============================================================

#[test]
fn notes_command_bar_minimal_when_no_selection() {
    let info = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids = action_ids(&actions);

    assert!(
        ids.contains(&"new_note"),
        "new_note should always be present"
    );
    assert!(
        ids.contains(&"browse_notes"),
        "browse_notes should always be present"
    );
    assert!(
        !ids.contains(&"duplicate_note"),
        "duplicate_note requires selection"
    );
    assert!(
        !ids.contains(&"find_in_note"),
        "find_in_note requires selection"
    );
    assert!(!ids.contains(&"export"), "export requires selection");
}

#[test]
fn notes_command_bar_full_when_selected() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids = action_ids(&actions);

    let expected = [
        "new_note",
        "duplicate_note",
        "browse_notes",
        "find_in_note",
        "format",
        "copy_note_as",
        "copy_deeplink",
        "create_quicklink",
        "export",
    ];
    for &expected_id in &expected {
        assert!(
            ids.contains(&expected_id),
            "Notes with selection should have {}",
            expected_id
        );
    }
}

#[test]
fn notes_command_bar_trash_view_suppresses_editing() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: true,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids = action_ids(&actions);

    // Trash view with selection should NOT show editing/copy actions
    assert!(
        !ids.contains(&"duplicate_note"),
        "Trash view should not have duplicate_note"
    );
    assert!(
        !ids.contains(&"find_in_note"),
        "Trash view should not have find_in_note"
    );
    assert!(
        !ids.contains(&"export"),
        "Trash view should not have export"
    );

    // But new_note and browse_notes should still be available
    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
}

#[test]
fn notes_command_bar_auto_sizing_toggle() {
    let disabled = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&disabled);
    let toggle = actions
        .iter()
        .find(|action| action.id == "toggle_auto_sizing")
        .expect("auto-sizing toggle should always be present");
    assert_eq!(toggle.title, "Enable Auto-Sizing");

    let enabled = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&enabled);
    let toggle = actions
        .iter()
        .find(|action| action.id == "toggle_auto_sizing")
        .expect("auto-sizing toggle should always be present");
    assert_eq!(toggle.title, "Disable Auto-Sizing");
}

// ============================================================

// --- merged from part_03.rs ---
// 8. Scriptlet context actions with custom H3 actions
// ============================================================

#[test]
fn scriptlet_context_includes_shortcut_and_alias_dynamics() {
    // Scriptlet with shortcut + alias
    let info = ScriptInfo::scriptlet(
        "Quick Open",
        "/path/to/urls.md#quick-open",
        Some("cmd+o".into()),
        Some("qo".into()),
    );
    let actions = get_scriptlet_context_actions_with_custom(&info, None);
    let ids = action_ids(&actions);

    // Should have update/remove (not add) for both shortcut and alias
    assert!(ids.contains(&"update_shortcut"));
    assert!(ids.contains(&"remove_shortcut"));
    assert!(!ids.contains(&"add_shortcut"));

    assert!(ids.contains(&"update_alias"));
    assert!(ids.contains(&"remove_alias"));
    assert!(!ids.contains(&"add_alias"));

    // Should have scriptlet-specific actions
    assert!(ids.contains(&"edit_scriptlet"));
    assert!(ids.contains(&"reveal_scriptlet_in_finder"));
    assert!(ids.contains(&"copy_scriptlet_path"));
    assert!(ids.contains(&"copy_content"));
    assert!(ids.contains(&"copy_deeplink"));
}

#[test]
fn scriptlet_context_without_shortcut_shows_add() {
    let info = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
    let actions = get_scriptlet_context_actions_with_custom(&info, None);
    let ids = action_ids(&actions);

    assert!(ids.contains(&"add_shortcut"));
    assert!(ids.contains(&"add_alias"));
    assert!(!ids.contains(&"update_shortcut"));
    assert!(!ids.contains(&"remove_shortcut"));
}

// ============================================================
// 9. Action struct builder methods
// ============================================================

#[test]
fn action_with_shortcut_caches_lowercase() {
    let action =
        Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧K");

    assert_eq!(action.shortcut, Some("⌘⇧K".into()));
    assert!(action.shortcut_lower.is_some());
}

#[test]
fn action_with_shortcut_opt_none_no_op() {
    let action =
        Action::new("test", "Test", None, ActionCategory::ScriptContext).with_shortcut_opt(None);

    assert!(action.shortcut.is_none());
    assert!(action.shortcut_lower.is_none());
}

#[test]
fn action_caches_lowercase_title_and_description() {
    let action = Action::new(
        "test",
        "Copy Path",
        Some("Copy the FULL Path".into()),
        ActionCategory::ScriptContext,
    );

    assert_eq!(action.title_lower, "copy path");
    assert_eq!(
        action.description_lower.as_deref(),
        Some("copy the full path")
    );
}

#[test]
fn action_has_action_defaults_to_false() {
    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
    assert!(
        !action.has_action,
        "Built-in actions should default has_action to false"
    );
}

// ============================================================
// 10. Global actions (seeded with launcher ops, verify contract)
// ============================================================

#[test]
fn global_actions_are_seeded() {
    let actions = get_global_actions();
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(
        ids.contains(&"reload_scripts"),
        "missing reload_scripts: {ids:?}"
    );
    assert!(ids.contains(&"settings"), "missing settings: {ids:?}");
    assert!(ids.contains(&"view_logs"), "missing view_logs: {ids:?}");
}

// ============================================================
// 11. Note switcher actions (Cmd+P in Notes window)
// ============================================================

#[test]
fn note_switcher_empty_shows_no_notes_message() {
    let notes: Vec<NoteSwitcherNoteInfo> = vec![];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, "no_notes");
    assert_eq!(actions[0].title, "No notes yet");
    assert!(actions[0].description.as_ref().unwrap().contains("⌘N"));
    assert_eq!(actions[0].section.as_deref(), Some("Notes"));
}

#[test]
fn note_switcher_single_current_note() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "uuid-1".into(),
        title: "My Note".into(),
        char_count: 42,
        is_current: true,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, "note_uuid-1");
    assert!(
        actions[0].title.starts_with("• "),
        "Current note should have bullet prefix"
    );
    assert!(actions[0].title.contains("My Note"));
    assert_eq!(actions[0].description.as_deref(), Some("42 chars"));
}

#[test]
fn note_switcher_pinned_note_icon() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "pinned-1".into(),
            title: "Pinned Note".into(),
            char_count: 100,
            is_current: false,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "current-1".into(),
            title: "Current Note".into(),
            char_count: 50,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "plain-1".into(),
            title: "Plain Note".into(),
            char_count: 10,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];
    let actions = get_note_switcher_actions(&notes);

    assert_eq!(actions.len(), 3);

    // Pinned note gets StarFilled icon
    let pinned = find_action(&actions, "note_pinned-1").unwrap();
    assert_eq!(
        pinned.icon,
        Some(crate::designs::icon_variations::IconName::StarFilled)
    );
    assert!(
        !pinned.title.starts_with("• "),
        "Non-current should not have bullet"
    );

    // Current note gets Check icon
    let current = find_action(&actions, "note_current-1").unwrap();
    assert_eq!(
        current.icon,
        Some(crate::designs::icon_variations::IconName::Check)
    );
    assert!(current.title.starts_with("• "));

    // Plain note gets File icon
    let plain = find_action(&actions, "note_plain-1").unwrap();
    assert_eq!(
        plain.icon,
        Some(crate::designs::icon_variations::IconName::File)
    );
}

#[test]
fn note_switcher_singular_character_count() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "one-char".into(),
        title: "Tiny".into(),
        char_count: 1,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(
        actions[0].description.as_deref(),
        Some("1 char"),
        "Single char should use singular 'char'"
    );
}

#[test]
fn note_switcher_zero_characters() {
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
    assert_eq!(
        actions[0].description.as_deref(),
        Some("0 chars"),
        "Zero chars should use plural 'chars'"
    );
}

#[test]
fn note_switcher_all_notes_have_section() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "A".into(),
            char_count: 5,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "b".into(),
            title: "B".into(),
            char_count: 10,
            is_current: true,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];
    let actions = get_note_switcher_actions(&notes);
    for action in &actions {
        let section = action.section.as_deref();
        assert!(
            section == Some("Pinned") || section == Some("Recent"),
            "Note switcher action should be in 'Pinned' or 'Recent' section, got {:?}",
            section
        );
    }
}


// --- merged from part_04.rs ---
// ============================================================
// 12. New chat actions (AI window new chat dropdown)
// ============================================================

#[test]
fn new_chat_actions_empty_inputs() {
    let actions = get_new_chat_actions(&[], &[], &[]);
    assert!(actions.is_empty(), "No inputs should produce no actions");
}

#[test]
fn new_chat_actions_last_used_section() {
    let last_used = vec![
        NewChatModelInfo {
            model_id: "claude-sonnet".into(),
            display_name: "Claude Sonnet".into(),
            provider: "anthropic".into(),
            provider_display_name: "Anthropic".into(),
        },
        NewChatModelInfo {
            model_id: "gpt-4".into(),
            display_name: "GPT-4".into(),
            provider: "openai".into(),
            provider_display_name: "OpenAI".into(),
        },
    ];
    let actions = get_new_chat_actions(&last_used, &[], &[]);
    assert_eq!(actions.len(), 2);

    assert_eq!(actions[0].id, "last_used_anthropic::claude-sonnet");
    assert_eq!(actions[0].title, "Claude Sonnet");
    assert_eq!(actions[0].description.as_deref(), Some("Uses Anthropic"));
    assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));

    assert_eq!(actions[1].id, "last_used_openai::gpt-4");
    assert_eq!(actions[1].title, "GPT-4");
    assert_eq!(actions[1].description.as_deref(), Some("Uses OpenAI"));
}

#[test]
fn new_chat_actions_presets_section() {
    let presets = vec![
        NewChatPresetInfo {
            id: "general".into(),
            name: "General".into(),
            icon: crate::designs::icon_variations::IconName::Settings,
        },
        NewChatPresetInfo {
            id: "code".into(),
            name: "Code".into(),
            icon: crate::designs::icon_variations::IconName::Code,
        },
    ];
    let actions = get_new_chat_actions(&[], &presets, &[]);
    assert_eq!(actions.len(), 2);

    assert_eq!(actions[0].id, "preset_general");
    assert_eq!(actions[0].title, "General");
    assert_eq!(actions[0].section.as_deref(), Some("Presets"));
    assert_eq!(
        actions[0].description.as_deref(),
        Some("Uses General preset")
    );

    assert_eq!(actions[1].id, "preset_code");
    assert_eq!(actions[1].title, "Code");
}

#[test]
fn new_chat_actions_models_section() {
    let models = vec![NewChatModelInfo {
        model_id: "opus".into(),
        display_name: "Claude Opus".into(),
        provider: "anthropic".into(),
        provider_display_name: "Anthropic".into(),
    }];
    let actions = get_new_chat_actions(&[], &[], &models);
    assert_eq!(actions.len(), 1);

    assert_eq!(actions[0].id, "model_anthropic::opus");
    assert_eq!(actions[0].title, "Claude Opus");
    assert_eq!(actions[0].description.as_deref(), Some("Uses Anthropic"));
    assert_eq!(actions[0].section.as_deref(), Some("Models"));
}

#[test]
fn new_chat_actions_combined_ordering() {
    let last_used = vec![NewChatModelInfo {
        model_id: "last".into(),
        display_name: "Last".into(),
        provider: "p".into(),
        provider_display_name: "P".into(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "preset".into(),
        name: "Preset".into(),
        icon: crate::designs::icon_variations::IconName::Star,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "model".into(),
        display_name: "Model".into(),
        provider: "p".into(),
        provider_display_name: "P".into(),
    }];

    let actions = get_new_chat_actions(&last_used, &presets, &models);
    assert_eq!(actions.len(), 3);

    // Ordering: Last Used -> Presets -> Models
    assert_eq!(actions[0].section.as_deref(), Some("Last Used Settings"));
    assert_eq!(actions[1].section.as_deref(), Some("Presets"));
    assert_eq!(actions[2].section.as_deref(), Some("Models"));
}

#[test]
fn new_chat_actions_all_have_icons() {
    let last_used = vec![NewChatModelInfo {
        model_id: "m".into(),
        display_name: "M".into(),
        provider: "p".into(),
        provider_display_name: "P".into(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "pr".into(),
        name: "PR".into(),
        icon: crate::designs::icon_variations::IconName::Code,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "mo".into(),
        display_name: "MO".into(),
        provider: "p".into(),
        provider_display_name: "P".into(),
    }];

    let actions = get_new_chat_actions(&last_used, &presets, &models);
    for action in &actions {
        assert!(
            action.icon.is_some(),
            "New chat action '{}' should have an icon",
            action.id
        );
    }
}

// ============================================================
// 13. Agent-specific script context actions
// ============================================================

#[test]
fn agent_context_has_agent_actions() {
    let mut agent = ScriptInfo::new("My Agent", "/path/to/agent.claude.md");
    agent.is_agent = true;
    agent.is_script = false;

    let actions = get_script_context_actions(&agent);
    let ids = action_ids(&actions);

    // Agent should have: edit (as "Edit Agent"), reveal, copy_path, copy_content
    assert!(
        ids.contains(&"edit_script"),
        "Agent should have edit_script action"
    );
    assert!(ids.contains(&"reveal_in_finder"));
    assert!(ids.contains(&"copy_path"));
    assert!(ids.contains(&"copy_content"));

    // Verify edit title says "Edit Agent" not "Edit Script"
    let edit = find_action(&actions, "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
    assert_eq!(
        edit.description.as_deref(),
        Some("Open the agent file in $EDITOR")
    );
}

#[test]
fn agent_context_no_script_only_actions() {
    let mut agent = ScriptInfo::new("Agent", "/path/to/agent.md");
    agent.is_agent = true;
    agent.is_script = false;

    let actions = get_script_context_actions(&agent);
    let ids = action_ids(&actions);

    // Agent should NOT have view_logs (script-only)
    assert!(
        !ids.contains(&"view_logs"),
        "Agent should not have view_logs"
    );
}

#[test]
fn agent_context_has_deeplink_and_shortcut() {
    let mut agent = ScriptInfo::new("Code Review Agent", "/path/to/agent.md");
    agent.is_agent = true;
    agent.is_script = false;

    let actions = get_script_context_actions(&agent);
    let ids = action_ids(&actions);

    // Should still have universal actions
    assert!(ids.contains(&"run_script"), "Agent should have run action");
    assert!(
        ids.contains(&"copy_deeplink"),
        "Agent should have copy_deeplink"
    );
    assert!(
        !ids.contains(&"add_shortcut"),
        "Agent should not advertise unsupported add_shortcut"
    );
    assert!(
        !ids.contains(&"add_alias"),
        "Agent should not advertise unsupported add_alias"
    );
}

#[test]
fn agent_context_with_frecency_shows_reset() {
    let mut agent = ScriptInfo::new("Agent", "/path/to/agent.md");
    agent.is_agent = true;
    agent.is_script = false;
    let agent = agent.with_frecency(true, Some("agent:path".into()));

    let actions = get_script_context_actions(&agent);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"reset_ranking"));
}

#[test]
fn mixed_script_and_agent_flags_do_not_create_duplicate_action_ids() {
    let mut script = ScriptInfo::new("Mixed Flags", "/path/to/mixed.ts");
    script.is_agent = true;
    // Keep is_script=true to simulate accidental mixed flag state.

    let actions = get_script_context_actions(&script);

    let mut seen = HashSet::new();
    for action in &actions {
        assert!(
            seen.insert(action.id.as_str()),
            "duplicate action id generated: {}",
            action.id
        );
    }

    let edit_script_count = actions.iter().filter(|a| a.id == "edit_script").count();
    assert_eq!(edit_script_count, 1);
}
