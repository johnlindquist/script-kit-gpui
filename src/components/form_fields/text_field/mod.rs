//! Single-line text field form component with validation and selection.

use gpui::*;

use crate::protocol::Field;

use super::helpers::{byte_idx_from_char_idx, char_len, drain_char_range, slice_by_char_range};
use super::{form_field_type_allows_candidate_value, FormFieldColors, FormFieldState};

mod render;

pub struct FormTextField {
    /// Field definition from protocol
    field: Field,
    /// Pre-computed colors
    colors: FormFieldColors,
    /// Current text value
    pub value: String,
    /// Cursor position in the text (CHAR INDEX, not bytes)
    pub cursor_position: usize,
    /// Selection anchor (CHAR INDEX). None = no selection.
    pub selection_anchor: Option<usize>,
    input_revision: u64,
    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,
    /// Whether to mask the text (for password fields)
    is_password: bool,
    /// Shared state for external access
    pub state: FormFieldState,
}

impl FormTextField {
    /// Create a new text field from a Field definition
    pub fn new(field: Field, colors: FormFieldColors, cx: &mut App) -> Self {
        let initial_value = field.value.clone().unwrap_or_default();
        let is_password = field
            .field_type
            .as_deref()
            .is_some_and(|field_type| field_type.eq_ignore_ascii_case("password"));
        let state = FormFieldState::new(initial_value.clone());

        Self {
            field,
            colors,
            value: initial_value.clone(),
            cursor_position: char_len(&initial_value),
            selection_anchor: None,
            input_revision: 0,
            focus_handle: cx.focus_handle(),
            is_password,
            state,
        }
    }

    /// Get the current value
    pub fn value(&self) -> &str {
        &self.value
    }

    pub(crate) fn revision(&self) -> u64 {
        self.input_revision
    }

    fn advance_input_revision(&mut self) {
        assert!(
            self.input_revision < u64::MAX,
            "form input revision exhausted"
        );
        self.input_revision += 1;
    }

    fn selection_state(&self) -> (usize, usize) {
        (
            self.selection_anchor.unwrap_or(self.cursor_position),
            self.cursor_position,
        )
    }

    fn record_selection_change(&mut self, previous: (usize, usize)) {
        if self.selection_state() != previous {
            self.advance_input_revision();
        }
    }

    pub(crate) fn update_colors(&mut self, colors: FormFieldColors, cx: &mut Context<Self>) {
        self.colors = colors;
        cx.notify();
    }

    /// Get the field name
    pub fn name(&self) -> &str {
        &self.field.name
    }

    /// Set the value programmatically
    pub fn set_value(&mut self, value: String) {
        let cursor_position = char_len(&value);
        if self.value != value || self.cursor_position != cursor_position || self.has_selection() {
            self.advance_input_revision();
        }
        self.value = value.clone();
        self.cursor_position = cursor_position;
        self.selection_anchor = None;
        self.state.set_value(value);
    }

    /// Get the focus handle for this text field
    ///
    /// This allows parent components to delegate focus to this field.
    /// Used by FormPromptState to implement the Focusable trait by returning
    /// the child's focus handle instead of its own, preventing focus stealing.
    pub fn get_focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    /// Get the display text (masked for password fields)
    fn display_text(&self) -> String {
        if self.is_password {
            // Use char count to prevent panics with multibyte chars
            "•".repeat(char_len(&self.value))
        } else {
            self.value.clone()
        }
    }

    fn text_len_chars(&self) -> usize {
        char_len(&self.value)
    }

    fn has_selection(&self) -> bool {
        self.selection_anchor.is_some() && self.selection_anchor != Some(self.cursor_position)
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|a| {
            if a <= self.cursor_position {
                (a, self.cursor_position)
            } else {
                (self.cursor_position, a)
            }
        })
    }

    fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    fn select_all(&mut self) {
        let previous = self.selection_state();
        self.selection_anchor = Some(0);
        self.cursor_position = self.text_len_chars();
        self.record_selection_change(previous);
    }

    fn get_selected_text(&self) -> String {
        if let Some((start, end)) = self.selection_range() {
            if start != end {
                return slice_by_char_range(&self.value, start, end).to_string();
            }
        }
        String::new()
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            if start != end {
                self.advance_input_revision();
                drain_char_range(&mut self.value, start, end);
                self.cursor_position = start;
                self.selection_anchor = None;
                self.state.set_value(self.value.clone());
                return true;
            }
        }
        false
    }

    fn insert_text_at_cursor(&mut self, text: &str) {
        self.delete_selection();
        if !text.is_empty() {
            self.advance_input_revision();
        }
        let insert_byte = byte_idx_from_char_idx(&self.value, self.cursor_position);
        self.value.insert_str(insert_byte, text);
        self.cursor_position = (self.cursor_position + char_len(text)).min(self.text_len_chars());
        self.state.set_value(self.value.clone());
    }

    fn candidate_value_with_inserted_text(&self, text: &str) -> String {
        let mut next = self.value.clone();
        let mut insert_position = self.cursor_position;

        if let Some((start, end)) = self.selection_range() {
            if start != end {
                drain_char_range(&mut next, start, end);
                insert_position = start;
            }
        }

        let insert_byte = byte_idx_from_char_idx(&next, insert_position);
        next.insert_str(insert_byte, text);
        next
    }

    fn allows_text_insertion_for_field_type(&self, text: &str) -> bool {
        let candidate = self.candidate_value_with_inserted_text(text);
        form_field_type_allows_candidate_value(self.field.field_type.as_deref(), &candidate)
    }

    fn move_left(&mut self, extend_selection: bool) {
        let previous = self.selection_state();
        if !extend_selection && self.has_selection() {
            if let Some((start, _)) = self.selection_range() {
                self.cursor_position = start;
            }
            self.clear_selection();
            self.record_selection_change(previous);
            return;
        }
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
        if !extend_selection {
            self.clear_selection();
        }
        self.record_selection_change(previous);
    }

    fn move_right(&mut self, extend_selection: bool) {
        let previous = self.selection_state();
        if !extend_selection && self.has_selection() {
            if let Some((_, end)) = self.selection_range() {
                self.cursor_position = end;
            }
            self.clear_selection();
            self.record_selection_change(previous);
            return;
        }
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        let len = self.text_len_chars();
        if self.cursor_position < len {
            self.cursor_position += 1;
        }
        if !extend_selection {
            self.clear_selection();
        }
        self.record_selection_change(previous);
    }

    fn move_home(&mut self, extend_selection: bool) {
        let previous = self.selection_state();
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.cursor_position = 0;
        if !extend_selection {
            self.clear_selection();
        }
        self.record_selection_change(previous);
    }

    fn move_end(&mut self, extend_selection: bool) {
        let previous = self.selection_state();
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.cursor_position = self.text_len_chars();
        if !extend_selection {
            self.clear_selection();
        }
        self.record_selection_change(previous);
    }

    fn backspace_char(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_position == 0 {
            return;
        }
        let del_start = self.cursor_position - 1;
        self.advance_input_revision();
        drain_char_range(&mut self.value, del_start, self.cursor_position);
        self.cursor_position = del_start;
        self.state.set_value(self.value.clone());
    }

    fn delete_forward_char(&mut self) {
        if self.delete_selection() {
            return;
        }
        let len = self.text_len_chars();
        if self.cursor_position >= len {
            return;
        }
        self.advance_input_revision();
        drain_char_range(
            &mut self.value,
            self.cursor_position,
            self.cursor_position + 1,
        );
        self.state.set_value(self.value.clone());
    }

    fn copy(&self, cx: &mut Context<Self>) {
        let text = self.get_selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        let text = self.get_selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.delete_selection();
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                if self.allows_text_insertion_for_field_type(&text) {
                    self.insert_text_at_cursor(&text);
                }
            }
        }
    }

    /// Unified key handler with selection and clipboard support
    pub fn handle_key_event(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;

        // Select all
        if cmd && !shift && key.eq_ignore_ascii_case("a") {
            self.select_all();
            cx.notify();
            return;
        }
        // Clipboard
        if cmd && !shift && key.eq_ignore_ascii_case("c") {
            self.copy(cx);
            return;
        }
        if cmd && !shift && key.eq_ignore_ascii_case("x") {
            self.cut(cx);
            cx.notify();
            return;
        }
        if cmd && !shift && key.eq_ignore_ascii_case("v") {
            self.paste(cx);
            cx.notify();
            return;
        }
        // Navigation with optional selection
        if !cmd && (key.eq_ignore_ascii_case("left") || key.eq_ignore_ascii_case("arrowleft")) {
            self.move_left(shift);
            cx.notify();
            return;
        }
        if !cmd && (key.eq_ignore_ascii_case("right") || key.eq_ignore_ascii_case("arrowright")) {
            self.move_right(shift);
            cx.notify();
            return;
        }
        if !cmd && key.eq_ignore_ascii_case("home") {
            self.move_home(shift);
            cx.notify();
            return;
        }
        if !cmd && key.eq_ignore_ascii_case("end") {
            self.move_end(shift);
            cx.notify();
            return;
        }
        // Editing
        if !cmd && key.eq_ignore_ascii_case("backspace") {
            self.backspace_char();
            cx.notify();
            return;
        }
        if !cmd && key.eq_ignore_ascii_case("delete") {
            self.delete_forward_char();
            cx.notify();
            return;
        }

        // Printable character input (ignore when cmd/ctrl held)
        if !cmd {
            if let Some(ref key_char) = event.keystroke.key_char {
                let s = key_char.to_string();
                if !s.is_empty()
                    && !s.chars().all(|c| c.is_control())
                    && self.allows_text_insertion_for_field_type(&s)
                {
                    self.insert_text_at_cursor(&s);
                    cx.notify();
                }
            }
        }
    }
}

#[cfg(test)]
#[gpui::test]
fn text_field_revision_tracks_text_selection_and_no_ops(cx: &mut gpui::TestAppContext) {
    let field: Field =
        serde_json::from_value(serde_json::json!({"name": "field", "value": "hé"})).unwrap();
    let colors = FormFieldColors::from_theme(&crate::theme::Theme::default());
    let input = cx.new(|cx| FormTextField::new(field, colors, cx));
    input.update(cx, |input, cx| {
        let initial = input.revision();
        input.set_value("hé".into());
        input.move_right(true);
        input.move_right(false);
        input.delete_forward_char();
        input.insert_text_at_cursor("");
        input.update_colors(colors, cx);
        assert_eq!(input.revision(), initial);
        input.set_value("àb".into());
        let changed = input.revision();
        assert!(changed > initial);
        input.set_value("hé".into());
        assert!(input.revision() > changed);
        let before_cursor = input.revision();
        input.move_left(false);
        input.move_right(false);
        assert!(input.revision() > before_cursor);
        let before_selection = input.revision();
        input.select_all();
        assert!(input.revision() > before_selection);
        let selected = input.revision();
        input.select_all();
        assert_eq!(input.revision(), selected);
        input.insert_text_at_cursor("hé");
        assert_eq!(input.value(), "hé");
        assert!(!input.has_selection());
        assert!(input.revision() > selected);
        let before_delete = input.revision();
        input.backspace_char();
        assert_eq!(input.value(), "h");
        assert!(input.revision() > before_delete);
    });
}
