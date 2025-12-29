//! SelectPrompt - Multi-select from choices
//!
//! Features:
//! - Select multiple items from a list
//! - Toggle selection with Space
//! - Filter choices by typing
//! - Submit selected items

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, SharedString, Window,
};
use std::sync::Arc;

use crate::logging;
use crate::protocol::{Choice, generate_semantic_id};
use crate::theme;
use crate::designs::{DesignVariant, get_tokens};

use super::SubmitCallback;

/// SelectPrompt - Multi-select from choices
///
/// Allows selecting multiple items from a list of choices.
/// Use Space to toggle selection, Enter to submit selected items.
pub struct SelectPrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Placeholder text for the search input
    pub placeholder: Option<String>,
    /// Available choices
    pub choices: Vec<Choice>,
    /// Indices of selected choices
    pub selected: Vec<usize>,
    /// Filtered choice indices (for display)
    pub filtered_choices: Vec<usize>,
    /// Currently focused index in filtered list
    pub focused_index: usize,
    /// Filter text
    pub filter_text: String,
    /// Whether multiple selection is allowed
    pub multiple: bool,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
}

impl SelectPrompt {
    pub fn new(
        id: String,
        placeholder: Option<String>,
        choices: Vec<Choice>,
        multiple: bool,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        logging::log("PROMPTS", &format!("SelectPrompt::new with {} choices (multiple: {})", choices.len(), multiple));
        
        let filtered_choices: Vec<usize> = (0..choices.len()).collect();
        
        SelectPrompt {
            id,
            placeholder,
            choices,
            selected: Vec::new(),
            filtered_choices,
            focused_index: 0,
            filter_text: String::new(),
            multiple,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
        }
    }

    /// Refilter choices based on current filter_text
    fn refilter(&mut self) {
        let filter_lower = self.filter_text.to_lowercase();
        self.filtered_choices = self.choices.iter()
            .enumerate()
            .filter(|(_, choice)| choice.name.to_lowercase().contains(&filter_lower))
            .map(|(idx, _)| idx)
            .collect();
        self.focused_index = 0;
    }

    /// Toggle selection of currently focused item
    fn toggle_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(&choice_idx) = self.filtered_choices.get(self.focused_index) {
            if self.multiple {
                if let Some(pos) = self.selected.iter().position(|&x| x == choice_idx) {
                    self.selected.remove(pos);
                } else {
                    self.selected.push(choice_idx);
                }
            } else {
                // Single select mode - replace selection
                self.selected = vec![choice_idx];
            }
            cx.notify();
        }
    }

    /// Submit selected items as JSON array
    fn submit(&mut self) {
        let selected_values: Vec<String> = self.selected.iter()
            .filter_map(|&idx| self.choices.get(idx))
            .map(|c| c.value.clone())
            .collect();
        
        let json_str = serde_json::to_string(&selected_values).unwrap_or_else(|_| "[]".to_string());
        (self.on_submit)(self.id.clone(), Some(json_str));
    }

    /// Cancel - submit None
    fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Move focus up
    fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.focused_index > 0 {
            self.focused_index -= 1;
            cx.notify();
        }
    }

    /// Move focus down
    fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.focused_index < self.filtered_choices.len().saturating_sub(1) {
            self.focused_index += 1;
            cx.notify();
        }
    }

    /// Handle character input
    fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.filter_text.push(ch);
        self.refilter();
        cx.notify();
    }

    /// Handle backspace
    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.refilter();
            cx.notify();
        }
    }
}

impl Focusable for SelectPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectPrompt {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = get_tokens(self.design_variant);
        let colors = tokens.colors();
        let spacing = tokens.spacing();
        let visual = tokens.visual();

        let handle_key = cx.listener(|this: &mut Self, event: &gpui::KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            
            match key_str.as_str() {
                "up" | "arrowup" => this.move_up(cx),
                "down" | "arrowdown" => this.move_down(cx),
                "space" | " " => this.toggle_selection(cx),
                "enter" => this.submit(),
                "escape" => this.submit_cancel(),
                "backspace" => this.handle_backspace(cx),
                _ => {
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if !ch.is_control() && ch != ' ' {
                                this.handle_char(ch, cx);
                            }
                        }
                    }
                }
            }
        });

        let (main_bg, text_color, muted_color, border_color) = if self.design_variant == DesignVariant::Default {
            (
                rgb(self.theme.colors.background.main),
                rgb(self.theme.colors.text.secondary),
                rgb(self.theme.colors.text.muted),
                rgb(self.theme.colors.ui.border),
            )
        } else {
            (
                rgb(colors.background),
                rgb(colors.text_secondary),
                rgb(colors.text_muted),
                rgb(colors.border),
            )
        };

        let placeholder = self.placeholder.clone()
            .unwrap_or_else(|| "Search...".to_string());

        let input_display = if self.filter_text.is_empty() {
            SharedString::from(placeholder)
        } else {
            SharedString::from(self.filter_text.clone())
        };

        // Search input
        let input_container = div()
            .id(gpui::ElementId::Name("input:select-filter".into()))
            .w_full()
            .px(px(spacing.item_padding_x))
            .py(px(spacing.padding_md))
            .bg(rgb(self.theme.colors.background.search_box))
            .border_b_1()
            .border_color(border_color)
            .flex()
            .flex_row()
            .gap_2()
            .items_center()
            .child(div().text_color(muted_color).child("🔍"))
            .child(
                div()
                    .flex_1()
                    .text_color(if self.filter_text.is_empty() { muted_color } else { text_color })
                    .child(input_display)
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted_color)
                    .child(format!("{} selected", self.selected.len()))
            );

        // Choices list
        let mut choices_container = div()
            .id(gpui::ElementId::Name("list:select-choices".into()))
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .overflow_y_hidden();

        if self.filtered_choices.is_empty() {
            choices_container = choices_container.child(
                div()
                    .w_full()
                    .py(px(spacing.padding_xl))
                    .px(px(spacing.item_padding_x))
                    .text_color(muted_color)
                    .child("No choices match your filter")
            );
        } else {
            for (display_idx, &choice_idx) in self.filtered_choices.iter().enumerate() {
                if let Some(choice) = self.choices.get(choice_idx) {
                    let is_focused = display_idx == self.focused_index;
                    let is_selected = self.selected.contains(&choice_idx);
                    
                    let semantic_id = choice.semantic_id.clone()
                        .unwrap_or_else(|| generate_semantic_id("select", display_idx, &choice.value));
                    
                    let bg = if is_focused {
                        rgb(self.theme.colors.accent.selected)
                    } else {
                        main_bg
                    };

                    let checkbox = if is_selected { "☑" } else { "☐" };

                    let mut choice_item = div()
                        .id(gpui::ElementId::Name(semantic_id.into()))
                        .w_full()
                        .px(px(spacing.item_padding_x))
                        .py(px(spacing.item_padding_y))
                        .bg(bg)
                        .border_b_1()
                        .border_color(border_color)
                        .rounded(px(visual.radius_sm))
                        .flex()
                        .flex_row()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_color(if is_selected { rgb(self.theme.colors.accent.selected) } else { muted_color })
                                .child(checkbox)
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(text_color)
                                .child(choice.name.clone())
                        );

                    if let Some(desc) = &choice.description {
                        choice_item = choice_item.child(
                            div()
                                .text_sm()
                                .text_color(muted_color)
                                .child(desc.clone())
                        );
                    }

                    choices_container = choices_container.child(choice_item);
                }
            }
        }

        div()
            .id(gpui::ElementId::Name("window:select".into()))
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(main_bg)
            .text_color(text_color)
            .key_context("select_prompt")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(input_container)
            .child(choices_container)
    }
}
