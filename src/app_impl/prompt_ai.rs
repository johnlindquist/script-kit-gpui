use super::*;
use anyhow::{anyhow, Context as AnyhowContext, Result};

#[derive(Debug, Clone, Copy, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub(crate) enum AiScriptGenerationStage {
    SelectModel,
    ResolveProvider,
    RequestCompletion,
    ExtractScript,
    CreateScriptFile,
    WriteScriptFile,
    OpenEditor,
}

impl AiScriptGenerationStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SelectModel => "select_model",
            Self::ResolveProvider => "resolve_provider",
            Self::RequestCompletion => "request_completion",
            Self::ExtractScript => "extract_script",
            Self::CreateScriptFile => "create_script_file",
            Self::WriteScriptFile => "write_script_file",
            Self::OpenEditor => "open_editor",
        }
    }
}

pub(crate) const AI_SCRIPT_GENERATION_SYSTEM_PROMPT: &str = r#"You are ScriptKitScriptGenerator.
Generate a complete Script Kit TypeScript script and return code only.
Requirements:
- Include `export const metadata = { name, description }` near the top.
- Include exactly one SDK import near the top:
  import "@scriptkit/sdk";
- Default to the shared main-menu-sized prompt flow. Prefer `arg()` / `fields()` / `select()` before inventing a denser surface.
- Treat expanded split-view previews as rare exceptions for information-dense selection tasks.
- Avoid `setPreview()`, `setPanel()`, and choice `preview` fields for ordinary commands.
- Use top-level await with Script Kit APIs directly when useful:
  arg(), fields(), select(), div(), editor(), notify(), md()
- For file tasks, prefer Script Kit helpers like home() and writeFile().
- Return a full runnable script with practical defaults and no extra explanation."#;

pub(crate) fn build_ai_script_generation_user_prompt(description: &str) -> String {
    format!(
        "Generate a complete Script Kit script for this request:\n\n{}\n\nReturn only the TypeScript script source.",
        description.trim()
    )
}

pub(crate) fn select_default_ai_script_model(
    registry: &crate::ai::ProviderRegistry,
) -> Option<crate::ai::ModelInfo> {
    let all_models = registry.get_all_models();
    all_models
        .iter()
        .find(|model| model.provider.eq_ignore_ascii_case("vercel"))
        .cloned()
        .or_else(|| all_models.first().cloned())
}

pub(crate) fn extract_generated_script_source(raw_response: &str) -> Option<String> {
    let trimmed = raw_response.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Prefer typed code fences first, then generic fences.
    for fence in ["```typescript", "```ts", "```javascript", "```js", "```"] {
        if let Some(content) = extract_first_fenced_block(trimmed, fence) {
            return normalize_generated_script(content);
        }
    }

    normalize_generated_script(trimmed.to_string())
}

pub(crate) fn extract_first_fenced_block(response: &str, fence_start: &str) -> Option<String> {
    let start_index = response.find(fence_start)?;
    let after_fence = &response[start_index + fence_start.len()..];
    let after_newline = after_fence
        .strip_prefix("\r\n")
        .or_else(|| after_fence.strip_prefix('\n'))
        .unwrap_or(after_fence);
    let end_index = after_newline.find("```")?;
    Some(after_newline[..end_index].trim().to_string())
}

pub(crate) fn normalize_generated_script(content: String) -> Option<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("{trimmed}\n"))
    }
}

pub(crate) fn derive_script_name_from_description(description: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in description.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }

        if slug.len() >= 48 {
            break;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "ai-generated-script".to_string()
    } else {
        slug
    }
}

#[allow(dead_code)]
fn generate_script_via_ai_backend(
    registry: &crate::ai::ProviderRegistry,
    model_id: &str,
    prompt_description: &str,
    config: &crate::config::Config,
) -> Result<std::path::PathBuf> {
    let provider = registry
        .find_provider_for_model(model_id)
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "state={} attempted=find_provider_for_model model_id={} failure=provider_not_found",
                AiScriptGenerationStage::ResolveProvider.as_str(),
                model_id
            )
        })?;

    let request_messages = vec![
        crate::ai::ProviderMessage::system(AI_SCRIPT_GENERATION_SYSTEM_PROMPT),
        crate::ai::ProviderMessage::user(build_ai_script_generation_user_prompt(
            prompt_description,
        )),
    ];

    let ai_response = provider
        .send_message(&request_messages, model_id)
        .with_context(|| {
            format!(
                "state={} attempted=send_message model_id={} provider={} failure=provider_call_failed",
                AiScriptGenerationStage::RequestCompletion.as_str(),
                model_id,
                provider.provider_id()
            )
        })?;

    let generated_script = extract_generated_script_source(&ai_response).ok_or_else(|| {
        anyhow!(
            "state={} attempted=extract_generated_script model_id={} failure=empty_script_response",
            AiScriptGenerationStage::ExtractScript.as_str(),
            model_id
        )
    })?;

    let script_name = derive_script_name_from_description(prompt_description);
    let script_path =
        crate::script_creation::create_new_script(&script_name).with_context(|| {
            format!(
                "state={} attempted=create_new_script name={} failure=create_script_failed",
                AiScriptGenerationStage::CreateScriptFile.as_str(),
                script_name
            )
        })?;

    std::fs::write(&script_path, generated_script).with_context(|| {
        format!(
            "state={} attempted=write_script path={} failure=write_failed",
            AiScriptGenerationStage::WriteScriptFile.as_str(),
            script_path.display()
        )
    })?;

    crate::script_creation::open_in_editor(&script_path, config).with_context(|| {
        format!(
            "state={} attempted=open_in_editor path={} failure=open_editor_failed",
            AiScriptGenerationStage::OpenEditor.as_str(),
            script_path.display()
        )
    })?;

    Ok(script_path)
}

impl ScriptListApp {
    pub(crate) fn capture_mini_ai_close_snapshot(
        &mut self,
        source: MiniAiCloseSource,
        cx: &mut Context<Self>,
    ) -> MiniAiCloseSnapshot {
        let (prompt_id, draft_len, pending_submit) = match &self.current_view {
            AppView::ChatPrompt { id, entity } if id.starts_with("inline-ai") => {
                let chat = entity.read(cx);
                (id.clone(), chat.draft_len(), chat.pending_submit())
            }
            _ => ("inline-ai".to_string(), 0, false),
        };

        let snapshot = MiniAiCloseSnapshot {
            prompt_id,
            main_window_mode: self.main_window_mode,
            source,
            draft_len,
            pending_submit,
            handoff_source: Some("mini_ai_launcher".to_string()),
            return_origin: Some("scriptList".to_string()),
        };

        tracing::info!(
            target: "script_kit::mini_ai",
            event = "mini_ai_close_snapshot",
            prompt_id = %snapshot.prompt_id,
            main_window_mode = ?snapshot.main_window_mode,
            source = snapshot.source.as_str(),
            draft_len = snapshot.draft_len,
            pending_submit = snapshot.pending_submit,
            handoff_source = ?snapshot.handoff_source,
            return_origin = ?snapshot.return_origin,
            "Mini AI close snapshot"
        );
        self.mini_ai_last_close_snapshot = Some(snapshot.clone());
        snapshot
    }

    pub(crate) fn mini_ai_state_snapshot(&self, cx: &Context<Self>) -> serde_json::Value {
        let current = match &self.current_view {
            AppView::ChatPrompt { id, entity } if id.starts_with("inline-ai") => {
                let chat = entity.read(cx);
                Some((id.clone(), chat.draft_len(), chat.pending_submit()))
            }
            _ => None,
        };
        let last_source = self
            .mini_ai_last_close_snapshot
            .as_ref()
            .map(|snapshot| snapshot.source.as_str());
        serde_json::json!({
            "visible": current.is_some(),
            "promptId": current.as_ref().map(|(id, _, _)| id.clone()),
            "mainWindowMode": match self.main_window_mode {
                MainWindowMode::Mini => "mini",
                MainWindowMode::Full => "full",
            },
            "draftLen": current.as_ref().map(|(_, draft_len, _)| *draft_len).or_else(|| self.mini_ai_last_close_snapshot.as_ref().map(|snapshot| snapshot.draft_len)).unwrap_or(0),
            "pendingSubmit": current.as_ref().map(|(_, _, pending)| *pending).or_else(|| self.mini_ai_last_close_snapshot.as_ref().map(|snapshot| snapshot.pending_submit)).unwrap_or(false),
            "handoffSource": self.mini_ai_last_close_snapshot.as_ref().and_then(|snapshot| snapshot.handoff_source.clone()).or_else(|| Some("mini_ai_launcher".to_string())),
            "returnOrigin": self.mini_ai_last_close_snapshot.as_ref().and_then(|snapshot| snapshot.return_origin.clone()).or_else(|| Some("scriptList".to_string())),
            "lastCloseSource": last_source,
        })
    }

    pub(crate) fn is_in_prompt(&self) -> bool {
        !matches!(
            self.current_view,
            AppView::ScriptList | AppView::ActionsDialog | AppView::CreationFeedback { .. }
        )
    }

    /// Submit a response to the current prompt
    ///
    /// Uses try_send() to avoid blocking the UI thread if the script's input
    /// channel is full. User-initiated actions should never freeze the UI.
    pub(crate) fn submit_prompt_response(
        &mut self,
        id: String,
        value: Option<String>,
        cx: &mut Context<Self>,
    ) {
        use crate::prompt_completion::{PromptCompletionBinding, PromptOutcome};
        let binding = if let Some(binding) = &self.prompt_completion {
            if binding.instance().id != id {
                self.show_error_toast("stale_prompt_submission".to_string(), cx);
                return;
            }
            binding.clone()
        } else {
            let binding = PromptCompletionBinding::sdk(id, self.response_sender.clone());
            self.prompt_completion = Some(binding.clone());
            binding
        };
        let outcome = value.map_or(PromptOutcome::Cancelled, |value| {
            PromptOutcome::Submitted(crate::protocol::SubmitValue::Text(value))
        });
        if let Err(error) = binding.try_complete(outcome) {
            self.show_error_toast(error.to_string(), cx);
            return;
        }
        self.mark_main_data_changed();
        cx.notify();
    }

    /// Get filtered choices for arg prompt (also handles mini/micro variants)
    pub(crate) fn filtered_arg_choices(&self) -> Vec<(usize, &Choice)> {
        let choices = match &self.current_view {
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => choices,
            _ => return vec![],
        };
        if self.arg_input.is_empty() {
            choices.iter().enumerate().collect()
        } else {
            let filter = self.arg_input.text().to_lowercase();
            choices
                .iter()
                .enumerate()
                .filter(|(_, c)| c.name.to_lowercase().contains(&filter))
                .collect()
        }
    }

    /// P0: Get filtered choices as owned data for uniform_list closure
    pub(crate) fn get_filtered_arg_choices_owned(&self) -> Vec<(usize, Choice)> {
        let choices = match &self.current_view {
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => choices,
            _ => return vec![],
        };
        if self.arg_input.is_empty() {
            choices
                .iter()
                .enumerate()
                .map(|(i, c)| (i, c.clone()))
                .collect()
        } else {
            let filter = self.arg_input.text().to_lowercase();
            choices
                .iter()
                .enumerate()
                .filter(|(_, c)| c.name.to_lowercase().contains(&filter))
                .map(|(i, c)| (i, c.clone()))
                .collect()
        }
    }

    // NOTE: hex_to_rgba_with_opacity moved to crate::ui_foundation (centralized)

    /// Create box shadows from theme configuration
    pub(crate) fn create_box_shadows(&self) -> Vec<BoxShadow> {
        let shadow_config = self.theme.get_drop_shadow();

        if !shadow_config.enabled {
            return vec![];
        }

        // Convert hex color to HSLA
        // For black (0x000000), we use h=0, s=0, l=0
        let r = ((shadow_config.color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((shadow_config.color >> 8) & 0xFF) as f32 / 255.0;
        let b = (shadow_config.color & 0xFF) as f32 / 255.0;

        // Simple RGB to HSL conversion for shadow color
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        let (h, s) = if max == min {
            (0.0, 0.0) // achromatic
        } else {
            let d = max - min;
            let s = if l > 0.5 {
                d / (2.0 - max - min)
            } else {
                d / (max + min)
            };
            let h = if max == r {
                (g - b) / d + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h / 6.0, s)
        };

        vec![BoxShadow {
            color: hsla(h, s, l, shadow_config.opacity),
            offset: point(px(shadow_config.offset_x), px(shadow_config.offset_y)),
            blur_radius: px(shadow_config.blur_radius),
            spread_radius: px(shadow_config.spread_radius),
        }]
    }

    /// Show inline AI chat prompt with built-in AI provider support.
    /// This switches to the ChatPrompt view with direct AI integration (no SDK needed).
    /// Prefers Vercel AI Gateway if configured, otherwise uses the first available provider.
    pub fn show_inline_ai_chat(&mut self, initial_query: Option<String>, cx: &mut Context<Self>) {
        use crate::ai::ProviderRegistry;
        use crate::prompts::ChatPrompt;

        // Mark as opened from main menu so ESC returns to main menu
        self.opened_from_main_menu = true;

        let main_window_mode = self.main_window_mode;
        let handoff_source = Some("mini_ai_launcher".to_string());
        let return_origin = Some("scriptList".to_string());

        let dismiss_sender = self.inline_chat_escape_sender.clone();
        let dismiss_callback: crate::prompts::ChatPromptDismissCallback =
            std::sync::Arc::new(move |request| {
                tracing::info!(
                    target: "script_kit::mini_ai",
                    event = "mini_ai_window_close_requested",
                    prompt_id = %request.prompt_id,
                    main_window_mode = ?main_window_mode,
                    trigger = ?request.trigger,
                    draft_len = 0usize,
                    pending_submit = false,
                    handoff_source = ?handoff_source,
                    return_origin = ?return_origin,
                    "Mini AI dismiss requested"
                );
                let _ = dismiss_sender.try_send(());
            });

        // Use cached registry if available, otherwise build synchronously as fallback
        let registry = self
            .cached_provider_registry
            .clone()
            .unwrap_or_else(|| ProviderRegistry::from_environment_with_config(Some(&self.config)));

        if !registry.has_any_provider() {
            crate::logging::log("CHAT", "No AI providers configured - showing setup card");

            // Create configure callback that signals via channel
            let configure_sender = self.inline_chat_configure_sender.clone();
            let configure_callback: crate::prompts::ChatConfigureCallback =
                std::sync::Arc::new(move || {
                    crate::logging::log("CHAT", "Configure callback triggered - sending signal");
                    let _ = configure_sender.try_send(());
                });

            // Create Claude Code callback that signals via channel
            let claude_code_sender = self.inline_chat_claude_code_sender.clone();
            let claude_code_callback: crate::prompts::ChatClaudeCodeCallback =
                std::sync::Arc::new(move || {
                    crate::logging::log("CHAT", "Claude Code callback triggered - sending signal");
                    let _ = claude_code_sender.try_send(());
                });

            let chat_prompt = ChatPrompt::new(
                "inline-ai-setup".to_string(),
                Some("Configure API key to continue...".to_string()),
                vec![],
                None, // No hint needed - setup card is the UI
                None,
                self.focus_handle.clone(),
                None,
                std::sync::Arc::clone(&self.theme),
            )
            .with_title("Ask AI")
            .with_save_history(false) // Don't save setup state to history
            .with_dismiss_binding(crate::prompts::ChatPromptDismissBinding {
                route: crate::prompts::ChatPromptDismissRoute::Back,
                active_work: crate::components::conversation_actions::ActiveWorkDismissal::RequiresExplicitStop,
                callback: dismiss_callback.clone(),
            })
            .with_needs_setup(true)
            .with_configure_callback(configure_callback)
            .with_claude_code_callback(claude_code_callback)
            .with_mini_mode(self.main_window_mode == MainWindowMode::Mini);

            let entity = cx.new(|_| chat_prompt);
            self.current_view = AppView::ChatPrompt {
                id: "inline-ai-setup".to_string(),
                entity,
            };
            self.note_main_route_changed();
            self.focused_input = FocusedInput::None;
            self.pending_focus = Some(FocusTarget::ChatPrompt);
            resize_to_view_sync(
                super::ui_window::compact_ai_view_type_for_mode(self.main_window_mode),
                0,
            );
            cx.notify();
            return;
        }

        crate::logging::log(
            "CHAT",
            &format!(
                "Showing inline AI chat with {} providers",
                registry.provider_ids().len()
            ),
        );

        let placeholder = Some("Ask anything...".to_string());

        let mut chat_prompt = ChatPrompt::new(
            "inline-ai".to_string(),
            placeholder,
            vec![],
            None,
            None,
            self.focus_handle.clone(),
            None,
            std::sync::Arc::clone(&self.theme),
        )
        .with_title("Ask AI")
        .with_save_history(true)
        .with_dismiss_binding(crate::prompts::ChatPromptDismissBinding {
            route: crate::prompts::ChatPromptDismissRoute::Back,
            active_work:
                crate::components::conversation_actions::ActiveWorkDismissal::RequiresExplicitStop,
            callback: dismiss_callback,
        })
        .with_builtin_ai(registry, true) // true = prefer Vercel AI Gateway
        .with_mini_mode(self.main_window_mode == MainWindowMode::Mini);

        tracing::info!(
            event = "inline_chat.construction",
            id = "inline-ai",
            mini_mode = (self.main_window_mode == MainWindowMode::Mini),
            on_show_actions_set = true,
            "ChatPrompt constructed with on_show_actions wired"
        );

        // If there's an initial query, set it in the input and auto-submit
        if let Some(query) = initial_query {
            chat_prompt.input.set_text(&query);
            chat_prompt = chat_prompt.with_pending_submit(true);
        }

        let entity = cx.new(|_| chat_prompt);

        // Wire on_show_actions so ChatPrompt can request the actions dialog from within
        // (e.g., footer button). ⌘K is also intercepted at the parent level as a fallback.
        let actions_sender = self.inline_chat_actions_sender.clone();
        entity.update(cx, |chat, _cx| {
            chat.set_on_show_actions(std::sync::Arc::new(move |prompt_id| {
                tracing::info!(
                    target: "script_kit::mini_ai",
                    event = "mini_ai_actions_requested",
                    source = "chat_prompt",
                    prompt_id = %prompt_id,
                    "Mini AI requested actions popup"
                );
                let _ = actions_sender.try_send(MiniAiUiRequest::ToggleActions {
                    prompt_id: prompt_id.to_string(),
                    source: "mini_ai_chat",
                });
            }));
        });

        self.current_view = AppView::ChatPrompt {
            id: "inline-ai".to_string(),
            entity,
        };
        self.note_main_route_changed();
        self.focused_input = FocusedInput::None;
        self.pending_focus = Some(FocusTarget::ChatPrompt);
        resize_to_view_sync(
            super::ui_window::compact_ai_view_type_for_mode(self.main_window_mode),
            0,
        );
        cx.notify();
    }

    /// Compatibility shim — routes script generation requests to the instant
    /// harness terminal instead of the legacy ChatPrompt surface.
    pub fn show_script_generation_chat(
        &mut self,
        initial_query: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let query = initial_query.unwrap_or_default().trim().to_string();

        if query.is_empty() {
            self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
        } else {
            self.open_tab_ai_chat_with_entry_intent(Some(query), cx);
        }
    }

    pub(crate) fn dispatch_ai_script_generation_from_query(
        &mut self,
        query: String,
        cx: &mut Context<Self>,
    ) {
        let query = query.trim().to_string();
        if query.is_empty() {
            logging::log(
                "AI_SCRIPT_GEN",
                "state=ignored attempted=dispatch_ai_script_generation_from_query reason=empty_query",
            );
            return;
        }

        // Route through the current harness/Agent Chat entry path, but suppress the
        // launcher's focused fallback chip so recipe-driven generation uses
        // only the generated prompt as context.
        self.open_tab_ai_chat_with_entry_intent_suppressing_focused_part(Some(query), cx);
    }

    /// Generate a Script Kit script from a natural-language prompt using the built-in AI backend.
    /// The generated script is saved to disk and opened in the configured editor.
    #[allow(dead_code)]
    pub fn generate_script_from_ai_prompt(
        &mut self,
        prompt_description: String,
        cx: &mut Context<Self>,
    ) {
        let prompt_description = prompt_description.trim().to_string();
        if prompt_description.is_empty() {
            return;
        }

        let registry = self.cached_provider_registry.clone().unwrap_or_else(|| {
            crate::ai::ProviderRegistry::from_environment_with_config(Some(&self.config))
        });

        if !registry.has_any_provider() {
            self.toast_manager.push(
                components::toast::Toast::error(
                    "No AI providers configured for script generation",
                    &self.theme,
                )
                .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
            return;
        }

        let selected_model = match select_default_ai_script_model(&registry) {
            Some(model) => model,
            None => {
                let stage = AiScriptGenerationStage::SelectModel.as_str();
                logging::log(
                    "AI_SCRIPT_GEN",
                    &format!(
                        "state={} attempted=select_default_model failure=no_available_models",
                        stage
                    ),
                );
                self.toast_manager.push(
                    components::toast::Toast::error(
                        "No AI models available for script generation",
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
                cx.notify();
                return;
            }
        };

        let model_id = selected_model.id.clone();
        let provider = selected_model.provider.clone();
        let config = self.config.clone();
        let (tx, rx) = async_channel::bounded::<std::result::Result<std::path::PathBuf, String>>(1);

        logging::log(
            "AI_SCRIPT_GEN",
            &format!(
                "state=queued attempted=shift_tab_script_generation model_id={} provider={} prompt_len={}",
                model_id,
                provider,
                prompt_description.len()
            ),
        );
        self.show_hud(
            "Generating script with AI...".to_string(),
            Some(HUD_SHORT_MS),
            cx,
        );

        std::thread::spawn(move || {
            logging::log(
                "AI_SCRIPT_GEN",
                &format!(
                    "state=running attempted=generate_script model_id={} provider={}",
                    model_id, provider
                ),
            );

            let generation_result =
                generate_script_via_ai_backend(&registry, &model_id, &prompt_description, &config)
                    .map_err(|error| error.to_string());

            if tx.send_blocking(generation_result).is_err() {
                logging::log(
                    "AI_SCRIPT_GEN",
                    "state=aborted attempted=send_result failure=result_channel_closed",
                );
            }
        });

        cx.spawn(async move |this, cx| {
            let Ok(result) = rx.recv().await else {
                logging::log(
                    "AI_SCRIPT_GEN",
                    "state=aborted attempted=recv_result failure=result_channel_closed",
                );
                return;
            };

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| match result {
                    Ok(script_path) => {
                        let script_name = script_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("generated script");
                        logging::log(
                            "AI_SCRIPT_GEN",
                            &format!(
                                "state=completed attempted=generate_script path={}",
                                script_path.display()
                            ),
                        );
                        app.toast_manager.push(
                            components::toast::Toast::success(
                                format!("Generated and opened {}", script_name),
                                &app.theme,
                            )
                            .duration_ms(Some(TOAST_INFO_MS)),
                        );
                        app.close_and_reset_window(cx);
                    }
                    Err(error) => {
                        logging::log(
                            "AI_SCRIPT_GEN",
                            &format!("state=failed attempted=generate_script error={}", error),
                        );
                        app.toast_manager.push(
                            components::toast::Toast::error(
                                format!("Failed to generate script: {}", error),
                                &app.theme,
                            )
                            .duration_ms(Some(TOAST_ERROR_DETAILED_MS)),
                        );
                        cx.notify();
                    }
                })
            });
        })
        .detach();
    }

    /// Rebuild the cached provider registry in a background thread.
    /// Called after config changes (API key saved, Claude Code enabled, etc.)
    pub fn rebuild_provider_registry_async(&mut self, cx: &mut Context<Self>) {
        self.cached_provider_registry = None;
        let config_clone = self.config.clone();
        let (tx, rx) = async_channel::bounded::<crate::ai::ProviderRegistry>(1);

        std::thread::spawn(move || {
            let registry =
                crate::ai::ProviderRegistry::from_environment_with_config(Some(&config_clone));
            if tx.send_blocking(registry).is_err() {
                logging::log(
                    "APP",
                    "Provider registry rebuild result dropped: receiver unavailable",
                );
            }
        });

        cx.spawn(async move |this, cx| {
            let Ok(registry) = rx.recv().await else {
                logging::log("APP", "Provider registry rebuild failed: channel closed");
                return;
            };

            let provider_count = registry.provider_ids().len();
            let _ = cx.update(|cx| {
                this.update(cx, |app, _cx| {
                    app.cached_provider_registry = Some(registry);
                    logging::log(
                        "APP",
                        &format!("Provider registry rebuilt: {} providers", provider_count),
                    );
                })
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ai_script_generation_user_prompt_includes_description() {
        let prompt = build_ai_script_generation_user_prompt("create a weather script");
        assert!(prompt.contains("create a weather script"));
        assert!(prompt.contains("Script Kit script"));
    }

    #[test]
    fn test_extract_generated_script_source_prefers_typescript_fence() {
        let response = r#"
Here's your script:
```typescript
// Name: Weather
import "@scriptkit/sdk";
```
"#;
        let extracted =
            extract_generated_script_source(response).expect("script should be extracted");
        assert!(extracted.contains("// Name: Weather"));
        assert!(!extracted.contains("```"));
    }

    #[test]
    fn test_extract_generated_script_source_falls_back_to_plain_text() {
        let response = "// Name: Plain\nimport \"@scriptkit/sdk\";";
        let extracted =
            extract_generated_script_source(response).expect("script should be extracted");
        assert_eq!(extracted, "// Name: Plain\nimport \"@scriptkit/sdk\";\n");
    }

    #[test]
    fn test_derive_script_name_from_description_uses_fallback_for_symbols() {
        let name = derive_script_name_from_description("@@@ !!!");
        assert_eq!(name, "ai-generated-script");
    }

    #[test]
    fn test_ai_script_generation_system_prompt_references_scriptkit_sdk_not_legacy_kit() {
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("import \"@scriptkit/sdk\";"));
        assert!(!AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("@johnlindquist/kit"));
    }

    #[test]
    fn test_ai_script_generation_system_prompt_defaults_to_main_menu_dimensions() {
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT.contains("main-menu-sized prompt flow"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT
            .contains("expanded split-view previews as rare exceptions"));
        assert!(AI_SCRIPT_GENERATION_SYSTEM_PROMPT
            .contains("Avoid `setPreview()`, `setPanel()`, and choice `preview` fields"));
    }
}
