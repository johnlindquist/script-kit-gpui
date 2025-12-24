//! GPUI Prompt UI Components
//!
//! Implements interactive prompt components for Script Kit:
//! - ArgPrompt: Selectable list with search/filtering
//! - DivPrompt: HTML content display

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, SharedString, Window,
};
use std::sync::Arc;

use crate::protocol::Choice;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

/// ArgPrompt - Interactive argument selection with search
///
/// Features:
/// - Searchable list of choices
/// - Keyboard navigation (up/down)
/// - Live filtering as you type
/// - Submit selected choice or cancel with Escape
pub struct ArgPrompt {
    pub id: String,
    pub placeholder: String,
    pub choices: Vec<Choice>,
    pub filtered_choices: Vec<usize>, // Indices into choices
    pub selected_index: usize,         // Index within filtered_choices
    pub input_text: String,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
}

impl ArgPrompt {
    pub fn new(
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
    ) -> Self {
        let filtered_choices: Vec<usize> = (0..choices.len()).collect();
        ArgPrompt {
            id,
            placeholder,
            choices,
            filtered_choices,
            selected_index: 0,
            input_text: String::new(),
            focus_handle,
            on_submit,
        }
    }

    /// Refilter choices based on current input_text
    fn refilter(&mut self) {
        let filter_lower = self.input_text.to_lowercase();
        self.filtered_choices = self
            .choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.name.to_lowercase().contains(&filter_lower))
            .map(|(idx, _)| idx)
            .collect();
        self.selected_index = 0; // Reset selection when filtering
    }

    /// Handle character input - append to input_text and refilter
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.input_text.push(ch);
        self.refilter();
        cx.notify();
    }

    /// Handle backspace - remove last character and refilter
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.input_text.is_empty() {
            self.input_text.pop();
            self.refilter();
            cx.notify();
        }
    }

    /// Move selection up within filtered choices
    fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            cx.notify();
        }
    }

    /// Move selection down within filtered choices
    fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < self.filtered_choices.len().saturating_sub(1) {
            self.selected_index += 1;
            cx.notify();
        }
    }

    /// Submit the selected choice
    fn submit_selected(&mut self) {
        if let Some(&choice_idx) = self.filtered_choices.get(self.selected_index) {
            if let Some(choice) = self.choices.get(choice_idx) {
                (self.on_submit)(self.id.clone(), Some(choice.value.clone()));
            }
        }
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }
}

impl Focusable for ArgPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ArgPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "up" | "arrowup" => this.move_up(cx),
                "down" | "arrowdown" => this.move_down(cx),
                "enter" => this.submit_selected(),
                "escape" => this.submit_cancel(),
                "backspace" => this.handle_backspace(cx),
                _ => {
                    // Try to capture printable characters
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

        // Render input field
        let input_display = if self.input_text.is_empty() {
            SharedString::from(self.placeholder.clone())
        } else {
            SharedString::from(self.input_text.clone())
        };

        let input_container = div()
            .w_full()
            .px(px(16.))
            .py(px(12.))
            .bg(rgb(0x2d2d2d))
            .border_b_1()
            .border_color(rgb(0x3d3d3d))
            .flex()
            .flex_row()
            .gap_2()
            .items_center()
            .child(div().text_color(rgb(0x888888)).child("🔍"))
            .child(
                div()
                    .flex_1()
                    .text_color(if self.input_text.is_empty() {
                        rgb(0x666666)
                    } else {
                        rgb(0xcccccc)
                    })
                    .child(input_display),
            );

        // Render choice list
        let mut choices_container = div()
            .flex()
            .flex_col()
            .flex_1()
            .w_full();

        if self.filtered_choices.is_empty() {
            choices_container = choices_container.child(
                div()
                    .w_full()
                    .py(px(32.))
                    .px(px(16.))
                    .text_color(rgb(0x666666))
                    .child("No choices match your filter"),
            );
        } else {
            for (idx, &choice_idx) in self.filtered_choices.iter().enumerate() {
                if let Some(choice) = self.choices.get(choice_idx) {
                    let is_selected = idx == self.selected_index;
                    let bg = if is_selected {
                        rgb(0x0e47a1) // Blue highlight
                    } else {
                        rgb(0x1e1e1e)
                    };

                    let name_color = if is_selected {
                        rgb(0xffffff)
                    } else {
                        rgb(0xcccccc)
                    };

                    let desc_color = if is_selected {
                        rgb(0xaaaaaa)
                    } else {
                        rgb(0x888888)
                    };

                    let mut choice_item = div()
                        .w_full()
                        .px(px(16.))
                        .py(px(10.))
                        .bg(bg)
                        .border_b_1()
                        .border_color(rgb(0x3d3d3d))
                        .flex()
                        .flex_col()
                        .gap_1();

                    // Choice name (bold-ish via uppercase and text styling)
                    choice_item = choice_item.child(
                        div()
                            .text_color(name_color)
                            .text_base()
                            .child(choice.name.clone()),
                    );

                    // Choice description if present (dimmed)
                    if let Some(desc) = &choice.description {
                        choice_item = choice_item.child(
                            div()
                                .text_color(desc_color)
                                .text_sm()
                                .child(desc.clone()),
                        );
                    }

                    choices_container = choices_container.child(choice_item);
                }
            }
        }

        // Main container
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xcccccc))
            .key_context("arg_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(input_container)
            .child(choices_container)
    }
}

/// DivPrompt - HTML content display
///
/// Features:
/// - Display HTML content (text extraction for prototype)
/// - Optional Tailwind styling
/// - Simple keyboard: Enter or Escape to submit
pub struct DivPrompt {
    pub id: String,
    pub html: String,
    pub tailwind: Option<String>,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
}

impl DivPrompt {
    pub fn new(
        id: String,
        html: String,
        tailwind: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
    ) -> Self {
        DivPrompt {
            id,
            html,
            tailwind,
            focus_handle,
            on_submit,
        }
    }

    /// Extract plain text from HTML by removing tags
    /// Simple regex-based strip for prototype
    fn strip_html_tags(html: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        let mut pending_space = false;

        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    pending_space = true; // Add space between tags
                }
                _ if !in_tag => {
                    if ch.is_whitespace() {
                        if !result.is_empty() && !result.ends_with(' ') {
                            pending_space = true;
                        }
                    } else {
                        if pending_space && !result.is_empty() {
                            result.push(' ');
                            pending_space = false;
                        }
                        result.push(ch);
                    }
                }
                _ => {} // Skip characters inside tags
            }
        }

        result.trim().to_string()
    }

    /// Submit - always with None value (just acknowledgment)
    fn submit(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }
}

impl Focusable for DivPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DivPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "enter" | "escape" => this.submit(),
                _ => {}
            }
        });

        // Extract and render text content
        let display_text = Self::strip_html_tags(&self.html);

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xcccccc))
            .p(px(16.))
            .key_context("div_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .child(display_text),
            )
            .child(
                div()
                    .w_full()
                    .pt(px(16.))
                    .text_color(rgb(0x888888))
                    .text_sm()
                    .child("Press Enter or Escape to continue"),
            )
    }
}
