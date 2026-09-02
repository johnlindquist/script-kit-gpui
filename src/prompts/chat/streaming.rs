use super::*;

impl ChatPrompt {
    pub(super) fn handle_submit(&mut self, cx: &mut Context<Self>) {
        let display_text = self.input.text().to_string();
        if display_text.trim().is_empty() && self.pending_image.is_none() {
            return;
        }
        let outbound_text = self.expand_pasted_text_tokens(&display_text);
        let prepared = ChatPromptPreparedRequest::new(&self.id, display_text, outbound_text);
        if self.has_builtin_ai() {
            if self.builtin_is_streaming {
                return;
            }
            if let Err(error) =
                crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
            {
                self.command_status = Some(error.code.to_string());
                cx.notify();
                return;
            }
            let pending_image = self.pending_image.take();
            let pending_render = self.pending_image_render.take();
            self.input.clear();
            self.pasted_text_tokens.clear();
            self.clear_script_generation_status();
            self.handle_builtin_ai_submit(prepared, pending_image, pending_render, cx);
        } else {
            let result = self
                .on_submit
                .as_ref()
                .ok_or_else(|| "chat_submission_sink_missing".to_string())
                .and_then(|callback| callback(prepared.clone()));
            match result {
                Ok(()) => {
                    self.pending_prepared_request = Some(prepared);
                    self.input.clear();
                    self.pending_image = None;
                    self.pending_image_render = None;
                    self.pasted_text_tokens.clear();
                    self.clear_script_generation_status();
                    self.command_status = None;
                }
                Err(error) => {
                    let failure = crate::ai::reliability::runtime_closed_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Provider,
                        &error,
                    );
                    self.command_status = Some(failure.primary_message().to_string());
                }
            }
            cx.notify();
        }
    }

    /// Handle submission in built-in AI mode - calls AI provider directly
    pub(super) fn handle_builtin_ai_submit(
        &mut self,
        prepared: ChatPromptPreparedRequest,
        pending_image: Option<String>,
        pending_render: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        let display_text = prepared.display_text().to_string();
        let outbound_text = prepared.outbound_text().to_string();
        // Don't allow new messages while streaming
        if self.builtin_is_streaming {
            return;
        }

        // Step 1: Expand @context mentions (e.g., @clipboard, @file:path)
        let expanded_text = expand_context(&outbound_text, cx);

        // Step 2: Process slash commands (e.g., /explain, /fix, /test)
        let (system_context, user_message_text) = transform_with_command(&expanded_text);

        // Log if slash command was detected
        if let Some(ref ctx) = system_context {
            logging::log(
                "CHAT",
                &format!(
                    "Slash command detected, system context: {}...",
                    &ctx[..ctx.len().min(50)]
                ),
            );
        }

        // Add user message to UI (ChatPromptMessage::user auto-generates UUID)
        let user_message = ChatPromptMessage::user(display_text.clone());
        let user_message_id = user_message.id.clone().unwrap_or_default();
        self.messages.push(user_message);

        // Cache the render image for conversation history display
        if let Some(render) = pending_render {
            self.image_render_cache
                .insert(user_message_id.clone(), render);
        }

        self.mark_conversation_turns_dirty();
        self.force_scroll_turns_to_bottom();
        cx.notify();

        // Get the selected model and provider
        let (model_id, provider) = match &self.selected_model {
            Some(m) => (m.id.clone(), m.provider.clone()),
            None => {
                logging::log("CHAT", "No model selected for built-in AI");
                let error_msg = ChatPromptMessage::assistant(
                    "No AI model configured. Please set up an API key.",
                );
                self.messages.push(error_msg);
                self.mark_conversation_turns_dirty();
                self.ensure_conversation_turns_cache();
                cx.notify();
                return;
            }
        };

        let registry = match &self.provider_registry {
            Some(r) => r.clone(),
            None => return,
        };

        let ai_provider = match registry.find_provider_for_model(&model_id) {
            Some(p) => p.clone(),
            None => {
                logging::log(
                    "CHAT",
                    &format!("No provider found for model: {}", model_id),
                );
                let error_msg = ChatPromptMessage::assistant(format!(
                    "Provider not found for model: {}",
                    model_id
                ));
                self.messages.push(error_msg);
                self.mark_conversation_turns_dirty();
                self.ensure_conversation_turns_cache();
                cx.notify();
                return;
            }
        };

        // Build messages for the API call (convert our messages to provider format)
        let mut api_messages: Vec<ProviderMessage> = Vec::new();

        if let Some(system_prompt) = self.builtin_system_prompt.clone() {
            api_messages.push(ProviderMessage::system(system_prompt));
        }

        // If slash command detected, prepend system context
        if let Some(ref ctx) = system_context {
            api_messages.push(ProviderMessage::system(ctx.clone()));
        }

        // Add conversation history (all messages except the last user message)
        for (i, m) in self.messages.iter().enumerate() {
            // Skip the last message (current user input) - we'll add the transformed version
            if i == self.messages.len() - 1 && m.is_user() {
                continue;
            }
            if m.is_user() {
                api_messages.push(ProviderMessage::user(m.get_content()));
            } else {
                api_messages.push(ProviderMessage::assistant(m.get_content()));
            }
        }

        // Add the current user message (with expanded context and slash command processing)
        if let Some(image_base64) = pending_image {
            let images = vec![crate::ai::providers::ProviderImage::png(image_base64)];
            api_messages.push(ProviderMessage::user_with_images(
                user_message_text.clone(),
                images,
            ));
        } else {
            api_messages.push(ProviderMessage::user(user_message_text.clone()));
        }

        // Set streaming state
        self.builtin_is_streaming = true;
        self.builtin_streaming_content.clear();

        // Add placeholder for assistant response (assistant() auto-generates UUID)
        let assistant_message = ChatPromptMessage::assistant("").with_streaming(true);
        let assistant_msg_id = assistant_message.id.clone().unwrap_or_default();
        self.messages.push(assistant_message);
        self.streaming_message_id = Some(assistant_msg_id.clone());
        self.mark_conversation_turns_dirty();
        self.force_scroll_turns_to_bottom();
        cx.notify();

        logging::log(
            "CHAT",
            &format!(
                "Starting built-in AI call: model={}, provider={}, messages={}, request={}",
                model_id,
                provider,
                api_messages.len(),
                prepared.payload_fingerprint().0
            ),
        );

        self.prepared_requests_by_assistant_id
            .insert(assistant_msg_id.clone(), prepared.clone());
        self.builtin_replay_payloads.insert(
            prepared.request_ref().clone(),
            super::prompt::BuiltinPreparedPayload {
                provider: ai_provider.clone(),
                api_messages: api_messages.clone(),
                model_id: model_id.clone(),
            },
        );
        self.spawn_streaming_reveal(ai_provider, api_messages, model_id, assistant_msg_id, cx);
    }

    /// Handle initial response for pre-populated messages (scriptlets using chat())
    /// Unlike handle_builtin_ai_submit, this doesn't add a new user message - messages are already in self.messages
    pub(super) fn handle_initial_response(&mut self, cx: &mut Context<Self>) {
        // Don't allow if already streaming
        if self.builtin_is_streaming {
            return;
        }

        // Check if we have messages and the last one is from user
        let has_user_message = self.messages.last().map(|m| m.is_user()).unwrap_or(false);

        if !has_user_message {
            logging::log(
                "CHAT",
                "handle_initial_response: No user message to respond to",
            );
            return;
        }

        logging::log(
            "CHAT",
            &format!(
                "handle_initial_response: Auto-responding to {} initial messages",
                self.messages.len()
            ),
        );

        // Get the selected model and provider
        let (model_id, provider) = match &self.selected_model {
            Some(m) => (m.id.clone(), m.provider.clone()),
            None => {
                logging::log("CHAT", "No model selected for built-in AI initial response");
                let error_msg = ChatPromptMessage::assistant(
                    "No AI model configured. Please set up an API key.",
                );
                self.messages.push(error_msg);
                self.mark_conversation_turns_dirty();
                self.ensure_conversation_turns_cache();
                cx.notify();
                return;
            }
        };

        let registry = match &self.provider_registry {
            Some(r) => r.clone(),
            None => return,
        };

        let ai_provider = match registry.find_provider_for_model(&model_id) {
            Some(p) => p.clone(),
            None => {
                logging::log(
                    "CHAT",
                    &format!("No provider found for model: {}", model_id),
                );
                let error_msg = ChatPromptMessage::assistant(format!(
                    "Provider not found for model: {}",
                    model_id
                ));
                self.messages.push(error_msg);
                self.mark_conversation_turns_dirty();
                self.ensure_conversation_turns_cache();
                cx.notify();
                return;
            }
        };

        // Build messages for the API call (convert our messages to provider format)
        let mut api_messages: Vec<ProviderMessage> = Vec::new();
        if let Some(system_prompt) = self.builtin_system_prompt.clone() {
            api_messages.push(ProviderMessage::system(system_prompt));
        }
        api_messages.extend(self.messages.iter().map(|m| {
            if m.is_user() {
                ProviderMessage::user(m.get_content())
            } else if matches!(m.role, Some(crate::protocol::ChatMessageRole::System)) {
                ProviderMessage::system(m.get_content())
            } else {
                ProviderMessage::assistant(m.get_content())
            }
        }));

        // Set streaming state
        self.builtin_is_streaming = true;
        self.builtin_streaming_content.clear();

        // Add placeholder for assistant response
        let assistant_message = ChatPromptMessage::assistant("").with_streaming(true);
        let assistant_msg_id = assistant_message.id.clone().unwrap_or_default();
        self.messages.push(assistant_message);
        self.streaming_message_id = Some(assistant_msg_id.clone());
        self.mark_conversation_turns_dirty();
        self.force_scroll_turns_to_bottom();
        cx.notify();

        let initial_text = self
            .messages
            .iter()
            .rev()
            .find(|message| message.is_user())
            .map(|message| message.get_content().to_string())
            .unwrap_or_default();
        let prepared = ChatPromptPreparedRequest::new(&self.id, initial_text.clone(), initial_text);
        logging::log(
            "CHAT",
            &format!(
                "Starting built-in AI initial response: model={}, provider={}, messages={}, request={}",
                model_id,
                provider,
                api_messages.len(),
                prepared.payload_fingerprint().0
            ),
        );
        self.prepared_requests_by_assistant_id
            .insert(assistant_msg_id.clone(), prepared.clone());
        self.builtin_replay_payloads.insert(
            prepared.request_ref().clone(),
            super::prompt::BuiltinPreparedPayload {
                provider: ai_provider.clone(),
                api_messages: api_messages.clone(),
                model_id: model_id.clone(),
            },
        );

        self.spawn_streaming_reveal(ai_provider, api_messages, model_id, assistant_msg_id, cx);
    }

    /// Spawn the provider streaming thread and the word-buffered reveal loop.
    ///
    /// The background thread accumulates raw chunks into a shared buffer.
    /// The reveal loop reads from that buffer and advances a word-at-a-time
    /// watermark at ~30-55ms per word, giving a smooth typewriter feel.
    pub(super) fn spawn_streaming_reveal(
        &mut self,
        ai_provider: Arc<dyn crate::ai::providers::AiProvider>,
        api_messages: Vec<ProviderMessage>,
        model_id: String,
        msg_id: String,
        cx: &mut Context<Self>,
    ) {
        // Reset reveal state
        self.builtin_accumulated_content.clear();
        self.builtin_reveal_offset = 0;

        // Shared buffer between provider thread and reveal loop
        let shared_content = Arc::new(std::sync::Mutex::new(String::new()));
        let shared_done = Arc::new(AtomicBool::new(false));
        let shared_error = Arc::new(std::sync::Mutex::new(
            None::<crate::ai::reliability::AppFailureRecord>,
        ));
        let cancel_signal = Arc::new(AtomicBool::new(false));
        self.builtin_cancel_signal = Some(cancel_signal.clone());

        let content_clone = shared_content.clone();
        let done_clone = shared_done.clone();
        let error_clone = shared_error.clone();
        let model_id_clone = model_id.clone();
        let session_id = self.cli_session_id.clone();

        // Background thread: accumulate raw chunks from the provider
        let model_id_for_thread = model_id.clone();
        let cancel_for_thread = cancel_signal.clone();
        std::thread::spawn(move || {
            logging::log(
                "CHAT",
                &format!(
                    "Background provider thread started for model={}",
                    model_id_for_thread
                ),
            );
            let chunk_count = std::sync::atomic::AtomicUsize::new(0);
            let result = ai_provider.stream_message(
                &api_messages,
                &model_id_clone,
                Box::new(move |chunk| {
                    let count = chunk_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count == 0 {
                        logging::log("CHAT", "First chunk received from provider");
                    }
                    if cancel_for_thread.load(Ordering::SeqCst) {
                        return false;
                    }
                    if let Ok(mut content) = content_clone.lock() {
                        content.push_str(&chunk);
                    }
                    !cancel_for_thread.load(Ordering::SeqCst)
                }),
                Some(&session_id),
            );

            match result {
                Ok(()) => {
                    logging::log("CHAT", "Provider stream_message completed successfully");
                    done_clone.store(true, Ordering::SeqCst);
                }
                Err(e) => {
                    let failure = crate::ai::reliability::provider_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Provider,
                        e.to_string(),
                    );
                    logging::log(
                        "CHAT",
                        &format!("Provider stream_message failed: {:?}", failure.failure.code),
                    );
                    if let Ok(mut err) = error_clone.lock() {
                        *err = Some(failure);
                    }
                    done_clone.store(true, Ordering::SeqCst);
                }
            }
        });

        // Word-buffered reveal loop
        let content_for_poll = shared_content.clone();
        let done_for_poll = shared_done.clone();
        let error_for_poll = shared_error.clone();
        let msg_id_for_loop = msg_id.clone();

        cx.spawn(async move |this, cx| {
            let mut delay_counter: u64 = 0;

            loop {
                // Variable delay per word: 50-80ms for natural pacing
                // (kept above 50ms to avoid excessive markdown re-parsing)
                delay_counter = delay_counter.wrapping_add(17);
                let delay = 50 + (delay_counter % 30);
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;

                // Read is_done BEFORE accumulated content to avoid race condition:
                // if done is true, stream_message has returned, so all chunks have
                // been pushed to shared_content. Reading in the other order could
                // snapshot partial content then see done=true, finalizing truncated text.
                let is_done = done_for_poll.load(Ordering::SeqCst);
                let accumulated = content_for_poll.lock().ok().map(|c| c.clone());
                let error = if is_done {
                    error_for_poll.lock().ok().and_then(|e| e.clone())
                } else {
                    None
                };

                let Some(full_text) = accumulated else {
                    continue;
                };

                let msg_id = msg_id_for_loop.clone();
                let should_break = match cx.update(|cx| {
                    this.update(cx, |chat, cx| {
                        if should_ignore_stream_reveal_update(
                            chat.streaming_message_id.as_deref(),
                            &msg_id,
                        ) {
                            logging::log(
                                "CHAT",
                                "Stopping stale stream reveal loop after stream handoff/stop",
                            );
                            return true;
                        }

                        // Error path
                        if let Some(failure) = &error {
                            logging::log(
                                "CHAT",
                                &format!("Built-in AI error: {:?}", failure.failure.code),
                            );
                            chat.builtin_is_streaming = false;
                            chat.builtin_cancel_signal = None;
                            chat.streaming_message_id = None;
                            if let Some(msg) = chat
                                .messages
                                .iter_mut()
                                .find(|m| m.id.as_deref() == Some(&msg_id))
                            {
                                msg.error = Some(failure.primary_message().to_string());
                                msg.failure = Some(failure.failure.clone());
                                msg.streaming = false;
                            }
                            chat.mark_conversation_turns_dirty();
                            chat.ensure_conversation_turns_cache();
                            chat.scroll_turns_to_bottom();
                            cx.notify();
                            return true; // break
                        }

                        let current_offset = chat.builtin_reveal_offset;

                        if is_done {
                            // Stream finished: flush everything remaining.
                            // Always set content here — don't rely on offset comparison,
                            // because accumulated content may have grown since the offset
                            // was last stored (the previous tick's full_text was shorter).
                            chat.builtin_reveal_offset = full_text.len();
                            if let Some(msg) = chat
                                .messages
                                .iter_mut()
                                .find(|m| m.id.as_deref() == Some(&msg_id))
                            {
                                msg.set_content(&full_text);
                            }
                            chat.builtin_streaming_content = full_text.clone();
                            chat.builtin_accumulated_content = full_text.clone();
                            chat.mark_conversation_turns_dirty();
                            chat.scroll_turns_to_bottom();
                            cx.notify();
                        } else if let Some(new_offset) =
                            next_reveal_boundary(&full_text, current_offset)
                        {
                            if new_offset > current_offset {
                                chat.builtin_reveal_offset = new_offset;
                                let revealed = &full_text[..new_offset];

                                if let Some(msg) = chat
                                    .messages
                                    .iter_mut()
                                    .find(|m| m.id.as_deref() == Some(&msg_id))
                                {
                                    msg.set_content(revealed);
                                }
                                chat.builtin_streaming_content = revealed.to_string();
                                chat.builtin_accumulated_content = full_text.clone();
                                chat.mark_conversation_turns_dirty();
                                chat.scroll_turns_to_bottom();
                                cx.notify();
                            }
                        }

                        // Check completion: done AND fully revealed
                        if is_done {
                            logging::log(
                                "CHAT",
                                &format!("Built-in AI complete: {} chars", full_text.len()),
                            );
                            chat.builtin_is_streaming = false;
                            chat.builtin_cancel_signal = None;
                            chat.streaming_message_id = None;
                            if let Some(msg) = chat
                                .messages
                                .iter_mut()
                                .find(|m| m.id.as_deref() == Some(&msg_id))
                            {
                                msg.streaming = false;
                            }
                            chat.mark_conversation_turns_dirty();
                            chat.ensure_conversation_turns_cache();
                            chat.scroll_turns_to_bottom();
                            cx.notify();
                            return true; // break
                        }

                        false // continue
                    })
                }) {
                    Ok(should_break) => should_break,
                    Err(error) => {
                        logging::log(
                            "CHAT",
                            &format!("Stopping stream reveal loop due to update error: {}", error),
                        );
                        true
                    }
                };

                if should_break {
                    break;
                }
            }
        })
        .detach();
    }
}
