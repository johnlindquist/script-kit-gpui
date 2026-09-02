#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchResultExecutionAction {
    LaunchApp,
    FocusWindow,
}

impl SearchResultExecutionAction {
    fn failure_message(self, target_name: &str, error: impl std::fmt::Display) -> String {
        match self {
            Self::LaunchApp => format!("Failed to launch {target_name}: {error}"),
            Self::FocusWindow => format!("Failed to focus window: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScratchPadExecutionAction {
    CreateDirectory,
    CreateFile,
    ReadFile,
    SubmitSave,
    AutoSave,
}

impl ScratchPadExecutionAction {
    fn log_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::CreateDirectory => format!("Failed to create scratch pad directory: {error}"),
            Self::CreateFile => format!("Failed to create scratch pad file: {error}"),
            Self::ReadFile => format!("Failed to read scratch pad: {error}"),
            Self::SubmitSave => format!("Failed to save scratch pad on submit: {error}"),
            Self::AutoSave => format!("Auto-save failed: {error}"),
        }
    }

    fn toast_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::CreateDirectory => format!("Failed to create directory: {error}"),
            Self::CreateFile => format!("Failed to create scratch pad: {error}"),
            Self::ReadFile => format!("Failed to read scratch pad: {error}"),
            Self::SubmitSave => format!("Failed to save scratch pad: {error}"),
            Self::AutoSave => format!("Auto-save failed: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeCodeEnableAction {
    EnableProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiKeyCompletionOutcome {
    Saved,
    NotSaved,
}

impl ClaudeCodeEnableAction {
    fn validation_restored_message(self) -> &'static str {
        match self {
            Self::EnableProvider => {
                "Failed to enable Claude Code (invalid config). Backup restored."
            }
        }
    }

    fn validation_no_backup_message(self, reason: impl std::fmt::Display) -> String {
        match self {
            Self::EnableProvider => {
                format!("Failed to enable Claude Code: {reason}. No backup available.")
            }
        }
    }

    fn validation_recovery_failed_message(
        self,
        reason: impl std::fmt::Display,
        recover_error: impl std::fmt::Display,
    ) -> String {
        match self {
            Self::EnableProvider => {
                format!("Failed to enable Claude Code: {reason}. Recovery failed: {recover_error}")
            }
        }
    }

    fn write_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::EnableProvider => format!("Failed to enable Claude Code: {error}"),
        }
    }
}

impl ApiKeyCompletionOutcome {
    fn from_success(success: bool) -> Self {
        match success {
            true => Self::Saved,
            false => Self::NotSaved,
        }
    }

    fn success_message(self, provider: &str) -> Option<String> {
        match self {
            Self::Saved => Some(format!("{} API key saved successfully", provider)),
            Self::NotSaved => None,
        }
    }

    fn should_rebuild_provider_registry(self) -> bool {
        matches!(self, Self::Saved)
    }
}

impl ScriptListApp {
    fn execute_app(&mut self, app: &app_launcher::AppInfo, cx: &mut Context<Self>) {
        let execution_action = SearchResultExecutionAction::LaunchApp;
        tracing::info!(message = %&format!("Launching app from search: {}", app.name));

        if let Err(e) = app_launcher::launch_application(app) {
            let message = execution_action.failure_message(&app.name, &e);
            tracing::error!(message = %message);
            self.show_error_toast(message, cx);
        } else {
            tracing::info!(message = %&format!("Launched app: {}", app.name));
            self.close_and_reset_window(cx);
        }
    }

    /// Focus a window from the main search results
    fn execute_window_focus(
        &mut self,
        window: &window_control::WindowInfo,
        cx: &mut Context<Self>,
    ) {
        let execution_action = SearchResultExecutionAction::FocusWindow;
        tracing::info!(message = %&format!("Focusing window: {} - {}", window.app, window.title),
        );

        if let Err(e) = window_control::focus_window(window.id) {
            let message = execution_action.failure_message(&window.title, &e);
            tracing::error!(message = %message);
            self.show_error_toast(message, cx);
        } else {
            tracing::info!(message = %&format!("Focused window: {}", window.title));
            self.root_search
                .record_root_window_focus(window.selection_key());
            self.close_and_reset_window(cx);
        }
    }

    /// Show an API key configuration prompt.
    ///
    /// This creates an EnvPrompt that stores the key in the system keyring.
    /// Once stored, the key will be available to:
    /// - Agent Chat Chat window (via DetectedKeys::from_environment)
    /// - Scripts using `await env("SCRIPT_KIT_*_API_KEY")`
    fn show_api_key_prompt(
        &mut self,
        key_name: &str,
        prompt_text: &str,
        provider_name: &str,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(message = %&format!("Showing API key prompt for: {}", provider_name),
        );

        let id = format!("configure-{}", key_name.to_lowercase());
        let key = key_name.to_string();
        let prompt = Some(prompt_text.to_string());
        let secret = true; // API keys are always secrets

        // Store provider name for success message after completion
        self.pending_api_key_config = Some(provider_name.to_string());

        // Create submit callback that signals completion
        // The actual toast and view reset happens in handle_api_key_completion
        let completion_sender = self.api_key_completion_sender.clone();
        let provider_for_callback = provider_name.to_string();
        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(move |_id, value| {
                // Value being Some means the user submitted a value (key was saved)
                // Value being None means the user cancelled
                let success = value.is_some();
                tracing::info!(message = %&format!(
                        "API key config callback: provider={}, success={}",
                        provider_for_callback, success
                    ),
                );
                // Signal completion to the app
                let _ = completion_sender.try_send((provider_for_callback.clone(), success));
            });

        // Check if key already exists in secrets (for UX messaging). Missing
        // keys stay distinct from storage/decrypt/parse failures.
        let (exists_in_keyring, modified_at, stored_secret_value, secret_store_error) =
            match secrets::get_secret_info_result(&key) {
                Ok(secret_info) => {
                    let exists = secret_info
                        .as_ref()
                        .map(|info| !info.value.is_empty())
                        .unwrap_or(false);
                    let modified_at = secret_info.as_ref().map(|info| info.modified_at);
                    let value = secret_info.map(|info| info.value);
                    (exists, modified_at, value, None)
                }
                Err(error) => {
                    tracing::warn!(
                        key = %key,
                        kind = error.kind_str(),
                        "API key prompt secret store unavailable"
                    );
                    (false, None, None, Some(error))
                }
            };

        if exists_in_keyring {
            tracing::info!(message = %&format!(
                    "{} API key already configured (modified: {:?}) - showing update prompt",
                    provider_name, modified_at
                ),
            );
        }

        // Create EnvPrompt entity
        let focus_handle = self.focus_handle.clone();
        let env_prompt = prompts::EnvPrompt::new(
            id.clone(),
            key.clone(),
            prompt,
            Some(provider_name.to_string()), // title
            secret,
            focus_handle,
            submit_callback,
            std::sync::Arc::clone(&self.theme),
            exists_in_keyring,
            modified_at,
            stored_secret_value,
            secret_store_error,
        );

        let entity = cx.new(|_| env_prompt);
        self.current_view = AppView::EnvPrompt { id, entity };
        self.note_main_route_changed();
        self.focused_input = FocusedInput::None; // EnvPrompt has its own focus handling
        self.pending_focus = Some(FocusTarget::EnvPrompt);

        // Resize to standard height for full-window centered layout
        resize_to_view_sync(ViewType::DivPrompt, 0);
        cx.notify();
    }

    /// Handle API key configuration completion.
    /// Called when the EnvPrompt callback signals completion.
    ///
    /// NOTE: This is called from render(), so we must use deferred resize via Window::defer
    /// to avoid layout issues where the macOS window resizes but GPUI's layout doesn't update.
    fn handle_api_key_completion(
        &mut self,
        provider: String,
        success: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_api_key_config = None;
        let completion_outcome = ApiKeyCompletionOutcome::from_success(success);

        if let Some(message) = completion_outcome.success_message(&provider) {
            // Show success toast
            self.toast_manager.push(
                components::toast::Toast::success(message, &self.theme)
                    .duration_ms(Some(TOAST_SUCCESS_MS)),
            );
        }

        if completion_outcome.should_rebuild_provider_registry() {
            // Rebuild provider registry so new key is available next time chat opens
            self.rebuild_provider_registry_async(cx);
        }

        // Return to main menu
        self.reset_to_script_list(cx);

        // CRITICAL: Use deferred resize because this is called from render().
        // Synchronous resize (resize_to_view_sync) would resize the macOS window
        // but GPUI's layout system wouldn't update until the next frame,
        // causing the content to render at the wrong size (empty list bug).
        let target_height = window_resize::height_for_view(ViewType::ScriptList, 0);
        window.defer(cx, move |_window, _cx| {
            window_resize::resize_first_window_to_height(target_height);
        });

        cx.notify();
    }

    /// Enable Claude Code in config.ts and re-show the inline chat.
    ///
    /// This modifies the user's config.ts to enable Claude Code provider,
    /// reloads the config, and then re-opens the inline chat with
    /// the newly available Claude Code provider.
    pub fn enable_claude_code_in_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::config::editor::{self, ConfigWriteError, WriteOutcome};

        let enable_action = ClaudeCodeEnableAction::EnableProvider;
        tracing::info!(message = %"Enabling Claude Code in config.ts");

        let config_path =
            std::path::PathBuf::from(shellexpand::tilde("~/.scriptkit/config.ts").as_ref());
        let bun_path = self.config.bun_path.as_deref();

        match editor::enable_claude_code_safely(&config_path, bun_path) {
            Ok(WriteOutcome::Written) => {
                tracing::info!(message = %"Claude Code enabled in config.ts");
            }
            Ok(WriteOutcome::Created) => {
                tracing::info!(message = %"Created new config.ts with Claude Code enabled");
            }
            Ok(WriteOutcome::AlreadySet) => {
                tracing::info!(message = %"Claude Code already enabled in config.ts");
            }
            Err(ConfigWriteError::ValidationFailed(reason)) => {
                tracing::info!(message = %&format!("Config validation failed: {}", reason));
                // Attempt to recover from backup
                match editor::recover_from_backup(&config_path, bun_path) {
                    Ok(true) => {
                        tracing::info!(message = %"Config restored from backup after validation failure",
                        );
                        self.show_error_toast(enable_action.validation_restored_message(), cx);
                    }
                    Ok(false) => {
                        self.show_error_toast(
                            enable_action.validation_no_backup_message(&reason),
                            cx,
                        );
                    }
                    Err(recover_err) => {
                        tracing::info!(message = %&format!("Backup recovery also failed: {}", recover_err),
                        );
                        self.show_error_toast(
                            enable_action.validation_recovery_failed_message(&reason, &recover_err),
                            cx,
                        );
                    }
                }
                return;
            }
            Err(e) => {
                let message = enable_action.write_failure_message(&e);
                tracing::info!(message = %message);
                self.show_error_toast(message, cx);
                return;
            }
        }

        // Reload config and rebuild provider registry in background
        self.update_config(cx);
        self.rebuild_provider_registry_async(cx);

        // Check if Claude CLI is actually installed (this is an explicit user action,
        // so the brief sync check is acceptable for correct toast messaging)
        let claude_path = self
            .config
            .get_claude_code()
            .path
            .unwrap_or_else(|| "claude".to_string());
        let claude_available = std::process::Command::new(&claude_path)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if claude_available {
            self.toast_manager.push(
                components::toast::Toast::success(
                    "Claude Code enabled! Ready to use.".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_SUCCESS_MS)),
            );

            // Go back to main menu, then re-show inline chat
            self.go_back_or_close(window, cx);
            self.show_inline_ai_chat(None, cx);
        } else {
            self.toast_manager.push(
                components::toast::Toast::warning(
                    "Config saved! Install Claude CLI: npm install -g @anthropic-ai/claude-code"
                        .to_string(),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_ERROR_DETAILED_MS)),
            );
            tracing::info!(message = %"Claude Code config saved but CLI not found - user needs to install it",
            );
        }

        cx.notify();
    }

    /// Get the scratch pad file path
    fn get_scratch_pad_path() -> std::path::PathBuf {
        setup::get_kit_path().join("scratch-pad.md")
    }

    /// Open the scratch pad editor with auto-save functionality
    fn open_scratch_pad(&mut self, cx: &mut Context<Self>) {
        tracing::info!(message = %"Opening Scratch Pad");
        if let Err(error) =
            crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ExternalStorage)
        {
            self.show_error_toast(error.to_string(), cx);
            return;
        }

        // Get or create scratch pad file path
        let scratch_path = Self::get_scratch_pad_path();

        // Ensure parent directory exists
        if let Some(parent) = scratch_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                let action = ScratchPadExecutionAction::CreateDirectory;
                tracing::error!(message = %action.log_message(&e));
                self.show_error_toast(action.toast_message(&e), cx);
                return;
            }
        }

        // Load existing content or create empty file
        let content = match std::fs::read_to_string(&scratch_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Create empty file
                if let Err(write_err) = std::fs::write(&scratch_path, "") {
                    let action = ScratchPadExecutionAction::CreateFile;
                    tracing::error!(message = %action.log_message(&write_err));
                    self.show_error_toast(action.toast_message(&write_err), cx);
                    return;
                }
                String::new()
            }
            Err(e) => {
                let action = ScratchPadExecutionAction::ReadFile;
                tracing::error!(message = %action.log_message(&e));
                self.show_error_toast(action.toast_message(&e), cx);
                return;
            }
        };

        tracing::info!(message = %&format!("Loaded scratch pad with {} bytes", content.len()),
        );

        use crate::design_evaluation::prompt_fixtures::{
            EditorPromptSeed, PromptSeed, PromptSeedCommon, ScratchPadPromptSeed,
        };
        let seed = PromptSeed::ScratchPad(ScratchPadPromptSeed {
            editor: EditorPromptSeed {
                common: PromptSeedCommon::local("scratch-pad"),
                content,
                language: "markdown".to_string(),
                template: None,
            },
            path: scratch_path,
        });
        if let Err(error) = self.construct_prompt_seed(seed, cx) {
            self.show_error_toast(error.to_string(), cx);
            return;
        }
        self.prepare_constructed_sdk_prompt("scratch-pad", true, cx);
    }
}
