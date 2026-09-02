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
    let constructed = TextInputState::with_text("\nalpha\r\nbeta\ngamma\rdelta\r");
    assert_eq!(constructed.text(), "alpha beta gamma delta");

    let mut input = TextInputState::new();
    input.set_text("one\r\ntwo\nthree\rfour");
    assert_eq!(input.text(), "one two three four");
    assert!(!input.text().chars().any(|ch| matches!(ch, '\n' | '\r')));

    input.move_to_end(false);
    input.insert_str(" five\r\nsix\r");
    input.insert_char('\n');
    input.insert_char('\r');
    assert_eq!(input.text(), "one two three four five six");
    assert_eq!(input.cursor(), input.text().chars().count());
}

#[test]
fn revision_tracks_same_length_edits_and_text_aba() {
    let mut input = TextInputState::with_text("cat");
    let original = input.revision();
    input.set_text("dog");
    let changed = input.revision();
    assert!(changed > original);
    assert_eq!(input.cursor(), 3);

    input.set_text("cat");
    assert_eq!(input.text(), "cat");
    assert_eq!(input.cursor(), 3);
    assert!(input.revision() > changed);
}

#[test]
fn revision_tracks_selection_replacement_and_undo_redo() {
    let mut input = TextInputState::with_text("hé");
    input.select_all();
    let selected = input.revision();
    input.insert_str("àb");
    let edited = input.revision();
    assert!(edited > selected);
    assert_eq!(input.text(), "àb");
    assert_eq!(input.selection(), TextSelection::caret(2));

    assert!(input.undo());
    let undone = input.revision();
    assert!(undone > edited);
    assert_eq!(input.text(), "hé");
    assert_eq!(
        input.selection(),
        TextSelection {
            anchor: 0,
            cursor: 2
        }
    );

    assert!(input.redo());
    assert!(input.revision() > undone);
    assert_eq!(input.text(), "àb");
    assert_eq!(input.selection(), TextSelection::caret(2));

    assert!(input.undo());
    input.insert_char('x');
    let branched = input.revision();
    assert_eq!(input.text(), "x");
    assert!(!input.redo());
    assert_eq!(input.revision(), branched);
}

#[test]
fn revision_tracks_cursor_and_selection_navigation_including_aba() {
    let mut input = TextInputState::with_text("alpha beta");
    let steps: &[(fn(&mut TextInputState), usize, usize)] = &[
        (|input| input.move_left(false), 9, 9),
        (|input| input.move_right(false), 10, 10),
        (|input| input.move_to_start(false), 0, 0),
        (|input| input.move_right(true), 0, 1),
        (|input| input.move_right(true), 0, 2),
        (|input| input.move_left(true), 0, 1),
        (|input| input.move_to_end(true), 0, 10),
        (|input| input.move_word_left(true), 0, 6),
        (|input| input.move_word_right(true), 0, 10),
        (|input| input.move_to_start(true), 0, 0),
        (|input| input.move_word_right(false), 6, 6),
        (|input| input.move_word_left(false), 0, 0),
        (|input| input.move_to_end(false), 10, 10),
        (TextInputState::select_all, 0, 10),
        (|input| input.move_left(false), 0, 0),
        (TextInputState::select_all, 0, 10),
        (|input| input.move_right(false), 10, 10),
        (TextInputState::select_all, 0, 10),
        (|input| input.set_cursor(10), 10, 10),
        (|input| input.set_cursor(3), 3, 3),
        (|input| input.set_cursor(usize::MAX), 10, 10),
    ];
    for (step, anchor, cursor) in steps {
        let before = input.revision();
        step(&mut input);
        assert!(input.revision() > before);
        assert_eq!(
            input.selection(),
            TextSelection {
                anchor: *anchor,
                cursor: *cursor
            }
        );
        assert_eq!(input.text(), "alpha beta");
    }
    // Navigation does not add edit history.
    assert!(!input.undo());
}

#[test]
fn revision_tracks_insertion_deletion_clear_and_shortcuts() {
    let edits: &[(usize, fn(&mut TextInputState), &str)] = &[
        (10, |input| input.insert_char('!'), "alpha beta!"),
        (10, |input| input.insert_str("!"), "alpha beta!"),
        (10, TextInputState::backspace, "alpha bet"),
        (0, TextInputState::delete, "lpha beta"),
        (10, TextInputState::clear, ""),
        (10, |input| input.handle_backspace_shortcut(true, false), ""),
        (
            10,
            |input| input.handle_backspace_shortcut(false, true),
            "alpha ",
        ),
        (0, |input| input.handle_delete_shortcut(true), "beta"),
    ];
    for (cursor, edit, expected) in edits {
        let mut input = TextInputState::with_text("alpha beta");
        input.set_cursor(*cursor);
        let before = input.revision();
        edit(&mut input);
        assert!(input.revision() > before);
        assert_eq!(input.text(), *expected);
    }

    let selected_edits: &[fn(&mut TextInputState)] = &[
        TextInputState::backspace,
        TextInputState::delete,
        |input| input.insert_str(""),
        |input| input.handle_backspace_shortcut(true, false),
        |input| input.handle_backspace_shortcut(false, true),
        |input| input.handle_delete_shortcut(true),
    ];
    for edit in selected_edits {
        let mut input = TextInputState::with_text("hé");
        input.select_all();
        let before = input.revision();
        edit(&mut input);
        assert!(input.revision() > before);
        assert!(input.is_empty());
        assert_eq!(input.selection(), TextSelection::caret(0));
    }
}

#[test]
fn revision_ignores_no_ops_and_read_only_access() {
    let mut input = TextInputState::new();
    let initial = input.revision();
    input.clear();
    input.set_text("");
    input.set_cursor(usize::MAX);
    input.select_all();
    input.backspace();
    input.delete();
    input.insert_str("");
    input.insert_str("\r\n");
    input.insert_char('\n');
    input.insert_char('\r');
    for extend in [false, true] {
        input.move_left(extend);
        input.move_right(extend);
        input.move_to_start(extend);
        input.move_to_end(extend);
        input.move_word_left(extend);
        input.move_word_right(extend);
    }
    assert!(!input.undo());
    assert!(!input.redo());
    assert_eq!(input.revision(), initial);

    let mut input = TextInputState::with_text("alpha beta");
    let initial = input.revision();
    input.set_text("\nalpha beta\r\n");
    input.set_cursor(usize::MAX);
    input.delete();
    input.handle_delete_shortcut(true);
    for extend in [false, true] {
        input.move_right(extend);
        input.move_to_end(extend);
        input.move_word_right(extend);
    }
    assert!(!input.undo());
    assert_eq!(input.revision(), initial);

    input.move_to_start(false);
    let at_start = input.revision();
    input.backspace();
    input.handle_backspace_shortcut(true, false);
    input.handle_backspace_shortcut(false, true);
    for extend in [false, true] {
        input.move_left(extend);
        input.move_to_start(extend);
        input.move_word_left(extend);
    }
    assert_eq!(input.revision(), at_start);

    input.select_all();
    let selected = input.revision();
    input.select_all();
    assert_eq!(input.text(), "alpha beta");
    assert_eq!(input.cursor(), 10);
    assert_eq!(
        input.selection(),
        TextSelection {
            anchor: 0,
            cursor: 10
        }
    );
    assert_eq!(input.selected_text(), "alpha beta");
    assert!(!input.is_empty());
    assert_eq!(input.visible_window_range(4), (6, 10));
    assert_eq!(input.revision(), selected);
    assert_eq!(input.revision(), selected);
}

#[test]
fn revision_tracks_selection_only_setter_but_not_identical_history_restore() {
    let mut input = TextInputState::with_text("abc");
    input.select_all();
    let selected = input.revision();
    input.set_text("abc");
    assert!(input.revision() > selected);
    assert_eq!(input.selection(), TextSelection::caret(3));

    // Revisit the snapshot through navigation before undoing a selection-only edit.
    input.select_all();
    let revisited = input.revision();
    assert!(input.undo());
    assert_eq!(input.revision(), revisited);
    assert!(input.redo());
    assert_eq!(input.revision(), revisited);
    assert_eq!(input.text(), "abc");
    assert_eq!(
        input.selection(),
        TextSelection {
            anchor: 0,
            cursor: 3
        }
    );
}

#[test]
fn revision_initialization_and_clone_preserve_state_and_history() {
    assert_eq!(
        TextInputState::new().revision(),
        TextInputState::default().revision()
    );
    assert_eq!(
        TextInputState::new().revision(),
        TextInputState::with_text("abc").revision()
    );

    let mut original = TextInputState::with_text("abc");
    original.insert_char('d');
    let frozen = original.revision();
    let mut cloned = original.clone();
    assert_eq!(cloned.revision(), frozen);
    assert_eq!(cloned.text(), original.text());
    assert_eq!(cloned.selection(), original.selection());

    assert!(cloned.undo());
    assert!(cloned.revision() > frozen);
    assert_eq!(cloned.text(), "abc");
    assert_eq!(original.text(), "abcd");
    assert_eq!(original.revision(), frozen);
    assert!(original.undo());
    assert!(original.revision() > frozen);
    assert_eq!(original.text(), "abc");
}
