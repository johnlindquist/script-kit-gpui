//! EditorPrompt - Using gpui-component's Input in code_editor mode
//!
//! Full-featured code editor component using gpui-component which includes:
//! - High-performance editing (200K+ lines)
//! - Built-in Find/Replace with SearchPanel (Cmd+F)
//! - Syntax highlighting via Tree Sitter
//! - Undo/Redo with proper history
//! - Line numbers, soft wrap, indentation
//! - LSP hooks for diagnostics/completion
//! - Template/snippet support with tabstop navigation

use gpui::{
    div, prelude::*, px, rgb, Context, Entity, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Styled, Subscription, Window,
};
use gpui_component::input::{IndentInline, Input, InputEvent, InputState, OutdentInline, Position};
use std::sync::Arc;

use crate::config::Config;
use crate::logging;
use crate::snippet::ParsedSnippet;
use crate::theme::Theme;

/// Convert a character offset to a Position (line, column)
///
/// This is needed because gpui-component's InputState uses Position (line, column)
/// for cursor placement, but our snippet parser tracks char offsets.
#[allow(dead_code)]
fn char_offset_to_position(text: &str, char_offset: usize) -> Position {
    let mut line: u32 = 0;
    let mut col: u32 = 0;

    for (current_char, ch) in text.chars().enumerate() {
        if current_char >= char_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Position {
        line,
        character: col,
    }
}

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// Pending initialization state - stored until first render when window is available
struct PendingInit {
    content: String,
    language: String,
}

/// State for template/snippet navigation
///
/// Tracks the current position within a template's tabstops, allowing
/// Tab/Shift+Tab navigation through the placeholders.
#[derive(Debug, Clone)]
pub struct SnippetState {
    /// The parsed snippet with tabstop information
    pub snippet: ParsedSnippet,
    /// Current index into snippet.tabstops (0-based position in navigation order)
    pub current_tabstop_idx: usize,
    /// Current placeholder values (updated when user edits a tabstop)
    /// Index matches snippet.tabstops order
    pub current_values: Vec<String>,
    /// Tracks the last known selection range (char offsets) for each tabstop
    /// Used to detect when user has edited a tabstop and update current_values
    pub last_selection_ranges: Vec<Option<(usize, usize)>>,
}

/// State for the choice dropdown popup
/// Shown when a tabstop has multiple choices (${1|opt1,opt2,opt3|})
#[derive(Debug, Clone)]
pub struct ChoicesPopupState {
    /// The list of choices to display
    pub choices: Vec<String>,
    /// Currently highlighted index in the list
    pub selected_index: usize,
    /// The tabstop index this popup is for
    pub tabstop_idx: usize,
}

/// EditorPrompt - Full-featured code editor using gpui-component
///
/// Uses deferred initialization pattern: the InputState is created on first render
/// when the Window reference is available, not at construction time.
pub struct EditorPrompt {
    // Identity
    pub id: String,

    // gpui-component editor state (created on first render)
    editor_state: Option<Entity<InputState>>,

    // Pending initialization data (consumed on first render)
    pending_init: Option<PendingInit>,

    // Template/snippet state for tabstop navigation
    snippet_state: Option<SnippetState>,

    // Language for syntax highlighting (displayed in footer)
    language: String,

    // GPUI
    focus_handle: FocusHandle,
    on_submit: SubmitCallback,
    theme: Arc<Theme>,
    #[allow(dead_code)]
    config: Arc<Config>,

    // Layout - explicit height for proper sizing
    content_height: Option<gpui::Pixels>,

    // Subscriptions to keep alive
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,

    // When true, ignore all key events (used when actions panel is open)
    pub suppress_keys: bool,

    // Flag to request focus on next render (used for auto-focus after initialization)
    needs_focus: bool,

    // Flag to indicate we need to select the first tabstop after initialization
    needs_initial_tabstop_selection: bool,

    // Choice dropdown popup state (shown when tabstop has choices)
    choices_popup: Option<ChoicesPopupState>,
}

impl EditorPrompt {
    /// Create a new EditorPrompt with explicit height
    ///
    /// This is the compatible constructor that matches the original EditorPrompt API.
    /// The InputState is created lazily on first render when window is available.
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
                "EditorPrompt::with_height id={}, lang={}, content_len={}, height={:?}",
                id,
                language,
                content.len(),
                content_height
            ),
        );

        Self {
            id,
            editor_state: None, // Created on first render
            pending_init: Some(PendingInit {
                content,
                language: language.clone(),
            }),
            snippet_state: None,
            language,
            focus_handle,
            on_submit,
            theme,
            config,
            content_height,
            subscriptions: Vec::new(),
            suppress_keys: false,
            choices_popup: None,
            needs_focus: true, // Auto-focus on first render
            needs_initial_tabstop_selection: false,
        }
    }

    /// Create a new EditorPrompt in template/snippet mode
    ///
    /// Parses the template for VSCode-style tabstops and enables Tab/Shift+Tab navigation.
    /// Template syntax:
    /// - `$1`, `$2`, `$3` - Simple tabstops (numbered positions)
    /// - `${1:default}` - Tabstops with placeholder text
    /// - `${1|a,b,c|}` - Choice tabstops (first choice is used as default)
    /// - `$0` - Final cursor position
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

        // Parse the template for tabstops
        let snippet = ParsedSnippet::parse(&template);

        logging::log(
            "EDITOR",
            &format!(
                "Template parsed: {} tabstops, expanded_len={}",
                snippet.tabstops.len(),
                snippet.text.len()
            ),
        );

        // If there are tabstops, set up snippet state
        let (content, snippet_state, needs_initial_selection) = if snippet.tabstops.is_empty() {
            // No tabstops - use the expanded text as plain content
            (snippet.text.clone(), None, false)
        } else {
            // Has tabstops - set up navigation state
            // Initialize current_values with the original placeholder text
            let current_values: Vec<String> = snippet
                .tabstops
                .iter()
                .map(|ts| {
                    ts.placeholder
                        .clone()
                        .or_else(|| ts.choices.as_ref().and_then(|c| c.first().cloned()))
                        .unwrap_or_default()
                })
                .collect();

            // Initialize last_selection_ranges from the original ranges
            let last_selection_ranges: Vec<Option<(usize, usize)>> = snippet
                .tabstops
                .iter()
                .map(|ts| ts.ranges.first().copied())
                .collect();

            let state = SnippetState {
                snippet: snippet.clone(),
                current_tabstop_idx: 0, // Start at first tabstop
                current_values,
                last_selection_ranges,
            };
            (snippet.text.clone(), Some(state), true)
        };

        Self {
            id,
            editor_state: None, // Created on first render
            pending_init: Some(PendingInit {
                content,
                language: language.clone(),
            }),
            snippet_state,
            language,
            focus_handle,
            on_submit,
            theme,
            config,
            content_height,
            subscriptions: Vec::new(),
            suppress_keys: false,
            choices_popup: None,
            needs_focus: true, // Auto-focus on first render
            needs_initial_tabstop_selection: needs_initial_selection,
        }
    }

    /// Initialize the editor state (called on first render)
    fn ensure_initialized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_state.is_some() {
            return; // Already initialized
        }

        let Some(pending) = self.pending_init.take() else {
            logging::log("EDITOR", "Warning: No pending init data");
            return;
        };

        logging::log(
            "EDITOR",
            &format!(
                "Initializing editor state: lang={}, content_len={}",
                pending.language,
                pending.content.len()
            ),
        );

        // Create the gpui-component InputState in code_editor mode
        // Enable tab_navigation mode if we're in snippet mode (Tab moves between tabstops)
        let in_snippet = self.snippet_state.is_some();
        let editor_state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(&pending.language) // Sets up syntax highlighting
                .searchable(true) // Enable Cmd+F find/replace
                .line_number(false) // No line numbers - cleaner UI
                .soft_wrap(false) // Code should not wrap by default
                .default_value(pending.content)
                .tab_navigation(in_snippet) // Propagate Tab when in snippet mode
        });

        // Subscribe to editor changes
        let editor_sub = cx.subscribe_in(&editor_state, window, {
            move |_this, _, ev: &InputEvent, _window, cx| match ev {
                InputEvent::Change => {
                    cx.notify();
                }
                InputEvent::PressEnter { secondary: _ } => {
                    // Multi-line editor handles Enter internally for newlines
                }
                InputEvent::Focus => {
                    logging::log("EDITOR", "Editor focused");
                }
                InputEvent::Blur => {
                    logging::log("EDITOR", "Editor blurred");
                }
            }
        });

        self.subscriptions = vec![editor_sub];
        self.editor_state = Some(editor_state);

        logging::log("EDITOR", "Editor initialized, focus pending");
    }

    /// Get the current content as a String
    pub fn content(&self, cx: &Context<Self>) -> String {
        self.editor_state
            .as_ref()
            .map(|state| state.read(cx).value().to_string())
            .unwrap_or_else(|| {
                // Fall back to pending content if not yet initialized
                self.pending_init
                    .as_ref()
                    .map(|p| p.content.clone())
                    .unwrap_or_default()
            })
    }

    /// Get the language
    #[allow(dead_code)]
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Set the content
    #[allow(dead_code)]
    pub fn set_content(&mut self, content: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref editor_state) = self.editor_state {
            editor_state.update(cx, |state, cx| {
                state.set_value(content, window, cx);
            });
        } else {
            // Update pending content if not yet initialized
            if let Some(ref mut pending) = self.pending_init {
                pending.content = content;
            }
        }
    }

    /// Set the language for syntax highlighting
    #[allow(dead_code)]
    pub fn set_language(&mut self, language: String, cx: &mut Context<Self>) {
        self.language = language.clone();
        if let Some(ref editor_state) = self.editor_state {
            editor_state.update(cx, |state, cx| {
                state.set_highlighter(language, cx);
            });
        } else {
            // Update pending language if not yet initialized
            if let Some(ref mut pending) = self.pending_init {
                pending.language = language;
            }
        }
    }

    /// Set the content height (for dynamic resizing)
    #[allow(dead_code)]
    pub fn set_height(&mut self, height: gpui::Pixels) {
        self.content_height = Some(height);
    }

    // -------------------------------------------------------------------------
    // Snippet/Template Navigation
    // -------------------------------------------------------------------------

    /// Check if we're currently in snippet/template navigation mode
    pub fn in_snippet_mode(&self) -> bool {
        self.snippet_state.is_some()
    }

    /// Get the current tabstop index (0-based index into tabstops array)
    #[allow(dead_code)]
    pub fn current_tabstop_index(&self) -> Option<usize> {
        self.snippet_state.as_ref().map(|s| s.current_tabstop_idx)
    }

    /// Move to the next tabstop (public wrapper for testing via stdin commands)
    pub fn next_tabstop_public(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.next_tabstop(window, cx)
    }

    /// Move to the next tabstop. Returns true if we moved, false if we exited snippet mode.
    fn next_tabstop(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        logging::log("EDITOR", "next_tabstop called");

        // First, capture what the user typed at the current tabstop
        self.capture_current_tabstop_value(cx);

        let Some(ref mut state) = self.snippet_state else {
            logging::log("EDITOR", "next_tabstop: no snippet_state!");
            return false;
        };
        logging::log(
            "EDITOR",
            &format!(
                "next_tabstop: current_idx={}, total_tabstops={}",
                state.current_tabstop_idx,
                state.snippet.tabstops.len()
            ),
        );

        let tabstop_count = state.snippet.tabstops.len();
        if tabstop_count == 0 {
            self.exit_snippet_mode(window, cx);
            return false;
        }

        // Move to next tabstop
        let next_idx = state.current_tabstop_idx + 1;

        if next_idx >= tabstop_count {
            // We've gone past the last tabstop - check if there's a $0 final cursor
            let last_tabstop = &state.snippet.tabstops[tabstop_count - 1];
            if last_tabstop.index == 0 {
                // We were on the $0 tabstop, exit snippet mode
                logging::log("EDITOR", "Snippet: exiting after $0");
                self.exit_snippet_mode(window, cx);
                return false;
            } else {
                // No $0 tabstop - exit snippet mode
                logging::log("EDITOR", "Snippet: exiting after last tabstop");
                self.exit_snippet_mode(window, cx);
                return false;
            }
        }

        state.current_tabstop_idx = next_idx;
        logging::log(
            "EDITOR",
            &format!(
                "Snippet: moved to tabstop {} (index {})",
                state.snippet.tabstops[next_idx].index, next_idx
            ),
        );

        self.select_current_tabstop(window, cx);
        true
    }

    /// Move to the previous tabstop. Returns true if we moved, false if we're at the start.
    fn prev_tabstop(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // First, capture what the user typed at the current tabstop
        self.capture_current_tabstop_value(cx);

        let Some(ref mut state) = self.snippet_state else {
            return false;
        };

        if state.current_tabstop_idx == 0 {
            // Already at first tabstop
            return false;
        }

        state.current_tabstop_idx -= 1;
        logging::log(
            "EDITOR",
            &format!(
                "Snippet: moved to tabstop {} (index {})",
                state.snippet.tabstops[state.current_tabstop_idx].index, state.current_tabstop_idx
            ),
        );

        self.select_current_tabstop(window, cx);
        true
    }

    /// Capture the current tabstop's edited value before moving to another tabstop
    ///
    /// This is called before next_tabstop/prev_tabstop to record what the user typed,
    /// so we can calculate correct offsets for subsequent tabstops.
    ///
    /// The key insight: when the user types to replace a selected placeholder,
    /// the selection disappears and the cursor ends up at the end of what they typed.
    /// We need to read from the ORIGINAL start position of this tabstop to the
    /// current cursor position to capture what they actually typed.
    fn capture_current_tabstop_value(&mut self, cx: &mut Context<Self>) {
        // First, gather all the info we need with immutable borrows
        let (current_idx, tabstop_start_char, old_value) = {
            let Some(ref state) = self.snippet_state else {
                return;
            };
            let current_idx = state.current_tabstop_idx;
            if current_idx >= state.current_values.len() {
                return;
            }

            // Get the last known start position for this tabstop
            let tabstop_start_char = state
                .last_selection_ranges
                .get(current_idx)
                .and_then(|r| r.map(|(start, _)| start));

            let old_value = state.current_values[current_idx].clone();

            (current_idx, tabstop_start_char, old_value)
        };

        let Some(ref editor_state) = self.editor_state else {
            return;
        };

        // Get current editor state
        let (cursor_pos_char, selection_start_char, selection_end_char, full_text): (
            usize,
            usize,
            usize,
            String,
        ) = editor_state.update(cx, |input_state, _cx| {
            let selection = input_state.selection();
            let text = input_state.value();

            // Use cursor position (selection.end when collapsed, or end of selection)
            let cursor_byte = selection.end;
            let cursor_char = text
                .get(..cursor_byte)
                .map(|s| s.chars().count())
                .unwrap_or(0);

            let sel_start_char = text
                .get(..selection.start)
                .map(|s| s.chars().count())
                .unwrap_or(0);
            let sel_end_char = cursor_char;

            (cursor_char, sel_start_char, sel_end_char, text.to_string())
        });

        logging::log(
            "EDITOR",
            &format!(
                "Snippet capture: tabstop_idx={}, tabstop_start={:?}, cursor={}, selection=[{},{}), text='{}'",
                current_idx, tabstop_start_char, cursor_pos_char, selection_start_char, selection_end_char, full_text
            ),
        );

        // Determine the range to capture
        let (capture_start, capture_end) = if let Some(start) = tabstop_start_char {
            // We have a known start position - read from there to cursor
            // This handles the case where user typed to replace the placeholder
            (start, cursor_pos_char)
        } else {
            // Fallback: use original tabstop range adjusted for previous edits
            if let Some((adj_start, adj_end)) = self.calculate_adjusted_offset(current_idx) {
                (adj_start, adj_end)
            } else {
                return;
            }
        };

        // Extract the text at this range (convert char offsets to byte offsets)
        let captured_value: String = {
            let chars: Vec<char> = full_text.chars().collect();
            let start = capture_start.min(chars.len());
            let end = capture_end.min(chars.len());
            if start <= end {
                chars[start..end].iter().collect()
            } else {
                String::new()
            }
        };

        // Only update if we actually have something (could be empty if user deleted all)
        if captured_value != old_value {
            logging::log(
                "EDITOR",
                &format!(
                    "Snippet: captured tabstop {} value '{}' -> '{}' (range [{}, {}))",
                    current_idx, old_value, captured_value, capture_start, capture_end
                ),
            );
            // Now we can mutably borrow
            if let Some(ref mut state) = self.snippet_state {
                state.current_values[current_idx] = captured_value;
                state.last_selection_ranges[current_idx] = Some((capture_start, capture_end));
            }
        }
    }

    /// Calculate the adjusted offset for a tabstop based on edits to previous tabstops
    ///
    /// When a user edits tabstop 1 from "name" (4 chars) to "John Doe" (8 chars),
    /// tabstop 2's offset needs to shift by +4 characters.
    fn calculate_adjusted_offset(&self, tabstop_idx: usize) -> Option<(usize, usize)> {
        let state = self.snippet_state.as_ref()?;

        // Get the original range for this tabstop
        let original_range = state.snippet.tabstops.get(tabstop_idx)?.ranges.first()?;
        let (mut start, mut end) = *original_range;

        // Calculate cumulative offset adjustment from all previous tabstops
        for i in 0..tabstop_idx {
            let original_ts = state.snippet.tabstops.get(i)?;
            let original_placeholder = original_ts
                .placeholder
                .as_deref()
                .or_else(|| {
                    original_ts
                        .choices
                        .as_ref()
                        .and_then(|c| c.first().map(|s| s.as_str()))
                })
                .unwrap_or("");

            let current_value = state
                .current_values
                .get(i)
                .map(|s| s.as_str())
                .unwrap_or("");

            // Calculate the difference in character length
            let original_len = original_placeholder.chars().count();
            let current_len = current_value.chars().count();
            let diff = current_len as isize - original_len as isize;

            // Adjust if this tabstop was before our target (compare start positions)
            if let Some(&(ts_start, _)) = original_ts.ranges.first() {
                let (original_start, _) = *original_range;
                if ts_start < original_start {
                    start = (start as isize + diff).max(0) as usize;
                    end = (end as isize + diff).max(0) as usize;
                }
            }
        }

        Some((start, end))
    }

    /// Select the current tabstop placeholder text using gpui-component's set_selection API
    ///
    /// This method calculates the correct offset based on any edits the user has made
    /// to previous tabstops.
    fn select_current_tabstop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // First, calculate the adjusted offset (needs immutable borrow)
        let adjusted_range = self.calculate_adjusted_offset_for_current();
        let Some((start, end, tabstop_index)) = adjusted_range else {
            logging::log("EDITOR", "Snippet: could not calculate adjusted offset");
            return;
        };

        let Some(ref editor_state) = self.editor_state else {
            return;
        };

        logging::log(
            "EDITOR",
            &format!(
                "Snippet: selecting tabstop {} adjusted range [{}, {})",
                tabstop_index, start, end
            ),
        );

        // Use gpui-component's set_selection to select the tabstop text
        editor_state.update(cx, |input_state, cx| {
            let text = input_state.value();
            let text_len = text.chars().count();

            // Clamp to valid range
            let start_clamped = start.min(text_len);
            let end_clamped = end.min(text_len);

            // Convert char offsets to byte offsets
            let start_bytes = text
                .char_indices()
                .nth(start_clamped)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let end_bytes = text
                .char_indices()
                .nth(end_clamped)
                .map(|(i, _)| i)
                .unwrap_or(text.len());

            // Log what text we're actually selecting
            let selected_text = if start_bytes < end_bytes && end_bytes <= text.len() {
                &text[start_bytes..end_bytes]
            } else {
                ""
            };
            logging::log(
                "EDITOR",
                &format!(
                    "Snippet: setting selection bytes [{}, {}) = '{}' in text len={}, full_text='{}'",
                    start_bytes,
                    end_bytes,
                    selected_text,
                    text.len(),
                    text
                ),
            );

            input_state.set_selection(start_bytes, end_bytes, window, cx);
        });

        // Update the last selection range and check for choices
        if let Some(ref mut state) = self.snippet_state {
            let current_idx = state.current_tabstop_idx;
            if current_idx < state.last_selection_ranges.len() {
                state.last_selection_ranges[current_idx] = Some((start, end));
            }

            // Check if this tabstop has choices - if so, show the dropdown
            if let Some(tabstop) = state.snippet.tabstops.get(current_idx) {
                if let Some(ref choices) = tabstop.choices {
                    if choices.len() > 1 {
                        logging::log(
                            "EDITOR",
                            &format!(
                                "Snippet: tabstop {} has {} choices, showing popup",
                                current_idx,
                                choices.len()
                            ),
                        );
                        self.choices_popup = Some(ChoicesPopupState {
                            choices: choices.clone(),
                            selected_index: 0,
                            tabstop_idx: current_idx,
                        });
                    }
                }
            }
        }

        cx.notify();
    }

    /// Helper to calculate adjusted offset for the current tabstop
    /// Returns (start, end, tabstop_index) or None
    fn calculate_adjusted_offset_for_current(&self) -> Option<(usize, usize, usize)> {
        let state = self.snippet_state.as_ref()?;
        let current_idx = state.current_tabstop_idx;

        if current_idx >= state.snippet.tabstops.len() {
            return None;
        }

        let tabstop_index = state.snippet.tabstops[current_idx].index;
        let (start, end) = self.calculate_adjusted_offset(current_idx)?;
        Some((start, end, tabstop_index))
    }

    /// Exit snippet mode and restore normal Tab behavior
    fn exit_snippet_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.snippet_state.is_some() {
            logging::log("EDITOR", "Exiting snippet mode");
            self.snippet_state = None;

            // Disable tab navigation mode so Tab inserts tabs again
            if let Some(ref editor_state) = self.editor_state {
                editor_state.update(cx, |state, cx| {
                    state.set_tab_navigation(false, window, cx);
                });
            }
        }
    }

    /// Submit the current content
    fn submit(&self, cx: &Context<Self>) {
        let content = self.content(cx);
        logging::log("EDITOR", &format!("Submit id={}", self.id));
        (self.on_submit)(self.id.clone(), Some(content));
    }

    /// Cancel - submit None
    #[allow(dead_code)]
    fn cancel(&self) {
        logging::log("EDITOR", &format!("Cancel id={}", self.id));
        (self.on_submit)(self.id.clone(), None);
    }

    /// Focus the editor
    #[allow(dead_code)]
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref editor_state) = self.editor_state {
            editor_state.update(cx, |state, cx| {
                state.focus(window, cx);
            });
        }
    }

    /// Request focus on next render (useful when called outside of render context)
    #[allow(dead_code)]
    pub fn request_focus(&mut self) {
        self.needs_focus = true;
    }

    // === Choice Popup Methods ===

    /// Check if the choice popup is currently visible
    pub fn is_choice_popup_visible(&self) -> bool {
        self.choices_popup.is_some()
    }

    /// Public wrapper for choice_popup_up (for SimulateKey)
    pub fn choice_popup_up_public(&mut self, cx: &mut Context<Self>) {
        self.choice_popup_up(cx);
    }

    /// Public wrapper for choice_popup_down (for SimulateKey)
    pub fn choice_popup_down_public(&mut self, cx: &mut Context<Self>) {
        self.choice_popup_down(cx);
    }

    /// Public wrapper for choice_popup_confirm (for SimulateKey)
    pub fn choice_popup_confirm_public(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.choice_popup_confirm(window, cx);
    }

    /// Public wrapper for choice_popup_cancel (for SimulateKey)
    pub fn choice_popup_cancel_public(&mut self, cx: &mut Context<Self>) {
        self.choice_popup_cancel(cx);
    }

    /// Move selection up in the choice popup
    fn choice_popup_up(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut popup) = self.choices_popup {
            if popup.selected_index > 0 {
                popup.selected_index -= 1;
                logging::log(
                    "EDITOR",
                    &format!("Choice popup: moved up to index {}", popup.selected_index),
                );
                cx.notify();
            }
        }
    }

    /// Move selection down in the choice popup
    fn choice_popup_down(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut popup) = self.choices_popup {
            if popup.selected_index + 1 < popup.choices.len() {
                popup.selected_index += 1;
                logging::log(
                    "EDITOR",
                    &format!("Choice popup: moved down to index {}", popup.selected_index),
                );
                cx.notify();
            }
        }
    }

    /// Confirm the current choice and replace the selection
    fn choice_popup_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(popup) = self.choices_popup.take() else {
            return;
        };

        let Some(chosen) = popup.choices.get(popup.selected_index) else {
            return;
        };

        logging::log(
            "EDITOR",
            &format!(
                "Choice popup: confirmed '{}' at index {}",
                chosen, popup.selected_index
            ),
        );

        // Replace the current selection with the chosen text
        if let Some(ref editor_state) = self.editor_state {
            let chosen_clone = chosen.clone();
            editor_state.update(cx, |input_state, cx| {
                // The current tabstop text should be selected
                // Replace it with the chosen value
                input_state.insert(&chosen_clone, window, cx);
            });
        }

        // Update current_values for offset tracking
        if let Some(ref mut state) = self.snippet_state {
            if popup.tabstop_idx < state.current_values.len() {
                state.current_values[popup.tabstop_idx] = chosen.clone();
            }
        }

        cx.notify();
    }

    /// Cancel the choice popup (dismiss without changing selection)
    fn choice_popup_cancel(&mut self, cx: &mut Context<Self>) {
        if self.choices_popup.is_some() {
            logging::log("EDITOR", "Choice popup: cancelled");
            self.choices_popup = None;
            cx.notify();
        }
    }

    /// Render the choice popup overlay
    fn render_choices_popup(&self, _cx: &Context<Self>) -> Option<impl IntoElement> {
        let popup = self.choices_popup.as_ref()?;
        let colors = &self.theme.colors;

        Some(
            div()
                .absolute()
                .top(px(40.)) // Position below the editor toolbar area
                .left(px(16.))
                //.z_index(1000) // Not available in GPUI, using layer order instead
                .min_w(px(200.))
                .max_w(px(400.))
                .bg(rgb(colors.background.main))
                .border_1()
                .border_color(rgb(colors.ui.border))
                .rounded_md()
                .shadow_lg()
                .py(px(4.))
                .children(popup.choices.iter().enumerate().map(|(idx, choice)| {
                    let is_selected = idx == popup.selected_index;
                    let bg_color = if is_selected {
                        rgb(colors.accent.selected)
                    } else {
                        rgb(colors.background.main)
                    };
                    // Use contrasting text color for selected item
                    let text_color = if is_selected {
                        // White text on accent background for better contrast
                        rgb(0xffffff)
                    } else {
                        rgb(colors.text.primary)
                    };

                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(bg_color)
                        .text_color(text_color)
                        .text_sm()
                        .cursor_pointer()
                        .child(choice.clone())
                })),
        )
    }
}

impl Focusable for EditorPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EditorPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Ensure InputState is initialized on first render
        self.ensure_initialized(window, cx);

        // Handle deferred focus - focus the editor's InputState after initialization
        if self.needs_focus {
            if let Some(ref editor_state) = self.editor_state {
                editor_state.update(cx, |state, cx| {
                    state.focus(window, cx);
                });
                self.needs_focus = false;
                logging::log("EDITOR", "Editor focused via deferred focus");
            }
        }

        // Handle initial tabstop selection for templates
        if self.needs_initial_tabstop_selection && self.editor_state.is_some() {
            self.needs_initial_tabstop_selection = false;
            self.select_current_tabstop(window, cx);
            logging::log("EDITOR", "Initial tabstop selected");
        }

        let colors = &self.theme.colors;

        // Key handler for submit/cancel, snippet navigation, and choice popup
        // IMPORTANT: We intercept Tab here BEFORE gpui-component's Input processes it,
        // so we don't get tab characters inserted when navigating snippets.
        let handle_key = cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
            if this.suppress_keys {
                return;
            }

            let key = event.keystroke.key.to_lowercase();
            let cmd = event.keystroke.modifiers.platform;
            let shift = event.keystroke.modifiers.shift;

            // Debug logging for key events
            logging::log(
                "EDITOR",
                &format!(
                    "Key event: key='{}', cmd={}, shift={}, in_snippet_mode={}, choice_popup={}",
                    key,
                    cmd,
                    shift,
                    this.in_snippet_mode(),
                    this.is_choice_popup_visible()
                ),
            );

            // Handle choice popup keys first (takes priority)
            if this.is_choice_popup_visible() {
                match key.as_str() {
                    "up" | "arrowup" => {
                        this.choice_popup_up(cx);
                        return; // Don't propagate
                    }
                    "down" | "arrowdown" => {
                        this.choice_popup_down(cx);
                        return; // Don't propagate
                    }
                    "enter" if !cmd => {
                        this.choice_popup_confirm(window, cx);
                        return; // Don't propagate
                    }
                    "escape" => {
                        this.choice_popup_cancel(cx);
                        return; // Don't propagate
                    }
                    "tab" if !shift => {
                        // Tab confirms the choice and moves to next tabstop
                        this.choice_popup_confirm(window, cx);
                        this.next_tabstop(window, cx);
                        return; // Don't propagate
                    }
                    _ => {
                        // Other keys close the popup and propagate
                        this.choice_popup_cancel(cx);
                        // Fall through to normal handling
                    }
                }
            }

            match (key.as_str(), cmd, shift) {
                // Cmd+Enter submits
                ("enter", true, _) => {
                    this.submit(cx);
                    // Don't propagate - we handled it
                }
                // Cmd+S also submits (save)
                ("s", true, _) => {
                    this.submit(cx);
                    // Don't propagate - we handled it
                }
                // Tab - snippet navigation (when in snippet mode)
                ("tab", false, false) if this.in_snippet_mode() => {
                    logging::log(
                        "EDITOR",
                        "Tab pressed in snippet mode - calling next_tabstop",
                    );
                    this.next_tabstop(window, cx);
                    // Don't propagate - prevents tab character insertion
                }
                // Shift+Tab - snippet navigation backwards (when in snippet mode)
                ("tab", false, true) if this.in_snippet_mode() => {
                    this.prev_tabstop(window, cx);
                    // Don't propagate - prevents tab character insertion
                }
                // Escape - exit snippet mode or let parent handle
                ("escape", false, _) => {
                    if this.in_snippet_mode() {
                        this.exit_snippet_mode(window, cx);
                        cx.notify();
                        // Don't propagate when exiting snippet mode
                    } else {
                        // Let parent handle escape for closing the editor
                        cx.propagate();
                    }
                }
                _ => {
                    // Let other keys propagate to the Input component
                    cx.propagate();
                }
            }
        });

        // Calculate height
        let height = self.content_height.unwrap_or_else(|| px(500.)); // Default height if not specified

        // Get mono font family for code editor
        let fonts = self.theme.get_fonts();
        let mono_font: SharedString = fonts.mono_family.into();

        // Action handlers for snippet Tab navigation
        // GPUI actions bubble up from focused element to parents, but only if the
        // focused element calls cx.propagate(). Since gpui-component's Input handles
        // IndentInline without propagating, we need to intercept at the Input wrapper level.
        let handle_indent = cx.listener(|this, _: &IndentInline, window, cx| {
            logging::log(
                "EDITOR",
                &format!(
                    "IndentInline action received, in_snippet_mode={}",
                    this.in_snippet_mode()
                ),
            );
            if this.in_snippet_mode() {
                this.next_tabstop(window, cx);
                // Don't propagate - we handled it
            } else {
                cx.propagate(); // Let Input handle normal indent
            }
        });

        let handle_outdent = cx.listener(|this, _: &OutdentInline, window, cx| {
            logging::log(
                "EDITOR",
                &format!(
                    "OutdentInline action received, in_snippet_mode={}",
                    this.in_snippet_mode()
                ),
            );
            if this.in_snippet_mode() {
                this.prev_tabstop(window, cx);
                // Don't propagate - we handled it
            } else {
                cx.propagate(); // Let Input handle normal outdent
            }
        });

        // Build the main container - code editor fills the space completely
        // Note: We don't track focus on the container because the InputState
        // has its own focus handle. Key events will be handled by the Input.
        let mut container = div()
            .id("editor-v2")
            .flex()
            .flex_col()
            .w_full()
            .h(height)
            .bg(rgb(colors.background.main))
            .text_color(rgb(colors.text.primary))
            .font_family(mono_font) // Use monospace font for code
            .on_key_down(handle_key)
            .on_action(handle_indent)
            .on_action(handle_outdent);

        // Add the editor content if initialized
        if let Some(ref editor_state) = self.editor_state {
            container = container.child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
                    .overflow_hidden()
                    // No padding - editor fills the space completely
                    // The Input component from gpui-component
                    // appearance(false) removes border styling for seamless integration
                    .child(Input::new(editor_state).size_full().appearance(false)),
            );
        } else {
            // Show loading placeholder while initializing
            container = container.child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("Loading editor..."),
            );
        }

        // Footer with language indicator and snippet state
        let language_display: SharedString = self.language.clone().into();

        // Build snippet indicator if in snippet mode
        let snippet_indicator = if let Some(ref state) = self.snippet_state {
            let current = state.current_tabstop_idx + 1; // 1-based for display
            let total = state.snippet.tabstops.len();

            // Get the current tabstop's display name (placeholder or index)
            let current_name = state
                .snippet
                .tabstops
                .get(state.current_tabstop_idx)
                .and_then(|ts| {
                    ts.placeholder
                        .clone()
                        .or_else(|| ts.choices.as_ref().and_then(|c| c.first().cloned()))
                })
                .unwrap_or_else(|| format!("${}", current));

            Some(format!(
                "Tab {} of {} · \"{}\" · Tab to continue, Esc to exit",
                current, total, current_name
            ))
        } else {
            None
        };

        container = container.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between() // Space between left and right
                .w_full()
                .h(px(24.))
                .px(px(12.))
                .bg(rgb(colors.background.title_bar))
                .border_t_1()
                .border_color(rgb(colors.ui.border))
                // Left side: snippet indicator (if in snippet mode)
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(colors.accent.selected))
                        .when_some(snippet_indicator, |d, indicator| {
                            d.child(SharedString::from(indicator))
                        }),
                )
                // Right side: language indicator
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(colors.text.muted))
                        .child(language_display),
                ),
        );

        // Wrap in a relative container to support absolute positioning for the choices popup
        let mut wrapper = div().relative().w_full().h_full().child(container);

        // Add the choices popup overlay if visible
        if let Some(popup_element) = self.render_choices_popup(cx) {
            wrapper = wrapper.child(popup_element);
        }

        wrapper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_creation() {
        // Basic smoke test - just verify the struct can be created with expected fields
        // Full integration tests require GPUI context
    }

    #[test]
    fn test_char_offset_to_position_single_line() {
        let text = "Hello World";
        let pos0 = char_offset_to_position(text, 0);
        assert_eq!((pos0.line, pos0.character), (0, 0));

        let pos5 = char_offset_to_position(text, 5);
        assert_eq!((pos5.line, pos5.character), (0, 5));

        let pos11 = char_offset_to_position(text, 11);
        assert_eq!((pos11.line, pos11.character), (0, 11));
    }

    #[test]
    fn test_char_offset_to_position_multi_line() {
        let text = "Hello\nWorld\nTest";
        // Line 0: "Hello" (0-4), newline at 5
        // Line 1: "World" (6-10), newline at 11
        // Line 2: "Test" (12-15)
        let pos0 = char_offset_to_position(text, 0);
        assert_eq!((pos0.line, pos0.character), (0, 0)); // 'H'

        let pos5 = char_offset_to_position(text, 5);
        assert_eq!((pos5.line, pos5.character), (0, 5)); // '\n'

        let pos6 = char_offset_to_position(text, 6);
        assert_eq!((pos6.line, pos6.character), (1, 0)); // 'W'

        let pos11 = char_offset_to_position(text, 11);
        assert_eq!((pos11.line, pos11.character), (1, 5)); // '\n'

        let pos12 = char_offset_to_position(text, 12);
        assert_eq!((pos12.line, pos12.character), (2, 0)); // 'T'

        let pos16 = char_offset_to_position(text, 16);
        assert_eq!((pos16.line, pos16.character), (2, 4)); // past end
    }

    #[test]
    fn test_char_offset_to_position_empty() {
        let text = "";
        let pos = char_offset_to_position(text, 0);
        assert_eq!((pos.line, pos.character), (0, 0));
    }

    #[test]
    fn test_snippet_state_creation() {
        // Test that SnippetState is properly initialized from a template
        let snippet = ParsedSnippet::parse("Hello ${1:name}!");

        let current_values = vec!["name".to_string()];
        let last_selection_ranges = vec![Some((6, 10))];

        let state = SnippetState {
            snippet: snippet.clone(),
            current_tabstop_idx: 0,
            current_values,
            last_selection_ranges,
        };

        assert_eq!(state.current_tabstop_idx, 0);
        assert_eq!(state.snippet.tabstops.len(), 1);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.text, "Hello name!");
    }

    #[test]
    fn test_snippet_state_multiple_tabstops() {
        let snippet = ParsedSnippet::parse("Hello ${1:name}, welcome to ${2:place}!");

        let current_values = vec!["name".to_string(), "place".to_string()];
        let last_selection_ranges = vec![Some((6, 10)), Some((23, 28))];

        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
            current_values,
            last_selection_ranges,
        };

        assert_eq!(state.snippet.tabstops.len(), 2);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.tabstops[1].index, 2);
        assert_eq!(state.snippet.text, "Hello name, welcome to place!");
    }

    #[test]
    fn test_snippet_state_with_final_cursor() {
        let snippet = ParsedSnippet::parse("Hello ${1:name}!$0");

        let current_values = vec!["name".to_string(), "".to_string()];
        let last_selection_ranges = vec![Some((6, 10)), Some((11, 11))];

        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
            current_values,
            last_selection_ranges,
        };

        // Should have 2 tabstops: index 1 first, then index 0 ($0) at end
        assert_eq!(state.snippet.tabstops.len(), 2);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.tabstops[1].index, 0);
    }
}
