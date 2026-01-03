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
use gpui_component::input::{Input, InputEvent, InputState, Position};
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
            let state = SnippetState {
                snippet: snippet.clone(),
                current_tabstop_idx: 0, // Start at first tabstop
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
        let editor_state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(&pending.language) // Sets up syntax highlighting
                .searchable(true) // Enable Cmd+F find/replace
                .line_number(false) // No line numbers - cleaner UI
                .soft_wrap(false) // Code should not wrap by default
                .default_value(pending.content)
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

    /// Move to the next tabstop. Returns true if we moved, false if we exited snippet mode.
    fn next_tabstop(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(ref mut state) = self.snippet_state else {
            return false;
        };

        let tabstop_count = state.snippet.tabstops.len();
        if tabstop_count == 0 {
            self.exit_snippet_mode();
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
                self.exit_snippet_mode();
                return false;
            } else {
                // No $0 tabstop - exit snippet mode
                logging::log("EDITOR", "Snippet: exiting after last tabstop");
                self.exit_snippet_mode();
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

    /// Select the current tabstop placeholder text using gpui-component's set_selection API
    fn select_current_tabstop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref state) = self.snippet_state else {
            return;
        };
        let Some(ref editor_state) = self.editor_state else {
            return;
        };

        let tabstops = &state.snippet.tabstops;
        if state.current_tabstop_idx >= tabstops.len() {
            return;
        }

        let tabstop = &tabstops[state.current_tabstop_idx];

        // Get the first range for this tabstop (linked tabstops have multiple ranges)
        if let Some(&(start, end)) = tabstop.ranges.first() {
            logging::log(
                "EDITOR",
                &format!(
                    "Snippet: selecting tabstop {} range [{}, {})",
                    tabstop.index, start, end
                ),
            );

            // Use gpui-component's set_selection to select the tabstop text
            editor_state.update(cx, |input_state, cx| {
                let text = input_state.value();
                // Convert char offsets to byte offsets
                let start_bytes = text.char_indices().nth(start).map(|(i, _)| i).unwrap_or(0);
                let end_bytes = text
                    .char_indices()
                    .nth(end)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());

                input_state.set_selection(start_bytes, end_bytes, window, cx);
            });

            cx.notify();
        }
    }

    /// Exit snippet mode
    fn exit_snippet_mode(&mut self) {
        if self.snippet_state.is_some() {
            logging::log("EDITOR", "Exiting snippet mode");
            self.snippet_state = None;
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

        // Key handler for submit/cancel and snippet navigation
        // IMPORTANT: We intercept Tab here BEFORE gpui-component's Input processes it,
        // so we don't get tab characters inserted when navigating snippets.
        let handle_key = cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
            if this.suppress_keys {
                return;
            }

            let key = event.keystroke.key.to_lowercase();
            let cmd = event.keystroke.modifiers.platform;
            let shift = event.keystroke.modifiers.shift;

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
                        this.exit_snippet_mode();
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
            .on_key_down(handle_key);

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

        // Footer with language indicator
        let language_display: SharedString = self.language.clone().into();
        container = container.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .w_full()
                .h(px(24.))
                .px(px(12.))
                .bg(rgb(colors.background.title_bar))
                .border_t_1()
                .border_color(rgb(colors.ui.border))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(colors.text.muted))
                        .child(language_display),
                ),
        );

        container
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

        let state = SnippetState {
            snippet: snippet.clone(),
            current_tabstop_idx: 0,
        };

        assert_eq!(state.current_tabstop_idx, 0);
        assert_eq!(state.snippet.tabstops.len(), 1);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.text, "Hello name!");
    }

    #[test]
    fn test_snippet_state_multiple_tabstops() {
        let snippet = ParsedSnippet::parse("Hello ${1:name}, welcome to ${2:place}!");

        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
        };

        assert_eq!(state.snippet.tabstops.len(), 2);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.tabstops[1].index, 2);
        assert_eq!(state.snippet.text, "Hello name, welcome to place!");
    }

    #[test]
    fn test_snippet_state_with_final_cursor() {
        let snippet = ParsedSnippet::parse("Hello ${1:name}!$0");

        let state = SnippetState {
            snippet,
            current_tabstop_idx: 0,
        };

        // Should have 2 tabstops: index 1 first, then index 0 ($0) at end
        assert_eq!(state.snippet.tabstops.len(), 2);
        assert_eq!(state.snippet.tabstops[0].index, 1);
        assert_eq!(state.snippet.tabstops[1].index, 0);
    }
}
