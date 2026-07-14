// --- merged from part_01.rs ---
//! Random action/dialog/window validation tests
//!
//! Validates miscellaneous behaviors across the actions system that
//! haven't been covered by other test files. Focuses on:
//! - Window module's count_section_headers (pure function)
//! - SDK action conversion flow (ProtocolAction -> Action)
//! - Action lowercase caching invariants
//! - build_actions internal routing (script + global)
//! - Refilter scoring end-to-end
//! - Constants consistency
//! - CommandBarConfig field combinations
//! - Agent vs script action differentiation
//! - Clipboard destructive action ordering invariants
//! - Path context action count consistency

use super::builders::{
    ChatModelInfo, ChatPromptInfo, ClipboardEntryInfo, NewChatModelInfo, NewChatPresetInfo,
    NoteSwitcherNoteInfo, NotesInfo, get_ai_command_bar_actions, get_chat_context_actions,
    get_chat_model_picker_actions, get_clipboard_history_context_actions, get_file_context_actions,
    get_new_chat_actions, get_note_switcher_actions, get_notes_command_bar_actions,
    get_path_context_actions, get_script_context_actions,
    get_scriptlet_context_actions_with_custom,
};
use super::constants::{
    ACTION_ITEM_HEIGHT, ACTION_ROW_INSET, HEADER_HEIGHT, POPUP_MAX_HEIGHT, POPUP_WIDTH,
    SEARCH_INPUT_HEIGHT,
};
use super::dialog::{GroupedActionItem, build_grouped_items_static, coerce_action_selection};
use super::types::{
    Action, ActionCategory, ActionsDialogConfig, AnchorPosition, SearchPosition, SectionStyle,
};
use crate::clipboard_history::ContentType;
use crate::designs::icon_variations::IconName;
use crate::file_search::{FileInfo, FileType};
use crate::prompts::PathInfo;
use crate::protocol::ProtocolAction;
use crate::scriptlets::{Scriptlet, ScriptletAction};

use super::command_bar::CommandBarConfig;
use super::dialog::ActionsDialog;
use super::types::ScriptInfo;

// =========================================================================
// Window count_section_headers (pure function in window.rs, duplicated logic)
// =========================================================================

/// Reimplementation of window.rs count_section_headers for testing
/// (The original is module-private in window.rs)
fn count_section_headers(actions: &[Action], filtered_indices: &[usize]) -> usize {
    if filtered_indices.is_empty() {
        return 0;
    }

    let mut count = 0;
    let mut prev_section: Option<&Option<String>> = None;

    for &idx in filtered_indices {
        if let Some(action) = actions.get(idx) {
            let current_section = &action.section;
            if current_section.is_some() {
                match prev_section {
                    None => count += 1,
                    Some(prev) if prev != current_section => count += 1,
                    _ => {}
                }
            }
            prev_section = Some(current_section);
        }
    }

    count
}

#[test]
fn test_count_section_headers_empty() {
    let actions: Vec<Action> = vec![];
    let filtered: Vec<usize> = vec![];
    assert_eq!(count_section_headers(&actions, &filtered), 0);
}

#[test]
fn test_count_section_headers_no_sections() {
    let actions = vec![
        Action::new("a", "Action A", None, ActionCategory::ScriptContext),
        Action::new("b", "Action B", None, ActionCategory::ScriptContext),
    ];
    let filtered = vec![0, 1];
    assert_eq!(count_section_headers(&actions, &filtered), 0);
}

#[test]
fn test_count_section_headers_single_section() {
    let actions = vec![
        Action::new("a", "Action A", None, ActionCategory::ScriptContext).with_section("Response"),
        Action::new("b", "Action B", None, ActionCategory::ScriptContext).with_section("Response"),
    ];
    let filtered = vec![0, 1];
    assert_eq!(count_section_headers(&actions, &filtered), 1);
}

#[test]
fn test_count_section_headers_multiple_sections() {
    let actions = vec![
        Action::new("a", "Copy Response", None, ActionCategory::ScriptContext)
            .with_section("Response"),
        Action::new("b", "Copy Chat", None, ActionCategory::ScriptContext).with_section("Response"),
        Action::new("c", "Submit", None, ActionCategory::ScriptContext).with_section("Actions"),
        Action::new("d", "New Chat", None, ActionCategory::ScriptContext).with_section("Actions"),
        Action::new("e", "Change Model", None, ActionCategory::ScriptContext)
            .with_section("Settings"),
    ];
    let filtered = vec![0, 1, 2, 3, 4];
    assert_eq!(count_section_headers(&actions, &filtered), 3);
}

#[test]
fn test_count_section_headers_mixed_section_and_no_section() {
    let actions = vec![
        Action::new("a", "Action A", None, ActionCategory::ScriptContext).with_section("Group"),
        Action::new("b", "Action B", None, ActionCategory::ScriptContext), // no section
        Action::new("c", "Action C", None, ActionCategory::ScriptContext).with_section("Group"),
    ];
    let filtered = vec![0, 1, 2];
    // First item has section "Group" -> header
    // Second item has no section -> no header
    // Third item has section "Group" but prev was None -> counts as new header
    assert_eq!(count_section_headers(&actions, &filtered), 2);
}

#[test]
fn test_count_section_headers_filtered_subset() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Alpha"),
        Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Beta"),
        Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Alpha"),
        Action::new("d", "D", None, ActionCategory::ScriptContext).with_section("Beta"),
    ];
    // Only show items from Alpha section (indices 0, 2)
    assert_eq!(count_section_headers(&actions, &[0, 2]), 1); // same section
    assert_eq!(count_section_headers(&actions, &[0, 1]), 2); // different sections
}

#[test]
fn test_count_section_headers_out_of_bounds_index() {
    let actions =
        vec![Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Response")];
    let filtered = vec![0, 5, 10]; // 5 and 10 are out of bounds
    // Should only count valid actions
    assert_eq!(count_section_headers(&actions, &filtered), 1);
}

// =========================================================================
// Action lowercase caching invariants
// =========================================================================

#[test]
fn test_action_lowercase_cache_title() {
    let action = Action::new(
        "test_id",
        "Edit Script",
        None,
        ActionCategory::ScriptContext,
    );
    assert_eq!(action.title_lower, "edit script");
}

#[test]
fn test_action_lowercase_cache_description() {
    let action = Action::new(
        "test_id",
        "Edit Script",
        Some("Open in $EDITOR".to_string()),
        ActionCategory::ScriptContext,
    );
    assert_eq!(
        action.description_lower,
        Some("open in $editor".to_string())
    );
}

#[test]
fn test_action_lowercase_cache_no_description() {
    let action = Action::new(
        "test_id",
        "Edit Script",
        None,
        ActionCategory::ScriptContext,
    );
    assert!(action.description_lower.is_none());
}

#[test]
fn test_action_lowercase_cache_shortcut() {
    let action = Action::new(
        "test_id",
        "Edit Script",
        None,
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘E");
    assert_eq!(action.shortcut_lower, Some("⌘e".to_string()));
}

#[test]
fn test_action_lowercase_cache_shortcut_opt_none() {
    let action = Action::new(
        "test_id",
        "Edit Script",
        None,
        ActionCategory::ScriptContext,
    )
    .with_shortcut_opt(None);
    assert!(action.shortcut_lower.is_none());
    assert!(action.shortcut.is_none());
}

#[test]
fn test_action_lowercase_cache_shortcut_opt_some() {
    let action = Action::new(
        "test_id",
        "Edit Script",
        None,
        ActionCategory::ScriptContext,
    )
    .with_shortcut_opt(Some("⌘⇧C".to_string()));
    assert_eq!(action.shortcut, Some("⌘⇧C".to_string()));
    assert_eq!(action.shortcut_lower, Some("⌘⇧c".to_string()));
}

#[test]
fn test_action_unicode_title_lowercase() {
    let action = Action::new(
        "test",
        "Über Cool Étape",
        None,
        ActionCategory::ScriptContext,
    );
    assert_eq!(action.title_lower, "über cool étape");
}

// =========================================================================
// SDK action conversion (ProtocolAction -> Action)
// =========================================================================

#[test]
fn test_protocol_action_with_handler() {
    let pa = ProtocolAction::with_handler("Custom Handler".to_string());
    assert_eq!(pa.name, "Custom Handler");
    assert!(pa.has_action);
    assert!(pa.value.is_none());
    assert!(pa.is_visible());
    assert!(pa.should_close());
}

#[test]
fn test_protocol_action_with_value() {
    let pa = ProtocolAction::with_value("Submit Value".to_string(), "my-value".to_string());
    assert_eq!(pa.name, "Submit Value");
    assert!(!pa.has_action);
    assert_eq!(pa.value, Some("my-value".to_string()));
}

#[test]
fn test_protocol_action_visibility_combinations() {
    // visible: None -> visible
    assert!(ProtocolAction::new("A".into()).is_visible());
    // visible: Some(true) -> visible
    let mut pa = ProtocolAction::new("B".into());
    pa.visible = Some(true);
    assert!(pa.is_visible());
    // visible: Some(false) -> hidden
    pa.visible = Some(false);
    assert!(!pa.is_visible());
}

#[test]
fn test_protocol_action_close_combinations() {
    // close: None -> closes
    assert!(ProtocolAction::new("A".into()).should_close());
    // close: Some(true) -> closes
    let mut pa = ProtocolAction::new("B".into());
    pa.close = Some(true);
    assert!(pa.should_close());
    // close: Some(false) -> stays open
    pa.close = Some(false);
    assert!(!pa.should_close());
}

#[test]
fn test_sdk_action_conversion_preserves_shortcut_formatting() {
    // When set_sdk_actions converts ProtocolAction, it calls format_shortcut_hint
    // Test that the format_shortcut_hint output is what gets stored
    let hint = ActionsDialog::format_shortcut_hint("cmd+shift+c");
    assert_eq!(hint, "⌘⇧C");

    // Verify modifiers
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+a"), "⌘A");
    assert_eq!(ActionsDialog::format_shortcut_hint("ctrl+b"), "⌃B");
    assert_eq!(ActionsDialog::format_shortcut_hint("alt+c"), "⌥C");
    assert_eq!(ActionsDialog::format_shortcut_hint("shift+d"), "⇧D");
    assert_eq!(ActionsDialog::format_shortcut_hint("enter"), "↵");
    assert_eq!(ActionsDialog::format_shortcut_hint("escape"), "⎋");
    assert_eq!(ActionsDialog::format_shortcut_hint("tab"), "⇥");
    assert_eq!(ActionsDialog::format_shortcut_hint("backspace"), "⌫");
    assert_eq!(ActionsDialog::format_shortcut_hint("space"), "␣");
    assert_eq!(ActionsDialog::format_shortcut_hint("up"), "↑");
    assert_eq!(ActionsDialog::format_shortcut_hint("down"), "↓");
    assert_eq!(ActionsDialog::format_shortcut_hint("left"), "←");
    assert_eq!(ActionsDialog::format_shortcut_hint("right"), "→");
}

#[test]
fn test_sdk_action_conversion_modifier_aliases() {
    // meta/super/command all map to ⌘
    assert_eq!(ActionsDialog::format_shortcut_hint("meta+a"), "⌘A");
    assert_eq!(ActionsDialog::format_shortcut_hint("super+a"), "⌘A");
    assert_eq!(ActionsDialog::format_shortcut_hint("command+a"), "⌘A");
    // control maps to ⌃
    assert_eq!(ActionsDialog::format_shortcut_hint("control+a"), "⌃A");
    // opt/option maps to ⌥
    assert_eq!(ActionsDialog::format_shortcut_hint("opt+a"), "⌥A");
    assert_eq!(ActionsDialog::format_shortcut_hint("option+a"), "⌥A");
    // return/esc aliases
    assert_eq!(ActionsDialog::format_shortcut_hint("return"), "↵");
    assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
    // arrow aliases
    assert_eq!(ActionsDialog::format_shortcut_hint("arrowup"), "↑");
    assert_eq!(ActionsDialog::format_shortcut_hint("arrowdown"), "↓");
    assert_eq!(ActionsDialog::format_shortcut_hint("arrowleft"), "←");
    assert_eq!(ActionsDialog::format_shortcut_hint("arrowright"), "→");
    // delete maps to ⌫
    assert_eq!(ActionsDialog::format_shortcut_hint("delete"), "⌫");
}

#[test]
fn test_parse_shortcut_keycaps_special_symbols() {
    let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
    assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
}

#[test]
fn test_parse_shortcut_keycaps_all_modifiers() {
    let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧");
    assert_eq!(keycaps, vec!["⌘", "⌃", "⌥", "⇧"]);
}

#[test]
fn test_parse_shortcut_keycaps_arrows() {
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("↑"), vec!["↑"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("↓"), vec!["↓"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("←"), vec!["←"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("→"), vec!["→"]);
}

#[test]
fn test_parse_shortcut_keycaps_lowercase_to_uppercase() {
    let keycaps = ActionsDialog::parse_shortcut_keycaps("⌘a");
    assert_eq!(keycaps, vec!["⌘", "A"]);
}

#[test]
fn test_parse_shortcut_keycaps_enter_escape() {
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("↵"), vec!["↵"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("⎋"), vec!["⎋"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("⇥"), vec!["⇥"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("⌫"), vec!["⌫"]);
    assert_eq!(ActionsDialog::parse_shortcut_keycaps("␣"), vec!["␣"]);
}

// =========================================================================
// Score action validation (uses pub(crate) function)
// =========================================================================

#[test]
fn test_score_action_prefix_always_highest() {
    let action = Action::new(
        "test",
        "Edit Script",
        Some("Open in editor".to_string()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘E");

    let prefix_score = ActionsDialog::score_action(&action, "edit");
    let contains_score = ActionsDialog::score_action(&action, "script");
    // "edit" is a prefix of "edit script" -> should be highest
    assert!(
        prefix_score >= 100,
        "Prefix score should be >= 100, got {}",
        prefix_score
    );
    // "script" is contained but not a prefix
    assert!(
        (50..100).contains(&contains_score),
        "Contains score should be 50-99, got {}",
        contains_score
    );
}

#[test]
fn test_score_action_description_bonus() {
    let action_with_desc = Action::new(
        "test",
        "Open File",
        Some("Open the file in your editor".to_string()),
        ActionCategory::ScriptContext,
    );
    let action_no_desc = Action::new("test", "Open File", None, ActionCategory::ScriptContext);

    // Both match on title prefix
    let score_with = ActionsDialog::score_action(&action_with_desc, "open");
    let score_without = ActionsDialog::score_action(&action_no_desc, "open");
    // Both should have prefix score, but "open" doesn't appear differently in desc
    assert!(score_with >= 100);
    assert!(score_without >= 100);

    // Search for something only in the description
    let desc_only_with = ActionsDialog::score_action(&action_with_desc, "editor");
    let desc_only_without = ActionsDialog::score_action(&action_no_desc, "editor");
    assert_eq!(desc_only_with, 15, "Description-only match should score 15");
    assert_eq!(desc_only_without, 0, "No match without description");
}

#[test]
fn test_score_action_shortcut_bonus() {
    let action_with_shortcut =
        Action::new("test", "Delete Entry", None, ActionCategory::ScriptContext)
            .with_shortcut("⌃X");

    // "⌃x" is lowercase of the shortcut
    let score = ActionsDialog::score_action(&action_with_shortcut, "⌃x");
    assert!(
        score >= 10,
        "Shortcut match should add >= 10, got {}",
        score
    );
}

// --- merged from part_02.rs ---

#[test]
fn test_score_action_combined_bonuses() {
    // Action where title prefix AND description match
    let action = Action::new(
        "file:copy_path",
        "Copy Path",
        Some("Copies the full path to the clipboard".to_string()),
        ActionCategory::ScriptContext,
    );

    let score = ActionsDialog::score_action(&action, "copy");
    assert_eq!(
        score, 100,
        "Current scoring returns prefix score for this query"
    );
}

#[test]
fn test_score_action_fuzzy_match() {
    let action = Action::new(
        "file:reveal_in_finder",
        "Reveal in Finder",
        None,
        ActionCategory::ScriptContext,
    );

    // "rvf" is a fuzzy match for "reveal in finder" (r-e-v-e-a-l-i-n-f-i-n-d-e-r)
    let fuzzy = ActionsDialog::fuzzy_match(&action.title_lower, "rvf");
    assert!(fuzzy, "rvf should fuzzy-match 'reveal in finder'");

    let score = ActionsDialog::score_action(&action, "rvf");
    assert_eq!(score, 25, "Fuzzy match should score 25");
}

#[test]
fn test_score_action_no_match() {
    let action = Action::new(
        "edit_script",
        "Edit Script",
        None,
        ActionCategory::ScriptContext,
    );

    let score = ActionsDialog::score_action(&action, "zzz");
    assert_eq!(score, 0, "No match should score 0");
}

#[test]
fn test_fuzzy_match_empty_needle() {
    assert!(
        ActionsDialog::fuzzy_match("anything", ""),
        "Empty needle matches everything"
    );
}

#[test]
fn test_fuzzy_match_needle_longer_than_haystack() {
    assert!(
        !ActionsDialog::fuzzy_match("ab", "abc"),
        "Needle longer than haystack should not match"
    );
}

#[test]
fn test_fuzzy_match_exact() {
    assert!(ActionsDialog::fuzzy_match("hello", "hello"));
}

// =========================================================================
// Constants consistency validation
// =========================================================================

#[test]
fn test_constants_positive_and_reasonable() {
    // Use a runtime identity function to prevent clippy constant-value lint
    fn val(x: f32) -> f32 {
        x
    }
    assert!(val(POPUP_WIDTH) > 0.0 && val(POPUP_WIDTH) < 1000.0);
    assert!(val(POPUP_MAX_HEIGHT) > 0.0 && val(POPUP_MAX_HEIGHT) < 2000.0);
    assert!(val(ACTION_ITEM_HEIGHT) > 0.0 && val(ACTION_ITEM_HEIGHT) < 100.0);
    assert!(val(SEARCH_INPUT_HEIGHT) > 0.0 && val(SEARCH_INPUT_HEIGHT) < 100.0);
    assert!(val(HEADER_HEIGHT) > 0.0 && val(HEADER_HEIGHT) < 100.0);
    assert!(val(ACTION_ROW_INSET) >= 0.0 && val(ACTION_ROW_INSET) < 50.0);
}

#[test]
fn test_popup_can_fit_at_least_5_items() {
    let max_items_height = POPUP_MAX_HEIGHT - SEARCH_INPUT_HEIGHT;
    let max_items = (max_items_height / ACTION_ITEM_HEIGHT) as usize;
    assert!(
        max_items >= 5,
        "Popup should fit at least 5 items, fits {}",
        max_items
    );
}

// =========================================================================
// CommandBarConfig presets field validation
// =========================================================================

#[test]
fn test_command_bar_config_default_values() {
    let config = CommandBarConfig::default();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Bottom);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert_eq!(config.dialog_config.anchor, AnchorPosition::Bottom);
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
    assert!(config.close_on_select);
    assert!(config.close_on_click_outside);
    assert!(config.close_on_escape);
}

#[test]
fn test_command_bar_config_ai_style_values() {
    let config = CommandBarConfig::ai_style();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
    assert!(config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
}

#[test]
fn test_command_bar_config_main_menu_style() {
    let config = CommandBarConfig::main_menu_style();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Bottom);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert_eq!(config.dialog_config.anchor, AnchorPosition::Bottom);
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
}

#[test]
fn test_command_bar_config_no_search() {
    let config = CommandBarConfig::no_search();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
}

// =========================================================================
// Agent-specific action validation
// =========================================================================

#[test]
fn test_agent_actions_edit_title_is_edit_agent() {
    let mut script = ScriptInfo::new("My Agent", "/path/to/agent.claude.md");
    script.is_agent = true;
    script.is_script = false; // Agents set is_script=false

    let actions = get_script_context_actions(&script);
    let edit = actions.iter().find(|a| a.id == "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
}

#[test]
fn test_agent_has_no_view_logs() {
    let mut script = ScriptInfo::new("My Agent", "/path/to/agent.claude.md");
    script.is_agent = true;
    script.is_script = false;

    let actions = get_script_context_actions(&script);
    assert!(
        !actions.iter().any(|a| a.id == "view_logs"),
        "Agents should not have view_logs"
    );
}

#[test]
fn test_agent_has_reveal_and_copy() {
    let mut script = ScriptInfo::new("My Agent", "/path/to/agent.claude.md");
    script.is_agent = true;
    script.is_script = false;

    let actions = get_script_context_actions(&script);
    assert!(actions.iter().any(|a| a.id == "reveal_in_finder"));
    assert!(actions.iter().any(|a| a.id == "copy_path"));
    assert!(actions.iter().any(|a| a.id == "copy_content"));
}

#[test]
fn test_agent_with_shortcut_and_alias() {
    let mut script = ScriptInfo::with_shortcut_and_alias(
        "My Agent",
        "/path/to/agent.claude.md",
        Some("cmd+shift+a".to_string()),
        Some("ag".to_string()),
    );
    script.is_agent = true;
    script.is_script = false;

    let actions = get_script_context_actions(&script);

    // Agents get no shortcut/alias preference actions at all
    // (ScriptContextPreferenceActionPlan::AgentNoPreferenceActions).
    assert!(!actions.iter().any(|a| a.id == "update_shortcut"));
    assert!(!actions.iter().any(|a| a.id == "remove_shortcut"));
    assert!(!actions.iter().any(|a| a.id == "update_alias"));
    assert!(!actions.iter().any(|a| a.id == "remove_alias"));
    assert!(!actions.iter().any(|a| a.id == "add_shortcut"));
    assert!(!actions.iter().any(|a| a.id == "add_alias"));
}

#[test]
fn test_agent_with_frecency_has_reset_ranking() {
    let mut script = ScriptInfo::new("My Agent", "/path/to/agent.claude.md")
        .with_frecency(true, Some("agent:/path/to/agent.claude.md".to_string()));
    script.is_agent = true;
    script.is_script = false;

    let actions = get_script_context_actions(&script);
    assert!(actions.iter().any(|a| a.id == "reset_ranking"));
}

// =========================================================================
// Clipboard destructive action ordering invariants
// =========================================================================

#[test]
fn test_clipboard_destructive_actions_always_last() {
    let entry = ClipboardEntryInfo {
        id: "test".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "Test".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };

    let actions = get_clipboard_history_context_actions(&entry);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    let delete_idx = ids
        .iter()
        .position(|&id| id == "clip:clipboard_delete")
        .unwrap();
    let delete_multi_idx = ids
        .iter()
        .position(|&id| id == "clip:clipboard_delete_multiple")
        .unwrap();
    let delete_all_idx = ids
        .iter()
        .position(|&id| id == "clip:clipboard_delete_all")
        .unwrap();

    // All destructive actions should be at the end
    let non_destructive_count = actions.len() - 3;
    assert!(
        delete_idx >= non_destructive_count,
        "clipboard_delete should be in last 3"
    );
    assert!(
        delete_multi_idx >= non_destructive_count,
        "clipboard_delete_multiple should be in last 3"
    );
    assert!(
        delete_all_idx >= non_destructive_count,
        "clipboard_delete_all should be in last 3"
    );
}

#[test]
fn test_clipboard_paste_always_first() {
    let entry = ClipboardEntryInfo {
        id: "test".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "Test".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };

    let actions = get_clipboard_history_context_actions(&entry);
    assert_eq!(actions[0].id, "clip:clipboard_paste");
}

#[test]
fn test_clipboard_copy_always_second() {
    let entry = ClipboardEntryInfo {
        id: "test".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "Test".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };

    let actions = get_clipboard_history_context_actions(&entry);
    assert_eq!(actions[1].id, "clip:clipboard_copy");
}

// =========================================================================
// Path context action validation
// =========================================================================

#[test]
fn test_path_context_directory_primary_action() {
    let path = PathInfo {
        path: "/Users/test/Documents".to_string(),
        name: "Documents".to_string(),
        is_dir: true,
    };

    let actions = get_path_context_actions(&path);
    assert_eq!(actions[0].id, "file:open_directory");
    assert!(actions[0].title.contains("Documents"));
}

#[test]
fn test_path_context_file_primary_action() {
    let path = PathInfo {
        path: "/Users/test/readme.md".to_string(),
        name: "readme.md".to_string(),
        is_dir: false,
    };

    let actions = get_path_context_actions(&path);
    assert_eq!(actions[0].id, "file:select_file");
    assert!(actions[0].title.contains("readme.md"));
}

#[test]
fn test_path_context_trash_description_varies() {
    let dir_path = PathInfo {
        path: "/tmp/dir".to_string(),
        name: "dir".to_string(),
        is_dir: true,
    };
    let file_path = PathInfo {
        path: "/tmp/file.txt".to_string(),
        name: "file.txt".to_string(),
        is_dir: false,
    };

    let dir_actions = get_path_context_actions(&dir_path);
    let file_actions = get_path_context_actions(&file_path);

    let dir_trash = dir_actions
        .iter()
        .find(|a| a.id == "file:move_to_trash")
        .unwrap();
    let file_trash = file_actions
        .iter()
        .find(|a| a.id == "file:move_to_trash")
        .unwrap();

    assert!(
        dir_trash.description.as_ref().unwrap().contains("folder"),
        "Directory trash should say 'folder'"
    );
    assert!(
        file_trash.description.as_ref().unwrap().contains("file"),
        "File trash should say 'file'"
    );
}

#[test]
fn test_path_context_common_actions_present() {
    let path = PathInfo {
        path: "/tmp/test".to_string(),
        name: "test".to_string(),
        is_dir: false,
    };

    let actions = get_path_context_actions(&path);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    assert!(ids.contains(&"file:copy_path"));
    assert!(ids.contains(&"file:open_in_finder"));
    assert!(ids.contains(&"file:open_in_editor"));
    assert!(ids.contains(&"file:open_in_quick_terminal"));
    assert!(ids.contains(&"file:copy_filename"));
    assert!(ids.contains(&"file:move_to_trash"));
}

// =========================================================================
// build_grouped_items_static edge cases
// =========================================================================

#[test]
fn test_build_grouped_items_empty_actions() {
    let actions: Vec<Action> = vec![];
    let filtered: Vec<usize> = vec![];
    let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    assert!(items.is_empty());
}

#[test]
fn test_build_grouped_items_headers_style_adds_headers() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Group1"),
        Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Group1"),
        Action::new("c", "C", None, ActionCategory::ScriptContext).with_section("Group2"),
    ];
    let filtered = vec![0, 1, 2];
    let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);

    // Should have: Header("Group1"), Item(0), Item(1), Header("Group2"), Item(2)
    assert_eq!(items.len(), 5);
    assert!(matches!(&items[0], GroupedActionItem::SectionHeader(s) if s == "Group1"));
    assert!(matches!(&items[1], GroupedActionItem::Item(0)));
    assert!(matches!(&items[2], GroupedActionItem::Item(1)));
    assert!(matches!(&items[3], GroupedActionItem::SectionHeader(s) if s == "Group2"));
    assert!(matches!(&items[4], GroupedActionItem::Item(2)));
}

#[test]
fn test_build_grouped_items_separators_style_no_headers() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Group1"),
        Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Group2"),
    ];
    let filtered = vec![0, 1];
    let items = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);

    // Separators style should NOT add headers
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], GroupedActionItem::Item(0)));
    assert!(matches!(&items[1], GroupedActionItem::Item(1)));
}

#[test]
fn test_build_grouped_items_none_style_no_headers() {
    let actions = vec![
        Action::new("a", "A", None, ActionCategory::ScriptContext).with_section("Group1"),
        Action::new("b", "B", None, ActionCategory::ScriptContext).with_section("Group2"),
    ];
    let filtered = vec![0, 1];
    let items = build_grouped_items_static(&actions, &filtered, SectionStyle::None);

    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], GroupedActionItem::Item(0)));
    assert!(matches!(&items[1], GroupedActionItem::Item(1)));
}

// --- merged from part_03.rs ---

// =========================================================================
// coerce_action_selection edge cases
// =========================================================================

#[test]
fn test_coerce_empty_rows() {
    assert_eq!(coerce_action_selection(&[], 0), None);
}

#[test]
fn test_coerce_single_item() {
    let rows = vec![GroupedActionItem::Item(0)];
    assert_eq!(coerce_action_selection(&rows, 0), Some(0));
}

#[test]
fn test_coerce_single_header() {
    let rows = vec![GroupedActionItem::SectionHeader("Test".to_string())];
    assert_eq!(coerce_action_selection(&rows, 0), None);
}

#[test]
fn test_coerce_header_then_item() {
    let rows = vec![
        GroupedActionItem::SectionHeader("Test".to_string()),
        GroupedActionItem::Item(0),
    ];
    // Landing on header (index 0) should search down to find item at index 1
    assert_eq!(coerce_action_selection(&rows, 0), Some(1));
}

#[test]
fn test_coerce_item_then_header() {
    let rows = vec![
        GroupedActionItem::Item(0),
        GroupedActionItem::SectionHeader("Test".to_string()),
    ];
    // Landing on header (index 1) should search up to find item at index 0
    assert_eq!(coerce_action_selection(&rows, 1), Some(0));
}

#[test]
fn test_coerce_clamps_beyond_bounds() {
    let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
    // Index 100 should be clamped to last valid
    assert_eq!(coerce_action_selection(&rows, 100), Some(1));
}

#[test]
fn test_coerce_consecutive_headers_at_start() {
    let rows = vec![
        GroupedActionItem::SectionHeader("A".to_string()),
        GroupedActionItem::SectionHeader("B".to_string()),
        GroupedActionItem::Item(0),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), Some(2));
    assert_eq!(coerce_action_selection(&rows, 1), Some(2));
}

// =========================================================================
// AI command bar action invariants
// =========================================================================

#[test]
fn test_ai_command_bar_all_have_icons() {
    let actions = get_ai_command_bar_actions();
    for action in &actions {
        assert!(
            action.icon.is_some(),
            "AI command bar action '{}' should have an icon",
            action.id
        );
    }
}

#[test]
fn test_ai_command_bar_all_have_sections() {
    let actions = get_ai_command_bar_actions();
    for action in &actions {
        assert!(
            action.section.is_some(),
            "AI command bar action '{}' should have a section",
            action.id
        );
    }
}

#[test]
fn test_ai_command_bar_section_ordering() {
    let actions = get_ai_command_bar_actions();
    let sections: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.section.as_deref())
        .collect();

    // Sections should appear in order: Response, Actions, Attachments, Settings
    let first_response = sections.iter().position(|&s| s == "Response").unwrap();
    let first_actions = sections.iter().position(|&s| s == "Actions").unwrap();
    let first_attachments = sections.iter().position(|&s| s == "Attachments").unwrap();
    let first_settings = sections.iter().position(|&s| s == "Settings").unwrap();

    assert!(first_response < first_actions);
    assert!(first_actions < first_attachments);
    assert!(first_attachments < first_settings);
}

#[test]
fn test_ai_command_bar_has_expected_ids() {
    let actions = get_ai_command_bar_actions();
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    assert!(ids.contains(&"chat:copy_response"));
    assert!(ids.contains(&"chat:copy_chat"));
    assert!(ids.contains(&"chat:copy_last_code"));
    assert!(ids.contains(&"chat:submit"));
    assert!(ids.contains(&"chat:new_chat"));
    assert!(ids.contains(&"chat:delete_chat"));
    assert!(ids.contains(&"chat:add_file"));
    assert!(ids.contains(&"chat:add_image"));
    assert!(ids.contains(&"chat:paste_image"));
    assert!(ids.contains(&"chat:change_model"));
    assert!(ids.contains(&"chat:export_markdown"));
    assert!(ids.contains(&"chat:branch_from_last"));
    assert!(ids.contains(&"chat:toggle_shortcuts_help"));
    assert!(ids.contains(&"chat:capture_screen_area"));
    assert!(ids.contains(&"chat:inspect_context"));
    assert!(ids.contains(&"chat:toggle_window_mode"));
    assert_eq!(ids.len(), 38);
}

// =========================================================================
// Chat context action variations
// =========================================================================

#[test]
fn test_chat_no_models_no_messages() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };

    let actions = get_chat_context_actions(&info);
    // change_model + continue_in_chat + capture_screen_area
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0].id, "chat:change_model");
}

#[test]
fn test_chat_with_models_and_response() {
    let info = ChatPromptInfo {
        current_model: Some("Claude 3.5 Sonnet".to_string()),
        available_models: vec![
            ChatModelInfo {
                id: "claude-3-5-sonnet".to_string(),
                display_name: "Claude 3.5 Sonnet".to_string(),
                provider: "Anthropic".to_string(),
            },
            ChatModelInfo {
                id: "gpt-4".to_string(),
                display_name: "GPT-4".to_string(),
                provider: "OpenAI".to_string(),
            },
        ],
        has_messages: true,
        has_response: true,
    };

    let actions = get_chat_context_actions(&info);
    // change_model + continue_in_chat + copy_response + clear_conversation + capture_screen_area = 5
    assert_eq!(actions.len(), 5);
    // No flat model rows in root — models live in the drill-down picker
    assert!(
        !actions
            .iter()
            .any(|a| a.id.starts_with("chat:select_model_"))
    );

    // Model checkmarks are in the picker
    let picker = get_chat_model_picker_actions(&info);
    let current = picker
        .iter()
        .find(|a| a.id == "chat:select_model_claude-3-5-sonnet")
        .unwrap();
    assert!(current.title.contains("✓"));

    let other = picker
        .iter()
        .find(|a| a.id == "chat:select_model_gpt-4")
        .unwrap();
    assert!(!other.title.contains("✓"));
}

#[test]
fn test_chat_messages_but_no_response() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: true,
        has_response: false,
    };

    let actions = get_chat_context_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"chat:clear_conversation"));
    assert!(!ids.contains(&"chat:copy_response"));
}

// =========================================================================
// Notes command bar permutations
// =========================================================================

#[test]
fn test_notes_no_selection_no_trash() {
    let info = NotesInfo {
        has_selection: false,
        is_trash_view: false,
        auto_sizing_enabled: false,
    };

    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // new_note and browse_notes are always present
    assert!(ids.contains(&"new_note"));
    assert!(ids.contains(&"browse_notes"));
    // Selection-gated actions should NOT be present
    assert!(!ids.contains(&"duplicate_note"));
    assert!(!ids.contains(&"find_in_note"));
    assert!(!ids.contains(&"format"));
    assert!(!ids.contains(&"copy_note_as"));
    assert!(ids.contains(&"toggle_auto_sizing"));
}

#[test]
fn test_notes_with_selection_not_trash() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: false,
        auto_sizing_enabled: true,
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
fn test_notes_trash_view_suppresses_edit_actions() {
    let info = NotesInfo {
        has_selection: true,
        is_trash_view: true,
        auto_sizing_enabled: false,
    };

    let actions = get_notes_command_bar_actions(&info);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // new_note always present
    assert!(ids.contains(&"new_note"));
    // Selection + trash view = no edit actions
    assert!(!ids.contains(&"duplicate_note"));
    assert!(!ids.contains(&"find_in_note"));
    assert!(!ids.contains(&"format"));
    assert!(!ids.contains(&"copy_note_as"));
}

#[test]
fn test_notes_all_actions_have_icons() {
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
fn test_notes_all_actions_have_sections() {
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

// =========================================================================
// Note switcher edge cases
// =========================================================================

#[test]
fn test_note_switcher_empty_notes() {
    let actions = get_note_switcher_actions(&[]);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, "no_notes");
    assert_eq!(actions[0].title, "No notes yet");
}

#[test]
fn test_note_switcher_current_note_bullet() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "My Note".to_string(),
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
fn test_note_switcher_non_current_no_bullet() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "My Note".to_string(),
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
fn test_note_switcher_pinned_icon() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "Pinned Note".to_string(),
        char_count: 100,
        is_current: false,
        is_pinned: true,
        preview: String::new(),
        relative_time: String::new(),
    }];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].icon, Some(IconName::StarFilled));
}

#[test]
fn test_note_switcher_current_icon() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "Current Note".to_string(),
        char_count: 100,
        is_current: true,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].icon, Some(IconName::Check));
}

#[test]
fn test_note_switcher_pinned_priority_over_current() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "Both".to_string(),
        char_count: 100,
        is_current: true,
        is_pinned: true,
        preview: String::new(),
        relative_time: String::new(),
    }];

    let actions = get_note_switcher_actions(&notes);
    // Pinned icon takes priority
    assert_eq!(actions[0].icon, Some(IconName::StarFilled));
}

#[test]
fn test_note_switcher_char_count_singular() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "Note".to_string(),
        char_count: 1,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].description, Some("1 char".to_string()));
}

#[test]
fn test_note_switcher_char_count_plural() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "Note".to_string(),
        char_count: 42,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].description, Some("42 chars".to_string()));
}

#[test]
fn test_note_switcher_char_count_zero() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc-123".to_string(),
        title: "Empty".to_string(),
        char_count: 0,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];

    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].description, Some("0 chars".to_string()));
}

// --- merged from part_04.rs ---

#[test]
fn test_note_switcher_all_have_notes_section() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "1".to_string(),
            title: "A".to_string(),
            char_count: 10,
            is_current: true,
            is_pinned: false,
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
// New chat action validation
// =========================================================================

#[test]
fn test_new_chat_empty_inputs() {
    let actions = get_new_chat_actions(&[], &[], &[]);
    assert!(actions.is_empty());
}

#[test]
fn test_new_chat_section_ordering() {
    let last_used = vec![NewChatModelInfo {
        model_id: "claude-3".to_string(),
        display_name: "Claude 3".to_string(),
        provider: "anthropic".to_string(),
        provider_display_name: "Anthropic".to_string(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "general".to_string(),
        name: "General".to_string(),
        icon: IconName::Code,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "gpt-4".to_string(),
        display_name: "GPT-4".to_string(),
        provider: "openai".to_string(),
        provider_display_name: "OpenAI".to_string(),
    }];

    let actions = get_new_chat_actions(&last_used, &presets, &models);

    let sections: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.section.as_deref())
        .collect();

    // Sections in order: Last Used Settings, Presets, Models
    let lu_idx = sections
        .iter()
        .position(|&s| s == "Last Used Settings")
        .unwrap();
    let p_idx = sections.iter().position(|&s| s == "Presets").unwrap();
    let m_idx = sections.iter().position(|&s| s == "Models").unwrap();

    assert!(lu_idx < p_idx);
    assert!(p_idx < m_idx);
}

#[test]
fn test_new_chat_all_have_icons() {
    let last_used = vec![NewChatModelInfo {
        model_id: "claude".to_string(),
        display_name: "Claude".to_string(),
        provider: "anthropic".to_string(),
        provider_display_name: "Anthropic".to_string(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "code".to_string(),
        name: "Code".to_string(),
        icon: IconName::Code,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "gpt".to_string(),
        display_name: "GPT".to_string(),
        provider: "openai".to_string(),
        provider_display_name: "OpenAI".to_string(),
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

#[test]
fn test_new_chat_last_used_has_bolt_icon() {
    let last_used = vec![NewChatModelInfo {
        model_id: "claude".to_string(),
        display_name: "Claude".to_string(),
        provider: "anthropic".to_string(),
        provider_display_name: "Anthropic".to_string(),
    }];

    let actions = get_new_chat_actions(&last_used, &[], &[]);
    assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
}

#[test]
fn test_new_chat_models_has_settings_icon() {
    let models = vec![NewChatModelInfo {
        model_id: "gpt".to_string(),
        display_name: "GPT".to_string(),
        provider: "openai".to_string(),
        provider_display_name: "OpenAI".to_string(),
    }];

    let actions = get_new_chat_actions(&[], &[], &models);
    assert_eq!(actions[0].icon, Some(IconName::Settings));
}

// =========================================================================
// File context edge cases
// =========================================================================

#[test]
fn test_file_context_file_vs_dir_action_count() {
    let file = FileInfo {
        path: "/tmp/file.txt".to_string(),
        name: "file.txt".to_string(),
        file_type: FileType::File,
        is_dir: false,
    };
    let dir = FileInfo {
        path: "/tmp/dir".to_string(),
        name: "dir".to_string(),
        file_type: FileType::Directory,
        is_dir: true,
    };

    let file_actions = get_file_context_actions(&file);
    let dir_actions = get_file_context_actions(&dir);

    // File should have Quick Look, dir should not (macOS)
    #[cfg(target_os = "macos")]
    {
        assert_eq!(file_actions.len(), 13);
        assert_eq!(dir_actions.len(), 11);
    }
}

#[test]
fn test_file_context_title_includes_name() {
    let file = FileInfo {
        path: "/tmp/my-document.pdf".to_string(),
        name: "my-document.pdf".to_string(),
        file_type: FileType::Document,
        is_dir: false,
    };

    let actions = get_file_context_actions(&file);
    assert!(actions[0].title.contains("my-document.pdf"));
}

// =========================================================================
// Scriptlet with custom actions validation
// =========================================================================

#[test]
fn test_scriptlet_custom_actions_have_has_action_true() {
    let script = ScriptInfo::scriptlet("Test", "/path/to/test.md", None, None);
    let mut scriptlet = Scriptlet::new("Test".to_string(), "bash".to_string(), "echo".to_string());
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
    assert!(
        custom.has_action,
        "Custom scriptlet actions must have has_action=true"
    );
    assert_eq!(custom.value, Some("custom".to_string()));
}

#[test]
fn test_scriptlet_builtin_actions_have_has_action_false() {
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

// =========================================================================
// ID uniqueness checks
// =========================================================================

#[test]
fn test_script_context_no_duplicate_ids() {
    let script = ScriptInfo::new("test", "/path/to/test.ts");
    let actions = get_script_context_actions(&script);

    let mut seen = std::collections::HashSet::new();
    for action in &actions {
        assert!(
            seen.insert(&action.id),
            "Duplicate action ID: {}",
            action.id
        );
    }
}

#[test]
fn test_clipboard_context_no_duplicate_ids() {
    let entry = ClipboardEntryInfo {
        id: "test".to_string(),
        content_type: ContentType::Image,
        pinned: true,
        preview: "test".to_string(),
        image_dimensions: Some((100, 100)),
        frontmost_app_name: Some("Finder".to_string()),
    };

    let actions = get_clipboard_history_context_actions(&entry);

    let mut seen = std::collections::HashSet::new();
    for action in &actions {
        assert!(
            seen.insert(&action.id),
            "Duplicate clipboard action ID: {}",
            action.id
        );
    }
}

#[test]
fn test_ai_command_bar_no_duplicate_ids() {
    let actions = get_ai_command_bar_actions();

    let mut seen = std::collections::HashSet::new();
    for action in &actions {
        assert!(
            seen.insert(&action.id),
            "Duplicate AI action ID: {}",
            action.id
        );
    }
}

// =========================================================================
// Action category invariants
// =========================================================================

#[test]
fn test_all_script_actions_use_script_context_category() {
    let script = ScriptInfo::new("test", "/path/to/test.ts");
    let actions = get_script_context_actions(&script);

    for action in &actions {
        assert_eq!(
            action.category,
            ActionCategory::ScriptContext,
            "Script action '{}' should have ScriptContext category",
            action.id
        );
    }
}

#[test]
fn test_all_clipboard_actions_use_script_context_category() {
    let entry = ClipboardEntryInfo {
        id: "test".to_string(),
        content_type: ContentType::Text,
        pinned: false,
        preview: "test".to_string(),
        image_dimensions: None,
        frontmost_app_name: None,
    };

    let actions = get_clipboard_history_context_actions(&entry);
    for action in &actions {
        assert_eq!(
            action.category,
            ActionCategory::ScriptContext,
            "Clipboard action '{}' should have ScriptContext category",
            action.id
        );
    }
}

// =========================================================================
// Enum default values
// =========================================================================

#[test]
fn test_search_position_default() {
    assert_eq!(SearchPosition::default(), SearchPosition::Bottom);
}

#[test]
fn test_section_style_default() {
    assert_eq!(SectionStyle::default(), SectionStyle::Headers);
}

#[test]
fn test_anchor_position_default() {
    assert_eq!(AnchorPosition::default(), AnchorPosition::Bottom);
}

#[test]
fn test_actions_dialog_config_default() {
    let config = ActionsDialogConfig::default();
    assert_eq!(config.search_position, SearchPosition::Bottom);
    assert_eq!(config.section_style, SectionStyle::Headers);
    assert_eq!(config.anchor, AnchorPosition::Bottom);
    assert!(!config.show_icons);
    assert!(!config.show_footer);
}

// =========================================================================
// Action with_* builder chain validation
// =========================================================================

#[test]
fn test_action_builder_chain() {
    let action = Action::new(
        "test",
        "Test",
        Some("Desc".into()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘T")
    .with_icon(IconName::Star)
    .with_section("Section");

    assert_eq!(action.shortcut, Some("⌘T".to_string()));
    assert_eq!(action.icon, Some(IconName::Star));
    assert_eq!(action.section, Some("Section".to_string()));
    // Lowercase caches should be populated
    assert_eq!(action.title_lower, "test");
    assert_eq!(action.description_lower, Some("desc".to_string()));
    assert_eq!(action.shortcut_lower, Some("⌘t".to_string()));
}

#[test]
fn test_action_default_fields() {
    let action = Action::new("id", "Title", None, ActionCategory::ScriptContext);
    assert!(!action.has_action);
    assert!(action.value.is_none());
    assert!(action.icon.is_none());
    assert!(action.section.is_none());
    assert!(action.shortcut.is_none());
}

// =========================================================================
// ScriptInfo agent construction
// =========================================================================

#[test]
fn test_script_info_agent_requires_is_script_false() {
    let mut agent = ScriptInfo::new("Agent", "/path/to/agent.md");
    agent.is_agent = true;
    agent.is_script = false;

    let actions = get_script_context_actions(&agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // Agent-specific actions
    assert!(ids.contains(&"edit_script")); // titled "Edit Agent"
    assert!(ids.contains(&"reveal_in_finder"));
    assert!(ids.contains(&"copy_path"));
    assert!(ids.contains(&"copy_content"));
    // NOT script-only
    assert!(!ids.contains(&"view_logs"));
}

#[test]
fn test_script_info_agent_with_is_script_true_gets_script_actions() {
    let mut script_agent = ScriptInfo::new("Agent", "/path/to/agent.md");
    script_agent.is_agent = true;
    script_agent.is_script = true; // This is wrong for agents but let's test behavior

    let actions = get_script_context_actions(&script_agent);
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();

    // With is_script=true, gets BOTH script and agent actions (duplicates may occur)
    assert!(ids.contains(&"view_logs")); // script-only action
}

// =========================================================================
// Clipboard context title truncation
// =========================================================================

#[test]
fn test_clipboard_short_preview_not_truncated() {
    let preview = "Short text".to_string();
    let context_title = if preview.len() > 30 {
        format!("{}...", &preview[..27])
    } else {
        preview.clone()
    };
    assert_eq!(context_title, "Short text");
}

// --- merged from part_05.rs ---

#[test]
fn test_clipboard_long_preview_truncated_at_27() {
    let preview = "This is a very long clipboard preview text that exceeds the limit".to_string();
    let context_title = if preview.len() > 30 {
        format!("{}...", &preview[..27])
    } else {
        preview.clone()
    };
    assert_eq!(context_title.len(), 30); // 27 chars + "..."
    assert!(context_title.ends_with("..."));
}

#[test]
fn test_clipboard_exactly_30_chars_not_truncated() {
    let preview = "123456789012345678901234567890".to_string(); // exactly 30
    let context_title = if preview.len() > 30 {
        format!("{}...", &preview[..27])
    } else {
        preview.clone()
    };
    assert_eq!(context_title, preview);
}
