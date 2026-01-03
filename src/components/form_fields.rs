//! Native Form Field Components for GPUI Script Kit
//!
//! This module provides reusable form field components for rendering HTML form fields
//! as native GPUI elements. Components include:
//!
//! - [`FormTextField`] - Text input for text/password/email/number types
//! - [`FormTextArea`] - Multi-line text input
//! - [`FormCheckbox`] - Checkbox with label
//!
//!
//! # Design Patterns
//!
//! All components follow these patterns:
//! - **Colors struct**: Pre-computed colors (Copy/Clone) for efficient closure use
//! - **FocusHandle**: Each component manages its own focus for Tab navigation
//! - **Value state**: Components maintain their own value state
//! - **IntoElement trait**: Compatible with GPUI's element system

#![allow(dead_code)]

use gpui::*;
use std::sync::{Arc, Mutex};

use crate::protocol::Field;

// --- Text indexing helpers (char-indexed cursor/selection) --------------------

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Convert a character index (0..=char_len) into a byte index (0..=s.len()).
/// If char_idx is past the end, returns s.len().
fn byte_idx_from_char_idx(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or_else(|| s.len())
}

/// Remove a char range [start_char, end_char) from a String (char indices).
fn drain_char_range(s: &mut String, start_char: usize, end_char: usize) {
    let start_b = byte_idx_from_char_idx(s, start_char);
    let end_b = byte_idx_from_char_idx(s, end_char);
    if start_b < end_b && start_b <= s.len() && end_b <= s.len() {
        s.drain(start_b..end_b);
    }
}

/// Slice a &str by char indices [start_char, end_char).
fn slice_by_char_range(s: &str, start_char: usize, end_char: usize) -> &str {
    let start_b = byte_idx_from_char_idx(s, start_char);
    let end_b = byte_idx_from_char_idx(s, end_char);
    &s[start_b..end_b]
}

// Tunables for click-to-position. These are *approximations*.
const TEXTFIELD_CHAR_WIDTH_PX: f32 = 8.0;
const TEXTAREA_LINE_HEIGHT_PX: f32 = 24.0;
const INPUT_PADDING_X_PX: f32 = 12.0;
const TEXTAREA_PADDING_Y_PX: f32 = 8.0;

/// Pre-computed colors for form field rendering
///
/// This struct holds the color values needed for form field rendering,
/// allowing efficient use in closures without cloning the full theme.
#[derive(Clone, Copy, Debug)]
pub struct FormFieldColors {
    /// Background color of the input
    pub background: u32,
    /// Background color when focused
    pub background_focused: u32,
    /// Text color when typing
    pub text: u32,
    /// Placeholder text color
    pub placeholder: u32,
    /// Label text color
    pub label: u32,
    /// Border color
    pub border: u32,
    /// Border color when focused
    pub border_focused: u32,
    /// Cursor color
    pub cursor: u32,
    /// Checkbox checked background
    pub checkbox_checked: u32,
    /// Checkbox check mark color
    pub checkbox_mark: u32,
}

impl FormFieldColors {
    /// Create FormFieldColors from a Theme
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            background: theme.colors.background.search_box,
            background_focused: theme.colors.background.main,
            text: theme.colors.text.primary,
            placeholder: theme.colors.text.muted,
            label: theme.colors.text.secondary,
            border: theme.colors.ui.border,
            border_focused: theme.colors.accent.selected,
            cursor: 0x00ffff, // Cyan cursor
            checkbox_checked: theme.colors.accent.selected,
            checkbox_mark: theme.colors.background.main,
        }
    }

    /// Create FormFieldColors from design colors
    pub fn from_design(colors: &crate::designs::DesignColors) -> Self {
        Self {
            background: colors.background_secondary,
            background_focused: colors.background,
            text: colors.text_primary,
            placeholder: colors.text_muted,
            label: colors.text_secondary,
            border: colors.border,
            border_focused: colors.accent,
            cursor: 0x00ffff,
            checkbox_checked: colors.accent,
            checkbox_mark: colors.background,
        }
    }
}

impl Default for FormFieldColors {
    fn default() -> Self {
        Self {
            background: 0x2d2d30,
            background_focused: 0x1e1e1e,
            text: 0xffffff,
            placeholder: 0x808080,
            label: 0xcccccc,
            border: 0x464647,
            border_focused: 0xfbbf24, // Script Kit yellow/gold
            cursor: 0x00ffff,
            checkbox_checked: 0xfbbf24,
            checkbox_mark: 0x1e1e1e,
        }
    }
}

/// Shared state for form field values
///
/// This allows parent components to access field values for form submission
#[derive(Clone)]
pub struct FormFieldState {
    value: Arc<Mutex<String>>,
}

impl FormFieldState {
    /// Create a new form field state with an initial value
    pub fn new(initial_value: String) -> Self {
        Self {
            value: Arc::new(Mutex::new(initial_value)),
        }
    }

    /// Get the current value
    pub fn get_value(&self) -> String {
        self.value.lock().unwrap().clone()
    }

    /// Set the value
    pub fn set_value(&self, value: String) {
        *self.value.lock().unwrap() = value;
    }
}

/// A text input field component for single-line text entry
///
/// Supports:
/// - text, password, email, and number input types
/// - Placeholder text
/// - Label display
/// - Focus management for Tab navigation
/// - Password masking
/// - Selection (Shift+Arrow, Cmd/Ctrl+A)
/// - Clipboard (Cmd/Ctrl+C/X/V)
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
        let is_password = field.field_type.as_deref() == Some("password");
        let state = FormFieldState::new(initial_value.clone());

        Self {
            field,
            colors,
            value: initial_value.clone(),
            cursor_position: char_len(&initial_value),
            selection_anchor: None,
            focus_handle: cx.focus_handle(),
            is_password,
            state,
        }
    }

    /// Get the current value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Get the field name
    pub fn name(&self) -> &str {
        &self.field.name
    }

    /// Set the value programmatically
    pub fn set_value(&mut self, value: String) {
        self.value = value.clone();
        self.cursor_position = char_len(&self.value);
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

    /// Handle text input
    fn handle_input(&mut self, text: &str, cx: &mut Context<Self>) {
        // Insert text at cursor position
        self.value.insert_str(self.cursor_position, text);
        self.cursor_position += text.len();
        self.state.set_value(self.value.clone());
        cx.notify();
    }

    /// Handle key down events
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();

        match key {
            "backspace" => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.value.remove(self.cursor_position);
                    self.state.set_value(self.value.clone());
                    cx.notify();
                }
            }
            "delete" => {
                if self.cursor_position < self.value.len() {
                    self.value.remove(self.cursor_position);
                    self.state.set_value(self.value.clone());
                    cx.notify();
                }
            }
            "left" | "arrowleft" => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    cx.notify();
                }
            }
            "right" | "arrowright" => {
                if self.cursor_position < self.value.len() {
                    self.cursor_position += 1;
                    cx.notify();
                }
            }
            "home" => {
                self.cursor_position = 0;
                cx.notify();
            }
            "end" => {
                self.cursor_position = self.value.len();
                cx.notify();
            }
            _ => {}
        }
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
        self.selection_anchor = Some(0);
        self.cursor_position = self.text_len_chars();
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
        let insert_byte = byte_idx_from_char_idx(&self.value, self.cursor_position);
        self.value.insert_str(insert_byte, text);
        self.cursor_position = (self.cursor_position + char_len(text)).min(self.text_len_chars());
        self.state.set_value(self.value.clone());
    }

    fn move_left(&mut self, extend_selection: bool) {
        if !extend_selection && self.has_selection() {
            if let Some((start, _)) = self.selection_range() {
                self.cursor_position = start;
            }
            self.clear_selection();
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
    }

    fn move_right(&mut self, extend_selection: bool) {
        if !extend_selection && self.has_selection() {
            if let Some((_, end)) = self.selection_range() {
                self.cursor_position = end;
            }
            self.clear_selection();
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
    }

    fn move_home(&mut self, extend_selection: bool) {
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.cursor_position = 0;
        if !extend_selection {
            self.clear_selection();
        }
    }

    fn move_end(&mut self, extend_selection: bool) {
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.cursor_position = self.text_len_chars();
        if !extend_selection {
            self.clear_selection();
        }
    }

    fn backspace_char(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor_position == 0 {
            return;
        }
        let del_start = self.cursor_position - 1;
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
                self.insert_text_at_cursor(&text);
            }
        }
    }

    /// Unified key handler with selection and clipboard support
    pub fn handle_key_event(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_lowercase();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;

        match (key.as_str(), cmd, shift) {
            // Select all
            ("a", true, false) => {
                self.select_all();
                cx.notify();
                return;
            }
            // Clipboard
            ("c", true, false) => {
                self.copy(cx);
                return;
            }
            ("x", true, false) => {
                self.cut(cx);
                cx.notify();
                return;
            }
            ("v", true, false) => {
                self.paste(cx);
                cx.notify();
                return;
            }
            // Navigation with optional selection
            ("left" | "arrowleft", false, s) => {
                self.move_left(s);
                cx.notify();
                return;
            }
            ("right" | "arrowright", false, s) => {
                self.move_right(s);
                cx.notify();
                return;
            }
            ("home", false, s) => {
                self.move_home(s);
                cx.notify();
                return;
            }
            ("end", false, s) => {
                self.move_end(s);
                cx.notify();
                return;
            }
            // Editing
            ("backspace", false, _) => {
                self.backspace_char();
                cx.notify();
                return;
            }
            ("delete", false, _) => {
                self.delete_forward_char();
                cx.notify();
                return;
            }
            _ => {}
        }

        // Printable character input (ignore when cmd/ctrl held)
        if !cmd {
            if let Some(ref key_char) = event.keystroke.key_char {
                let s = key_char.to_string();
                if !s.is_empty() && !s.chars().all(|c| c.is_control()) {
                    self.insert_text_at_cursor(&s);
                    cx.notify();
                }
            }
        }
    }
}

impl Focusable for FormTextField {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormTextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let is_focused = self.focus_handle.is_focused(window);
        let display_text = self.display_text();
        let placeholder = self.field.placeholder.clone().unwrap_or_default();
        let label = self.field.label.clone();
        let cursor_pos = self.cursor_position;
        let has_value = !self.value.is_empty();

        // Log focus state for debugging
        crate::logging::log(
            "FIELD",
            &format!(
                "TextField[{}] render: is_focused={}, value='{}'",
                self.field.name, is_focused, self.value
            ),
        );

        // Calculate border and background based on focus
        let border_color = if is_focused {
            rgb(colors.border_focused)
        } else {
            rgb(colors.border)
        };
        let bg_color = if is_focused {
            rgba((colors.background_focused << 8) | 0xff)
        } else {
            rgba((colors.background << 8) | 0x80)
        };

        let field_name = self.field.name.clone();
        let field_name_for_log = field_name.clone();

        // Keyboard handler for text input
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &KeyDownEvent,
                  _window: &mut Window,
                  cx: &mut Context<Self>| {
                let key = event.keystroke.key.as_str();
                crate::logging::log(
                    "FIELD",
                    &format!(
                        "TextField[{}] key: '{}' (key_char: {:?})",
                        field_name_for_log, key, event.keystroke.key_char
                    ),
                );

                // First handle special keys (backspace, delete, arrows, etc.)
                this.handle_key_down(event, cx);

                // Then handle printable character input
                if let Some(ref key_char) = event.keystroke.key_char {
                    if let Some(ch) = key_char.chars().next() {
                        if !ch.is_control() {
                            crate::logging::log(
                                "FIELD",
                                &format!(
                                    "TextField[{}] inserting char: '{}'",
                                    field_name_for_log, ch
                                ),
                            );
                            this.handle_input(&ch.to_string(), cx);
                        }
                    }
                }
            },
        );

        // Build cursor element (2px width is fixed for crisp rendering)
        let cursor_element = div().w(px(2.)).h(rems(1.125)).bg(rgb(colors.cursor));

        // Build text content based on value and focus state
        let text_content: Div = if has_value {
            let mut content = div()
                .flex()
                .flex_row()
                .items_center()
                // Text before cursor
                .child(
                    div()
                        .text_lg()
                        .text_color(rgb(colors.text))
                        .child(display_text[..cursor_pos.min(display_text.len())].to_string()),
                );

            // Cursor (only when focused)
            if is_focused {
                content = content.child(cursor_element);
            }

            // Text after cursor
            content.child(
                div()
                    .text_lg()
                    .text_color(rgb(colors.text))
                    .child(display_text[cursor_pos.min(display_text.len())..].to_string()),
            )
        } else {
            let mut content = div().flex().flex_row().items_center();

            if is_focused {
                // Cursor when focused and empty
                content = content.child(cursor_element);
            } else {
                // Placeholder when not focused
                content = content.child(
                    div()
                        .text_lg()
                        .text_color(rgb(colors.placeholder))
                        .child(placeholder),
                );
            }
            content
        };

        // Build the main container - horizontal layout with label beside input
        let mut container = div()
            .id(ElementId::Name(format!("form-field-{}", field_name).into()))
            .flex()
            .flex_row()
            .items_center()
            .gap(rems(0.75))
            .w_full();

        // Add label if present - fixed width for alignment
        if let Some(label_text) = label {
            container = container.child(
                div()
                    .w(rems(7.5))
                    .text_sm()
                    .text_color(rgb(colors.label))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label_text),
            );
        }

        // Add input container - fills remaining space
        // Handle click to focus this field
        let focus_handle_for_click = self.focus_handle.clone();
        let handle_click = cx.listener(
            move |_this: &mut Self,
                  _event: &ClickEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                crate::logging::log("FIELD", "TextField clicked - focusing");
                focus_handle_for_click.focus(window, cx);
            },
        );

        container.child(
            div()
                .id(ElementId::Name(format!("input-{}", field_name).into()))
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key)
                .on_click(handle_click)
                .flex()
                .flex_row()
                .items_center()
                .flex_1()
                .h(rems(2.25))
                .px(rems(0.75))
                .bg(bg_color)
                .border_1()
                .border_color(border_color)
                .rounded(px(6.))
                .cursor_text()
                // Text content or placeholder
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_1()
                        .overflow_hidden()
                        .child(text_content),
                ),
        )
    }
}

/// A multi-line text area component
///
/// Supports:
/// - Multi-line text input
/// - Placeholder text
/// - Label display
/// - Focus management
/// - Selection support (Shift+Arrow, mouse drag)
/// - Clipboard operations (Cmd/Ctrl+C/X/V)
pub struct FormTextArea {
    /// Field definition from protocol
    field: Field,
    /// Pre-computed colors
    colors: FormFieldColors,
    /// Current text value (lines)
    pub value: String,
    /// Cursor position in the text (char index)
    pub cursor_position: usize,
    /// Selection anchor (char index), None if no selection
    pub selection_anchor: Option<usize>,
    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,
    /// Number of visible rows
    rows: usize,
    /// Shared state for external access
    pub state: FormFieldState,
}

impl FormTextArea {
    /// Create a new text area from a Field definition
    pub fn new(field: Field, colors: FormFieldColors, rows: usize, cx: &mut App) -> Self {
        let initial_value = field.value.clone().unwrap_or_default();
        let cursor_pos = char_len(&initial_value);
        let state = FormFieldState::new(initial_value.clone());

        Self {
            field,
            colors,
            value: initial_value,
            cursor_position: cursor_pos,
            selection_anchor: None,
            focus_handle: cx.focus_handle(),
            rows,
            state,
        }
    }

    /// Get the current value
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Get the field name
    pub fn name(&self) -> &str {
        &self.field.name
    }

    /// Set the value programmatically
    pub fn set_value(&mut self, value: String) {
        self.cursor_position = char_len(&value);
        self.selection_anchor = None;
        self.value = value.clone();
        self.state.set_value(value);
    }

    /// Get the focus handle for this text area
    ///
    /// This allows parent components to delegate focus to this field.
    /// Used by FormPromptState to implement the Focusable trait by returning
    /// the child's focus handle instead of its own, preventing focus stealing.
    pub fn get_focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    // ───── Selection helpers ─────

    /// Get selection range as (start, end) in char indices, ordered
    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor_position);
            let end = anchor.max(self.cursor_position);
            (start, end)
        })
    }

    /// Check if there is an active selection
    fn has_selection(&self) -> bool {
        self.selection_anchor
            .is_some_and(|a| a != self.cursor_position)
    }

    /// Get selected text
    fn selected_text(&self) -> Option<String> {
        self.selection_range()
            .map(|(start, end)| slice_by_char_range(&self.value, start, end).to_string())
    }

    /// Delete selected text, collapse cursor to start
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection_range() {
            drain_char_range(&mut self.value, start, end);
            self.cursor_position = start;
            self.selection_anchor = None;
            self.state.set_value(self.value.clone());
        }
    }

    /// Clear selection without deleting
    fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Select all text
    fn select_all(&mut self) {
        let len = char_len(&self.value);
        if len > 0 {
            self.selection_anchor = Some(0);
            self.cursor_position = len;
        }
    }

    // ───── Clipboard ─────

    fn copy(&self, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        self.delete_selection();
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                self.insert_text_at_cursor(&text);
            }
        }
    }

    // ───── Cursor movement ─────

    fn move_left(&mut self, extend_selection: bool) {
        if !extend_selection {
            // If selection exists, collapse to start
            if let Some((start, _)) = self.selection_range() {
                self.cursor_position = start;
                self.clear_selection();
                return;
            }
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
        if !extend_selection {
            self.clear_selection();
        }
    }

    fn move_right(&mut self, extend_selection: bool) {
        let len = char_len(&self.value);
        if !extend_selection {
            if let Some((_, end)) = self.selection_range() {
                self.cursor_position = end;
                self.clear_selection();
                return;
            }
        } else if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        if self.cursor_position < len {
            self.cursor_position += 1;
        }
        if !extend_selection {
            self.clear_selection();
        }
    }

    fn move_home(&mut self, extend_selection: bool) {
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.cursor_position = 0;
        if !extend_selection {
            self.clear_selection();
        }
    }

    fn move_end(&mut self, extend_selection: bool) {
        if extend_selection && self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
        self.cursor_position = char_len(&self.value);
        if !extend_selection {
            self.clear_selection();
        }
    }

    // ───── Editing ─────

    fn insert_text_at_cursor(&mut self, text: &str) {
        if self.has_selection() {
            self.delete_selection();
        }
        let byte_idx = byte_idx_from_char_idx(&self.value, self.cursor_position);
        self.value.insert_str(byte_idx, text);
        self.cursor_position += char_len(text);
        self.state.set_value(self.value.clone());
    }

    fn backspace_char(&mut self) {
        if self.has_selection() {
            self.delete_selection();
        } else if self.cursor_position > 0 {
            drain_char_range(
                &mut self.value,
                self.cursor_position - 1,
                self.cursor_position,
            );
            self.cursor_position -= 1;
            self.state.set_value(self.value.clone());
        }
    }

    fn delete_forward_char(&mut self) {
        if self.has_selection() {
            self.delete_selection();
        } else if self.cursor_position < char_len(&self.value) {
            drain_char_range(
                &mut self.value,
                self.cursor_position,
                self.cursor_position + 1,
            );
            self.state.set_value(self.value.clone());
        }
    }

    /// Handle text input (legacy, kept for render callback)
    fn handle_input(&mut self, text: &str, _cx: &mut Context<Self>) {
        self.insert_text_at_cursor(text);
    }

    /// Handle key down events (legacy, kept for render callback)
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str().to_lowercase();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;

        match (key.as_str(), cmd, shift) {
            // Select all
            ("a", true, false) => {
                self.select_all();
                cx.notify();
            }
            // Clipboard
            ("c", true, false) => {
                self.copy(cx);
            }
            ("x", true, false) => {
                self.cut(cx);
                cx.notify();
            }
            ("v", true, false) => {
                self.paste(cx);
                cx.notify();
            }
            // Navigation with optional selection
            ("left" | "arrowleft", false, s) => {
                self.move_left(s);
                cx.notify();
            }
            ("right" | "arrowright", false, s) => {
                self.move_right(s);
                cx.notify();
            }
            ("home", false, s) => {
                self.move_home(s);
                cx.notify();
            }
            ("end", false, s) => {
                self.move_end(s);
                cx.notify();
            }
            // Editing
            ("backspace", false, _) => {
                self.backspace_char();
                cx.notify();
            }
            ("delete", false, _) => {
                self.delete_forward_char();
                cx.notify();
            }
            // Enter inserts newline
            ("enter", false, _) => {
                self.insert_text_at_cursor("\n");
                cx.notify();
            }
            _ => {}
        }
    }

    /// Unified key event handler called by form_prompt.rs
    ///
    /// Handles: Selection (Shift+Arrow), Clipboard (Cmd+C/X/V/A),
    /// Navigation (Arrow, Home, End), Editing (Backspace, Delete, Enter),
    /// and printable character input.
    pub fn handle_key_event(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str().to_lowercase();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;

        match (key.as_str(), cmd, shift) {
            // Select all
            ("a", true, false) => {
                self.select_all();
                cx.notify();
                return;
            }
            // Clipboard
            ("c", true, false) => {
                self.copy(cx);
                return;
            }
            ("x", true, false) => {
                self.cut(cx);
                cx.notify();
                return;
            }
            ("v", true, false) => {
                self.paste(cx);
                cx.notify();
                return;
            }
            // Navigation with optional selection
            ("left" | "arrowleft", false, s) => {
                self.move_left(s);
                cx.notify();
                return;
            }
            ("right" | "arrowright", false, s) => {
                self.move_right(s);
                cx.notify();
                return;
            }
            ("home", false, s) => {
                self.move_home(s);
                cx.notify();
                return;
            }
            ("end", false, s) => {
                self.move_end(s);
                cx.notify();
                return;
            }
            // Editing
            ("backspace", false, _) => {
                self.backspace_char();
                cx.notify();
                return;
            }
            ("delete", false, _) => {
                self.delete_forward_char();
                cx.notify();
                return;
            }
            // Enter inserts newline
            ("enter", false, _) => {
                self.insert_text_at_cursor("\n");
                cx.notify();
                return;
            }
            _ => {}
        }

        // Printable character input (ignore when cmd/ctrl held)
        if !cmd {
            if let Some(ref key_char) = event.keystroke.key_char {
                let s = key_char.to_string();
                if !s.is_empty() && !s.chars().all(|c| c.is_control()) {
                    self.insert_text_at_cursor(&s);
                    cx.notify();
                }
            }
        }
    }
}

impl Focusable for FormTextArea {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormTextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let is_focused = self.focus_handle.is_focused(window);
        let display_text = self.value.clone();
        let placeholder = self.field.placeholder.clone().unwrap_or_default();
        let label = self.field.label.clone();
        let rows = self.rows;
        let has_value = !self.value.is_empty();

        // Calculate border and background based on focus
        let border_color = if is_focused {
            rgb(colors.border_focused)
        } else {
            rgb(colors.border)
        };
        let bg_color = if is_focused {
            rgba((colors.background_focused << 8) | 0xff)
        } else {
            rgba((colors.background << 8) | 0x80)
        };

        // Calculate height based on rows (1.5rem per row + 1rem padding)
        let height_rems = (rows as f32) * 1.5 + 1.0;

        let field_name = self.field.name.clone();
        let field_name_for_log = field_name.clone();

        // Handle click to focus this field
        let focus_handle_for_click = self.focus_handle.clone();
        let handle_click = cx.listener(
            move |_this: &mut Self,
                  _event: &ClickEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                crate::logging::log(
                    "FIELD",
                    &format!("TextArea[{}] clicked - focusing", field_name_for_log),
                );
                focus_handle_for_click.focus(window, cx);
            },
        );

        // Keyboard handler for text input
        let handle_key = cx.listener(
            |this: &mut Self,
             event: &KeyDownEvent,
             _window: &mut Window,
             cx: &mut Context<Self>| {
                // First handle special keys (backspace, delete, arrows, etc.)
                this.handle_key_down(event, cx);

                // Then handle printable character input
                if let Some(ref key_char) = event.keystroke.key_char {
                    if let Some(ch) = key_char.chars().next() {
                        if !ch.is_control() {
                            this.handle_input(&ch.to_string(), cx);
                        }
                    }
                }
            },
        );

        // Build text content
        let text_content: Div = if has_value {
            div()
                .flex()
                .flex_col()
                .text_sm()
                .text_color(rgb(colors.text))
                .child(display_text)
        } else {
            div()
                .text_sm()
                .text_color(rgb(colors.placeholder))
                .child(placeholder)
        };

        // Build the main container - horizontal layout with label beside textarea
        let mut container = div()
            .id(ElementId::Name(
                format!("form-textarea-{}", field_name).into(),
            ))
            .flex()
            .flex_row()
            .items_start() // Align label to top of textarea
            .gap(rems(0.75))
            .w_full();

        // Add label if present - fixed width for alignment
        if let Some(label_text) = label {
            container = container.child(
                div()
                    .w(rems(7.5))
                    .pt(rems(0.5)) // Align with textarea padding
                    .text_sm()
                    .text_color(rgb(colors.label))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label_text),
            );
        }

        // Add input container - fills remaining space
        container.child(
            div()
                .id(ElementId::Name(format!("textarea-{}", field_name).into()))
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key)
                .on_click(handle_click)
                .flex()
                .flex_col()
                .flex_1()
                .h(rems(height_rems))
                .px(rems(0.75))
                .py(rems(0.5))
                .bg(bg_color)
                .border_1()
                .border_color(border_color)
                .rounded(px(6.))
                .cursor_text()
                .overflow_hidden()
                // Text content or placeholder
                .child(text_content),
        )
    }
}

/// A checkbox component with label
///
/// Supports:
/// - Checked/unchecked state
/// - Label display
/// - Focus management
/// - Click to toggle
pub struct FormCheckbox {
    /// Field definition from protocol
    field: Field,
    /// Pre-computed colors
    colors: FormFieldColors,
    /// Whether the checkbox is checked
    checked: bool,
    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,
    /// Shared state for external access (stores "true" or "false")
    pub state: FormFieldState,
}

impl FormCheckbox {
    /// Create a new checkbox from a Field definition
    pub fn new(field: Field, colors: FormFieldColors, cx: &mut App) -> Self {
        // Parse initial checked state from value
        let checked = field.value.as_deref() == Some("true");
        let state = FormFieldState::new(if checked {
            "true".to_string()
        } else {
            "false".to_string()
        });

        Self {
            field,
            colors,
            checked,
            focus_handle: cx.focus_handle(),
            state,
        }
    }

    /// Get whether the checkbox is checked
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Get the field name
    pub fn name(&self) -> &str {
        &self.field.name
    }

    /// Toggle the checkbox state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.checked = !self.checked;
        self.state.set_value(if self.checked {
            "true".to_string()
        } else {
            "false".to_string()
        });
        cx.notify();
    }

    /// Set the checked state
    pub fn set_checked(&mut self, checked: bool, cx: &mut Context<Self>) {
        self.checked = checked;
        self.state.set_value(if checked {
            "true".to_string()
        } else {
            "false".to_string()
        });
        cx.notify();
    }
}

impl Focusable for FormCheckbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormCheckbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let is_focused = self.focus_handle.is_focused(window);
        let checked = self.checked;
        let label = self
            .field
            .label
            .clone()
            .unwrap_or_else(|| self.field.name.clone());

        // Calculate border based on focus
        let border_color = if is_focused {
            rgb(colors.border_focused)
        } else {
            rgb(colors.border)
        };

        // Checkbox box styling
        let box_bg = if checked {
            rgb(colors.checkbox_checked)
        } else {
            rgba((colors.background << 8) | 0x80)
        };

        let field_name = self.field.name.clone();

        // Keyboard handler for Space key to toggle
        let handle_key = cx.listener(
            |this: &mut Self,
             event: &KeyDownEvent,
             _window: &mut Window,
             cx: &mut Context<Self>| {
                let key = event.keystroke.key.as_str();
                if key == "space" || key == " " {
                    this.toggle(cx);
                }
            },
        );

        // Build checkbox box with optional checkmark
        let mut checkbox_box = div()
            .flex()
            .items_center()
            .justify_center()
            .w(rems(1.125))
            .h(rems(1.125))
            .bg(box_bg)
            .border_1()
            .border_color(border_color)
            .rounded(px(4.));

        // Add checkmark when checked
        if checked {
            checkbox_box = checkbox_box.child(
                div()
                    .text_sm()
                    .text_color(rgb(colors.checkbox_mark))
                    .font_weight(FontWeight::BOLD)
                    .child("✓"),
            );
        }

        // Main container - horizontal layout consistent with other form fields
        div()
            .id(ElementId::Name(
                format!("form-checkbox-{}", field_name).into(),
            ))
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .flex()
            .flex_row()
            .items_center()
            .gap(rems(0.75))
            .w_full()
            .cursor_pointer()
            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                this.toggle(cx);
            }))
            // Empty label area for alignment with other fields
            .child(div().w(rems(7.5)))
            // Checkbox and label group
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(rems(0.5))
                    // Checkbox box
                    .child(checkbox_box)
                    // Label
                    .child(div().text_sm().text_color(rgb(colors.text)).child(label)),
            )
    }
}

// Note: Tests omitted for this module due to GPUI macro recursion limit issues.
// The form field components are integration-tested via the main application's
// form prompt rendering.
//
// Verified traits:
// - FormFieldColors: Copy, Clone, Debug, Default
// - FormFieldState: Clone with get_value()/set_value() for shared state
// - FormTextField: Render + Focusable with value state management
// - FormTextArea: Render + Focusable with multi-line value state
// - FormCheckbox: Render + Focusable with checked/unchecked toggle
