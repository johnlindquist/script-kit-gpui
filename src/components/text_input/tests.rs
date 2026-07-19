use super::*;

#[test]
fn test_new_input() {
    let input = TextInputState::new();
    assert!(input.is_empty());
    assert_eq!(input.cursor(), 0);
    assert!(input.selection().is_empty());
}

#[test]
fn test_with_text() {
    let input = TextInputState::with_text("hello");
    assert_eq!(input.text(), "hello");
    assert_eq!(input.cursor(), 5); // At end
}

#[test]
fn test_insert_char() {
    let mut input = TextInputState::new();
    input.insert_char('a');
    input.insert_char('b');
    assert_eq!(input.text(), "ab");
    assert_eq!(input.cursor(), 2);
}

#[test]
fn test_backspace() {
    let mut input = TextInputState::with_text("abc");
    input.backspace();
    assert_eq!(input.text(), "ab");
    assert_eq!(input.cursor(), 2);
}

#[test]
fn test_selection() {
    let mut input = TextInputState::with_text("hello");
    input.move_to_start(false);
    input.move_right(true); // Select 'h'
    input.move_right(true); // Select 'he'
    assert_eq!(input.selected_text(), "he");
    assert!(!input.selection().is_empty());
}

#[test]
fn test_select_all() {
    let mut input = TextInputState::with_text("hello");
    input.select_all();
    assert_eq!(input.selected_text(), "hello");
}

#[test]
fn test_delete_selection() {
    let mut input = TextInputState::with_text("hello");
    input.select_all();
    input.backspace();
    assert!(input.is_empty());
}

#[test]
fn test_insert_replaces_selection() {
    let mut input = TextInputState::with_text("hello");
    input.select_all();
    input.insert_char('x');
    assert_eq!(input.text(), "x");
}

#[test]
fn test_move_collapse_selection() {
    let mut input = TextInputState::with_text("hello");
    input.select_all();
    input.move_left(false); // Should collapse to start
    assert!(input.selection().is_empty());
    assert_eq!(input.cursor(), 0);
}

#[test]
fn test_word_boundary() {
    let mut input = TextInputState::with_text("hello world");
    input.move_to_end(false);
    input.move_word_left(false);
    assert_eq!(input.cursor(), 6); // At 'w'
    input.move_word_left(false);
    assert_eq!(input.cursor(), 0); // At start
}

#[test]
fn test_unicode() {
    let mut input = TextInputState::with_text("héllo");
    assert_eq!(input.text().chars().count(), 5);
    input.move_to_start(false);
    input.move_right(false);
    input.move_right(false);
    assert_eq!(input.cursor(), 2); // After 'hé'
}

#[test]
fn test_undo_redo_restores_text_and_selection_snapshot() {
    let mut input = TextInputState::with_text("hello");
    input.move_to_start(false);
    input.move_right(true);
    input.move_right(true); // Select "he"
    input.insert_str("xy");
    assert_eq!(input.text(), "xyllo");
    assert_eq!(input.cursor(), 2);
    assert!(input.selection().is_empty());

    assert!(input.undo());
    assert_eq!(input.text(), "hello");
    assert_eq!(
        input.selection(),
        TextSelection {
            anchor: 0,
            cursor: 2,
        }
    );

    assert!(input.redo());
    assert_eq!(input.text(), "xyllo");
    assert_eq!(input.cursor(), 2);
    assert!(input.selection().is_empty());
}

#[test]
fn test_undo_clears_redo_after_new_edit() {
    let mut input = TextInputState::new();
    input.insert_str("abc");
    input.insert_char('d');
    assert_eq!(input.text(), "abcd");

    assert!(input.undo());
    assert_eq!(input.text(), "abc");

    input.insert_char('z');
    assert_eq!(input.text(), "abcz");
    assert!(!input.redo());
}

#[test]
fn test_undo_stack_is_bounded_to_100_snapshots() {
    let mut input = TextInputState::new();
    for _ in 0..150 {
        input.insert_char('x');
    }
    assert_eq!(input.text().chars().count(), 150);

    let mut undo_count = 0;
    while input.undo() {
        undo_count += 1;
    }

    assert_eq!(undo_count, 100);
    assert_eq!(input.text().chars().count(), 50);
}

#[test]
fn test_cmd_backspace_deletes_selection_first() {
    let mut input = TextInputState::with_text("hello world");
    input.move_word_left(true); // select "world"
    assert_eq!(input.selected_text(), "world");

    input.handle_backspace_shortcut(true, false);
    assert_eq!(input.text(), "hello ");
    assert!(input.selection().is_empty());
    assert_eq!(input.cursor(), 6);
}

#[test]
fn test_cmd_backspace_with_middle_selection_deletes_only_selected_text() {
    let mut input = TextInputState::with_text("hello world");
    input.move_to_start(false);
    for _ in 0..5 {
        input.move_right(false);
    }
    for _ in 0..5 {
        input.move_right(true);
    }
    assert_eq!(input.selected_text(), " worl");

    input.handle_backspace_shortcut(true, false);
    assert_eq!(input.text(), "hellod");
    assert_eq!(input.cursor(), 5);
    assert!(input.selection().is_empty());
}

#[test]
fn test_alt_backspace_with_middle_selection_deletes_only_selected_text() {
    let mut input = TextInputState::with_text("hello world");
    input.move_to_start(false);
    for _ in 0..5 {
        input.move_right(false);
    }
    for _ in 0..5 {
        input.move_right(true);
    }
    assert_eq!(input.selected_text(), " worl");

    input.handle_backspace_shortcut(false, true);
    assert_eq!(input.text(), "hellod");
    assert_eq!(input.cursor(), 5);
    assert!(input.selection().is_empty());
}

#[test]
fn test_alt_backspace_deletes_selection_first() {
    let mut input = TextInputState::with_text("alpha beta gamma");
    input.move_word_left(true); // select "gamma"
    assert_eq!(input.selected_text(), "gamma");

    input.handle_backspace_shortcut(false, true);
    assert_eq!(input.text(), "alpha beta ");
    assert_eq!(input.cursor(), 11);
    assert!(input.selection().is_empty());
}

#[test]
fn test_alt_delete_deletes_selection_first() {
    let mut input = TextInputState::with_text("alpha beta gamma");
    input.move_to_start(false);
    input.move_word_right(true); // select "alpha "
    assert_eq!(input.selected_text(), "alpha ");

    input.handle_delete_shortcut(true);
    assert_eq!(input.text(), "beta gamma");
    assert_eq!(input.cursor(), 0);
    assert!(input.selection().is_empty());
}

#[test]
fn test_visible_window_range_keeps_cursor_visible_near_end() {
    let mut input = TextInputState::with_text("abcdefghijklmnopqrstuvwxyz");
    let (start, end) = input.visible_window_range(8);
    assert_eq!((start, end), (18, 26));

    input.move_to_start(false);
    let (start, end) = input.visible_window_range(8);
    assert_eq!((start, end), (0, 8));
}

#[test]
fn test_visible_window_range_centers_cursor_when_possible() {
    let mut input = TextInputState::with_text("abcdefghijklmnopqrstuvwxyz");
    input.move_to_start(false);
    for _ in 0..13 {
        input.move_right(false);
    }

    let (start, end) = input.visible_window_range(9);
    assert_eq!((start, end), (9, 18));
    assert!(input.cursor() >= start && input.cursor() <= end);
}

#[test]
fn test_set_cursor_clamps_to_text_length() {
    let mut input = TextInputState::with_text("hello");
    input.set_cursor(2);
    assert_eq!(input.cursor(), 2);
    assert!(input.selection().is_empty());

    input.set_cursor(99);
    assert_eq!(input.cursor(), 5);
    assert!(input.selection().is_empty());
}

#[test]
fn programmatic_set_and_insert_normalize_multiline_text_at_single_line_boundary() {
    let constructed = TextInputState::with_text("alpha\r\nbeta\ngamma\rdelta");
    assert_eq!(constructed.text(), "alphabetagammadelta");

    let mut input = TextInputState::new();
    input.set_text("one\r\ntwo\nthree\rfour");
    assert_eq!(input.text(), "onetwothreefour");
    assert!(!input.text().chars().any(|ch| matches!(ch, '\n' | '\r')));

    input.move_to_end(false);
    input.insert_str("\nfive\r\nsix\r");
    input.insert_char('\n');
    input.insert_char('\r');
    assert_eq!(input.text(), "onetwothreefourfivesix");
    assert_eq!(input.cursor(), input.text().chars().count());
}
