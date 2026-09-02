use super::*;
use validation::{build_filename, build_submit_payload, derive_naming_state, normalize_extension};

/// A starter template selected in the
/// [`crate::main_sections::app_view_state::AppView::ScriptTemplateCatalogView`]
/// and threaded through the naming dialog. `label` is the human-readable title
/// shown inside the naming dialog; `id` is the opaque identifier used by
/// [`crate::mcp_resources::find_script_template`] to resolve the template on
/// submit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSelection {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NamingPromptConfig {
    pub placeholder: Option<String>,
    pub hint: Option<String>,
    pub extension: String,
    pub target: NamingTarget,
    pub target_directory: PathBuf,
    pub design_variant: DesignVariant,
    pub template: Option<TemplateSelection>,
}

impl NamingPromptConfig {
    pub fn new(
        target: NamingTarget,
        target_directory: PathBuf,
        extension: impl Into<String>,
    ) -> Self {
        Self {
            placeholder: None,
            hint: None,
            extension: normalize_extension(&extension.into()),
            target,
            target_directory,
            design_variant: DesignVariant::Default,
            template: None,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn design_variant(mut self, design_variant: DesignVariant) -> Self {
        self.design_variant = design_variant;
        self
    }

    /// Seed the naming dialog with a starter template. The template id is
    /// carried through to [`crate::prompts::NamingSubmitResult::template_id`]
    /// so the caller can overwrite the freshly-created file with the template
    /// body before the editor opens.
    pub fn template(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.template = Some(TemplateSelection {
            id: id.into(),
            label: label.into(),
        });
        self
    }
}

pub struct NamingPrompt {
    pub id: String,
    pub placeholder: Option<String>,
    pub hint: Option<String>,
    pub target: NamingTarget,
    pub target_directory: PathBuf,
    pub extension: String,
    pub friendly_name: String,
    pub friendly_name_trimmed: String,
    pub filename_stem: String,
    pub filename: String,
    pub validation_error: Option<NamingValidationError>,
    pub focus_handle: FocusHandle,
    pub on_submit: SubmitCallback,
    pub theme: Arc<theme::Theme>,
    pub design_variant: DesignVariant,
    pub template: Option<TemplateSelection>,
}

impl NamingPrompt {
    pub fn new(
        id: String,
        config: NamingPromptConfig,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let mut prompt = Self {
            id,
            placeholder: config.placeholder,
            hint: config.hint,
            target: config.target,
            target_directory: config.target_directory,
            extension: config.extension,
            friendly_name: String::new(),
            friendly_name_trimmed: String::new(),
            filename_stem: String::new(),
            filename: String::new(),
            validation_error: None,
            focus_handle,
            on_submit,
            theme,
            design_variant: config.design_variant,
            template: config.template,
        };

        prompt.refresh_derived_state();
        prompt
    }

    fn refresh_derived_state(&mut self) {
        let state =
            derive_naming_state(&self.friendly_name, &self.extension, &self.target_directory);
        self.friendly_name_trimmed = state.friendly_name_trimmed;
        self.filename_stem = state.filename_stem;
        self.filename = state.filename;
        self.validation_error = state.validation_error;
    }

    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        if self.friendly_name == text {
            return;
        }

        self.friendly_name = text;
        self.refresh_derived_state();
        cx.notify();
    }

    pub(super) fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        if ch.is_control() {
            return;
        }

        self.friendly_name.push(ch);
        self.refresh_derived_state();
        cx.notify();
    }

    pub(super) fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if self.friendly_name.pop().is_none() {
            return;
        }

        self.refresh_derived_state();
        cx.notify();
    }

    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
        self.refresh_derived_state();

        if self.validation_error.is_some() {
            cx.notify();
            return;
        }

        let template_id = self.template.as_ref().map(|t| t.id.as_str());
        match build_submit_payload(
            &self.friendly_name_trimmed,
            &self.filename,
            self.target,
            template_id,
        ) {
            Ok(payload) => {
                (self.on_submit)(self.id.clone(), Some(payload));
            }
            Err(error) => {
                tracing::error!(
                    event = "naming_prompt_submit_failed",
                    attempt = "serialize_submit_payload",
                    error = %error,
                    prompt_id = %self.id,
                    target = self.target.as_str(),
                    friendly_name = %self.friendly_name_trimmed,
                    filename = %self.filename,
                    target_directory = %self.target_directory.display(),
                    "Failed to serialize naming prompt payload"
                );
                self.validation_error = Some(NamingValidationError::SubmissionEncodingFailed);
                cx.notify();
            }
        }
    }

    pub(super) fn submit_cancel(&mut self) {
        (self.on_submit)(self.id.clone(), None);
    }

    pub(super) fn filename_preview(&self) -> String {
        if self.filename_stem.is_empty() {
            build_filename("your-name", &self.extension)
        } else {
            self.filename.clone()
        }
    }

    pub(super) fn extension_label(&self) -> String {
        if self.extension.is_empty() {
            "(none)".to_string()
        } else {
            format!(".{}", self.extension)
        }
    }
}

#[cfg(test)]
mod seeded_behavior_tests {
    use super::*;

    #[gpui::test]
    fn naming_validates_and_submits_without_creating_artifact(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let directory = tempfile::tempdir().unwrap();
        let submitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let output = submitted.clone();
        let prompt = cx.new(|cx| {
            NamingPrompt::new(
                "naming-local".into(),
                NamingPromptConfig::new(NamingTarget::Script, directory.path().to_owned(), "ts"),
                cx.focus_handle(),
                Arc::new(move |_, value| output.lock().push(value)),
                Arc::new(theme::Theme::default()),
            )
        });
        prompt.update(cx, |prompt, cx| {
            prompt.submit(cx);
            assert!(submitted.lock().is_empty());
            prompt.set_input("Daily Report".into(), cx);
            prompt.submit(cx);
            assert!(!directory.path().join(&prompt.filename).exists());
        });
        let value: serde_json::Value =
            serde_json::from_str(submitted.lock()[0].as_deref().unwrap()).unwrap();
        assert_eq!(value["friendly_name"], "Daily Report");
    }
}
