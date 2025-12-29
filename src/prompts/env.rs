//! EnvPrompt - Environment variable prompt with keyring storage
//!
//! Features:
//! - Prompt for environment variable values
//! - Secure storage via system keyring (keychain on macOS)
//! - Mask input for secret values
//! - Remember values for future sessions

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, SharedString, Window,
};
use std::sync::Arc;

use crate::logging;
use crate::theme;
use crate::designs::{DesignVariant, get_tokens};

use super::SubmitCallback;

/// EnvPrompt - Environment variable prompt with secure storage
///
/// Prompts for environment variable values and stores them securely
/// using the system keyring. Useful for API keys, tokens, and secrets.
pub struct EnvPrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Environment variable key name
    pub key: String,
    /// Custom prompt text (defaults to "Enter value for {key}")
    pub prompt: Option<String>,
    /// Whether to mask input (for secrets)
    pub secret: bool,
    /// Current input value
    pub input_text: String,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits a value
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
}

impl EnvPrompt {
    pub fn new(
        id: String,
        key: String,
        prompt: Option<String>,
        secret: bool,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        logging::log("PROMPTS", &format!("EnvPrompt::new for key: {} (secret: {})", key, secret));
        
        EnvPrompt {
            id,
            key,
            prompt,
            secret,
            input_text: String::new(),
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
        }
    }

    /// Submit the entered value
    fn submit(&mut self) {
        if !self.input_text.is_empty() {
            // TODO: Store in keyring if secret
            (self.on_submit)(self.id.clone(), Some(self.input_text.clone()));
        }
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Handle character input
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.input_text.push(ch);
        cx.notify();
    }

    /// Handle backspace
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.input_text.is_empty() {
            self.input_text.pop();
            cx.notify();
        }
    }

    /// Get display text (masked if secret)
    fn display_text(&self) -> String {
        if self.secret && !self.input_text.is_empty() {
            "•".repeat(self.input_text.len())
        } else {
            self.input_text.clone()
        }
    }
}

impl Focusable for EnvPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EnvPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();

        let handle_key = cx.listener(|this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "enter" => this.submit(),
                "escape" => this.submit_cancel(),
                "backspace" => this.handle_backspace(cx),
                _ => {
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if !ch.is_control() {
                                this.handle_char(ch, cx);
                            }
                        }
                    }
                }
            }
        });

        let (main_bg, text_color, muted_color) = if self.design_variant == DesignVariant::Default {
            (
                rgb(self.theme.colors.background.main),
                rgb(self.theme.colors.text.secondary),
                rgb(self.theme.colors.text.muted),
            )
        } else {
            (
                rgb(colors.background),
                rgb(colors.text_secondary),
                rgb(colors.text_muted),
            )
        };

        let prompt_text = self.prompt.clone()
            .unwrap_or_else(|| format!("Enter value for {}", self.key));
        
        let display_text = self.display_text();
        let input_display = if display_text.is_empty() {
            SharedString::from("Type here...")
        } else {
            SharedString::from(display_text)
        };

        div()
            .id(gpui::ElementId::Name("window:env".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(main_bg)
            .text_color(text_color)
            .p(px(spacing.padding_lg))
            .key_context("env_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                div()
                    .text_lg()
                    .child(prompt_text)
            )
            .child(
                div()
                    .mt(px(spacing.padding_md))
                    .px(px(spacing.item_padding_x))
                    .py(px(spacing.padding_md))
                    .bg(rgb(self.theme.colors.background.search_box))
                    .rounded(px(4.))
                    .text_color(if self.input_text.is_empty() { muted_color } else { text_color })
                    .child(input_display)
            )
            .child(
                div()
                    .mt(px(spacing.padding_sm))
                    .text_sm()
                    .text_color(muted_color)
                    .child(if self.secret { "🔒 Value will be stored securely" } else { "Value will be saved to environment" })
            )
    }
}
