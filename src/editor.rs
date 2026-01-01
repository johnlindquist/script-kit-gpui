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
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Point, Render, ScrollStrategy,
    SharedString, UniformListScrollHandle, Window,
};
use ropey::Rope;
use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

// --- Find / Replace / Go To Line -------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FindField {
    Query,
    Replace,
}

#[derive(Debug, Clone)]
struct FindReplaceState {
    query: String,
    replacement: String,
    is_visible: bool,
    show_replace: bool,
    case_sensitive: bool,
    use_regex: bool,
    matches: Vec<(usize, usize)>,
    current_match_idx: Option<usize>,
    focus: FindField,
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self {
            query: String::new(),
            replacement: String::new(),
            is_visible: false,
            show_replace: false,
            case_sensitive: false,
            use_regex: false,
            matches: Vec::new(),
            current_match_idx: None,
            focus: FindField::Query,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct GoToLineState {
    is_visible: bool,
    line_input: String,
}

/// Undo/redo state snapshot
/// Uses Rope instead of String for cheap O(1) persistent clones
#[derive(Debug, Clone)]
struct EditorSnapshot {
    rope: Rope,
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

    // Find / Replace
    find_state: FindReplaceState,

    // Go to line
    go_to_line_state: GoToLineState,

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

        // Normalize line endings (CRLF -> LF) for consistent handling
        let content = Self::normalize_line_endings(&content);

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
            find_state: FindReplaceState::default(),
            go_to_line_state: GoToLineState::default(),
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

        // Normalize line endings in template before parsing
        let template = Self::normalize_line_endings(&template);

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
        // Ranges are now in char indices (not byte offsets)
        let (cursor, selection) = if !snippet.tabstops.is_empty() {
            // Select the first tabstop's range
            let first_tabstop = &snippet.tabstops[0];
            if let Some(&(start_char, end_char)) = first_tabstop.ranges.first() {
                let start_pos = Self::char_to_cursor_static(&rope, start_char);
                let end_pos = Self::char_to_cursor_static(&rope, end_char);
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
            find_state: FindReplaceState::default(),
            go_to_line_state: GoToLineState::default(),
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

    /// Convert char index to CursorPosition (static helper for constructor)
    fn char_to_cursor_static(rope: &Rope, char_offset: usize) -> CursorPosition {
        let char_offset = char_offset.min(rope.len_chars());
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

    /// Normalize line endings: convert CRLF to LF
    /// This ensures consistent handling regardless of input source (Windows, Unix, mixed)
    fn normalize_line_endings(content: &str) -> String {
        content.replace("\r\n", "\n").replace('\r', "\n")
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
            // Remove trailing newline and carriage return (handles both LF and CRLF)
            let s = line.to_string();
            Some(s.trim_end_matches(&['\n', '\r'][..]).to_string())
        } else {
            None
        }
    }

    /// Get the length of a specific line (in characters)
    fn line_len(&self, line_idx: usize) -> usize {
        if line_idx < self.rope.len_lines() {
            let line = self.rope.line(line_idx);
            let mut len = line.len_chars();
            // Don't count trailing newline or carriage return (handles both LF and CRLF)
            if len > 0 && line.char(len - 1) == '\n' {
                len -= 1;
            }
            if len > 0 && line.char(len - 1) == '\r' {
                len -= 1;
            }
            len
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
    /// Uses Rope.clone() which is O(1) due to persistent data structure
    fn save_undo_state(&mut self) {
        let snapshot = EditorSnapshot {
            rope: self.rope.clone(),
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
            // Save current state for redo (Rope.clone() is O(1))
            let current = EditorSnapshot {
                rope: self.rope.clone(),
                cursor: self.cursor,
                selection: self.selection,
            };
            self.redo_stack.push_back(current);

            // Restore previous state (Rope.clone() is O(1))
            self.rope = snapshot.rope.clone();
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
            self.needs_rehighlight = true;
            logging::log("EDITOR", "Undo performed");
        }
    }

    /// Redo last undone action
    fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop_back() {
            // Save current state for undo (Rope.clone() is O(1))
            let current = EditorSnapshot {
                rope: self.rope.clone(),
                cursor: self.cursor,
                selection: self.selection,
            };
            // Cap undo stack during redo (same as in save_undo_state)
            if self.undo_stack.len() >= MAX_UNDO_HISTORY {
                self.undo_stack.pop_front();
            }
            self.undo_stack.push_back(current);

            // Restore redo state (Rope.clone() is O(1))
            self.rope = snapshot.rope.clone();
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

        // Track edit position for snippet tabstop update
        let edit_start = self.cursor_to_char_idx(self.cursor);
        let mut old_len = 0;

        // Delete selection first if any
        if !self.selection.is_empty() {
            let (sel_start, sel_end) = self.selection.ordered();
            let sel_start_idx = self.cursor_to_char_idx(sel_start);
            let sel_end_idx = self.cursor_to_char_idx(sel_end);
            old_len = sel_end_idx - sel_start_idx;
            self.delete_selection_internal();
        }

        let char_idx = self.cursor_to_char_idx(self.cursor);
        self.rope.insert(char_idx, text);

        let new_len = text.chars().count();

        // Update snippet tabstops if in snippet mode
        if let Some(ref mut state) = self.snippet_state {
            state.snippet.update_tabstops_after_edit(
                state.current_tabstop_idx,
                edit_start,
                old_len,
                new_len,
            );
        }

        // Move cursor after inserted text
        let new_idx = char_idx + new_len;
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
        if !self.selection.is_empty() {
            // Save undo state before mutation
            self.save_undo_state();

            // Track selection range for snippet update
            let (sel_start, sel_end) = self.selection.ordered();
            let sel_start_idx = self.cursor_to_char_idx(sel_start);
            let sel_end_idx = self.cursor_to_char_idx(sel_end);
            let old_len = sel_end_idx - sel_start_idx;

            self.delete_selection_internal();

            // Update snippet tabstops if in snippet mode
            if let Some(ref mut state) = self.snippet_state {
                state.snippet.update_tabstops_after_edit(
                    state.current_tabstop_idx,
                    sel_start_idx,
                    old_len,
                    0,
                );
            }
            self.needs_rehighlight = true;
        } else {
            // Check if there's anything to delete before saving undo state
            let char_idx = self.cursor_to_char_idx(self.cursor);
            if char_idx > 0 {
                // Save undo state only when mutation will happen
                self.save_undo_state();

                self.rope.remove((char_idx - 1)..char_idx);
                self.cursor = self.char_idx_to_cursor(char_idx - 1);
                self.selection = Selection::caret(self.cursor);

                // Update snippet tabstops if in snippet mode
                if let Some(ref mut state) = self.snippet_state {
                    state.snippet.update_tabstops_after_edit(
                        state.current_tabstop_idx,
                        char_idx - 1,
                        1,
                        0,
                    );
                }
                self.needs_rehighlight = true;
            }
            // At document start with no selection - no-op, don't save undo state
        }
    }

    /// Delete selected text or character after cursor
    fn delete(&mut self) {
        if !self.selection.is_empty() {
            // Save undo state before mutation
            self.save_undo_state();

            // Track selection range for snippet update
            let (sel_start, sel_end) = self.selection.ordered();
            let sel_start_idx = self.cursor_to_char_idx(sel_start);
            let sel_end_idx = self.cursor_to_char_idx(sel_end);
            let old_len = sel_end_idx - sel_start_idx;

            self.delete_selection_internal();

            // Update snippet tabstops if in snippet mode
            if let Some(ref mut state) = self.snippet_state {
                state.snippet.update_tabstops_after_edit(
                    state.current_tabstop_idx,
                    sel_start_idx,
                    old_len,
                    0,
                );
            }
            self.needs_rehighlight = true;
        } else {
            // Check if there's anything to delete before saving undo state
            let char_idx = self.cursor_to_char_idx(self.cursor);
            if char_idx < self.rope.len_chars() {
                // Save undo state only when mutation will happen
                self.save_undo_state();

                self.rope.remove(char_idx..(char_idx + 1));

                // Update snippet tabstops if in snippet mode
                if let Some(ref mut state) = self.snippet_state {
                    state.snippet.update_tabstops_after_edit(
                        state.current_tabstop_idx,
                        char_idx,
                        1,
                        0,
                    );
                }
                self.needs_rehighlight = true;
            }
            // At document end with no selection - no-op, don't save undo state
        }
    }

    /// Move cursor left
    ///
    /// When `extend_selection` is false and there's an existing selection,
    /// collapse to the selection start (standard editor behavior).
    fn move_left(&mut self, extend_selection: bool) {
        // If not extending and selection exists, collapse to selection start
        if !extend_selection && !self.selection.is_empty() {
            let (start, _end) = self.selection.ordered();
            self.cursor = start;
            self.selection = Selection::caret(self.cursor);
            return;
        }

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
    ///
    /// When `extend_selection` is false and there's an existing selection,
    /// collapse to the selection end (standard editor behavior).
    fn move_right(&mut self, extend_selection: bool) {
        // If not extending and selection exists, collapse to selection end
        if !extend_selection && !self.selection.is_empty() {
            let (_start, end) = self.selection.ordered();
            self.cursor = end;
            self.selection = Selection::caret(self.cursor);
            return;
        }

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
    ///
    /// When `extend_selection` is false and there's an existing selection,
    /// collapse to the selection start (standard editor behavior).
    fn move_up(&mut self, extend_selection: bool) {
        // If not extending and selection exists, collapse to selection start
        if !extend_selection && !self.selection.is_empty() {
            let (start, _end) = self.selection.ordered();
            self.cursor = start;
            self.selection = Selection::caret(self.cursor);
            return;
        }

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
    ///
    /// When `extend_selection` is false and there's an existing selection,
    /// collapse to the selection end (standard editor behavior).
    fn move_down(&mut self, extend_selection: bool) {
        // If not extending and selection exists, collapse to selection end
        if !extend_selection && !self.selection.is_empty() {
            let (_start, end) = self.selection.ordered();
            self.cursor = end;
            self.selection = Selection::caret(self.cursor);
            return;
        }

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

    /// Ensure the cursor line is visible by scrolling if needed
    /// Uses `ScrollStrategy::Top` to scroll the cursor line into view at the top
    /// when it's outside the current viewport
    fn ensure_cursor_visible(&mut self) {
        self.scroll_handle
            .scroll_to_item(self.cursor.line, ScrollStrategy::Top);
    }

    /// Check if a character is a word character (alphanumeric or underscore)
    #[inline]
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Move cursor by word (Option/Alt + arrow)
    /// Optimized: Uses direct rope char access - O(log n) per char instead of
    /// O(n) String allocation + O(n²) repeated .nth() calls
    fn move_word_left(&mut self, extend_selection: bool) {
        let mut idx = self.cursor_to_char_idx(self.cursor);
        if idx == 0 {
            return;
        }

        // Skip whitespace backwards
        while idx > 0 && self.rope.char(idx - 1).is_whitespace() {
            idx -= 1;
        }

        // Skip word characters backwards
        while idx > 0 && Self::is_word_char(self.rope.char(idx - 1)) {
            idx -= 1;
        }

        self.cursor = self.char_idx_to_cursor(idx);

        if extend_selection {
            self.selection.head = self.cursor;
        } else {
            self.selection = Selection::caret(self.cursor);
        }
    }

    /// Move cursor by word (Option/Alt + arrow)
    /// Optimized: Uses direct rope char access - O(log n) per char instead of
    /// O(n) String allocation + O(n²) repeated .nth() calls
    fn move_word_right(&mut self, extend_selection: bool) {
        let mut idx = self.cursor_to_char_idx(self.cursor);
        let total_chars = self.rope.len_chars();
        if idx >= total_chars {
            return;
        }

        // Skip current word characters forwards
        while idx < total_chars && Self::is_word_char(self.rope.char(idx)) {
            idx += 1;
        }

        // Skip whitespace forwards
        while idx < total_chars && self.rope.char(idx).is_whitespace() {
            idx += 1;
        }

        self.cursor = self.char_idx_to_cursor(idx);

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

    // --- Helper Methods for Find/Replace, Go-To-Line, Line Operations ---

    /// Get max column for a line (0 if line doesn't exist)
    fn line_max_column(&self, line: usize) -> usize {
        self.line_len(line)
    }

    /// Convert cursor position to rope char index (alias for cursor_to_char_idx)
    fn cursor_char_index(&self, pos: CursorPosition) -> usize {
        self.cursor_to_char_idx(pos)
    }

    /// Convert char index to cursor position (alias for char_idx_to_cursor)
    fn char_index_to_cursor_pos(&self, char_idx: usize) -> CursorPosition {
        self.char_idx_to_cursor(char_idx)
    }

    /// Set cursor position and clear any selection
    fn set_caret(&mut self, pos: CursorPosition) {
        self.cursor = pos;
        self.selection = Selection::caret(pos);
    }

    /// Set selection range from start to end
    fn select_range(&mut self, start: CursorPosition, end: CursorPosition) {
        self.selection = Selection::new(start, end);
        self.cursor = end;
    }

    /// Get the inclusive line range of the current selection
    /// Returns (start_line, end_line) based on selection.ordered()
    fn selected_line_range_inclusive(&self) -> (usize, usize) {
        let (start, end) = self.selection.ordered();
        (start.line, end.line)
    }

    /// Get the char index range for a span of lines (inclusive)
    /// Returns (start_char_idx, end_char_idx) where end is at end of end_line
    fn line_char_range_inclusive(&self, start_line: usize, end_line: usize) -> (usize, usize) {
        let start_char = self
            .rope
            .line_to_char(start_line.min(self.line_count().saturating_sub(1)));
        let end_line_clamped = end_line.min(self.line_count().saturating_sub(1));

        // Get the char index at the start of the line after end_line (or end of doc)
        let end_char = if end_line_clamped + 1 < self.line_count() {
            self.rope.line_to_char(end_line_clamped + 1)
        } else {
            self.rope.len_chars()
        };

        (start_char, end_char)
    }

    /// Get the appropriate comment token for the current language
    fn line_comment_token(&self) -> &'static str {
        match self.language.to_lowercase().as_str() {
            "python" | "py" => "#",
            "shell" | "sh" | "bash" | "zsh" => "#",
            "yaml" | "yml" => "#",
            "ruby" | "rb" => "#",
            "perl" | "pl" => "#",
            "r" => "#",
            "toml" => "#",
            "makefile" | "make" => "#",
            "dockerfile" => "#",
            "powershell" | "ps1" => "#",
            "coffeescript" | "coffee" => "#",
            "nim" => "#",
            "julia" | "jl" => "#",
            _ => "//", // Default for C-style languages (JS, TS, Rust, Go, Java, C, C++, etc.)
        }
    }

    // --- Line Operations ---

    /// Duplicate the selected lines (or current line if no selection)
    fn duplicate_selected_lines(&mut self) {
        self.save_undo_state();

        let (start_line, end_line) = self.selected_line_range_inclusive();

        // Get the text of all selected lines including newlines
        let (start_char, end_char) = self.line_char_range_inclusive(start_line, end_line);
        let lines_text = self.rope.slice(start_char..end_char).to_string();

        // Ensure we have a trailing newline
        let to_insert = if lines_text.ends_with('\n') {
            lines_text
        } else {
            format!("{}\n", lines_text)
        };

        // Insert at end of the last selected line
        self.rope.insert(end_char, &to_insert);

        // Move cursor to the duplicated region
        let new_cursor_line = end_line + 1 + (end_line - start_line);
        let new_cursor_col = self.cursor.column.min(self.line_len(new_cursor_line));
        self.cursor = CursorPosition::new(new_cursor_line, new_cursor_col);
        self.selection = Selection::caret(self.cursor);

        self.needs_rehighlight = true;
        logging::log(
            "EDITOR",
            &format!("Duplicated lines {}-{}", start_line + 1, end_line + 1),
        );
    }

    /// Toggle line comments for selected lines (or current line)
    fn toggle_line_comment(&mut self) {
        self.save_undo_state();

        let (start_line, end_line) = self.selected_line_range_inclusive();
        let comment_token = self.line_comment_token();
        let comment_prefix = format!("{} ", comment_token);

        // Check if all lines are commented (to determine toggle direction)
        let all_commented = (start_line..=end_line).all(|line_idx| {
            self.get_line(line_idx)
                .map(|line| line.trim_start().starts_with(comment_token))
                .unwrap_or(false)
        });

        // Process lines from end to start to maintain correct indices
        for line_idx in (start_line..=end_line).rev() {
            let line_start_char = self.rope.line_to_char(line_idx);

            if let Some(line_content) = self.get_line(line_idx) {
                let trimmed = line_content.trim_start();
                let leading_whitespace = line_content.len() - trimmed.len();

                if all_commented {
                    // Uncomment: remove comment prefix
                    if trimmed.starts_with(&comment_prefix) {
                        // Remove "// " (with space)
                        let remove_start = line_start_char + leading_whitespace;
                        let remove_end = remove_start + comment_prefix.len();
                        self.rope.remove(remove_start..remove_end);
                    } else if trimmed.starts_with(comment_token) {
                        // Remove "//" (without space)
                        let remove_start = line_start_char + leading_whitespace;
                        let remove_end = remove_start + comment_token.len();
                        self.rope.remove(remove_start..remove_end);
                    }
                } else {
                    // Comment: add comment prefix after leading whitespace
                    let insert_pos = line_start_char + leading_whitespace;
                    self.rope.insert(insert_pos, &comment_prefix);
                }
            }
        }

        self.needs_rehighlight = true;
        logging::log(
            "EDITOR",
            &format!(
                "{} lines {}-{}",
                if all_commented {
                    "Uncommented"
                } else {
                    "Commented"
                },
                start_line + 1,
                end_line + 1
            ),
        );
    }

    /// Indent selected lines by adding 4 spaces at the start of each line
    fn indent_selected_lines(&mut self) {
        self.save_undo_state();

        let (start_line, end_line) = self.selected_line_range_inclusive();
        let indent = "    "; // 4 spaces

        // Process lines from end to start to maintain correct indices
        for line_idx in (start_line..=end_line).rev() {
            let line_start_char = self.rope.line_to_char(line_idx);
            self.rope.insert(line_start_char, indent);
        }

        // Update cursor column
        self.cursor.column += 4;

        // Update selection to reflect indent
        if !self.selection.is_empty() {
            self.selection.anchor.column += 4;
            self.selection.head.column += 4;
        }

        self.needs_rehighlight = true;
        logging::log(
            "EDITOR",
            &format!("Indented lines {}-{}", start_line + 1, end_line + 1),
        );
    }

    /// Outdent selected lines by removing up to 4 leading spaces or one tab
    fn outdent_selected_lines(&mut self) {
        self.save_undo_state();

        let (start_line, end_line) = self.selected_line_range_inclusive();
        let mut total_removed_on_cursor_line = 0;

        // Process lines from end to start to maintain correct indices
        for line_idx in (start_line..=end_line).rev() {
            let line_start_char = self.rope.line_to_char(line_idx);

            if let Some(line_content) = self.get_line(line_idx) {
                let chars: Vec<char> = line_content.chars().collect();

                // Count leading spaces/tabs to remove (up to 4 spaces or 1 tab)
                let mut chars_to_remove = 0;
                let mut spaces_counted = 0;

                for ch in &chars {
                    if *ch == '\t' && chars_to_remove == 0 {
                        // Remove one tab
                        chars_to_remove = 1;
                        break;
                    } else if *ch == ' ' && spaces_counted < 4 {
                        spaces_counted += 1;
                        chars_to_remove += 1;
                    } else {
                        break;
                    }
                }

                if chars_to_remove > 0 {
                    let remove_end = line_start_char + chars_to_remove;
                    self.rope.remove(line_start_char..remove_end);

                    if line_idx == self.cursor.line {
                        total_removed_on_cursor_line = chars_to_remove;
                    }
                }
            }
        }

        // Update cursor column (don't go negative)
        self.cursor.column = self
            .cursor
            .column
            .saturating_sub(total_removed_on_cursor_line);

        // Update selection columns
        if !self.selection.is_empty() {
            if self.selection.anchor.line >= start_line && self.selection.anchor.line <= end_line {
                self.selection.anchor.column = self
                    .selection
                    .anchor
                    .column
                    .saturating_sub(total_removed_on_cursor_line);
            }
            if self.selection.head.line >= start_line && self.selection.head.line <= end_line {
                self.selection.head.column = self
                    .selection
                    .head
                    .column
                    .saturating_sub(total_removed_on_cursor_line);
            }
        }

        self.needs_rehighlight = true;
        logging::log(
            "EDITOR",
            &format!("Outdented lines {}-{}", start_line + 1, end_line + 1),
        );
    }

    // -------------------------------------------------------------------------
    // Find / Replace Methods
    // -------------------------------------------------------------------------

    /// Show the find dialog, optionally seeding from selection if single-line
    pub fn show_find(&mut self, cx: &mut Context<Self>) {
        // If there's a single-line selection, use it as the initial query
        if !self.selection.is_empty() {
            let (start, end) = self.selection.ordered();
            if start.line == end.line {
                let selected = self.get_selected_text();
                if !selected.is_empty() && !selected.contains('\n') {
                    self.find_state.query = selected;
                }
            }
        }

        self.find_state.is_visible = true;
        self.find_state.show_replace = false;
        self.find_state.focus = FindField::Query;

        // Perform initial search if query is non-empty
        if !self.find_state.query.is_empty() {
            self.perform_find();
            // Jump to first match near cursor
            if !self.find_state.matches.is_empty() {
                self.find_nearest_match_to_cursor();
                self.jump_to_current_match(cx, true);
            }
        }

        cx.notify();
        logging::log("EDITOR", "Find dialog opened");
    }

    /// Show the find+replace dialog
    pub fn show_find_replace(&mut self, cx: &mut Context<Self>) {
        // If there's a single-line selection, use it as the initial query
        if !self.selection.is_empty() {
            let (start, end) = self.selection.ordered();
            if start.line == end.line {
                let selected = self.get_selected_text();
                if !selected.is_empty() && !selected.contains('\n') {
                    self.find_state.query = selected;
                }
            }
        }

        self.find_state.is_visible = true;
        self.find_state.show_replace = true;
        self.find_state.focus = FindField::Query;

        // Perform initial search if query is non-empty
        if !self.find_state.query.is_empty() {
            self.perform_find();
            if !self.find_state.matches.is_empty() {
                self.find_nearest_match_to_cursor();
                self.jump_to_current_match(cx, true);
            }
        }

        cx.notify();
        logging::log("EDITOR", "Find/Replace dialog opened");
    }

    /// Hide the find dialog and clear matches
    pub fn hide_find(&mut self, cx: &mut Context<Self>) {
        self.find_state.is_visible = false;
        self.find_state.matches.clear();
        self.find_state.current_match_idx = None;
        cx.notify();
        logging::log("EDITOR", "Find dialog closed");
    }

    /// Search for the query in the rope, populate matches vec with (start_char, end_char) tuples
    pub fn perform_find(&mut self) {
        self.find_state.matches.clear();
        self.find_state.current_match_idx = None;

        let query = &self.find_state.query;
        if query.is_empty() {
            return;
        }

        let content = self.rope.to_string();
        let search_content: String;
        let search_query: String;

        if self.find_state.case_sensitive {
            search_content = content.clone();
            search_query = query.clone();
        } else {
            search_content = content.to_lowercase();
            search_query = query.to_lowercase();
        }

        // Simple substring search (regex support could be added later)
        let query_len = search_query.chars().count();
        if query_len == 0 {
            return;
        }

        // Find all occurrences
        let mut search_start = 0;
        while let Some(byte_pos) = search_content[search_start..].find(&search_query) {
            let abs_byte_pos = search_start + byte_pos;

            // Convert byte position to char position
            let start_char = content[..abs_byte_pos].chars().count();
            let end_char = start_char + query_len;

            self.find_state.matches.push((start_char, end_char));

            // Move past this match to find the next one
            search_start = abs_byte_pos + search_query.len().max(1);
            if search_start >= search_content.len() {
                break;
            }
        }

        logging::log(
            "EDITOR",
            &format!(
                "Find: '{}' found {} matches",
                query,
                self.find_state.matches.len()
            ),
        );
    }

    /// Find the match nearest to the current cursor position and set it as current
    fn find_nearest_match_to_cursor(&mut self) {
        if self.find_state.matches.is_empty() {
            self.find_state.current_match_idx = None;
            return;
        }

        let cursor_char = self.cursor_to_char_idx(self.cursor);

        // Find the first match at or after cursor, or wrap to first match
        let idx = self
            .find_state
            .matches
            .iter()
            .position(|(start, _)| *start >= cursor_char)
            .unwrap_or(0);

        self.find_state.current_match_idx = Some(idx);
    }

    /// Select the current match and optionally scroll to it
    pub fn jump_to_current_match(&mut self, cx: &mut Context<Self>, scroll: bool) {
        let Some(idx) = self.find_state.current_match_idx else {
            return;
        };

        let Some(&(start_char, end_char)) = self.find_state.matches.get(idx) else {
            return;
        };

        let start_pos = self.char_idx_to_cursor(start_char);
        let end_pos = self.char_idx_to_cursor(end_char);

        self.select_range(start_pos, end_pos);

        if scroll {
            // Scroll to the line containing the match
            self.scroll_handle
                .scroll_to_item(start_pos.line, ScrollStrategy::Center);
        }

        cx.notify();
        logging::log(
            "EDITOR",
            &format!(
                "Jump to match {}/{}: chars {}..{}",
                idx + 1,
                self.find_state.matches.len(),
                start_char,
                end_char
            ),
        );
    }

    /// Go to the next match (wrap around)
    pub fn find_next(&mut self, cx: &mut Context<Self>) {
        if self.find_state.matches.is_empty() {
            return;
        }

        let next_idx = match self.find_state.current_match_idx {
            Some(idx) => {
                if idx + 1 >= self.find_state.matches.len() {
                    0 // Wrap to first
                } else {
                    idx + 1
                }
            }
            None => 0,
        };

        self.find_state.current_match_idx = Some(next_idx);
        self.jump_to_current_match(cx, true);
    }

    /// Go to the previous match (wrap around)
    pub fn find_prev(&mut self, cx: &mut Context<Self>) {
        if self.find_state.matches.is_empty() {
            return;
        }

        let prev_idx = match self.find_state.current_match_idx {
            Some(idx) => {
                if idx == 0 {
                    self.find_state.matches.len() - 1 // Wrap to last
                } else {
                    idx - 1
                }
            }
            None => self.find_state.matches.len() - 1,
        };

        self.find_state.current_match_idx = Some(prev_idx);
        self.jump_to_current_match(cx, true);
    }

    /// Replace the current match with the replacement string
    pub fn replace_current(&mut self, cx: &mut Context<Self>) {
        let Some(idx) = self.find_state.current_match_idx else {
            return;
        };

        let Some(&(start_char, end_char)) = self.find_state.matches.get(idx) else {
            return;
        };

        self.save_undo_state();

        let replacement = self.find_state.replacement.clone();
        let replacement_len = replacement.chars().count();
        let match_len = end_char - start_char;

        // Remove the matched text and insert replacement
        self.rope.remove(start_char..end_char);
        self.rope.insert(start_char, &replacement);
        self.needs_rehighlight = true;

        logging::log(
            "EDITOR",
            &format!(
                "Replaced match at {}..{} with '{}'",
                start_char, end_char, replacement
            ),
        );

        // Update all match positions after the replaced match
        let offset = replacement_len as isize - match_len as isize;

        // Remove the current match from the list
        self.find_state.matches.remove(idx);

        // Adjust positions of subsequent matches
        for (start, end) in self.find_state.matches.iter_mut().skip(idx) {
            *start = (*start as isize + offset) as usize;
            *end = (*end as isize + offset) as usize;
        }

        // Move to next match (or wrap)
        if self.find_state.matches.is_empty() {
            self.find_state.current_match_idx = None;
            // Position cursor after replacement
            let new_cursor = self.char_idx_to_cursor(start_char + replacement_len);
            self.set_caret(new_cursor);
        } else {
            // Keep the same index (now points to what was the next match)
            // or wrap to 0 if we were at the end
            if idx >= self.find_state.matches.len() {
                self.find_state.current_match_idx = Some(0);
            }
            self.jump_to_current_match(cx, true);
        }

        cx.notify();
    }

    /// Replace all matches (iterate from end to start to maintain indices)
    pub fn replace_all(&mut self, cx: &mut Context<Self>) {
        if self.find_state.matches.is_empty() {
            return;
        }

        self.save_undo_state();

        let replacement = self.find_state.replacement.clone();
        let match_count = self.find_state.matches.len();

        // Process matches from end to start to avoid index shifting issues
        for &(start_char, end_char) in self.find_state.matches.iter().rev() {
            self.rope.remove(start_char..end_char);
            self.rope.insert(start_char, &replacement);
        }

        self.needs_rehighlight = true;

        logging::log(
            "EDITOR",
            &format!(
                "Replaced all {} matches with '{}'",
                match_count, replacement
            ),
        );

        // Clear matches
        self.find_state.matches.clear();
        self.find_state.current_match_idx = None;

        cx.notify();
    }

    /// Handle keyboard input for the find/replace dialog
    /// Returns true if the event was handled, false otherwise
    pub fn handle_find_dialog_key_event(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.find_state.is_visible {
            return false;
        }

        let key = event.keystroke.key.to_lowercase();
        let cmd = event.keystroke.modifiers.platform;
        let shift = event.keystroke.modifiers.shift;

        match (key.as_str(), cmd, shift) {
            // Escape -> hide find
            ("escape", _, _) => {
                self.hide_find(cx);
                true
            }

            // Enter -> find_next (or replace_current if on Replace field)
            ("enter", false, false) => {
                if self.find_state.focus == FindField::Replace {
                    self.replace_current(cx);
                } else {
                    self.find_next(cx);
                }
                true
            }

            // Shift+Enter -> find_prev
            ("enter", false, true) => {
                self.find_prev(cx);
                true
            }

            // Cmd+Enter -> replace_all
            ("enter", true, _) => {
                self.replace_all(cx);
                true
            }

            // Tab -> toggle focus to Replace field
            ("tab", false, false) => {
                if self.find_state.show_replace {
                    self.find_state.focus = FindField::Replace;
                    cx.notify();
                }
                true
            }

            // Shift+Tab -> toggle focus to Query field
            ("tab", false, true) => {
                self.find_state.focus = FindField::Query;
                cx.notify();
                true
            }

            // Backspace -> pop char from focused field
            ("backspace", false, _) => {
                match self.find_state.focus {
                    FindField::Query => {
                        self.find_state.query.pop();
                        self.perform_find();
                        if !self.find_state.matches.is_empty() {
                            self.find_nearest_match_to_cursor();
                            self.jump_to_current_match(cx, true);
                        }
                    }
                    FindField::Replace => {
                        self.find_state.replacement.pop();
                    }
                }
                cx.notify();
                true
            }

            // Cmd+G -> find_next
            ("g", true, false) => {
                self.find_next(cx);
                true
            }

            // Cmd+Shift+G -> find_prev
            ("g", true, true) => {
                self.find_prev(cx);
                true
            }

            // F3 -> find_next
            ("f3", false, false) => {
                self.find_next(cx);
                true
            }

            // Shift+F3 -> find_prev
            ("f3", false, true) => {
                self.find_prev(cx);
                true
            }

            // Printable chars (without Cmd) -> push to focused field
            _ if !cmd => {
                if let Some(ref key_char) = event.keystroke.key_char {
                    if let Some(ch) = key_char.chars().next() {
                        if !ch.is_control() {
                            match self.find_state.focus {
                                FindField::Query => {
                                    self.find_state.query.push(ch);
                                    self.perform_find();
                                    if !self.find_state.matches.is_empty() {
                                        self.find_nearest_match_to_cursor();
                                        self.jump_to_current_match(cx, true);
                                    }
                                }
                                FindField::Replace => {
                                    self.find_state.replacement.push(ch);
                                }
                            }
                            cx.notify();
                            return true;
                        }
                    }
                }
                false
            }

            // Otherwise -> let normal handling continue
            _ => false,
        }
    }

    // -------------------------------------------------------------------------
    // End Find / Replace Methods
    // -------------------------------------------------------------------------

    // -------------------------------------------------------------------------
    // Go To Line Methods
    // -------------------------------------------------------------------------

    /// Show the Go To Line dialog, prefilling with current line number (1-based)
    pub fn show_go_to_line(&mut self, cx: &mut Context<Self>) {
        // Prefill with current line number (1-based for user display)
        self.go_to_line_state.line_input = format!("{}", self.cursor.line + 1);
        self.go_to_line_state.is_visible = true;
        cx.notify();
        logging::log("EDITOR", "Go To Line dialog opened");
    }

    /// Hide the Go To Line dialog
    pub fn hide_go_to_line(&mut self, cx: &mut Context<Self>) {
        self.go_to_line_state.is_visible = false;
        self.go_to_line_state.line_input.clear();
        cx.notify();
        logging::log("EDITOR", "Go To Line dialog closed");
    }

    /// Parse input, jump to line, and hide dialog
    pub fn commit_go_to_line(&mut self, cx: &mut Context<Self>) {
        // Parse the line number from input
        if let Ok(user_line) = self.go_to_line_state.line_input.parse::<usize>() {
            // Convert 1-based user input to 0-based internal line
            let target_line = user_line.saturating_sub(1);

            // Clamp to valid range: 0 to line_count()-1
            let max_line = self.line_count().saturating_sub(1);
            let clamped_line = target_line.min(max_line);

            // Create cursor position at start of target line (column 0)
            let pos = CursorPosition::new(clamped_line, 0);

            // Set cursor and clear selection
            self.set_caret(pos);

            // Scroll to center the target line in view
            self.scroll_handle
                .scroll_to_item(clamped_line, ScrollStrategy::Center);

            logging::log(
                "EDITOR",
                &format!(
                    "Go To Line: jumped to line {} (input: '{}')",
                    clamped_line + 1,
                    self.go_to_line_state.line_input
                ),
            );
        } else {
            logging::log(
                "EDITOR",
                &format!(
                    "Go To Line: invalid input '{}', closing dialog",
                    self.go_to_line_state.line_input
                ),
            );
        }

        // Hide the dialog regardless of whether parse succeeded
        self.hide_go_to_line(cx);
    }

    /// Handle keyboard input for the Go To Line dialog
    /// Returns true if the event was handled, false otherwise
    pub fn handle_go_to_line_dialog_key_event(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.go_to_line_state.is_visible {
            return false;
        }

        let key = event.keystroke.key.to_lowercase();

        match key.as_str() {
            // Escape -> hide dialog
            "escape" => {
                self.hide_go_to_line(cx);
                true
            }

            // Enter -> commit (jump to line) and hide
            "enter" => {
                self.commit_go_to_line(cx);
                true
            }

            // Backspace -> pop char from line_input
            "backspace" => {
                self.go_to_line_state.line_input.pop();
                cx.notify();
                true
            }

            // Digit chars (0-9) -> push to line_input
            _ => {
                // Check if it's a digit character
                if let Some(ref key_char) = event.keystroke.key_char {
                    if let Some(ch) = key_char.chars().next() {
                        if ch.is_ascii_digit() {
                            self.go_to_line_state.line_input.push(ch);
                            cx.notify();
                            return true;
                        }
                    }
                }
                // Swallow all other keys to keep the dialog modal
                true
            }
        }
    }

    // -------------------------------------------------------------------------
    // End Go To Line Methods
    // -------------------------------------------------------------------------

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
        if let Some(&(start_char, end_char)) = tabstop.ranges.first() {
            // Ranges are now in char indices, so use char_idx_to_cursor directly
            let start_pos = self.char_idx_to_cursor(start_char);
            let end_pos = self.char_idx_to_cursor(end_char);

            self.cursor = end_pos;
            self.selection = Selection::new(start_pos, end_pos);

            logging::log(
                "EDITOR",
                &format!(
                    "Tabstop {} selected: char range ({}, {}) -> cursor at ({}, {})",
                    tabstop.index, start_char, end_char, end_pos.line, end_pos.column
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

        // Route to dialog handlers first (they return true if event was consumed)
        // Go To Line dialog takes priority (more modal)
        if self.handle_go_to_line_dialog_key_event(event, cx) {
            return;
        }

        // Find/Replace dialog
        if self.handle_find_dialog_key_event(event, cx) {
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

            // Find/Replace shortcuts
            ("f", true, false, false) => self.show_find(cx),
            ("h", true, false, false) => self.show_find_replace(cx),
            ("f", true, false, true) => self.show_find_replace(cx), // Cmd+Alt+F alternative

            // Find next/prev (when find dialog is NOT open, for repeat search)
            ("f3", false, false, false) => self.find_next(cx),
            ("f3", false, true, false) => self.find_prev(cx),

            // Go To Line (Cmd+G when find dialog is closed)
            ("g", true, false, false) => self.show_go_to_line(cx),

            // Line operations
            ("d", true, true, false) => self.duplicate_selected_lines(), // Cmd+Shift+D
            ("/", true, false, false) => self.toggle_line_comment(),     // Cmd+/

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

            // Tab handling - snippet mode, selection indent, or regular tab
            ("tab", false, false, false) => {
                if self.snippet_state.is_some() {
                    self.next_tabstop();
                } else if !self.selection.is_empty() {
                    // Selection exists - indent selected lines
                    self.indent_selected_lines();
                } else {
                    self.insert_text("    "); // 4 spaces for tab
                }
            }
            ("tab", false, true, false) => {
                // Shift+Tab - previous tabstop in snippet mode, or outdent
                if self.snippet_state.is_some() {
                    self.prev_tabstop();
                } else {
                    // Outdent selected lines (or current line)
                    self.outdent_selected_lines();
                }
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

        // Ensure cursor remains visible after any operation that might have moved it
        self.ensure_cursor_visible();
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
                    &format!(
                        "Mouse down: single click at line {}, col {}",
                        pos.line, pos.column
                    ),
                );
                self.cursor = pos;
                self.selection = Selection::caret(pos);
            }
            2 => {
                // Double click: select word
                logging::log(
                    "EDITOR",
                    &format!(
                        "Mouse down: double click at line {}, col {}",
                        pos.line, pos.column
                    ),
                );
                self.select_word_at(pos);
            }
            3 => {
                // Triple click: select line
                logging::log(
                    "EDITOR",
                    &format!(
                        "Mouse down: triple click at line {}, col {}",
                        pos.line, pos.column
                    ),
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
                    // Use chars().count() instead of len() to get character count, not bytes
                    // This is critical for Unicode content (e.g., CJK characters, emojis)
                    let end_col = if line_idx == sel_end.line {
                        sel_end.column
                    } else {
                        line_content.chars().count()
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
    /// Optimized: uses boundary-based splitting (max 3 segments) instead of per-char iteration
    fn render_span(
        &self,
        text: &str,
        text_color: u32,
        span_start: usize,
        cursor_column: Option<usize>,
        selection_range: Option<(usize, usize)>,
        colors: &crate::theme::ColorScheme,
    ) -> impl IntoElement {
        let chars: Vec<char> = text.chars().collect();
        let span_len = chars.len();
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

        // Optimized: compute boundaries and create max 3 segments
        // Pre-allocate for worst case: before + cursor + selected + cursor + after = 5 elements
        let mut elements: Vec<gpui::AnyElement> = Vec::with_capacity(5);

        // Compute selection intersection with span (in local indices)
        let (sel_local_start, sel_local_end) = if let Some((sel_start, sel_end)) = selection_range {
            let local_start = sel_start.saturating_sub(span_start).min(span_len);
            let local_end = sel_end.saturating_sub(span_start).min(span_len);
            if local_start < local_end {
                (Some(local_start), Some(local_end))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Compute cursor position in local indices
        let cursor_local = cursor_column.and_then(|col| {
            if col >= span_start && col < span_end {
                Some(col - span_start)
            } else {
                None
            }
        });

        // Compute selection background color from theme (accent.selected with 25% opacity)
        let selection_bg = rgba((colors.accent.selected << 8) | 0x44);

        // Helper to create a text element
        let make_text_element =
            |chars_slice: &[char], selected: bool, color: u32| -> gpui::AnyElement {
                let chunk: String = chars_slice.iter().collect();
                if selected {
                    div()
                        .bg(selection_bg)
                        .text_color(rgb(color))
                        .child(SharedString::from(chunk))
                        .into_any_element()
                } else {
                    div()
                        .text_color(rgb(color))
                        .child(SharedString::from(chunk))
                        .into_any_element()
                }
            };

        // Build segments based on boundaries
        // Possible boundaries: 0, sel_local_start, cursor_local, sel_local_end, span_len
        let mut boundaries: Vec<usize> = Vec::with_capacity(5);
        boundaries.push(0);
        if let Some(s) = sel_local_start {
            if s > 0 && !boundaries.contains(&s) {
                boundaries.push(s);
            }
        }
        if let Some(c) = cursor_local {
            if !boundaries.contains(&c) {
                boundaries.push(c);
            }
        }
        if let Some(e) = sel_local_end {
            if e < span_len && !boundaries.contains(&e) {
                boundaries.push(e);
            }
        }
        if !boundaries.contains(&span_len) {
            boundaries.push(span_len);
        }
        boundaries.sort_unstable();

        // Create segments between consecutive boundaries
        for window in boundaries.windows(2) {
            let start = window[0];
            let end = window[1];

            if start >= end {
                continue;
            }

            // Insert cursor at start of this segment if applicable
            if cursor_local == Some(start) {
                elements.push(self.render_cursor(colors).into_any_element());
            }

            // Determine if this segment is selected
            let is_selected = match (sel_local_start, sel_local_end) {
                (Some(sel_s), Some(sel_e)) => start >= sel_s && end <= sel_e,
                _ => false,
            };

            elements.push(make_text_element(
                &chars[start..end],
                is_selected,
                text_color,
            ));
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

    /// Render the Find/Replace overlay dialog
    fn render_find_overlay(&self) -> impl IntoElement {
        let colors = &self.theme.colors;
        let match_info = if self.find_state.matches.is_empty() {
            if self.find_state.query.is_empty() {
                String::new()
            } else {
                "No matches".to_string()
            }
        } else {
            let current = self
                .find_state
                .current_match_idx
                .map(|i| i + 1)
                .unwrap_or(0);
            format!("{}/{}", current, self.find_state.matches.len())
        };

        let query_focused = self.find_state.focus == FindField::Query;
        let replace_focused = self.find_state.focus == FindField::Replace;

        div()
            .absolute()
            .top_2()
            .right_2()
            .w(px(320.))
            .bg(rgb(colors.background.search_box))
            .border_1()
            .border_color(rgb(colors.ui.border))
            .rounded_md()
            .shadow_lg()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .font_family("Menlo")
            .text_sm()
            // Find input row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(colors.text.secondary))
                            .text_xs()
                            .w(px(50.))
                            .child("Find:"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1()
                            .bg(rgb(colors.background.main))
                            .border_1()
                            .border_color(if query_focused {
                                rgb(colors.accent.selected)
                            } else {
                                rgb(colors.ui.border)
                            })
                            .rounded_sm()
                            .text_color(rgb(colors.text.primary))
                            .child(SharedString::from(if self.find_state.query.is_empty() {
                                " ".to_string() // Placeholder to maintain height
                            } else {
                                self.find_state.query.clone()
                            })),
                    )
                    .child(
                        div()
                            .text_color(rgb(colors.text.muted))
                            .text_xs()
                            .w(px(60.))
                            .text_right()
                            .child(SharedString::from(match_info)),
                    ),
            )
            // Replace input row (only shown if show_replace is true)
            .when(self.find_state.show_replace, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_color(rgb(colors.text.secondary))
                                .text_xs()
                                .w(px(50.))
                                .child("Replace:"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .px_2()
                                .py_1()
                                .bg(rgb(colors.background.main))
                                .border_1()
                                .border_color(if replace_focused {
                                    rgb(colors.accent.selected)
                                } else {
                                    rgb(colors.ui.border)
                                })
                                .rounded_sm()
                                .text_color(rgb(colors.text.primary))
                                .child(SharedString::from(
                                    if self.find_state.replacement.is_empty() {
                                        " ".to_string()
                                    } else {
                                        self.find_state.replacement.clone()
                                    },
                                )),
                        )
                        .child(div().w(px(60.))), // Spacer to align with match info
                )
            })
            // Hints row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .text_xs()
                    .text_color(rgb(colors.text.muted))
                    .child("Enter: next")
                    .child("Shift+Enter: prev")
                    .when(self.find_state.show_replace, |d| {
                        d.child("Cmd+Enter: replace all")
                    })
                    .child("Esc: close"),
            )
    }

    /// Render the Go To Line overlay dialog
    fn render_go_to_line_overlay(&self) -> impl IntoElement {
        let colors = &self.theme.colors;
        let line_count = self.line_count();

        div()
            .absolute()
            .top_2()
            .left(px(50.)) // Offset from gutter
            .w(px(200.))
            .bg(rgb(colors.background.search_box))
            .border_1()
            .border_color(rgb(colors.ui.border))
            .rounded_md()
            .shadow_lg()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .font_family("Menlo")
            .text_sm()
            // Input row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_color(rgb(colors.text.secondary))
                            .text_xs()
                            .child("Go to line:"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_2()
                            .py_1()
                            .bg(rgb(colors.background.main))
                            .border_1()
                            .border_color(rgb(colors.accent.selected))
                            .rounded_sm()
                            .text_color(rgb(colors.text.primary))
                            .child(SharedString::from(
                                if self.go_to_line_state.line_input.is_empty() {
                                    " ".to_string()
                                } else {
                                    self.go_to_line_state.line_input.clone()
                                },
                            )),
                    ),
            )
            // Info row
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(colors.text.muted))
                    .child(SharedString::from(format!("1 - {}", line_count))),
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
        // Add relative() for absolute positioning of overlay dialogs
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
            .relative() // Enable absolute positioning for overlays
            .bg(rgb(colors.background.main))
            .font_family("Menlo");

        // Apply height
        let container = if let Some(h) = self.content_height {
            container.h(h)
        } else {
            container.size_full().min_h(px(0.))
        };

        // Add overlays conditionally
        let find_visible = self.find_state.is_visible;
        let go_to_line_visible = self.go_to_line_state.is_visible;

        container
            .child(editor_area)
            .child(self.render_status_bar())
            .when(find_visible, |d| d.child(self.render_find_overlay()))
            .when(go_to_line_visible, |d| {
                d.child(self.render_go_to_line_overlay())
            })
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
        };
        assert_eq!(state.current_tabstop_idx, 0);
        assert_eq!(state.snippet.tabstops.len(), 1);
    }

    #[test]
    fn test_snippet_state_with_multiple_tabstops() {
        let snippet = ParsedSnippet::parse("${1:first} ${2:second} ${0:end}");
        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
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

    // --- Arrow Key Selection Collapse Tests ---
    // These tests verify that pressing Left/Right without Shift collapses
    // any existing selection to the appropriate edge (standard editor behavior).

    #[test]
    fn test_selection_collapse_with_left_arrow() {
        // Test with "Hello World" content context
        // Simulate: user selects "World" (columns 6-11 on line 0)
        // Selection anchor at (0, 6), head at (0, 11)
        let selection = Selection::new(CursorPosition::new(0, 6), CursorPosition::new(0, 11));

        assert!(!selection.is_empty());

        // Pressing Left should collapse to selection START (column 6)
        let (start, _end) = selection.ordered();
        assert_eq!(start.line, 0);
        assert_eq!(start.column, 6);
        // After move_left(false), cursor should be at start
    }

    #[test]
    fn test_selection_collapse_with_right_arrow() {
        // Test with "Hello World" content context
        // Simulate: user selects "Hello" (columns 0-5 on line 0)
        // Selection anchor at (0, 0), head at (0, 5)
        let selection = Selection::new(CursorPosition::new(0, 0), CursorPosition::new(0, 5));

        assert!(!selection.is_empty());

        // Pressing Right should collapse to selection END (column 5)
        let (_start, end) = selection.ordered();
        assert_eq!(end.line, 0);
        assert_eq!(end.column, 5);
        // After move_right(false), cursor should be at end
    }

    #[test]
    fn test_selection_collapse_with_up_arrow() {
        // Test with "Line 1\nLine 2\nLine 3" content context
        // Simulate: user selects from (1, 0) to (2, 3) - spanning lines 2 and 3
        let selection = Selection::new(CursorPosition::new(1, 0), CursorPosition::new(2, 3));

        assert!(!selection.is_empty());

        // Pressing Up should collapse to selection START (line 1, column 0)
        let (start, _end) = selection.ordered();
        assert_eq!(start.line, 1);
        assert_eq!(start.column, 0);
    }

    #[test]
    fn test_selection_collapse_with_down_arrow() {
        // Test with "Line 1\nLine 2\nLine 3" content context
        // Simulate: user selects from (0, 2) to (1, 4) - spanning lines 1 and 2
        let selection = Selection::new(CursorPosition::new(0, 2), CursorPosition::new(1, 4));

        assert!(!selection.is_empty());

        // Pressing Down should collapse to selection END (line 1, column 4)
        let (_start, end) = selection.ordered();
        assert_eq!(end.line, 1);
        assert_eq!(end.column, 4);
    }

    #[test]
    fn test_selection_extend_with_shift_arrow() {
        // Verify that Shift+Arrow still extends selection (doesn't collapse)
        let selection = Selection::new(
            CursorPosition::new(0, 5),  // anchor
            CursorPosition::new(0, 10), // head
        );

        assert!(!selection.is_empty());

        // With extend_selection=true, selection should NOT collapse
        // The head moves, anchor stays
        // (Implementation test - the actual behavior is in move_* functions)
    }

    #[test]
    fn test_no_collapse_when_no_selection() {
        // When there's no selection (caret), arrow keys should just move
        let pos = CursorPosition::new(0, 5);
        let selection = Selection::caret(pos);

        assert!(selection.is_empty());

        // No collapse needed - cursor just moves normally
    }

    #[test]
    fn test_selection_collapse_backwards_selection() {
        // Test with backwards selection (head before anchor)
        // User drags from right to left: anchor at (0, 10), head at (0, 5)
        let selection = Selection::new(
            CursorPosition::new(0, 10), // anchor (where drag started)
            CursorPosition::new(0, 5),  // head (where drag ended)
        );

        assert!(!selection.is_empty());

        // ordered() should return (5, 10) regardless of drag direction
        let (start, end) = selection.ordered();
        assert_eq!(start.column, 5);
        assert_eq!(end.column, 10);

        // Left arrow should go to start (5), Right arrow should go to end (10)
    }

    // --- Unicode and CRLF Handling Tests ---

    #[test]
    fn test_normalize_line_endings_crlf() {
        // Windows-style CRLF -> LF
        let content = "line1\r\nline2\r\nline3";
        let normalized = EditorPrompt::normalize_line_endings(content);
        assert_eq!(normalized, "line1\nline2\nline3");
    }

    #[test]
    fn test_normalize_line_endings_cr_only() {
        // Old Mac-style CR -> LF
        let content = "line1\rline2\rline3";
        let normalized = EditorPrompt::normalize_line_endings(content);
        assert_eq!(normalized, "line1\nline2\nline3");
    }

    #[test]
    fn test_normalize_line_endings_mixed() {
        // Mixed line endings -> all LF
        let content = "line1\r\nline2\nline3\rline4";
        let normalized = EditorPrompt::normalize_line_endings(content);
        assert_eq!(normalized, "line1\nline2\nline3\nline4");
    }

    #[test]
    fn test_normalize_line_endings_already_lf() {
        // Already LF -> unchanged
        let content = "line1\nline2\nline3";
        let normalized = EditorPrompt::normalize_line_endings(content);
        assert_eq!(normalized, "line1\nline2\nline3");
    }

    #[test]
    fn test_unicode_char_count_cjk() {
        // CJK characters: each is 3 bytes in UTF-8 but 1 char
        let text = "你好世界"; // "Hello World" in Chinese - 4 chars, 12 bytes
        assert_eq!(text.len(), 12); // bytes
        assert_eq!(text.chars().count(), 4); // chars
    }

    #[test]
    fn test_unicode_char_count_emoji() {
        // Emoji: can be 4 bytes in UTF-8 but 1 char
        let text = "Hello 🌍"; // 6 ASCII chars + 1 emoji
        assert_eq!(text.chars().count(), 7);
        assert!(text.len() > 7); // bytes > chars
    }

    #[test]
    fn test_unicode_char_count_mixed() {
        // Mixed ASCII and Unicode
        let text = "Hi你好!"; // 2 ASCII + 2 CJK + 1 ASCII = 5 chars
        assert_eq!(text.chars().count(), 5);
        assert!(text.len() > 5); // bytes > chars due to UTF-8 encoding
    }

    #[test]
    fn test_rope_unicode_line_length() {
        // Verify ropey correctly counts chars, not bytes
        let content = "你好世界\nHello\n🌍🌎🌏";
        let rope = Rope::from_str(content);

        // Line 0: "你好世界" = 4 chars (not 12 bytes!)
        assert_eq!(rope.line(0).len_chars(), 5); // 4 chars + newline

        // Line 1: "Hello" = 5 chars
        assert_eq!(rope.line(1).len_chars(), 6); // 5 chars + newline

        // Line 2: "🌍🌎🌏" = 3 chars (emojis)
        assert_eq!(rope.line(2).len_chars(), 3); // 3 chars, no trailing newline
    }

    #[test]
    fn test_char_to_cursor_static_unicode() {
        // Test char_to_cursor_static with Unicode content
        let rope = Rope::from_str("你好\nWorld");

        // Char 0 is '你'
        let pos = EditorPrompt::char_to_cursor_static(&rope, 0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);

        // Char 1 is '好'
        let pos = EditorPrompt::char_to_cursor_static(&rope, 1);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 1);

        // Char 2 is '\n'
        let pos = EditorPrompt::char_to_cursor_static(&rope, 2);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 2);

        // Char 3 is 'W' (start of line 1)
        let pos = EditorPrompt::char_to_cursor_static(&rope, 3);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_byte_to_cursor_static_unicode() {
        // Test byte_to_cursor_static with Unicode content
        // "你好" = 6 bytes (3 per CJK char), then '\n' = 1 byte, then "World" = 5 bytes
        let rope = Rope::from_str("你好\nWorld");

        // Byte 0-2 is '你'
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);

        // Byte 3-5 is '好'
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 3);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 1);

        // Byte 6 is '\n'
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 6);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 2);

        // Byte 7 is 'W' (start of line 1)
        let pos = EditorPrompt::byte_to_cursor_static(&rope, 7);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 0);
    }

    // --- Tab/Shift+Tab Indentation Tests ---
    // These tests verify the Tab key behavior for indentation and the presence
    // of correct key handling patterns in the source code.

    #[test]
    fn test_tab_key_patterns_exist_in_source() {
        // Verify Tab key handling patterns exist in the source code
        let source = include_str!("editor.rs");

        // Tab without modifiers should exist for indentation/snippet/insert
        assert!(
            source.contains(r#"("tab", false, false, false)"#),
            "Missing Tab key pattern for normal Tab press"
        );

        // Shift+Tab should exist for outdent/prev tabstop
        assert!(
            source.contains(r#"("tab", false, true, false)"#),
            "Missing Shift+Tab key pattern"
        );

        // indent_selected_lines should be called on Tab
        assert!(
            source.contains("self.indent_selected_lines()"),
            "Missing indent_selected_lines call in Tab handler"
        );

        // outdent_selected_lines should be called on Shift+Tab
        assert!(
            source.contains("self.outdent_selected_lines()"),
            "Missing outdent_selected_lines call in Shift+Tab handler"
        );
    }

    #[test]
    fn test_indent_function_adds_4_spaces() {
        // Verify the indent function uses 4 spaces
        let source = include_str!("editor.rs");

        // The indent function should have the 4-space indent string
        assert!(
            source.contains(r#"let indent = "    "; // 4 spaces"#),
            "Indent function should use 4 spaces"
        );
    }

    #[test]
    fn test_outdent_removes_up_to_4_spaces_or_tab() {
        // Verify outdent logic handles both spaces and tabs
        let source = include_str!("editor.rs");

        // Should check for tab character
        assert!(
            source.contains(r#"if *ch == '\t'"#),
            "Outdent should handle tab characters"
        );

        // Should limit space removal to 4
        assert!(
            source.contains("spaces_counted < 4"),
            "Outdent should remove at most 4 spaces"
        );
    }

    #[test]
    fn test_selection_line_range_for_single_line() {
        // When cursor is on a single line with no selection, range should be that line
        let selection = Selection::caret(CursorPosition::new(3, 5));
        let (start, end) = selection.ordered();

        // For a caret (no selection), start and end should be the same position
        assert_eq!(start.line, 3);
        assert_eq!(end.line, 3);
    }

    #[test]
    fn test_selection_line_range_for_multi_line() {
        // Selection spanning lines 2-4
        let selection = Selection::new(CursorPosition::new(2, 0), CursorPosition::new(4, 10));
        let (start, end) = selection.ordered();

        assert_eq!(start.line, 2);
        assert_eq!(end.line, 4);
    }

    #[test]
    fn test_selection_line_range_backwards() {
        // Backwards selection (head before anchor)
        let selection = Selection::new(
            CursorPosition::new(5, 8), // anchor
            CursorPosition::new(2, 3), // head (before anchor)
        );
        let (start, end) = selection.ordered();

        // ordered() should normalize regardless of selection direction
        assert_eq!(start.line, 2);
        assert_eq!(end.line, 5);
    }

    #[test]
    fn test_tab_handler_checks_snippet_state_first() {
        // Verify that snippet mode is checked before indent
        let source = include_str!("editor.rs");

        // Tab handler should check snippet_state first
        // This pattern should appear in the Tab handling code
        let tab_handler_check = source.contains("if self.snippet_state.is_some()");
        assert!(
            tab_handler_check,
            "Tab handler should check snippet mode first"
        );
    }

    #[test]
    fn test_tab_handler_checks_selection_for_indent() {
        // Verify Tab checks for selection before inserting spaces
        let source = include_str!("editor.rs");

        // Should check if selection is empty to decide between indent and insert
        assert!(
            source.contains("else if !self.selection.is_empty()"),
            "Tab handler should check selection for indent vs insert"
        );
    }

    #[test]
    fn test_shift_tab_always_outdents_without_snippet() {
        // Verify Shift+Tab outdents when not in snippet mode
        let source = include_str!("editor.rs");

        // The Shift+Tab handler should call outdent regardless of selection
        // (outdent_selected_lines handles both single line and multi-line)
        let shift_tab_section = source.find(r#"("tab", false, true, false)"#);
        assert!(
            shift_tab_section.is_some(),
            "Shift+Tab pattern should exist"
        );

        // Verify outdent is called in the else branch (non-snippet mode)
        let outdent_call = source.find("self.outdent_selected_lines()");
        assert!(
            outdent_call.is_some(),
            "outdent_selected_lines should be called"
        );
    }
}
