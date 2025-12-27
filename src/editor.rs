//! GPUI Editor Prompt Component
//!
//! Implements a code editor prompt for Script Kit:
//! - EditorPrompt: Code display with syntax highlighting
//!
//! Phase 1 MVP: Read-only display with syntax highlighting,
//! line numbers, scroll handling, and basic keyboard (Cmd+Enter to submit, Escape to cancel).

#![allow(dead_code)]

use gpui::{
    div, prelude::*, px, rgb, uniform_list, Context, FocusHandle, Focusable, Pixels, Render,
    SharedString, UniformListScrollHandle, Window,
};
use std::ops::Range;
use std::sync::Arc;

use crate::logging;
use crate::syntax::{highlight_code_lines, HighlightedLine};
use crate::theme::Theme;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// EditorPrompt - Code editor with syntax highlighting
///
/// Phase 1 MVP Features:
/// - Display code with syntax highlighting (using existing syntect module)
/// - Line numbers
/// - Scrolling with virtualization (uniform_list)
/// - Submit (Cmd+Enter) and Cancel (Escape) keyboard handling
///
/// Future phases will add:
/// - Text editing (insert, delete)
/// - Cursor navigation
/// - Selection & clipboard
/// - Undo/redo
pub struct EditorPrompt {
    // Identity
    pub id: String,

    // Content (read-only for Phase 1)
    content: String,
    language: String,

    // Display
    highlighted_lines: Vec<HighlightedLine>,
    scroll_handle: UniformListScrollHandle,
    line_height: Pixels,

    // GPUI
    focus_handle: FocusHandle,
    on_submit: SubmitCallback,
    theme: Arc<Theme>,
}

impl EditorPrompt {
    /// Create a new EditorPrompt
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this prompt
    /// * `content` - Initial code content to display
    /// * `language` - Language for syntax highlighting (e.g., "typescript", "javascript", "rust")
    /// * `focus_handle` - GPUI focus handle for keyboard events
    /// * `on_submit` - Callback when user submits (Cmd+Enter) or cancels (Escape)
    /// * `theme` - Theme for styling
    pub fn new(
        id: String,
        content: String,
        language: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<Theme>,
    ) -> Self {
        logging::log(
            "EDITOR",
            &format!(
                "EditorPrompt::new id={}, lang={}, content_len={}",
                id,
                language,
                content.len()
            ),
        );

        // Pre-compute syntax highlighting for all lines
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
            content,
            language,
            highlighted_lines,
            scroll_handle: UniformListScrollHandle::new(),
            line_height: px(20.),
            focus_handle,
            on_submit,
            theme,
        }
    }

    /// Get the current content (for submission)
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the language
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Get line count
    pub fn line_count(&self) -> usize {
        self.highlighted_lines.len().max(1)
    }

    /// Submit the current content
    fn submit(&self) {
        logging::log("EDITOR", &format!("Submit id={}", self.id));
        (self.on_submit)(self.id.clone(), Some(self.content.clone()));
    }

    /// Cancel - submit None
    fn cancel(&self) {
        logging::log("EDITOR", &format!("Cancel id={}", self.id));
        (self.on_submit)(self.id.clone(), None);
    }

    /// Render a range of lines for uniform_list virtualization
    fn render_lines(&self, range: Range<usize>) -> Vec<impl IntoElement> {
        let colors = &self.theme.colors;
        let gutter_width = px(50.);

        range
            .map(|line_idx| {
                let line = self.highlighted_lines.get(line_idx);
                let line_number = line_idx + 1;

                div()
                    .id(("editor-line", line_idx))
                    .flex()
                    .flex_row()
                    .h(self.line_height)
                    .w_full()
                    .child(
                        // Line number gutter
                        div()
                            .w(gutter_width)
                            .flex_shrink_0()
                            .text_color(rgb(colors.text.muted))
                            .text_sm()
                            .px_2()
                            .flex()
                            .items_center()
                            .justify_end()
                            .child(SharedString::from(format!("{}", line_number))),
                    )
                    .child(
                        // Code content
                        div()
                            .flex_1()
                            .px_2()
                            .flex()
                            .flex_row()
                            .items_center()
                            .overflow_hidden()
                            .children(line.map(|l| self.render_line_spans(l)).unwrap_or_default()),
                    )
            })
            .collect()
    }

    /// Render the spans within a single line
    fn render_line_spans(&self, line: &HighlightedLine) -> Vec<impl IntoElement> {
        line.spans
            .iter()
            .map(|span| {
                div()
                    .text_color(rgb(span.color))
                    .child(SharedString::from(span.text.clone()))
            })
            .collect()
    }

    /// Render the status bar at the bottom
    fn render_status_bar(&self) -> impl IntoElement {
        let colors = &self.theme.colors;
        let line_count = self.line_count();

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
            .child(
                div()
                    .text_color(rgb(colors.text.secondary))
                    .text_xs()
                    .child(SharedString::from(format!("{} lines", line_count))),
            )
            .child(
                div()
                    .text_color(rgb(colors.text.muted))
                    .text_xs()
                    .child(SharedString::from(format!(
                        "{} | Cmd+Enter to submit, Escape to cancel",
                        self.language
                    ))),
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

        // Keyboard handler
        let handle_key =
            cx.listener(
                move |this: &mut Self,
                      event: &gpui::KeyDownEvent,
                      _window: &mut Window,
                      _cx: &mut Context<Self>| {
                    let key_str = event.keystroke.key.to_lowercase();
                    let cmd = event.keystroke.modifiers.platform;

                    match (key_str.as_str(), cmd) {
                        ("enter", true) => this.submit(),
                        ("escape", _) => this.cancel(),
                        _ => {
                            // Phase 1: Read-only, ignore other keys
                            // Future phases will handle editing here
                        }
                    }
                },
            );

        div()
            .id("editor-prompt")
            .key_context("EditorPrompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(colors.background.main))
            .child(
                // Editor content area with virtualized line rendering
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "editor-lines",
                            line_count,
                            cx.processor(|this, range: std::ops::Range<usize>, _window, _cx| {
                                this.render_lines(range)
                            }),
                        )
                        .track_scroll(&self.scroll_handle)
                        .h_full(),
                    ),
            )
            .child(self.render_status_bar())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_prompt_creation() {
        // Test that EditorPrompt struct fields can be accessed
        // Note: Full GPUI tests require TestAppContext, which is only
        // available with the test harness feature enabled
    }

    #[test]
    fn test_line_count_empty() {
        // Empty content should still report at least 1 line
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
        // Should have at least one line with spans
        assert!(!lines[0].spans.is_empty());
    }
}
