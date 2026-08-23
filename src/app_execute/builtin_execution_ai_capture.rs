impl ScriptListApp {
    fn spawn_send_screen_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
        let capture_action = AiImageCaptureBuiltinAction::screen();
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "SendScreenToAi",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling screen capture for AI"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let capture_result = cx
                .background_executor()
                .spawn(async { platform::capture_screen_screenshot() })
                .await;

            match capture_result {
                Ok((png_data, width, height)) => {
                    let size_bytes = png_data.len();
                    if size_bytes > crate::prompts::chat::MAX_IMAGE_BYTES {
                        tracing::warn!(
                            category = "AI",
                            event = "ai_capture_rejected",
                            source_action = "SendScreenToAi",
                            trace_id = %trace_id,
                            size_bytes,
                            max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                            "Rejecting screen capture larger than 10 MB"
                        );
                        this.update(cx, |this, cx| {
                            this.show_error_toast(
                                "Screen capture exceeds 10 MB limit".to_string(),
                                cx,
                            );
                        })
                        .ok();
                        return;
                    }

                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &png_data,
                    );
                    let message = format!(
                        "[Screenshot captured: {}x{} pixels]\n\nPlease analyze this screenshot.",
                        width, height
                    );

                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendScreenToAi",
                        trace_id = %trace_id,
                        width,
                        height,
                        size_bytes,
                        "Screen captured for AI"
                    );

                    this.update(cx, |this, cx| {
                        this.open_agent_chat_after_already_hidden(
                            "SendScreenToAi",
                            &trace_id,
                            DeferredAgentChatAction::SetInputWithImage {
                                text: message,
                                image_base64: base64_data,
                                submit: false,
                            },
                            cx,
                        );
                    })
                    .ok();
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action = "SendScreenToAi",
                        trace_id = %trace_id,
                        error = %error,
                        "Failed to capture screen for AI"
                    );
                    let message = capture_action.failure_message(&error);
                    this.update(cx, |this, cx| {
                        this.show_error_toast(message, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn spawn_send_focused_window_to_ai_after_hide(
        &mut self,
        trace_id: &str,
        cx: &mut Context<Self>,
    ) {
        let capture_action = AiImageCaptureBuiltinAction::focused_window();
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "SendFocusedWindowToAi",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling focused window capture for AI"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let capture_result = cx
                .background_executor()
                .spawn(async { platform::capture_focused_window_screenshot() })
                .await;

            match capture_result {
                Ok(capture) => {
                    let size_bytes = capture.png_data.len();
                    if size_bytes > crate::prompts::chat::MAX_IMAGE_BYTES {
                        tracing::warn!(
                            category = "AI",
                            event = "ai_capture_rejected",
                            source_action = "SendFocusedWindowToAi",
                            trace_id = %trace_id,
                            size_bytes,
                            max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                            "Rejecting window capture larger than 10 MB"
                        );
                        this.update(cx, |this, cx| {
                            this.show_error_toast(
                                "Window capture exceeds 10 MB limit".to_string(),
                                cx,
                            );
                        })
                        .ok();
                        return;
                    }

                    let fallback_warning = capture.used_fallback.then(|| {
                        format!(
                            "No focused window found — captured '{}'",
                            capture.window_title
                        )
                    });
                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &capture.png_data,
                    );
                    let message = format!(
                        "[Window: {} - {}x{} pixels]\n\nPlease analyze this window screenshot.",
                        capture.window_title, capture.width, capture.height
                    );

                    let safe_window_title =
                        crate::logging::log_private_user_value(&capture.window_title);
                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendFocusedWindowToAi",
                        trace_id = %trace_id,
                        window_title_bytes = safe_window_title.raw_bytes,
                        window_title_sha256 = %safe_window_title.sha256,
                        width = capture.width,
                        height = capture.height,
                        size_bytes,
                        used_fallback = capture.used_fallback,
                        "Focused window captured for AI"
                    );

                    this.update(cx, |this, cx| {
                        if let Some(warning_message) = fallback_warning {
                            this.toast_manager.push(
                                components::toast::Toast::warning(warning_message, &this.theme)
                                    .duration_ms(Some(TOAST_WARNING_MS)),
                            );
                            cx.notify();
                        }

                        this.open_agent_chat_after_already_hidden(
                            "SendFocusedWindowToAi",
                            &trace_id,
                            DeferredAgentChatAction::SetInputWithImage {
                                text: message,
                                image_base64: base64_data,
                                submit: false,
                            },
                            cx,
                        );
                    })
                    .ok();
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action = "SendFocusedWindowToAi",
                        trace_id = %trace_id,
                        error = %error,
                        "Failed to capture focused window for AI"
                    );
                    let message = capture_action.failure_message(&error);
                    this.update(cx, |this, cx| {
                        this.show_error_toast(message, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn spawn_send_screen_area_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
        let capture_action = AiImageCaptureBuiltinAction::screen_area();
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "SendScreenAreaToAi",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling screen area capture for AI"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let capture_result = cx
                .background_executor()
                .spawn(async { platform::capture_screen_area() })
                .await;

            match capture_result {
                Ok(Some(capture)) => {
                    let size_bytes = capture.png_data.len();
                    if size_bytes > crate::prompts::chat::MAX_IMAGE_BYTES {
                        tracing::warn!(
                            category = "AI",
                            event = "ai_capture_rejected",
                            source_action = "SendScreenAreaToAi",
                            trace_id = %trace_id,
                            size_bytes,
                            max_bytes = crate::prompts::chat::MAX_IMAGE_BYTES,
                            "Rejecting screen area capture larger than 10 MB"
                        );
                        this.update(cx, |this, cx| {
                            this.show_error_toast(
                                "Screen area capture exceeds 10 MB limit".to_string(),
                                cx,
                            );
                        })
                        .ok();
                        return;
                    }

                    let base64_data = base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        &capture.png_data,
                    );
                    let message = format!(
                        "[Screen area captured: {}x{} pixels]\n\nPlease analyze this selected screen area.",
                        capture.width, capture.height
                    );

                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendScreenAreaToAi",
                        trace_id = %trace_id,
                        width = capture.width,
                        height = capture.height,
                        size_bytes,
                        "Screen area captured for AI"
                    );

                    this.update(cx, |this, cx| {
                        this.open_agent_chat_after_already_hidden(
                            "SendScreenAreaToAi",
                            &trace_id,
                            DeferredAgentChatAction::SetInputWithImage {
                                text: message,
                                image_base64: base64_data,
                                submit: false,
                            },
                            cx,
                        );
                    })
                    .ok();
                }
                Ok(None) => {
                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_cancelled",
                        source_action = "SendScreenAreaToAi",
                        trace_id = %trace_id,
                        "Screen area selection cancelled by user"
                    );
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action = "SendScreenAreaToAi",
                        trace_id = %trace_id,
                        error = %error,
                        "Failed to capture screen area for AI"
                    );
                    let message = capture_action.failure_message(&error);
                    this.update(cx, |this, cx| {
                        this.show_error_toast(message, cx);
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_capture_text_to_ai_after_already_hidden<C, F>(
        &mut self,
        source_action: &'static str,
        trace_id: &str,
        capture_kind: &'static str,
        capture_fn: C,
        format_fn: F,
        cx: &mut Context<Self>,
    ) where
        C: FnOnce() -> Result<DeferredAiCapturedText, String> + Send + 'static,
        F: FnOnce(String) -> String + Send + 'static,
    {
        let trace_id = trace_id.to_string();

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action,
            trace_id = %trace_id,
            capture_kind,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Scheduled deferred AI text capture"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let (result_tx, result_rx) =
                async_channel::bounded::<Result<DeferredAiCapturedText, String>>(1);

            let trace_id_for_thread = trace_id.clone();
            std::thread::spawn(move || {
                let started_at = std::time::Instant::now();
                let result = capture_fn();

                let (success, result_state) = match &result {
                    Ok(DeferredAiCapturedText::Ready(_)) => (true, "ready"),
                    Ok(DeferredAiCapturedText::Empty(_)) => (true, "empty"),
                    Err(_) => (false, "error"),
                };

                tracing::info!(
                    category = "AI",
                    event = "ai_capture_completed",
                    source_action,
                    trace_id = %trace_id_for_thread,
                    capture_kind,
                    result_state,
                    success,
                    duration_ms = started_at.elapsed().as_millis() as u64,
                    "Deferred AI text capture finished"
                );

                let _ = result_tx.send_blocking(result);
            });

            let Ok(result) = result_rx.recv().await else {
                return;
            };

            let _ = this.update(cx, |this, cx| match result {
                Ok(DeferredAiCapturedText::Ready(captured)) => {
                    this.open_agent_chat_after_already_hidden(
                        source_action,
                        &trace_id,
                        DeferredAgentChatAction::SetInput {
                            text: format_fn(captured),
                            submit: false,
                        },
                        cx,
                    );
                }
                Ok(DeferredAiCapturedText::Empty(message)) => {
                    this.toast_manager.push(
                        components::toast::Toast::info(message, &this.theme)
                            .duration_ms(Some(TOAST_INFO_MS)),
                    );
                    cx.notify();
                }
                Err(error) => {
                    tracing::error!(
                        category = "AI",
                        event = "ai_capture_failed",
                        source_action,
                        trace_id = %trace_id,
                        capture_kind,
                        error = %error,
                        "Deferred AI text capture failed"
                    );
                    let message = AiTextCaptureBuiltinAction::agent_chat_failure_message(&error);
                    this.toast_manager.push(
                        components::toast::Toast::error(message, &this.theme)
                            .duration_ms(Some(TOAST_CRITICAL_MS)),
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn spawn_send_selected_text_to_ai_after_hide(
        &mut self,
        trace_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.spawn_capture_text_to_ai_after_already_hidden(
            "SendSelectedTextToAi",
            trace_id,
            "selected_text",
            || {
                crate::selected_text::get_selected_text()
                    .map_err(|error| error.to_string())
                    .map(|text| {
                        let trimmed = text.trim().to_string();
                        if trimmed.is_empty() {
                            DeferredAiCapturedText::Empty(
                                "No text selected. Select some text first.".to_string(),
                            )
                        } else {
                            DeferredAiCapturedText::Ready(trimmed)
                        }
                    })
            },
            |text| {
                format!(
                    "I've selected the following text:\n\n```\n{}\n```\n\nPlease help me with this.",
                    text
                )
            },
            cx,
        );
    }

    fn spawn_send_browser_tab_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
        self.spawn_capture_text_to_ai_after_already_hidden(
            "SendBrowserTabToAi",
            trace_id,
            "browser_url",
            || {
                platform::get_focused_browser_tab_url()
                    .map_err(|error| error.to_string())
                    .map(|url| {
                        let trimmed = url.trim().to_string();
                        if trimmed.is_empty() {
                            DeferredAiCapturedText::Empty(
                                "No browser URL found in the frontmost tab.".to_string(),
                            )
                        } else {
                            DeferredAiCapturedText::Ready(trimmed)
                        }
                    })
            },
            |url| {
                format!(
                    "I'm looking at this webpage:\n\n{}\n\nPlease help me analyze or understand its content.",
                    url
                )
            },
            cx,
        );
    }

    fn spawn_generate_script_from_current_app_after_hide(
        &mut self,
        trace_id: String,
        query_override: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let fallback_query = query_override.unwrap_or_else(|| self.filter_text.clone());

        tracing::info!(
            category = "AI",
            event = "ai_capture_scheduled",
            source_action = "GenerateScriptFromCurrentApp",
            trace_id = %trace_id,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling context capture for script generation"
        );

        platform::defer_hide_main_window(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            let snapshot_result = cx
                .background_executor()
                .spawn(async { crate::menu_bar::load_frontmost_menu_snapshot() })
                .await;

            let selected_text = match crate::selected_text::get_selected_text() {
                Ok(text) if !text.trim().is_empty() => Some(text),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(
                        trace_id = %trace_id,
                        error = %error,
                        "ai_generate_script_from_current_app.selected_text_unavailable"
                    );
                    None
                }
            };

            let browser_url = match platform::get_focused_browser_tab_url() {
                Ok(url) if !url.trim().is_empty() => Some(url),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!(
                        trace_id = %trace_id,
                        error = %error,
                        "ai_generate_script_from_current_app.browser_url_unavailable"
                    );
                    None
                }
            };

            // Build prompt outside entity borrow so we can show window safely.
            let prompt_or_error = match snapshot_result {
                Ok(snapshot) => {
                    let user_request =
                        crate::menu_bar::current_app_commands::normalize_generate_script_from_current_app_request(
                            Some(fallback_query.as_str()),
                        );

                    let (prompt, receipt) =
                        crate::menu_bar::current_app_commands::build_generate_script_prompt_from_snapshot(
                            snapshot,
                            user_request,
                            selected_text.as_deref(),
                            browser_url.as_deref(),
                        );

                    tracing::info!(
                        trace_id = %trace_id,
                        app_name = %receipt.app_name,
                        bundle_id = %receipt.bundle_id,
                        total_menu_items = receipt.total_menu_items,
                        included_menu_items = receipt.included_menu_items,
                        included_user_request = receipt.included_user_request,
                        included_selected_text = receipt.included_selected_text,
                        included_browser_url = receipt.included_browser_url,
                        "ai_generate_script_from_current_app.prompt_ready"
                    );

                    Ok(prompt)
                }
                Err(error) => Err(error),
            };

            match prompt_or_error {
                Ok(prompt) => {
                    // Platform calls — trigger macOS delegate callbacks.
                    // Safe here: no AppCell borrow is active.
                    script_kit_gpui::set_main_window_visible(true);
                    tracing::info!(
                        trace_id = %trace_id,
                        "ai_generate_script_from_current_app.showing_window"
                    );
                    crate::platform::show_main_window_without_activation();

                    // GPUI state changes inside entity borrow.
                    let _ = this.update(cx, |app, cx| {
                        app.dispatch_ai_script_generation_from_query(prompt, cx);
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| {
                        let message =
                            AiTextCaptureBuiltinAction::current_app_failure_message(&error);
                        app.show_error_toast(message.clone(), cx);
                        tracing::error!(
                            trace_id = %trace_id,
                            error = %error,
                            "ai_generate_script_from_current_app.capture_failed"
                        );
                    });
                }
            }
        })
        .detach();
    }

    /// Like `spawn_generate_script_from_current_app_after_hide`, but reuses an
    /// already-built recipe instead of recapturing live context after hide.
    ///
    /// This eliminates prompt drift: the prompt copied in the recipe is
    /// byte-for-byte the prompt sent to the AI generation path.
    fn spawn_generate_script_from_recipe_after_hide(
        &mut self,
        trace_id: String,
        recipe: crate::menu_bar::current_app_commands::CurrentAppCommandRecipe,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            category = "AI",
            event = "ai_recipe_generation_scheduled",
            source_action = "TurnThisIntoCommand",
            trace_id = %trace_id,
            recipe_prompt_bytes = recipe.prompt.len(),
            recipe_bundle_id = %recipe.prompt_receipt.bundle_id,
            recipe_included_selected_text = recipe.prompt_receipt.included_selected_text,
            recipe_included_browser_url = recipe.prompt_receipt.included_browser_url,
            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
            "Deferring main window hide and scheduling recipe-based script generation (no recapture)"
        );

        platform::defer_hide_main_window(cx);

        let prompt =
            crate::menu_bar::current_app_commands::build_generated_script_prompt_from_recipe(
                &recipe,
            );

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(ai_capture_hide_settle_duration())
                .await;

            tracing::info!(
                trace_id = %trace_id,
                recipe_prompt_bytes = prompt.len(),
                recipe_bundle_id = %recipe.prompt_receipt.bundle_id,
                recipe_included_selected_text = recipe.prompt_receipt.included_selected_text,
                recipe_included_browser_url = recipe.prompt_receipt.included_browser_url,
                "ai_generate_script_from_recipe.prompt_ready"
            );

            // Platform calls — trigger macOS delegate callbacks.
            // Safe here: no AppCell borrow is active.
            script_kit_gpui::set_main_window_visible(true);
            tracing::info!(
                trace_id = %trace_id,
                "ai_generate_script_from_recipe.showing_window"
            );
            crate::platform::show_main_window_without_activation();

            // GPUI state changes inside entity borrow.
            let _ = this.update(cx, |app, cx| {
                app.dispatch_ai_script_generation_from_query(prompt, cx);
            });
        })
        .detach();
    }

    /// Schedule the DoInCurrentApp→GenerateScript flow, capturing selected
    /// text and the focused browser URL off the UI thread.
    ///
    /// Both `get_selected_text()` (AX-first, clipboard fallback) and
    /// `get_focused_browser_tab_url()` (single `osascript` call gated by the
    /// in-process frontmost-app tracker) can block for hundreds of
    /// milliseconds. Running them on `cx.background_executor()` keeps the
    /// launcher responsive while macOS answers; the memory lookup, recipe
    /// build, and dispatch run back on the main thread once capture completes.
    fn spawn_generate_script_from_current_app_with_capture(
        &mut self,
        request: CurrentAppScriptCaptureRequest,
        cx: &mut Context<Self>,
    ) {
        let safe_query = crate::logging::log_private_user_value(&request.raw_query);
        tracing::info!(
            trace_id = %request.trace_id,
            raw_query_bytes = safe_query.raw_bytes,
            raw_query_sha256 = %safe_query.sha256,
            "do_in_current_app.spawn_context_capture"
        );

        cx.spawn(async move |this, cx| {
            let capture_started_at = std::time::Instant::now();
            let (selected_text, browser_url) = cx
                .background_executor()
                .spawn(async {
                    let selected_text = crate::selected_text::get_selected_text()
                        .ok()
                        .filter(|text| !text.trim().is_empty());
                    let browser_url = crate::platform::get_focused_browser_tab_url()
                        .ok()
                        .filter(|url| !url.trim().is_empty());
                    (selected_text, browser_url)
                })
                .await;

            tracing::info!(
                trace_id = %request.trace_id,
                capture_ms = capture_started_at.elapsed().as_millis() as u64,
                has_selected_text = selected_text.is_some(),
                has_browser_url = browser_url.is_some(),
                "do_in_current_app.context_capture_complete"
            );

            let _ = this.update(cx, |this, cx| {
                this.continue_generate_script_from_current_app_after_capture(
                    request,
                    CurrentAppCapturedContext {
                        selected_text,
                        browser_url,
                    },
                    cx,
                );
            });
        })
        .detach();
    }

    /// Continuation of `spawn_generate_script_from_current_app_with_capture`,
    /// invoked back on the main thread once the blocking capture finishes.
    fn continue_generate_script_from_current_app_after_capture(
        &mut self,
        request: CurrentAppScriptCaptureRequest,
        captured: CurrentAppCapturedContext,
        cx: &mut Context<Self>,
    ) {
        let CurrentAppScriptCaptureRequest {
            trace_id,
            raw_query: raw_query_owned,
            snapshot: snapshot_for_recipe,
            entries,
            snapshot_receipt,
            snapshot_pid,
        } = request;
        let CurrentAppCapturedContext {
            selected_text,
            browser_url,
        } = captured;
        let memory_decision = crate::ai::resolve_current_app_automation_from_memory(
            &raw_query_owned,
            &snapshot_for_recipe,
            &entries,
            selected_text.as_deref(),
            browser_url.as_deref(),
        );

        if let Ok(ref decision) = memory_decision {
            if let Some(ref replay) = decision.replay {
                tracing::info!(
                    category = "CURRENT_APP_AUTOMATION_MEMORY",
                    trace_id = %trace_id,
                    action = %decision.action,
                    best_score = decision.best_score,
                    matched_slug = decision
                        .matched
                        .as_ref()
                        .map(|entry| entry.slug.as_str())
                        .unwrap_or(""),
                    reason = %decision.reason,
                    "do_in_current_app.memory_resolved"
                );

                match decision.action.as_str() {
                    "replay_recipe" => match replay.action.as_str() {
                        "execute_entry" => {
                            if let Some(entry_index) = replay.selected_entry_index {
                                if entry_index < entries.len() {
                                    let entry = entries[entry_index].clone();
                                    let dctx = crate::action_helpers::DispatchContext {
                                        trace_id: trace_id.clone(),
                                        surface: crate::action_helpers::DispatchSurface::Builtin,
                                        action_id: entry.id.clone(),
                                    };
                                    let _ = self.execute_builtin_inner(
                                        &entry,
                                        Some(&raw_query_owned),
                                        &dctx,
                                        cx,
                                    );
                                    return;
                                }
                            }
                        }
                        "open_command_palette" => {
                            let filter = replay.verification.live_recipe.effective_query.clone();
                            self.present_current_app_commands_entries(
                                entries.clone(),
                                &snapshot_receipt,
                                snapshot_pid,
                                &filter,
                                cx,
                            );
                            return;
                        }
                        "generate_script" => {
                            self.spawn_generate_script_from_recipe_after_hide(
                                trace_id.clone(),
                                replay.verification.live_recipe.clone(),
                                cx,
                            );
                            return;
                        }
                        _ => {}
                    },
                    "repair_recipe" => {
                        self.spawn_generate_script_from_recipe_after_hide(
                            trace_id.clone(),
                            replay.verification.live_recipe.clone(),
                            cx,
                        );
                        return;
                    }
                    _ => {}
                }
            }
        }

        let recipe = crate::menu_bar::current_app_commands::build_current_app_command_recipe(
            snapshot_for_recipe,
            Some(&raw_query_owned),
            selected_text.as_deref(),
            browser_url.as_deref(),
        );

        match serde_json::to_string_pretty(&recipe) {
            Ok(json) => {
                let safe_query = crate::logging::log_private_user_value(&recipe.effective_query);
                let safe_script_name =
                    crate::logging::log_private_user_value(&recipe.suggested_script_name);
                tracing::info!(
                    category = "CURRENT_APP_RECIPE",
                    trace_id = %trace_id,
                    app_name = %recipe.prompt_receipt.app_name,
                    bundle_id = %recipe.prompt_receipt.bundle_id,
                    effective_query_bytes = safe_query.raw_bytes,
                    effective_query_sha256 = %safe_query.sha256,
                    route = %recipe.trace.action,
                    suggested_script_name_bytes = safe_script_name.raw_bytes,
                    suggested_script_name_sha256 = %safe_script_name.sha256,
                    included_selected_text = recipe.prompt_receipt.included_selected_text,
                    included_browser_url = recipe.prompt_receipt.included_browser_url,
                    json_bytes = json.len(),
                    "do_in_current_app.recipe_prepared"
                );
            }
            Err(error) => {
                tracing::warn!(
                    trace_id = %trace_id,
                    error = %error,
                    "do_in_current_app.recipe_serialize_failed"
                );
            }
        }

        self.spawn_generate_script_from_recipe_after_hide(trace_id, recipe, cx);
    }
}
