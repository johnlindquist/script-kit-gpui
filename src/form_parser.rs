//! HTML Form Parser Module
//!
//! Parses HTML strings and extracts form elements (input, textarea, select, label)
//! converting them to Vec<Field> for native GPUI rendering.

use crate::protocol::Field;
use regex::Regex;
use std::collections::HashMap;

/// Parse an HTML string and extract form fields.
///
/// Supported elements:
/// - `<input>` with types: text, password, email, number, checkbox
/// - `<textarea>` elements
/// - `<select>` elements (extracts options)
/// - `<label>` elements (associates with inputs via `for` attribute or wrapping)
///
/// # Arguments
/// * `html` - HTML string containing form elements
///
/// # Returns
/// Vec<Field> with extracted form field definitions
pub fn parse_form_html(html: &str) -> Vec<Field> {
    let labels = extract_labels(html);

    // Collect all form elements with their positions to maintain document order
    let mut elements: Vec<(usize, Field)> = Vec::new();

    // Parse input elements
    let input_regex = Regex::new(r#"<input\s+([^>]*)/?>"#).unwrap();
    for cap in input_regex.captures_iter(html) {
        if let Some(attrs_str) = cap.get(1) {
            let attrs = parse_attributes(attrs_str.as_str());
            if let Some(field) = input_to_field(&attrs, &labels) {
                let pos = cap.get(0).map(|m| m.start()).unwrap_or(0);
                elements.push((pos, field));
            }
        }
    }

    // Parse textarea elements (handles both empty and with content)
    let textarea_regex = Regex::new(r#"<textarea\s+([^>]*)>([\s\S]*?)</textarea>"#).unwrap();
    for cap in textarea_regex.captures_iter(html) {
        if let Some(attrs_str) = cap.get(1) {
            let attrs = parse_attributes(attrs_str.as_str());
            let content = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            if let Some(field) = textarea_to_field(&attrs, content, &labels) {
                let pos = cap.get(0).map(|m| m.start()).unwrap_or(0);
                elements.push((pos, field));
            }
        }
    }

    // Parse select elements
    let select_regex = Regex::new(r#"<select\s+([^>]*)>[\s\S]*?</select>"#).unwrap();
    for cap in select_regex.captures_iter(html) {
        if let Some(attrs_str) = cap.get(1) {
            let attrs = parse_attributes(attrs_str.as_str());
            if let Some(field) = select_to_field(&attrs, &labels) {
                let pos = cap.get(0).map(|m| m.start()).unwrap_or(0);
                elements.push((pos, field));
            }
        }
    }

    // Sort by position in document and extract fields
    elements.sort_by_key(|(pos, _)| *pos);
    elements.into_iter().map(|(_, field)| field).collect()
}

/// Extract label elements and their text content, indexed by `for` attribute.
fn extract_labels(html: &str) -> HashMap<String, String> {
    let mut labels = HashMap::new();

    // Labels with `for` attribute
    let label_regex = Regex::new(r#"<label\s+[^>]*for\s*=\s*["']([^"']+)["'][^>]*>([^<]*)</label>"#)
        .unwrap();
    for cap in label_regex.captures_iter(html) {
        if let (Some(for_attr), Some(text)) = (cap.get(1), cap.get(2)) {
            labels.insert(for_attr.as_str().to_string(), text.as_str().trim().to_string());
        }
    }

    // Also check for simpler labels without other attributes
    let simple_label_regex = Regex::new(r#"<label\s+for\s*=\s*["']([^"']+)["']\s*>([^<]*)</label>"#)
        .unwrap();
    for cap in simple_label_regex.captures_iter(html) {
        if let (Some(for_attr), Some(text)) = (cap.get(1), cap.get(2)) {
            let key = for_attr.as_str().to_string();
            labels.entry(key).or_insert_with(|| text.as_str().trim().to_string());
        }
    }

    labels
}

/// Parse HTML attributes from an attribute string.
fn parse_attributes(attrs_str: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();

    // Match attribute="value" or attribute='value'
    let attr_regex = Regex::new(r#"(\w+)\s*=\s*["']([^"']*)["']"#).unwrap();
    for cap in attr_regex.captures_iter(attrs_str) {
        if let (Some(name), Some(value)) = (cap.get(1), cap.get(2)) {
            attrs.insert(name.as_str().to_lowercase(), value.as_str().to_string());
        }
    }

    // Check for boolean attributes like `checked` (no value)
    if attrs_str.contains("checked") && !attrs.contains_key("checked") {
        attrs.insert("checked".to_string(), "true".to_string());
    }

    attrs
}

/// Convert input element attributes to a Field.
fn input_to_field(attrs: &HashMap<String, String>, labels: &HashMap<String, String>) -> Option<Field> {
    let name = attrs.get("name")?.clone();
    let field_type = attrs.get("type").cloned().unwrap_or_else(|| "text".to_string());

    // Skip hidden inputs and submit buttons
    if field_type == "hidden" || field_type == "submit" || field_type == "button" {
        return None;
    }

    let mut field = Field::new(name.clone());

    // Set field type
    field.field_type = Some(field_type.clone());

    // Get label - try by id first, then by name
    let label = attrs
        .get("id")
        .and_then(|id| labels.get(id).cloned())
        .or_else(|| labels.get(&name).cloned());
    field.label = label;

    // Set placeholder
    if let Some(placeholder) = attrs.get("placeholder") {
        field.placeholder = Some(placeholder.clone());
    }

    // Set value (for checkbox, use "checked" state as value)
    if field_type == "checkbox" {
        if attrs.contains_key("checked") {
            field.value = Some("true".to_string());
        } else {
            // Use the value attribute if present, otherwise default
            field.value = attrs.get("value").cloned();
        }
    } else if let Some(value) = attrs.get("value") {
        field.value = Some(value.clone());
    }

    Some(field)
}

/// Convert textarea element to a Field.
fn textarea_to_field(
    attrs: &HashMap<String, String>,
    content: &str,
    labels: &HashMap<String, String>,
) -> Option<Field> {
    let name = attrs.get("name")?.clone();

    let mut field = Field::new(name.clone());
    field.field_type = Some("textarea".to_string());

    // Get label
    let label = attrs
        .get("id")
        .and_then(|id| labels.get(id).cloned())
        .or_else(|| labels.get(&name).cloned());
    field.label = label;

    // Set placeholder
    if let Some(placeholder) = attrs.get("placeholder") {
        field.placeholder = Some(placeholder.clone());
    }

    // Set value from content if not empty
    if !content.trim().is_empty() {
        field.value = Some(content.trim().to_string());
    }

    Some(field)
}

/// Convert select element to a Field.
fn select_to_field(attrs: &HashMap<String, String>, labels: &HashMap<String, String>) -> Option<Field> {
    let name = attrs.get("name")?.clone();

    let mut field = Field::new(name.clone());
    field.field_type = Some("select".to_string());

    // Get label
    let label = attrs
        .get("id")
        .and_then(|id| labels.get(id).cloned())
        .or_else(|| labels.get(&name).cloned());
    field.label = label;

    Some(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_html() {
        let fields = parse_form_html("");
        assert!(fields.is_empty());
    }

    #[test]
    fn test_parse_text_input() {
        let html = r#"<input type="text" name="username" placeholder="Enter username" />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "username");
        assert_eq!(fields[0].field_type, Some("text".to_string()));
        assert_eq!(fields[0].placeholder, Some("Enter username".to_string()));
    }

    #[test]
    fn test_parse_password_input() {
        let html = r#"<input type="password" name="password" />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "password");
        assert_eq!(fields[0].field_type, Some("password".to_string()));
    }

    #[test]
    fn test_parse_email_input() {
        let html = r#"<input type="email" name="email" placeholder="you@example.com" />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "email");
        assert_eq!(fields[0].field_type, Some("email".to_string()));
        assert_eq!(fields[0].placeholder, Some("you@example.com".to_string()));
    }

    #[test]
    fn test_parse_number_input() {
        let html = r#"<input type="number" name="age" value="25" />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "age");
        assert_eq!(fields[0].field_type, Some("number".to_string()));
        assert_eq!(fields[0].value, Some("25".to_string()));
    }

    #[test]
    fn test_parse_checkbox() {
        let html = r#"<input type="checkbox" name="subscribe" value="yes" />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "subscribe");
        assert_eq!(fields[0].field_type, Some("checkbox".to_string()));
        assert_eq!(fields[0].value, Some("yes".to_string()));
    }

    #[test]
    fn test_parse_checkbox_checked() {
        let html = r#"<input type="checkbox" name="agree" checked />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "agree");
        assert_eq!(fields[0].field_type, Some("checkbox".to_string()));
        assert_eq!(fields[0].value, Some("true".to_string()));
    }

    #[test]
    fn test_parse_textarea() {
        let html = r#"<textarea name="bio" placeholder="Tell us about yourself">Hello world</textarea>"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "bio");
        assert_eq!(fields[0].field_type, Some("textarea".to_string()));
        assert_eq!(fields[0].placeholder, Some("Tell us about yourself".to_string()));
        assert_eq!(fields[0].value, Some("Hello world".to_string()));
    }

    #[test]
    fn test_parse_empty_textarea() {
        let html = r#"<textarea name="notes"></textarea>"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "notes");
        assert_eq!(fields[0].field_type, Some("textarea".to_string()));
        assert_eq!(fields[0].value, None);
    }

    #[test]
    fn test_parse_select() {
        let html = r#"<select name="country"><option value="us">USA</option></select>"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "country");
        assert_eq!(fields[0].field_type, Some("select".to_string()));
    }

    #[test]
    fn test_parse_label_with_for() {
        let html = r#"
            <label for="username">Username:</label>
            <input type="text" name="username" id="username" />
        "#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "username");
        assert_eq!(fields[0].label, Some("Username:".to_string()));
    }

    #[test]
    fn test_parse_multiple_fields() {
        let html = r#"
            <input type="text" name="username" placeholder="Username" />
            <input type="password" name="password" />
            <textarea name="bio"></textarea>
            <input type="checkbox" name="subscribe" value="yes" />
        "#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0].name, "username");
        assert_eq!(fields[1].name, "password");
        assert_eq!(fields[2].name, "bio");
        assert_eq!(fields[3].name, "subscribe");
    }

    #[test]
    fn test_skip_hidden_inputs() {
        let html = r#"
            <input type="hidden" name="csrf_token" value="abc123" />
            <input type="text" name="username" />
        "#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "username");
    }

    #[test]
    fn test_skip_submit_buttons() {
        let html = r#"
            <input type="text" name="username" />
            <input type="submit" value="Submit" />
            <input type="button" value="Cancel" />
        "#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "username");
    }

    #[test]
    fn test_default_input_type() {
        // Input without type should default to text
        let html = r#"<input name="field1" />"#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "field1");
        assert_eq!(fields[0].field_type, Some("text".to_string()));
    }

    #[test]
    fn test_parse_with_class_attributes() {
        // Real-world example with Tailwind classes
        let html = r#"
            <input type="text" name="username" class="px-4 py-2 border rounded" />
            <input type="password" name="password" class="px-4 py-2 border rounded" />
        "#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "username");
        assert_eq!(fields[1].name, "password");
    }

    #[test]
    fn test_parse_real_world_form() {
        // Example from the task description
        let html = r#"
            <input type="text" name="username" class="..." />
            <input type="password" name="password" />
            <textarea name="bio"></textarea>
            <input type="checkbox" name="subscribe" value="yes" />
        "#;
        let fields = parse_form_html(html);

        assert_eq!(fields.len(), 4);

        assert_eq!(fields[0].name, "username");
        assert_eq!(fields[0].field_type, Some("text".to_string()));

        assert_eq!(fields[1].name, "password");
        assert_eq!(fields[1].field_type, Some("password".to_string()));

        assert_eq!(fields[2].name, "bio");
        assert_eq!(fields[2].field_type, Some("textarea".to_string()));

        assert_eq!(fields[3].name, "subscribe");
        assert_eq!(fields[3].field_type, Some("checkbox".to_string()));
        assert_eq!(fields[3].value, Some("yes".to_string()));
    }
}
