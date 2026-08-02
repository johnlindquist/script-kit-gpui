// --- merged from part_01.rs ---
//! Dialog and window action behavior validation tests
//!
//! Validates random built-in actions from dialog/window contexts:
//! - CommandBarConfig preset variants (main_menu, ai, no_search)
//! - Action ordering guarantees across contexts
//! - Notes command bar all 8 (selection x trash x auto-sizing) permutations
//! - Clipboard action structure: paste first, destructive last
//! - Chat context conditional action combos
//! - Note switcher ordering and icon assignment
//! - Path/file context action count consistency
//! - Deeplink name edge cases
//! - Agent with shortcut+alias+frecency combined
//! - ScriptInfo constructor invariant checks

use super::builders::{
    get_chat_context_actions, get_chat_model_picker_actions,
    get_clipboard_history_context_actions, get_file_context_actions, get_new_chat_actions,
    get_note_switcher_actions, get_notes_command_bar_actions, get_path_context_actions,
    get_script_context_actions, get_scriptlet_context_actions_with_custom, to_deeplink_name,
    ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
    NoteSwitcherNoteInfo, NotesInfo,
};
use super::command_bar::CommandBarConfig;
use super::dialog::ActionsDialog;
use super::types::{
    Action, ActionCategory, AnchorPosition, ScriptInfo, SearchPosition, SectionStyle,
};
use crate::clipboard_history::ContentType;
use crate::designs::icon_variations::IconName;
use crate::file_search::FileInfo;
use crate::prompts::PathInfo;
use crate::protocol::ProtocolAction;

// =========================================================================
// CommandBarConfig preset variants
// =========================================================================

#[test]
fn command_bar_config_default_values() {
    let config = CommandBarConfig::default();
    assert!(matches!(
        config.dialog_config.search_position,
        SearchPosition::Top
    ));
    assert!(matches!(
        config.dialog_config.section_style,
        SectionStyle::Headers
    ));
    assert!(matches!(
        config.dialog_config.anchor,
        AnchorPosition::Bottom
    ));
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

#[test]
fn command_bar_config_main_menu_style() {
    let config = CommandBarConfig::main_menu_style();
    assert!(matches!(
        config.dialog_config.search_position,
        SearchPosition::Top
    ));
    assert!(matches!(
        config.dialog_config.section_style,
        SectionStyle::Headers
    ));
    assert!(matches!(
        config.dialog_config.anchor,
        AnchorPosition::Bottom
    ));
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
    // Inherits default close behaviors
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

#[test]
fn command_bar_config_ai_style() {
    let config = CommandBarConfig::ai_style();
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
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

#[test]
fn command_bar_config_no_search() {
    let config = CommandBarConfig::no_search();
    assert!(matches!(
        config.dialog_config.search_position,
        SearchPosition::Hidden
    ));
    assert!(matches!(
        config.dialog_config.section_style,
        SectionStyle::Headers
    ));
    assert!(matches!(
        config.dialog_config.anchor,
        AnchorPosition::Bottom
    ));
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
}

#[test]
fn command_bar_config_notes_style() {
    let config = CommandBarConfig::notes_style();
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
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

// =========================================================================
// Action ordering: paste always first in clipboard
// =========================================================================

#[test]
fn clipboard_paste_is_always_first_action() {
    // Text entry
    let text_entry = ClipboardEntryInfo {
        id: "t1".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "hello".to_string(),
        image_dimensions: None,
        frontmost_app_name: Some("Finder".to_string()),
    };
    let text_actions = get_clipboard_history_context_actions(&text_entry);
    assert_eq!(text_actions[0].id, "clip:clipboard_paste");

    // Image entry
    let img_entry = ClipboardEntryInfo {
        id: "i1".to_string(),
        content_type: ContentType::Image,
        pinned: true,
        preview: "Screenshot".to_string(),
        image_dimensions: Some((800, 600)),
        frontmost_app_name: None,
    };
    let img_actions = get_clipboard_history_context_actions(&img_entry);
    assert_eq!(img_actions[0].id, "clip:clipboard_paste");
}

#[test]
fn clipboard_destructive_actions_are_last_three() {
    let entry = ClipboardEntryInfo {
        id: "d1".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "test".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&entry);
    let len = actions.len();

    // Last three actions should be the destructive ones
    assert_eq!(actions[len - 3].id, "clip:clipboard_delete");
    assert_eq!(actions[len - 2].id, "clip:clipboard_delete_multiple");
    assert_eq!(actions[len - 1].id, "clip:clipboard_delete_all");
}

#[test]
fn clipboard_copy_is_second_action() {
    let entry = ClipboardEntryInfo {
        id: "c1".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "text".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&entry);
    assert_eq!(actions[1].id, "clip:clipboard_copy");
}

// =========================================================================
// Chat context: all conditional action combinations
// =========================================================================

#[test]
fn chat_no_messages_no_response_has_only_continue() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_response: false,
        has_messages: false,
    };
    let actions = get_chat_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"chat:change_model"));
    assert!(ids.contains(&"chat:continue_in_chat"));
    assert!(!ids.contains(&"chat:copy_response"));
    assert!(!ids.contains(&"chat:clear_conversation"));
    // change_model + continue_in_chat + capture_screen_area
    assert_eq!(actions.len(), 3);
}

#[test]
fn chat_has_response_only_shows_copy_response() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_response: true,
        has_messages: false,
    };
    let actions = get_chat_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"chat:continue_in_chat"));
    assert!(ids.contains(&"chat:copy_response"));
    assert!(!ids.contains(&"chat:clear_conversation"));
}

#[test]
fn chat_has_messages_only_shows_clear() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_response: false,
        has_messages: true,
    };
    let actions = get_chat_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"chat:continue_in_chat"));
    assert!(!ids.contains(&"chat:copy_response"));
    assert!(ids.contains(&"chat:clear_conversation"));
}

#[test]
fn chat_has_both_response_and_messages() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_response: true,
        has_messages: true,
    };
    let actions = get_chat_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"chat:continue_in_chat"));
    assert!(ids.contains(&"chat:copy_response"));
    assert!(ids.contains(&"chat:clear_conversation"));
}

#[test]
fn chat_model_checkmark_exact_format() {
    let info = ChatPromptInfo {
        current_model: Some("Claude Sonnet".to_string()),
        available_models: vec![
            ChatModelInfo {
                id: "sonnet".to_string(),
                display_name: "Claude Sonnet".to_string(),
                provider: "Anthropic".to_string(),
            },
            ChatModelInfo {
                id: "gpt4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "OpenAI".to_string(),
            },
        ],
        has_response: false,
        has_messages: false,
    };
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);

    // Current model should have checkmark in title
    let sonnet = picker
        .iter()
        .find(|a| a.id == "chat:select_model_sonnet")
        .unwrap();
    assert_eq!(sonnet.title, "Claude Sonnet \u{2713}"); // "Claude Sonnet ✓"

    // Other model should NOT have checkmark
    let gpt4 = picker
        .iter()
        .find(|a| a.id == "chat:select_model_gpt4")
        .unwrap();
    assert_eq!(gpt4.title, "GPT-4");
    assert!(!gpt4.title.contains('\u{2713}'));
}

#[test]
fn chat_model_description_shows_provider() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![ChatModelInfo {
            id: "haiku".to_string(),
            display_name: "Claude Haiku".to_string(),
            provider: "Anthropic".to_string(),
        }],
        has_response: false,
        has_messages: false,
    };
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);
    let haiku = picker
        .iter()
        .find(|a| a.id == "chat:select_model_haiku")
        .unwrap();
    assert_eq!(haiku.description, Some("Uses Anthropic".to_string()));
}

// =========================================================================
// Notes command bar: all 8 permutations
// =========================================================================

#[test]
fn notes_no_selection_no_trash_no_autosizing() {
    let info = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
    assert!(ids.contains(&"toggle_auto_sizing"));
    // No selection → no edit/copy/export actions
    assert!(!ids.contains(&"duplicate_note"));
    assert!(!ids.contains(&"find_in_note"));
    assert!(!ids.contains(&"copy_note_as"));
    assert!(!ids.contains(&"export"));
}

#[test]
fn notes_with_selection_no_trash_no_autosizing() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
    assert!(ids.contains(&"duplicate_note"));
    assert!(ids.contains(&"find_in_note"));
    assert!(ids.contains(&"format"));
    assert!(ids.contains(&"copy_note_as"));
    assert!(ids.contains(&"copy_deeplink"));
    assert!(ids.contains(&"create_quicklink"));
    assert!(ids.contains(&"export"));
    assert!(ids.contains(&"toggle_auto_sizing"));
}

#[test]
fn notes_with_selection_no_trash_with_autosizing() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    assert!(ids.contains(&"toggle_auto_sizing"));
    // Still has all selection actions
    assert!(ids.contains(&"duplicate_note"));
    assert!(ids.contains(&"find_in_note"));
}

#[test]
fn notes_trash_view_with_selection_suppresses_edit_copy_export() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: true,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Trash view suppresses all selection-gated actions
    assert!(!ids.contains(&"duplicate_note"));
    assert!(!ids.contains(&"find_in_note"));
    assert!(!ids.contains(&"format"));
    assert!(!ids.contains(&"copy_note_as"));
    assert!(!ids.contains(&"script:copy_deeplink"));
    assert!(!ids.contains(&"create_quicklink"));
    assert!(!ids.contains(&"export"));

    // Always-present actions remain
    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
}

#[test]
fn notes_no_selection_trash_view() {
    let info = NotesInfo {
        has_selection: false,
        is_trash_view: true,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Minimal actions only
    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
    assert!(!ids.contains(&"duplicate_note"));
    assert!(ids.contains(&"toggle_auto_sizing"));
}

#[test]
fn notes_all_actions_have_icons() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);

    for action in &actions {
        assert!(
            action.icon.is_some(),
            "Notes action '{}' should have an icon",
            action.id
        );
    }
}

#[test]
fn notes_all_actions_have_sections() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);

    for action in &actions {
        assert!(
            action.section.is_some(),
            "Notes action '{}' should have a section",
            action.id
        );
    }
}

// --- merged from part_02.rs ---

// =========================================================================
// Note switcher: ordering, icons, multiple notes
// =========================================================================

#[test]
fn note_switcher_multiple_notes_ordering_preserved() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "aaa".to_string(),
            title: "First".to_string(),
            char_count: 100,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "bbb".to_string(),
            title: "Second".to_string(),
            char_count: 200,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "ccc".to_string(),
            title: "Third".to_string(),
            char_count: 50,
            is_current: false,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].id, "note_aaa");
    assert_eq!(actions[1].id, "note_bbb");
    assert_eq!(actions[2].id, "note_ccc");
}

#[test]
fn note_switcher_icon_priority_pinned_over_current() {
    // A note that is BOTH pinned and current should show StarFilled (pinned wins)
    let notes = vec![NoteSwitcherNoteInfo {
        id: "x".to_string(),
        title: "Both".to_string(),
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
fn note_switcher_current_note_title_prefix() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "cur".to_string(),
        title: "My Note".to_string(),
        char_count: 5,
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
    assert!(actions[0].title.contains("My Note"));
}

#[test]
fn note_switcher_non_current_no_prefix() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "other".to_string(),
        title: "Other Note".to_string(),
        char_count: 42,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].title, "Other Note");
    assert!(!actions[0].title.starts_with("• "));
}

#[test]
fn note_switcher_char_count_description() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "z".to_string(),
            title: "Zero".to_string(),
            char_count: 0,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "o".to_string(),
            title: "One".to_string(),
            char_count: 1,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "m".to_string(),
            title: "Many".to_string(),
            char_count: 500,
            is_current: false,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(
        actions[0].description,
        Some("0 chars".to_string()),
        "Zero chars should be plural"
    );
    assert_eq!(
        actions[1].description,
        Some("1 char".to_string()),
        "One char should be singular"
    );
    assert_eq!(
        actions[2].description,
        Some("500 chars".to_string()),
        "Many chars should be plural"
    );
}

#[test]
fn note_switcher_all_have_notes_section() {
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
            is_current: true,
            is_pinned: true,
            preview: String::new(),
            relative_time: String::new(),
        },
    ];
    let actions = get_note_switcher_actions(&notes);
    for action in &actions {
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
// New chat actions: section ordering
// =========================================================================

#[test]
fn new_chat_actions_section_ordering() {
    let last_used = vec![NewChatModelInfo {
        model_id: "lu1".to_string(),
        display_name: "Last Used Model".to_string(),
        provider: "test".to_string(),
        provider_display_name: "Test Provider".to_string(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "general".to_string(),
        name: "General".to_string(),
        icon: IconName::Star,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "m1".to_string(),
        display_name: "Model One".to_string(),
        provider: "test".to_string(),
        provider_display_name: "Test".to_string(),
    }];

    let actions = get_new_chat_actions(&last_used, &presets, &models);
    assert_eq!(actions.len(), 3);

    // Ordering: Last Used, then Presets, then Models
    assert_eq!(
        actions[0].section,
        Some("Last Used Settings".to_string()),
        "First should be Last Used section"
    );
    assert_eq!(
        actions[1].section,
        Some("Presets".to_string()),
        "Second should be Presets section"
    );
    assert_eq!(
        actions[2].section,
        Some("Models".to_string()),
        "Third should be Models section"
    );
}

#[test]
fn new_chat_actions_all_have_icons() {
    let last_used = vec![NewChatModelInfo {
        model_id: "lu".to_string(),
        display_name: "LU".to_string(),
        provider: "p".to_string(),
        provider_display_name: "P".to_string(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "p1".to_string(),
        name: "P1".to_string(),
        icon: IconName::Code,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "m".to_string(),
        display_name: "M".to_string(),
        provider: "p".to_string(),
        provider_display_name: "P".to_string(),
    }];

    let actions = get_new_chat_actions(&last_used, &presets, &models);
    for action in &actions {
        assert!(
            action.icon.is_some(),
            "New chat action '{}' should have icon",
            action.id
        );
    }
}

#[test]
fn new_chat_last_used_has_bolt_icon() {
    let last_used = vec![NewChatModelInfo {
        model_id: "lu".to_string(),
        display_name: "LU".to_string(),
        provider: "p".to_string(),
        provider_display_name: "P".to_string(),
    }];
    let actions = get_new_chat_actions(&last_used, &[], &[]);
    assert_eq!(
        actions[0].icon,
        Some(IconName::BoltFilled),
        "Last used entries should have BoltFilled icon"
    );
}

#[test]
fn new_chat_models_has_settings_icon() {
    let models = vec![NewChatModelInfo {
        model_id: "m".to_string(),
        display_name: "M".to_string(),
        provider: "p".to_string(),
        provider_display_name: "P".to_string(),
    }];
    let actions = get_new_chat_actions(&[], &[], &models);
    assert_eq!(
        actions[0].icon,
        Some(IconName::Settings),
        "Model entries should have Settings icon"
    );
}

#[test]
fn new_chat_preset_uses_custom_icon() {
    let presets = vec![NewChatPresetInfo {
        id: "code".to_string(),
        name: "Code".to_string(),
        icon: IconName::Code,
    }];
    let actions = get_new_chat_actions(&[], &presets, &[]);
    assert_eq!(
        actions[0].icon,
        Some(IconName::Code),
        "Preset should use its own icon"
    );
}

// =========================================================================
// Deeplink name edge cases
// =========================================================================

#[test]
fn deeplink_name_empty_string() {
    assert_eq!(to_deeplink_name(""), "_unnamed");
}

#[test]
fn deeplink_name_all_special_chars() {
    assert_eq!(to_deeplink_name("!@#$%^&*()"), "_unnamed");
}

#[test]
fn deeplink_name_leading_trailing_spaces() {
    assert_eq!(to_deeplink_name("  My Script  "), "my-script");
}

#[test]
fn deeplink_name_consecutive_separators() {
    assert_eq!(to_deeplink_name("a--b__c  d"), "a-b-c-d");
}

#[test]
fn deeplink_name_single_char() {
    assert_eq!(to_deeplink_name("X"), "x");
}

#[test]
fn deeplink_name_numbers() {
    assert_eq!(to_deeplink_name("Script 123"), "script-123");
}

// =========================================================================
// Agent with shortcut + alias + frecency
// =========================================================================

#[test]
fn agent_with_shortcut_alias_frecency() {
    let mut agent = ScriptInfo::with_shortcut_and_alias(
        "code-agent",
        "/agents/code.md",
        Some("cmd+shift+a".to_string()),
        Some("ca".to_string()),
    );
    agent.is_agent = true;
    agent.is_script = false;
    let agent = agent.with_frecency(true, Some("/agents/code.md".to_string()));

    let actions = get_script_context_actions(&agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Agents get no shortcut/alias preference actions
    // (ScriptContextPreferenceActionPlan::AgentNoPreferenceActions).
    assert!(!ids.contains(&"update_shortcut"));
    assert!(!ids.contains(&"remove_shortcut"));
    assert!(!ids.contains(&"update_alias"));
    assert!(!ids.contains(&"remove_alias"));
    assert!(ids.contains(&"reset_ranking"));

    // Agent-specific actions
    assert!(ids.contains(&"edit_script")); // title says "Edit Agent"
    assert!(ids.contains(&"reveal_in_finder"));
    assert!(ids.contains(&"copy_path"));
    assert!(ids.contains(&"copy_content"));
    assert!(ids.contains(&"copy_deeplink"));

    // Should NOT have script-only or add variants
    assert!(!ids.contains(&"view_logs"));
    assert!(!ids.contains(&"add_shortcut"));
    assert!(!ids.contains(&"add_alias"));

    // Verify edit title
    let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
}

// =========================================================================
// All built-in action IDs use snake_case
// =========================================================================

#[test]
fn all_script_action_ids_are_snake_case() {
    let script =
        ScriptInfo::new("test", "/path/test.ts").with_frecency(true, Some("/path".to_string()));
    let actions = get_script_context_actions(&script);

    for action in &actions {
        assert!(
            !action.id.contains(' '),
            "Action ID '{}' should not contain spaces",
            action.id
        );
        assert!(
            !action.id.contains('-'),
            "Action ID '{}' should use underscores, not hyphens",
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
fn all_clipboard_action_ids_are_snake_case() {
    let entry = ClipboardEntryInfo {
        id: "test".to_string(),
        content_type: ContentType::Image,
        pinned: true,
        preview: "img".to_string(),
        image_dimensions: Some((100, 100)),
        frontmost_app_name: Some("App".to_string()),
    };
    let actions = get_clipboard_history_context_actions(&entry);

    for action in &actions {
        assert!(
            !action.id.contains(' '),
            "Clipboard action ID '{}' should not contain spaces",
            action.id
        );
        assert_eq!(
            action.id,
            action.id.to_lowercase(),
            "Clipboard action ID '{}' should be lowercase",
            action.id
        );
    }
}

// --- merged from part_03.rs ---

#[test]
fn all_file_action_ids_are_snake_case() {
    let file = FileInfo {
        name: "test.txt".to_string(),
        path: "/test.txt".to_string(),
        is_dir: false,
        file_type: crate::file_search::FileType::File,
    };
    let actions = get_file_context_actions(&file);

    for action in &actions {
        assert!(
            !action.id.contains(' '),
            "File action ID '{}' should not contain spaces",
            action.id
        );
        assert_eq!(
            action.id,
            action.id.to_lowercase(),
            "File action ID '{}' should be lowercase",
            action.id
        );
    }
}

// =========================================================================
// Path context: action count and common actions
// =========================================================================

#[test]
fn path_context_directory_has_all_expected_actions() {
    let path = PathInfo {
        name: "Downloads".to_string(),
        path: "/Users/test/Downloads".to_string(),
        is_dir: true,
    };
    let actions = get_path_context_actions(&path);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Directory should have: open_directory + 6 common
    assert!(ids.contains(&"file:open_directory"));
    assert!(ids.contains(&"file:copy_path"));
    assert!(ids.contains(&"file:open_in_finder"));
    assert!(ids.contains(&"file:open_in_editor"));
    assert!(ids.contains(&"file:open_in_quick_terminal"));
    assert!(ids.contains(&"file:copy_filename"));
    assert!(ids.contains(&"file:move_to_trash"));
    assert_eq!(actions.len(), 7);
}

#[test]
fn path_context_file_has_all_expected_actions() {
    let path = PathInfo {
        name: "doc.txt".to_string(),
        path: "/Users/test/doc.txt".to_string(),
        is_dir: false,
    };
    let actions = get_path_context_actions(&path);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // File should have: select_file + 6 common
    assert!(ids.contains(&"file:select_file"));
    assert!(ids.contains(&"file:copy_path"));
    assert!(ids.contains(&"file:open_in_finder"));
    assert!(ids.contains(&"file:open_in_editor"));
    assert!(ids.contains(&"file:open_in_quick_terminal"));
    assert!(ids.contains(&"file:copy_filename"));
    assert!(ids.contains(&"file:move_to_trash"));
    assert_eq!(actions.len(), 7);
}

#[test]
fn path_trash_description_folder_vs_file() {
    let dir_path = PathInfo {
        name: "Docs".to_string(),
        path: "/Docs".to_string(),
        is_dir: true,
    };
    let dir_actions = get_path_context_actions(&dir_path);
    let dir_trash = dir_actions
        .iter()
        .find(|a| a.id == "file:move_to_trash")
        .unwrap();
    assert_eq!(
        dir_trash.description,
        Some("Moves this folder to the Trash".to_string())
    );

    let file_path = PathInfo {
        name: "test.txt".to_string(),
        path: "/test.txt".to_string(),
        is_dir: false,
    };
    let file_actions = get_path_context_actions(&file_path);
    let file_trash = file_actions
        .iter()
        .find(|a| a.id == "file:move_to_trash")
        .unwrap();
    assert_eq!(
        file_trash.description,
        Some("Moves this file to the Trash".to_string())
    );
}

// =========================================================================
// File context: directory Quick Look exclusion
// =========================================================================

#[cfg(target_os = "macos")]
#[test]
fn file_directory_excludes_quick_look_includes_editor_terminal_trash() {
    let dir = FileInfo {
        name: "Folder".to_string(),
        path: "/Folder".to_string(),
        is_dir: true,
        file_type: crate::file_search::FileType::Directory,
    };
    let actions = get_file_context_actions(&dir);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    assert!(
        !ids.contains(&"file:quick_look"),
        "Dir should NOT have Quick Look"
    );
    assert!(ids.contains(&"file:open_in_editor"), "Dir should have Open in Editor");
    assert!(ids.contains(&"file:open_in_quick_terminal"), "Dir should have Open in Quick Terminal");
    assert!(ids.contains(&"file:move_to_trash"), "Dir should have Move to Trash");
    assert!(ids.contains(&"file:show_info"), "Dir should have Show Info");
}

// =========================================================================
// Scriptlet with custom actions: ordering guarantee
// =========================================================================

#[test]
fn scriptlet_custom_actions_appear_between_run_and_builtins() {
    use crate::scriptlets::{Scriptlet, ScriptletAction};

    let script = ScriptInfo::scriptlet("Test", "/path/test.md", None, None);
    let mut scriptlet = Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
    scriptlet.actions = vec![
        ScriptletAction {
            name: "Custom A".to_string(),
            command: "cmd-a".to_string(),
            tool: "bash".to_string(),
            code: "echo a".to_string(),
            inputs: vec![],
            shortcut: None,
            description: None,
        },
        ScriptletAction {
            name: "Custom B".to_string(),
            command: "cmd-b".to_string(),
            tool: "bash".to_string(),
            code: "echo b".to_string(),
            inputs: vec![],
            shortcut: Some("cmd+b".to_string()),
            description: None,
        },
    ];

    let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));

    // Find positions
    let run_pos = actions.iter().position(|a| a.id == "run_script").unwrap();
    let custom_a_pos = actions
        .iter()
        .position(|a| a.id == "scriptlet_action:cmd-a")
        .unwrap();
    let custom_b_pos = actions
        .iter()
        .position(|a| a.id == "scriptlet_action:cmd-b")
        .unwrap();
    let edit_pos = actions
        .iter()
        .position(|a| a.id == "edit_scriptlet")
        .unwrap();

    assert_eq!(run_pos, 0, "run_script must be first");
    assert!(custom_a_pos > run_pos, "Custom A after run");
    assert!(custom_b_pos > custom_a_pos, "Custom B after Custom A");
    assert!(
        edit_pos > custom_b_pos,
        "Built-in edit after custom actions"
    );

    // Custom actions should have has_action=true
    let ca = actions
        .iter()
        .find(|a| a.id == "scriptlet_action:cmd-a")
        .unwrap();
    assert!(ca.has_action, "Custom actions should have has_action=true");

    // Custom B should have shortcut formatted
    let cb = actions
        .iter()
        .find(|a| a.id == "scriptlet_action:cmd-b")
        .unwrap();
    assert_eq!(cb.shortcut, Some("⌘B".to_string()));
}

// =========================================================================
// ProtocolAction: edge case combinations
// =========================================================================

#[test]
fn protocol_action_with_all_fields() {
    let pa = ProtocolAction {
        name: "Full Action".to_string(),
        description: Some("A complete action".to_string()),
        shortcut: Some("cmd+shift+f".to_string()),
        value: Some("full-value".to_string()),
        has_action: true,
        visible: Some(true),
        close: Some(false),
    };

    assert_eq!(pa.name, "Full Action");
    assert_eq!(pa.description, Some("A complete action".to_string()));
    assert_eq!(pa.shortcut, Some("cmd+shift+f".to_string()));
    assert_eq!(pa.value, Some("full-value".to_string()));
    assert!(pa.has_action);
    assert!(pa.is_visible());
    assert!(!pa.should_close());
}

#[test]
fn protocol_action_hidden_but_closes() {
    let pa = ProtocolAction {
        name: "Hidden Closer".to_string(),
        description: None,
        shortcut: None,
        value: None,
        has_action: false,
        visible: Some(false),
        close: Some(true),
    };
    assert!(!pa.is_visible());
    assert!(pa.should_close());
}

// =========================================================================
// Score action: combined scoring
// =========================================================================

#[test]
fn score_action_title_prefix_plus_description_match() {
    let action = Action::new(
        "file:copy_path",
        "Copy Path",
        Some("Copy the full path to clipboard".to_string()),
        ActionCategory::ScriptContext,
    );
    let score = ActionsDialog::score_action(&action, "copy");
    // Prefix match (100) + description contains "copy" (15) = 115
    assert_eq!(score, 115, "Prefix + description match should score 115");
}

#[test]
fn score_action_title_contains_plus_shortcut_match() {
    let action = Action::new(
        "script:reveal",
        "Reveal in Finder",
        None,
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘⇧F");
    let score = ActionsDialog::score_action(&action, "f");
    // Contains "f" in "reveal in finder" (50) + shortcut contains "f" in "⌘⇧f" (10) = 60
    assert_eq!(
        score, 60,
        "Contains + shortcut match should score 60, got {}",
        score
    );
}

#[test]
fn score_action_no_match_returns_zero() {
    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
    let score = ActionsDialog::score_action(&action, "xyz");
    assert_eq!(score, 0);
}

// =========================================================================
// ScriptInfo constructor invariants
// =========================================================================

#[test]
fn script_info_new_always_is_script_true() {
    let s = ScriptInfo::new("a", "/a");
    assert!(s.is_script);
    assert!(!s.is_scriptlet);
    assert!(!s.is_agent);
}

#[test]
fn script_info_builtin_never_is_script() {
    let s = ScriptInfo::builtin("X");
    assert!(!s.is_script);
    assert!(!s.is_scriptlet);
    assert!(!s.is_agent);
}

#[test]
fn script_info_scriptlet_never_is_script() {
    let s = ScriptInfo::scriptlet("X", "/x.md", None, None);
    assert!(!s.is_script);
    assert!(s.is_scriptlet);
    assert!(!s.is_agent);
}

#[test]
fn script_info_default_action_verb_is_run() {
    assert_eq!(ScriptInfo::new("a", "/a").action_verb, "Run");
    assert_eq!(ScriptInfo::builtin("X").action_verb, "Run");
    assert_eq!(
        ScriptInfo::scriptlet("X", "/x.md", None, None).action_verb,
        "Run"
    );
    assert_eq!(
        ScriptInfo::with_shortcut("X", "/x", None).action_verb,
        "Run"
    );
}

// =========================================================================
// No duplicate IDs: scriptlet context, path context, file context
// =========================================================================

#[test]
fn no_duplicate_ids_in_scriptlet_context() {
    let script = ScriptInfo::scriptlet(
        "Test",
        "/test.md",
        Some("cmd+t".to_string()),
        Some("ts".to_string()),
    )
    .with_frecency(true, Some("s:Test".to_string()));

    let actions = get_scriptlet_context_actions_with_custom(&script, None);
    let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "Scriptlet context should have no dups");
}

#[test]
fn no_duplicate_ids_in_path_context() {
    let path = PathInfo {
        name: "test".to_string(),
        path: "/test".to_string(),
        is_dir: true,
    };
    let actions = get_path_context_actions(&path);
    let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "Path context should have no dups");
}

#[test]
fn no_duplicate_ids_in_file_context() {
    let file = FileInfo {
        name: "f.txt".to_string(),
        path: "/f.txt".to_string(),
        is_dir: false,
        file_type: crate::file_search::FileType::File,
    };
    let actions = get_file_context_actions(&file);
    let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "File context should have no dups");
}

// =========================================================================
// All actions have categories
// =========================================================================

#[test]
fn all_script_actions_use_script_context_category() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
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
fn all_clipboard_actions_use_script_context_category() {
    let entry = ClipboardEntryInfo {
        id: "t".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "t".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&entry);
    for action in &actions {
        assert_eq!(
            action.category,
            ActionCategory::ScriptContext,
            "Clipboard action '{}' should be ScriptContext",
            action.id
        );
    }
}

// =========================================================================
// Clipboard: text vs image action count difference
// =========================================================================

#[test]
fn clipboard_image_has_more_actions_than_text() {
    let text = ClipboardEntryInfo {
        id: "t".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "txt".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };
    let img = ClipboardEntryInfo {
        id: "i".to_string(),
        content_type: ContentType::Image,
        pinned: false,
        preview: "img".to_string(),
        image_dimensions: Some((100, 100)),
        frontmost_app_name: None,
    };

    let text_actions = get_clipboard_history_context_actions(&text);
    let img_actions = get_clipboard_history_context_actions(&img);

    assert!(
        img_actions.len() > text_actions.len(),
        "Image should have more actions ({}) than text ({})",
        img_actions.len(),
        text_actions.len()
    );
}
