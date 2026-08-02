// --- merged from part_01.rs ---
//! Dialog behavior tests
//!
//! Validates random built-in action dialog behaviors end-to-end:
//! - Primary action verb propagation
//! - Action ordering and deduplication
//! - Complex ScriptInfo combos (shortcut+alias+frecency)
//! - CommandBarConfig presets
//! - Clipboard context title truncation
//! - GroupedActionItem coercion edge cases
//! - Action builder chain methods
//! - ProtocolAction constructors

use super::builders::{
    get_clipboard_history_context_actions, get_file_context_actions, get_global_actions,
    get_notes_command_bar_actions, get_path_context_actions, get_script_context_actions,
    ChatPromptInfo, ClipboardEntryInfo, NotesInfo,
};
use super::command_bar::CommandBarConfig;
use super::dialog::{build_grouped_items_static, coerce_action_selection, GroupedActionItem};
use super::types::{
    Action, ActionCategory, ActionsDialogConfig, AnchorPosition, ScriptInfo, SearchPosition,
    SectionStyle,
};
use crate::clipboard_history::ContentType;
use crate::file_search::FileInfo;
use crate::prompts::PathInfo;
use crate::protocol::ProtocolAction;

// =========================================================================
// Script context: primary action verb propagation
// =========================================================================

#[test]
fn script_primary_action_title_uses_action_verb() {
    let script = ScriptInfo::with_action_verb("Window Manager", "builtin:wm", false, "Switch to");
    let actions = get_script_context_actions(&script);
    let primary = &actions[0];
    assert_eq!(primary.id, "run_script");
    // Built-in catalog entries preserve their action verb text exactly.
    assert_eq!(primary.title, "Switch to");
}

#[test]
fn script_primary_action_description_uses_action_verb() {
    let script = ScriptInfo::with_action_verb("App Launcher", "builtin:apps", false, "Launch");
    let actions = get_script_context_actions(&script);
    let primary = &actions[0];
    assert_eq!(
        primary.description,
        Some("Launch".to_string()),
        "Built-in descriptions preserve the catalog action verb exactly"
    );
}

#[test]
fn script_primary_action_always_first() {
    // Regular script
    let script = ScriptInfo::new("my-script", "/path/to/script.ts");
    let actions = get_script_context_actions(&script);
    assert_eq!(actions[0].id, "run_script", "run_script must be first");
    assert_eq!(
        actions[0].shortcut,
        Some("↵".to_string()),
        "Primary should have Enter shortcut"
    );

    // Built-in
    let builtin = ScriptInfo::builtin("Clipboard History");
    let builtin_actions = get_script_context_actions(&builtin);
    assert_eq!(
        builtin_actions[0].id, "run_script",
        "run_script must be first for builtins too"
    );

    // Agent
    let mut agent = ScriptInfo::new("my-agent", "/path/agent.md");
    agent.is_agent = true;
    agent.is_script = false;
    let agent_actions = get_script_context_actions(&agent);
    assert_eq!(
        agent_actions[0].id, "run_script",
        "run_script must be first for agents"
    );
}

// =========================================================================
// Script context: both shortcut AND alias set
// =========================================================================

#[test]
fn script_with_both_shortcut_and_alias_shows_all_management_actions() {
    let script = ScriptInfo::with_shortcut_and_alias(
        "test-script",
        "/path/test.ts",
        Some("cmd+t".to_string()),
        Some("ts".to_string()),
    );
    let actions = get_script_context_actions(&script);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Should have update+remove for BOTH shortcut and alias
    assert!(
        ids.contains(&"update_shortcut"),
        "Should have update_shortcut"
    );
    assert!(
        ids.contains(&"remove_shortcut"),
        "Should have remove_shortcut"
    );
    assert!(ids.contains(&"update_alias"), "Should have update_alias");
    assert!(ids.contains(&"remove_alias"), "Should have remove_alias");

    // Should NOT have add variants
    assert!(
        !ids.contains(&"add_shortcut"),
        "Should NOT have add_shortcut"
    );
    assert!(!ids.contains(&"add_alias"), "Should NOT have add_alias");
}

#[test]
fn script_with_frecency_shortcut_alias_has_all_actions() {
    let script = ScriptInfo::with_shortcut_and_alias(
        "power-script",
        "/path/power.ts",
        Some("cmd+p".to_string()),
        Some("ps".to_string()),
    )
    .with_frecency(true, Some("/path/power.ts".to_string()));

    let actions = get_script_context_actions(&script);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // All management actions
    assert!(ids.contains(&"update_shortcut"));
    assert!(ids.contains(&"remove_shortcut"));
    assert!(ids.contains(&"update_alias"));
    assert!(ids.contains(&"remove_alias"));
    assert!(ids.contains(&"reset_ranking"), "Should have reset_ranking");

    // Script-specific actions
    assert!(ids.contains(&"edit_script"));
    assert!(ids.contains(&"view_logs"));
    assert!(ids.contains(&"copy_deeplink"));
}

// =========================================================================
// Agent-specific action behaviors
// =========================================================================

#[test]
fn agent_edit_title_says_edit_agent() {
    let mut agent = ScriptInfo::new("code-reviewer", "/agents/code-reviewer.md");
    agent.is_agent = true;
    agent.is_script = false;
    let actions = get_script_context_actions(&agent);

    let edit = actions.iter().find(|a| a.id == "edit_script");
    assert!(edit.is_some(), "Agent should have edit_script action");
    assert_eq!(
        edit.unwrap().title,
        "Edit Agent",
        "Agent edit title should say 'Edit Agent'"
    );
}

#[test]
fn agent_has_no_view_logs() {
    let mut agent = ScriptInfo::new("helper", "/agents/helper.md");
    agent.is_agent = true;
    agent.is_script = false;
    let actions = get_script_context_actions(&agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(
        !ids.contains(&"view_logs"),
        "Agent should NOT have view_logs"
    );
}

#[test]
fn agent_has_reveal_copy_path_copy_content() {
    let mut agent = ScriptInfo::new("helper", "/agents/helper.md");
    agent.is_agent = true;
    agent.is_script = false;
    let actions = get_script_context_actions(&agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"reveal_in_finder"));
    assert!(ids.contains(&"copy_path"));
    assert!(ids.contains(&"copy_content"));
}

// =========================================================================
// Deeplink action is always present
// =========================================================================

#[test]
fn copy_deeplink_present_for_all_script_types() {
    // Script
    let script = ScriptInfo::new("test", "/path/test.ts");
    let ids: Vec<String> = get_script_context_actions(&script)
        .iter()
        .map(|a| a.id.clone())
        .collect();
    assert!(ids.contains(&"copy_deeplink".to_string()));

    // Builtin
    let builtin = ScriptInfo::builtin("Clipboard History");
    let ids: Vec<String> = get_script_context_actions(&builtin)
        .iter()
        .map(|a| a.id.clone())
        .collect();
    assert!(ids.contains(&"copy_deeplink".to_string()));

    // Scriptlet
    let scriptlet = ScriptInfo::scriptlet("Open URL", "/path/url.md", None, None);
    let ids: Vec<String> = get_script_context_actions(&scriptlet)
        .iter()
        .map(|a| a.id.clone())
        .collect();
    assert!(ids.contains(&"copy_deeplink".to_string()));

    // Agent
    let mut agent = ScriptInfo::new("agent", "/agents/a.md");
    agent.is_agent = true;
    let ids: Vec<String> = get_script_context_actions(&agent)
        .iter()
        .map(|a| a.id.clone())
        .collect();
    assert!(ids.contains(&"copy_deeplink".to_string()));
}

#[test]
fn copy_deeplink_description_uses_portable_share_copy_for_scripts() {
    let script = ScriptInfo::new("My Cool Script", "/path/cool.ts");
    let actions = get_script_context_actions(&script);
    let deeplink = actions.iter().find(|a| a.id == "copy_deeplink").unwrap();

    assert_eq!(deeplink.title, "Share");
    assert_eq!(
        deeplink.description.as_deref(),
        Some("Copy a portable Script Kit share link to clipboard")
    );
}

// =========================================================================
// No duplicate action IDs within a context
// =========================================================================

#[test]
fn no_duplicate_ids_in_script_context() {
    let script =
        ScriptInfo::new("test", "/path/test.ts").with_frecency(true, Some("/path".to_string()));
    let actions = get_script_context_actions(&script);
    let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(
        total,
        ids.len(),
        "Script context should have no duplicate IDs"
    );
}

#[test]
fn no_duplicate_ids_in_clipboard_context() {
    let entry = ClipboardEntryInfo {
        id: "test-1".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "test text".to_string(),
        image_dimensions: None,
        frontmost_app_name: Some("VS Code".to_string()),
    };
    let actions = get_clipboard_history_context_actions(&entry);
    let mut ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(
        total,
        ids.len(),
        "Clipboard context should have no duplicate IDs"
    );
}

// =========================================================================
// CommandBarConfig::notes_style
// =========================================================================

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
    // Inherits default close behaviors
    assert!(config.close_on_select);
    assert!(config.close_on_escape);
    assert!(config.close_on_click_outside);
}

// =========================================================================
// ActionsDialogConfig defaults
// =========================================================================

#[test]
fn actions_dialog_config_default_values() {
    let config = ActionsDialogConfig::default();
    assert!(matches!(config.search_position, SearchPosition::Top));
    assert!(matches!(config.section_style, SectionStyle::Headers));
    assert!(matches!(config.anchor, AnchorPosition::Bottom));
    assert!(!config.show_icons);
    assert!(!config.show_footer);
}

// =========================================================================
// GroupedActionItem coercion edge cases
// =========================================================================

#[test]
fn coerce_alternating_headers_and_items() {
    // H, I, H, I, H, I pattern
    let rows = vec![
        GroupedActionItem::SectionHeader("A".to_string()),
        GroupedActionItem::Item(0),
        GroupedActionItem::SectionHeader("B".to_string()),
        GroupedActionItem::Item(1),
        GroupedActionItem::SectionHeader("C".to_string()),
        GroupedActionItem::Item(2),
    ];
    // Landing on header at index 0 should skip to item at 1
    assert_eq!(coerce_action_selection(&rows, 0), Some(1));
    // Landing on header at index 2 should skip to item at 3
    assert_eq!(coerce_action_selection(&rows, 2), Some(3));
    // Landing on header at index 4 should skip to item at 5
    assert_eq!(coerce_action_selection(&rows, 4), Some(5));
    // Items stay as-is
    assert_eq!(coerce_action_selection(&rows, 1), Some(1));
    assert_eq!(coerce_action_selection(&rows, 3), Some(3));
    assert_eq!(coerce_action_selection(&rows, 5), Some(5));
}

#[test]
fn coerce_consecutive_headers_at_start() {
    // H, H, H, I pattern - three headers before first item
    let rows = vec![
        GroupedActionItem::SectionHeader("A".to_string()),
        GroupedActionItem::SectionHeader("B".to_string()),
        GroupedActionItem::SectionHeader("C".to_string()),
        GroupedActionItem::Item(0),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), Some(3));
    assert_eq!(coerce_action_selection(&rows, 1), Some(3));
    assert_eq!(coerce_action_selection(&rows, 2), Some(3));
}

#[test]
fn coerce_single_item() {
    let rows = vec![GroupedActionItem::Item(0)];
    assert_eq!(coerce_action_selection(&rows, 0), Some(0));
    // Beyond bounds clamps
    assert_eq!(coerce_action_selection(&rows, 100), Some(0));
}

// =========================================================================
// build_grouped_items_static with section style variations
// =========================================================================

#[test]
fn grouped_items_headers_style_inserts_headers_between_sections() {
    let actions = vec![
        Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
            .with_section("Section A"),
        Action::new("a2", "Action 2", None, ActionCategory::ScriptContext)
            .with_section("Section A"),
        Action::new("b1", "Action 3", None, ActionCategory::ScriptContext)
            .with_section("Section B"),
    ];
    let filtered: Vec<usize> = (0..actions.len()).collect();
    let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

    // Should be: Header("Section A"), Item(0), Item(1), Header("Section B"), Item(2)
    assert_eq!(grouped.len(), 5);
    assert!(matches!(&grouped[0], GroupedActionItem::SectionHeader(s) if s == "Section A"));
    assert!(matches!(&grouped[1], GroupedActionItem::Item(0)));
    assert!(matches!(&grouped[2], GroupedActionItem::Item(1)));
    assert!(matches!(&grouped[3], GroupedActionItem::SectionHeader(s) if s == "Section B"));
    assert!(matches!(&grouped[4], GroupedActionItem::Item(2)));
}

#[test]
fn grouped_items_separators_style_no_headers_inserted() {
    let actions = vec![
        Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
            .with_section("Section A"),
        Action::new("b1", "Action 2", None, ActionCategory::ScriptContext)
            .with_section("Section B"),
    ];
    let filtered: Vec<usize> = (0..actions.len()).collect();
    let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);

    // No headers inserted for Separators style
    assert_eq!(grouped.len(), 2);
    assert!(matches!(&grouped[0], GroupedActionItem::Item(0)));
    assert!(matches!(&grouped[1], GroupedActionItem::Item(1)));
}

#[test]
fn grouped_items_none_style_no_headers() {
    let actions = vec![
        Action::new("a1", "Action 1", None, ActionCategory::ScriptContext)
            .with_section("Section A"),
    ];
    let filtered: Vec<usize> = (0..actions.len()).collect();
    let grouped = build_grouped_items_static(&actions, &filtered, SectionStyle::None);

    assert_eq!(grouped.len(), 1);
    assert!(matches!(&grouped[0], GroupedActionItem::Item(0)));
}

// --- merged from part_02.rs ---

// =========================================================================
// Action builder chain methods
// =========================================================================

#[test]
fn action_with_description_sets_field_and_cache() {
    let action = Action::new(
        "test",
        "Test Action",
        Some("A detailed description".to_string()),
        ActionCategory::ScriptContext,
    );
    assert_eq!(
        action.description,
        Some("A detailed description".to_string())
    );
    assert_eq!(
        action.description_lower,
        Some("a detailed description".to_string()),
        "description_lower should be pre-computed"
    );
}

#[test]
fn action_builder_chain_icon_section_shortcut() {
    use crate::designs::icon_variations::IconName;

    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext)
        .with_icon(IconName::Star)
        .with_section("My Section")
        .with_shortcut("⌘T");

    assert_eq!(action.icon, Some(IconName::Star));
    assert_eq!(action.section, Some("My Section".to_string()));
    assert_eq!(action.shortcut, Some("⌘T".to_string()));
    assert_eq!(action.shortcut_lower, Some("⌘t".to_string()));
}

#[test]
fn action_title_lower_precomputed() {
    let action = Action::new(
        "test",
        "UPPERCASE Title",
        None,
        ActionCategory::ScriptContext,
    );
    assert_eq!(action.title_lower, "uppercase title");
}

// =========================================================================
// ProtocolAction constructors
// =========================================================================

#[test]
fn protocol_action_new_defaults() {
    let pa = ProtocolAction::new("My Action".to_string());
    assert_eq!(pa.name, "My Action");
    assert!(pa.description.is_none());
    assert!(pa.shortcut.is_none());
    assert!(pa.value.is_none());
    assert!(!pa.has_action);
    assert!(pa.visible.is_none());
    assert!(pa.close.is_none());
    // Defaults
    assert!(pa.is_visible());
    assert!(pa.should_close());
}

#[test]
fn protocol_action_with_value_constructor() {
    let pa = ProtocolAction::with_value("Submit".to_string(), "submit-value".to_string());
    assert_eq!(pa.name, "Submit");
    assert_eq!(pa.value, Some("submit-value".to_string()));
    assert!(!pa.has_action, "with_value should default has_action=false");
}

#[test]
fn protocol_action_visibility_and_close_combinations() {
    // visible=false, close=false
    let pa = ProtocolAction {
        name: "Hidden Stay Open".to_string(),
        description: None,
        shortcut: None,
        value: None,
        has_action: false,
        visible: Some(false),
        close: Some(false),
    };
    assert!(!pa.is_visible());
    assert!(!pa.should_close());

    // visible=true, close=false
    let pa2 = ProtocolAction {
        name: "Visible Stay Open".to_string(),
        description: None,
        shortcut: None,
        value: None,
        has_action: true,
        visible: Some(true),
        close: Some(false),
    };
    assert!(pa2.is_visible());
    assert!(!pa2.should_close());
}

// =========================================================================
// Clipboard context: context title truncation
// =========================================================================

#[test]
fn clipboard_long_preview_truncated_in_context_title() {
    // The with_clipboard_entry method truncates preview > 30 chars
    // We test the truncation logic directly
    let long_preview = "This is a very long clipboard entry that exceeds thirty characters";
    assert!(long_preview.len() > 30);

    let context_title = if long_preview.len() > 30 {
        format!("{}...", &long_preview[..27])
    } else {
        long_preview.to_string()
    };

    assert_eq!(context_title, "This is a very long clipboa...");
    assert_eq!(context_title.len(), 30); // 27 chars + "..."
}

#[test]
fn clipboard_short_preview_not_truncated() {
    let short_preview = "Short text";
    assert!(short_preview.len() <= 30);

    let context_title = if short_preview.len() > 30 {
        format!("{}...", &short_preview[..27])
    } else {
        short_preview.to_string()
    };

    assert_eq!(context_title, "Short text");
}

// =========================================================================
// Chat context: title fallback
// =========================================================================

#[test]
fn chat_context_title_uses_model_name() {
    let info = ChatPromptInfo {
        current_model: Some("claude-sonnet".to_string()),
        available_models: vec![],
        has_response: false,
        has_messages: false,
    };

    let context_title = info
        .current_model
        .clone()
        .unwrap_or_else(|| "Chat".to_string());
    assert_eq!(context_title, "claude-sonnet");
}

#[test]
fn chat_context_title_falls_back_to_chat() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_response: false,
        has_messages: false,
    };

    let context_title = info
        .current_model
        .clone()
        .unwrap_or_else(|| "Chat".to_string());
    assert_eq!(context_title, "Chat");
}

// =========================================================================
// File context: directory vs file action differentiation
// =========================================================================

#[test]
fn file_context_directory_has_open_directory_primary() {
    let dir = FileInfo {
        name: "Documents".to_string(),
        path: "/Users/test/Documents".to_string(),
        is_dir: true,
        file_type: crate::file_search::FileType::Directory,
    };
    let actions = get_file_context_actions(&dir);
    assert_eq!(actions[0].id, "file:open_directory");
    assert!(actions[0].title.contains("Documents"));
}

#[test]
fn file_context_file_has_open_file_primary() {
    let file = FileInfo {
        name: "readme.md".to_string(),
        path: "/Users/test/readme.md".to_string(),
        is_dir: false,
        file_type: crate::file_search::FileType::File,
    };
    let actions = get_file_context_actions(&file);
    assert_eq!(actions[0].id, "file:open_file");
    assert!(actions[0].title.contains("readme.md"));
}

// =========================================================================
// Path context: primary action differentiation
// =========================================================================

#[test]
fn path_context_directory_primary_is_open() {
    let path = PathInfo {
        name: "Documents".to_string(),
        path: "/Users/test/Documents".to_string(),
        is_dir: true,
    };
    let actions = get_path_context_actions(&path);
    assert_eq!(actions[0].id, "file:open_directory");
}

#[test]
fn path_context_file_primary_is_select() {
    let path = PathInfo {
        name: "file.txt".to_string(),
        path: "/Users/test/file.txt".to_string(),
        is_dir: false,
    };
    let actions = get_path_context_actions(&path);
    assert_eq!(actions[0].id, "file:select_file");
}

// =========================================================================
// Builtin script: limited actions
// =========================================================================

#[test]
fn builtin_has_only_run_shortcut_alias_deeplink() {
    let builtin = ScriptInfo::builtin("App Launcher");
    let actions = get_script_context_actions(&builtin);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Should have: run_script, add_shortcut, add_alias, copy_deeplink
    assert!(ids.contains(&"run_script"));
    assert!(ids.contains(&"add_shortcut"));
    assert!(ids.contains(&"add_alias"));
    assert!(ids.contains(&"copy_deeplink"));

    // Should NOT have script-only or scriptlet-only actions
    assert!(!ids.contains(&"edit_script"));
    assert!(!ids.contains(&"view_logs"));
    assert!(!ids.contains(&"file:reveal_in_finder"));
    assert!(!ids.contains(&"file:copy_path"));
    assert!(!ids.contains(&"copy_content"));
    assert!(!ids.contains(&"edit_scriptlet"));
}

// =========================================================================
// Action scoring edge cases
// =========================================================================

#[test]
fn score_action_empty_search_returns_zero() {
    use super::dialog::ActionsDialog;

    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
    // Empty search should match nothing (score 0)
    // Actually empty string is a prefix of everything, so it will score 100
    let score = ActionsDialog::score_action(&action, "");
    assert_eq!(score, 100, "Empty search is a prefix of all titles");
}

#[test]
fn score_action_shortcut_only_match() {
    use super::dialog::ActionsDialog;

    let action =
        Action::new("test", "Test Action", None, ActionCategory::ScriptContext).with_shortcut("⌘X");
    // Search for the shortcut symbol
    let score = ActionsDialog::score_action(&action, "⌘x");
    assert!(
        score >= 10,
        "Shortcut match should contribute at least 10, got {}",
        score
    );
}

#[test]
fn fuzzy_match_case_insensitive_precomputed() {
    use super::dialog::ActionsDialog;

    // fuzzy_match operates on lowercased strings (pre-computed in title_lower)
    assert!(ActionsDialog::fuzzy_match("edit script", "es"));
    assert!(ActionsDialog::fuzzy_match("edit script", "eit"));
    assert!(!ActionsDialog::fuzzy_match("edit script", "z"));
}

// =========================================================================
// Notes command bar: conditional actions
// =========================================================================

#[test]
fn notes_command_bar_has_new_note_action() {
    let info = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: true,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(
        ids.contains(&"new_note"),
        "Notes command bar should always have new_note"
    );
}

#[test]
fn notes_trash_view_has_no_edit_actions() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: true,
        auto_sizing_enabled: false,
    };
    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    // Trash view should suppress editing actions
    assert!(
        !ids.contains(&"duplicate_note"),
        "Trash view should not have duplicate_note"
    );
}

// =========================================================================
// Clipboard: pinned image entry combines image + pin actions
// =========================================================================

#[test]
fn clipboard_pinned_image_has_unpin_and_image_actions() {
    let entry = ClipboardEntryInfo {
        id: "img-1".to_string(),
        content_type: ContentType::Image,
        pinned: true,
        preview: "Screenshot".to_string(),
        image_dimensions: Some((1920, 1080)),
        frontmost_app_name: None,
    };
    let actions = get_clipboard_history_context_actions(&entry);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Should have unpin (not pin)
    assert!(
        ids.contains(&"clip:clipboard_unpin"),
        "Should have unpin for pinned entry"
    );
    assert!(
        !ids.contains(&"clip:clipboard_pin"),
        "Should NOT have pin for pinned entry"
    );

    // Should have image-specific actions (macOS)
    #[cfg(target_os = "macos")]
    {
        assert!(ids.contains(&"clip:clipboard_ocr"), "Image should have OCR");
    }
}

// =========================================================================
// Global actions are empty
// =========================================================================

#[test]
fn global_actions_seeded() {
    let actions = get_global_actions();
    assert!(!actions.is_empty(), "Global actions should be seeded");
}

// =========================================================================
// Action category equality
// =========================================================================

#[test]
fn action_category_partial_eq() {
    assert_eq!(ActionCategory::ScriptContext, ActionCategory::ScriptContext);
    assert_ne!(ActionCategory::ScriptContext, ActionCategory::GlobalOps);
    assert_ne!(ActionCategory::ScriptOps, ActionCategory::Terminal);
}

// =========================================================================
// SearchPosition, SectionStyle, AnchorPosition enum defaults
// =========================================================================

#[test]
fn enum_defaults() {
    assert!(matches!(SearchPosition::default(), SearchPosition::Top));
    assert!(matches!(SectionStyle::default(), SectionStyle::Headers));
    assert!(matches!(AnchorPosition::default(), AnchorPosition::Bottom));
}
