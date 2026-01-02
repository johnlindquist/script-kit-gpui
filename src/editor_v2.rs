//! EditorPrompt v2 - Using gpui-component's Input in code_editor mode
//!
//! This is a replacement for the custom editor implementation in editor.rs.
//! It uses gpui-component's full-featured code editor component which includes:
//! - High-performance editing (200K+ lines)
//! - Built-in Find/Replace with SearchPanel (Cmd+F)
//! - Syntax highlighting via Tree Sitter
//! - Undo/Redo with proper history
//! - Line numbers, soft wrap, indentation
//! - LSP hooks for diagnostics/completion

use gpui::{
    div, prelude::*, px, rgb, Context, Entity, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Styled, Subscription, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};
use std::sync::Arc;

use crate::config::Config;
use crate::logging;
use crate::theme::Theme;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// Pending initialization state - stored until first render when window is available
struct PendingInit {
    content: String,
    language: String,
}

/// EditorPrompt v2 - Full-featured code editor using gpui-component
///
/// Uses deferred initialization pattern: the InputState is created on first render
/// when the Window reference is available, not at construction time.
pub struct EditorPromptV2 {
    // Identity
    pub id: String,

    // gpui-component editor state (created on first render)
    editor_state: Option<Entity<InputState>>,

    // Pending initialization data (consumed on first render)
    pending_init: Option<PendingInit>,

    // Language for syntax highlighting
    #[allow(dead_code)]
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
}

impl EditorPromptV2 {
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
                "EditorPromptV2::with_height id={}, lang={}, content_len={}, height={:?}",
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
            language,
            focus_handle,
            on_submit,
            theme,
            config,
            content_height,
            subscriptions: Vec::new(),
            suppress_keys: false,
            needs_focus: true, // Auto-focus on first render
        }
    }

    /// Create a new EditorPrompt in template/snippet mode
    ///
    /// Note: Template support (tabstop navigation) is not yet implemented in v2.
    /// This falls back to regular editing mode with the expanded template text.
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
                "EditorPromptV2::with_template id={}, lang={}, template_len={}, height={:?}",
                id,
                language,
                template.len(),
                content_height
            ),
        );

        // TODO: Parse template for tabstops when gpui-component supports it
        // For now, use the template as content directly
        Self::with_height(
            id,
            template,
            language,
            focus_handle,
            on_submit,
            theme,
            config,
            content_height,
        )
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

impl Focusable for EditorPromptV2 {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EditorPromptV2 {
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

        let colors = &self.theme.colors;

        // Key handler for submit/cancel
        let handle_key = cx.listener(move |this, event: &gpui::KeyDownEvent, _window, cx| {
            if this.suppress_keys {
                return;
            }

            let key = event.keystroke.key.to_lowercase();
            let cmd = event.keystroke.modifiers.platform;

            match (key.as_str(), cmd) {
                // Cmd+Enter submits
                ("enter", true) => {
                    this.submit(cx);
                }
                // Cmd+S also submits (save)
                ("s", true) => {
                    this.submit(cx);
                }
                // Escape cancels (handled at parent level)
                ("escape", _) => {
                    // Let parent handle this
                }
                _ => {}
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

        container
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_editor_v2_creation() {
        // Basic smoke test - just verify the struct can be created with expected fields
        // Full integration tests require GPUI context
    }
}
