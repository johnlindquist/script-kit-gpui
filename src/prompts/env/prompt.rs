use super::*;

/// EnvPrompt - Environment variable prompt with secure storage
///
/// Prompts for environment variable values and stores them securely
/// in the local age-encrypted secrets file. Useful for API keys, tokens, and secrets.
#[derive(Clone)]
pub enum EnvStorage {
    Encrypted,
    Local(Arc<parking_lot::Mutex<Option<String>>>),
}

impl EnvStorage {
    fn write(&self, key: &str, value: Option<&str>) -> Result<(), String> {
        match self {
            Self::Encrypted => {
                crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Credentials)
                    .map_err(|error| error.to_string())?;
                match value {
                    Some(value) => {
                        secrets::set_secret(key, value).map_err(|error| error.to_string())
                    }
                    None => secrets::delete_secret(key).map_err(|error| error.to_string()),
                }
            }
            Self::Local(store) => {
                *store.lock() = value.map(str::to_owned);
                Ok(())
            }
        }
    }
}

pub struct EnvPrompt {
    /// Unique ID for this prompt instance
    pub id: String,
    /// Environment variable key name
    pub key: String,
    /// Custom prompt text (defaults to "Enter value for {key}")
    pub prompt: Option<String>,
    /// Optional title (e.g., provider name like "Vercel AI Gateway")
    pub title: Option<String>,
    /// Whether to mask input (for secrets)
    pub secret: bool,
    /// Text input state with selection and clipboard support
    pub(super) input: TextInputState,
    /// Focus handle for keyboard input
    pub focus_handle: FocusHandle,
    /// Callback when user submits a value
    pub on_submit: SubmitCallback,
    /// Theme for styling
    pub theme: Arc<theme::Theme>,
    /// Design variant for styling
    pub design_variant: DesignVariant,
    /// Whether we checked the keyring already
    pub(super) checked_keyring: bool,
    /// Whether a value already exists in keyring (for UX messaging)
    pub exists_in_keyring: bool,
    /// When the secret was last modified (if exists)
    pub modified_at: Option<DateTime<Utc>>,
    /// Stored secret value used only for the no-context auto-submit path.
    pub(super) stored_secret_value: Option<String>,
    /// Secret storage load/read/decrypt/parse failure, if storage health is degraded.
    pub secret_store_error: Option<SecretStoreError>,
    /// Inline validation/persistence error shown to the user
    pub(super) validation_error: Option<String>,
    /// Whether secret text is currently visible
    pub(super) reveal_secret: bool,
    /// Monotonic counter used to cancel stale auto-hide timers
    pub(super) reveal_generation: u64,
    storage: EnvStorage,
}

impl EnvPrompt {
    pub(crate) fn dictation_input_revision(&self, _cx: &gpui::App) -> u64 {
        self.input.revision()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        key: String,
        prompt: Option<String>,
        title: Option<String>,
        secret: bool,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
        exists_in_keyring: bool,
        modified_at: Option<DateTime<Utc>>,
        stored_secret_value: Option<String>,
        secret_store_error: Option<SecretStoreError>,
    ) -> Self {
        let correlation_id = env_prompt_correlation_id(&id, &key);
        logging::log(
            "PROMPTS",
            &format!(
                "correlation_id={correlation_id} EnvPrompt::new key={key} secret={secret} exists={exists_in_keyring} store_error={} title={title:?} modified={modified_at:?}",
                secret_store_error
                    .as_ref()
                    .map(|error| error.kind_str())
                    .unwrap_or("none")
            ),
        );

        EnvPrompt {
            id,
            key,
            prompt,
            title,
            secret,
            input: TextInputState::new(),
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
            checked_keyring: false,
            exists_in_keyring,
            modified_at,
            stored_secret_value,
            secret_store_error,
            validation_error: None,
            reveal_secret: false,
            reveal_generation: 0,
            storage: EnvStorage::Encrypted,
        }
    }

    /// Inject local secret facts before any persistence or auto-submission.
    pub fn with_local_storage(mut self, storage: Arc<parking_lot::Mutex<Option<String>>>) -> Self {
        self.storage = EnvStorage::Local(storage);
        self
    }

    pub(super) fn correlation_id(&self) -> String {
        env_prompt_correlation_id(&self.id, &self.key)
    }

    pub fn input_text(&self) -> &str {
        self.input.text()
    }

    pub(crate) fn validation_message(&self) -> Option<&str> {
        self.validation_error.as_deref()
    }

    /// Returns true if the script provided contextual prompt or title text,
    /// indicating the UI should always be shown even if the value exists.
    pub fn has_prompt_or_title(&self) -> bool {
        let has_prompt = self
            .prompt
            .as_ref()
            .map(|p| !p.trim().is_empty())
            .unwrap_or(false);
        let has_title = self
            .title
            .as_ref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false);
        has_prompt || has_title
    }

    /// Check keyring and auto-submit if value exists
    /// Returns true if value was found and submitted
    pub fn check_keyring_and_auto_submit(&mut self) -> bool {
        if self.checked_keyring {
            return false;
        }
        self.checked_keyring = true;

        if let Some(error) = self.secret_store_error.clone() {
            self.validation_error = Some(error.user_message().to_string());
            logging::log(
                "PROMPTS",
                &format!(
                    "correlation_id={} EnvPrompt skipped auto-submit key={} store_error={}",
                    self.correlation_id(),
                    self.key,
                    error.kind_str()
                ),
            );
            return false;
        }

        if let Some(value) = self.stored_secret_value.take() {
            let correlation_id = self.correlation_id();
            logging::log(
                "PROMPTS",
                &format!(
                    "correlation_id={correlation_id} EnvPrompt auto-submit existing secret key={}",
                    self.key
                ),
            );
            // Auto-submit the stored value
            (self.on_submit)(self.id.clone(), Some(value));
            return true;
        }
        false
    }

    /// Submit the entered value
    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.input.text();
        if let Some(validation_error) = env_submit_validation_error(text) {
            self.validation_error = Some(validation_error.to_string());
            cx.notify();
            logging::log(
                "PROMPTS",
                &format!(
                    "correlation_id={} EnvPrompt submit blocked key={} reason={}",
                    self.correlation_id(),
                    self.key,
                    validation_error
                ),
            );
            return;
        }

        // Persist in encrypted storage only when this prompt is secret-mode.
        if self.secret {
            if let Some(error) = self.secret_store_error.clone() {
                self.validation_error = Some(error.user_message().to_string());
                cx.notify();
                logging::log(
                    "ERROR",
                    &format!(
                        "correlation_id={} EnvPrompt submit blocked by secret store error key={} kind={}",
                        self.correlation_id(),
                        self.key,
                        error.kind_str()
                    ),
                );
                return;
            }

            if let Err(e) = self.storage.write(&self.key, Some(text)) {
                self.validation_error =
                    Some("Failed to store secret. Check logs and try again.".to_string());
                cx.notify();
                logging::log(
                    "ERROR",
                    &format!(
                        "correlation_id={} EnvPrompt failed to store secret key={} error={}",
                        self.correlation_id(),
                        self.key,
                        e
                    ),
                );
                return;
            }
        }

        self.validation_error = None;
        (self.on_submit)(self.id.clone(), Some(text.to_string()));
    }

    /// Set the input text programmatically
    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        if self.input.text() == text {
            return;
        }

        self.input.set_text(text);
        self.validation_error = None;
        cx.notify();
    }

    pub(super) fn toggle_secret_reveal(&mut self, cx: &mut Context<Self>) {
        if !self.secret {
            return;
        }

        self.reveal_secret = !self.reveal_secret;
        self.reveal_generation = self.reveal_generation.wrapping_add(1);
        let reveal_generation = self.reveal_generation;
        let should_auto_hide = self.reveal_secret;
        cx.notify();

        if !should_auto_hide {
            return;
        }

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(5))
                .await;

            cx.update(|cx| {
                let _ = this.update(cx, |prompt, cx| {
                    if prompt.reveal_secret && prompt.reveal_generation == reveal_generation {
                        prompt.reveal_secret = false;
                        cx.notify();
                    }
                });
            });
        })
        .detach();
    }

    /// Cancel - submit None
    pub(super) fn submit_cancel(&mut self) {
        self.validation_error = None;
        (self.on_submit)(self.id.clone(), None);
    }

    /// Delete the secret and close the prompt
    pub(super) fn submit_delete(&mut self, cx: &mut Context<Self>) {
        let correlation_id = self.correlation_id();
        logging::log(
            "PROMPTS",
            &format!(
                "correlation_id={correlation_id} EnvPrompt deleting secret key={}",
                self.key
            ),
        );

        // Delete from keyring
        if let Err(e) = self.storage.write(&self.key, None) {
            self.validation_error =
                Some("Failed to delete stored value. Check logs and try again.".to_string());
            cx.notify();
            logging::log(
                "ERROR",
                &format!(
                    "correlation_id={correlation_id} EnvPrompt failed to delete secret key={} error={}",
                    self.key, e
                ),
            );
            return;
        }

        self.validation_error = None;
        // Call callback with None (same as cancel, but secret is now deleted)
        (self.on_submit)(self.id.clone(), None);
    }

    /// Get display text (masked if secret)
    pub(super) fn display_text(&self) -> String {
        if self.secret && !self.reveal_secret {
            masked_secret_value_for_display(self.input.text())
        } else {
            self.input.text().to_string()
        }
    }

    pub(super) fn render_text_with_cursor_and_selection(
        &self,
        text: &str,
        text_primary: u32,
        accent_color: u32,
    ) -> Div {
        crate::components::text_input::render_text_input_cursor_selection(
            crate::components::text_input::TextInputRenderConfig {
                cursor: self.input.cursor(),
                selection: Some(self.input.selection()),
                cursor_visible: true,
                cursor_color: text_primary,
                text_color: text_primary,
                selection_color: accent_color,
                selection_text_color: text_primary,
                overflow_x_hidden: true,
                ..crate::components::text_input::TextInputRenderConfig::default_for_prompt(text)
            },
        )
    }

    /// Render the text input with cursor and selection
    pub(super) fn render_input_text(&self, text_primary: u32, accent_color: u32) -> Div {
        let text = self.display_text();
        self.render_text_with_cursor_and_selection(&text, text_primary, accent_color)
    }
}

#[cfg(test)]
mod local_storage_tests {
    use super::*;

    #[test]
    fn local_secret_storage_writes_and_deletes_without_keyring() {
        let value = Arc::new(parking_lot::Mutex::new(None));
        let storage = EnvStorage::Local(value.clone());
        storage.write("FIXTURE", Some("local value")).unwrap();
        assert_eq!(value.lock().as_deref(), Some("local value"));
        storage.write("FIXTURE", None).unwrap();
        assert!(value.lock().is_none());
    }

    #[gpui::test]
    fn env_validation_precedes_local_storage_and_submission(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let stored = Arc::new(parking_lot::Mutex::new(None));
        let submitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let output = submitted.clone();
        let prompt = cx.new(|cx| {
            EnvPrompt::new(
                "env-local".into(),
                "FIXTURE".into(),
                Some("Value".into()),
                None,
                true,
                cx.focus_handle(),
                Arc::new(move |_, value| output.lock().push(value)),
                Arc::new(theme::Theme::default()),
                false,
                None,
                None,
                None,
            )
            .with_local_storage(stored.clone())
        });
        prompt.update(cx, |prompt, cx| {
            prompt.set_input("   ".into(), cx);
            prompt.submit(cx);
        });
        assert!(stored.lock().is_none());
        assert!(submitted.lock().is_empty());
        prompt.update(cx, |prompt, cx| {
            prompt.set_input("local value".into(), cx);
            prompt.submit(cx);
        });
        assert_eq!(stored.lock().as_deref(), Some("local value"));
        assert_eq!(*submitted.lock(), vec![Some("local value".into())]);
    }

    #[gpui::test]
    fn env_revision_tracks_effective_input_not_theme_or_reads(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let prompt = cx.new(|cx| {
            EnvPrompt::new(
                "epoch".into(),
                "FIXTURE".into(),
                None,
                None,
                false,
                cx.focus_handle(),
                Arc::new(|_, _| {}),
                Arc::new(theme::Theme::default()),
                false,
                None,
                None,
                None,
            )
        });
        prompt.update(cx, |prompt, cx| {
            prompt.set_input("cat".into(), cx);
            let before = prompt.dictation_input_revision(cx);
            prompt.set_input("cat".into(), cx);
            assert_eq!(prompt.dictation_input_revision(cx), before);
            prompt.set_input("dog".into(), cx);
            let changed = prompt.dictation_input_revision(cx);
            assert!(changed > before);
            prompt.set_input("cat".into(), cx);
            assert!(prompt.dictation_input_revision(cx) > changed);
            prompt.input.move_to_end(false);
            let at_end = prompt.dictation_input_revision(cx);
            prompt.input.move_left(true);
            assert!(prompt.dictation_input_revision(cx) > at_end);
            let selected = prompt.dictation_input_revision(cx);
            prompt.theme = Arc::new(theme::Theme::default());
            assert_eq!(prompt.dictation_input_revision(cx), selected);
        });
    }
}
