//! TemplatePrompt - String template with tab-through placeholders
//!
//! Features:
//! - Parse template strings with $1, $2, ${1:default} syntax
//! - Tab through placeholders like VSCode snippets
//! - Live preview of filled template

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, SharedString, Window,
};
use std::sync::Arc;

use crate::logging;
use crate::theme;
use crate::designs::{DesignVariant, get_tokens};

use super::SubmitCallback;

/// Input definition for a template placeholder
#[derive(Clone, Debug)]
pub struct TemplateInput {
    /// Name/index of the placeholder (e.g., "1", "2", or "name")
    pub name: String,
    /// Placeholder text to show when empty
    pub placeholder: Option<String>,
    /// Default value if provided
    pub default: Option<String>,
}

/// TemplatePrompt - Tab-through template editor
///
/// Allows editing template strings with multiple placeholders,
/// similar to VSCode snippets. Tab moves between placeholders.
pub struct TemplatePrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Original template string with placeholders
    pub template: String,
    /// Parsed input placeholders
    pub inputs: Vec<TemplateInput>,
    /// Current values for each input
    pub values: Vec<String>,
    /// Currently focused input index
    pub current_input: usize,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
}

impl TemplatePrompt {
    pub fn new(
        id: String,
        template: String,
        inputs: Option<Vec<TemplateInput>>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        logging::log("PROMPTS", &format!("TemplatePrompt::new template: {}", template));
        
        // Parse inputs from template if not provided
        let inputs = inputs.unwrap_or_else(|| Self::parse_template_inputs(&template));
        let values: Vec<String> = inputs.iter()
            .map(|i| i.default.clone().unwrap_or_default())
            .collect();
        
        TemplatePrompt {
            id,
            template,
            inputs,
            values,
            current_input: 0,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
        }
    }

    /// Parse template string to extract inputs
    /// Supports: $1, $2, ${1}, ${1:default}, ${name}, ${name:default}
    fn parse_template_inputs(template: &str) -> Vec<TemplateInput> {
        let mut inputs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        
        // Simple regex-like parsing for $1, ${1}, ${1:default} patterns
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;
        
        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                if chars[i + 1] == '{' {
                    // ${...} format
                    if let Some(end) = chars[i + 2..].iter().position(|&c| c == '}') {
                        let content: String = chars[i + 2..i + 2 + end].iter().collect();
                        let (name, default) = if let Some(colon_pos) = content.find(':') {
                            (content[..colon_pos].to_string(), Some(content[colon_pos + 1..].to_string()))
                        } else {
                            (content.clone(), None)
                        };
                        
                        if !seen.contains(&name) {
                            seen.insert(name.clone());
                            inputs.push(TemplateInput {
                                name: name.clone(),
                                placeholder: Some(format!("Enter {}", name)),
                                default,
                            });
                        }
                        i += 3 + end;
                        continue;
                    }
                } else if chars[i + 1].is_ascii_digit() {
                    // $1, $2, etc. format
                    let name = chars[i + 1].to_string();
                    if !seen.contains(&name) {
                        seen.insert(name.clone());
                        inputs.push(TemplateInput {
                            name: name.clone(),
                            placeholder: Some(format!("Input {}", name)),
                            default: None,
                        });
                    }
                    i += 2;
                    continue;
                }
            }
            i += 1;
        }
        
        inputs
    }

    /// Get the filled template string
    fn filled_template(&self) -> String {
        let mut result = self.template.clone();
        
        for (input, value) in self.inputs.iter().zip(self.values.iter()) {
            // Replace ${name:default} and ${name}
            result = result.replace(&format!("${{{}:{}}}", input.name, input.default.as_deref().unwrap_or("")), value);
            result = result.replace(&format!("${{{}}}", input.name), value);
            // Replace $1, $2, etc.
            result = result.replace(&format!("${}", input.name), value);
        }
        
        result
    }

    /// Submit the filled template
    fn submit(&mut self) {
        let filled = self.filled_template();
        (self.on_submit)(self.id.clone(), Some(filled));
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Move to next input (Tab)
    fn next_input(&mut self, cx: &mut Context<Self>) {
        if self.current_input < self.inputs.len().saturating_sub(1) {
            self.current_input += 1;
            cx.notify();
        }
    }

    /// Move to previous input (Shift+Tab)
    fn prev_input(&mut self, cx: &mut Context<Self>) {
        if self.current_input > 0 {
            self.current_input -= 1;
            cx.notify();
        }
    }

    /// Handle character input for current field
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        if let Some(value) = self.values.get_mut(self.current_input) {
            value.push(ch);
            cx.notify();
        }
    }

    /// Handle backspace for current field
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(value) = self.values.get_mut(self.current_input) {
            if !value.is_empty() {
                value.pop();
                cx.notify();
            }
        }
    }
}

impl Focusable for TemplatePrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TemplatePrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();

        let handle_key = cx.listener(|this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "tab" => {
                    if event.keystroke.modifiers.shift {
                        this.prev_input(cx);
                    } else {
                        this.next_input(cx);
                    }
                }
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

        let filled = self.filled_template();

        let mut container = div()
            .id(gpui::ElementId::Name("window:template".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(main_bg)
            .text_color(text_color)
            .p(px(spacing.padding_lg))
            .key_context("template_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key);

        // Preview of filled template
        container = container.child(
            div()
                .text_lg()
                .child("Preview:")
        ).child(
            div()
                .mt(px(spacing.padding_sm))
                .px(px(spacing.item_padding_x))
                .py(px(spacing.padding_md))
                .bg(rgb(self.theme.colors.background.search_box))
                .rounded(px(4.))
                .child(filled)
        );

        // Input fields
        container = container.child(
            div()
                .mt(px(spacing.padding_lg))
                .text_sm()
                .text_color(muted_color)
                .child(format!("Tab through {} field(s)", self.inputs.len()))
        );

        for (idx, input) in self.inputs.iter().enumerate() {
            let is_current = idx == self.current_input;
            let value = self.values.get(idx).cloned().unwrap_or_default();
            
            let display = if value.is_empty() {
                SharedString::from(input.placeholder.clone().unwrap_or_else(|| "...".to_string()))
            } else {
                SharedString::from(value)
            };

            let field_bg = if is_current {
                rgb(self.theme.colors.accent.selected_subtle)
            } else {
                rgb(self.theme.colors.background.search_box)
            };

            container = container.child(
                div()
                    .mt(px(spacing.padding_sm))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted_color)
                            .child(format!("${}:", input.name))
                    )
                    .child(
                        div()
                            .flex_1()
                            .px(px(spacing.item_padding_x))
                            .py(px(spacing.padding_sm))
                            .bg(field_bg)
                            .rounded(px(4.))
                            .child(display)
                    )
            );
        }

        container
    }
}
