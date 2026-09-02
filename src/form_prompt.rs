use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, ElementId, Entity, FocusHandle, Focusable,
    KeyDownEvent, Render, Window,
};

use crate::components::{FormCheckbox, FormFieldColors, FormTextArea, FormTextField};
use crate::{form_parser, logging, protocol};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormPromptOutputMode {
    ObjectByName,
    ArrayByOrder,
}

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
    focus_revision: u64,
    /// Focus handle for this form.
    pub focus_handle: FocusHandle,
    /// Output contract for SDK resolution.
    pub output_mode: FormPromptOutputMode,
    pub(crate) validation_error: Option<String>,
}

impl FormPromptState {
    /// Aggregate source epochs without hashing field contents or mutating on reads.
    /// Include every field so changing focus cannot exchange one field's epoch for another.
    pub fn dictation_input_revision(&self, cx: &App) -> u64 {
        self.fields
            .iter()
            .fold(self.focus_revision, |revision, (_, field)| {
                let field_revision = match field {
                    FormFieldEntity::TextField(entity) => entity.read(cx).revision(),
                    FormFieldEntity::TextArea(entity) => entity.read(cx).revision(),
                    FormFieldEntity::Checkbox(entity) => entity.read(cx).revision(),
                };
                assert!(
                    revision <= u64::MAX - field_revision,
                    "form input revision exhausted"
                );
                revision + field_revision
            })
    }

    pub fn dictation_input_len(&self, cx: &App) -> Option<usize> {
        match &self.fields.get(self.focused_index)?.1 {
            FormFieldEntity::TextField(entity) => Some(entity.read(cx).value().len()),
            FormFieldEntity::TextArea(entity) => Some(entity.read(cx).value().len()),
            FormFieldEntity::Checkbox(_) => None,
        }
    }

    fn set_focused_index(&mut self, index: usize) {
        if self.focused_index != index {
            assert!(
                self.focus_revision < u64::MAX,
                "form focus revision exhausted"
            );
            self.focus_revision += 1;
            self.focused_index = index;
        }
    }

    pub fn update_theme(&mut self, theme: &crate::theme::Theme, cx: &mut Context<Self>) {
        let colors = FormFieldColors::from_theme(theme);
        self.colors = colors;
        for (_, field) in &self.fields {
            match field {
                FormFieldEntity::TextField(entity) => {
                    entity.update(cx, |field, cx| field.update_colors(colors, cx))
                }
                FormFieldEntity::TextArea(entity) => {
                    entity.update(cx, |field, cx| field.update_colors(colors, cx))
                }
                FormFieldEntity::Checkbox(entity) => {
                    entity.update(cx, |field, cx| field.update_colors(colors, cx))
                }
            }
        }
        cx.notify();
    }

    pub fn field_values(&self, cx: &App) -> Vec<(String, String)> {
        self.fields
            .iter()
            .map(|(field, entity)| (field.name.clone(), Self::field_value(entity, cx)))
            .collect()
    }

    pub fn focused_value(&self, cx: &App) -> Option<String> {
        self.fields
            .get(self.focused_index)
            .map(|(_, entity)| Self::field_value(entity, cx))
    }
    pub fn validated_submit_value(&mut self, cx: &mut Context<Self>) -> Option<String> {
        self.validation_error = self.submit_validation_message(cx);
        cx.notify();
        self.validation_error
            .is_none()
            .then(|| self.collect_values(cx))
    }

    fn build_values_json(values: impl IntoIterator<Item = (String, String)>) -> String {
        let mut map = serde_json::Map::new();
        for (key, value) in values {
            map.insert(key, serde_json::Value::String(value));
        }
        serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
    }

    fn build_field_entities(
        parsed_fields: Vec<protocol::Field>,
        colors: FormFieldColors,
        cx: &mut App,
    ) -> Vec<(protocol::Field, FormFieldEntity)> {
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

                let normalized_type = field_type.to_ascii_lowercase();
                let entity = match normalized_type.as_str() {
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

        fields
    }

    fn from_fields_with_mode(
        id: String,
        html: String,
        parsed_fields: Vec<protocol::Field>,
        colors: FormFieldColors,
        output_mode: FormPromptOutputMode,
        cx: &mut App,
    ) -> Self {
        let fields = Self::build_field_entities(parsed_fields, colors, cx);

        Self {
            id,
            html,
            fields,
            colors,
            focused_index: 0,
            focus_revision: 0,
            focus_handle: cx.focus_handle(),
            output_mode,
            validation_error: None,
        }
    }

    /// Create a new form prompt state from HTML.
    pub fn new(id: String, html: String, colors: FormFieldColors, cx: &mut App) -> Self {
        let parsed_fields = form_parser::parse_form_html(&html);

        logging::log(
            "FORM",
            &format!("Parsed {} form fields from HTML", parsed_fields.len()),
        );

        Self::from_fields_with_mode(
            id,
            html,
            parsed_fields,
            colors,
            FormPromptOutputMode::ObjectByName,
            cx,
        )
    }

    /// Create a form prompt state from SDK fields() definitions.
    pub fn from_fields(
        id: String,
        fields: Vec<protocol::Field>,
        colors: FormFieldColors,
        cx: &mut App,
    ) -> Self {
        logging::log(
            "FORM",
            &format!("Creating fields() prompt with {} fields", fields.len()),
        );

        Self::from_fields_with_mode(
            id,
            String::new(),
            fields,
            colors,
            FormPromptOutputMode::ArrayByOrder,
            cx,
        )
    }

    pub fn prompt_type(&self) -> &'static str {
        match self.output_mode {
            FormPromptOutputMode::ObjectByName => "form",
            FormPromptOutputMode::ArrayByOrder => "fields",
        }
    }

    pub fn semantic_prefix(&self) -> &'static str {
        match self.output_mode {
            FormPromptOutputMode::ObjectByName => "form",
            FormPromptOutputMode::ArrayByOrder => "fields",
        }
    }

    fn field_value(entity: &FormFieldEntity, cx: &App) -> String {
        match entity {
            FormFieldEntity::TextField(e) => e.read(cx).value().to_string(),
            FormFieldEntity::TextArea(e) => e.read(cx).value().to_string(),
            FormFieldEntity::Checkbox(e) => {
                if e.read(cx).is_checked() {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
        }
    }

    /// Get all field values using the SDK contract for this prompt source.
    pub fn collect_values(&self, cx: &App) -> String {
        match self.output_mode {
            FormPromptOutputMode::ObjectByName => {
                let values = self.fields.iter().map(|(field_def, entity)| {
                    (field_def.name.clone(), Self::field_value(entity, cx))
                });
                Self::build_values_json(values)
            }
            FormPromptOutputMode::ArrayByOrder => {
                let values: Vec<String> = self
                    .fields
                    .iter()
                    .map(|(_, entity)| Self::field_value(entity, cx))
                    .collect();
                serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string())
            }
        }
    }

    pub fn submit_validation_errors(&self, cx: &App) -> Vec<String> {
        let mut invalid_fields = Vec::new();

        for (field_definition, field_entity) in &self.fields {
            let value = Self::field_value(field_entity, cx);
            if field_value_is_valid_for_submit(field_definition.field_type.as_deref(), &value) {
                continue;
            }

            let field_type = field_definition
                .field_type
                .as_deref()
                .unwrap_or("text")
                .to_string();
            invalid_fields.push(format!("{} ({})", field_definition.name, field_type));
        }

        invalid_fields
    }

    pub fn submit_validation_message(&self, cx: &App) -> Option<String> {
        let invalid_fields = self.submit_validation_errors(cx);
        if invalid_fields.is_empty() {
            return None;
        }

        Some(if invalid_fields.len() == 1 {
            format!("Fix invalid field before submitting: {}", invalid_fields[0])
        } else {
            format!(
                "Fix invalid fields before submitting: {}",
                invalid_fields.join(", ")
            )
        })
    }

    /// Focus the next field (for Tab navigation).
    pub fn focus_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fields.is_empty() {
            return;
        }
        let next = (self.focused_index + 1) % self.fields.len();
        self.focus_field_at(next, window, cx);
    }

    /// Focus the previous field (for Shift+Tab navigation).
    pub fn focus_previous(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.fields.is_empty() {
            return;
        }
        let previous = if self.focused_index == 0 {
            self.fields.len() - 1
        } else {
            self.focused_index - 1
        };
        self.focus_field_at(previous, window, cx);
    }

    fn focus_handle_at(&self, index: usize, cx: &App) -> Option<FocusHandle> {
        self.fields.get(index).map(|(_, entity)| match entity {
            FormFieldEntity::TextField(e) => e.read(cx).focus_handle(cx),
            FormFieldEntity::TextArea(e) => e.read(cx).focus_handle(cx),
            FormFieldEntity::Checkbox(e) => e.read(cx).focus_handle(cx),
        })
    }

    fn focus_field_at(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.fields.is_empty() {
            return;
        }
        self.set_focused_index(index.min(self.fields.len() - 1));
        if let Some(focus_handle) = self.focus_handle_at(self.focused_index, cx) {
            focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub fn focus_field_by_semantic_id(&mut self, semantic_id: &str) -> Option<String> {
        let semantic_prefix = self.semantic_prefix();
        let index = self
            .fields
            .iter()
            .enumerate()
            .find_map(|(index, (field, _))| {
                let semantic_name = format!("{semantic_prefix}-{}", field.name);
                let field_semantic_id =
                    protocol::generate_semantic_id_named("input", &semantic_name);
                (field_semantic_id == semantic_id).then_some(index)
            })?;

        self.set_focused_index(index);
        self.fields
            .get(index)
            .map(|(field, _)| field.label.clone().unwrap_or_else(|| field.name.clone()))
    }

    /// Get the focus handle for the currently focused field.
    pub fn current_focus_handle(&self, cx: &App) -> Option<FocusHandle> {
        self.focus_handle_at(self.focused_index, cx)
    }

    /// Handle keyboard input by forwarding to the currently focused field.
    ///
    /// This forwards key events to the field's unified `handle_key_event` method
    /// which properly handles:
    /// - Char-based cursor positioning (not byte-based)
    /// - Modifier keys (Cmd/Ctrl+C/V/X/A work correctly)
    /// - Selection with Shift+Arrow
    /// - Clipboard operations
    pub fn handle_key_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if let Some((_, entity)) = self.fields.get(self.focused_index) {
            match entity {
                FormFieldEntity::TextField(e) => {
                    e.update(cx, |field, cx| {
                        field.handle_key_event(event, cx);
                    });
                }
                FormFieldEntity::TextArea(e) => {
                    e.update(cx, |field, cx| {
                        field.handle_key_event(event, cx);
                    });
                }
                FormFieldEntity::Checkbox(e) => {
                    // Space toggles checkbox
                    let key = event.keystroke.key.as_str();
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

#[inline]
fn is_valid_email_submit_value(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }

    if value
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return false;
    }

    let mut parts = value.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();

    if local.is_empty() || domain.is_empty() || parts.next().is_some() {
        return false;
    }

    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }

    domain.contains('.')
}

#[inline]
fn is_valid_number_submit_value(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    trimmed.parse::<f64>().is_ok()
}

#[inline]
fn field_value_is_valid_for_submit(field_type: Option<&str>, value: &str) -> bool {
    match field_type {
        Some(field_type) if field_type.eq_ignore_ascii_case("email") => {
            is_valid_email_submit_value(value)
        }
        Some(field_type) if field_type.eq_ignore_ascii_case("number") => {
            is_valid_number_submit_value(value)
        }
        _ => true,
    }
}

impl Render for FormPromptState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        tracing::info!(
            surface = %format!("{}_prompt", self.semantic_prefix()),
            field_count = self.fields.len(),
            focused_index = self.focused_index,
            "prompt_surface_rendered"
        );

        let colors = self.colors;

        // Ensure the currently selected field is the keyboard focus target.
        if let Some(focus_handle) = self.current_focus_handle(cx) {
            if !focus_handle.is_focused(window) {
                focus_handle.focus(window, cx);
            }
        }

        // Build the form fields container
        let mut container = div().flex().flex_col().gap(px(16.)).w_full();
        if let Some(error) = &self.validation_error {
            container = container.child(
                div()
                    .id("form-validation-error")
                    .text_sm()
                    .text_color(gpui::rgb(colors.error))
                    .child(error.clone()),
            );
        }

        for (index, (_field_def, entity)) in self.fields.iter().enumerate() {
            let focus_slot_click = cx.listener(
                move |this: &mut Self,
                      _event: &ClickEvent,
                      window: &mut Window,
                      cx: &mut Context<Self>| {
                    this.focus_field_at(index, window, cx);
                },
            );
            let slot = div()
                .id(ElementId::Name(format!("form-field-slot-{index}").into()))
                .w_full()
                .on_click(focus_slot_click);
            container = match entity {
                FormFieldEntity::TextField(e) => container.child(slot.child(e.clone())),
                FormFieldEntity::TextArea(e) => container.child(slot.child(e.clone())),
                FormFieldEntity::Checkbox(e) => container.child(slot.child(e.clone())),
            };
        }

        // If no fields, show an error message
        if self.fields.is_empty() {
            container = container.child(
                div()
                    .p(px(16.))
                    .text_color(colors.label)
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

#[cfg(test)]
#[gpui::test]
fn form_dictation_epoch_tracks_field_and_focus_aba(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let colors = FormFieldColors::from_theme(&crate::theme::Theme::default());
    let form = cx.new(|cx| FormPromptState::new("epoch".into(),
        "<input name='first' value='cat'><textarea name='second'>hé</textarea><input name='check' type='checkbox'>".into(), colors, cx));
    form.update(cx, |form, cx| {
        let initial = form.dictation_input_revision(cx);
        assert_eq!(form.dictation_input_len(cx), Some(3));
        form.set_input("cat".into(), cx);
        form.update_theme(&crate::theme::Theme::default(), cx);
        assert_eq!(form.dictation_input_revision(cx), initial);
        form.set_input("dog".into(), cx);
        let changed = form.dictation_input_revision(cx);
        assert!(changed > initial);
        form.set_input("cat".into(), cx);
        assert!(form.dictation_input_revision(cx) > changed);
        let before_focus = form.dictation_input_revision(cx);
        form.set_focused_index(1);
        form.set_input("hé".into(), cx);
        assert_eq!(form.dictation_input_len(cx), Some("hé".len()));
        form.set_focused_index(0);
        assert_eq!(form.focused_value(cx).as_deref(), Some("cat"));
        assert!(form.dictation_input_revision(cx) > before_focus);
        let focused = form.dictation_input_revision(cx);
        form.set_focused_index(0);
        form.field_values(cx);
        assert_eq!(form.dictation_input_revision(cx), focused);
        form.set_focused_index(2);
        assert_eq!(form.dictation_input_len(cx), None);
        let checkbox_epoch = form.dictation_input_revision(cx);
        let FormFieldEntity::Checkbox(checkbox) = form.fields[2].1.clone() else {
            panic!("checkbox fixture")
        };
        checkbox.update(cx, |checkbox, cx| {
            checkbox.set_checked(false, cx);
            assert_eq!(checkbox.revision(), 0);
            checkbox.toggle(cx);
            checkbox.toggle(cx);
            assert!(!checkbox.is_checked());
        });
        assert!(form.dictation_input_revision(cx) > checkbox_epoch);
    });
}
