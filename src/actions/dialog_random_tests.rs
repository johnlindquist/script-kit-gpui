// --- merged from part_01.rs ---
//! Random action/dialog/window behavior validation tests
//!
//! Tests validate various edge cases and behaviors across action builders,
//! dialog helpers, window utilities, and configuration presets.

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
    Action, ActionCategory, ActionsDialogConfig, AnchorPosition, ScriptInfo, SearchPosition,
    SectionStyle,
};
use super::window::{WindowPosition, count_section_headers};
use crate::clipboard_history::ContentType;
use crate::designs::icon_variations::IconName;
use crate::file_search::{FileInfo, FileType};
use crate::prompts::PathInfo;
use crate::protocol::ProtocolAction;
use crate::scriptlets::{Scriptlet, ScriptletAction};

// =========================================================================
// Helper functions
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
// 1. Window count_section_headers — edge cases
// =========================================================================

#[test]
fn window_count_section_headers_empty_indices() {
    let actions = vec![make_action("a", "A", Some("S1"))];
    assert_eq!(count_section_headers(&actions, &[]), 0);
}

#[test]
fn window_count_section_headers_all_none_sections() {
    let actions = vec![
        make_action("a", "A", None),
        make_action("b", "B", None),
        make_action("c", "C", None),
    ];
    assert_eq!(count_section_headers(&actions, &[0, 1, 2]), 0);
}

#[test]
fn window_count_section_headers_single_section() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S1")),
    ];
    // Only 1 header for "S1" (first item introduces the section)
    assert_eq!(count_section_headers(&actions, &[0, 1]), 1);
}

#[test]
fn window_count_section_headers_alternating_sections() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S2")),
        make_action("c", "C", Some("S1")),
        make_action("d", "D", Some("S2")),
    ];
    // S1 -> S2 -> S1 -> S2 = 4 section changes
    assert_eq!(count_section_headers(&actions, &[0, 1, 2, 3]), 4);
}

#[test]
fn window_count_section_headers_none_then_some() {
    let actions = vec![
        make_action("a", "A", None),
        make_action("b", "B", Some("S1")),
        make_action("c", "C", Some("S1")),
    ];
    // None does not count; S1 introduced once
    assert_eq!(count_section_headers(&actions, &[0, 1, 2]), 1);
}

#[test]
fn window_count_section_headers_some_then_none_then_some() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", None),
        make_action("c", "C", Some("S1")),
    ];
    // S1(count) -> None(skip) -> S1(same section as last sectioned row, no new header)
    assert_eq!(count_section_headers(&actions, &[0, 1, 2]), 1);
}

#[test]
fn window_count_section_headers_filtered_subset() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S2")),
        make_action("c", "C", Some("S3")),
    ];
    // Only show S1 and S3 — skipping S2
    assert_eq!(count_section_headers(&actions, &[0, 2]), 2);
}

#[test]
fn window_count_section_headers_out_of_bounds_index() {
    let actions = vec![make_action("a", "A", Some("S1"))];
    // Index 99 is out of bounds; .get(99) returns None so it's skipped
    assert_eq!(count_section_headers(&actions, &[0, 99]), 1);
}

// =========================================================================
// 2. WindowPosition enum
// =========================================================================

#[test]
fn window_position_default_is_bottom_right() {
    let pos = WindowPosition::default();
    assert_eq!(pos, WindowPosition::BottomRight);
}

#[test]
fn window_position_variants_are_distinct() {
    assert_ne!(WindowPosition::BottomRight, WindowPosition::TopRight);
    assert_ne!(WindowPosition::TopRight, WindowPosition::TopCenter);
    assert_ne!(WindowPosition::BottomRight, WindowPosition::TopCenter);
}

// =========================================================================
// 3. format_shortcut_hint — edge cases
// =========================================================================

#[test]
fn format_shortcut_hint_empty_string() {
    let result = ActionsDialog::format_shortcut_hint("");
    assert_eq!(result, "");
}

#[test]
fn format_shortcut_hint_single_key_no_modifier() {
    let result = ActionsDialog::format_shortcut_hint("c");
    assert_eq!(result, "C");
}

#[test]
fn format_shortcut_hint_all_modifier_aliases() {
    // command, meta, super all map to ⌘
    assert!(ActionsDialog::format_shortcut_hint("command+a").contains('⌘'));
    assert!(ActionsDialog::format_shortcut_hint("meta+a").contains('⌘'));
    assert!(ActionsDialog::format_shortcut_hint("super+a").contains('⌘'));
    // control maps to ⌃
    assert!(ActionsDialog::format_shortcut_hint("control+a").contains('⌃'));
    // opt, option map to ⌥
    assert!(ActionsDialog::format_shortcut_hint("opt+a").contains('⌥'));
    assert!(ActionsDialog::format_shortcut_hint("option+a").contains('⌥'));
}

#[test]
fn format_shortcut_hint_mixed_case_modifiers() {
    let result = ActionsDialog::format_shortcut_hint("CMD+SHIFT+c");
    assert!(result.contains('⌘'));
    assert!(result.contains('⇧'));
    assert!(result.contains('C'));
}

#[test]
fn format_shortcut_hint_special_keys_as_last_part() {
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+enter"), "⌘↵");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+escape"), "⌘⎋");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+tab"), "⌘⇥");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+backspace"), "⌘⌫");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+space"), "⌘␣");
}

#[test]
fn format_shortcut_hint_arrow_keys() {
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+up"), "⌘↑");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+arrowdown"), "⌘↓");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+arrowleft"), "⌘←");
    assert_eq!(ActionsDialog::format_shortcut_hint("cmd+arrowright"), "⌘→");
}

#[test]
fn format_shortcut_hint_return_and_esc_aliases() {
    assert_eq!(ActionsDialog::format_shortcut_hint("return"), "↵");
    assert_eq!(ActionsDialog::format_shortcut_hint("esc"), "⎋");
}

#[test]
fn format_shortcut_hint_whitespace_around_parts() {
    let result = ActionsDialog::format_shortcut_hint(" cmd + shift + c ");
    assert!(result.contains('⌘'));
    assert!(result.contains('⇧'));
    assert!(result.contains('C'));
}

#[test]
fn format_shortcut_hint_multi_char_key() {
    // A multi-char key like "f12" should be uppercased
    let result = ActionsDialog::format_shortcut_hint("cmd+f12");
    assert!(result.contains('⌘'));
    assert!(result.contains("F12"));
}

// =========================================================================
// 4. parse_shortcut_keycaps — edge cases
// =========================================================================

#[test]
fn parse_shortcut_keycaps_empty() {
    assert!(ActionsDialog::parse_shortcut_keycaps("").is_empty());
}

#[test]
fn parse_shortcut_keycaps_all_modifiers() {
    let caps = ActionsDialog::parse_shortcut_keycaps("⌘⌃⌥⇧");
    assert_eq!(caps, vec!["⌘", "⌃", "⌥", "⇧"]);
}

#[test]
fn parse_shortcut_keycaps_arrow_symbols() {
    let caps = ActionsDialog::parse_shortcut_keycaps("↑↓←→");
    assert_eq!(caps, vec!["↑", "↓", "←", "→"]);
}

#[test]
fn parse_shortcut_keycaps_special_symbols() {
    let caps = ActionsDialog::parse_shortcut_keycaps("↵⎋⇥⌫␣");
    assert_eq!(caps, vec!["↵", "⎋", "⇥", "⌫", "␣"]);
}

#[test]
fn parse_shortcut_keycaps_lowercase_uppercased() {
    let caps = ActionsDialog::parse_shortcut_keycaps("⌘a");
    assert_eq!(caps, vec!["⌘", "A"]);
}

#[test]
fn parse_shortcut_keycaps_number_key() {
    let caps = ActionsDialog::parse_shortcut_keycaps("⌘1");
    assert_eq!(caps, vec!["⌘", "1"]);
}

// =========================================================================
// 5. score_action — cumulative scoring edge cases
// =========================================================================

#[test]
fn score_action_empty_search_matches_all_with_prefix() {
    let action = Action::new("test", "Run Script", None, ActionCategory::ScriptContext);
    // Empty search matches as prefix of anything
    let score = ActionsDialog::score_action(&action, "");
    assert!(
        score >= 100,
        "Empty search should be prefix match: {}",
        score
    );
}

#[test]
fn score_action_all_fields_match() {
    let action = Action::new(
        "edit_script",
        "Edit Script",
        Some("Edit the script in your editor".to_string()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘E");
    // "edit" matches title as prefix (+100), description contains "edit" (+15), shortcut doesn't contain "edit"
    let score = ActionsDialog::score_action(&action, "edit");
    assert_eq!(score, 115, "Prefix(100) + description(15) = 115");
}

#[test]
fn score_action_description_only_match() {
    let action = Action::new(
        "file:open_file",
        "Open File",
        Some("Launch with default application".to_string()),
        ActionCategory::ScriptContext,
    );
    // "launch" doesn't match title at all, but matches description
    let score = ActionsDialog::score_action(&action, "launch");
    assert_eq!(score, 15, "Description-only match should be 15");
}

#[test]
fn score_action_shortcut_only_match() {
    let action = Action::new(
        "script:run",
        "Run Script",
        None,
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘enter");
    // "enter" doesn't match title prefix/contains/fuzzy but matches shortcut (lowercase)
    let score = ActionsDialog::score_action(&action, "enter");
    // "enter" doesn't appear in "run script", no description, but shortcut_lower = "⌘enter" contains "enter"
    assert_eq!(score, 10, "Shortcut-only match should be 10");
}

#[test]
fn score_action_fuzzy_plus_description() {
    let action = Action::new(
        "exit_dialog",
        "Exit Dialog",
        Some("Close the exit dialog panel".to_string()),
        ActionCategory::ScriptContext,
    );
    // "edg" — fuzzy matches "exit dialog" (e-x-i-t- -d-i-a-l-o-g: e...d...g not found)
    // Actually let's use "eid" which fuzzy matches: e(xit d)i(alog) — but "d" doesn't come after "i" in remaining
    // Use "xit" which fuzzy matches and title contains "xit"
    let score = ActionsDialog::score_action(&action, "xit");
    // "xit" is contained in "exit dialog" → +50 (contains), description "close the exit dialog panel" also contains "xit" → +15
    assert_eq!(score, 65, "Contains(50) + description(15) = 65");
}

#[test]
fn score_action_no_match_at_all() {
    let action = Action::new(
        "run_script",
        "Run Script",
        Some("Execute the script".to_string()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘R");
    let score = ActionsDialog::score_action(&action, "zzz");
    assert_eq!(score, 0, "No match should score 0");
}

// =========================================================================
// 6. fuzzy_match — edge cases
// =========================================================================

#[test]
fn fuzzy_match_empty_needle_returns_true() {
    assert!(ActionsDialog::fuzzy_match("anything", ""));
}

#[test]
fn fuzzy_match_empty_haystack_non_empty_needle() {
    assert!(!ActionsDialog::fuzzy_match("", "x"));
}

#[test]
fn fuzzy_match_exact_match() {
    assert!(ActionsDialog::fuzzy_match("hello", "hello"));
}

#[test]
fn fuzzy_match_subsequence() {
    assert!(ActionsDialog::fuzzy_match("hello world", "hlw"));
}

#[test]
fn fuzzy_match_needle_longer_than_haystack() {
    assert!(!ActionsDialog::fuzzy_match("ab", "abc"));
}

#[test]
fn fuzzy_match_repeated_chars_in_needle() {
    // "aab" in "abracadabra" — first 'a' at 0, second 'a' at 2, 'b' at 4
    assert!(ActionsDialog::fuzzy_match("abracadabra", "aab"));
}

#[test]
fn fuzzy_match_same_char_repeated() {
    // "aaa" in "banana" — b-a-n-a-n-a: 'a' at 1, 'a' at 3, 'a' at 5
    assert!(ActionsDialog::fuzzy_match("banana", "aaa"));
}

#[test]
fn fuzzy_match_case_sensitive() {
    // Fuzzy match is case-sensitive
    assert!(!ActionsDialog::fuzzy_match("hello", "H"));
    assert!(ActionsDialog::fuzzy_match("Hello", "H"));
}

// =========================================================================
// 7. coerce_action_selection — more patterns
// =========================================================================

#[test]
fn coerce_all_headers_returns_none() {
    let rows = vec![
        GroupedActionItem::SectionHeader("S1".to_string()),
        GroupedActionItem::SectionHeader("S2".to_string()),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), None);
}

#[test]
fn coerce_header_then_item_selects_item() {
    let rows = vec![
        GroupedActionItem::SectionHeader("S1".to_string()),
        GroupedActionItem::Item(0),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), Some(1));
}

#[test]
fn coerce_item_then_header_at_end() {
    let rows = vec![
        GroupedActionItem::Item(0),
        GroupedActionItem::SectionHeader("S1".to_string()),
    ];
    // Index 1 is header; search down finds nothing, search up finds Item at 0
    assert_eq!(coerce_action_selection(&rows, 1), Some(0));
}

#[test]
fn coerce_multiple_headers_before_item() {
    let rows = vec![
        GroupedActionItem::SectionHeader("S1".to_string()),
        GroupedActionItem::SectionHeader("S2".to_string()),
        GroupedActionItem::SectionHeader("S3".to_string()),
        GroupedActionItem::Item(0),
    ];
    // Index 0 is header; searching down finds Item at 3
    assert_eq!(coerce_action_selection(&rows, 0), Some(3));
}

#[test]
fn coerce_beyond_bounds_clamps() {
    let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
    // Index 999 clamps to len-1 = 1, which is an Item
    assert_eq!(coerce_action_selection(&rows, 999), Some(1));
}

// --- merged from part_02.rs ---

// =========================================================================
// 8. build_grouped_items_static — section style effects
// =========================================================================

#[test]
fn build_grouped_items_headers_style_adds_headers() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S1")),
        make_action("c", "C", Some("S2")),
    ];
    let result = build_grouped_items_static(&actions, &[0, 1, 2], SectionStyle::Headers);
    // Should have: Header("S1"), Item(0), Item(1), Header("S2"), Item(2)
    assert_eq!(result.len(), 5);
    assert!(matches!(&result[0], GroupedActionItem::SectionHeader(s) if s == "S1"));
    assert!(matches!(result[1], GroupedActionItem::Item(0)));
    assert!(matches!(result[2], GroupedActionItem::Item(1)));
    assert!(matches!(&result[3], GroupedActionItem::SectionHeader(s) if s == "S2"));
    assert!(matches!(result[4], GroupedActionItem::Item(2)));
}

#[test]
fn build_grouped_items_separators_style_no_headers() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S2")),
    ];
    let result = build_grouped_items_static(&actions, &[0, 1], SectionStyle::Separators);
    // Separators style: no headers, just items
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0], GroupedActionItem::Item(0)));
    assert!(matches!(result[1], GroupedActionItem::Item(1)));
}

#[test]
fn build_grouped_items_none_style_no_headers() {
    let actions = vec![
        make_action("a", "A", Some("S1")),
        make_action("b", "B", Some("S2")),
    ];
    let result = build_grouped_items_static(&actions, &[0, 1], SectionStyle::None);
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0], GroupedActionItem::Item(0)));
    assert!(matches!(result[1], GroupedActionItem::Item(1)));
}

#[test]
fn build_grouped_items_empty_actions() {
    let actions: Vec<Action> = vec![];
    let result = build_grouped_items_static(&actions, &[], SectionStyle::Headers);
    assert!(result.is_empty());
}

#[test]
fn build_grouped_items_no_sections_with_headers_style() {
    let actions = vec![make_action("a", "A", None), make_action("b", "B", None)];
    let result = build_grouped_items_static(&actions, &[0, 1], SectionStyle::Headers);
    // No sections → no headers, just items
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0], GroupedActionItem::Item(0)));
    assert!(matches!(result[1], GroupedActionItem::Item(1)));
}

// =========================================================================
// 9. to_deeplink_name — unicode and edge cases
// =========================================================================

#[test]
fn to_deeplink_name_unicode_accents() {
    let result = to_deeplink_name("Café Résumé");
    assert_eq!(result, "caf%C3%A9-r%C3%A9sum%C3%A9");
}

#[test]
fn to_deeplink_name_all_special_chars() {
    let result = to_deeplink_name("!@#$%^&*()");
    assert_eq!(result, "_unnamed");
}

#[test]
fn to_deeplink_name_consecutive_separators() {
    let result = to_deeplink_name("a___b---c   d");
    assert_eq!(result, "a-b-c-d");
}

#[test]
fn to_deeplink_name_numbers_only() {
    let result = to_deeplink_name("123");
    assert_eq!(result, "123");
}

#[test]
fn to_deeplink_name_mixed_case_and_underscores() {
    let result = to_deeplink_name("My_Script_Name");
    assert_eq!(result, "my-script-name");
}

#[test]
fn to_deeplink_name_emoji() {
    // Emoji are alphanumeric in Unicode (they pass is_alphanumeric)
    // Actually, emoji may NOT pass is_alphanumeric; let's test what happens
    let result = to_deeplink_name("hello 🎉 world");
    // 🎉 is NOT alphanumeric, so it becomes a hyphen
    assert!(result == "hello-world" || result == "hello-🎉-world");
}

#[test]
fn to_deeplink_name_single_char() {
    assert_eq!(to_deeplink_name("A"), "a");
}

#[test]
fn to_deeplink_name_leading_trailing_spaces() {
    let result = to_deeplink_name("  hello world  ");
    assert_eq!(result, "hello-world");
}

// =========================================================================
// 10. Agent action validation
// =========================================================================

#[test]
fn agent_has_edit_agent_title() {
    let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
    script.is_agent = true;
    script.is_script = false;
    let actions = get_script_context_actions(&script);
    let edit = find_action(&actions, "edit_script").unwrap();
    assert_eq!(edit.title, "Edit Agent");
}

#[test]
fn agent_has_no_view_logs() {
    let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
    script.is_agent = true;
    script.is_script = false;
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(!ids.contains(&"view_logs"));
}

#[test]
fn agent_has_reveal_copy_content() {
    let mut script = ScriptInfo::new("my-agent", "/path/agent.md");
    script.is_agent = true;
    script.is_script = false;
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"reveal_in_finder"));
    assert!(ids.contains(&"copy_path"));
    assert!(ids.contains(&"copy_content"));
}

#[test]
fn agent_with_shortcut_and_alias() {
    let mut script = ScriptInfo::with_shortcut_and_alias(
        "my-agent",
        "/path/agent.md",
        Some("cmd+a".to_string()),
        Some("agt".to_string()),
    );
    script.is_agent = true;
    script.is_script = false;
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    // Agents get no shortcut/alias preference actions
    // (ScriptContextPreferenceActionPlan::AgentNoPreferenceActions).
    assert!(!ids.contains(&"update_shortcut"));
    assert!(!ids.contains(&"remove_shortcut"));
    assert!(!ids.contains(&"update_alias"));
    assert!(!ids.contains(&"remove_alias"));
    assert!(!ids.contains(&"add_shortcut"));
    assert!(!ids.contains(&"add_alias"));
}

#[test]
fn agent_suggested_has_reset_ranking() {
    let mut script =
        ScriptInfo::new("my-agent", "/path/agent.md").with_frecency(true, Some("/path".into()));
    script.is_agent = true;
    script.is_script = false;
    let actions = get_script_context_actions(&script);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"reset_ranking"));
}

// =========================================================================
// 11. Clipboard actions — comprehensive conditional branches
// =========================================================================

fn make_text_entry(pinned: bool, app_name: Option<&str>) -> ClipboardEntryInfo {
    ClipboardEntryInfo {
        id: "entry-1".to_string(),
        content_type: ContentType::Text,
        pinned,
        preview: "Hello, world!".to_string(),
        image_dimensions: None,
        frontmost_app_name: app_name.map(|s| s.to_string()),
    }
}

fn make_image_entry(pinned: bool) -> ClipboardEntryInfo {
    ClipboardEntryInfo {
        id: "entry-2".to_string(),
        content_type: ContentType::Image,
        pinned,
        preview: String::new(),
        image_dimensions: Some((800, 600)),
        frontmost_app_name: Some("Preview".to_string()),
    }
}

#[test]
fn clipboard_text_unpinned_has_pin() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"clip:clipboard_pin"));
    assert!(!ids.contains(&"clip:clipboard_unpin"));
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
fn clipboard_paste_title_shows_app_name() {
    let entry = make_text_entry(false, Some("VS Code"));
    let actions = get_clipboard_history_context_actions(&entry);
    let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
    assert_eq!(paste.title, "Paste to VS Code");
}

#[test]
fn clipboard_paste_title_fallback() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let paste = find_action(&actions, "clip:clipboard_paste").unwrap();
    assert_eq!(paste.title, "Paste to Active App");
}

#[test]
fn clipboard_text_no_ocr() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    assert!(!ids.contains(&"clip:clipboard_ocr"));
}

#[test]
fn clipboard_image_has_ocr() {
    let entry = make_image_entry(false);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"clip:clipboard_ocr"));
}

#[test]
fn clipboard_destructive_always_last_three() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    let len = ids.len();
    assert!(len >= 3);
    assert_eq!(ids[len - 3], "clip:clipboard_delete");
    assert_eq!(ids[len - 2], "clip:clipboard_delete_multiple");
    assert_eq!(ids[len - 1], "clip:clipboard_delete_all");
}

#[test]
fn clipboard_paste_always_first() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    assert_eq!(ids[0], "clip:clipboard_paste");
}

#[test]
fn clipboard_copy_always_second() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let ids = action_ids(&actions);
    assert_eq!(ids[1], "clip:clipboard_copy");
}

#[test]
fn clipboard_image_has_more_actions_than_text() {
    let text = make_text_entry(false, None);
    let image = make_image_entry(false);
    let text_count = get_clipboard_history_context_actions(&text).len();
    let image_count = get_clipboard_history_context_actions(&image).len();
    assert!(
        image_count > text_count,
        "Image({}) should have more actions than text({})",
        image_count,
        text_count
    );
}

// =========================================================================
// 12. Chat context actions — edge cases
// =========================================================================

#[test]
fn chat_zero_models() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };
    let actions = get_chat_context_actions(&info);
    // Root actions: change_model + continue_in_chat + capture_screen_area (no copy_response, no clear)
    assert!(actions.iter().any(|a| a.id == "chat:change_model"));
    assert!(actions.iter().any(|a| a.id == "chat:continue_in_chat"));
    // No flat model rows in root — models live in the drill-down picker
    assert!(
        !actions
            .iter()
            .any(|a| a.id.starts_with("chat:select_model_"))
    );
}

#[test]
fn chat_current_model_checkmark() {
    let info = ChatPromptInfo {
        current_model: Some("Claude 3".to_string()),
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
    // Model rows live in the drill-down picker, not the root context actions
    let picker = get_chat_model_picker_actions(&info);
    let claude = find_action(&picker, "chat:select_model_claude-3").unwrap();
    assert!(claude.title.contains('✓'));
    let gpt = find_action(&picker, "chat:select_model_gpt-4").unwrap();
    assert!(!gpt.title.contains('✓'));
}

#[test]
fn chat_has_response_and_messages() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: true,
        has_response: true,
    };
    let actions = get_chat_context_actions(&info);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"chat:copy_response"));
    assert!(ids.contains(&"chat:clear_conversation"));
}

#[test]
fn chat_no_response_no_messages() {
    let info = ChatPromptInfo {
        current_model: None,
        available_models: vec![],
        has_messages: false,
        has_response: false,
    };
    let actions = get_chat_context_actions(&info);
    let ids = action_ids(&actions);
    assert!(!ids.contains(&"chat:copy_response"));
    assert!(!ids.contains(&"chat:clear_conversation"));
}

#[test]
fn chat_model_description_has_provider() {
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
    // Model rows live in the drill-down picker
    let picker = get_chat_model_picker_actions(&info);
    let model_action = find_action(&picker, "chat:select_model_claude-3").unwrap();
    assert_eq!(model_action.description, Some("Uses Anthropic".to_string()));
}

// =========================================================================
// 13. Notes command bar — all 8 permutations
// =========================================================================

fn notes_action_ids(has_sel: bool, trash: bool, auto: bool) -> Vec<String> {
    let info = NotesInfo {
        has_selection: has_sel,
        is_trash_view: trash,
        auto_sizing_enabled: auto,
    };
    get_notes_command_bar_actions(&info)
        .iter()
        .map(|a| a.id.clone())
        .collect()
}

#[test]
fn notes_new_note_always_present() {
    for &sel in &[false, true] {
        for &trash in &[false, true] {
            for &auto in &[false, true] {
                let ids = notes_action_ids(sel, trash, auto);
                assert!(
                    ids.contains(&"new_note".to_string()),
                    "new_note missing for sel={}, trash={}, auto={}",
                    sel,
                    trash,
                    auto
                );
            }
        }
    }
}

#[test]
fn notes_browse_notes_always_present() {
    for &sel in &[false, true] {
        for &trash in &[false, true] {
            for &auto in &[false, true] {
                let ids = notes_action_ids(sel, trash, auto);
                assert!(
                    ids.contains(&"browse_notes".to_string()),
                    "browse_notes missing for sel={}, trash={}, auto={}",
                    sel,
                    trash,
                    auto
                );
            }
        }
    }
}

// --- merged from part_03.rs ---

#[test]
fn notes_duplicate_only_when_selected_and_not_trash() {
    assert!(notes_action_ids(true, false, false).contains(&"duplicate_note".to_string()));
    assert!(!notes_action_ids(false, false, false).contains(&"duplicate_note".to_string()));
    assert!(!notes_action_ids(true, true, false).contains(&"duplicate_note".to_string()));
}

#[test]
fn notes_edit_actions_only_when_selected_and_not_trash() {
    let with = notes_action_ids(true, false, false);
    assert!(with.contains(&"find_in_note".to_string()));
    assert!(with.contains(&"format".to_string()));

    let without_sel = notes_action_ids(false, false, false);
    assert!(!without_sel.contains(&"find_in_note".to_string()));

    let trash = notes_action_ids(true, true, false);
    assert!(!trash.contains(&"find_in_note".to_string()));
}

#[test]
fn notes_auto_sizing_toggle() {
    assert!(notes_action_ids(false, false, false).contains(&"toggle_auto_sizing".to_string()));
    assert!(notes_action_ids(false, false, true).contains(&"toggle_auto_sizing".to_string()));
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
        assert!(action.icon.is_some(), "Action '{}' missing icon", action.id);
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
            "Action '{}' missing section",
            action.id
        );
    }
}

// =========================================================================
// 14. AI command bar actions
// =========================================================================

#[test]
fn ai_command_bar_all_twelve_ids() {
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
        "chat:add_image",
        "chat:paste_image",
        "chat:change_model",
        "chat:export_markdown",
        "chat:branch_from_last",
        "chat:toggle_shortcuts_help",
        "chat:capture_screen_area",
        "chat:inspect_context",
    ];
    for id in &expected {
        assert!(ids.contains(id), "Missing AI action: {}", id);
    }
    assert_eq!(actions.len(), 38);
}

#[test]
fn ai_command_bar_section_ordering() {
    let actions = get_ai_command_bar_actions();
    let sections: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.section.as_deref())
        .collect();
    // Response comes before Actions, Actions before Attachments, Attachments before Settings
    let resp_idx = sections.iter().position(|&s| s == "Response").unwrap();
    let act_idx = sections.iter().position(|&s| s == "Actions").unwrap();
    let att_idx = sections.iter().position(|&s| s == "Attachments").unwrap();
    let set_idx = sections.iter().position(|&s| s == "Settings").unwrap();
    assert!(resp_idx < act_idx);
    assert!(act_idx < att_idx);
    assert!(att_idx < set_idx);
}

#[test]
fn ai_command_bar_all_have_icons() {
    for action in &get_ai_command_bar_actions() {
        assert!(
            action.icon.is_some(),
            "AI action '{}' missing icon",
            action.id
        );
    }
}

// =========================================================================
// 15. Note switcher actions
// =========================================================================

#[test]
fn note_switcher_empty_shows_no_notes() {
    let actions = get_note_switcher_actions(&[]);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].id, "no_notes");
    assert_eq!(actions[0].title, "No notes yet");
}

#[test]
fn note_switcher_current_has_bullet() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "My Note".to_string(),
        char_count: 42,
        is_current: true,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert!(actions[0].title.starts_with("• "));
}

#[test]
fn note_switcher_non_current_no_bullet() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "My Note".to_string(),
        char_count: 42,
        is_current: false,
        is_pinned: false,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert!(!actions[0].title.starts_with("• "));
}

#[test]
fn note_switcher_pinned_gets_star_icon() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "Pinned".to_string(),
        char_count: 10,
        is_current: false,
        is_pinned: true,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    assert_eq!(actions[0].icon, Some(IconName::StarFilled));
}

#[test]
fn note_switcher_current_gets_check_icon() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "Current".to_string(),
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
fn note_switcher_pinned_priority_over_current() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "Both".to_string(),
        char_count: 10,
        is_current: true,
        is_pinned: true,
        preview: String::new(),
        relative_time: String::new(),
    }];
    let actions = get_note_switcher_actions(&notes);
    // Pinned takes priority: StarFilled
    assert_eq!(actions[0].icon, Some(IconName::StarFilled));
}

#[test]
fn note_switcher_char_count_singular() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "Single".to_string(),
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
fn note_switcher_char_count_plural() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
        title: "Multi".to_string(),
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
fn note_switcher_char_count_zero() {
    let notes = vec![NoteSwitcherNoteInfo {
        id: "abc".to_string(),
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

#[test]
fn note_switcher_all_have_notes_section() {
    let notes = vec![
        NoteSwitcherNoteInfo {
            id: "1".to_string(),
            title: "A".to_string(),
            char_count: 1,
            is_current: true,
            is_pinned: false,
            preview: String::new(),
            relative_time: String::new(),
        },
        NoteSwitcherNoteInfo {
            id: "2".to_string(),
            title: "B".to_string(),
            char_count: 2,
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
// 16. New chat actions
// =========================================================================

#[test]
fn new_chat_empty_inputs() {
    let actions = get_new_chat_actions(&[], &[], &[]);
    assert!(actions.is_empty());
}

#[test]
fn new_chat_section_ordering() {
    let last_used = vec![NewChatModelInfo {
        model_id: "c3".to_string(),
        display_name: "Claude 3".to_string(),
        provider: "anthropic".to_string(),
        provider_display_name: "Anthropic".to_string(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "gen".to_string(),
        name: "General".to_string(),
        icon: IconName::Star,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "g4".to_string(),
        display_name: "GPT-4".to_string(),
        provider: "openai".to_string(),
        provider_display_name: "OpenAI".to_string(),
    }];
    let actions = get_new_chat_actions(&last_used, &presets, &models);
    let sections: Vec<&str> = actions
        .iter()
        .filter_map(|a| a.section.as_deref())
        .collect();
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
fn new_chat_all_have_icons() {
    let last_used = vec![NewChatModelInfo {
        model_id: "c3".to_string(),
        display_name: "Claude 3".to_string(),
        provider: "anthropic".to_string(),
        provider_display_name: "Anthropic".to_string(),
    }];
    let presets = vec![NewChatPresetInfo {
        id: "gen".to_string(),
        name: "General".to_string(),
        icon: IconName::Star,
    }];
    let models = vec![NewChatModelInfo {
        model_id: "g4".to_string(),
        display_name: "GPT-4".to_string(),
        provider: "openai".to_string(),
        provider_display_name: "OpenAI".to_string(),
    }];
    let actions = get_new_chat_actions(&last_used, &presets, &models);
    for action in &actions {
        assert!(
            action.icon.is_some(),
            "New chat action '{}' missing icon",
            action.id
        );
    }
}

#[test]
fn new_chat_last_used_has_bolt_icon() {
    let last_used = vec![NewChatModelInfo {
        model_id: "c3".to_string(),
        display_name: "Claude 3".to_string(),
        provider: "anthropic".to_string(),
        provider_display_name: "Anthropic".to_string(),
    }];
    let actions = get_new_chat_actions(&last_used, &[], &[]);
    assert_eq!(actions[0].icon, Some(IconName::BoltFilled));
}

#[test]
fn new_chat_models_has_settings_icon() {
    let models = vec![NewChatModelInfo {
        model_id: "g4".to_string(),
        display_name: "GPT-4".to_string(),
        provider: "openai".to_string(),
        provider_display_name: "OpenAI".to_string(),
    }];
    let actions = get_new_chat_actions(&[], &[], &models);
    assert_eq!(actions[0].icon, Some(IconName::Settings));
}

// =========================================================================
// 17. CommandBarConfig presets
// =========================================================================

#[test]
fn command_bar_default_config() {
    let config = CommandBarConfig::default();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert_eq!(config.dialog_config.anchor, AnchorPosition::Bottom);
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
    assert!(config.close_on_select);
    assert!(config.close_on_click_outside);
    assert!(config.close_on_escape);
}

#[test]
fn command_bar_ai_style() {
    let config = CommandBarConfig::ai_style();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
    assert!(config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
}

#[test]
fn command_bar_notes_style() {
    let config = CommandBarConfig::notes_style();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert_eq!(config.dialog_config.anchor, AnchorPosition::Top);
    assert!(config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
}

#[test]
fn command_bar_no_search() {
    let config = CommandBarConfig::no_search();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Hidden);
}

#[test]
fn command_bar_main_menu_style() {
    let config = CommandBarConfig::main_menu_style();
    assert_eq!(config.dialog_config.search_position, SearchPosition::Top);
    assert_eq!(config.dialog_config.section_style, SectionStyle::Headers);
    assert!(!config.dialog_config.show_icons);
    assert!(!config.dialog_config.show_footer);
}

// =========================================================================
// 18. Action lowercase caching
// =========================================================================

#[test]
fn action_title_lower_is_cached() {
    let action = Action::new(
        "test",
        "UPPERCASE TITLE",
        None,
        ActionCategory::ScriptContext,
    );
    assert_eq!(action.title_lower, "uppercase title");
}

// --- merged from part_04.rs ---

#[test]
fn action_description_lower_is_cached() {
    let action = Action::new(
        "test",
        "title",
        Some("UPPERCASE DESC".to_string()),
        ActionCategory::ScriptContext,
    );
    assert_eq!(action.description_lower, Some("uppercase desc".to_string()));
}

#[test]
fn action_description_lower_none_when_no_description() {
    let action = Action::new("test", "title", None, ActionCategory::ScriptContext);
    assert!(action.description_lower.is_none());
}

#[test]
fn action_shortcut_lower_is_cached_after_with_shortcut() {
    let action =
        Action::new("test", "title", None, ActionCategory::ScriptContext).with_shortcut("⌘⇧C");
    assert_eq!(action.shortcut_lower, Some("⌘⇧c".to_string()));
}

#[test]
fn action_shortcut_lower_none_by_default() {
    let action = Action::new("test", "title", None, ActionCategory::ScriptContext);
    assert!(action.shortcut_lower.is_none());
}

// =========================================================================
// 19. Action with_shortcut_opt
// =========================================================================

#[test]
fn action_with_shortcut_opt_some() {
    let action = Action::new("test", "title", None, ActionCategory::ScriptContext)
        .with_shortcut_opt(Some("⌘X".to_string()));
    assert_eq!(action.shortcut, Some("⌘X".to_string()));
    assert_eq!(action.shortcut_lower, Some("⌘x".to_string()));
}

#[test]
fn action_with_shortcut_opt_none() {
    let action =
        Action::new("test", "title", None, ActionCategory::ScriptContext).with_shortcut_opt(None);
    assert!(action.shortcut.is_none());
    assert!(action.shortcut_lower.is_none());
}

// =========================================================================
// 20. Action builder chain
// =========================================================================

#[test]
fn action_builder_chain_all_methods() {
    let action = Action::new(
        "test",
        "Test",
        Some("desc".to_string()),
        ActionCategory::ScriptContext,
    )
    .with_shortcut("⌘T")
    .with_icon(IconName::Star)
    .with_section("Section");

    assert_eq!(action.id, "test");
    assert_eq!(action.title, "Test");
    assert_eq!(action.description, Some("desc".to_string()));
    assert_eq!(action.shortcut, Some("⌘T".to_string()));
    assert_eq!(action.icon, Some(IconName::Star));
    assert_eq!(action.section, Some("Section".to_string()));
    assert!(!action.has_action);
    assert!(action.value.is_none());
}

#[test]
fn action_has_action_default_false() {
    let action = Action::new("test", "Test", None, ActionCategory::ScriptContext);
    assert!(!action.has_action);
}

// =========================================================================
// 21. ProtocolAction constructors and methods
// =========================================================================

#[test]
fn protocol_action_new_defaults() {
    let pa = ProtocolAction::new("Test".to_string());
    assert_eq!(pa.name, "Test");
    assert!(pa.description.is_none());
    assert!(pa.shortcut.is_none());
    assert!(pa.value.is_none());
    assert!(!pa.has_action);
    assert!(pa.visible.is_none());
    assert!(pa.close.is_none());
    assert!(pa.is_visible()); // None defaults to true
    assert!(pa.should_close()); // None defaults to true
}

#[test]
fn protocol_action_with_value() {
    let pa = ProtocolAction::with_value("Submit".to_string(), "submit-val".to_string());
    assert_eq!(pa.value, Some("submit-val".to_string()));
    assert!(!pa.has_action);
}

#[test]
fn protocol_action_visibility_combinations() {
    assert!(
        ProtocolAction {
            visible: None,
            ..ProtocolAction::new("a".into())
        }
        .is_visible()
    );
    assert!(
        ProtocolAction {
            visible: Some(true),
            ..ProtocolAction::new("a".into())
        }
        .is_visible()
    );
    assert!(
        !ProtocolAction {
            visible: Some(false),
            ..ProtocolAction::new("a".into())
        }
        .is_visible()
    );
}

#[test]
fn protocol_action_close_combinations() {
    assert!(
        ProtocolAction {
            close: None,
            ..ProtocolAction::new("a".into())
        }
        .should_close()
    );
    assert!(
        ProtocolAction {
            close: Some(true),
            ..ProtocolAction::new("a".into())
        }
        .should_close()
    );
    assert!(
        !ProtocolAction {
            close: Some(false),
            ..ProtocolAction::new("a".into())
        }
        .should_close()
    );
}

// =========================================================================
// 22. File context actions
// =========================================================================

#[test]
fn file_context_directory_primary_action() {
    let info = FileInfo {
        name: "Documents".to_string(),
        path: "/Users/test/Documents".to_string(),
        is_dir: true,
        file_type: FileType::Directory,
    };
    let actions = get_file_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_directory");
    assert!(actions[0].title.contains("Documents"));
}

#[test]
fn file_context_file_primary_action() {
    let info = FileInfo {
        name: "readme.md".to_string(),
        path: "/Users/test/readme.md".to_string(),
        is_dir: false,
        file_type: FileType::File,
    };
    let actions = get_file_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_file");
    assert!(actions[0].title.contains("readme.md"));
}

#[test]
fn file_context_common_actions() {
    let info = FileInfo {
        name: "test.txt".to_string(),
        path: "/test.txt".to_string(),
        is_dir: false,
        file_type: FileType::File,
    };
    let actions = get_file_context_actions(&info);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"file:reveal_in_finder"));
    assert!(ids.contains(&"file:copy_path"));
    assert!(ids.contains(&"file:copy_filename"));
}

// =========================================================================
// 23. Path context actions
// =========================================================================

#[test]
fn path_context_directory_has_open_directory() {
    let info = PathInfo {
        name: "src".to_string(),
        path: "/Users/test/src".to_string(),
        is_dir: true,
    };
    let actions = get_path_context_actions(&info);
    assert_eq!(actions[0].id, "file:open_directory");
}

#[test]
fn path_context_file_has_select_file() {
    let info = PathInfo {
        name: "file.txt".to_string(),
        path: "/Users/test/file.txt".to_string(),
        is_dir: false,
    };
    let actions = get_path_context_actions(&info);
    assert_eq!(actions[0].id, "file:select_file");
}

#[test]
fn path_context_trash_description_folder_vs_file() {
    let dir_info = PathInfo {
        name: "src".to_string(),
        path: "/src".to_string(),
        is_dir: true,
    };
    let file_info = PathInfo {
        name: "f.txt".to_string(),
        path: "/f.txt".to_string(),
        is_dir: false,
    };
    let dir_actions = get_path_context_actions(&dir_info);
    let dir_trash = find_action(&dir_actions, "file:move_to_trash").unwrap();
    let file_actions = get_path_context_actions(&file_info);
    let file_trash = find_action(&file_actions, "file:move_to_trash").unwrap();
    assert!(dir_trash.description.as_ref().unwrap().contains("folder"));
    assert!(file_trash.description.as_ref().unwrap().contains("file"));
}

#[test]
fn path_context_common_actions() {
    let info = PathInfo {
        name: "test".to_string(),
        path: "/test".to_string(),
        is_dir: false,
    };
    let actions = get_path_context_actions(&info);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"file:copy_path"));
    assert!(ids.contains(&"file:open_in_finder"));
    assert!(ids.contains(&"file:open_in_editor"));
    assert!(ids.contains(&"file:open_in_quick_terminal"));
    assert!(ids.contains(&"file:copy_filename"));
    assert!(ids.contains(&"file:move_to_trash"));
}

// =========================================================================
// 24. Scriptlet with custom actions
// =========================================================================

#[test]
fn scriptlet_custom_actions_ordering() {
    let script = ScriptInfo::scriptlet("Test Scriptlet", "/path/test.md", None, None);
    let scriptlet = Scriptlet::new(
        "Test Scriptlet".to_string(),
        "bash".to_string(),
        "echo hi".to_string(),
    );
    let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
    // run_script should always be first
    assert_eq!(actions[0].id, "run_script");
}

#[test]
fn scriptlet_with_custom_action_has_has_action_true() {
    let script = ScriptInfo::scriptlet("Test Scriptlet", "/path/test.md", None, None);
    let mut scriptlet = Scriptlet::new(
        "Test Scriptlet".to_string(),
        "bash".to_string(),
        "echo hi".to_string(),
    );
    scriptlet.actions.push(ScriptletAction {
        name: "Copy to Clipboard".to_string(),
        command: "copy-to-clipboard".to_string(),
        tool: "bash".to_string(),
        code: "pbcopy".to_string(),
        inputs: vec![],
        shortcut: Some("cmd+c".to_string()),
        description: None,
    });
    let actions = get_scriptlet_context_actions_with_custom(&script, Some(&scriptlet));
    let custom = actions
        .iter()
        .find(|a| a.id.starts_with("scriptlet_action:"))
        .unwrap();
    assert!(custom.has_action);
    assert!(custom.value.is_some());
}

// =========================================================================
// 25. ID uniqueness across contexts
// =========================================================================

#[test]
fn script_context_no_duplicate_ids() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    let mut ids: Vec<&str> = action_ids(&actions);
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "Duplicate IDs found in script context");
}

#[test]
fn clipboard_context_no_duplicate_ids() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    let mut ids: Vec<&str> = action_ids(&actions);
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "Duplicate IDs found in clipboard context");
}

#[test]
fn ai_command_bar_no_duplicate_ids() {
    let actions = get_ai_command_bar_actions();
    let mut ids: Vec<&str> = action_ids(&actions);
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "Duplicate IDs found in AI command bar");
}

#[test]
fn path_context_no_duplicate_ids() {
    let info = PathInfo {
        name: "test".to_string(),
        path: "/test".to_string(),
        is_dir: false,
    };
    let actions = get_path_context_actions(&info);
    let mut ids: Vec<&str> = action_ids(&actions);
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(total, ids.len(), "Duplicate IDs found in path context");
}

// =========================================================================
// 26. Enum defaults
// =========================================================================

#[test]
fn enum_defaults() {
    assert_eq!(SearchPosition::default(), SearchPosition::Top);
    assert_eq!(SectionStyle::default(), SectionStyle::Headers);
    assert_eq!(AnchorPosition::default(), AnchorPosition::Bottom);
}

#[test]
fn actions_dialog_config_default() {
    let config = ActionsDialogConfig::default();
    assert_eq!(config.search_position, SearchPosition::Top);
    assert_eq!(config.section_style, SectionStyle::Headers);
    assert_eq!(config.anchor, AnchorPosition::Bottom);
    assert!(!config.show_icons);
    assert!(!config.show_footer);
}

// =========================================================================
// 27. Action categories
// =========================================================================

#[test]
fn all_script_context_actions_use_script_context_category() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    for action in &actions {
        assert_eq!(
            action.category,
            ActionCategory::ScriptContext,
            "Action '{}' has wrong category",
            action.id
        );
    }
}

#[test]
fn all_clipboard_actions_use_script_context_category() {
    let entry = make_text_entry(false, None);
    let actions = get_clipboard_history_context_actions(&entry);
    for action in &actions {
        assert_eq!(
            action.category,
            ActionCategory::ScriptContext,
            "Clipboard action '{}' has wrong category",
            action.id
        );
    }
}

// =========================================================================
// 28. Snake_case ID convention
// =========================================================================

#[test]
fn script_action_ids_are_snake_case() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    for action in &get_script_context_actions(&script) {
        assert!(
            !action.id.contains(' '),
            "Action ID '{}' contains space",
            action.id
        );
        assert!(
            action.id == action.id.to_lowercase()
                || action
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_'),
            "Action ID '{}' should be snake_case",
            action.id
        );
    }
}

#[test]
fn clipboard_action_ids_are_snake_case() {
    let entry = make_text_entry(false, None);
    for action in &get_clipboard_history_context_actions(&entry) {
        assert!(
            !action.id.contains(' '),
            "Clipboard action ID '{}' contains space",
            action.id
        );
    }
}

// --- merged from part_05.rs ---

// =========================================================================
// 29. Deeplink in script actions
// =========================================================================

#[test]
fn script_deeplink_description_uses_portable_share_copy() {
    let script = ScriptInfo::new("My Cool Script", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    let deeplink = find_action(&actions, "copy_deeplink").unwrap();
    assert_eq!(deeplink.title, "Share");
    assert_eq!(
        deeplink.description.as_deref(),
        Some("Copy a portable Script Kit share link to clipboard")
    );
}

#[test]
fn builtin_also_has_deeplink() {
    let builtin = ScriptInfo::builtin("Clipboard History");
    let actions = get_script_context_actions(&builtin);
    let ids = action_ids(&actions);
    assert!(ids.contains(&"copy_deeplink"));
}

// =========================================================================
// 30. Primary action verb propagation
// =========================================================================

#[test]
fn primary_action_uses_action_verb() {
    let script = ScriptInfo::with_action_verb("App Launcher", "builtin:launcher", false, "Launch");
    let actions = get_script_context_actions(&script);
    let run = find_action(&actions, "run_script").unwrap();
    assert!(run.title.starts_with("Launch"));
    assert!(run.description.as_ref().unwrap().contains("Launch"),);
}

#[test]
fn default_action_verb_is_run() {
    let script = ScriptInfo::new("test", "/path/test.ts");
    let actions = get_script_context_actions(&script);
    let run = find_action(&actions, "run_script").unwrap();
    assert!(run.title.starts_with("Run"));
}
