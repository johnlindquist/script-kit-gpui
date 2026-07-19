use gpui::{ClipboardItem, Context, Render};
use std::collections::VecDeque;

/// Selection in a single-line text input
/// anchor = where selection started, cursor = current position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextSelection {
    /// Where selection started (fixed point)
    pub anchor: usize,
    /// Current cursor position (moves with arrows)
    pub cursor: usize,
}

const TEXT_INPUT_UNDO_STACK_LIMIT: usize = 100;

/// Normalize arbitrary text before it reaches a GPUI single-line layout.
pub(crate) fn normalize_single_line_text(text: impl Into<String>) -> String {
    text.into()
        .chars()
        .filter(|character| !matches!(character, '\n' | '\r'))
        .collect()
}

#[derive(Debug, Clone)]
struct TextSnapshot {
    text: String,
    selection: TextSelection,
}

impl TextSnapshot {
    fn capture(state: &TextInputState) -> Self {
        Self {
            text: state.text.clone(),
            selection: state.selection,
        }
    }
}

impl TextSelection {
    pub fn caret(pos: usize) -> Self {
        Self {
            anchor: pos,
            cursor: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    /// Get selection as ordered range (start, end)
    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// Get the length of the selection
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        let (start, end) = self.range();
        end - start
    }
}

/// State for a single-line text input with selection support
#[derive(Debug, Clone)]
pub struct TextInputState {
    /// The text content
    text: String,
    /// Selection state (anchor and cursor positions)
    selection: TextSelection,
    /// Previous edit snapshots (bounded; oldest entries are dropped first)
    undo_stack: VecDeque<TextSnapshot>,
    /// Snapshots that can be restored after an undo
    redo_stack: VecDeque<TextSnapshot>,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInputState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            selection: TextSelection::caret(0),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = normalize_single_line_text(text);
        let len = text.chars().count();
        Self {
            text,
            selection: TextSelection::caret(len), // Cursor at end
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    // === Getters ===

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.selection.cursor
    }

    pub fn selection(&self) -> TextSelection {
        self.selection
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Get selected text, or empty string if no selection
    pub fn selected_text(&self) -> &str {
        if self.selection.is_empty() {
            return "";
        }
        let (start, end) = self.selection.range();
        let start_byte = self.char_to_byte(start);
        let end_byte = self.char_to_byte(end);
        &self.text[start_byte..end_byte]
    }

    /// Compute a visible text window `[start, end)` where the cursor remains visible.
    pub fn visible_window_range(&self, max_visible_chars: usize) -> (usize, usize) {
        let text_len = self.text.chars().count();
        if max_visible_chars == 0 || text_len <= max_visible_chars {
            return (0, text_len);
        }

        let window_width = max_visible_chars.max(1);
        let cursor = self.selection.cursor.min(text_len);

        // Prefer to keep some context on both sides, then clamp to valid bounds.
        let mut start = cursor.saturating_sub(window_width / 2);
        let mut end = (start + window_width).min(text_len);
        if end == text_len {
            start = end.saturating_sub(window_width);
        }

        // Enforce cursor visibility after clamping.
        if cursor < start {
            start = cursor;
            end = (start + window_width).min(text_len);
        } else if cursor >= end {
            end = (cursor + 1).min(text_len);
            start = end.saturating_sub(window_width);
        }

        (start, end)
    }

    // === Setters ===

    pub fn set_text(&mut self, text: impl Into<String>) {
        let new_text = normalize_single_line_text(text);
        let len = new_text.chars().count();
        let new_selection = TextSelection::caret(len.min(self.selection.cursor));
        if self.text == new_text && self.selection == new_selection {
            return;
        }
        self.record_edit_snapshot();
        self.text = new_text;
        self.selection = new_selection;
    }

    /// Place the caret at a specific character index, clamped to the text length.
    pub fn set_cursor(&mut self, cursor: usize) {
        let clamped = cursor.min(self.text.chars().count());
        self.selection = TextSelection::caret(clamped);
    }

    pub fn clear(&mut self) {
        if self.text.is_empty() && self.selection == TextSelection::caret(0) {
            return;
        }
        self.record_edit_snapshot();
        self.text.clear();
        self.selection = TextSelection::caret(0);
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop_back() else {
            return false;
        };

        let current_snapshot = TextSnapshot::capture(self);
        Self::push_snapshot(&mut self.redo_stack, current_snapshot);
        self.restore_snapshot(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo_stack.pop_back() else {
            return false;
        };

        let current_snapshot = TextSnapshot::capture(self);
        Self::push_snapshot(&mut self.undo_stack, current_snapshot);
        self.restore_snapshot(snapshot);
        true
    }

    // === Text Manipulation ===

    /// Insert a character at cursor, replacing selection if any
    pub fn insert_char(&mut self, ch: char) {
        if matches!(ch, '\n' | '\r') {
            return;
        }
        self.record_edit_snapshot();
        self.delete_selection();
        let byte_pos = self.char_to_byte(self.selection.cursor);
        self.text.insert(byte_pos, ch);
        self.selection = TextSelection::caret(self.selection.cursor + 1);
    }

    /// Insert a string at cursor, replacing selection if any
    pub fn insert_str(&mut self, s: &str) {
        let single_line = normalize_single_line_text(s);
        if single_line.is_empty() && self.selection.is_empty() {
            return;
        }
        self.record_edit_snapshot();
        self.delete_selection();
        let byte_pos = self.char_to_byte(self.selection.cursor);
        self.text.insert_str(byte_pos, &single_line);
        let inserted_chars = single_line.chars().count();
        self.selection = TextSelection::caret(self.selection.cursor + inserted_chars);
    }

    /// Delete selection, or character before cursor if no selection
    pub fn backspace(&mut self) {
        if !self.selection.is_empty() {
            self.record_edit_snapshot();
            self.delete_selection();
        } else if self.selection.cursor > 0 {
            self.record_edit_snapshot();
            let new_pos = self.selection.cursor - 1;
            let byte_start = self.char_to_byte(new_pos);
            let byte_end = self.char_to_byte(self.selection.cursor);
            self.text.replace_range(byte_start..byte_end, "");
            self.selection = TextSelection::caret(new_pos);
        }
    }

    /// Delete selection, or character after cursor if no selection
    pub fn delete(&mut self) {
        if !self.selection.is_empty() {
            self.record_edit_snapshot();
            self.delete_selection();
        } else {
            let len = self.text.chars().count();
            if self.selection.cursor < len {
                self.record_edit_snapshot();
                let byte_start = self.char_to_byte(self.selection.cursor);
                let byte_end = self.char_to_byte(self.selection.cursor + 1);
                self.text.replace_range(byte_start..byte_end, "");
            }
        }
    }

    pub(crate) fn handle_backspace_shortcut(&mut self, cmd: bool, alt: bool) {
        if !self.selection.is_empty() {
            self.backspace();
            return;
        }

        if cmd {
            // Cmd+Backspace: delete to start of line
            let end = self.selection.cursor;
            if end > 0 {
                self.record_edit_snapshot();
                self.selection = TextSelection {
                    anchor: 0,
                    cursor: end,
                };
                self.delete_selection();
            }
            return;
        }

        if alt {
            // Alt+Backspace: delete word left
            let start = self.find_word_boundary_left();
            let end = self.selection.cursor;
            if start < end {
                self.record_edit_snapshot();
                self.selection = TextSelection {
                    anchor: start,
                    cursor: end,
                };
                self.delete_selection();
            }
            return;
        }

        self.backspace();
    }

    pub(crate) fn handle_delete_shortcut(&mut self, alt: bool) {
        if !self.selection.is_empty() {
            self.delete();
            return;
        }

        if alt {
            // Alt+Delete: delete word right
            let start = self.selection.cursor;
            let end = self.find_word_boundary_right();
            if start < end {
                self.record_edit_snapshot();
                self.selection = TextSelection {
                    anchor: start,
                    cursor: end,
                };
                self.delete_selection();
            }
            return;
        }

        self.delete();
    }

    /// Delete the selected text (internal)
    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let (start, end) = self.selection.range();
        let byte_start = self.char_to_byte(start);
        let byte_end = self.char_to_byte(end);
        self.text.replace_range(byte_start..byte_end, "");
        self.selection = TextSelection::caret(start);
    }

    // === Cursor Movement ===

    /// Move cursor left, optionally extending selection
    pub fn move_left(&mut self, extend_selection: bool) {
        if !extend_selection && !self.selection.is_empty() {
            // Collapse to start of selection
            let (start, _) = self.selection.range();
            self.selection = TextSelection::caret(start);
        } else if self.selection.cursor > 0 {
            let new_pos = self.selection.cursor - 1;
            if extend_selection {
                self.selection.cursor = new_pos;
            } else {
                self.selection = TextSelection::caret(new_pos);
            }
        }
    }

    /// Move cursor right, optionally extending selection
    pub fn move_right(&mut self, extend_selection: bool) {
        let len = self.text.chars().count();
        if !extend_selection && !self.selection.is_empty() {
            // Collapse to end of selection
            let (_, end) = self.selection.range();
            self.selection = TextSelection::caret(end);
        } else if self.selection.cursor < len {
            let new_pos = self.selection.cursor + 1;
            if extend_selection {
                self.selection.cursor = new_pos;
            } else {
                self.selection = TextSelection::caret(new_pos);
            }
        }
    }

    /// Move cursor to start of line, optionally extending selection
    pub fn move_to_start(&mut self, extend_selection: bool) {
        if extend_selection {
            self.selection.cursor = 0;
        } else {
            self.selection = TextSelection::caret(0);
        }
    }

    /// Move cursor to end of line, optionally extending selection
    pub fn move_to_end(&mut self, extend_selection: bool) {
        let len = self.text.chars().count();
        if extend_selection {
            self.selection.cursor = len;
        } else {
            self.selection = TextSelection::caret(len);
        }
    }

    /// Move cursor to previous word boundary
    pub fn move_word_left(&mut self, extend_selection: bool) {
        let new_pos = self.find_word_boundary_left();
        if extend_selection {
            self.selection.cursor = new_pos;
        } else {
            self.selection = TextSelection::caret(new_pos);
        }
    }

    /// Move cursor to next word boundary
    pub fn move_word_right(&mut self, extend_selection: bool) {
        let new_pos = self.find_word_boundary_right();
        if extend_selection {
            self.selection.cursor = new_pos;
        } else {
            self.selection = TextSelection::caret(new_pos);
        }
    }

    /// Select all text
    pub fn select_all(&mut self) {
        let len = self.text.chars().count();
        self.selection = TextSelection {
            anchor: 0,
            cursor: len,
        };
    }

    // === Clipboard Operations ===

    /// Copy selected text to clipboard
    pub fn copy<T: Render>(&self, cx: &mut Context<T>) {
        if !self.selection.is_empty() {
            let text = self.selected_text().to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Cut selected text to clipboard
    pub fn cut<T: Render>(&mut self, cx: &mut Context<T>) {
        if !self.selection.is_empty() {
            let text = self.selected_text().to_string();
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.record_edit_snapshot();
            self.delete_selection();
        }
    }

    /// Paste from clipboard
    pub fn paste<T: Render>(&mut self, cx: &mut Context<T>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                self.insert_str(&text);
            }
        }
    }

    // === Key Handling ===

    /// Handle a key event. Returns true if the event was handled.
    pub fn handle_key<T: Render>(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        cmd: bool,
        alt: bool,
        shift: bool,
        cx: &mut Context<T>,
    ) -> bool {
        // Clipboard
        if cmd && !alt && key.eq_ignore_ascii_case("z") {
            if shift {
                self.redo();
            } else {
                self.undo();
            }
            return true;
        }
        if cmd && !alt && !shift && key.eq_ignore_ascii_case("y") {
            self.redo();
            return true;
        }
        if cmd && !alt && key.eq_ignore_ascii_case("c") {
            self.copy(cx);
            return true;
        }
        if cmd && !alt && key.eq_ignore_ascii_case("x") {
            self.cut(cx);
            return true;
        }
        if cmd && !alt && key.eq_ignore_ascii_case("v") {
            self.paste(cx);
            return true;
        }
        if cmd && !alt && key.eq_ignore_ascii_case("a") {
            self.select_all();
            return true;
        }

        // Navigation
        if key.eq_ignore_ascii_case("left") || key.eq_ignore_ascii_case("arrowleft") {
            if cmd {
                self.move_to_start(shift);
            } else if alt {
                self.move_word_left(shift);
            } else {
                self.move_left(shift);
            }
            return true;
        }
        if key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright") {
            if cmd {
                self.move_to_end(shift);
            } else if alt {
                self.move_word_right(shift);
            } else {
                self.move_right(shift);
            }
            return true;
        }
        if key.eq_ignore_ascii_case("home") {
            self.move_to_start(shift);
            return true;
        }
        if key.eq_ignore_ascii_case("end") {
            self.move_to_end(shift);
            return true;
        }

        // Deletion
        if key.eq_ignore_ascii_case("backspace") {
            if !self.selection.is_empty() {
                self.record_edit_snapshot();
                self.delete_selection();
                return true;
            }
            self.handle_backspace_shortcut(cmd, alt);
            return true;
        }
        if key.eq_ignore_ascii_case("delete") {
            if !self.selection.is_empty() {
                self.record_edit_snapshot();
                self.delete_selection();
                return true;
            }
            self.handle_delete_shortcut(alt);
            return true;
        }

        // Character input (no cmd modifier)
        if !cmd {
            if let Some(key_char) = key_char {
                if let Some(ch) = key_char.chars().next() {
                    if !ch.is_control() {
                        self.insert_char(ch);
                        return true;
                    }
                }
            }
        }

        false
    }

    // === Helper Methods ===

    /// Convert character index to byte index
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Find the previous word boundary from cursor
    fn find_word_boundary_left(&self) -> usize {
        if self.selection.cursor == 0 {
            return 0;
        }

        let chars: Vec<char> = self.text.chars().collect();
        let mut pos = self.selection.cursor - 1;

        // Skip whitespace
        while pos > 0 && chars[pos].is_whitespace() {
            pos -= 1;
        }

        // Skip word characters
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }

        pos
    }

    /// Find the next word boundary from cursor
    fn find_word_boundary_right(&self) -> usize {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut pos = self.selection.cursor;

        // Skip current word
        while pos < len && !chars[pos].is_whitespace() {
            pos += 1;
        }

        // Skip whitespace
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }

        pos
    }

    fn restore_snapshot(&mut self, snapshot: TextSnapshot) {
        self.text = snapshot.text;
        self.selection = snapshot.selection;
    }

    fn record_edit_snapshot(&mut self) {
        let snapshot = TextSnapshot::capture(self);
        Self::push_snapshot(&mut self.undo_stack, snapshot);
        self.redo_stack.clear();
    }

    fn push_snapshot(stack: &mut VecDeque<TextSnapshot>, snapshot: TextSnapshot) {
        if stack.len() >= TEXT_INPUT_UNDO_STACK_LIMIT {
            stack.pop_front();
        }
        stack.push_back(snapshot);
    }
}
