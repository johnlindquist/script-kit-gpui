//! GPUI Editor Prompt Component
//!
//! A full-featured code editor for Script Kit with:
//! - Text editing (insert, delete, backspace)
//! - Cursor navigation (arrows, home/end, word movement)
//! - Selection (shift+arrows, cmd+a, mouse)
//! - Clipboard (cmd+c/v/x)
//! - Undo/redo (cmd+z, cmd+shift+z)
//! - Syntax highlighting
//! - Line numbers
//! - Monospace font

#![allow(dead_code)]

use gpui::{
    div, prelude::*, px, rgb, rgba, uniform_list, ClipboardItem, Context, FocusHandle, Focusable,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point, Render, SharedString,
    UniformListScrollHandle, Window,
};
use std::time::{Duration, Instant};
use ropey::Rope;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

use crate::config::Config;
use crate::logging;
use crate::snippet::ParsedSnippet;
use crate::syntax::{highlight_code_lines, HighlightedLine};
use crate::theme::Theme;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// Character width in pixels (monospace) - base value for 14pt font
const BASE_CHAR_WIDTH: f32 = 8.4;
/// Base font size for calculating ratios
const BASE_FONT_SIZE: f32 = 14.0;
/// Line height multiplier (relative to font size)
const LINE_HEIGHT_MULTIPLIER: f32 = 1.43; // 20/14 ≈ 1.43
/// Gutter width for line numbers
const GUTTER_WIDTH: f32 = 50.0;
/// Maximum undo history size
const MAX_UNDO_HISTORY: usize = 100;

/// Cursor position in the editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    /// Line index (0-based)
    pub line: usize,
    /// Column index (0-based, in characters)
    pub column: usize,
}

impl CursorPosition {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    pub fn start() -> Self {
        Self { line: 0, column: 0 }
    }
}

/// Selection range in the editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Anchor position (where selection started)
    pub anchor: CursorPosition,
    /// Head position (where cursor is, end of selection)
    pub head: CursorPosition,
}

impl Selection {
    pub fn new(anchor: CursorPosition, head: CursorPosition) -> Self {
        Self { anchor, head }
    }

    pub fn caret(pos: CursorPosition) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Get the selection as an ordered range (start, end)
    pub fn ordered(&self) -> (CursorPosition, CursorPosition) {
        if self.anchor.line < self.head.line
            || (self.anchor.line == self.head.line && self.anchor.column <= self.head.column)
        {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

/// Undo/redo state snapshot
#[derive(Debug, Clone)]
struct EditorSnapshot {
    content: String,
    cursor: CursorPosition,
    selection: Selection,
}

/// State for snippet/template mode in the editor
#[derive(Debug, Clone)]
pub struct SnippetState {
    /// The parsed snippet with all tabstop info
    pub snippet: ParsedSnippet,
    /// Current tabstop index in the navigation order (0..tabstops.len())
    pub current_tabstop_idx: usize,
    /// Whether we've visited all tabstops (ready to exit snippet mode)
    pub completed: bool,
}

/// EditorPrompt - Full-featured code editor
pub struct EditorPrompt {
    // Identity
    pub id: String,

    // Content - using ropey for efficient text manipulation
    rope: Rope,
    language: String,

    // Cursor and selection
    cursor: CursorPosition,
    selection: Selection,
    cursor_visible: bool,

    // Display cache
    highlighted_lines: Vec<HighlightedLine>,
    needs_rehighlight: bool,
    scroll_handle: UniformListScrollHandle,

    // Undo/redo
    undo_stack: VecDeque<EditorSnapshot>,
    redo_stack: VecDeque<EditorSnapshot>,

    // GPUI
    focus_handle: FocusHandle,
    on_submit: SubmitCallback,
    theme: Arc<Theme>,
    config: Arc<Config>,

    // Layout - explicit height for proper sizing (GPUI entities don't inherit parent flex sizing)
    content_height: Option<gpui::Pixels>,

    // Change detection for render logging (avoid spam)
    last_render_state: Option<RenderState>,

    // Snippet/template mode state
    snippet_state: Option<SnippetState>,

    // When true, ignore all key events (used when actions panel is open)
    pub suppress_keys: bool,

    // Mouse selection state
    is_selecting: bool,
    last_click_time: Option<Instant>,
    last_click_position: Option<CursorPosition>,
    click_count: u8,
}

/// Tracked state for change detection in render logging
#[derive(Debug, Clone, PartialEq)]
struct RenderState {
    line_count: usize,
    total_height: Option<gpui::Pixels>,
    editor_height: Option<gpui::Pixels>,
}

impl EditorPrompt {
    /// Create a new EditorPrompt
    #[allow(dead_code)]
    pub fn new(
        id: String,
        content: String,
        language: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
        config: Arc<Config>,
    ) -> Self {
        Self::with_height(
            id,
            content,
            language,
            focus_handle,
            on_submit,
            theme,
            config,
            None,
        )
    }

    /// Create a new EditorPrompt with explicit height
    ///
    /// This is necessary because GPUI entities don't inherit parent flex sizing.
    /// When rendered as a child of a sized container, h_full() doesn't resolve
    /// to the parent's height. We must pass an explicit height.
    #[allow(clippy::too_many_arguments)]
    pub fn with_height(
        id: String,
        content: String,
        language: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
        config: Arc<Config>,
        content_height: Option<gpui::Pixels>,
    ) -> Self {
        logging::log(
            "EDITOR",
            &format!(
                "EditorPrompt::new id={}, lang={}, content_len={}, height={:?}",
                id,
                language,
                content.len(),
                content_height
            ),
        );

        let rope = Rope::from_str(&content);
        let highlighted_lines = highlight_code_lines(&content, &language);

        logging::log(
            "EDITOR",
            &format!(
                "Highlighted {} lines for language '{}'",
                highlighted_lines.len(),
                language
            ),
        );

        Self {
            id,
            rope,
            language,
            cursor: CursorPosition::start(),
            selection: Selection::caret(CursorPosition::start()),
            cursor_visible: true,
            highlighted_lines,
            needs_rehighlight: false,
            scroll_handle: UniformListScrollHandle::new(),
            undo_stack: VecDeque::with_capacity(MAX_UNDO_HISTORY),
            redo_stack: VecDeque::new(),
            focus_handle,
            on_submit,
            theme,
            config,
            content_height,
            last_render_state: None,
            snippet_state: None,
            suppress_keys: false,
            is_selecting: false,
            last_click_time: None,
            last_click_position: None,
            click_count: 0,
        }
    }

    /// Create a new EditorPrompt in template/snippet mode
    ///
    /// Parses the template string for VSCode snippet syntax ($1, ${1:default}, etc.)
    /// and enables tabstop navigation with Tab/Shift+Tab.
    #[allow(clippy::too_many_arguments)]
    pub fn with_template(
        id: String,
        template: String,
        language: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
        config: Arc<Config>,
        content_height: Option<gpui::Pixels>,
    ) -> Self {
        logging::log(
            "EDITOR",
            &format!(
                "EditorPrompt::with_template id={}, lang={}, template_len={}, height={:?}",
                id,
                language,
                template.len(),
                content_height
            ),
        );

        let snippet = ParsedSnippet::parse(&template);
        let content = snippet.text.clone();

        logging::log(
            "EDITOR",
            &format!(
                "Parsed snippet with {} tabstops, expanded to {} chars",
                snippet.tabstops.len(),
                content.len()
            ),
        );

        let rope = Rope::from_str(&content);
        let highlighted_lines = highlight_code_lines(&content, &language);

        // Set up initial cursor position and selection
        let (cursor, selection) = if !snippet.tabstops.is_empty() {
            // Select the first tabstop's range
            let first_tabstop = &snippet.tabstops[0];
            if let Some(&(start, end)) = first_tabstop.ranges.first() {
                let start_pos = Self::byte_to_cursor_static(&rope, start);
                let end_pos = Self::byte_to_cursor_static(&rope, end);
                (end_pos, Selection::new(start_pos, end_pos))
            } else {
                (
                    CursorPosition::start(),
                    Selection::caret(CursorPosition::start()),
                )
            }
        } else {
            (
                CursorPosition::start(),
                Selection::caret(CursorPosition::start()),
            )
        };

        let snippet_state = if snippet.tabstops.is_empty() {
            None
        } else {
            Some(SnippetState {
                snippet,
                current_tabstop_idx: 0,
                completed: false,
            })
        };

        Self {
            id,
            rope,
            language,
            cursor,
            selection,
            cursor_visible: true,
            highlighted_lines,
            needs_rehighlight: false,
            scroll_handle: UniformListScrollHandle::new(),
            undo_stack: VecDeque::with_capacity(MAX_UNDO_HISTORY),
            redo_stack: VecDeque::new(),
            focus_handle,
            on_submit,
            theme,
            config,
            content_height,
            last_render_state: None,
            snippet_state,
            suppress_keys: false,
            is_selecting: false,
            last_click_time: None,
            last_click_position: None,
            click_count: 0,
        }
    }

    /// Convert byte offset to CursorPosition (static helper for constructor)
    fn byte_to_cursor_static(rope: &Rope, byte_offset: usize) -> CursorPosition {
        // Convert byte offset to char offset
        let char_offset = rope.byte_to_char(byte_offset.min(rope.len_bytes()));
        let line = rope.char_to_line(char_offset);
        let line_start = rope.line_to_char(line);
        let column = char_offset - line_start;
        CursorPosition::new(line, column)
    }

    /// Set the content height (for dynamic resizing)
    pub fn set_height(&mut self, height: gpui::Pixels) {
        self.content_height = Some(height);
    }

    /// Get the configured font size
    fn font_size(&self) -> f32 {
        self.config.get_editor_font_size()
    }

    /// Get the line height based on configured font size
    fn line_height(&self) -> f32 {
        self.font_size() * LINE_HEIGHT_MULTIPLIER
    }

    /// Get char width scaled to configured font size
    fn char_width(&self) -> f32 {
        BASE_CHAR_WIDTH * (self.font_size() / BASE_FONT_SIZE)
    }

    /// Convert a pixel position to a cursor position (line, column)
    ///
    /// This accounts for:
    /// - The gutter width (line numbers)
    /// - Padding from config
    /// - Line height and character width
    /// - Scroll offset via uniform_list scroll tracking
    fn pixel_to_position(&self, pixel_pos: Point<gpui::Pixels>) -> CursorPosition {
        let padding = self.config.get_padding();
        let line_height = self.line_height();
        let char_width = self.char_width();

        // Convert from gpui::Pixels to f32
        let pos_x: f32 = pixel_pos.x.into();
        let pos_y: f32 = pixel_pos.y.into();

        // Account for top padding in pixel position
        let content_y = (pos_y - padding.top).max(0.0);

        // Calculate which visible line was clicked
        // uniform_list handles scroll internally - the position we receive is
        // relative to the viewport, and we need to calculate which item was hit
        let visible_line_idx = (content_y / line_height).floor() as usize;

        // For now, use a simple approach: assume we're looking at the top of the list
        // unless we have scroll info. uniform_list handles scroll but we need to
        // track which items are currently visible.
        //
        // TODO: For proper scroll-aware hit testing, we could:
        // 1. Store the last rendered range from render_lines()
        // 2. Use that to offset the visible_line_idx
        //
        // For now, clamp to valid range - this works well for short files
        // and reasonably for longer files when not scrolled.
        let line = visible_line_idx.min(self.line_count().saturating_sub(1));

        // Account for gutter and left padding for X position
        let content_x = pos_x - GUTTER_WIDTH - padding.left - 8.0; // 8.0 = pl_2() padding

        // Calculate column
        let column = if content_x <= 0.0 {
            0
        } else {
            let fractional_col = content_x / char_width;
            let col = fractional_col.round() as usize;
            // Clamp to line length
            col.min(self.line_len(line))
        };

        CursorPosition::new(line, column)
    }

    /// Get the current content as a String
    pub fn content(&self) -> String {
        self.rope.to_string()
    }

    /// Get the language
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    /// Get a specific line's content
    fn get_line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.rope.len_lines() {
            let line = self.rope.line(line_idx);
            // Remove trailing newline if present
            let s = line.to_string();
            Some(s.trim_end_matches('\n').to_string())
        } else {
            None
        }
    }

    /// Get the length of a specific line (in characters)
    fn line_len(&self, line_idx: usize) -> usize {
        if line_idx < self.rope.len_lines() {
            let line = self.rope.line(line_idx);
            let len = line.len_chars();
            // Don't count the newline character
            if len > 0 && line.char(len - 1) == '\n' {
                len - 1
            } else {
                len
            }
        } else {
            0
        }
    }

    /// Convert cursor position to rope char index
    fn cursor_to_char_idx(&self, pos: CursorPosition) -> usize {
        if pos.line >= self.rope.len_lines() {
            return self.rope.len_chars();
        }
        let line_start = self.rope.line_to_char(pos.line);
        let line_len = self.line_len(pos.line);
        line_start + pos.column.min(line_len)
    }

    /// Convert rope char index to cursor position
    fn char_idx_to_cursor(&self, char_idx: usize) -> CursorPosition {
        let char_idx = char_idx.min(self.rope.len_chars());
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        let column = char_idx - line_start;
        CursorPosition::new(line, column)
    }

    /// Save current state for undo
    fn save_undo_state(&mut self) {
        let snapshot = EditorSnapshot {
            content: self.rope.to_string(),
            cursor: self.cursor,
            selection: self.selection,
        };

        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(snapshot);
        self.redo_stack.clear();
    }

    /// Undo last action
    fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop_back() {
            // Save current state for redo
            let current = EditorSnapshot {
                content: self.rope.to_string(),
                cursor: self.cursor,
                selection: self.selection,
            };
            self.redo_stack.push_back(current);

            // Restore previous state
            self.rope = Rope::from_str(&snapshot.content);
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
            self.needs_rehighlight = true;
            logging::log("EDITOR", "Undo performed");
        }
    }

    /// Redo last undone action
    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop_back() {
            // Save current state for undo
            let current = EditorSnapshot {
                content: self.rope.to_string(),
                cursor: self.cursor,
                selection: self.selection,
            };
            self.undo_stack.push_back(current);

            // Restore redo state
            self.rope = Rope::from_str(&snapshot.content);
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
            self.needs_rehighlight = true;
            logging::log("EDITOR", "Redo performed");
        }
    }

    /// Rehighlight the content if needed
    fn rehighlight_if_needed(&mut self) {
        if self.needs_rehighlight {
            self.highlighted_lines = highlight_code_lines(&self.rope.to_string(), &self.language);
            self.needs_rehighlight = false;
        }
    }

    /// Insert text at cursor position
    fn insert_text(&mut self, text: &str) {
        self.save_undo_state();

        // Delete selection first if any
        if !self.selection.is_empty() {
            self.delete_selection_internal();
        }

        let char_idx = self.cursor_to_char_idx(self.cursor);
        self.rope.insert(char_idx, text);

        // Move cursor after inserted text
        let new_idx = char_idx + text.chars().count();
        self.cursor = self.char_idx_to_cursor(new_idx);
        self.selection = Selection::caret(self.cursor);
        self.needs_rehighlight = true;

        logging::log(
            "EDITOR",
            &format!("Inserted {} chars at {:?}", text.len(), self.cursor),
        );
    }

    /// Insert a single character
    fn insert_char(&mut self, ch: char) {
        self.insert_text(&ch.to_string());
    }

    /// Insert a newline
    fn insert_newline(&mut self) {
        self.insert_text("\n");
    }

    /// Delete selection without saving undo (internal use)
    fn delete_selection_internal(&mut self) {
        if self.selection.is_empty() {
            return;
        }

        let (start, end) = self.selection.ordered();
        let start_idx = self.cursor_to_char_idx(start);
        let end_idx = self.cursor_to_char_idx(end);

        self.rope.remove(start_idx..end_idx);
        self.cursor = start;
        self.selection = Selection::caret(start);
    }

    /// Delete selected text or character before cursor (backspace)
    fn backspace(&mut self) {
        self.save_undo_state();

        if !self.selection.is_empty() {
            self.delete_selection_internal();
        } else if self.cursor.line > 0 || self.cursor.column > 0 {
            let char_idx = self.cursor_to_char_idx(self.cursor);
            if char_idx > 0 {
                self.rope.remove((char_idx - 1)..char_idx);
                self.cursor = self.char_idx_to_cursor(char_idx - 1);
                self.selection = Selection::caret(self.cursor);
            }
        }
        self.needs_rehighlight = true;
    }

    /// Delete selected text or character after cursor
    fn delete(&mut self) {
        self.save_undo_state();

        if !self.selection.is_empty() {
            self.delete_selection_internal();
        } else {
            let char_idx = self.cursor_to_char_idx(self.cursor);
            if char_idx < self.rope.len_chars() {
                self.rope.remove(char_idx..(char_idx + 1));
            }
        }
        self.needs_rehighlight = true;
    }

    /// Move cursor left
    fn move_left(&mut self, extend_selection: bool) {
        let char_idx = self.cursor_to_char_idx(self.cursor);
        if char_idx > 0 {
            self.cursor = self.char_idx_to_cursor(char_idx - 1);
        }

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor right
    fn move_right(&mut self, extend_selection: bool) {
        let char_idx = self.cursor_to_char_idx(self.cursor);
        if char_idx < self.rope.len_chars() {
            self.cursor = self.char_idx_to_cursor(char_idx + 1);
        }

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor up
    fn move_up(&mut self, extend_selection: bool) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            let line_len = self.line_len(self.cursor.line);
            self.cursor.column = self.cursor.column.min(line_len);
        }

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor down
    fn move_down(&mut self, extend_selection: bool) {
        if self.cursor.line < self.line_count() - 1 {
            self.cursor.line += 1;
            let line_len = self.line_len(self.cursor.line);
            self.cursor.column = self.cursor.column.min(line_len);
        }

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor to start of line
    fn move_to_line_start(&mut self, extend_selection: bool) {
        self.cursor.column = 0;

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor to end of line
    fn move_to_line_end(&mut self, extend_selection: bool) {
        self.cursor.column = self.line_len(self.cursor.line);

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor to start of document
    fn move_to_document_start(&mut self, extend_selection: bool) {
        self.cursor = CursorPosition::start();

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor to end of document
    fn move_to_document_end(&mut self, extend_selection: bool) {
        let last_line = self.line_count().saturating_sub(1);
        self.cursor = CursorPosition::new(last_line, self.line_len(last_line));

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor by word (Option/Alt + arrow)
    fn move_word_left(&mut self, extend_selection: bool) {
        let char_idx = self.cursor_to_char_idx(self.cursor);
        if char_idx == 0 {
            return;
        }

        // Find the start of the previous word
        let text: String = self.rope.chars().take(char_idx).collect();
        let mut new_idx = char_idx;

        // Skip whitespace
        while new_idx > 0 {
            let ch = text.chars().nth(new_idx - 1).unwrap_or(' ');
            if !ch.is_whitespace() {
                break;
            }
            new_idx -= 1;
        }

        // Skip word characters
        while new_idx > 0 {
            let ch = text.chars().nth(new_idx - 1).unwrap_or(' ');
            if ch.is_whitespace() || !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            new_idx -= 1;
        }

        self.cursor = self.char_idx_to_cursor(new_idx);

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor by word (Option/Alt + arrow)
    fn move_word_right(&mut self, extend_selection: bool) {
        let char_idx = self.cursor_to_char_idx(self.cursor);
        let total_chars = self.rope.len_chars();
        if char_idx >= total_chars {
            return;
        }

        let text: String = self.rope.chars().collect();
        let mut new_idx = char_idx;

        // Skip current word characters
        while new_idx < total_chars {
            let ch = text.chars().nth(new_idx).unwrap_or(' ');
            if ch.is_whitespace() || (!ch.is_alphanumeric() && ch != '_') {
                break;
            }
            new_idx += 1;
        }

        // Skip whitespace
        while new_idx < total_chars {
            let ch = text.chars().nth(new_idx).unwrap_or(' ');
            if !ch.is_whitespace() {
                break;
            }
            new_idx += 1;
        }

        self.cursor = self.char_idx_to_cursor(new_idx);

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Select all text
    fn select_all(&mut self) {
        self.selection.anchor = CursorPosition::start();
        let last_line = self.line_count().saturating_sub(1);
        self.cursor = CursorPosition::new(last_line, self.line_len(last_line));
        self.selection.head = self.cursor;
    }

    /// Get selected text
    fn get_selected_text(&self) -> String {
        if self.selection.is_empty() {
            return String::new();
        }

        let (start, end) = self.selection.ordered();
        let start_idx = self.cursor_to_char_idx(start);
        let end_idx = self.cursor_to_char_idx(end);

        self.rope.slice(start_idx..end_idx).to_string()
    }

    /// Copy selection to clipboard
    fn copy(&self, cx: &mut Context<Self>) {
        let text = self.get_selected_text();
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            logging::log("EDITOR", "Copied to clipboard");
        }
    }

    /// Cut selection to clipboard
    fn cut(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }

        let text = self.get_selected_text();
        cx.write_to_clipboard(ClipboardItem::new_string(text));

        self.save_undo_state();
        self.delete_selection_internal();
        self.needs_rehighlight = true;
        logging::log("EDITOR", "Cut to clipboard");
    }

    /// Paste from clipboard
    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                self.insert_text(&text);
                logging::log("EDITOR", &format!("Pasted {} chars", text.len()));
            }
        }
    }

    /// Submit the current content
    fn submit(&self) {
        logging::log("EDITOR", &format!("Submit id={}", self.id));
        (self.on_submit)(self.id.clone(), Some(self.rope.to_string()));
    }

    /// Cancel - submit None
    fn cancel(&self) {
        logging::log("EDITOR", &format!("Cancel id={}", self.id));
        (self.on_submit)(self.id.clone(), None);
    }

    /// Navigate to the next tabstop in snippet mode
    fn next_tabstop(&mut self) {
        let Some(state) = &mut self.snippet_state else {
            return;
        };

        let tabstop_count = state.snippet.tabstops.len();
        if tabstop_count == 0 {
            return;
        }

        // Move to next tabstop
        state.current_tabstop_idx += 1;

        if state.current_tabstop_idx >= tabstop_count {
            // We've visited all tabstops - exit snippet mode
            state.completed = true;
            logging::log("EDITOR", "Snippet mode complete - all tabstops visited");

            // Clear snippet state and position cursor at end of last tabstop
            self.snippet_state = None;
            self.selection = Selection::caret(self.cursor);
            return;
        }

        // Select the new tabstop's range(s)
        self.select_current_tabstop();
    }

    /// Navigate to the previous tabstop in snippet mode
    fn prev_tabstop(&mut self) {
        let Some(state) = &mut self.snippet_state else {
            return;
        };

        if state.current_tabstop_idx == 0 {
            // Already at first tabstop, can't go back
            return;
        }

        state.current_tabstop_idx -= 1;
        self.select_current_tabstop();
    }

    /// Select the current tabstop's range(s) in snippet mode
    fn select_current_tabstop(&mut self) {
        let Some(state) = &self.snippet_state else {
            return;
        };

        let tabstops = &state.snippet.tabstops;
        if state.current_tabstop_idx >= tabstops.len() {
            return;
        }

        let tabstop = &tabstops[state.current_tabstop_idx];

        // For now, select only the first range (primary)
        // Linked editing support will be added in a future PR
        if let Some(&(start, end)) = tabstop.ranges.first() {
            let start_pos = Self::byte_to_cursor_static(&self.rope, start);
            let end_pos = Self::byte_to_cursor_static(&self.rope, end);

            self.cursor = end_pos;
            self.selection = Selection::new(start_pos, end_pos);

            logging::log(
                "EDITOR",
                &format!(
                    "Tabstop {} selected: range ({}, {}) -> cursor at ({}, {})",
                    tabstop.index, start, end, end_pos.line, end_pos.column
                ),
            );
        }
    }

    /// Check if we're currently in snippet mode
    #[allow(dead_code)]
    pub fn in_snippet_mode(&self) -> bool {
        self.snippet_state.is_some()
    }

    /// Get the current tabstop index (if in snippet mode)
    #[allow(dead_code)]
    pub fn current_tabstop_index(&self) -> Option<usize> {
        self.snippet_state.as_ref().map(|s| {
            s.snippet
                .tabstops
                .get(s.current_tabstop_idx)
                .map(|t| t.index)
                .unwrap_or(0)
        })
    }

    /// Handle keyboard input
    fn handle_key_event(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        // When actions panel is open, ignore all key events
        if self.suppress_keys {
            return;
        }

        let key = event.keystroke.key.to_lowercase();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;
        let alt = event.keystroke.modifiers.alt;

        match (key.as_str(), cmd, shift, alt) {
            // Submit/Cancel
            ("enter", true, false, false) => self.submit(),
            ("escape", _, _, _) => self.cancel(),

            // Undo/Redo
            ("z", true, false, false) => self.undo(),
            ("z", true, true, false) => self.redo(),

            // Clipboard
            ("c", true, false, false) => self.copy(cx),
            ("x", true, false, false) => self.cut(cx),
            ("v", true, false, false) => self.paste(cx),

            // Select all
            ("a", true, false, false) => self.select_all(),

            // Navigation (basic arrow keys, with or without shift for selection)
            // GPUI may send "up" or "arrowup" depending on platform/context
            ("left" | "arrowleft", false, _, false) => self.move_left(shift),
            ("right" | "arrowright", false, _, false) => self.move_right(shift),
            ("up" | "arrowup", false, _, false) => self.move_up(shift),
            ("down" | "arrowdown", false, _, false) => self.move_down(shift),

            // Word navigation (Alt/Option + arrow)
            ("left" | "arrowleft", false, _, true) => self.move_word_left(shift),
            ("right" | "arrowright", false, _, true) => self.move_word_right(shift),

            // Line start/end (Cmd+Left/Right on Mac)
            ("left" | "arrowleft", true, _, false) => self.move_to_line_start(shift),
            ("right" | "arrowright", true, _, false) => self.move_to_line_end(shift),
            ("home", false, _, _) => self.move_to_line_start(shift),
            ("end", false, _, _) => self.move_to_line_end(shift),

            // Document start/end (Cmd+Up/Down on Mac, Cmd+Home/End)
            ("up" | "arrowup", true, _, false) => self.move_to_document_start(shift),
            ("down" | "arrowdown", true, _, false) => self.move_to_document_end(shift),
            ("home", true, _, _) => self.move_to_document_start(shift),
            ("end", true, _, _) => self.move_to_document_end(shift),

            // Editing
            ("backspace", _, _, _) => self.backspace(),
            ("delete", _, _, _) => self.delete(),
            ("enter", false, _, _) => self.insert_newline(),

            // Tab handling - snippet mode or regular indent
            ("tab", false, false, false) => {
                if self.snippet_state.is_some() {
                    self.next_tabstop();
                } else {
                    self.insert_text("    "); // 4 spaces for tab
                }
            }
            ("tab", false, true, false) => {
                // Shift+Tab - previous tabstop in snippet mode
                if self.snippet_state.is_some() {
                    self.prev_tabstop();
                }
                // In non-snippet mode, Shift+Tab does nothing for now
            }

            // Character input
            _ => {
                if let Some(ref key_char) = event.keystroke.key_char {
                    if let Some(ch) = key_char.chars().next() {
                        if !ch.is_control() && !cmd {
                            self.insert_char(ch);
                        }
                    }
                }
            }
        }

        cx.notify();
    }

    /// Handle mouse down event - start selection or position cursor
    fn handle_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let pos = self.pixel_to_position(event.position);
        let now = Instant::now();
        let multi_click_threshold = Duration::from_millis(500);

        // Check if this is a multi-click (same position, within time window)
        let is_same_position = self.last_click_position == Some(pos);
        let is_quick_click = self
            .last_click_time
            .map(|t| now.duration_since(t) < multi_click_threshold)
            .unwrap_or(false);

        if is_same_position && is_quick_click {
            self.click_count = (self.click_count + 1).min(3);
        } else {
            self.click_count = 1;
        }

        self.last_click_time = Some(now);
        self.last_click_position = Some(pos);
        self.is_selecting = true;

        match self.click_count {
            1 => {
                // Single click: position cursor and start selection
                logging::log(
                    "EDITOR",
                    &format!("Mouse down: single click at line {}, col {}", pos.line, pos.column),
                );
                self.cursor = pos;
                self.selection = Selection::caret(pos);
            }
            2 => {
                // Double click: select word
                logging::log(
                    "EDITOR",
                    &format!("Mouse down: double click at line {}, col {}", pos.line, pos.column),
                );
                self.select_word_at(pos);
            }
            3 => {
                // Triple click: select line
                logging::log(
                    "EDITOR",
                    &format!("Mouse down: triple click at line {}, col {}", pos.line, pos.column),
                );
                self.select_line(pos.line);
            }
            _ => {}
        }

        cx.notify();
    }

    /// Handle mouse move event - extend selection while dragging
    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }

        let pos = self.pixel_to_position(event.position);

        // Extend selection based on click count
        match self.click_count {
            1 => {
                // Character-level selection
                self.cursor = pos;
                self.selection.head = pos;
            }
            2 => {
                // Word-level selection: extend to word boundaries
                self.extend_word_selection(pos);
            }
            3 => {
                // Line-level selection: extend to include full lines
                self.extend_line_selection(pos);
            }
            _ => {}
        }

        cx.notify();
    }

    /// Handle mouse up event - finalize selection
    fn handle_mouse_up(&mut self, _event: &MouseUpEvent, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.is_selecting = false;
            logging::log(
                "EDITOR",
                &format!(
                    "Mouse up: selection finalized, cursor at line {}, col {}",
                    self.cursor.line, self.cursor.column
                ),
            );
        }
        cx.notify();
    }

    /// Select the word at the given position
    fn select_word_at(&mut self, pos: CursorPosition) {
        if let Some(line_content) = self.get_line(pos.line) {
            let chars: Vec<char> = line_content.chars().collect();
            if chars.is_empty() || pos.column >= chars.len() {
                // Empty line or at end - just position cursor
                self.cursor = pos;
                self.selection = Selection::caret(pos);
                return;
            }

            // Find word boundaries
            let (start, end) = self.find_word_boundaries(&chars, pos.column);

            self.selection = Selection::new(
                CursorPosition::new(pos.line, start),
                CursorPosition::new(pos.line, end),
            );
            self.cursor = CursorPosition::new(pos.line, end);
        }
    }

    /// Find word boundaries around a given column position
    fn find_word_boundaries(&self, chars: &[char], column: usize) -> (usize, usize) {
        let column = column.min(chars.len().saturating_sub(1));

        // Check if we're on a word character
        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

        let start_char = chars.get(column).copied().unwrap_or(' ');

        if is_word_char(start_char) {
            // Find start of word
            let mut start = column;
            while start > 0 && is_word_char(chars[start - 1]) {
                start -= 1;
            }

            // Find end of word
            let mut end = column;
            while end < chars.len() && is_word_char(chars[end]) {
                end += 1;
            }

            (start, end)
        } else if start_char.is_whitespace() {
            // Select whitespace block
            let mut start = column;
            while start > 0 && chars[start - 1].is_whitespace() {
                start -= 1;
            }

            let mut end = column;
            while end < chars.len() && chars[end].is_whitespace() {
                end += 1;
            }

            (start, end)
        } else {
            // Select single punctuation character
            (column, column + 1)
        }
    }

    /// Select an entire line
    fn select_line(&mut self, line: usize) {
        let line = line.min(self.line_count().saturating_sub(1));
        let line_len = self.line_len(line);

        self.selection = Selection::new(
            CursorPosition::new(line, 0),
            CursorPosition::new(line, line_len),
        );
        self.cursor = CursorPosition::new(line, line_len);
    }

    /// Extend word selection during drag
    fn extend_word_selection(&mut self, pos: CursorPosition) {
        if let Some(line_content) = self.get_line(pos.line) {
            let chars: Vec<char> = line_content.chars().collect();
            let column = pos.column.min(chars.len());

            let (word_start, word_end) = if column < chars.len() {
                self.find_word_boundaries(&chars, column)
            } else {
                (column, column)
            };

            // Keep anchor at original word boundary, extend to current word boundary
            let anchor = self.selection.anchor;

            if pos.line < anchor.line || (pos.line == anchor.line && word_start < anchor.column) {
                // Extending backwards
                self.selection.head = CursorPosition::new(pos.line, word_start);
                self.cursor = self.selection.head;
            } else {
                // Extending forwards
                self.selection.head = CursorPosition::new(pos.line, word_end);
                self.cursor = self.selection.head;
            }
        }
    }

    /// Extend line selection during drag
    fn extend_line_selection(&mut self, pos: CursorPosition) {
        let anchor_line = self.selection.anchor.line;
        let current_line = pos.line.min(self.line_count().saturating_sub(1));

        if current_line < anchor_line {
            // Dragging upward
            self.selection = Selection::new(
                CursorPosition::new(anchor_line, self.line_len(anchor_line)),
                CursorPosition::new(current_line, 0),
            );
            self.cursor = CursorPosition::new(current_line, 0);
        } else {
            // Dragging downward or same line
            let end_line_len = self.line_len(current_line);
            self.selection = Selection::new(
                CursorPosition::new(anchor_line, 0),
                CursorPosition::new(current_line, end_line_len),
            );
            self.cursor = CursorPosition::new(current_line, end_line_len);
        }
    }

    /// Render a range of lines for uniform_list virtualization
    fn render_lines(
        &mut self,
        range: Range<usize>,
        _cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement> {
        self.rehighlight_if_needed();

        let colors = &self.theme.colors;
        let (sel_start, sel_end) = self.selection.ordered();

        range
            .map(|line_idx| {
                let line_content = self.get_line(line_idx).unwrap_or_default();
                let line_number = line_idx + 1;
                let highlighted_line = self.highlighted_lines.get(line_idx);

                // Check if cursor is on this line
                let cursor_on_line = self.cursor.line == line_idx && self.cursor_visible;
                let cursor_column = if cursor_on_line {
                    Some(self.cursor.column)
                } else {
                    None
                };

                // Check if this line has selection
                let line_has_selection = !self.selection.is_empty()
                    && line_idx >= sel_start.line
                    && line_idx <= sel_end.line;

                let selection_range = if line_has_selection {
                    let start_col = if line_idx == sel_start.line {
                        sel_start.column
                    } else {
                        0
                    };
                    let end_col = if line_idx == sel_end.line {
                        sel_end.column
                    } else {
                        line_content.len()
                    };
                    Some((start_col, end_col))
                } else {
                    None
                };

                self.render_line(
                    line_idx,
                    line_number,
                    &line_content,
                    highlighted_line,
                    cursor_column,
                    selection_range,
                    colors,
                )
            })
            .collect()
    }

    /// Render a single line with cursor and selection
    #[allow(clippy::too_many_arguments)]
    fn render_line(
        &self,
        line_idx: usize,
        line_number: usize,
        line_content: &str,
        highlighted_line: Option<&HighlightedLine>,
        cursor_column: Option<usize>,
        selection_range: Option<(usize, usize)>,
        colors: &crate::theme::ColorScheme,
    ) -> impl IntoElement {
        let line_height = px(self.line_height());
        let font_size = px(self.font_size());
        let gutter_width = px(GUTTER_WIDTH);

        // Build the line content with syntax highlighting, cursor, and selection
        let content_element = if let Some(hl_line) = highlighted_line {
            self.render_highlighted_line(
                line_content,
                hl_line,
                cursor_column,
                selection_range,
                colors,
            )
        } else {
            self.render_plain_line(line_content, cursor_column, selection_range, colors)
        };

        div()
            .id(("editor-line", line_idx))
            .flex()
            .flex_row()
            .h(line_height)
            .w_full()
            .font_family("Menlo")
            .text_size(font_size)
            .child(
                // Line number gutter
                div()
                    .w(gutter_width)
                    .flex_shrink_0()
                    .text_color(rgb(colors.text.muted))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(SharedString::from(format!("{}", line_number))),
            )
            .child(
                // Code content with cursor and selection
                div()
                    .flex_1()
                    .pl_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .overflow_hidden()
                    .child(content_element),
            )
    }

    /// Render a line with syntax highlighting
    fn render_highlighted_line(
        &self,
        _line_content: &str,
        hl_line: &HighlightedLine,
        cursor_column: Option<usize>,
        selection_range: Option<(usize, usize)>,
        colors: &crate::theme::ColorScheme,
    ) -> gpui::AnyElement {
        let mut elements: Vec<gpui::AnyElement> = Vec::new();
        let mut char_offset = 0;

        for span in &hl_line.spans {
            let span_len = span.text.chars().count();
            let span_start = char_offset;
            let span_end = char_offset + span_len;

            // Render this span, potentially with cursor and/or selection
            let span_element = self.render_span(
                &span.text,
                span.color,
                span_start,
                cursor_column,
                selection_range,
                colors,
            );
            elements.push(span_element.into_any_element());

            char_offset = span_end;
        }

        // If cursor is at the end of the line (after all content)
        if let Some(col) = cursor_column {
            if col >= char_offset {
                elements.push(self.render_cursor(colors).into_any_element());
            }
        }

        div()
            .flex()
            .flex_row()
            .children(elements)
            .into_any_element()
    }

    /// Render a plain line (no syntax highlighting)
    fn render_plain_line(
        &self,
        line_content: &str,
        cursor_column: Option<usize>,
        selection_range: Option<(usize, usize)>,
        colors: &crate::theme::ColorScheme,
    ) -> gpui::AnyElement {
        let span_element = self.render_span(
            line_content,
            colors.text.primary,
            0,
            cursor_column,
            selection_range,
            colors,
        );

        let mut elements: Vec<gpui::AnyElement> = vec![span_element.into_any_element()];

        // If cursor is at the end of the line
        if let Some(col) = cursor_column {
            if col >= line_content.chars().count() {
                elements.push(self.render_cursor(colors).into_any_element());
            }
        }

        div()
            .flex()
            .flex_row()
            .children(elements)
            .into_any_element()
    }

    /// Render a text span with potential cursor and selection
    fn render_span(
        &self,
        text: &str,
        text_color: u32,
        span_start: usize,
        cursor_column: Option<usize>,
        selection_range: Option<(usize, usize)>,
        colors: &crate::theme::ColorScheme,
    ) -> impl IntoElement {
        let span_len = text.chars().count();
        let span_end = span_start + span_len;

        // Check if cursor is within this span
        let cursor_in_span = cursor_column
            .map(|col| col >= span_start && col < span_end)
            .unwrap_or(false);

        // Check if selection overlaps this span
        let selection_in_span = selection_range
            .map(|(sel_start, sel_end)| sel_start < span_end && sel_end > span_start)
            .unwrap_or(false);

        if !cursor_in_span && !selection_in_span {
            // Simple case: no cursor or selection in this span
            return div()
                .text_color(rgb(text_color))
                .child(SharedString::from(text.to_string()))
                .into_any_element();
        }

        // Complex case: need to split the span
        let mut elements: Vec<gpui::AnyElement> = Vec::new();
        let chars: Vec<char> = text.chars().collect();

        let mut i = 0;
        while i < chars.len() {
            let global_idx = span_start + i;

            // Check if cursor is at this position
            if cursor_column == Some(global_idx) {
                elements.push(self.render_cursor(colors).into_any_element());
            }

            // Determine if this character is selected
            let is_selected = selection_range
                .map(|(sel_start, sel_end)| global_idx >= sel_start && global_idx < sel_end)
                .unwrap_or(false);

            // Collect consecutive characters with same selection state
            let mut end_i = i + 1;
            while end_i < chars.len() {
                let next_global_idx = span_start + end_i;
                let next_selected = selection_range
                    .map(|(sel_start, sel_end)| {
                        next_global_idx >= sel_start && next_global_idx < sel_end
                    })
                    .unwrap_or(false);

                // Break if selection state changes or cursor is here
                if next_selected != is_selected || cursor_column == Some(next_global_idx) {
                    break;
                }
                end_i += 1;
            }

            let chunk: String = chars[i..end_i].iter().collect();

            let chunk_element = if is_selected {
                div()
                    .bg(rgba(0x3399FF44))
                    .text_color(rgb(text_color))
                    .child(SharedString::from(chunk))
            } else {
                div()
                    .text_color(rgb(text_color))
                    .child(SharedString::from(chunk))
            };

            elements.push(chunk_element.into_any_element());
            i = end_i;
        }

        div()
            .flex()
            .flex_row()
            .children(elements)
            .into_any_element()
    }

    /// Render the cursor
    fn render_cursor(&self, colors: &crate::theme::ColorScheme) -> impl IntoElement {
        let cursor_height = self.line_height() - 4.0;
        div()
            .w(px(2.0))
            .h(px(cursor_height))
            .bg(rgb(colors.accent.selected))
            .my(px(2.0))
    }

    /// Render the status bar at the bottom
    fn render_status_bar(&self) -> impl IntoElement {
        let colors = &self.theme.colors;
        let line_count = self.line_count();
        let cursor_info = format!(
            "Ln {}, Col {}",
            self.cursor.line + 1,
            self.cursor.column + 1
        );

        div()
            .flex()
            .flex_row()
            .h(px(28.))
            .px_4()
            .items_center()
            .justify_between()
            .bg(rgb(colors.background.title_bar))
            .border_t_1()
            .border_color(rgb(colors.ui.border))
            .font_family("Menlo")
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(
                        div()
                            .text_color(rgb(colors.text.secondary))
                            .text_xs()
                            .child(SharedString::from(format!("{} lines", line_count))),
                    )
                    .child(
                        div()
                            .text_color(rgb(colors.text.secondary))
                            .text_xs()
                            .child(SharedString::from(cursor_info)),
                    ),
            )
            .child(
                div()
                    .text_color(rgb(colors.text.muted))
                    .text_xs()
                    .child(SharedString::from(self.language.clone())),
            )
    }
}

impl Focusable for EditorPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EditorPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &self.theme.colors;
        let line_count = self.line_count();

        // Get padding from config
        let padding = self.config.get_padding();

        // Keyboard handler
        let handle_key = cx.listener(|this, event: &gpui::KeyDownEvent, _window, cx| {
            this.handle_key_event(event, cx);
        });

        // Status bar height constant
        const STATUS_BAR_HEIGHT: f32 = 28.0;

        // Calculate editor area height: use explicit height if available, otherwise use flex
        let editor_area = if let Some(total_height) = self.content_height {
            // Explicit height: editor gets total - status bar
            // Note: padding is INSIDE the div (pt/pl/pr), not added to its height
            let editor_height = total_height - gpui::px(STATUS_BAR_HEIGHT);

            // Only log when render state changes (avoid log spam every ~500ms)
            let current_state = RenderState {
                line_count,
                total_height: Some(total_height),
                editor_height: Some(editor_height),
            };

            if self.last_render_state.as_ref() != Some(&current_state) {
                tracing::debug!(
                    target: "script_kit_gpui::editor",
                    total_height = ?total_height,
                    editor_height = ?editor_height,
                    status_bar = STATUS_BAR_HEIGHT,
                    line_count = line_count,
                    "Editor render state changed"
                );
                self.last_render_state = Some(current_state);
            }

            div()
                .w_full()
                .h(editor_height)
                .pt(px(padding.top))
                .pl(px(padding.left))
                .pr(px(padding.right))
                .overflow_hidden()
                .child(
                    uniform_list(
                        "editor-lines",
                        line_count,
                        cx.processor(|this, range: Range<usize>, _window, cx| {
                            this.render_lines(range, cx)
                        }),
                    )
                    .track_scroll(&self.scroll_handle)
                    .size_full(),
                )
        } else {
            // Fallback: use flex (may not work in all GPUI contexts)
            div()
                .flex_1()
                .w_full()
                .min_h(px(0.))
                .pt(px(padding.top))
                .pl(px(padding.left))
                .pr(px(padding.right))
                .overflow_hidden()
                .child(
                    uniform_list(
                        "editor-lines",
                        line_count,
                        cx.processor(|this, range: Range<usize>, _window, cx| {
                            this.render_lines(range, cx)
                        }),
                    )
                    .track_scroll(&self.scroll_handle)
                    .size_full(),
                )
        };

        // Mouse handlers
        let handle_mouse_down = cx.listener(|this, event: &MouseDownEvent, _window, cx| {
            this.handle_mouse_down(event, cx);
        });

        let handle_mouse_move = cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
            this.handle_mouse_move(event, cx);
        });

        let handle_mouse_up = cx.listener(|this, event: &MouseUpEvent, _window, cx| {
            this.handle_mouse_up(event, cx);
        });

        // Build the container - use explicit height if available
        let container = div()
            .id("editor-prompt")
            .key_context("EditorPrompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .on_mouse_down(MouseButton::Left, handle_mouse_down)
            .on_mouse_move(handle_mouse_move)
            .on_mouse_up(MouseButton::Left, handle_mouse_up)
            .flex()
            .flex_col()
            .w_full()
            .bg(rgb(colors.background.main))
            .font_family("Menlo");

        // Apply height
        let container = if let Some(h) = self.content_height {
            container.h(h)
        } else {
            container.size_full().min_h(px(0.))
        };

        container.child(editor_area).child(self.render_status_bar())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_position() {
        let pos = CursorPosition::new(5, 10);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 10);
    }

    #[test]
    fn test_selection_ordered() {
        let sel = Selection::new(CursorPosition::new(5, 10), CursorPosition::new(2, 5));
        let (start, end) = sel.ordered();
        assert_eq!(start.line, 2);
        assert_eq!(end.line, 5);
    }

    #[test]
    fn test_selection_is_empty() {
        let pos = CursorPosition::new(3, 7);
        let sel = Selection::caret(pos);
        assert!(sel.is_empty());

        let sel2 = Selection::new(CursorPosition::new(0, 0), CursorPosition::new(0, 5));
        assert!(!sel2.is_empty());
    }

    #[test]
    fn test_line_count_empty() {
        let content = "";
        let lines = highlight_code_lines(content, "text");
        assert!(lines.is_empty() || lines.len() == 1);
    }

    #[test]
    fn test_line_count_multiline() {
        let content = "line1\nline2\nline3";
        let lines = highlight_code_lines(content, "text");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_typescript_highlighting() {
        let content = "const x: number = 42;";
        let lines = highlight_code_lines(content, "typescript");
        assert!(!lines.is_empty());
        assert!(!lines[0].spans.is_empty());
    }

    /// Regression test: Verify arrow key patterns match BOTH short and long forms.
    /// GPUI sends "up"/"down"/"left"/"right" on macOS, but we must also handle
    /// "arrowup"/"arrowdown"/"arrowleft"/"arrowright" for cross-platform compatibility.
    ///
    /// This test reads the source code and verifies the patterns are correct.
    /// If this test fails, arrow keys will be broken in the editor!
    #[test]
    fn test_arrow_key_patterns_match_both_forms() {
        let source = include_str!("editor.rs");

        // These patterns MUST exist - they match both key name variants
        let required_patterns = [
            r#""up" | "arrowup""#,
            r#""down" | "arrowdown""#,
            r#""left" | "arrowleft""#,
            r#""right" | "arrowright""#,
        ];

        for pattern in required_patterns {
            assert!(
                source.contains(pattern),
                "CRITICAL: Missing arrow key pattern '{}' in editor.rs!\n\
                 Arrow keys will be BROKEN. GPUI sends short names like 'up' but we must match both forms.\n\
                 Fix: Use pattern matching like: \"up\" | \"arrowup\" => ...",
                pattern
            );
        }

        // These patterns are WRONG - they only match one form
        let forbidden_patterns = [
            // Standalone arrowup without the short form - this is broken!
            ("(\"arrowup\", false, _, false)", "arrowup without 'up'"),
            (
                "(\"arrowdown\", false, _, false)",
                "arrowdown without 'down'",
            ),
            (
                "(\"arrowleft\", false, _, false)",
                "arrowleft without 'left'",
            ),
            (
                "(\"arrowright\", false, _, false)",
                "arrowright without 'right'",
            ),
        ];

        for (pattern, desc) in forbidden_patterns {
            assert!(
                !source.contains(pattern),
                "CRITICAL: Found broken arrow key pattern ({}) in editor.rs!\n\
                 Pattern '{}' only matches long form. GPUI sends short names like 'up'.\n\
                 Fix: Use \"up\" | \"arrowup\" instead of just \"arrowup\"",
                desc,
                pattern
            );
        }
    }

    #[test]
    fn test_snippet_state_creation() {
        let snippet = ParsedSnippet::parse("Hello ${1:world}!");
        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
            completed: false,
        };
        assert_eq!(state.current_tabstop_idx, 0);
        assert!(!state.completed);
        assert_eq!(state.snippet.tabstops.len(), 1);
    }

    #[test]
    fn test_snippet_state_with_multiple_tabstops() {
        let snippet = ParsedSnippet::parse("${1:first} ${2:second} ${0:end}");
        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
            completed: false,
        };
        // Order should be 1, 2, 0 (0 is always last)
        assert_eq!(state.snippet.tabstops.len(), 3);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.tabstops[1].index, 2);
        assert_eq!(state.snippet.tabstops[2].index, 0);
    }

    #[test]
    fn test_byte_to_cursor_static() {
        let rope = Rope::from_str("Hello\nWorld");

        // "Hello" is 5 bytes, cursor at start
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);

        // After "Hello" (position 5)
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 5);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 5);

        // After "Hello\n" (position 6) - start of second line
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);

        // "Hello\nWor" (position 9)
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 9);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 3);
    }

    #[test]
    fn test_byte_to_cursor_static_clamps_to_end() {
        let rope = Rope::from_str("Hello");

        // Beyond end should clamp
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 100);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 5);
    }
}
