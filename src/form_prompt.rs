use gpui::{
    div, prelude::*, px, rgb, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Render,
    Window,
};

use crate::components::{FormCheckbox, FormFieldColors, FormTextArea, FormTextField};
use crate::{form_parser, logging, protocol};

/// Enum to hold different types of form field entities.
#[derive(Clone)]
pub enum FormFieldEntity {
    TextField(Entity<FormTextField>),
    TextArea(Entity<FormTextArea>),
    Checkbox(Entity<FormCheckbox>),
}

/// Form prompt state - holds the parsed form fields and their entities.
pub struct FormPromptState {
    /// Prompt ID for response.
    pub id: String,
    /// Original HTML for reference.
    #[allow(dead_code)]
    pub html: String,
    /// Parsed field definitions and their corresponding entities.
    pub fields: Vec<(protocol::Field, FormFieldEntity)>,
    /// Colors for form fields.
    pub colors: FormFieldColors,
    /// Currently focused field index (for Tab navigation).
    pub focused_index: usize,
    /// Focus handle for this form.
    pub focus_handle: FocusHandle,
    /// Whether we've done initial focus.
    pub did_initial_focus: bool,
}

impl FormPromptState {
    fn build_values_json(values: impl IntoIterator<Item = (String, String)>) -> String {
        let mut map = serde_json::Map::new();
        for (key, value) in values {
            map.insert(key, serde_json::Value::String(value));
        }
        serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
    }

    /// Create a new form prompt state from HTML.
    pub fn new(id: String, html: String, colors: FormFieldColors, cx: &mut App) -> Self {
        let parsed_fields = form_parser::parse_form_html(&html);

        logging::log(
            "FORM",
            &format!("Parsed {} form fields from HTML", parsed_fields.len()),
        );

        let fields: Vec<(protocol::Field, FormFieldEntity)> = parsed_fields
            .into_iter()
            .map(|field| {
                let field_type = field
                    .field_type
                    .clone()
                    .unwrap_or_else(|| "text".to_string());
                logging::log(
                    "FORM",
                    &format!("Creating field: {} (type: {})", field.name, field_type),
                );

                let entity = match field_type.as_str() {
                    "checkbox" => {
                        let checkbox = FormCheckbox::new(field.clone(), colors, cx);
                        FormFieldEntity::Checkbox(cx.new(|_| checkbox))
                    }
                    "textarea" => {
                        let textarea = FormTextArea::new(field.clone(), colors, 4, cx);
                        FormFieldEntity::TextArea(cx.new(|_| textarea))
                    }
                    _ => {
                        // text, password, email, number all use TextField
                        let textfield = FormTextField::new(field.clone(), colors, cx);
                        FormFieldEntity::TextField(cx.new(|_| textfield))
                    }
                };

                (field, entity)
            })
            .collect();

        Self {
            id,
            html,
            fields,
            colors,
            focused_index: 0,
            focus_handle: cx.focus_handle(),
            did_initial_focus: false,
        }
    }

    /// Get all field values as a JSON object string.
    pub fn collect_values(&self, cx: &App) -> String {
        let values = self.fields.iter().map(|(field_def, entity)| {
            let value = match entity {
                FormFieldEntity::TextField(e) => e.read(cx).value().to_string(),
                FormFieldEntity::TextArea(e) => e.read(cx).value().to_string(),
                FormFieldEntity::Checkbox(e) => {
                    if e.read(cx).is_checked() {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                }
            };
            (field_def.name.clone(), value)
        });

        Self::build_values_json(values)
    }

    /// Focus the next field (for Tab navigation).
    pub fn focus_next(&mut self, cx: &mut Context<Self>) {
        if self.fields.is_empty() {
            return;
        }
        self.focused_index = (self.focused_index + 1) % self.fields.len();
        cx.notify();
    }

    /// Focus the previous field (for Shift+Tab navigation).
    pub fn focus_previous(&mut self, cx: &mut Context<Self>) {
        if self.fields.is_empty() {
            return;
        }
        if self.focused_index == 0 {
            self.focused_index = self.fields.len() - 1;
        } else {
            self.focused_index -= 1;
        }
        cx.notify();
    }

    /// Get the focus handle for the currently focused field.
    pub fn current_focus_handle(&self, cx: &App) -> Option<FocusHandle> {
        self.fields
            .get(self.focused_index)
            .map(|(_, entity)| match entity {
                FormFieldEntity::TextField(e) => e.read(cx).focus_handle(cx),
                FormFieldEntity::TextArea(e) => e.read(cx).focus_handle(cx),
                FormFieldEntity::Checkbox(e) => e.read(cx).focus_handle(cx),
            })
    }

    /// Handle keyboard input by forwarding to the currently focused field.
    pub fn handle_key_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if let Some((_, entity)) = self.fields.get(self.focused_index) {
            let key = event.keystroke.key.as_str();

            match entity {
                FormFieldEntity::TextField(e) => {
                    e.update(cx, |field, cx| {
                        // Handle special keys
                        match key {
                            "backspace" => {
                                if field.cursor_position > 0 {
                                    field.cursor_position -= 1;
                                    field.value.remove(field.cursor_position);
                                    field.state.set_value(field.value.clone());
                                    cx.notify();
                                }
                            }
                            "delete" => {
                                if field.cursor_position < field.value.len() {
                                    field.value.remove(field.cursor_position);
                                    field.state.set_value(field.value.clone());
                                    cx.notify();
                                }
                            }
                            "left" | "arrowleft" => {
                                if field.cursor_position > 0 {
                                    field.cursor_position -= 1;
                                    cx.notify();
                                }
                            }
                            "right" | "arrowright" => {
                                if field.cursor_position < field.value.len() {
                                    field.cursor_position += 1;
                                    cx.notify();
                                }
                            }
                            "home" => {
                                field.cursor_position = 0;
                                cx.notify();
                            }
                            "end" => {
                                field.cursor_position = field.value.len();
                                cx.notify();
                            }
                            _ => {
                                // Handle printable character input
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            field.value.insert(field.cursor_position, ch);
                                            field.cursor_position += ch.len_utf8();
                                            field.state.set_value(field.value.clone());
                                            cx.notify();
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                FormFieldEntity::TextArea(e) => {
                    e.update(cx, |field, cx| {
                        // Handle special keys
                        match key {
                            "backspace" => {
                                if field.cursor_position > 0 {
                                    field.cursor_position -= 1;
                                    field.value.remove(field.cursor_position);
                                    field.state.set_value(field.value.clone());
                                    cx.notify();
                                }
                            }
                            "delete" => {
                                if field.cursor_position < field.value.len() {
                                    field.value.remove(field.cursor_position);
                                    field.state.set_value(field.value.clone());
                                    cx.notify();
                                }
                            }
                            "left" | "arrowleft" => {
                                if field.cursor_position > 0 {
                                    field.cursor_position -= 1;
                                    cx.notify();
                                }
                            }
                            "right" | "arrowright" => {
                                if field.cursor_position < field.value.len() {
                                    field.cursor_position += 1;
                                    cx.notify();
                                }
                            }
                            "home" => {
                                field.cursor_position = 0;
                                cx.notify();
                            }
                            "end" => {
                                field.cursor_position = field.value.len();
                                cx.notify();
                            }
                            _ => {
                                // Handle printable character input
                                if let Some(ref key_char) = event.keystroke.key_char {
                                    if let Some(ch) = key_char.chars().next() {
                                        if !ch.is_control() {
                                            field.value.insert(field.cursor_position, ch);
                                            field.cursor_position += ch.len_utf8();
                                            field.state.set_value(field.value.clone());
                                            cx.notify();
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                FormFieldEntity::Checkbox(e) => {
                    // Space toggles checkbox
                    if key == "space" || key == " " {
                        e.update(cx, |checkbox, cx| {
                            checkbox.toggle(cx);
                        });
                    }
                }
            }
        }
    }

    /// Set the current field's input text programmatically.
    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some((_, entity)) = self.fields.get(self.focused_index) {
            match entity {
                FormFieldEntity::TextField(e) => {
                    let value = text.clone();
                    e.update(cx, |field, cx| {
                        field.set_value(value);
                        cx.notify();
                    });
                }
                FormFieldEntity::TextArea(e) => {
                    let value = text.clone();
                    e.update(cx, |field, cx| {
                        field.set_value(value);
                        cx.notify();
                    });
                }
                FormFieldEntity::Checkbox(_) => {}
            }
        }
    }
}

impl Render for FormPromptState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;

        // Focus the first field on initial render
        if !self.did_initial_focus && !self.fields.is_empty() {
            self.did_initial_focus = true;
            if let Some(focus_handle) = self.current_focus_handle(cx) {
                focus_handle.focus(window, cx);
                let is_focused = focus_handle.is_focused(window);
                logging::log(
                    "FORM",
                    &format!(
                        "Initial focus set on first field (is_focused={})",
                        is_focused
                    ),
                );
            }
        }

        // Build the form fields container
        let mut container = div().flex().flex_col().gap(px(16.)).w_full();

        for (_field_def, entity) in &self.fields {
            container = match entity {
                FormFieldEntity::TextField(e) => container.child(e.clone()),
                FormFieldEntity::TextArea(e) => container.child(e.clone()),
                FormFieldEntity::Checkbox(e) => container.child(e.clone()),
            };
        }

        // If no fields, show an error message
        if self.fields.is_empty() {
            container = container.child(
                div()
                    .p(px(16.))
                    .text_color(rgb(colors.label))
                    .child("No form fields found in HTML"),
            );
        }

        container
    }
}

/// Delegated Focusable implementation for FormPromptState.
///
/// This implements the "delegated focus" pattern from Zed's BufferSearchBar:
/// Instead of returning our own focus_handle, we return the focused field's handle.
/// This prevents the parent container from "stealing" focus from child fields during re-renders.
///
/// When GPUI asks "what should be focused?", we answer with the currently focused
/// text field's handle, so focus stays on the actual input field, not the form container.
impl Focusable for FormPromptState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        // Return the focused field's handle, not our own
        // This delegates focus management to the child field, preventing focus stealing
        if let Some((_, entity)) = self.fields.get(self.focused_index) {
            match entity {
                FormFieldEntity::TextField(e) => e.read(cx).get_focus_handle(),
                FormFieldEntity::TextArea(e) => e.read(cx).get_focus_handle(),
                FormFieldEntity::Checkbox(e) => e.read(cx).focus_handle(cx),
            }
        } else {
            // Fallback to our own handle if no fields exist
            self.focus_handle.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_values_json_serializes_string_values() {
        let values = vec![
            ("username".to_string(), "Bob".to_string()),
            ("bio".to_string(), "Hello".to_string()),
            ("subscribe".to_string(), "true".to_string()),
        ];
        let parsed: serde_json::Value =
            serde_json::from_str(&FormPromptState::build_values_json(values))
                .expect("values should be json");
        assert_eq!(
            parsed,
            json!({
                "bio": "Hello",
                "subscribe": "true",
                "username": "Bob"
            })
        );
    }
}
