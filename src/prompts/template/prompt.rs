use super::*;
use itertools::Itertools;

/// TemplatePrompt - Tab-through template editor
///
/// Allows editing template strings with {{placeholder}} syntax.
/// Tab moves between placeholders, Enter submits the filled template.
pub struct TemplatePrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Original template string with placeholders
    pub template: String,
    /// Parsed input placeholders (unique, in order of appearance)
    pub inputs: Vec<TemplateInput>,
    /// Current values for each input
    pub values: Vec<String>,
    /// Per-field validation errors
    pub validation_errors: Vec<Option<String>>,
    /// Currently focused input index
    pub current_input: usize,
    input_revision: u64,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
}

#[derive(Debug)]
pub(super) struct TemplatePlaceholderMatch {
    start: usize,
    end: usize,
    name: String,
}

impl TemplatePrompt {
    pub(crate) fn dictation_input_revision(&self, _cx: &gpui::App) -> u64 {
        self.input_revision
    }

    fn advance_input_revision(&mut self) {
        assert!(
            self.input_revision < u64::MAX,
            "template input revision exhausted"
        );
        self.input_revision += 1;
    }

    pub(super) fn set_current_input(&mut self, index: usize) {
        if self.current_input != index {
            self.advance_input_revision();
            self.current_input = index;
        }
    }

    pub fn new(
        id: String,
        template: String,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        logging::log(
            "PROMPTS",
            &format!("TemplatePrompt::new template: {}", template),
        );

        // Parse inputs from template
        let inputs = Self::parse_template_inputs(&template);
        let values: Vec<String> = inputs.iter().map(|_| String::new()).collect();
        let validation_errors: Vec<Option<String>> = inputs.iter().map(|_| None).collect();

        TemplatePrompt {
            id,
            template,
            inputs,
            values,
            validation_errors,
            current_input: 0,
            input_revision: 0,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
        }
    }

    /// Parse template string to extract {{name}} placeholders
    /// Returns unique placeholders in order of first appearance
    pub(super) fn parse_template_inputs(template: &str) -> Vec<TemplateInput> {
        template_variables::extract_variable_names(template)
            .into_iter()
            .map(|name| TemplateInput {
                label: Self::label_for_field(&name),
                placeholder: Self::placeholder_for_field(&name),
                group: Self::group_for_field(&name),
                required: Self::is_required_field(&name),
                name,
            })
            .collect()
    }

    pub(super) fn is_supported_placeholder(raw_placeholder: &str, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        if raw_placeholder.starts_with("{{") {
            return !name.starts_with('#') && !name.starts_with('/') && name != "else";
        }

        !name.chars().any(char::is_whitespace) && !name.contains('(') && !name.contains(')')
    }

    pub(super) fn parse_placeholder_matches(template: &str) -> Vec<TemplatePlaceholderMatch> {
        let placeholder_re = match Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}|\$\{([^}]+)\}") {
            Ok(regex) => regex,
            Err(error) => {
                logging::log(
                    "PROMPTS",
                    &format!(
                        "TemplatePrompt::parse_placeholder_matches regex compile failed: {error}"
                    ),
                );
                return Vec::new();
            }
        };
        let mut matches = Vec::new();

        for captures in placeholder_re.captures_iter(template) {
            let Some(full_match) = captures.get(0) else {
                continue;
            };
            let Some(name_match) = captures.get(1).or_else(|| captures.get(2)) else {
                continue;
            };

            let name = name_match.as_str().trim();
            if !Self::is_supported_placeholder(full_match.as_str(), name) {
                continue;
            }

            matches.push(TemplatePlaceholderMatch {
                start: full_match.start(),
                end: full_match.end(),
                name: name.to_string(),
            });
        }

        matches
    }

    pub(super) fn render_template_single_pass<F>(
        template: &str,
        mut render_placeholder: F,
    ) -> String
    where
        F: FnMut(&str, &str) -> String,
    {
        let matches = Self::parse_placeholder_matches(template);
        if matches.is_empty() {
            return template.to_string();
        }

        let mut result = String::with_capacity(template.len());
        let mut cursor = 0;

        for placeholder_match in matches {
            if placeholder_match.start > cursor {
                result.push_str(&template[cursor..placeholder_match.start]);
            }

            let raw_placeholder = &template[placeholder_match.start..placeholder_match.end];
            result.push_str(&render_placeholder(
                &placeholder_match.name,
                raw_placeholder,
            ));
            cursor = placeholder_match.end;
        }

        if cursor < template.len() {
            result.push_str(&template[cursor..]);
        }

        result
    }

    pub(super) fn label_for_field(name: &str) -> String {
        let normalized = name.to_lowercase();
        match normalized.as_str() {
            "script_name" => "Script Name".to_string(),
            "extension_name" => "Scriptlet Bundle Name".to_string(),
            "name" => "Name".to_string(),
            "author" => "Author".to_string(),
            "description" => "Description".to_string(),
            "icon" => "Icon".to_string(),
            _ => normalized
                .split('_')
                .filter(|part| !part.is_empty())
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => {
                            let mut word = first.to_uppercase().to_string();
                            word.push_str(chars.as_str());
                            word
                        }
                        None => String::new(),
                    }
                })
                .join(" "),
        }
    }

    pub(super) fn placeholder_for_field(name: &str) -> String {
        let normalized = name.to_lowercase();
        match normalized.as_str() {
            "script_name" => "my-script-name".to_string(),
            "extension_name" => "my-scriptlet-bundle".to_string(),
            "name" => "My Bundle".to_string(),
            "author" => "Your Name".to_string(),
            "description" => "What this template creates".to_string(),
            "icon" => "wrench".to_string(),
            _ if normalized.contains("name") || normalized.contains("slug") => {
                "my-script-name".to_string()
            }
            _ => format!("Enter {}", normalized.replace('_', " ")),
        }
    }

    pub(super) fn group_for_field(name: &str) -> String {
        let normalized = name.to_lowercase();
        if normalized.contains("name") || normalized.contains("slug") {
            "Naming".to_string()
        } else if normalized.contains("author")
            || normalized.contains("description")
            || normalized.contains("icon")
            || normalized.contains("tag")
        {
            "Metadata".to_string()
        } else if normalized.contains("content")
            || normalized.contains("body")
            || normalized.contains("template")
            || normalized.contains("command")
        {
            "Content".to_string()
        } else {
            "Details".to_string()
        }
    }

    pub(super) fn is_required_field(name: &str) -> bool {
        let normalized = name.to_lowercase();
        normalized == "script_name"
            || normalized == "extension_name"
            || normalized == "name"
            || normalized.contains("slug")
    }

    pub(super) fn is_slug_field(name: &str) -> bool {
        let normalized = name.to_lowercase();
        normalized == "script_name"
            || normalized == "extension_name"
            || normalized.contains("slug")
            || normalized.ends_with("_slug")
    }

    pub(super) fn is_name_field(name: &str) -> bool {
        let normalized = name.to_lowercase();
        Self::is_slug_field(name) || normalized == "name" || normalized.ends_with("_name")
    }

    pub(super) fn is_slug_like(value: &str) -> bool {
        if value.is_empty() || value.starts_with('-') || value.ends_with('-') {
            return false;
        }

        let mut previous_hyphen = false;
        for ch in value.chars() {
            if ch == '-' {
                if previous_hyphen {
                    return false;
                }
                previous_hyphen = true;
                continue;
            }

            if !ch.is_ascii_lowercase() && !ch.is_ascii_digit() {
                return false;
            }
            previous_hyphen = false;
        }

        true
    }

    pub fn validate_input_value(input: &TemplateInput, raw_value: &str) -> Result<(), String> {
        let value = raw_value.trim();

        if input.required && value.is_empty() {
            return Err(format!("{} is required", input.label));
        }

        if value.is_empty() {
            return Ok(());
        }

        if Self::is_slug_field(&input.name) && !Self::is_slug_like(value) {
            return Err(format!(
                "{} must use lowercase letters, numbers, and hyphens",
                input.label
            ));
        }

        Ok(())
    }

    pub(super) fn validate_all_inputs(&mut self) -> bool {
        let mut is_valid = true;

        for idx in 0..self.inputs.len() {
            let value = self.values.get(idx).map(String::as_str).unwrap_or_default();
            let validation = Self::validate_input_value(&self.inputs[idx], value);
            let error = validation.err();
            if self.validation_errors[idx] != error {
                self.validation_errors[idx] = error;
                self.advance_input_revision();
            }
            if self.validation_errors[idx].is_some() {
                is_valid = false;
            }
        }

        is_valid
    }

    /// Get the filled template string by replacing all placeholders
    pub fn filled_template(&self) -> String {
        let values_by_name: HashMap<&str, &str> = self
            .inputs
            .iter()
            .zip(self.values.iter())
            .map(|(input, value)| (input.name.as_str(), value.as_str()))
            .collect();

        Self::render_template_single_pass(&self.template, |name, raw_placeholder| {
            match values_by_name.get(name).copied() {
                Some(value) if !value.is_empty() => value.to_string(),
                _ => raw_placeholder.to_string(),
            }
        })
    }

    /// Get the preview string - shows filled values or placeholder hints
    pub(super) fn preview_template(&self) -> String {
        let values_by_name: HashMap<&str, &str> = self
            .inputs
            .iter()
            .zip(self.values.iter())
            .map(|(input, value)| (input.name.as_str(), value.as_str()))
            .collect();
        let labels_by_name: HashMap<&str, &str> = self
            .inputs
            .iter()
            .map(|input| (input.name.as_str(), input.label.as_str()))
            .collect();

        Self::render_template_single_pass(&self.template, |name, raw_placeholder| {
            match values_by_name.get(name).copied() {
                Some(value) if !value.is_empty() => value.to_string(),
                Some(_) => {
                    let label = labels_by_name.get(name).copied().unwrap_or(name);
                    format!("[{}]", label)
                }
                None => raw_placeholder.to_string(),
            }
        })
    }

    /// Set the current input value programmatically
    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(value) = self.values.get_mut(self.current_input) {
            if *value == text {
                return;
            }
            *value = text;
            if let Some(input) = self.inputs.get(self.current_input) {
                self.validation_errors[self.current_input] =
                    Self::validate_input_value(input, value).err();
            }
            self.advance_input_revision();
            cx.notify();
        }
    }

    /// Submit the filled template
    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.validate_all_inputs() {
            if let Some(first_invalid) = self.validation_errors.iter().position(Option::is_some) {
                self.set_current_input(first_invalid);
            }
            cx.notify();
            return;
        }

        // Replace placeholders with actual values for final submission in a single pass.
        let values_by_name: HashMap<&str, &str> = self
            .inputs
            .iter()
            .zip(self.values.iter())
            .map(|(input, value)| (input.name.as_str(), value.as_str()))
            .collect();
        let result = Self::render_template_single_pass(&self.template, |name, raw_placeholder| {
            values_by_name
                .get(name)
                .map(|value| value.trim().to_string())
                .unwrap_or_else(|| raw_placeholder.to_string())
        });
        (self.on_submit)(self.id.clone(), Some(result));
        cx.notify();
    }

    /// Cancel - submit None
    pub(crate) fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    /// Move to next input (Tab)
    pub(crate) fn next_input(&mut self, cx: &mut Context<Self>) {
        if !self.inputs.is_empty() {
            self.set_current_input((self.current_input + 1) % self.inputs.len());
            cx.notify();
        }
    }

    /// Move to previous input (Shift+Tab)
    pub(crate) fn prev_input(&mut self, cx: &mut Context<Self>) {
        if !self.inputs.is_empty() {
            if self.current_input == 0 {
                self.set_current_input(self.inputs.len() - 1);
            } else {
                self.set_current_input(self.current_input - 1);
            }
            cx.notify();
        }
    }

    /// Append committed text to the current field, ignoring control characters.
    pub(crate) fn handle_text(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        if let Some(value) = self.values.get_mut(self.current_input) {
            let previous_len = value.len();
            value.extend(text.chars().filter(|ch| !ch.is_control()));
            if value.len() == previous_len {
                return false;
            }
            if let Some(input) = self.inputs.get(self.current_input) {
                self.validation_errors[self.current_input] =
                    Self::validate_input_value(input, value).err();
            }
            self.advance_input_revision();
            cx.notify();
            return true;
        }
        false
    }

    /// Handle backspace for current field
    pub(crate) fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(value) = self.values.get_mut(self.current_input) {
            if !value.is_empty() {
                value.pop();
                if let Some(input) = self.inputs.get(self.current_input) {
                    self.validation_errors[self.current_input] =
                        Self::validate_input_value(input, value).err();
                }
                self.advance_input_revision();
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod seeded_behavior_tests {
    use super::*;
    use gpui::{Entity, KeyDownEvent, Keystroke, PlatformInput, TestAppContext, WindowHandle};

    fn template_window(
        cx: &mut TestAppContext,
        template: &str,
        on_submit: SubmitCallback,
    ) -> WindowHandle<TemplatePrompt> {
        cx.update(gpui_component::init);
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| {
                    TemplatePrompt::new(
                        "template-local".into(),
                        template.into(),
                        cx.focus_handle(),
                        on_submit,
                        Arc::new(theme::Theme::default()),
                    )
                })
            })
            .expect("template test window should open")
        });
        window
            .update(cx, |prompt, window, cx| {
                window.focus(&prompt.focus_handle, cx);
            })
            .expect("template test window should focus");
        cx.run_until_parked();
        window
    }

    fn dispatch_template_key<V: Render + 'static>(
        cx: &mut TestAppContext,
        window: WindowHandle<V>,
        key: &str,
        text: Option<&str>,
    ) -> bool {
        let mut keystroke = Keystroke::parse(key).expect("valid template test key");
        keystroke.key_char = text.map(str::to_owned);
        let consumed = cx
            .update_window(*window, |_, window, cx| {
                !window
                    .dispatch_event(
                        PlatformInput::KeyDown(KeyDownEvent {
                            keystroke,
                            is_held: false,
                            prefer_character_input: false,
                        }),
                        cx,
                    )
                    .propagate
            })
            .expect("template event should dispatch");
        cx.run_until_parked();
        consumed
    }

    struct TemplateWithTabStop {
        prompt: Entity<TemplatePrompt>,
        competing_focus: FocusHandle,
    }

    impl Render for TemplateWithTabStop {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(self.prompt.clone()).child(
                div()
                    .track_focus(&self.competing_focus.clone().tab_stop(true))
                    .child("Competing tab stop"),
            )
        }
    }

    #[gpui::test]
    fn template_traversal_preserves_values_and_validates_before_submit(cx: &mut TestAppContext) {
        let submitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let output = submitted.clone();
        cx.update(gpui_component::init);
        let (window, prompt) = cx.update(|cx| {
            let prompt = cx.new(|cx| {
                TemplatePrompt::new(
                    "template-local".into(),
                    "Hello {{script_name}}, {{email}}".into(),
                    cx.focus_handle(),
                    Arc::new(move |_, value| output.lock().push(value)),
                    Arc::new(theme::Theme::default()),
                )
            });
            let host = cx.new(|cx| TemplateWithTabStop {
                prompt: prompt.clone(),
                competing_focus: cx.focus_handle(),
            });
            let window = cx
                .open_window(Default::default(), |window, cx| {
                    cx.new(|cx| gpui_component::Root::new(host, window, cx))
                })
                .expect("Root-wrapped template should open");
            (window, prompt)
        });
        window
            .update(cx, |_, window, cx| {
                let focus = prompt.read(cx).focus_handle.clone();
                window.focus(&focus, cx);
            })
            .unwrap();
        cx.run_until_parked();

        assert!(dispatch_template_key(cx, window, "enter", None));
        assert!(submitted.lock().is_empty());
        prompt.read_with(cx, |prompt, _| {
            assert!(prompt.validation_errors[0].is_some());
        });
        assert!(dispatch_template_key(
            cx,
            window,
            "e",
            Some("edited-fixture")
        ));
        assert!(dispatch_template_key(cx, window, "tab", None));
        window
            .update(cx, |_, window, cx| {
                let prompt = prompt.read(cx);
                assert_eq!(prompt.current_input, 1);
                assert!(prompt.focus_handle.is_focused(window));
                assert!(prompt.validation_errors[0].is_none());
            })
            .unwrap();
        assert!(dispatch_template_key(
            cx,
            window,
            "a",
            Some("ada@example.invalid"),
        ));
        assert!(dispatch_template_key(cx, window, "shift-tab", None));
        window
            .update(cx, |_, window, cx| {
                let prompt = prompt.read(cx);
                assert_eq!(prompt.current_input, 0);
                assert!(prompt.focus_handle.is_focused(window));
                assert_eq!(prompt.values, ["edited-fixture", "ada@example.invalid"]);
            })
            .unwrap();
        assert!(dispatch_template_key(cx, window, "enter", None));
        assert_eq!(
            *submitted.lock(),
            vec![Some("Hello edited-fixture, ada@example.invalid".into())]
        );
    }

    #[gpui::test]
    fn template_committed_unicode_filters_controls_and_revises_once(cx: &mut TestAppContext) {
        let window = template_window(cx, "{{name}}", Arc::new(|_, _| {}));
        let unicode = "e\u{301}東京\u{1f980}";
        assert!(dispatch_template_key(cx, window, "e", Some(unicode)));
        window
            .read_with(cx, |prompt, cx| {
                assert_eq!(prompt.values[0], unicode);
                assert_eq!(prompt.dictation_input_revision(cx), 1);
            })
            .unwrap();
        for text in ["", "\n\r\t\u{7f}"] {
            assert!(!dispatch_template_key(cx, window, "a", Some(text)));
        }
        assert!(!dispatch_template_key(cx, window, "a", None));
        window
            .read_with(cx, |prompt, cx| {
                assert_eq!(prompt.values[0], unicode);
                assert_eq!(prompt.dictation_input_revision(cx), 1);
            })
            .unwrap();
        assert!(dispatch_template_key(cx, window, "backspace", None));
        assert!(dispatch_template_key(cx, window, "alt-e", Some("é")));
        assert!(dispatch_template_key(cx, window, "a", Some("\nA\tB\u{7f}")));
        window
            .read_with(cx, |prompt, cx| {
                assert_eq!(prompt.values[0], "e\u{301}東京éAB");
                assert_eq!(prompt.dictation_input_revision(cx), 4);
            })
            .unwrap();
    }

    #[gpui::test]
    fn template_passes_shortcuts_without_mutating_or_submitting(cx: &mut TestAppContext) {
        let submitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let output = submitted.clone();
        let window = template_window(
            cx,
            "{{name}} {{email}}",
            Arc::new(move |_, value| output.lock().push(value)),
        );
        for key in ["cmd-w", "cmd-k", "cmd-q", "cmd-v", "ctrl-a", "fn-a"] {
            assert!(!dispatch_template_key(cx, window, key, Some("ignored")));
        }
        for key in [
            "cmd-tab",
            "ctrl-tab",
            "alt-tab",
            "cmd-enter",
            "ctrl-enter",
            "alt-enter",
            "alt-backspace",
        ] {
            assert!(!dispatch_template_key(cx, window, key, None));
        }
        window
            .update(cx, |prompt, window, cx| {
                assert_eq!(prompt.values, ["", ""]);
                assert_eq!(prompt.current_input, 0);
                assert_eq!(prompt.dictation_input_revision(cx), 0);
                assert!(prompt.focus_handle.is_focused(window));
            })
            .unwrap();
        assert!(submitted.lock().is_empty());
    }
}

#[cfg(test)]
#[gpui::test]
fn template_revision_tracks_field_focus_and_text_aba(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let prompt = cx.new(|cx| {
        TemplatePrompt::new(
            "epoch".into(),
            "{{name}} {{author}}".into(),
            cx.focus_handle(),
            Arc::new(|_, _| {}),
            Arc::new(theme::Theme::default()),
        )
    });
    prompt.update(cx, |prompt, cx| {
        let initial = prompt.dictation_input_revision(cx);
        prompt.set_input(String::new(), cx);
        prompt.handle_backspace(cx);
        prompt.set_current_input(0);
        assert_eq!(prompt.dictation_input_revision(cx), initial);
        prompt.set_input("cat".into(), cx);
        let original = prompt.dictation_input_revision(cx);
        prompt.set_input("dog".into(), cx);
        let changed = prompt.dictation_input_revision(cx);
        assert!(changed > original);
        prompt.set_input("cat".into(), cx);
        assert!(prompt.dictation_input_revision(cx) > changed);
        let before_focus = prompt.dictation_input_revision(cx);
        prompt.next_input(cx);
        prompt.prev_input(cx);
        assert_eq!(prompt.current_input, 0);
        assert!(prompt.dictation_input_revision(cx) > before_focus);
        let before_read = prompt.dictation_input_revision(cx);
        prompt.filled_template();
        assert_eq!(prompt.dictation_input_revision(cx), before_read);
    });
}

#[cfg(test)]
#[gpui::test]
fn template_submission_advances_bound_completion_without_text_mutation(
    cx: &mut gpui::TestAppContext,
) {
    use crate::prompt_completion::{PromptCompletionBinding, PromptOutcome, SubmissionError};
    use gpui::AppContext as _;
    let binding = PromptCompletionBinding::local("template-completion-epoch".into());
    let prompt = cx.new(|cx| {
        TemplatePrompt::new(
            binding.instance().id.clone(),
            "fixed output".into(),
            cx.focus_handle(),
            binding.submit_callback(),
            Arc::new(theme::Theme::default()),
        )
    });
    prompt.update(cx, |prompt, cx| {
        let input_revision = prompt.dictation_input_revision(cx);
        let before = binding.semantic_revision();
        prompt.submit(cx);
        let completed = binding.semantic_revision();
        assert!(completed > before);
        assert_eq!(prompt.dictation_input_revision(cx), input_revision);
        let receipt = binding.observation().receipt.unwrap();
        assert_eq!(receipt.sequence, 1);
        assert!(matches!(receipt.outcome, PromptOutcome::Submitted(crate::protocol::SubmitValue::Text(value)) if value == "fixed output"));
        prompt.submit(cx);
        let duplicate = binding.observation();
        assert_eq!(duplicate.error, Some(SubmissionError::AlreadyCompleted));
        assert_eq!(duplicate.receipt.unwrap().sequence, 1);
    });
}
