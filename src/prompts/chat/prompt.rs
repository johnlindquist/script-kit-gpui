use super::*;

#[derive(Clone)]
pub(super) struct BuiltinPreparedPayload {
    pub(super) provider: Arc<dyn crate::ai::providers::AiProvider>,
    pub(super) api_messages: Vec<ProviderMessage>,
    pub(super) model_id: String,
}

pub struct ChatPrompt {
    pub id: String,
    pub messages: Vec<ChatPromptMessage>,
    pub placeholder: Option<String>,
    pub hint: Option<String>,
    pub footer: Option<String>,
    pub model: Option<String>,
    pub models: Vec<ChatModel>,
    pub title: Option<String>,
    pub focus_handle: FocusHandle,
    pub input: TextInputState,
    pub on_submit: Option<ChatSubmitCallback>,
    pub on_stop: Option<ChatStopCallback>,
    pub dismiss_binding: Option<ChatPromptDismissBinding>,
    pub on_continue: Option<ChatContinueCallback>,
    pub on_retry: Option<ChatRetryCallback>,
    pub recovery_binding: Option<ChatPromptRecoveryBinding>,
    pub theme: Arc<theme::Theme>,
    pub turns_list_state: ListState,
    pub(super) prompt_colors: theme::PromptColors,
    pub(super) conversation_turns_cache: Arc<Vec<ConversationTurn>>,
    pub(super) conversation_turns_dirty: bool,
    pub(super) streaming_message_id: Option<String>,
    // Frame-rate coalescing for external streaming (`append_chunk`): text
    // accumulates immediately, but the full turns-cache rebuild + notify runs
    // at most once per STREAM_FLUSH_INTERVAL so token-rate deltas can't starve
    // wheel-scroll frames.
    pub(super) stream_flush_pending: bool,
    pub(super) last_stream_flush_at: Option<std::time::Instant>,
    pub(super) last_copy_receipt:
        Option<crate::components::conversation_actions::ConversationCopyReceipt>,
    pub(super) command_status: Option<String>,
    pub(super) pending_prepared_request: Option<ChatPromptPreparedRequest>,
    pub(super) stream_generation: u64,
    pub(super) theme_revision_seen: u64,
    pub(super) prepared_requests_by_assistant_id: HashMap<String, ChatPromptPreparedRequest>,
    pub(super) builtin_replay_payloads:
        HashMap<sk_protocol::ai_reliability::TurnRequestRef, BuiltinPreparedPayload>,
    pub(super) terminal_outcomes: HashMap<String, sk_protocol::ai_reliability::AiOutcome>,
    // Database persistence
    pub(super) save_history: bool,
    // Built-in AI provider support (for inline chat without SDK)
    pub(super) provider_registry: Option<ProviderRegistry>,
    pub(super) available_models: Vec<ModelInfo>,
    pub(super) selected_model: Option<ModelInfo>,
    pub(super) builtin_system_prompt: Option<String>,
    pub(super) builtin_streaming_content: String,
    pub(super) builtin_is_streaming: bool,
    pub(super) builtin_cancel_signal: Option<Arc<AtomicBool>>,
    // Word-buffered reveal: full accumulated content from provider and reveal watermark
    pub(super) builtin_accumulated_content: String,
    pub(super) builtin_reveal_offset: usize,
    // When true, streaming updates stop forcing the list to the bottom.
    // Reset on explicit "jump to latest" and new submissions.
    pub(super) user_has_scrolled_up: bool,
    // Auto-submit flag: when true, submit the input on first render (for Tab from main menu)
    pub(super) pending_submit: bool,
    // Auto-respond flag: when true, respond to initial messages on first render (for scriptlets)
    pub(super) needs_initial_response: bool,
    // One-shot focus state so chat input auto-focuses when opened without stealing focus later.
    pub(super) pending_auto_focus: bool,
    // Cursor blink state for input field
    pub(super) cursor_visible: bool,
    pub(super) cursor_blink_started: bool,
    // Loading providers: when true, shows "Connecting to AI..." placeholder while providers load
    pub(super) loading_providers: bool,
    // Setup mode: when true, shows API key configuration card instead of chat
    pub(super) needs_setup: bool,
    // Script generation mode: enables post-response Save/Run actions
    pub(super) script_generation_mode: bool,
    pub(super) script_generation_status: Option<String>,
    pub(super) script_generation_status_is_error: bool,
    // Setup card keyboard focus (0 = Configure API Key, 1 = Claude Code)
    pub(super) setup_focus_index: usize,
    pub(super) on_configure: Option<ChatConfigureCallback>,
    // Callback for "Connect to Claude Code" (enables Claude Code in config)
    pub(super) on_claude_code: Option<ChatClaudeCodeCallback>,
    // Callback for showing actions dialog (handled by parent)
    pub(super) on_show_actions: Option<ChatShowActionsCallback>,
    // Callback for running a saved generated script via parent app pipeline
    pub(super) on_run_script: Option<RunScriptCallback>,
    // Callback for when a generated script has been saved (show CreationFeedback)
    pub(super) on_script_saved: Option<ScriptSavedCallback>,
    // Stable UUID for Claude Code CLI session continuity within this prompt's lifetime.
    // Generated once at construction so all messages share the same session.
    pub(super) cli_session_id: String,
    // Image attachment support
    pub(super) pending_image: Option<String>,
    pub(super) pending_image_render: Option<Arc<RenderImage>>,
    pub(super) image_render_cache: HashMap<String, Arc<RenderImage>>,
    /// One persistent `TextViewState` per assistant answer region, keyed by the
    /// turn's STABLE [`ConversationTurnRenderKey`] (never `message_id`, which
    /// moves to the assistant id when the reply lands).
    ///
    /// Persisting the entity is what makes streaming cheap: the vendored
    /// `TextView` keeps its parsed document in this state, so an appended chunk
    /// can extend it instead of re-parsing the whole answer every tick.
    pub(super) assistant_text_views:
        HashMap<ConversationTurnRenderKey, gpui::Entity<gpui_component::text::TextViewState>>,
    /// The exact source last handed to each state above. Compared byte-wise to
    /// tell a pure append (the streaming case) from a rewrite, because the two
    /// need different update paths.
    pub(super) assistant_text_sources: HashMap<ConversationTurnRenderKey, String>,
    pub(super) pasted_text_tokens: Vec<crate::pasted_text::PastedTextToken>,
    /// Exhaustive host mode: the prompt is either fully self-hosted
    /// (`Standalone`, owning header/input/footer/keys, optionally mini) or a
    /// pure transcript body (`TranscriptOnly`, with an external host owning
    /// all chrome + lifecycle). Replaces the old independent booleans
    /// `mini_mode`/`escape_over_stop`/`external_header`/`external_input`/
    /// `external_footer` that allowed incoherent combinations.
    pub(super) host_mode: ChatPromptHostMode,
    /// Hosted empty state: replaces the stock starter chips with the
    /// hosting agent's own purpose line (flow sessions).
    pub(super) empty_state_note: Option<String>,
}

impl ChatPrompt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        placeholder: Option<String>,
        messages: Vec<ChatPromptMessage>,
        hint: Option<String>,
        footer: Option<String>,
        focus_handle: FocusHandle,
        on_submit: Option<ChatSubmitCallback>,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let prompt_colors = theme.colors.prompt_colors();
        logging::log("PROMPTS", &format!("ChatPrompt::new id={}", id));

        let models = default_models();
        let default_model = models.first().map(|m| m.name.clone());

        // S10: raw SDK error strings are classified at the door.
        let messages = messages
            .into_iter()
            .map(|mut message| {
                Self::normalize_message_failure(&mut message);
                message
            })
            .collect();

        Self {
            id,
            messages,
            placeholder,
            hint,
            footer,
            model: default_model,
            models,
            title: Some("Chat".to_string()),
            focus_handle,
            input: TextInputState::new(),
            on_submit,
            on_stop: None,
            dismiss_binding: None,
            on_continue: None,
            on_retry: None,
            recovery_binding: None,
            theme,
            turns_list_state: {
                // WP-B3: chat prompt (Quick AI / Flow chat) transcript list —
                // opt into the hot-counters so its layout passes are attributable.
                let ls = ListState::new(0, ListAlignment::Bottom, px(200.0)).measure_all();
                ls.set_hot_metered(true);
                ls
            },
            prompt_colors,
            conversation_turns_cache: Arc::new(Vec::new()),
            conversation_turns_dirty: true,
            streaming_message_id: None,
            stream_flush_pending: false,
            last_stream_flush_at: None,
            last_copy_receipt: None,
            command_status: None,
            pending_prepared_request: None,
            stream_generation: 0,
            theme_revision_seen: crate::theme::service::theme_revision(),
            prepared_requests_by_assistant_id: HashMap::new(),
            builtin_replay_payloads: HashMap::new(),
            terminal_outcomes: HashMap::new(),
            save_history: true, // Default to saving
            // Built-in AI fields (disabled by default)
            provider_registry: None,
            available_models: Vec::new(),
            selected_model: None,
            builtin_system_prompt: None,
            builtin_streaming_content: String::new(),
            builtin_is_streaming: false,
            builtin_cancel_signal: None,
            builtin_accumulated_content: String::new(),
            builtin_reveal_offset: 0,
            user_has_scrolled_up: false,
            pending_submit: false,
            needs_initial_response: false,
            pending_auto_focus: true,
            cursor_visible: true,
            cursor_blink_started: false,
            loading_providers: false,
            needs_setup: false,
            script_generation_mode: false,
            script_generation_status: None,
            script_generation_status_is_error: false,
            setup_focus_index: 0,
            on_configure: None,
            on_claude_code: None,
            on_show_actions: None,
            on_run_script: None,
            on_script_saved: None,
            cli_session_id: uuid::Uuid::new_v4().to_string(),
            pending_image: None,
            pending_image_render: None,
            image_render_cache: HashMap::new(),
            assistant_text_views: HashMap::new(),
            assistant_text_sources: HashMap::new(),
            pasted_text_tokens: Vec::new(),
            host_mode: ChatPromptHostMode::Standalone { mini: false },
            empty_state_note: None,
        }
    }

    /// SDK-owned chat never constructs a provider. History policy is explicit
    /// before the first message can be submitted or appended.
    #[allow(clippy::too_many_arguments)]
    pub fn new_sdk(
        id: String,
        placeholder: Option<String>,
        messages: Vec<ChatPromptMessage>,
        hint: Option<String>,
        footer: Option<String>,
        focus_handle: FocusHandle,
        on_submit: Option<ChatSubmitCallback>,
        save_history: bool,
        theme: Arc<theme::Theme>,
    ) -> Self {
        Self::new(
            id,
            placeholder,
            messages,
            hint,
            footer,
            focus_handle,
            on_submit,
            theme,
        )
        .with_save_history(save_history)
    }

    pub(crate) fn saves_history(&self) -> bool {
        self.save_history
    }

    pub(crate) fn accepted_sdk_request(&self) -> Option<&ChatPromptPreparedRequest> {
        self.pending_prepared_request.as_ref()
    }

    pub(crate) fn current_stream_message_id(&self) -> Option<&str> {
        self.streaming_message_id.as_deref()
    }

    pub(crate) fn input_text(&self) -> &str {
        self.input.text()
    }

    pub(crate) fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub(crate) fn applied_theme_revision(&self) -> u64 {
        self.theme_revision_seen
    }
    pub(crate) fn semantic_token(&self, _cx: &App) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hash);
        self.input.text().hash(&mut hash);
        self.input.revision().hash(&mut hash);
        self.input.cursor().hash(&mut hash);
        self.input.selection().anchor.hash(&mut hash);
        self.selected_model
            .as_ref()
            .map(|model| {
                (
                    &model.id,
                    &model.display_name,
                    &model.provider,
                    model.supports_streaming,
                    model.context_window,
                )
            })
            .hash(&mut hash);
        self.streaming_message_id.hash(&mut hash);
        self.command_status.hash(&mut hash);
        for message in &self.messages {
            message.id.hash(&mut hash);
            message.get_content().hash(&mut hash);
            message
                .role
                .as_ref()
                .map(std::mem::discriminant)
                .hash(&mut hash);
            message.streaming.hash(&mut hash);
            message.error.hash(&mut hash);
        }
        hash.finish()
    }

    pub(crate) fn start_sdk_response(
        &mut self,
        request: ChatPromptPreparedRequest,
        message_id: String,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if request.prompt_id() != self.id
            || !(self
                .pending_prepared_request
                .as_ref()
                .is_some_and(|pending| pending.request_ref() == request.request_ref())
                || self
                    .prepared_requests_by_assistant_id
                    .values()
                    .any(|prior| prior.request_ref() == request.request_ref()))
        {
            return Err("stale_sdk_chat_request".to_string());
        }
        self.pending_prepared_request = Some(request);
        self.start_streaming(message_id, ChatMessagePosition::Left, cx);
        Ok(())
    }

    /// Set the callback for showing actions dialog
    pub fn set_on_show_actions(&mut self, callback: ChatShowActionsCallback) {
        self.on_show_actions = Some(callback);
    }

    /// The exhaustive host mode (Standalone vs TranscriptOnly).
    pub fn host_mode(&self) -> ChatPromptHostMode {
        self.host_mode
    }

    /// Whether the standalone prompt renders the borderless mini chrome.
    /// A transcript-only host owns its own chrome, so this is always false
    /// there.
    pub(super) fn mini_mode(&self) -> bool {
        self.host_mode.mini()
    }

    /// Whether an external host owns all chrome + keys (transcript body only).
    pub(super) fn is_transcript_only(&self) -> bool {
        self.host_mode.is_transcript_only()
    }

    /// Toggle mini chrome on a standalone prompt (e.g. main-window
    /// mini/full transitions). A transcript-only host owns its own chrome,
    /// so mini never applies and this is a no-op there.
    pub fn set_mini_mode(&mut self, mini: bool) {
        if let ChatPromptHostMode::Standalone { mini: current } = &mut self.host_mode {
            *current = mini;
        }
    }

    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        self.input.set_text(text);
        cx.notify();
    }

    pub fn draft_len(&self) -> usize {
        self.input.text().chars().count()
    }

    pub fn pending_submit(&self) -> bool {
        self.pending_submit
    }

    /// Submit the current composer text through the same pipeline as
    /// pressing Enter. Public for host surfaces whose native footer "Send"
    /// button must mirror the keyboard grammar (e.g. flow sessions).
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        self.handle_submit(cx);
    }

    /// Enable mini mode — renders input as a borderless text field matching the
    /// mini main window. This mutates the mini flag ONLY when the host is
    /// already `Standalone`; on a `TranscriptOnly` host it is a no-op (C-R2
    /// builder-order bug: it used to `set_host_mode(Standalone { mini })`,
    /// silently converting a transcript-only host back to a self-hosted one
    /// regardless of builder order).
    pub fn with_mini_mode(mut self, mini: bool) -> Self {
        self.set_mini_mode(mini);
        self
    }

    /// Set the exhaustive host mode. Applies the transcript alignment and,
    /// for a transcript-only host, cancels the first-render auto-focus so the
    /// suppressed internal input never steals focus from the host's composer.
    pub fn with_host_mode(mut self, mode: ChatPromptHostMode) -> Self {
        self.set_host_mode(mode);
        self
    }

    fn set_host_mode(&mut self, mode: ChatPromptHostMode) {
        self.host_mode = mode;
        match mode {
            ChatPromptHostMode::Standalone { .. } => {}
            ChatPromptHostMode::TranscriptOnly { alignment } => {
                // The host owns the composer; the suppressed internal input
                // must never grab focus on first render.
                self.pending_auto_focus = false;
                let list_alignment = match alignment {
                    ChatTranscriptAlignment::Bottom => ListAlignment::Bottom,
                    ChatTranscriptAlignment::Top => ListAlignment::Top,
                };
                self.turns_list_state = ListState::new(0, list_alignment, px(200.0)).measure_all();
                self.turns_list_state.set_hot_metered(true);
            }
        }
    }

    /// Set the callback for running a generated script path in the parent app.
    pub fn with_run_script_callback(
        mut self,
        callback: impl Fn(std::path::PathBuf, &mut Context<Self>) + Send + Sync + 'static,
    ) -> Self {
        self.on_run_script = Some(Arc::new(callback));
        self
    }

    /// Set the callback for when a generated script has been saved to disk.
    pub fn with_script_saved_callback(
        mut self,
        callback: impl Fn(std::path::PathBuf, &mut Context<Self>) + Send + Sync + 'static,
    ) -> Self {
        self.on_script_saved = Some(Arc::new(callback));
        self
    }

    /// Keep the caret eligible; the fade itself is owned by the shared
    /// pulse animation in the text-input painter (`pulse_cursor_bar`),
    /// so no toggle timer may fight it by zeroing `cursor_visible`.
    pub fn start_cursor_blink(&mut self, _cx: &mut Context<Self>) {
        self.cursor_visible = true;
    }

    /// Reset cursor to visible (called on user input to keep cursor visible while typing)
    pub(super) fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
    }

    /// Normalize pasted text to Unix newlines so multi-line chat input is preserved.
    pub(super) fn normalize_pasted_text(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Render helper for input text: show newline intent in a single-line visual field.
    pub(super) fn input_display_text(text: &str) -> String {
        let mut rendered = String::with_capacity(text.len());
        for ch in text.chars() {
            if ch == '\n' {
                rendered.push('↵');
                rendered.push(' ');
            } else {
                rendered.push(ch);
            }
        }
        rendered
    }

    /// Paste text from clipboard while preserving line breaks.
    pub(super) fn paste_text_from_clipboard(&mut self, cx: &mut Context<Self>) -> bool {
        // The GPUI boundary owns interactive pasteboards and evaluator-local
        // stores alike. Never bypass it through arboard.
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return false;
        };
        let normalized = Self::normalize_pasted_text(&text);
        if normalized.is_empty() {
            return false;
        }
        let prepared =
            crate::pasted_text::prepare_pasted_text(&normalized, &self.pasted_text_tokens);
        if let Some(token) = prepared.token {
            self.pasted_text_tokens.push(token);
        }
        self.input.insert_str(&prepared.insertion_text);
        self.sync_pasted_text_tokens();
        self.reset_cursor_blink();
        cx.notify();
        true
    }

    pub(super) fn sync_pasted_text_tokens(&mut self) {
        crate::pasted_text::sync_pasted_text_tokens(
            &mut self.pasted_text_tokens,
            self.input.text(),
        );
    }

    pub(super) fn expand_pasted_text_tokens(&self, text: &str) -> String {
        crate::pasted_text::expand_pasted_text_tokens(text, &self.pasted_text_tokens)
    }

    /// Set custom models for the chat
    pub fn with_models(mut self, models: Vec<ChatModel>) -> Self {
        self.models = models;
        if self.model.is_none() {
            self.model = self.models.first().map(|m| m.name.clone());
        }
        self
    }

    /// Set models from string names (creates ChatModel entries with name=id)
    pub fn with_model_names(mut self, model_names: Vec<String>) -> Self {
        if !model_names.is_empty() {
            self.models = model_names
                .into_iter()
                .map(|name| ChatModel::new(name.clone(), name.clone(), "Custom"))
                .collect();
            if self.model.is_none() {
                self.model = self.models.first().map(|m| m.name.clone());
            }
        }
        self
    }

    /// Set the default model
    pub fn with_default_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn with_stop_callback(mut self, callback: ChatStopCallback) -> Self {
        self.on_stop = Some(callback);
        self
    }

    pub fn with_dismiss_binding(mut self, binding: ChatPromptDismissBinding) -> Self {
        self.dismiss_binding = Some(binding);
        self
    }

    /// Empty-state purpose line for hosted surfaces (replaces the stock
    /// conversation-starter chips).
    pub fn with_empty_state_note(mut self, note: impl Into<String>) -> Self {
        self.empty_state_note = Some(note.into());
        self
    }

    /// Set the continue callback
    pub fn with_continue_callback(mut self, callback: ChatContinueCallback) -> Self {
        self.on_continue = Some(callback);
        self
    }

    /// Set the retry callback
    pub fn with_retry_callback(mut self, callback: ChatRetryCallback) -> Self {
        self.on_retry = Some(callback);
        self
    }

    /// Install only the recovery actions the host can actually perform.
    pub fn with_recovery_binding(mut self, binding: ChatPromptRecoveryBinding) -> Self {
        self.recovery_binding = Some(binding);
        self
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set whether to save chat history to the database
    pub fn with_save_history(mut self, save: bool) -> Self {
        self.save_history = save;
        self
    }

    /// Enable built-in AI mode with the given provider registry.
    /// When enabled, the ChatPrompt will handle AI calls directly instead of using the SDK callback.
    /// If prefer_vercel is true and Vercel is available, it will be used as the default provider.
    pub fn with_builtin_ai(mut self, registry: ProviderRegistry, prefer_vercel: bool) -> Self {
        let available_models = registry.get_all_models();

        // Select default model: prefer Vercel models if available and preferred, otherwise first available
        let selected_model = if prefer_vercel {
            available_models
                .iter()
                .find(|m| m.provider.to_lowercase() == "vercel")
                .or_else(|| available_models.first())
                .cloned()
        } else {
            available_models.first().cloned()
        };

        // Update display models list from provider registry
        self.models = available_models
            .iter()
            .map(|m| ChatModel::new(m.id.clone(), m.display_name.clone(), m.provider.clone()))
            .collect();
        self.model = selected_model.as_ref().map(|m| m.display_name.clone());

        logging::log(
            "CHAT",
            &format!(
                "ChatPrompt with built-in AI: {} models, selected={:?}",
                available_models.len(),
                selected_model.as_ref().map(|m| &m.display_name)
            ),
        );

        self.provider_registry = Some(registry);
        self.available_models = available_models;
        self.selected_model = selected_model;
        self
    }

    /// Set a fixed system prompt used for built-in AI submissions.
    pub fn with_builtin_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.builtin_system_prompt = Some(prompt.into());
        self
    }

    /// Enable script generation mode, which shows Save/Run actions after responses complete.
    pub fn with_script_generation_mode(mut self, enabled: bool) -> Self {
        self.script_generation_mode = enabled;
        self
    }

    /// Set pending_submit flag - when true, auto-submit input on first render
    /// Used for Tab from main menu to immediately send the query to AI
    pub fn with_pending_submit(mut self, submit: bool) -> Self {
        self.pending_submit = submit;
        self
    }

    /// Set needs_initial_response flag - when true, auto-respond to initial messages on first render
    /// Used for scriptlets that call chat() with pre-populated messages
    pub fn with_needs_initial_response(mut self, needs: bool) -> Self {
        self.needs_initial_response = needs;
        self
    }

    /// Set needs_setup flag - when true, shows API configuration card instead of chat
    /// Used when no AI providers are configured
    pub fn with_needs_setup(mut self, needs_setup: bool) -> Self {
        self.needs_setup = needs_setup;
        if needs_setup {
            self.setup_focus_index = 0;
        }
        self
    }

    /// Set loading_providers flag - when true, shows "Connecting to AI..." placeholder
    /// Used while provider registry is being loaded in the background
    pub fn with_loading_providers(mut self, loading: bool) -> Self {
        self.loading_providers = loading;
        self
    }

    /// Whether providers are currently loading
    pub fn loading_providers(&self) -> bool {
        self.loading_providers
    }

    /// Mutably set the provider registry after construction (e.g., when background loading completes).
    /// Clears loading_providers and updates available models.
    pub fn set_provider_registry(
        &mut self,
        registry: ProviderRegistry,
        prefer_vercel: bool,
        cx: &mut Context<Self>,
    ) {
        let available_models = registry.get_all_models();

        let selected_model = if prefer_vercel {
            available_models
                .iter()
                .find(|m| m.provider.to_lowercase() == "vercel")
                .or_else(|| available_models.first())
                .cloned()
        } else {
            available_models.first().cloned()
        };

        self.models = available_models
            .iter()
            .map(|m| ChatModel::new(m.id.clone(), m.display_name.clone(), m.provider.clone()))
            .collect();
        self.model = selected_model.as_ref().map(|m| m.display_name.clone());

        logging::log(
            "CHAT",
            &format!(
                "set_provider_registry: {} models, selected={:?}",
                available_models.len(),
                selected_model.as_ref().map(|m| &m.display_name)
            ),
        );

        self.provider_registry = Some(registry);
        self.available_models = available_models;
        self.selected_model = selected_model;
        self.loading_providers = false;
        cx.notify();
    }

    /// Set the configure callback - called when user clicks "Configure API Key"
    pub fn with_configure_callback(mut self, callback: ChatConfigureCallback) -> Self {
        self.on_configure = Some(callback);
        self
    }

    /// Set the Claude Code callback - called when user clicks "Connect to Claude Code"
    pub fn with_claude_code_callback(mut self, callback: ChatClaudeCodeCallback) -> Self {
        self.on_claude_code = Some(callback);
        self
    }

    pub(crate) fn command_status_text(&self) -> Option<&str> {
        self.command_status.as_deref()
    }

    pub(crate) fn terminal_outcome_for_message(
        &self,
        message_id: &str,
    ) -> Option<&sk_protocol::ai_reliability::AiOutcome> {
        self.terminal_outcomes.get(message_id)
    }

    /// Whether the setup card is showing (no providers configured)
    pub fn needs_setup(&self) -> bool {
        self.needs_setup
    }

    /// Handle a key event in setup mode from an external interceptor.
    /// Returns true if the key was handled (caller should stop propagation).
    pub fn handle_setup_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        if !self.needs_setup {
            return false;
        }
        let (next_index, action, changed) =
            resolve_setup_card_key(key, shift, self.setup_focus_index);
        let handled = changed || !matches!(action, SetupCardAction::None);

        if changed {
            self.setup_focus_index = next_index;
            cx.notify();
        }

        match action {
            SetupCardAction::ActivateConfigure => {
                if let Some(ref callback) = self.on_configure {
                    callback();
                }
            }
            SetupCardAction::ActivateClaudeCode => {
                if let Some(ref callback) = self.on_claude_code {
                    callback();
                }
            }
            SetupCardAction::Escape => self.handle_escape(cx),
            SetupCardAction::None => {}
        }

        handled
    }

    /// Check if built-in AI mode is enabled
    pub fn has_builtin_ai(&self) -> bool {
        self.provider_registry.is_some()
    }

    pub(super) fn clear_script_generation_status(&mut self) {
        self.script_generation_status = None;
        self.script_generation_status_is_error = false;
    }

    pub(super) fn set_script_generation_status(
        &mut self,
        is_error: bool,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.script_generation_status = Some(message.into());
        self.script_generation_status_is_error = is_error;
        cx.notify();
    }

    pub(super) fn latest_script_generation_draft(&self) -> Option<(String, String)> {
        if !self.script_generation_mode {
            return None;
        }

        for (index, message) in self.messages.iter().enumerate().rev() {
            if message.is_user() || message.streaming || message.error.is_some() {
                continue;
            }

            let script_source = message.get_content().trim();
            if script_source.is_empty() {
                continue;
            }

            if let Some(user_message) = self.messages[..index].iter().rev().find(|m| m.is_user()) {
                let prompt_description = user_message.get_content().trim();
                if !prompt_description.is_empty() {
                    return Some((prompt_description.to_string(), script_source.to_string()));
                }
            }
        }

        None
    }

    pub(super) fn should_show_script_generation_actions(&self) -> bool {
        should_show_script_generation_actions(
            self.script_generation_mode,
            self.is_streaming(),
            self.latest_script_generation_draft().is_some(),
        )
    }

    /// Whether the conversation has at least one assistant turn (non-user message).
    pub fn has_assistant_turn(&self) -> bool {
        self.messages.iter().any(|m| !m.is_user())
    }
}
