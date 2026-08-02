use super::*;

#[test]
fn test_should_auto_close_actions_window_returns_true_when_neither_window_is_focused() {
    assert!(should_auto_close_actions_window(false, false));
}

#[test]
fn test_should_auto_close_actions_window_returns_false_when_main_window_is_focused() {
    assert!(!should_auto_close_actions_window(true, false));
}

#[test]
fn test_should_auto_close_actions_window_returns_false_when_actions_window_is_active() {
    assert!(!should_auto_close_actions_window(false, true));
}

#[test]
fn test_clear_window_slot_does_clear_when_value_is_present() {
    let mut slot = Some(42usize);
    let had_value = clear_window_slot(&mut slot);
    assert!(had_value);
    assert_eq!(slot, None);
}

#[test]
fn test_clear_window_slot_is_idempotent_when_called_multiple_times() {
    let mut slot = Some(42usize);
    assert!(clear_window_slot(&mut slot));
    assert!(!clear_window_slot(&mut slot));
    assert_eq!(slot, None);
}

fn make_action_for_header_count(id: &str, section: Option<&str>) -> Action {
    let mut action = Action::new(
        id,
        id,
        None,
        crate::actions::types::ActionCategory::ScriptContext,
    );
    if let Some(section) = section {
        action = action.with_section(section);
    }
    action
}

#[test]
fn test_count_section_headers_does_not_reset_on_unsectioned_rows() {
    let actions = vec![
        make_action_for_header_count("a", Some("S1")),
        make_action_for_header_count("b", None),
        make_action_for_header_count("c", Some("S1")),
    ];

    assert_eq!(count_section_headers(&actions, &[0, 1, 2]), 1);
}

#[test]
fn test_count_section_headers_counts_new_section_after_unsectioned_row() {
    let actions = vec![
        make_action_for_header_count("a", Some("S1")),
        make_action_for_header_count("b", None),
        make_action_for_header_count("c", Some("S2")),
    ];

    assert_eq!(count_section_headers(&actions, &[0, 1, 2]), 2);
}

#[test]
fn test_actions_window_key_intent_maps_required_navigation_key_variants() {
    let no_mods = gpui::Modifiers::default();

    assert_eq!(
        actions_window_key_intent("up", None, &no_mods),
        Some(ActionsWindowKeyIntent::MoveUp)
    );
    assert_eq!(
        actions_window_key_intent("arrowup", None, &no_mods),
        Some(ActionsWindowKeyIntent::MoveUp)
    );

    assert_eq!(
        actions_window_key_intent("down", None, &no_mods),
        Some(ActionsWindowKeyIntent::MoveDown)
    );
    assert_eq!(
        actions_window_key_intent("arrowdown", None, &no_mods),
        Some(ActionsWindowKeyIntent::MoveDown)
    );
}

#[test]
fn test_actions_window_key_intent_maps_required_confirm_and_cancel_key_variants() {
    let no_mods = gpui::Modifiers::default();
    let mut cmd_only = gpui::Modifiers::default();
    cmd_only.platform = true;

    assert_eq!(
        actions_window_key_intent("enter", None, &no_mods),
        Some(ActionsWindowKeyIntent::ExecuteSelected)
    );
    assert_eq!(
        actions_window_key_intent("Enter", None, &no_mods),
        Some(ActionsWindowKeyIntent::ExecuteSelected)
    );

    assert_eq!(
        actions_window_key_intent("escape", None, &no_mods),
        Some(ActionsWindowKeyIntent::Close)
    );
    assert_eq!(
        actions_window_key_intent("Escape", None, &no_mods),
        Some(ActionsWindowKeyIntent::Close)
    );
    // ⌘K is the toggle chord: it must fully dismiss (never route-pop),
    // unlike Escape's pop-then-close ladder.
    assert_eq!(
        actions_window_key_intent("k", None, &cmd_only),
        Some(ActionsWindowKeyIntent::Dismiss)
    );
}

#[test]
fn test_actions_window_key_intent_search_input_upgrades() {
    let no_mods = gpui::Modifiers::default();
    let mut shift_only = gpui::Modifiers::default();
    shift_only.shift = true;
    let mut alt_only = gpui::Modifiers::default();
    alt_only.alt = true;
    let mut cmd_only = gpui::Modifiers::default();
    cmd_only.platform = true;

    // Option+Backspace deletes a word; Cmd+Backspace falls through so
    // destructive action shortcuts (e.g. Delete Note ⌘⌫) can match.
    assert_eq!(
        actions_window_key_intent("backspace", None, &alt_only),
        Some(ActionsWindowKeyIntent::BackspaceWord)
    );
    assert_eq!(
        actions_window_key_intent("backspace", None, &cmd_only),
        None
    );

    assert_eq!(
        actions_window_key_intent("left", None, &no_mods),
        Some(ActionsWindowKeyIntent::MoveCursorLeft { select: false })
    );
    assert_eq!(
        actions_window_key_intent("right", None, &shift_only),
        Some(ActionsWindowKeyIntent::MoveCursorRight { select: true })
    );

    // Full printable charset arrives via key_char (shifted symbols,
    // punctuation), matching the main search input.
    assert_eq!(
        actions_window_key_intent("1", Some("!"), &shift_only),
        Some(ActionsWindowKeyIntent::TypeChar('!'))
    );
    assert_eq!(
        actions_window_key_intent(".", Some("."), &no_mods),
        Some(ActionsWindowKeyIntent::TypeChar('.'))
    );
}
