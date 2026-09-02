use gpui::*;

use crate::protocol::Field;

use super::{FormFieldColors, FormFieldMetrics, FormFieldState};

pub struct FormCheckbox {
    /// Field definition from protocol
    field: Field,
    /// Pre-computed colors
    colors: FormFieldColors,
    /// Whether the checkbox is checked
    checked: bool,
    input_revision: u64,
    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,
    /// Shared state for external access (stores "true" or "false")
    pub state: FormFieldState,
}

impl FormCheckbox {
    /// Create a new checkbox from a Field definition
    pub fn new(field: Field, colors: FormFieldColors, cx: &mut App) -> Self {
        // Parse initial checked state from value
        let checked = field.value.as_deref() == Some("true");
        let state = FormFieldState::new(if checked {
            "true".to_string()
        } else {
            "false".to_string()
        });

        Self {
            field,
            colors,
            checked,
            input_revision: 0,
            focus_handle: cx.focus_handle(),
            state,
        }
    }

    /// Get whether the checkbox is checked
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub(crate) fn revision(&self) -> u64 {
        self.input_revision
    }

    fn advance_input_revision(&mut self) {
        assert!(
            self.input_revision < u64::MAX,
            "form input revision exhausted"
        );
        self.input_revision += 1;
    }

    pub(crate) fn update_colors(&mut self, colors: FormFieldColors, cx: &mut Context<Self>) {
        self.colors = colors;
        cx.notify();
    }

    /// Get the field name
    pub fn name(&self) -> &str {
        &self.field.name
    }

    /// Toggle the checkbox state
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.advance_input_revision();
        self.checked = !self.checked;
        self.state.set_value(if self.checked {
            "true".to_string()
        } else {
            "false".to_string()
        });
        cx.notify();
    }

    /// Set the checked state
    pub fn set_checked(&mut self, checked: bool, cx: &mut Context<Self>) {
        if self.checked != checked {
            self.advance_input_revision();
        }
        self.checked = checked;
        self.state.set_value(if checked {
            "true".to_string()
        } else {
            "false".to_string()
        });
        cx.notify();
    }
}

impl Focusable for FormCheckbox {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FormCheckbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let metrics = FormFieldMetrics::from_colors(colors);
        let is_focused = self.focus_handle.is_focused(window);
        let checked = self.checked;
        let label = self
            .field
            .label
            .clone()
            .unwrap_or_else(|| self.field.name.clone());

        // Calculate border and background based on focus using shared whisper surface
        let surface = colors.whisper_surface(is_focused);
        let border_color = surface.border;

        // Checkbox box styling
        let box_bg = if checked {
            surface.checked_fill
        } else {
            surface.background
        };

        let field_name = self.field.name.clone();

        // Keyboard handler for Space key to toggle
        let handle_key = cx.listener(
            |this: &mut Self,
             event: &KeyDownEvent,
             _window: &mut Window,
             cx: &mut Context<Self>| {
                let key = event.keystroke.key.as_str();
                if key == "space" || key == " " {
                    this.toggle(cx);
                }
            },
        );

        // Build checkbox box with optional checkmark
        let mut checkbox_box = div()
            .flex()
            .items_center()
            .justify_center()
            .w(rems(metrics.checkbox_box_size_rems))
            .h(rems(metrics.checkbox_box_size_rems))
            .bg(box_bg)
            .border_1()
            .border_color(border_color)
            .rounded(px(metrics.checkbox_radius_px));

        // Add checkmark when checked
        if checked {
            checkbox_box = checkbox_box.child(
                div()
                    .text_size(px(colors.label_font_size))
                    .text_color(colors.checkbox_mark)
                    .font_weight(FontWeight::BOLD)
                    .child("✓"),
            );
        }

        let bg_color = surface.background;

        let focus_handle_for_click = self.focus_handle.clone();

        // Full-width interactive row - uses shared prompt_surface for consistent card chrome
        crate::components::prompt_surface(bg_color, border_color)
            .id(ElementId::Name(
                format!("form-checkbox-{}", field_name).into(),
            ))
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .on_click(cx.listener(move |this, _event: &ClickEvent, window, cx| {
                focus_handle_for_click.focus(window, cx);
                this.toggle(cx);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap(rems(metrics.checkbox_gap_rems))
            .cursor_pointer()
            .child(checkbox_box)
            .child(
                div()
                    .text_size(px(colors.input_font_size))
                    .text_color(colors.text)
                    .child(label),
            )
    }
}

// Note: Full GPUI component tests require the test harness which has macro recursion
// limit issues. The form field components are integration-tested via the main
// application's form prompt rendering. Unit tests for helper functions are in
// src/components/form_fields_tests.rs.
