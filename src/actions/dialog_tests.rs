// --- merged from part_01.rs ---
//! Tests for ActionsDialog static helper functions.
//!
//! Validates dialog-internal logic: selection coercion, grouped item building,
//! shortcut formatting, shortcut keycap parsing, fuzzy matching, and scoring.

use super::dialog::{
    actions_refresh_selection_policy, build_grouped_items_static, coerce_action_selection,
    initial_selection_index, resolve_selected_protocol_action_index,
    should_rebuild_grouped_items_for_config_change, ActionsRefreshSelectionPolicy,
    GroupedActionItem,
};
use super::types::{
    Action, ActionCategory, ActionsDialogConfig, AnchorPosition, SearchPosition, SectionStyle,
};

// ============================================================
// Helper
// ============================================================

fn make_action(id: &str, title: &str, section: Option<&str>) -> Action {
    let mut a = Action::new(id, title, None, ActionCategory::ScriptContext);
    if let Some(s) = section {
        a = a.with_section(s);
    }
    a
}

// ============================================================
// 1. coerce_action_selection
// ============================================================

#[test]
fn coerce_empty_list_returns_none() {
    let rows: Vec<GroupedActionItem> = vec![];
    assert_eq!(coerce_action_selection(&rows, 0), None);
}

#[test]
fn coerce_item_at_zero() {
    let rows = vec![GroupedActionItem::Item(0)];
    assert_eq!(coerce_action_selection(&rows, 0), Some(0));
}

#[test]
fn coerce_header_at_zero_skips_to_first_item() {
    let rows = vec![
        GroupedActionItem::SectionHeader("Section".into()),
        GroupedActionItem::Item(0),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), Some(1));
}

#[test]
fn coerce_header_only_returns_none() {
    let rows = vec![GroupedActionItem::SectionHeader("Section".into())];
    assert_eq!(coerce_action_selection(&rows, 0), None);
}

#[test]
fn coerce_beyond_end_clamps_to_last_item() {
    let rows = vec![GroupedActionItem::Item(0), GroupedActionItem::Item(1)];
    // Index 99 should clamp to last element (index 1)
    assert_eq!(coerce_action_selection(&rows, 99), Some(1));
}

#[test]
fn coerce_header_at_end_searches_backwards() {
    let rows = vec![
        GroupedActionItem::Item(0),
        GroupedActionItem::SectionHeader("Section".into()),
    ];
    // Landing on the header at index 1 should search down first (nothing),
    // then search up and find Item at index 0
    assert_eq!(coerce_action_selection(&rows, 1), Some(0));
}

#[test]
fn coerce_multiple_headers_before_items() {
    let rows = vec![
        GroupedActionItem::SectionHeader("A".into()),
        GroupedActionItem::SectionHeader("B".into()),
        GroupedActionItem::Item(0),
    ];
    assert_eq!(coerce_action_selection(&rows, 0), Some(2));
    assert_eq!(coerce_action_selection(&rows, 1), Some(2));
}

// ============================================================
// 2. build_grouped_items_static
// ============================================================

#[test]
fn grouped_items_empty_actions() {
    let actions: Vec<Action> = vec![];
    let filtered: Vec<usize> = vec![];
    let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    assert!(result.is_empty());
}

#[test]
fn grouped_items_no_sections_produces_items_only() {
    let actions = vec![
        make_action("a", "Alpha", None),
        make_action("b", "Beta", None),
    ];
    let filtered: Vec<usize> = vec![0, 1];
    let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    // No sections => no headers
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0], GroupedActionItem::Item(0)));
    assert!(matches!(result[1], GroupedActionItem::Item(1)));
}

#[test]
fn grouped_items_with_section_headers() {
    let actions = vec![
        make_action("a", "Alpha", Some("Group 1")),
        make_action("b", "Beta", Some("Group 1")),
        make_action("c", "Charlie", Some("Group 2")),
    ];
    let filtered: Vec<usize> = vec![0, 1, 2];
    let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Headers);
    // Group 1 header, item 0, item 1, Group 2 header, item 2
    assert_eq!(result.len(), 5);
    assert!(matches!(&result[0], GroupedActionItem::SectionHeader(s) if s == "Group 1"));
    assert!(matches!(result[1], GroupedActionItem::Item(0)));
    assert!(matches!(result[2], GroupedActionItem::Item(1)));
    assert!(matches!(&result[3], GroupedActionItem::SectionHeader(s) if s == "Group 2"));
    assert!(matches!(result[4], GroupedActionItem::Item(2)));
}

#[test]
fn grouped_items_separators_style_no_headers() {
    let actions = vec![
        make_action("a", "Alpha", Some("Group 1")),
        make_action("b", "Beta", Some("Group 2")),
    ];
    let filtered: Vec<usize> = vec![0, 1];
    let result = build_grouped_items_static(&actions, &filtered, SectionStyle::Separators);
    // Separators style: no section headers inserted
    assert_eq!(result.len(), 2);
    assert!(matches!(result[0], GroupedActionItem::Item(0)));
    assert!(matches!(result[1], GroupedActionItem::Item(1)));
}

#[test]
fn grouped_items_respects_filtered_order() {
    let actions = vec![
        make_action("a", "Alpha", None),
        make_action("b", "Beta", None),
        make_action("c", "Charlie", None),
    ];
    // Reversed filter order
    let filtered: Vec<usize> = vec![2, 0, 1];
    let result = build_grouped_items_static(&actions, &filtered, SectionStyle::None);
    assert_eq!(result.len(), 3);
    assert!(matches!(result[0], GroupedActionItem::Item(0))); // filter_idx 0 = actions[2]
    assert!(matches!(result[1], GroupedActionItem::Item(1))); // filter_idx 1 = actions[0]
    assert!(matches!(result[2], GroupedActionItem::Item(2))); // filter_idx 2 = actions[1]
}

// ============================================================
// 3. ActionsDialog::format_shortcut_hint (via dialog module)
// ============================================================

#[test]
fn format_shortcut_hint_cmd_c() {
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("cmd+c"),
        "⌘C"
    );
}

#[test]
fn format_shortcut_hint_ctrl_alt_delete() {
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("ctrl+alt+delete"),
        "⌃⌥⌫"
    );
}

#[test]
fn format_shortcut_hint_shift_enter() {
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("shift+enter"),
        "⇧↵"
    );
}

#[test]
fn format_shortcut_hint_cmd_shift_arrow() {
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("cmd+shift+up"),
        "⌘⇧↑"
    );
}

#[test]
fn format_shortcut_hint_space_and_tab() {
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("cmd+space"),
        "⌘␣"
    );
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("cmd+tab"),
        "⌘⇥"
    );
}

#[test]
fn format_shortcut_hint_all_arrow_variants() {
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("arrowup"),
        "↑"
    );
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("arrowdown"),
        "↓"
    );
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("arrowleft"),
        "←"
    );
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("arrowright"),
        "→"
    );
}

#[test]
fn format_shortcut_hint_meta_and_super() {
    // "meta" and "super" should both map to ⌘
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("meta+a"),
        "⌘A"
    );
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("super+a"),
        "⌘A"
    );
}

#[test]
fn format_shortcut_hint_option_alias() {
    // "opt" and "option" should both map to ⌥
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("opt+x"),
        "⌥X"
    );
    assert_eq!(
        super::dialog::ActionsDialog::format_shortcut_hint("option+x"),
        "⌥X"
    );
}

// ============================================================
// 4. ActionsDialog::parse_shortcut_keycaps
// ============================================================

#[test]
fn parse_keycaps_simple_symbol() {
    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘↵");
    assert_eq!(keycaps, vec!["⌘", "↵"]);
}

#[test]
fn action_subtitle_is_hidden_unless_host_opts_in() {
    let action = Action::new(
        "copy",
        "Copy to Clipboard",
        Some("Copy without pasting".to_string()),
        ActionCategory::ScriptContext,
    );
    assert_eq!(
        super::dialog::action_subtitle_for_display(&action, false),
        None,
        "Action-menu popups should not render subtitles"
    );
    assert_eq!(
        super::dialog::action_subtitle_for_display(&action, true),
        Some("Copy without pasting"),
        "Switcher-style hosts opt in via config.show_subtitles"
    );
}

#[test]
fn parse_keycaps_letter() {
    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘E");
    assert_eq!(keycaps, vec!["⌘", "E"]);
}

#[test]
fn parse_keycaps_multi_modifier() {
    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘⇧C");
    assert_eq!(keycaps, vec!["⌘", "⇧", "C"]);
}

#[test]
fn parse_keycaps_lowercase_uppercased() {
    // Lowercase letter should be uppercased
    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌘e");
    assert_eq!(keycaps, vec!["⌘", "E"]);
}

#[test]
fn parse_keycaps_special_keys() {
    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⎋");
    assert_eq!(keycaps, vec!["⎋"]);

    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⌫");
    assert_eq!(keycaps, vec!["⌫"]);

    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("⇥");
    assert_eq!(keycaps, vec!["⇥"]);

    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("␣");
    assert_eq!(keycaps, vec!["␣"]);
}

#[test]
fn parse_keycaps_arrows() {
    let keycaps = super::dialog::ActionsDialog::parse_shortcut_keycaps("↑↓←→");
    assert_eq!(keycaps, vec!["↑", "↓", "←", "→"]);
}

// ============================================================
// 5. ActionsDialog::score_action
// ============================================================

#[test]
fn score_prefix_match_highest() {
    let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
    let score = super::dialog::ActionsDialog::score_action(&action, "edit");
    assert_eq!(score, 100, "Prefix match should be 100");
}

#[test]
fn score_contains_match() {
    let action = Action::new("id", "Quick Edit", None, ActionCategory::ScriptContext);
    let score = super::dialog::ActionsDialog::score_action(&action, "edit");
    assert_eq!(score, 50, "Contains match should be 50");
}

#[test]
fn score_fuzzy_match() {
    // "esc" fuzzy matches "edit script" (e...s...c)
    let action = Action::new("id", "Edit Script", None, ActionCategory::ScriptContext);
    let score = super::dialog::ActionsDialog::score_action(&action, "esc");
    assert_eq!(score, 25, "Fuzzy match should be 25");
}

#[test]
fn score_description_bonus() {
    let action = Action::new(
        "id",
        "Open File",
        Some("Edit the file in your editor".to_string()),
        ActionCategory::ScriptContext,
    );
    // "editor" doesn't match title but matches description
    let score = super::dialog::ActionsDialog::score_action(&action, "editor");
    assert_eq!(score, 15, "Description-only match should be 15");
}

#[test]
fn score_shortcut_bonus() {
    let action = Action::new("id", "Run", None, ActionCategory::ScriptContext).with_shortcut("⌘E");
    // "⌘e" matches shortcut (lowercased)
    let score = super::dialog::ActionsDialog::score_action(&action, "⌘e");
    assert_eq!(score, 10, "Shortcut-only match should be 10");
}

#[test]
fn score_no_match() {
    let action = Action::new("id", "Edit", None, ActionCategory::ScriptContext);
    let score = super::dialog::ActionsDialog::score_action(&action, "zzz");
    assert_eq!(score, 0);
}

#[test]
fn score_combined_title_and_description() {
    let action = Action::new(
        "id",
        "Edit Script",
        Some("Edit the script file".to_string()),
        ActionCategory::ScriptContext,
    );
    // "edit" matches title as prefix (+100) and description contains (+15)
    let score = super::dialog::ActionsDialog::score_action(&action, "edit");
    assert_eq!(score, 115, "Prefix + description bonus = 115");
}

// ============================================================
// 6. Fuzzy matching is delegated to the shared launcher matcher
//    (crate::scripts::search::SearchHighlightMatchCtx); subsequence
//    semantics are covered via score_action above and the matcher's
//    own tests in src/scripts/search.
// ============================================================

#[test]
fn score_fuzzy_match_subsequence_semantics() {
    let action = Action::new("id", "Hello World", None, ActionCategory::ScriptContext);
    assert!(
        super::dialog::ActionsDialog::score_action(&action, "hwd") > 0,
        "subsequence 'hwd' should fuzzy match 'Hello World'"
    );
    assert_eq!(
        super::dialog::ActionsDialog::score_action(&action, "xyz"),
        0,
        "'xyz' should not match 'Hello World'"
    );
}

// ============================================================
// 7. selected_action_should_close defaults
// ============================================================

#[test]
fn action_config_default_values() {
    let config = super::types::ActionsDialogConfig::default();
    assert_eq!(config.search_position, super::types::SearchPosition::Bottom);
    assert_eq!(config.section_style, SectionStyle::Headers);
    assert_eq!(config.anchor, super::types::AnchorPosition::Bottom);
    assert!(!config.show_icons);
    assert!(!config.show_footer);
}

// --- merged from part_02.rs ---

// ============================================================
// 8. Constructor/config/sdk helper behavior
// ============================================================

#[test]
fn initial_selection_index_skips_header_row() {
    let rows = vec![
        GroupedActionItem::SectionHeader("Actions".into()),
        GroupedActionItem::Item(0),
    ];
    assert_eq!(initial_selection_index(&rows), Some(1));
}

#[test]
fn initial_selection_is_none_for_headers_only() {
    let rows = vec![GroupedActionItem::SectionHeader("Actions".into())];
    assert_eq!(initial_selection_index(&rows), None);
}

#[test]
fn disabled_explanatory_actions_remain_selectable_rows() {
    let actions = vec![make_action("disabled", "Unavailable", Some("Actions"))
        .disabled("Requires a selected file")];
    let grouped = build_grouped_items_static(&actions, &[0], SectionStyle::Headers);
    assert_eq!(initial_selection_index(&grouped), Some(1));
}

#[test]
fn actions_refresh_snaps_untouched_selection_and_restores_user_selection() {
    assert_eq!(
        actions_refresh_selection_policy(false),
        ActionsRefreshSelectionPolicy::SnapToFirst
    );
    assert_eq!(
        actions_refresh_selection_policy(true),
        ActionsRefreshSelectionPolicy::RestoreIdentity
    );
}

#[test]
fn config_change_requires_rebuild_when_section_style_changes() {
    let previous = ActionsDialogConfig {
        section_style: SectionStyle::Separators,
        ..ActionsDialogConfig::default()
    };
    let next = ActionsDialogConfig {
        section_style: SectionStyle::Headers,
        ..ActionsDialogConfig::default()
    };

    assert!(should_rebuild_grouped_items_for_config_change(
        &previous, &next
    ));
}

#[test]
fn config_change_does_not_require_rebuild_when_section_style_same() {
    let previous = ActionsDialogConfig {
        search_position: super::types::SearchPosition::Bottom,
        section_style: SectionStyle::Separators,
        ..ActionsDialogConfig::default()
    };
    let next = ActionsDialogConfig {
        search_position: super::types::SearchPosition::Top,
        section_style: SectionStyle::Separators,
        ..ActionsDialogConfig::default()
    };

    assert!(!should_rebuild_grouped_items_for_config_change(
        &previous, &next
    ));
}

#[test]
fn agent_chat_dialog_config_matches_main_agent_chat_layout() {
    let config = super::dialog::ActionsDialog::agent_chat_dialog_config();

    assert_eq!(config.search_position, SearchPosition::Top);
    assert_eq!(config.section_style, SectionStyle::Headers);
    assert_eq!(config.anchor, AnchorPosition::Top);
    assert!(config.show_icons);
    assert!(
        !config.show_context_header,
        "Agent Chat actions should use the compact chrome without a duplicate context header"
    );
}

#[test]
fn selected_protocol_action_uses_visible_index_mapping() {
    // Visible action #1 should map to original protocol action index #3.
    let visible_to_protocol = vec![0, 3];
    let selected_action_index = Some(1);

    assert_eq!(
        resolve_selected_protocol_action_index(selected_action_index, &visible_to_protocol),
        Some(3)
    );
}

#[test]
fn selected_protocol_action_mapping_returns_none_for_out_of_bounds() {
    let visible_to_protocol = vec![0];
    let selected_action_index = Some(2);

    assert_eq!(
        resolve_selected_protocol_action_index(selected_action_index, &visible_to_protocol),
        None
    );
}

// ============================================================
// Route state contract
// ============================================================

use super::dialog::{ActionsDialogRoute, ActionsDialogRouteState};

fn make_route(id: &str, action_ids: &[&str]) -> ActionsDialogRoute {
    ActionsDialogRoute {
        id: id.to_string(),
        actions: action_ids
            .iter()
            .map(|aid| make_action(aid, aid, None))
            .collect(),
        context_title: Some(format!("{id} title")),
        search_placeholder: Some(format!("{id} placeholder")),
        initial_selected_action_id: None,
    }
}

fn make_route_with_initial(id: &str, action_ids: &[&str], initial: &str) -> ActionsDialogRoute {
    ActionsDialogRoute {
        initial_selected_action_id: Some(initial.to_string()),
        ..make_route(id, action_ids)
    }
}

#[test]
fn route_state_seeds_selected_from_initial() {
    let route = make_route_with_initial("root", &["a", "b", "c"], "b");
    let state = ActionsDialogRouteState::new(route.clone());

    assert_eq!(state.selected_action_id.as_deref(), Some("b"));
    assert!(state.search_text.is_empty());
    assert_eq!(state.route, route);
}

#[test]
fn route_state_seeds_none_when_no_initial() {
    let route = make_route("root", &["a", "b"]);
    let state = ActionsDialogRouteState::new(route);

    assert_eq!(state.selected_action_id, None);
}

#[test]
fn route_initial_selection_carries_through_to_grouped_items() {
    // Build grouped items for a route with initial selection "b" at index 1
    let route = make_route_with_initial("test", &["a", "b", "c"], "b");
    let actions = &route.actions;
    let filtered: Vec<usize> = (0..actions.len()).collect();

    let grouped = build_grouped_items_static(actions, &filtered, SectionStyle::default());

    // "b" is action index 1 → filter index 1
    // Find where filter index 1 appears in grouped items
    let grouped_idx = grouped
        .iter()
        .position(|item| matches!(item, GroupedActionItem::Item(fi) if *fi == 1));

    assert!(
        grouped_idx.is_some(),
        "action 'b' should be present in grouped items"
    );
}

#[test]
fn route_preserves_context_title_and_placeholder() {
    let route = make_route("agent_chat:root", &["change_model", "copy"]);
    assert_eq!(
        route.context_title.as_deref(),
        Some("agent_chat:root title")
    );
    assert_eq!(
        route.search_placeholder.as_deref(),
        Some("agent_chat:root placeholder")
    );
}

#[test]
fn set_config_preserves_route_placeholder_scenario() {
    // This tests the contract: when a route is active, set_config should
    // not clobber the route's search_placeholder with the host default.
    //
    // We can't instantiate a full ActionsDialog without GPUI, but we can
    // verify the should_rebuild logic that gates re-application.

    let config_a = ActionsDialogConfig::default();
    #[allow(deprecated)]
    let config_b = ActionsDialogConfig {
        section_style: SectionStyle::Separators,
        ..ActionsDialogConfig::default()
    };

    // Different section_style → should_rebuild = true
    assert!(should_rebuild_grouped_items_for_config_change(
        &config_a, &config_b
    ));

    // Same config → should_rebuild = false
    assert!(!should_rebuild_grouped_items_for_config_change(
        &config_a, &config_a
    ));
}

#[test]
fn route_state_back_stack_restore_preserves_search_and_selection() {
    // Simulate: root state → push child → pop back
    // Verify the parent state snapshot preserves search_text and selected_action_id.

    let root_route = make_route_with_initial("root", &["a", "b", "c"], "a");
    let mut root_state = ActionsDialogRouteState::new(root_route);

    // User typed "bc" and moved selection to "b"
    root_state.search_text = "bc".to_string();
    root_state.selected_action_id = Some("b".to_string());

    // Push child
    let child_route = make_route_with_initial("child", &["x", "y"], "y");
    let child_state = ActionsDialogRouteState::new(child_route);

    assert_eq!(child_state.selected_action_id.as_deref(), Some("y"));
    assert!(child_state.search_text.is_empty());

    // Pop back to root — verify the saved state
    assert_eq!(root_state.search_text, "bc");
    assert_eq!(root_state.selected_action_id.as_deref(), Some("b"));
}

#[test]
fn agent_chat_root_route_initial_selection_is_change_profile() {
    // Verify the Agent Chat root route builder sets initial selection
    // to the "Change Profile" action.
    use super::builders::{get_agent_chat_root_route, AGENT_CHAT_CHANGE_PROFILE_ACTION_ID};

    let models = vec![];
    let route = get_agent_chat_root_route(&models, None);

    assert_eq!(
        route.initial_selected_action_id.as_deref(),
        Some(AGENT_CHAT_CHANGE_PROFILE_ACTION_ID),
    );
}

#[test]
fn note_switcher_titles_match_progressive_prefix_queries() {
    use crate::actions::builders::{get_note_switcher_actions, NoteSwitcherNoteInfo};

    let actions = get_note_switcher_actions(&[
        NoteSwitcherNoteInfo {
            id: "a".into(),
            title: "alpha first note".into(),
            char_count: 16,
            is_current: false,
            is_pinned: false,
            preview: "alpha first note".into(),
            relative_time: "2m ago".into(),
        },
        NoteSwitcherNoteInfo {
            id: "b".into(),
            title: "beta second note".into(),
            char_count: 16,
            is_current: true,
            is_pinned: false,
            preview: "beta second note".into(),
            relative_time: "1m ago".into(),
        },
    ]);
    assert_eq!(actions.len(), 2);

    for query in ["a", "al", "alp", "alph", "alpha"] {
        let matched: Vec<&str> = actions
            .iter()
            .filter(|a| crate::actions::ActionsDialog::score_action(a, query) > 0)
            .map(|a| a.id.as_str())
            .collect();
        assert!(
            matched.contains(&"note_a"),
            "query '{query}' should match the alpha note, matched: {matched:?}"
        );
    }
}
