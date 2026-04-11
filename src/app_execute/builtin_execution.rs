/// Delay between hiding the main window and starting a synchronous screenshot capture.
const AI_CAPTURE_HIDE_SETTLE_MS: u64 = 150;

/// Synthetic prompt ID used when the microphone-selection MiniPrompt is open.
/// Checked in `submit_arg_prompt_from_current_state` to intercept the submit
/// and persist the chosen device instead of sending a protocol message.
const BUILTIN_MIC_SELECT_PROMPT_ID: &str = "builtin:select-microphone";

/// Choice value representing "use system default" in the mic-selection prompt.
const BUILTIN_MIC_DEFAULT_VALUE: &str = "__system_default__";

/// Synthetic prompt ID for the dictation model download consent prompt.
/// Checked in `submit_arg_prompt_from_current_state` to intercept submit
/// and either start the Parakeet download or cancel.
const BUILTIN_DICTATION_MODEL_PROMPT_ID: &str = "builtin:dictation-model";

/// Choice value: user wants to download the Parakeet model.
const BUILTIN_DICTATION_MODEL_DOWNLOAD: &str = "download";

/// Choice value: user declines the download for now.
const BUILTIN_DICTATION_MODEL_CANCEL: &str = "cancel";

/// Choice value: user wants to hide the prompt while download continues.
const BUILTIN_DICTATION_MODEL_HIDE: &str = "builtin/dictation-model-hide";

/// Generate a stable semantic ID for a built-in prompt choice.
///
/// Format: `{prompt_id}:choice:{index}:{value_slug}`
///
/// `prompt_id` already contains the `builtin:` prefix (e.g. `builtin:select-microphone`).
fn builtin_choice_semantic_id(prompt_id: &str, index: usize, value: &str) -> String {
    crate::protocol::generate_semantic_id(&format!("{prompt_id}:choice"), index, value)
}

/// Typed progress events sent from the blocking download thread to the
/// async context for updating the in-prompt progress display.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DictationModelProgressEvent {
    Downloading {
        percentage: u8,
        downloaded_bytes: u64,
        total_bytes: u64,
        speed_bytes_per_sec: u64,
        eta_seconds: Option<u64>,
    },
    Extracting,
}

/// Simple rolling-window speed tracker for download progress.
struct SpeedTracker {
    last_bytes: u64,
    last_time: std::time::Instant,
    speed: u64,
}

impl SpeedTracker {
    fn new() -> Self {
        Self {
            last_bytes: 0,
            last_time: std::time::Instant::now(),
            speed: 0,
        }
    }

    fn update(&mut self, downloaded: u64) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        if elapsed >= 0.5 {
            let delta = downloaded.saturating_sub(self.last_bytes);
            self.speed = (delta as f64 / elapsed) as u64;
            self.last_bytes = downloaded;
            self.last_time = now;
        }
    }

    fn speed_bytes_per_sec(&self) -> u64 {
        self.speed
    }
}

/// Phases tracked by the UI coalescing emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationModelUiPhase {
    Downloading,
    Extracting,
}

/// Snapshot of the last UI-visible state, used to decide whether a new
/// progress event is worth publishing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DictationModelUiSnapshot {
    phase: DictationModelUiPhase,
    percentage: u8,
    eta_bucket_seconds: Option<u64>,
}

impl DictationModelUiSnapshot {
    fn downloading(percentage: u8, eta_seconds: Option<u64>) -> Self {
        Self {
            phase: DictationModelUiPhase::Downloading,
            percentage,
            eta_bucket_seconds: bucket_dictation_eta_seconds(eta_seconds),
        }
    }

    fn extracting() -> Self {
        Self {
            phase: DictationModelUiPhase::Extracting,
            percentage: 100,
            eta_bucket_seconds: Some(0),
        }
    }
}

/// Gates cosmetic UI updates so the download thread is never blocked on
/// repaints.  Publishes on meaningful change or after a ~300 ms heartbeat.
#[derive(Debug, Default)]
struct DictationModelUiEmitter {
    last_emit_at: Option<std::time::Instant>,
    last_snapshot: Option<DictationModelUiSnapshot>,
}

impl DictationModelUiEmitter {
    fn should_emit(&self, now: std::time::Instant, next: &DictationModelUiSnapshot) -> bool {
        const HEARTBEAT: std::time::Duration = std::time::Duration::from_millis(300);

        let Some(last_snapshot) = self.last_snapshot.as_ref() else {
            return true;
        };
        let Some(last_emit_at) = self.last_emit_at else {
            return true;
        };

        if last_snapshot.phase != next.phase {
            return true;
        }
        if last_snapshot.percentage != next.percentage {
            return true;
        }
        if last_snapshot.eta_bucket_seconds != next.eta_bucket_seconds {
            return true;
        }

        now.duration_since(last_emit_at) >= HEARTBEAT
    }

    fn record_emit(&mut self, now: std::time::Instant, next: &DictationModelUiSnapshot) {
        self.last_emit_at = Some(now);
        self.last_snapshot = Some(next.clone());
    }
}

/// Bucket ETA seconds into human-friendly steps so minor fluctuations
/// don't trigger a UI repaint.
fn bucket_dictation_eta_seconds(eta_seconds: Option<u64>) -> Option<u64> {
    eta_seconds.map(|value| match value {
        0..=15 => value,
        16..=60 => value - (value % 5),
        61..=300 => value - (value % 15),
        _ => value - (value % 60),
    })
}

/// Prevent overlapping Parakeet model downloads when the dictation hotkey is
/// pressed repeatedly while the model is still missing.
static PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static DICTATION_MODEL_PROMPT_STATUS: std::sync::OnceLock<
    parking_lot::Mutex<crate::dictation::DictationModelStatus>,
> = std::sync::OnceLock::new();

fn dictation_model_prompt_status(
) -> &'static parking_lot::Mutex<crate::dictation::DictationModelStatus> {
    DICTATION_MODEL_PROMPT_STATUS.get_or_init(|| {
        parking_lot::Mutex::new(crate::dictation::DictationModelStatus::NotDownloaded)
    })
}

static PARAKEET_MODEL_DOWNLOAD_CANCEL: std::sync::OnceLock<
    parking_lot::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
> = std::sync::OnceLock::new();

fn parakeet_model_download_cancel_slot(
) -> &'static parking_lot::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> {
    PARAKEET_MODEL_DOWNLOAD_CANCEL.get_or_init(|| parking_lot::Mutex::new(None))
}

#[cfg(test)]
fn ai_open_failure_message(error: impl std::fmt::Display) -> String {
    format!("Failed to open AI: {}", error)
}

#[derive(Debug)]
enum DeferredAiCapturedText {
    Ready(String),
    Empty(String),
}

fn ai_capture_hide_settle_duration() -> std::time::Duration {
    std::time::Duration::from_millis(AI_CAPTURE_HIDE_SETTLE_MS)
}

fn ai_command_keeps_main_window_visible(cmd_type: &builtins::AiCommandType) -> bool {
    // All active AI commands now route to the harness terminal, which is
    // a view inside the main window — keep it visible.
    match cmd_type {
        builtins::AiCommandType::GenerateScript
        | builtins::AiCommandType::GenerateScriptFromCurrentApp
        | builtins::AiCommandType::SendScreenToAi
        | builtins::AiCommandType::SendFocusedWindowToAi
        | builtins::AiCommandType::SendSelectedTextToAi
        | builtins::AiCommandType::SendBrowserTabToAi
        | builtins::AiCommandType::SendScreenAreaToAi => true,
        // Legacy aliases (OpenAi, MiniAi, NewConversation, ClearConversation)
        // also open the harness terminal inside the main window.
        cmd if cmd.is_legacy_harness_alias() => true,
        // Preset commands (debug-only) retain their original behavior.
        _ => false,
    }
}

fn ai_command_uses_hide_then_capture_flow(_cmd_type: &builtins::AiCommandType) -> bool {
    // Legacy capture flow no longer needed — all active AI commands route to
    // the harness terminal which captures context inline.
    false
}

#[cfg(test)]
fn favorites_loaded_message(count: usize) -> String {
    if count == 1 {
        "Loaded 1 favorite".to_string()
    } else {
        format!("Loaded {} favorites", count)
    }
}

#[cfg(test)]
fn created_file_path_for_feedback(path: &std::path::Path) -> std::path::PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    match std::env::current_dir() {
        Ok(current_dir) => current_dir.join(path),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn applescript_list_literal(values: &[String]) -> String {
    let escaped_values = values
        .iter()
        .map(|value| format!("\"{}\"", crate::utils::escape_applescript_string(value)))
        .join(", ");
    format!("{{{}}}", escaped_values)
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn choose_from_list(
    prompt: &str,
    ok_button: &str,
    values: &[String],
) -> Result<Option<String>, String> {
    if values.is_empty() {
        return Ok(None);
    }

    let list_literal = applescript_list_literal(values);
    let script = format!(
        r#"set selectedItem to choose from list {list_literal} with prompt "{prompt}" OK button name "{ok_button}" cancel button name "Cancel" without multiple selections allowed
if selectedItem is false then
    return ""
end if
return item 1 of selectedItem"#,
        list_literal = list_literal,
        prompt = crate::utils::escape_applescript_string(prompt),
        ok_button = crate::utils::escape_applescript_string(ok_button),
    );

    let selected = crate::platform::run_osascript(&script, "builtin_picker_choose_from_list")
        .map_err(|error| error.to_string())?;
    if selected.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selected))
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Retained for potential future AppleScript-based pickers
fn prompt_for_text(
    prompt: &str,
    default_value: &str,
    ok_button: &str,
) -> Result<Option<String>, String> {
    let script = format!(
        r#"try
set dialogResult to display dialog "{prompt}" default answer "{default_value}" buttons {{"Cancel", "{ok_button}"}} default button "{ok_button}"
return text returned of dialogResult
on error number -128
return ""
end try"#,
        prompt = crate::utils::escape_applescript_string(prompt),
        default_value = crate::utils::escape_applescript_string(default_value),
        ok_button = crate::utils::escape_applescript_string(ok_button),
    );

    let value = crate::platform::run_osascript(&script, "builtin_picker_prompt_for_text")
        .map_err(|error| error.to_string())?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

#[cfg(test)]
fn emoji_picker_label(emoji: &script_kit_gpui::emoji::Emoji) -> String {
    format!("{}  {}", emoji.emoji, emoji.name)
}

impl ScriptListApp {
    fn spawn_send_screen_to_ai_after_hide(&mut self, trace_id: &str, cx: &mut Context<Self>) {
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
                        this.open_ai_window_after_already_hidden(
                            "SendScreenToAi",
                            &trace_id,
                            DeferredAiWindowAction::SetInputWithImage {
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
                    let message = format!("Failed to capture screen: {}", error);
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

                    tracing::info!(
                        category = "AI",
                        event = "ai_capture_completed",
                        source_action = "SendFocusedWindowToAi",
                        trace_id = %trace_id,
                        window_title = %capture.window_title,
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

                        this.open_ai_window_after_already_hidden(
                            "SendFocusedWindowToAi",
                            &trace_id,
                            DeferredAiWindowAction::SetInputWithImage {
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
                    let message = format!("Failed to capture window: {}", error);
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
                        this.open_ai_window_after_already_hidden(
                            "SendScreenAreaToAi",
                            &trace_id,
                            DeferredAiWindowAction::SetInputWithImage {
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
                    let message = format!("Failed to capture screen area: {}", error);
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
                    this.open_ai_window_after_already_hidden(
                        source_action,
                        &trace_id,
                        DeferredAiWindowAction::SetInput {
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
                    let message = format!("Failed to capture content for ACP Chat: {}", error);
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
                        let message = format!("Failed to capture current app context: {}", error);
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

    fn system_action_feedback_message(
        &self,
        action_type: &builtins::SystemActionType,
    ) -> Option<String> {
        let dark_mode_enabled = if matches!(action_type, builtins::SystemActionType::ToggleDarkMode)
        {
            system_actions::is_dark_mode().ok()
        } else {
            None
        };

        builtins::system_action_hud_message(*action_type, dark_mode_enabled)
    }

    /// Shared dispatch for system actions — used by both the normal and confirmed paths.
    /// Maps a `SystemActionType` to its implementation, handles special cases
    /// (TestConfirmation, QuitScriptKit), and routes the result through
    /// `handle_system_action_result`.
    /// Structured outcome logger for builtin execution paths.
    ///
    /// Emits a single log line with all fields needed for machine consumption
    /// and human debugging: builtin_id, trace_id, surface, handler, status,
    /// error_code, and duration_ms.
    fn log_builtin_outcome(
        builtin_id: &str,
        dctx: &crate::action_helpers::DispatchContext,
        handler: &str,
        outcome: &crate::action_helpers::DispatchOutcome,
        start: &std::time::Instant,
    ) {
        let duration_ms = start.elapsed().as_millis() as u64;
        let trace_id = outcome
            .trace_id
            .as_deref()
            .unwrap_or(dctx.trace_id.as_str());

        match outcome.status {
            crate::action_helpers::ActionOutcomeStatus::Error => {
                tracing::error!(
                    category = "BUILTIN",
                    builtin_id = %builtin_id,
                    trace_id = %trace_id,
                    surface = %dctx.surface,
                    handler,
                    status = %outcome.status,
                    error_code = outcome.error_code,
                    duration_ms,
                    detail = ?outcome.detail,
                    "Builtin execution finished"
                );
            }
            _ => {
                tracing::info!(
                    category = "BUILTIN",
                    builtin_id = %builtin_id,
                    trace_id = %trace_id,
                    surface = %dctx.surface,
                    handler,
                    status = %outcome.status,
                    error_code = outcome.error_code,
                    duration_ms,
                    detail = ?outcome.detail,
                    "Builtin execution finished"
                );
            }
        }
    }

    /// Build a success outcome carrying the dispatch context's trace_id.
    fn builtin_success(
        dctx: &crate::action_helpers::DispatchContext,
        detail: impl Into<String>,
    ) -> crate::action_helpers::DispatchOutcome {
        crate::action_helpers::DispatchOutcome::success()
            .with_trace_id(dctx.trace_id.clone())
            .with_detail(detail)
    }

    /// Build an error outcome carrying the dispatch context's trace_id.
    fn builtin_error(
        dctx: &crate::action_helpers::DispatchContext,
        code: &'static str,
        message: impl Into<String>,
        detail: impl Into<String>,
    ) -> crate::action_helpers::DispatchOutcome {
        crate::action_helpers::DispatchOutcome::error(code, message)
            .with_trace_id(dctx.trace_id.clone())
            .with_detail(detail)
    }

    fn dispatch_system_action(
        &mut self,
        action_type: &builtins::SystemActionType,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let start = std::time::Instant::now();

        tracing::info!(
            category = "BUILTIN",
            builtin_id = %dctx.action_id,
            trace_id = %dctx.trace_id,
            surface = %dctx.surface,
            action_type = ?action_type,
            status = "dispatched",
            "system_action_dispatch"
        );

        #[cfg(target_os = "macos")]
        {
            use builtins::SystemActionType;

            let result = match action_type {
                // Power management
                SystemActionType::EmptyTrash => system_actions::empty_trash(),
                SystemActionType::LockScreen => system_actions::lock_screen(),
                SystemActionType::Sleep => system_actions::sleep(),
                SystemActionType::Restart => system_actions::restart(),
                SystemActionType::ShutDown => system_actions::shut_down(),
                SystemActionType::LogOut => system_actions::log_out(),

                // UI controls
                SystemActionType::ToggleDarkMode => system_actions::toggle_dark_mode(),
                SystemActionType::ShowDesktop => system_actions::show_desktop(),
                SystemActionType::MissionControl => system_actions::mission_control(),
                SystemActionType::Launchpad => system_actions::launchpad(),
                SystemActionType::ForceQuitApps => system_actions::force_quit_apps(),

                // Volume controls (preset levels)
                SystemActionType::Volume0 => system_actions::set_volume(0),
                SystemActionType::Volume25 => system_actions::set_volume(25),
                SystemActionType::Volume50 => system_actions::set_volume(50),
                SystemActionType::Volume75 => system_actions::set_volume(75),
                SystemActionType::Volume100 => system_actions::set_volume(100),
                SystemActionType::VolumeMute => system_actions::volume_mute(),

                // Dev/test actions
                #[cfg(debug_assertions)]
                SystemActionType::TestConfirmation => {
                    self.toast_manager.push(
                        components::toast::Toast::success("Confirmation test passed!", &self.theme)
                            .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                    cx.notify();
                    return Self::builtin_success(dctx, "system_action_test_confirmation");
                }

                // App control
                SystemActionType::QuitScriptKit => {
                    Self::prepare_script_kit_shutdown();
                    cx.quit();
                    return Self::builtin_success(dctx, "quit_script_kit");
                }

                // System utilities
                SystemActionType::ToggleDoNotDisturb => system_actions::toggle_do_not_disturb(),
                SystemActionType::StartScreenSaver => system_actions::start_screen_saver(),

                // System Preferences
                SystemActionType::OpenSystemPreferences => {
                    system_actions::open_system_preferences_main()
                }
                SystemActionType::OpenPrivacySettings => system_actions::open_privacy_settings(),
                SystemActionType::OpenDisplaySettings => system_actions::open_display_settings(),
                SystemActionType::OpenSoundSettings => system_actions::open_sound_settings(),
                SystemActionType::OpenNetworkSettings => system_actions::open_network_settings(),
                SystemActionType::OpenKeyboardSettings => system_actions::open_keyboard_settings(),
                SystemActionType::OpenBluetoothSettings => {
                    system_actions::open_bluetooth_settings()
                }
                SystemActionType::OpenNotificationsSettings => {
                    system_actions::open_notifications_settings()
                }
            };

            self.handle_system_action_result(result, action_type, dctx, start.elapsed(), cx)
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = action_type;
            self.show_unsupported_platform_toast("System actions", cx);
            Self::builtin_error(
                dctx,
                crate::action_helpers::ERROR_UNSUPPORTED_PLATFORM,
                "System actions are not supported on this platform",
                "system_action_unsupported_platform",
            )
        }
    }

    /// Shared result handler for system actions — shows HUD on success, Toast on error.
    /// Returns a `DispatchOutcome` for structured logging at the call boundary.
    fn handle_system_action_result(
        &mut self,
        result: Result<(), String>,
        action_type: &builtins::SystemActionType,
        dctx: &crate::action_helpers::DispatchContext,
        elapsed: std::time::Duration,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        let duration_ms = elapsed.as_millis() as u64;
        match result {
            Ok(()) => {
                tracing::info!(
                    category = "BUILTIN",
                    builtin_id = %dctx.action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    action_type = ?action_type,
                    status = "success",
                    duration_ms,
                    "system_action_dispatch"
                );
                if let Some(message) = self.system_action_feedback_message(action_type) {
                    cx.notify();
                    self.show_hud(message, Some(HUD_MEDIUM_MS), cx);
                    self.hide_main_and_reset(cx);
                } else {
                    self.close_and_reset_window(cx);
                }
                Self::builtin_success(dctx, format!("system_action::{action_type:?}"))
            }
            Err(error) => {
                tracing::error!(
                    category = "BUILTIN",
                    builtin_id = %dctx.action_id,
                    trace_id = %dctx.trace_id,
                    surface = %dctx.surface,
                    action_type = ?action_type,
                    status = "error",
                    error_code = crate::action_helpers::ERROR_LAUNCH_FAILED,
                    duration_ms,
                    error = %error,
                    "system_action_dispatch"
                );
                self.show_error_toast(format!("System action failed: {}", error), cx);
                Self::builtin_error(
                    dctx,
                    crate::action_helpers::ERROR_LAUNCH_FAILED,
                    format!("System action failed: {}", error),
                    format!("system_action::{action_type:?}; error={error}"),
                )
            }
        }
    }

    fn prepare_script_kit_shutdown() {
        tracing::info!(
            category = "UI",
            event = "prepare_script_kit_shutdown",
            "prepare_script_kit_shutdown"
        );
        PROCESS_MANAGER.kill_all_processes();
        PROCESS_MANAGER.remove_main_pid();
    }

    fn quit_script_kit_confirm_options() -> crate::confirm::ParentConfirmOptions {
        crate::confirm::ParentConfirmOptions::destructive(
            "Quit Script Kit",
            "Quit Script Kit and stop all running processes?",
            "Quit",
        )
    }

    fn builtin_confirmation_options(
        entry_id: &str,
        entry_name: &str,
    ) -> crate::confirm::ParentConfirmOptions {
        match entry_id {
            "builtin/quit-script-kit" => Self::quit_script_kit_confirm_options(),
            "builtin/shut-down" => crate::confirm::ParentConfirmOptions::destructive(
                "Shut Down Mac",
                "Shut down this Mac now?",
                "Shut Down",
            ),
            "builtin/restart" => crate::confirm::ParentConfirmOptions::destructive(
                "Restart Mac",
                "Restart this Mac now?",
                "Restart",
            ),
            "builtin/log-out" => crate::confirm::ParentConfirmOptions::destructive(
                "Log Out",
                "Log out of the current macOS session?",
                "Log Out",
            ),
            "builtin/empty-trash" => crate::confirm::ParentConfirmOptions::destructive(
                "Empty Trash",
                "Empty Trash now? This cannot be undone.",
                "Empty Trash",
            ),
            "builtin/sleep" => crate::confirm::ParentConfirmOptions {
                title: "Sleep Mac".into(),
                body: "Put this Mac to sleep now?".into(),
                confirm_text: "Sleep".into(),
                cancel_text: "Cancel".into(),
                ..Default::default()
            },
            "builtin/force-quit" => crate::confirm::ParentConfirmOptions::destructive(
                "Force Quit Apps",
                "Open Force Quit Apps?",
                "Force Quit",
            ),
            "builtin/stop-all-processes" => crate::confirm::ParentConfirmOptions::destructive(
                "Stop All Processes",
                "Stop all running Script Kit processes?",
                "Stop All",
            ),
            "builtin/clear-suggested" => crate::confirm::ParentConfirmOptions::destructive(
                "Clear Suggested",
                "Clear suggested items and reset their ranking data?",
                "Clear Suggested",
            ),
            "builtin/test-confirmation" => crate::confirm::ParentConfirmOptions {
                title: "Test Confirmation".into(),
                body: "Open the confirmation test action?".into(),
                confirm_text: "Run Test".into(),
                cancel_text: "Cancel".into(),
                ..Default::default()
            },
            _ => crate::confirm::ParentConfirmOptions {
                title: "Confirm".into(),
                body: format!("Are you sure you want to {}?", entry_name).into(),
                confirm_text: "Continue".into(),
                cancel_text: "Cancel".into(),
                ..Default::default()
            },
        }
    }

    fn execute_builtin(&mut self, entry: &builtins::BuiltInEntry, cx: &mut Context<Self>) {
        self.execute_builtin_with_query(entry, None, cx);
    }

    fn execute_builtin_with_query(
        &mut self,
        entry: &builtins::BuiltInEntry,
        query_override: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let start = std::time::Instant::now();
        let dctx = crate::action_helpers::DispatchContext::for_builtin(&entry.id);

        tracing::info!(
            category = "BUILTIN",
            builtin_id = %entry.id,
            builtin_name = %entry.name,
            trace_id = %dctx.trace_id,
            surface = %dctx.surface,
            "Builtin execution started"
        );

        // Clear any stale actions popup from previous view
        self.show_actions_popup = false;
        self.actions_dialog = None;

        // Check if this command requires confirmation - open modal if so
        if self.config.requires_confirmation(&entry.id) {
            let confirmation_start = std::time::Instant::now();
            let entry_id = entry.id.clone();
            let query_owned = query_override.map(|s| s.to_string());
            let dctx_owned = dctx.clone();
            let confirm_options = Self::builtin_confirmation_options(&entry.id, &entry.name);

            // Spawn a task to show confirmation dialog via shared parent dialog helper
            cx.spawn(async move |this, cx| {
                match crate::confirm::confirm_with_parent_dialog(
                    cx,
                    confirm_options,
                    &dctx_owned.trace_id,
                )
                .await
                {
                    Ok(true) => {
                        let _ = this.update(cx, |this, cx| {
                            this.handle_builtin_confirmation(
                                entry_id,
                                true,
                                query_owned,
                                &dctx_owned,
                                cx,
                            );
                        });
                    }
                    Ok(false) => {
                        let outcome = crate::action_helpers::DispatchOutcome::cancelled()
                            .with_trace_id(dctx_owned.trace_id.clone())
                            .with_detail("builtin_confirmation_cancelled");
                        let _ = this.update(cx, |_, _| {
                            Self::log_builtin_outcome(
                                &entry_id,
                                &dctx_owned,
                                "confirmation_gate",
                                &outcome,
                                &confirmation_start,
                            );
                        });
                    }
                    Err(e) => {
                        let _ = this.update(cx, |this, cx| {
                            tracing::error!(
                                builtin_id = %entry_id,
                                trace_id = %dctx_owned.trace_id,
                                error = %e,
                                "failed to open confirmation modal"
                            );
                            this.show_error_toast_with_code(
                                "Failed to open confirmation dialog",
                                Some(crate::action_helpers::ERROR_MODAL_FAILED),
                                cx,
                            );
                            let outcome = Self::builtin_error(
                                &dctx_owned,
                                crate::action_helpers::ERROR_MODAL_FAILED,
                                "Failed to open confirmation dialog",
                                format!("confirmation_modal_error={e}"),
                            );
                            Self::log_builtin_outcome(
                                &entry_id,
                                &dctx_owned,
                                "confirmation_gate",
                                &outcome,
                                &confirmation_start,
                            );
                        });
                    }
                }
            })
            .detach();

            tracing::info!(
                category = "BUILTIN",
                trace_id = %dctx.trace_id,
                builtin_id = %entry.id,
                status = "awaiting_confirmation",
                duration_ms = start.elapsed().as_millis() as u64,
                "Builtin execution deferred to confirmation modal"
            );
            return; // Wait for modal callback
        }

        // All builtins now return DispatchOutcome — system actions are handled
        // inside execute_builtin_inner as well.
        let outcome = self.execute_builtin_inner(entry, query_override, &dctx, cx);

        Self::log_builtin_outcome(&entry.id, &dctx, "builtin_execution", &outcome, &start);
    }

    /// Open a filterable main-window builtin view with a consistent UX contract.
    ///
    /// Every filterable builtin should go through this helper so that focus,
    /// placeholder, filter reset, hover clearing, resize, and opened-from-menu
    /// state are always set the same way.
    fn open_builtin_filterable_view(
        &mut self,
        view: AppView,
        placeholder: &str,
        cx: &mut Context<Self>,
    ) {
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(placeholder.to_string());
        self.current_view = view;
        self.hovered_index = None;
        self.opened_from_main_menu = true;
        resize_to_view_sync(ViewType::ScriptList, 0);
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    fn open_theme_chooser_view(&mut self, cx: &mut Context<Self>) {
        self.theme_before_chooser = Some(self.theme.clone());
        let start_index = theme::presets::find_current_preset_index(&self.theme);

        self.open_builtin_filterable_view(
            AppView::ThemeChooserView {
                filter: String::new(),
                selected_index: start_index,
            },
            "Search themes or paste #hex...",
            cx,
        );

        let item_count = theme::presets::presets_cached().len();
        let old_count = self.theme_chooser_list_state.item_count();
        if old_count != item_count {
            self.theme_chooser_list_state
                .splice(0..old_count, item_count);
        }
        self.theme_chooser_list_state
            .scroll_to_reveal_item(start_index);
    }

    fn open_mini_main_window(&mut self, cx: &mut Context<Self>) {
        self.filter_text.clear();
        self.computed_filter_text.clear();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search scripts, apps, and commands…".to_string());
        self.current_view = AppView::ScriptList;
        self.main_window_mode = MainWindowMode::Mini;
        self.hovered_index = None;
        self.selected_index = 0;
        self.opened_from_main_menu = true;
        self.invalidate_grouped_cache();
        self.sync_list_state();
        let (grouped_items, _) = self.get_grouped_results_cached();
        let item_count = grouped_items.len();
        // Skip section headers — select first actual item so cmd+k works immediately
        let first_selectable =
            crate::list_item::GroupedListState::from_items(&grouped_items).first_selectable;
        self.selected_index = first_selectable;
        tracing::info!(
            event = "open_mini_main_window",
            item_count = item_count,
            selected_index = self.selected_index,
            first_selectable = first_selectable,
            grouped_cache_key = %self.grouped_cache_key,
            computed_filter = %self.computed_filter_text,
            filter_text = %self.filter_text,
            pending_filter_sync = self.pending_filter_sync,
            "open_mini_main_window: items={}, selected={}",
            item_count,
            self.selected_index,
        );
        resize_to_view_sync(ViewType::MiniMainWindow, item_count);
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    /// Open a filterable builtin view with an initial filter value.
    ///
    /// Same UX contract as [`open_builtin_filterable_view`] but pre-fills the
    /// filter input instead of clearing it. Used by `DoInCurrentApp` to open
    /// the command palette with the user's query already typed.
    fn open_builtin_filterable_view_with_filter(
        &mut self,
        view: AppView,
        filter: &str,
        placeholder: &str,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            view = ?view,
            filter = %filter,
            placeholder = %placeholder,
            "open_builtin_filterable_view_with_filter — setting current_view and filter"
        );
        self.filter_text = filter.to_string();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some(placeholder.to_string());
        self.current_view = view;
        self.hovered_index = None;
        self.opened_from_main_menu = true;
        resize_to_view_sync(ViewType::ScriptList, 0);
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        cx.notify();
    }

    pub(crate) fn present_current_app_commands_entries(
        &mut self,
        entries: Vec<crate::builtins::BuiltInEntry>,
        receipt: &crate::menu_bar::FrontmostMenuSnapshotReceipt,
        filter: &str,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            app_name = %receipt.app_name,
            bundle_id = %receipt.bundle_id,
            top_level_menu_count = receipt.top_level_menu_count,
            leaf_entry_count = receipt.leaf_entry_count,
            placeholder = %receipt.placeholder,
            source = receipt.source,
            filter = %filter,
            "current_app_commands.present"
        );

        self.cached_current_app_entries = entries;
        self.open_builtin_filterable_view_with_filter(
            AppView::CurrentAppCommandsView {
                filter: filter.to_string(),
                selected_index: 0,
            },
            filter,
            &receipt.placeholder,
            cx,
        );

        if self.cached_current_app_entries.is_empty() {
            tracing::info!(
                app_name = %receipt.app_name,
                bundle_id = %receipt.bundle_id,
                "current_app_commands.present_empty_state"
            );
        }
    }

    pub(crate) fn open_current_app_commands_from_tray(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let snapshot = crate::menu_bar::load_frontmost_menu_snapshot()
            .map_err(|e| anyhow::anyhow!("Failed to load frontmost app menu bar: {e}"))?;
        let (entries, receipt) = snapshot.into_entries_with_receipt();
        self.present_current_app_commands_entries(entries, &receipt, "", cx);
        Ok(())
    }

    /// Inner builtin executor — runs the actual action logic.
    /// Called from both the normal path (after confirmation check) and the
    /// confirmed path (after modal approval), ensuring a single implementation.
    ///
    /// Returns a `DispatchOutcome` so callers can log the real result instead
    /// of a synthetic success.
    fn execute_builtin_inner(
        &mut self,
        entry: &builtins::BuiltInEntry,
        query_override: Option<&str>,
        dctx: &crate::action_helpers::DispatchContext,
        cx: &mut Context<Self>,
    ) -> crate::action_helpers::DispatchOutcome {
        match &entry.feature {
            builtins::BuiltInFeature::ClipboardHistory => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Clipboard History"
                );
                self.cached_clipboard_entries = clipboard_history::get_cached_entries(100);
                self.focused_clipboard_entry_id = self
                    .cached_clipboard_entries
                    .first()
                    .map(|entry| entry.id.clone());
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    count = self.cached_clipboard_entries.len(),
                    "Loaded clipboard entries"
                );

                self.open_builtin_filterable_view(
                    AppView::ClipboardHistoryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search clipboard history...",
                    cx,
                );

                Self::builtin_success(dctx, "open_clipboard_history")
            }
            builtins::BuiltInFeature::PasteSequentially => {
                tracing::info!(
                    action = "paste_sequential",
                    event = "trigger",
                    trace_id = %dctx.trace_id,
                    "Paste Sequentially triggered"
                );
                match clipboard_history::advance_paste_sequence(&mut self.paste_sequential_state) {
                    clipboard_history::PasteSequentialOutcome::Pasted(entry_id) => {
                        tracing::info!(
                            action = "paste_sequential",
                            event = "paste_entry",
                            entry_id = %entry_id,
                            trace_id = %dctx.trace_id,
                            "Enqueuing sequential paste via serialized worker"
                        );
                        match clipboard_history::enqueue_sequential_paste(entry_id) {
                            Ok(()) => {
                                clipboard_history::commit_paste_sequence(
                                    &mut self.paste_sequential_state,
                                );
                                self.hide_main_and_reset(cx);
                                Self::builtin_success(dctx, "paste_sequential")
                            }
                            Err(clipboard_history::EnqueuePasteError::WorkerDisconnected) => {
                                tracing::error!(
                                    action = "paste_sequential",
                                    event = "enqueue_failed",
                                    error_code = "worker_disconnected",
                                    trace_id = %dctx.trace_id,
                                    "Paste worker is not running"
                                );
                                self.toast_manager.push(
                                    components::toast::Toast::error(
                                        "Paste worker crashed — restart Script Kit",
                                        &self.theme,
                                    )
                                    .duration_ms(Some(TOAST_CRITICAL_MS)),
                                );
                                cx.notify();
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    "Paste worker crashed",
                                    "paste_sequential_worker_disconnected",
                                )
                            }
                        }
                    }
                    clipboard_history::PasteSequentialOutcome::Exhausted => {
                        tracing::info!(
                            action = "paste_sequential",
                            event = "sequence_exhausted",
                            trace_id = %dctx.trace_id,
                            "Sequential paste exhausted all entries"
                        );
                        self.show_hud("Sequence complete".to_string(), Some(HUD_SHORT_MS), cx);
                        Self::builtin_success(dctx, "paste_sequential_exhausted")
                    }
                    clipboard_history::PasteSequentialOutcome::Empty => {
                        tracing::info!(
                            action = "paste_sequential",
                            event = "history_empty",
                            trace_id = %dctx.trace_id,
                            "No clipboard history available for sequential paste"
                        );
                        self.show_hud("No clipboard history".to_string(), Some(HUD_SHORT_MS), cx);
                        Self::builtin_success(dctx, "paste_sequential_empty")
                    }
                }
            }
            builtins::BuiltInFeature::Favorites => {
                tracing::info!(
                    category = "BUILTIN",
                    action = "open_favorites_view",
                    trace_id = %dctx.trace_id,
                    "Opening Favorites browse view"
                );

                self.open_builtin_filterable_view(
                    AppView::FavoritesBrowseView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search favorites...",
                    cx,
                );

                Self::builtin_success(dctx, "open_favorites_view")
            }
            builtins::BuiltInFeature::AppLauncher => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening App Launcher"
                );
                self.apps = app_launcher::scan_applications().clone();
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    count = self.apps.len(),
                    "Loaded applications"
                );
                self.invalidate_filter_cache();
                self.invalidate_grouped_cache();
                self.sync_list_state();

                self.open_builtin_filterable_view(
                    AppView::AppLauncherView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search applications...",
                    cx,
                );

                Self::builtin_success(dctx, "open_app_launcher")
            }
            builtins::BuiltInFeature::App(app_name) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    app = %app_name,
                    "Launching app"
                );
                let apps = app_launcher::scan_applications();
                if let Some(app) = apps.iter().find(|a| a.name == *app_name) {
                    if let Err(e) = app_launcher::launch_application(app) {
                        let message = format!("Failed to launch {}: {}", app_name, e);
                        self.show_error_toast(message.clone(), cx);
                        Self::builtin_error(
                            dctx,
                            crate::action_helpers::ERROR_LAUNCH_FAILED,
                            message,
                            format!("launch_app::{app_name}"),
                        )
                    } else {
                        self.close_and_reset_window(cx);
                        Self::builtin_success(dctx, format!("launch_app::{app_name}"))
                    }
                } else {
                    let message = format!("App not found: {}", app_name);
                    self.show_error_toast(message.clone(), cx);
                    Self::builtin_error(
                        dctx,
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        message,
                        format!("launch_app_not_found::{app_name}"),
                    )
                }
            }
            builtins::BuiltInFeature::WindowSwitcher => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Window Switcher"
                );
                match window_control::list_windows() {
                    Ok(windows) => {
                        tracing::info!(
                            category = "BUILTIN",
                            trace_id = %dctx.trace_id,
                            count = windows.len(),
                            "Loaded windows"
                        );
                        self.cached_windows = windows;

                        self.open_builtin_filterable_view(
                            AppView::WindowSwitcherView {
                                filter: String::new(),
                                selected_index: 0,
                            },
                            "Search windows...",
                            cx,
                        );

                        Self::builtin_success(dctx, "open_window_switcher")
                    }
                    Err(e) => {
                        let message = format!("Failed to list windows: {}", e);
                        self.show_error_toast(message.clone(), cx);
                        cx.notify();
                        Self::builtin_error(
                            dctx,
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            message,
                            "open_window_switcher_failed",
                        )
                    }
                }
            }
            builtins::BuiltInFeature::DesignGallery => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Design Gallery"
                );

                self.open_builtin_filterable_view(
                    AppView::DesignGalleryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search designs...",
                    cx,
                );

                Self::builtin_success(dctx, "open_design_gallery")
            }
            #[cfg(feature = "storybook")]
            builtins::BuiltInFeature::DesignExplorer => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Design Explorer"
                );

                let explorer = cx.new(|cx| {
                    let mut browser = script_kit_gpui::storybook::StoryBrowser::new(cx);
                    browser.configure_for_design_explorer(Some(
                        script_kit_gpui::storybook::StorySurface::MainMenu,
                    ));
                    browser.open_compare_mode();
                    let _ = browser.select_variant_id("current-main-menu");
                    tracing::info!(
                        event = "design_explorer_opened",
                        surface = "main-menu",
                        preview_mode = "compare",
                        variant_id = "current-main-menu",
                        "Opened in-app design explorer on the compare-ready Main Menu surface"
                    );
                    browser
                });

                self.current_view = AppView::DesignExplorerView { entity: explorer };
                cx.notify();

                Self::builtin_success(dctx, "open_design_explorer")
            }
            builtins::BuiltInFeature::AiChat => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening ACP Chat"
                );
                self.open_tab_ai_acp_with_entry_intent(None, cx);

                Self::builtin_success(dctx, "open_ai_harness_dispatched")
            }
            builtins::BuiltInFeature::Notes => {
                tracing::info!(
                    target: "script_kit::keyboard",
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    event = "notes_handoff_preserving_launcher_context",
                    filter_text_len = self.filter_text.len(),
                    show_actions_popup = self.show_actions_popup,
                    "Opening Notes window (preserving launcher context)"
                );

                // Close companion UI before hiding so it does not stay stale.
                if crate::confirm::is_confirm_window_open() {
                    crate::confirm::route_key_to_confirm_popup("escape", cx);
                }
                if crate::actions::is_actions_window_open() {
                    crate::actions::close_actions_window(cx);
                    self.show_actions_popup = false;
                    self.actions_closed_at = Some(std::time::Instant::now());
                    self.actions_dialog = None;
                    self.mark_filter_resync_after_actions_if_needed();
                    self.pop_focus_overlay(cx);
                }

                self.pending_focus = None;
                script_kit_gpui::set_main_window_visible(false);
                platform::defer_hide_main_window(cx);
                if let Err(e) = notes::open_notes_window_without_launcher_restore(cx) {
                    script_kit_gpui::set_main_window_visible(true);
                    platform::show_main_window_without_activation();
                    let message = format!("Failed to open Notes: {}", e);
                    self.show_error_toast(message.clone(), cx);
                    Self::builtin_error(
                        dctx,
                        crate::action_helpers::ERROR_LAUNCH_FAILED,
                        message,
                        "open_notes_failed",
                    )
                } else {
                    Self::builtin_success(dctx, "open_notes")
                }
            }
            builtins::BuiltInFeature::EmojiPicker => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Emoji Picker"
                );
                // EmojiPicker has an extra selected_category field, so use the
                // shared helper for the common state and then set the view.
                self.open_builtin_filterable_view(
                    AppView::EmojiPickerView {
                        filter: String::new(),
                        selected_index: 0,
                        selected_category: None,
                    },
                    "Search Emoji & Symbols...",
                    cx,
                );

                Self::builtin_success(dctx, "open_emoji_picker")
            }
            builtins::BuiltInFeature::MenuBarAction(action) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    bundle_id = %action.bundle_id,
                    "Executing menu bar action"
                );
                #[cfg(target_os = "macos")]
                {
                    match script_kit_gpui::menu_executor::execute_menu_action(
                        &action.bundle_id,
                        &action.menu_path,
                    ) {
                        Ok(()) => {
                            self.close_and_reset_window(cx);
                            Self::builtin_success(dctx, "menu_bar_action")
                        }
                        Err(e) => {
                            let message = format!("Menu action failed: {}", e);
                            self.show_error_toast(message.clone(), cx);
                            Self::builtin_error(
                                dctx,
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                message,
                                "menu_bar_action_failed",
                            )
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    self.show_unsupported_platform_toast("Menu bar actions", cx);
                    Self::builtin_error(
                        dctx,
                        crate::action_helpers::ERROR_UNSUPPORTED_PLATFORM,
                        "Menu bar actions only supported on macOS",
                        "menu_bar_action_unsupported",
                    )
                }
            }

            // =========================================================================
            // System Actions
            // =========================================================================
            builtins::BuiltInFeature::SystemAction(action_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    action_type = ?action_type,
                    "Executing system action via inner path"
                );
                self.dispatch_system_action(action_type, dctx, cx)
            }

            // NOTE: Window Actions removed - now handled by window-management extension
            // SDK tileWindow() still works via protocol messages in execute_script.rs

            // =========================================================================
            // Notes Commands
            // =========================================================================
            builtins::BuiltInFeature::NotesCommand(cmd_type) => {
                tracing::info!(
                    target: "script_kit::keyboard",
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    notes_command = ?cmd_type,
                    event = "notes_command_handoff_preserving_launcher_context",
                    filter_text_len = self.filter_text.len(),
                    show_actions_popup = self.show_actions_popup,
                    "Executing notes command (preserving launcher context)"
                );

                use builtins::NotesCommandType;

                // Close companion UI before hiding so it does not stay stale.
                if crate::confirm::is_confirm_window_open() {
                    crate::confirm::route_key_to_confirm_popup("escape", cx);
                }
                if crate::actions::is_actions_window_open() {
                    crate::actions::close_actions_window(cx);
                    self.show_actions_popup = false;
                    self.actions_closed_at = Some(std::time::Instant::now());
                    self.actions_dialog = None;
                    self.mark_filter_resync_after_actions_if_needed();
                    self.pop_focus_overlay(cx);
                }

                self.pending_focus = None;
                script_kit_gpui::set_main_window_visible(false);
                platform::defer_hide_main_window(cx);

                let result = match cmd_type {
                    NotesCommandType::OpenNotes
                    | NotesCommandType::NewNote
                    | NotesCommandType::SearchNotes => {
                        notes::open_notes_window_without_launcher_restore(cx)
                    }
                    NotesCommandType::QuickCapture => notes::quick_capture(cx),
                };

                if let Err(e) = result {
                    script_kit_gpui::set_main_window_visible(true);
                    platform::show_main_window_without_activation();
                    let message = format!("Notes command failed: {}", e);
                    self.show_error_toast(message.clone(), cx);
                    Self::builtin_error(
                        dctx,
                        crate::action_helpers::ERROR_LAUNCH_FAILED,
                        message,
                        format!("notes_command_failed::{cmd_type:?}"),
                    )
                } else {
                    Self::builtin_success(dctx, format!("notes_command::{cmd_type:?}"))
                }
            }

            // =========================================================================
            // AI Commands
            // =========================================================================
            builtins::BuiltInFeature::AiCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    ai_command = ?cmd_type,
                    "Executing AI command"
                );

                use builtins::AiCommandType;

                let keeps_main_window_visible = ai_command_keeps_main_window_visible(cmd_type);
                let uses_hide_then_capture_flow = ai_command_uses_hide_then_capture_flow(cmd_type);
                if !keeps_main_window_visible {
                    script_kit_gpui::set_main_window_visible(false);
                    self.reset_to_script_list(cx);
                    if uses_hide_then_capture_flow {
                        tracing::debug!(
                            action = ?cmd_type,
                            hide_settle_ms = AI_CAPTURE_HIDE_SETTLE_MS,
                            "Deferring main window hide to async capture flow"
                        );
                    } else {
                        platform::defer_hide_main_window(cx);
                    }
                }

                match cmd_type {
                    // -------------------------------------------------------
                    // All active AI commands now route to the harness terminal.
                    // The harness captures context inline via its own snapshot.
                    // -------------------------------------------------------
                    AiCommandType::GenerateScript => {
                        let request =
                            crate::menu_bar::current_app_commands::normalize_generate_script_request(
                                Some(query_override.unwrap_or(&self.filter_text)),
                            )
                            .map(str::to_string);
                        if let Some(request) = request {
                            self.open_tab_ai_chat_with_entry_intent(Some(request), cx);
                        } else {
                            self.open_tab_ai_acp_with_entry_intent(None, cx);
                        }
                        Self::builtin_success(dctx, "ai_generate_script_routed_to_harness")
                    }

                    AiCommandType::GenerateScriptFromCurrentApp => {
                        let request = crate::menu_bar::current_app_commands::normalize_generate_script_from_current_app_request(
                            Some(query_override.unwrap_or(&self.filter_text)),
                        );
                        let intent = if let Some(request) = request {
                            format!(
                                "Generate a Script Kit script for the frontmost app \
                                 using the current menu, selection, and browser context. \
                                 User request: {request}"
                            )
                        } else {
                            "Generate a Script Kit script for the frontmost app \
                             using the current menu, selection, and browser context."
                                .to_string()
                        };
                        self.open_tab_ai_chat_with_entry_intent(Some(intent), cx);
                        Self::builtin_success(
                            dctx,
                            "ai_generate_script_from_current_app_routed_to_harness",
                        )
                    }

                    AiCommandType::SendScreenToAi => {
                        self.open_tab_ai_chat_with_capture_kind(
                            Some("Capture and analyze the full screen.".to_string()),
                            crate::ai::TabAiCaptureKind::FullScreen,
                            cx,
                        );
                        Self::builtin_success(dctx, "ai_send_screen_routed_to_harness")
                    }

                    AiCommandType::SendFocusedWindowToAi => {
                        self.open_tab_ai_chat_with_capture_kind(
                            Some("Capture and analyze the focused window.".to_string()),
                            crate::ai::TabAiCaptureKind::FocusedWindow,
                            cx,
                        );
                        Self::builtin_success(dctx, "ai_send_focused_window_routed_to_harness")
                    }

                    AiCommandType::SendScreenAreaToAi => {
                        let message = "Send Screen Area to AI is unavailable until \
                                       selected-area capture is attached to the harness.";
                        self.toast_manager.push(
                            components::toast::Toast::error(message, &self.theme)
                                .duration_ms(Some(TOAST_ERROR_MS)),
                        );
                        cx.notify();
                        Self::builtin_error(
                            dctx,
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            message.to_string(),
                            "ai_send_screen_area_unavailable",
                        )
                    }

                    AiCommandType::SendSelectedTextToAi => {
                        self.open_tab_ai_chat_with_capture_kind(
                            Some(
                                "Use the current selected text as the primary subject.".to_string(),
                            ),
                            crate::ai::TabAiCaptureKind::SelectedText,
                            cx,
                        );
                        Self::builtin_success(dctx, "ai_send_selected_text_routed_to_harness")
                    }

                    AiCommandType::SendBrowserTabToAi => {
                        self.open_tab_ai_chat_with_capture_kind(
                            Some(
                                "Use the current browser tab URL and page context \
                                 as the primary subject."
                                    .to_string(),
                            ),
                            crate::ai::TabAiCaptureKind::BrowserTab,
                            cx,
                        );
                        Self::builtin_success(dctx, "ai_send_browser_tab_routed_to_harness")
                    }

                    AiCommandType::CreateAiPreset => {
                        tracing::info!(
                            action = "create_ai_preset",
                            trace_id = %dctx.trace_id,
                            "Opening create AI preset form"
                        );
                        self.current_view = AppView::CreateAiPresetView {
                            name: String::new(),
                            system_prompt: String::new(),
                            model: String::new(),
                            active_field: 0,
                        };
                        self.pending_focus = Some(FocusTarget::AppRoot);
                        cx.notify();
                        Self::builtin_success(dctx, "ai_create_preset")
                    }

                    AiCommandType::ImportAiPresets => {
                        tracing::info!(
                            action = "import_ai_presets",
                            "Opening file picker for AI preset import"
                        );
                        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                            files: true,
                            directories: false,
                            multiple: false,
                            prompt: Some("Select AI presets JSON file".into()),
                            allowed_extensions: vec!["json".into()],
                        });

                        cx.spawn(async move |this, cx| {
                            match rx.await {
                                Ok(Ok(Some(paths))) => {
                                    if let Some(path) = paths.first() {
                                        // Validate file contents before importing
                                        let import_result = cx
                                            .background_executor()
                                            .spawn({
                                                let path = path.clone();
                                                async move {
                                                    let contents = std::fs::read_to_string(&path)
                                                        .map_err(|e| {
                                                        format!("Failed to read file: {}", e)
                                                    })?;
                                                    ai::presets::validate_presets_json(&contents)
                                                        .map_err(|e| {
                                                        format!("Invalid preset file: {}", e)
                                                    })?;
                                                    ai::presets::import_presets_from_file(&path)
                                                        .map_err(|e| {
                                                            format!("Import failed: {}", e)
                                                        })
                                                }
                                            })
                                            .await;

                                        let _ = this.update(cx, |this, cx| {
                                            match import_result {
                                                Ok(total) => {
                                                    tracing::info!(
                                                        total = total,
                                                        action = "import_presets_success",
                                                        "Imported AI presets via file picker"
                                                    );
                                                    this.show_hud(
                                                        format!(
                                                            "Imported presets ({} total)",
                                                            total
                                                        ),
                                                        Some(HUD_SHORT_MS),
                                                        cx,
                                                    );
                                                    ai::reload_ai_presets(cx);
                                                }
                                                Err(e) => {
                                                    tracing::error!(
                                                        error = %e,
                                                        action = "import_presets_failed",
                                                        "Failed to import presets"
                                                    );
                                                    this.show_error_toast(
                                                        format!("Failed to import presets: {}", e),
                                                        cx,
                                                    );
                                                }
                                            }
                                            cx.notify();
                                        });
                                    }
                                }
                                Ok(Ok(None)) => {
                                    tracing::info!(
                                        action = "import_presets_cancelled",
                                        "User cancelled import file picker"
                                    );
                                }
                                Ok(Err(e)) => {
                                    tracing::warn!(error = %e, "Import file picker returned error");
                                }
                                Err(_) => {
                                    tracing::warn!(
                                        "Import file picker channel closed unexpectedly"
                                    );
                                }
                            }
                        })
                        .detach();
                        // Async — outcome tracked in spawned task
                        Self::builtin_success(dctx, "ai_import_presets_dispatched")
                    }

                    AiCommandType::ExportAiPresets => {
                        tracing::info!(
                            action = "export_ai_presets",
                            trace_id = %dctx.trace_id,
                            "Opening save dialog for AI preset export"
                        );
                        let default_dir = ai::presets::get_presets_path()
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(crate::setup::get_kit_path);

                        let rx =
                            cx.prompt_for_new_path(&default_dir, Some("ai-presets-export.json"));

                        cx.spawn(async move |this, cx| match rx.await {
                            Ok(Ok(Some(path))) => {
                                let export_result = cx
                                    .background_executor()
                                    .spawn({
                                        let path = path.clone();
                                        async move {
                                            ai::presets::export_presets_to_file(&path)
                                                .map_err(|e| format!("Export failed: {}", e))
                                        }
                                    })
                                    .await;

                                let _ = this.update(cx, |this, cx| {
                                    match export_result {
                                        Ok(count) => {
                                            tracing::info!(
                                                count = count,
                                                path = %path.display(),
                                                action = "export_presets_success",
                                                "Exported AI presets via file picker"
                                            );
                                            this.show_hud(
                                                format!("Exported {} presets", count),
                                                Some(HUD_SHORT_MS),
                                                cx,
                                            );
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                error = %e,
                                                action = "export_presets_failed",
                                                "Failed to export presets"
                                            );
                                            this.show_error_toast(
                                                format!("Failed to export presets: {}", e),
                                                cx,
                                            );
                                        }
                                    }
                                    cx.notify();
                                });
                            }
                            Ok(Ok(None)) => {
                                tracing::info!(
                                    action = "export_presets_cancelled",
                                    "User cancelled export save dialog"
                                );
                            }
                            Ok(Err(e)) => {
                                tracing::warn!(error = %e, "Export save dialog returned error");
                            }
                            Err(_) => {
                                tracing::warn!("Export save dialog channel closed unexpectedly");
                            }
                        })
                        .detach();
                        // Async — outcome tracked in spawned task
                        Self::builtin_success(dctx, "ai_export_presets_dispatched")
                    }

                    AiCommandType::SearchAiPresets => {
                        tracing::info!(
                            action = "search_ai_presets",
                            trace_id = %dctx.trace_id,
                            "Opening AI presets search"
                        );
                        self.current_view = AppView::SearchAiPresetsView {
                            filter: String::new(),
                            selected_index: 0,
                        };
                        self.pending_focus = Some(FocusTarget::MainFilter);
                        cx.notify();
                        Self::builtin_success(dctx, "ai_search_presets")
                    }

                    // Legacy AI aliases — all route to the harness terminal.
                    // Classification is centralized in `AiCommandType::is_legacy_harness_alias()`.
                    cmd => {
                        debug_assert!(
                            cmd.is_legacy_harness_alias(),
                            "unexpected AiCommandType variant {cmd:?} reached legacy alias arm"
                        );
                        self.open_tab_ai_acp_with_entry_intent(None, cx);
                        Self::builtin_success(dctx, format!("ai_{cmd:?}_routed_to_harness"))
                    }
                }
            }

            // =========================================================================
            // Script Commands
            // =========================================================================
            builtins::BuiltInFeature::ScriptCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    script_command = ?cmd_type,
                    "Executing script command"
                );

                use builtins::ScriptCommandType;

                let target = match cmd_type {
                    ScriptCommandType::NewScript => prompts::NamingTarget::Script,
                    ScriptCommandType::NewExtension => prompts::NamingTarget::Extension,
                };
                self.show_naming_dialog(target, cx);
                Self::builtin_success(dctx, format!("script_command::{cmd_type:?}"))
            }

            // =========================================================================
            // Permission Commands
            // =========================================================================
            builtins::BuiltInFeature::PermissionCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    permission_command = ?cmd_type,
                    "Executing permission command"
                );

                use builtins::PermissionCommandType;

                match cmd_type {
                    PermissionCommandType::CheckPermissions => {
                        let status = permissions_wizard::check_all_permissions();
                        if status.all_granted() {
                            self.show_hud(
                                "All permissions granted!".to_string(),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                        } else {
                            let missing: Vec<_> = status
                                .missing_permissions()
                                .iter()
                                .map(|p| p.permission_type.name())
                                .collect();
                            self.toast_manager.push(
                                components::toast::Toast::warning(
                                    format!("Missing permissions: {}", missing.join(", ")),
                                    &self.theme,
                                )
                                .duration_ms(Some(TOAST_WARNING_MS)),
                            );
                        }
                        cx.notify();
                        Self::builtin_success(dctx, "check_permissions")
                    }
                    PermissionCommandType::RequestAccessibility => {
                        let granted = permissions_wizard::request_accessibility_permission();
                        if granted {
                            self.show_hud(
                                "Accessibility permission granted!".to_string(),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                        } else {
                            self.toast_manager.push(
                                components::toast::Toast::warning(
                                    "Accessibility permission not granted. Some features may not work.",
                                    &self.theme,
                                )
                                .duration_ms(Some(TOAST_WARNING_MS)),
                            );
                        }
                        cx.notify();
                        Self::builtin_success(dctx, "request_accessibility")
                    }
                    PermissionCommandType::OpenAccessibilitySettings => {
                        if let Err(e) = permissions_wizard::open_accessibility_settings() {
                            let message = format!("Failed to open settings: {}", e);
                            self.show_error_toast(message.clone(), cx);
                            Self::builtin_error(
                                dctx,
                                crate::action_helpers::ERROR_LAUNCH_FAILED,
                                message,
                                "open_accessibility_settings_failed",
                            )
                        } else {
                            self.close_and_reset_window(cx);
                            Self::builtin_success(dctx, "open_accessibility_settings")
                        }
                    }
                }
            }

            // =========================================================================
            // Frecency/Suggested Commands
            // =========================================================================
            builtins::BuiltInFeature::FrecencyCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    frecency_command = ?cmd_type,
                    "Executing frecency command"
                );

                use builtins::FrecencyCommandType;

                match cmd_type {
                    FrecencyCommandType::ClearSuggested => {
                        self.frecency_store.clear();
                        if let Err(e) = self.frecency_store.save() {
                            let message = format!("Failed to clear suggested: {}", e);
                            self.show_error_toast(message.clone(), cx);
                            cx.notify();
                            Self::builtin_error(
                                dctx,
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                message,
                                "clear_suggested_failed",
                            )
                        } else {
                            tracing::info!(
                                trace_id = %dctx.trace_id,
                                "Cleared all suggested items"
                            );
                            self.invalidate_grouped_cache();
                            self.reset_to_script_list(cx);
                            resize_to_view_sync(ViewType::ScriptList, 0);
                            self.show_hud(
                                "Suggested items cleared".to_string(),
                                Some(HUD_SHORT_MS),
                                cx,
                            );
                            cx.notify();
                            Self::builtin_success(dctx, "clear_suggested")
                        }
                    }
                }
            }

            // =========================================================================
            // Settings Commands (Reset Window Positions, etc.)
            // =========================================================================
            builtins::BuiltInFeature::SettingsCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    settings_command = ?cmd_type,
                    "Executing settings command"
                );

                use builtins::SettingsCommandType;

                match cmd_type {
                    SettingsCommandType::ResetWindowPositions => {
                        crate::window_state::suppress_save();
                        crate::window_state::reset_all_positions();
                        self.show_hud(
                            "Window positions reset - takes effect next open".to_string(),
                            Some(HUD_SHORT_MS),
                            cx,
                        );
                        self.close_and_reset_window(cx);
                        Self::builtin_success(dctx, "reset_window_positions")
                    }
                    SettingsCommandType::ConfigureVercelApiKey => {
                        self.show_api_key_prompt(
                            "SCRIPT_KIT_VERCEL_API_KEY",
                            "Enter your Vercel AI Gateway API key",
                            "Vercel AI Gateway",
                            cx,
                        );
                        Self::builtin_success(dctx, "configure_vercel_api_key")
                    }
                    SettingsCommandType::ConfigureOpenAiApiKey => {
                        self.show_api_key_prompt(
                            "SCRIPT_KIT_OPENAI_API_KEY",
                            "Enter your OpenAI API key",
                            "OpenAI",
                            cx,
                        );
                        Self::builtin_success(dctx, "configure_openai_api_key")
                    }
                    SettingsCommandType::ConfigureAnthropicApiKey => {
                        self.show_api_key_prompt(
                            "SCRIPT_KIT_ANTHROPIC_API_KEY",
                            "Enter your Anthropic API key",
                            "Anthropic",
                            cx,
                        );
                        Self::builtin_success(dctx, "configure_anthropic_api_key")
                    }
                    SettingsCommandType::ChooseTheme => {
                        self.open_theme_chooser_view(cx);

                        Self::builtin_success(dctx, "choose_theme")
                    }
                    SettingsCommandType::DisableWindowSnapping
                    | SettingsCommandType::SnapModeSimple
                    | SettingsCommandType::SnapModeExpanded
                    | SettingsCommandType::SnapModePrecision => {
                        let target_mode = match cmd_type {
                            SettingsCommandType::DisableWindowSnapping => {
                                window_control::SnapMode::Off
                            }
                            SettingsCommandType::SnapModeSimple => window_control::SnapMode::Simple,
                            SettingsCommandType::SnapModeExpanded => {
                                window_control::SnapMode::Expanded
                            }
                            SettingsCommandType::SnapModePrecision => {
                                window_control::SnapMode::Precision
                            }
                            _ => unreachable!("snap mode command arm only handles snap commands"),
                        };

                        let previous = window_control::current_snap_mode();
                        let runtime_active = window_control::is_snap_runtime_active();

                        let mode = match window_control::persist_snap_mode(target_mode) {
                            Ok(mode) => mode,
                            Err(error) => {
                                tracing::error!(
                                    category = "WINDOW",
                                    trace_id = %dctx.trace_id,
                                    %error,
                                    ?target_mode,
                                    "Failed to persist snap mode from built-in command"
                                );
                                self.show_hud(
                                    format!("Failed to update snap mode: {error}"),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                                return Self::builtin_error(
                                    dctx,
                                    "set_snap_mode_failed",
                                    "Failed to save snap mode",
                                    error.to_string(),
                                );
                            }
                        };

                        if runtime_active {
                            let runtime_result = if mode == window_control::SnapMode::Off {
                                window_control::cancel_snap_runtime(cx)
                            } else {
                                window_control::refresh_snap_runtime_for_mode(cx)
                            };

                            if let Err(error) = runtime_result {
                                tracing::warn!(
                                    category = "WINDOW",
                                    trace_id = %dctx.trace_id,
                                    %error,
                                    ?mode,
                                    "Failed to apply runtime transition after snap mode change"
                                );
                            }
                        }

                        tracing::info!(
                            category = "WINDOW",
                            trace_id = %dctx.trace_id,
                            previous = ?previous,
                            ?mode,
                            runtime_active,
                            "Updated snap mode from built-in command"
                        );

                        let hud_text = match mode {
                            window_control::SnapMode::Off => "Window snapping disabled".to_string(),
                            window_control::SnapMode::Simple => "Snap mode: Simple".to_string(),
                            window_control::SnapMode::Expanded => "Snap mode: Expanded".to_string(),
                            window_control::SnapMode::Precision => {
                                "Snap mode: Precision".to_string()
                            }
                        };
                        let success_id = match mode {
                            window_control::SnapMode::Off => "set_snap_mode::off",
                            window_control::SnapMode::Simple => "set_snap_mode::simple",
                            window_control::SnapMode::Expanded => "set_snap_mode::expanded",
                            window_control::SnapMode::Precision => "set_snap_mode::precision",
                        };

                        self.show_hud(hud_text, Some(HUD_SHORT_MS), cx);
                        Self::builtin_success(dctx, success_id)
                    }
                    SettingsCommandType::SelectMicrophone => {
                        let prefs = crate::config::load_user_preferences();
                        let menu_items = match crate::dictation::list_input_device_menu_items(
                            prefs.dictation.selected_device_id.as_deref(),
                        ) {
                            Ok(items) => items,
                            Err(error) => {
                                tracing::error!(
                                    category = "DICTATION",
                                    error = %error,
                                    "Failed to enumerate microphone devices"
                                );
                                self.show_hud(
                                    format!("Failed to list microphones: {error}"),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                                return Self::builtin_error(
                                    dctx,
                                    "select_microphone_failed",
                                    "Failed to list microphones",
                                    error.to_string(),
                                );
                            }
                        };

                        let mut start_index: usize = 0;
                        let choices: Vec<Choice> = menu_items
                            .iter()
                            .enumerate()
                            .map(|(idx, item)| {
                                let value = match &item.action {
                                    crate::dictation::DictationDeviceSelectionAction::UseSystemDefault => {
                                        BUILTIN_MIC_DEFAULT_VALUE.to_string()
                                    }
                                    crate::dictation::DictationDeviceSelectionAction::UseDevice(id) => {
                                        id.0.clone()
                                    }
                                };
                                let name = if item.is_selected {
                                    if start_index == 0 && idx > 0 {
                                        start_index = idx;
                                    }
                                    format!("{} (current)", item.title)
                                } else {
                                    item.title.clone()
                                };
                                Choice {
                                    name,
                                    value: value.clone(),
                                    description: Some(item.subtitle.clone()),
                                    key: None,
                                    semantic_id: Some(builtin_choice_semantic_id(
                                        BUILTIN_MIC_SELECT_PROMPT_ID,
                                        idx,
                                        &value,
                                    )),
                                }
                            })
                            .collect();

                        // Follow the canonical ShowMini pattern from prompt_handler
                        // (not open_builtin_filterable_view which targets MainFilter focus)
                        let choice_count = choices.len();
                        tracing::info!(
                            category = "AUTOMATION",
                            prompt_id = BUILTIN_MIC_SELECT_PROMPT_ID,
                            choice_count = choice_count,
                            selected_index = start_index,
                            semantic_ids_populated =
                                choices.iter().all(|c| c.semantic_id.is_some()),
                            "opened_builtin_microphone_prompt"
                        );
                        self.opened_from_main_menu = true;
                        self.arg_input.clear();
                        self.arg_selected_index = start_index;
                        self.focused_input = FocusedInput::ArgPrompt;
                        self.filter_text.clear();
                        self.pending_filter_sync = true;
                        self.pending_placeholder = Some("Select microphone...".to_string());
                        self.pending_focus = Some(FocusTarget::MainFilter);
                        self.current_view = AppView::MiniPrompt {
                            id: BUILTIN_MIC_SELECT_PROMPT_ID.to_string(),
                            placeholder: "Select microphone...".to_string(),
                            choices,
                        };
                        resize_to_view_sync(ViewType::ArgPromptWithChoices, choice_count);
                        cx.notify();

                        Self::builtin_success(dctx, "select_microphone")
                    }
                }
            }

            // =========================================================================
            // Utility Commands (Scratch Pad, Quick Terminal, Claude Code Harness, Process Manager)
            // =========================================================================
            builtins::BuiltInFeature::UtilityCommand(cmd_type) => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    utility_command = ?cmd_type,
                    "Executing utility command"
                );

                use builtins::UtilityCommandType;

                match cmd_type {
                    UtilityCommandType::MiniMainWindow => {
                        tracing::info!(
                            category = "BUILTIN",
                            trace_id = %dctx.trace_id,
                            "Opening Mini Main Window"
                        );
                        self.open_mini_main_window(cx);
                        Self::builtin_success(dctx, "open_mini_main_window")
                    }
                    UtilityCommandType::ScratchPad => {
                        self.opened_from_main_menu = true;
                        self.open_scratch_pad(cx);
                        Self::builtin_success(dctx, "open_scratch_pad")
                    }
                    UtilityCommandType::QuickTerminal => {
                        self.opened_from_main_menu = true;
                        self.open_quick_terminal(cx);
                        Self::builtin_success(dctx, "open_quick_terminal")
                    }
                    UtilityCommandType::ClaudeCode => {
                        self.opened_from_main_menu = true;
                        self.open_claude_code_terminal(cx);
                        Self::builtin_success(dctx, "open_claude_code_terminal")
                    }
                    UtilityCommandType::ProcessManager => {
                        let processes =
                            crate::process_manager::PROCESS_MANAGER.get_active_processes_sorted();
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            active_process_count = processes.len(),
                            "process_manager.open_view"
                        );

                        self.cached_processes = processes;

                        self.open_builtin_filterable_view(
                            AppView::ProcessManagerView {
                                filter: String::new(),
                                selected_index: 0,
                            },
                            "Search running scripts...",
                            cx,
                        );

                        self.start_process_manager_refresh(cx);
                        Self::builtin_success(dctx, "open_process_manager")
                    }
                    UtilityCommandType::StopAllProcesses => {
                        let process_count = crate::process_manager::PROCESS_MANAGER.active_count();
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            requested_count = process_count,
                            "process_manager.stop_all"
                        );

                        if process_count == 0 {
                            self.show_hud(
                                "No running scripts to stop.".to_string(),
                                Some(HUD_2200_MS),
                                cx,
                            );
                        } else {
                            crate::process_manager::PROCESS_MANAGER.kill_all_processes();
                            self.show_hud(
                                format!("Stopped {} running script process(es).", process_count),
                                Some(HUD_MEDIUM_MS),
                                cx,
                            );
                            self.close_and_reset_window(cx);
                        }
                        Self::builtin_success(dctx, "stop_all_processes")
                    }
                    UtilityCommandType::InspectCurrentContext => {
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            "context_snapshot.inspect_requested"
                        );

                        let started_at = std::time::Instant::now();

                        let snapshot = crate::context_snapshot::capture_context_snapshot(
                            &crate::context_snapshot::CaptureContextOptions::default(),
                        );

                        match serde_json::to_string_pretty(&snapshot) {
                            Ok(json) => {
                                let receipt = crate::context_snapshot::build_inspection_receipt(
                                    &snapshot,
                                    json.len(),
                                );

                                tracing::info!(
                                    category = "CONTEXT",
                                    event = "context_snapshot_copied",
                                    trace_id = %dctx.trace_id,
                                    schema_version = receipt.schema_version,
                                    warning_count = receipt.warning_count,
                                    has_selected_text = receipt.has_selected_text,
                                    has_frontmost_app = receipt.has_frontmost_app,
                                    top_level_menu_count = receipt.top_level_menu_count,
                                    has_browser = receipt.has_browser,
                                    has_focused_window = receipt.has_focused_window,
                                    json_bytes = receipt.json_bytes,
                                    status = %receipt.status,
                                    duration_ms = started_at.elapsed().as_millis() as u64,
                                    "Copied current context snapshot to clipboard"
                                );

                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(json));
                                let hud_message =
                                    crate::context_snapshot::build_inspection_hud_message(&receipt);
                                self.show_hud(hud_message, Some(HUD_MEDIUM_MS), cx);
                                self.close_and_reset_window(cx);

                                Self::builtin_success(dctx, "inspect_current_context")
                            }
                            Err(e) => {
                                let message =
                                    format!("Failed to serialize context snapshot: {}", e);
                                tracing::error!(
                                    trace_id = %dctx.trace_id,
                                    error = %e,
                                    "context_snapshot.serialize_failed"
                                );
                                self.show_error_toast(message.clone(), cx);
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "inspect_current_context_failed",
                                )
                            }
                        }
                    }
                    UtilityCommandType::TraceCurrentAppIntent => {
                        let raw_query_owned =
                            query_override.unwrap_or(&self.filter_text).to_string();
                        let effective_query =
                            crate::menu_bar::current_app_commands::normalize_trace_current_app_intent_request(
                                Some(&raw_query_owned),
                            )
                            .unwrap_or_default();

                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            raw_query = %raw_query_owned,
                            effective_query = %effective_query,
                            "current_app_intent_trace.requested"
                        );

                        match crate::menu_bar::load_frontmost_menu_snapshot() {
                            Ok(snapshot) => {
                                let trace_receipt =
                                    crate::menu_bar::current_app_commands::build_current_app_intent_trace_receipt(
                                        snapshot,
                                        Some(&raw_query_owned),
                                    );

                                match serde_json::to_string_pretty(&trace_receipt) {
                                    Ok(json) => {
                                        tracing::info!(
                                            category = "CURRENT_APP_TRACE",
                                            trace_id = %dctx.trace_id,
                                            app_name = %trace_receipt.app_name,
                                            bundle_id = %trace_receipt.bundle_id,
                                            raw_query = %trace_receipt.raw_query,
                                            effective_query = %trace_receipt.effective_query,
                                            normalized_query = %trace_receipt.normalized_query,
                                            action = %trace_receipt.action,
                                            filtered_entries = trace_receipt.filtered_entries,
                                            exact_matches = trace_receipt.exact_matches,
                                            candidate_count = trace_receipt.candidates.len(),
                                            has_prompt_preview = trace_receipt.prompt_preview.is_some(),
                                            json_bytes = json.len(),
                                            "current_app_intent_trace.copied"
                                        );

                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            json,
                                        ));
                                        self.show_hud(
                                            format!(
                                                "Copied app intent trace: {} ({} exact / {} filtered)",
                                                trace_receipt.action,
                                                trace_receipt.exact_matches,
                                                trace_receipt.filtered_entries,
                                            ),
                                            Some(HUD_MEDIUM_MS),
                                            cx,
                                        );
                                        self.close_and_reset_window(cx);
                                        Self::builtin_success(dctx, "trace_current_app_intent")
                                    }
                                    Err(e) => {
                                        let message = format!(
                                            "Failed to serialize current app intent trace: {}",
                                            e
                                        );
                                        tracing::error!(
                                            trace_id = %dctx.trace_id,
                                            error = %e,
                                            "current_app_intent_trace.serialize_failed"
                                        );
                                        self.show_error_toast(message.clone(), cx);
                                        Self::builtin_error(
                                            dctx,
                                            crate::action_helpers::ERROR_ACTION_FAILED,
                                            message,
                                            "trace_current_app_intent_serialize_failed",
                                        )
                                    }
                                }
                            }
                            Err(e) => {
                                let message = format!(
                                    "Failed to inspect current app intent: {}. Check Accessibility permission in System Settings \u{2192} Privacy & Security \u{2192} Accessibility, then refocus the target app and try again.",
                                    e
                                );
                                tracing::warn!(
                                    trace_id = %dctx.trace_id,
                                    error = %e,
                                    "current_app_intent_trace.capture_failed"
                                );
                                self.show_error_toast(message.clone(), cx);
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "trace_current_app_intent_capture_failed",
                                )
                            }
                        }
                    }
                    UtilityCommandType::VerifyCurrentAppRecipe => {
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            "verify_current_app_recipe.requested"
                        );

                        let stored_recipe =
                            match crate::menu_bar::current_app_commands::load_current_app_command_recipe_from_clipboard()
                            {
                                Ok(recipe) => recipe,
                                Err(error) => {
                                    let message = format!("Verify Current App Recipe failed: {}", error);
                                    self.show_error_toast(message.clone(), cx);
                                    return Self::builtin_error(
                                        dctx,
                                        crate::action_helpers::ERROR_ACTION_FAILED,
                                        message,
                                        "verify_current_app_recipe_clipboard_failed",
                                    );
                                }
                            };

                        match crate::menu_bar::current_app_commands::load_frontmost_menu_snapshot()
                        {
                            Ok(snapshot) => {
                                let selected_text = crate::selected_text::get_selected_text()
                                    .ok()
                                    .filter(|text| !text.trim().is_empty());

                                let browser_url = crate::platform::get_focused_browser_tab_url()
                                    .ok()
                                    .filter(|url| !url.trim().is_empty());

                                let verification =
                                    crate::menu_bar::current_app_commands::verify_current_app_command_recipe(
                                        &stored_recipe,
                                        snapshot,
                                        selected_text.as_deref(),
                                        browser_url.as_deref(),
                                    );

                                match serde_json::to_string_pretty(&verification) {
                                    Ok(json) => {
                                        tracing::info!(
                                            category = "CURRENT_APP_RECIPE_VERIFY",
                                            trace_id = %dctx.trace_id,
                                            expected_bundle_id = %verification.expected_bundle_id,
                                            actual_bundle_id = %verification.actual_bundle_id,
                                            expected_route = %verification.expected_route,
                                            actual_route = %verification.actual_route,
                                            prompt_matches = verification.prompt_matches,
                                            selected_text_expected = verification.selected_text_expected,
                                            selected_text_present = verification.selected_text_present,
                                            browser_url_expected = verification.browser_url_expected,
                                            browser_url_present = verification.browser_url_present,
                                            warning_count = verification.warning_count,
                                            status = %verification.status,
                                            json_bytes = json.len(),
                                            "verify_current_app_recipe.completed"
                                        );

                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            json,
                                        ));
                                        self.show_hud(
                                            crate::menu_bar::current_app_commands::build_current_app_command_verification_hud_message(
                                                &verification,
                                            ),
                                            Some(HUD_MEDIUM_MS),
                                            cx,
                                        );
                                        self.close_and_reset_window(cx);

                                        Self::builtin_success(dctx, "verify_current_app_recipe")
                                    }
                                    Err(error) => {
                                        let message = format!(
                                            "Failed to serialize current app recipe verification: {}",
                                            error
                                        );
                                        self.show_error_toast(message.clone(), cx);
                                        Self::builtin_error(
                                            dctx,
                                            crate::action_helpers::ERROR_ACTION_FAILED,
                                            message,
                                            "verify_current_app_recipe_serialize_failed",
                                        )
                                    }
                                }
                            }
                            Err(error) => {
                                let message = format!(
                                    "Failed to verify current app recipe: {}. Check Accessibility permission in System Settings → Privacy & Security → Accessibility, then refocus the target app and try again.",
                                    error
                                );
                                self.show_error_toast(message.clone(), cx);
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "verify_current_app_recipe_capture_failed",
                                )
                            }
                        }
                    }
                    UtilityCommandType::ReplayCurrentAppRecipe => {
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            "replay_current_app_recipe.requested"
                        );

                        let stored_recipe =
                            match crate::menu_bar::current_app_commands::load_current_app_command_recipe_from_clipboard()
                            {
                                Ok(recipe) => recipe,
                                Err(error) => {
                                    let message = format!("Replay Current App Recipe failed: {}", error);
                                    self.show_error_toast(message.clone(), cx);
                                    return Self::builtin_error(
                                        dctx,
                                        crate::action_helpers::ERROR_ACTION_FAILED,
                                        message,
                                        "replay_current_app_recipe_clipboard_failed",
                                    );
                                }
                            };

                        match crate::menu_bar::current_app_commands::load_frontmost_menu_snapshot()
                        {
                            Ok(snapshot) => {
                                let (entries, snapshot_receipt) =
                                    snapshot.clone().into_entries_with_receipt();

                                let selected_text = crate::selected_text::get_selected_text()
                                    .ok()
                                    .filter(|text| !text.trim().is_empty());

                                let browser_url = crate::platform::get_focused_browser_tab_url()
                                    .ok()
                                    .filter(|url| !url.trim().is_empty());

                                let replay_receipt =
                                    crate::menu_bar::current_app_commands::build_replay_current_app_recipe_receipt(
                                        &stored_recipe,
                                        &entries,
                                        snapshot,
                                        selected_text.as_deref(),
                                        browser_url.as_deref(),
                                    );

                                tracing::info!(
                                    category = "CURRENT_APP_RECIPE_REPLAY",
                                    trace_id = %dctx.trace_id,
                                    action = %replay_receipt.action,
                                    status = %replay_receipt.verification.status,
                                    warning_count = replay_receipt.verification.warning_count,
                                    expected_bundle_id = %replay_receipt.verification.expected_bundle_id,
                                    actual_bundle_id = %replay_receipt.verification.actual_bundle_id,
                                    expected_route = %replay_receipt.verification.expected_route,
                                    actual_route = %replay_receipt.verification.actual_route,
                                    selected_entry_index = replay_receipt.selected_entry_index,
                                    "replay_current_app_recipe.resolved"
                                );

                                if replay_receipt.verification.warning_count > 0 {
                                    let json = match serde_json::to_string_pretty(&replay_receipt) {
                                        Ok(json) => json,
                                        Err(error) => {
                                            let message = format!(
                                                "Failed to serialize replay current app recipe receipt: {}",
                                                error
                                            );
                                            self.show_error_toast(message.clone(), cx);
                                            return Self::builtin_error(
                                                dctx,
                                                crate::action_helpers::ERROR_ACTION_FAILED,
                                                message,
                                                "replay_current_app_recipe_serialize_failed",
                                            );
                                        }
                                    };

                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(json));

                                    let message =
                                        crate::menu_bar::current_app_commands::build_replay_current_app_recipe_hud_message(
                                            &replay_receipt,
                                        );

                                    self.show_error_toast(
                                        format!("{}. Copied replay report to clipboard.", message),
                                        cx,
                                    );

                                    return Self::builtin_error(
                                        dctx,
                                        crate::action_helpers::ERROR_ACTION_FAILED,
                                        message,
                                        "replay_current_app_recipe_drift",
                                    );
                                }

                                match replay_receipt.action.as_str() {
                                    "execute_entry" => {
                                        let Some(entry_index) = replay_receipt.selected_entry_index
                                        else {
                                            let message =
                                                "Replay Current App Recipe resolved to execute_entry without an entry index"
                                                    .to_string();
                                            self.show_error_toast(message.clone(), cx);
                                            return Self::builtin_error(
                                                dctx,
                                                crate::action_helpers::ERROR_ACTION_FAILED,
                                                message,
                                                "replay_current_app_recipe_missing_entry_index",
                                            );
                                        };

                                        let entry = entries[entry_index].clone();
                                        self.execute_builtin_inner(
                                            &entry,
                                            Some(
                                                replay_receipt
                                                    .verification
                                                    .live_recipe
                                                    .effective_query
                                                    .as_str(),
                                            ),
                                            dctx,
                                            cx,
                                        )
                                    }
                                    "open_command_palette" => {
                                        let filter = replay_receipt
                                            .verification
                                            .live_recipe
                                            .effective_query
                                            .clone();
                                        self.present_current_app_commands_entries(
                                            entries,
                                            &snapshot_receipt,
                                            &filter,
                                            cx,
                                        );

                                        Self::builtin_success(
                                            dctx,
                                            "replay_current_app_recipe_open_palette",
                                        )
                                    }
                                    "generate_script" => {
                                        self.spawn_generate_script_from_recipe_after_hide(
                                            dctx.trace_id.to_string(),
                                            replay_receipt.verification.live_recipe.clone(),
                                            cx,
                                        );
                                        Self::builtin_success(
                                            dctx,
                                            "replay_current_app_recipe_generate_script",
                                        )
                                    }
                                    other => {
                                        let message = format!(
                                            "Replay Current App Recipe resolved to unsupported action: {}",
                                            other
                                        );
                                        self.show_error_toast(message.clone(), cx);
                                        Self::builtin_error(
                                            dctx,
                                            crate::action_helpers::ERROR_ACTION_FAILED,
                                            message,
                                            "replay_current_app_recipe_unknown_action",
                                        )
                                    }
                                }
                            }
                            Err(error) => {
                                let message = format!(
                                    "Failed to replay current app recipe: {}. Check Accessibility permission in System Settings → Privacy & Security → Accessibility, then refocus the target app and try again.",
                                    error
                                );
                                self.show_error_toast(message.clone(), cx);
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "replay_current_app_recipe_capture_failed",
                                )
                            }
                        }
                    }
                    UtilityCommandType::TurnThisIntoCommand => {
                        let raw_query_owned =
                            query_override.unwrap_or(&self.filter_text).to_string();

                        let effective_query =
                            crate::menu_bar::current_app_commands::normalize_turn_this_into_a_command_request(
                                Some(&raw_query_owned),
                            )
                            .unwrap_or_default();

                        if effective_query.is_empty() {
                            let message =
                                "Type what you want to automate after \"Turn This Into a Command\""
                                    .to_string();
                            self.show_error_toast(message.clone(), cx);
                            Self::builtin_error(
                                dctx,
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                message,
                                "turn_this_into_command_missing_query",
                            )
                        } else {
                            tracing::info!(
                                trace_id = %dctx.trace_id,
                                raw_query = %raw_query_owned,
                                effective_query = %effective_query,
                                "turn_this_into_command.requested"
                            );

                            match crate::menu_bar::load_frontmost_menu_snapshot() {
                                Ok(snapshot) => {
                                    let selected_text = crate::selected_text::get_selected_text()
                                        .ok()
                                        .filter(|text| !text.trim().is_empty());

                                    let browser_url =
                                        crate::platform::get_focused_browser_tab_url()
                                            .ok()
                                            .filter(|url| !url.trim().is_empty());

                                    let recipe =
                                        crate::menu_bar::current_app_commands::build_current_app_command_recipe(
                                            snapshot,
                                            Some(&raw_query_owned),
                                            selected_text.as_deref(),
                                            browser_url.as_deref(),
                                        );

                                    match serde_json::to_string_pretty(&recipe) {
                                        Ok(json) => {
                                            tracing::info!(
                                                category = "CURRENT_APP_RECIPE",
                                                trace_id = %dctx.trace_id,
                                                app_name = %recipe.prompt_receipt.app_name,
                                                bundle_id = %recipe.prompt_receipt.bundle_id,
                                                effective_query = %recipe.effective_query,
                                                route = %recipe.trace.action,
                                                suggested_script_name = %recipe.suggested_script_name,
                                                candidate_count = recipe.trace.candidates.len(),
                                                included_selected_text = recipe.prompt_receipt.included_selected_text,
                                                included_browser_url = recipe.prompt_receipt.included_browser_url,
                                                json_bytes = json.len(),
                                                "turn_this_into_command.recipe_copied"
                                            );

                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                json,
                                            ));

                                            self.show_hud(
                                                format!(
                                                    "Automation recipe copied: {}",
                                                    recipe.suggested_script_name,
                                                ),
                                                Some(HUD_MEDIUM_MS),
                                                cx,
                                            );

                                            self.spawn_generate_script_from_recipe_after_hide(
                                                dctx.trace_id.to_string(),
                                                recipe.clone(),
                                                cx,
                                            );

                                            Self::builtin_success(dctx, "turn_this_into_command")
                                        }
                                        Err(e) => {
                                            let message = format!(
                                                "Failed to serialize current app command recipe: {}",
                                                e
                                            );
                                            tracing::error!(
                                                trace_id = %dctx.trace_id,
                                                error = %e,
                                                "turn_this_into_command.serialize_failed"
                                            );
                                            self.show_error_toast(message.clone(), cx);
                                            Self::builtin_error(
                                                dctx,
                                                crate::action_helpers::ERROR_ACTION_FAILED,
                                                message,
                                                "turn_this_into_command_serialize_failed",
                                            )
                                        }
                                    }
                                }
                                Err(e) => {
                                    let message = format!(
                                        "Failed to capture current app command recipe: {}. Check Accessibility permission in System Settings \u{2192} Privacy & Security \u{2192} Accessibility, then refocus the target app and try again.",
                                        e
                                    );
                                    tracing::warn!(
                                        trace_id = %dctx.trace_id,
                                        error = %e,
                                        "turn_this_into_command.capture_failed"
                                    );
                                    self.show_error_toast(message.clone(), cx);
                                    Self::builtin_error(
                                        dctx,
                                        crate::action_helpers::ERROR_ACTION_FAILED,
                                        message,
                                        "turn_this_into_command_capture_failed",
                                    )
                                }
                            }
                        }
                    }
                    UtilityCommandType::DoInCurrentApp => {
                        let raw_query_owned =
                            query_override.unwrap_or(&self.filter_text).to_string();
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            raw_query = %raw_query_owned,
                            filter_text = %self.filter_text,
                            query_override = ?query_override,
                            "do_in_current_app.execution_entry — raw inputs"
                        );
                        let trimmed_query =
                            crate::menu_bar::current_app_commands::normalize_do_in_current_app_request(
                                Some(&raw_query_owned),
                            )
                            .unwrap_or_default()
                            .to_string();

                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            query = %trimmed_query,
                            "do_in_current_app.requested"
                        );

                        match crate::menu_bar::load_frontmost_menu_snapshot() {
                            Ok(snapshot) => {
                                let snapshot_for_recipe = snapshot.clone();
                                let (entries, snapshot_receipt) =
                                    snapshot.into_entries_with_receipt();

                                if entries.is_empty() && trimmed_query.is_empty() {
                                    let message = format!(
                                        "No enabled menu bar commands found for {}",
                                        snapshot_receipt.app_name
                                    );
                                    self.show_error_toast(message.clone(), cx);
                                    Self::builtin_error(
                                        dctx,
                                        crate::action_helpers::ERROR_ACTION_FAILED,
                                        message,
                                        "do_in_current_app_empty_snapshot",
                                    )
                                } else {
                                    let (action, intent_receipt) =
                                        crate::menu_bar::current_app_commands::resolve_do_in_current_app_intent(
                                            &entries,
                                            Some(&raw_query_owned),
                                        );

                                    tracing::info!(
                                        trace_id = %dctx.trace_id,
                                        app_name = %snapshot_receipt.app_name,
                                        bundle_id = %snapshot_receipt.bundle_id,
                                        leaf_entry_count = snapshot_receipt.leaf_entry_count,
                                        query = %trimmed_query,
                                        filtered_entries = intent_receipt.filtered_entries,
                                        exact_matches = intent_receipt.exact_matches,
                                        resolved_action = intent_receipt.action,
                                        "do_in_current_app.resolved"
                                    );

                                    match action {
                                        crate::menu_bar::current_app_commands::DoInCurrentAppAction::OpenCommandPalette => {
                                            tracing::info!(
                                                trace_id = %dctx.trace_id,
                                                cached_entries = entries.len(),
                                                filter = %trimmed_query,
                                                placeholder = %snapshot_receipt.placeholder,
                                                "do_in_current_app.action → OpenCommandPalette — switching to CurrentAppCommandsView"
                                            );
                                            self.present_current_app_commands_entries(
                                                entries,
                                                &snapshot_receipt,
                                                &trimmed_query,
                                                cx,
                                            );
                                            Self::builtin_success(dctx, "do_in_current_app_open_palette")
                                        }
                                        crate::menu_bar::current_app_commands::DoInCurrentAppAction::ExecuteEntry(entry_index) => {
                                            tracing::info!(
                                                trace_id = %dctx.trace_id,
                                                entry_index = entry_index,
                                                entry_name = %entries[entry_index].name,
                                                "do_in_current_app.action → ExecuteEntry — running menu command directly"
                                            );
                                            let entry = entries[entry_index].clone();
                                            self.execute_builtin_inner(&entry, Some(&raw_query_owned), dctx, cx)
                                        }
                                        crate::menu_bar::current_app_commands::DoInCurrentAppAction::GenerateScript => {
                                            tracing::info!(
                                                trace_id = %dctx.trace_id,
                                                trimmed_query = %trimmed_query,
                                                "do_in_current_app.action → GenerateScript — routing through recipe flow"
                                            );

                                            let selected_text = crate::selected_text::get_selected_text()
                                                .ok()
                                                .filter(|text| !text.trim().is_empty());

                                            let browser_url = crate::platform::get_focused_browser_tab_url()
                                                .ok()
                                                .filter(|url| !url.trim().is_empty());

                                            // --- Automation memory lookup: replay/repair/generate ---
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
                                                        trace_id = %dctx.trace_id,
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
                                                        "replay_recipe" => {
                                                            match replay.action.as_str() {
                                                                "execute_entry" => {
                                                                    if let Some(entry_index) = replay.selected_entry_index {
                                                                        if entry_index < entries.len() {
                                                                            let entry = entries[entry_index].clone();
                                                                            return self.execute_builtin_inner(
                                                                                &entry,
                                                                                Some(&raw_query_owned),
                                                                                dctx,
                                                                                cx,
                                                                            );
                                                                        }
                                                                    }
                                                                }
                                                                "open_command_palette" => {
                                                                    let filter = replay.verification.live_recipe.effective_query.clone();
                                                                    self.present_current_app_commands_entries(
                                                                        entries.clone(),
                                                                        &snapshot_receipt,
                                                                        &filter,
                                                                        cx,
                                                                    );
                                                                    return Self::builtin_success(
                                                                        dctx,
                                                                        "do_in_current_app_replay_memory_open_palette",
                                                                    );
                                                                }
                                                                "generate_script" => {
                                                                    self.spawn_generate_script_from_recipe_after_hide(
                                                                        dctx.trace_id.to_string(),
                                                                        replay.verification.live_recipe.clone(),
                                                                        cx,
                                                                    );
                                                                    return Self::builtin_success(
                                                                        dctx,
                                                                        "do_in_current_app_replay_memory_generate_script",
                                                                    );
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                        "repair_recipe" => {
                                                            self.spawn_generate_script_from_recipe_after_hide(
                                                                dctx.trace_id.to_string(),
                                                                replay.verification.live_recipe.clone(),
                                                                cx,
                                                            );
                                                            return Self::builtin_success(
                                                                dctx,
                                                                "do_in_current_app_repair_memory_recipe",
                                                            );
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                            // --- End automation memory lookup ---

                                            let recipe =
                                                crate::menu_bar::current_app_commands::build_current_app_command_recipe(
                                                    snapshot_for_recipe,
                                                    Some(&raw_query_owned),
                                                    selected_text.as_deref(),
                                                    browser_url.as_deref(),
                                                );

                                            match serde_json::to_string_pretty(&recipe) {
                                                Ok(json) => {
                                                    tracing::info!(
                                                        category = "CURRENT_APP_RECIPE",
                                                        trace_id = %dctx.trace_id,
                                                        app_name = %recipe.prompt_receipt.app_name,
                                                        bundle_id = %recipe.prompt_receipt.bundle_id,
                                                        effective_query = %recipe.effective_query,
                                                        route = %recipe.trace.action,
                                                        suggested_script_name = %recipe.suggested_script_name,
                                                        included_selected_text = recipe.prompt_receipt.included_selected_text,
                                                        included_browser_url = recipe.prompt_receipt.included_browser_url,
                                                        json_bytes = json.len(),
                                                        "do_in_current_app.recipe_prepared"
                                                    );
                                                }
                                                Err(error) => {
                                                    tracing::warn!(
                                                        trace_id = %dctx.trace_id,
                                                        error = %error,
                                                        "do_in_current_app.recipe_serialize_failed"
                                                    );
                                                }
                                            }

                                            self.spawn_generate_script_from_recipe_after_hide(
                                                dctx.trace_id.to_string(),
                                                recipe,
                                                cx,
                                            );
                                            Self::builtin_success(dctx, "do_in_current_app_generate_script_from_recipe")
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let message =
                                    format!("Failed to load frontmost app menu bar: {}", e);
                                self.show_error_toast(message.clone(), cx);
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "do_in_current_app_capture_failed",
                                )
                            }
                        }
                    }
                    UtilityCommandType::CurrentAppCommands => {
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            "current_app_commands.open_requested"
                        );
                        match crate::menu_bar::load_frontmost_menu_snapshot() {
                            Ok(snapshot) => {
                                let (entries, receipt) = snapshot.into_entries_with_receipt();

                                tracing::info!(
                                    trace_id = %dctx.trace_id,
                                    app_name = %receipt.app_name,
                                    bundle_id = %receipt.bundle_id,
                                    top_level_menu_count = receipt.top_level_menu_count,
                                    leaf_entry_count = receipt.leaf_entry_count,
                                    placeholder = %receipt.placeholder,
                                    source = receipt.source,
                                    "current_app_commands.snapshot_ready"
                                );

                                self.present_current_app_commands_entries(
                                    entries, &receipt, "", cx,
                                );
                                Self::builtin_success(dctx, "open_current_app_commands")
                            }
                            Err(e) => {
                                let message =
                                    format!("Failed to load frontmost app menu bar: {}", e);
                                tracing::warn!(
                                    trace_id = %dctx.trace_id,
                                    error = %e,
                                    "current_app_commands.capture_failed"
                                );
                                self.show_error_toast(message.clone(), cx);
                                Self::builtin_error(
                                    dctx,
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    message,
                                    "current_app_commands_capture_failed",
                                )
                            }
                        }
                    }
                }
            }

            // =========================================================================
            // Kit Store Commands
            // =========================================================================
            builtins::BuiltInFeature::KitStoreCommand(cmd_type) => {
                use builtins::KitStoreCommandType;
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    kit_store_command = ?cmd_type,
                    "Executing kit store command"
                );

                self.opened_from_main_menu = true;

                match cmd_type {
                    KitStoreCommandType::BrowseKits => {
                        self.current_view = AppView::BrowseKitsView {
                            query: String::new(),
                            selected_index: 0,
                            results: Vec::new(),
                        };
                        self.pending_focus = Some(FocusTarget::AppRoot);
                        cx.notify();

                        cx.spawn(async move |this, cx| {
                            let results = cx
                                .background_executor()
                                .spawn(async { Self::kit_store_search_results("") })
                                .await;
                            let _ = this.update(cx, |this, cx| {
                                if let AppView::BrowseKitsView {
                                    results: view_results,
                                    ..
                                } = &mut this.current_view
                                {
                                    *view_results = results;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                        Self::builtin_success(dctx, "browse_kits_dispatched")
                    }
                    KitStoreCommandType::InstalledKits => {
                        let kits = Self::kit_store_list_installed();
                        tracing::info!(
                            trace_id = %dctx.trace_id,
                            installed_count = kits.len(),
                            "Loaded installed kits"
                        );
                        self.current_view = AppView::InstalledKitsView {
                            selected_index: 0,
                            kits,
                        };
                        self.pending_focus = Some(FocusTarget::AppRoot);
                        cx.notify();
                        Self::builtin_success(dctx, "installed_kits")
                    }
                    KitStoreCommandType::UpdateAllKits => {
                        cx.spawn(async move |this, cx| {
                            let (updated, failed) = cx
                                .background_executor()
                                .spawn(async {
                                    let kits =
                                        script_kit_gpui::kit_store::storage::list_installed_kits()
                                            .unwrap_or_default();
                                    let mut updated = 0usize;
                                    let mut failed = 0usize;
                                    for kit in &kits {
                                        let pull_output = std::process::Command::new("git")
                                            .arg("-C")
                                            .arg(&kit.path)
                                            .arg("pull")
                                            .arg("--ff-only")
                                            .output();
                                        match pull_output {
                                            Ok(output) if output.status.success() => {
                                                updated += 1;
                                            }
                                            _ => {
                                                failed += 1;
                                                tracing::warn!(
                                                    kit_name = %kit.name,
                                                    "Kit update-all failed for kit"
                                                );
                                            }
                                        }
                                    }
                                    (updated, failed)
                                })
                                .await;

                            let _ = this.update(cx, |this, cx| {
                                let message = if failed == 0 {
                                    format!("Updated {} kit(s) successfully", updated)
                                } else {
                                    format!("Updated {} kit(s), {} failed", updated, failed)
                                };
                                if failed > 0 {
                                    this.toast_manager.push(
                                        components::toast::Toast::error(message, &this.theme)
                                            .duration_ms(Some(TOAST_ERROR_MS)),
                                    );
                                } else {
                                    this.show_hud(message, Some(HUD_MEDIUM_MS), cx);
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                        // Async — outcome tracked in spawned task
                        Self::builtin_success(dctx, "update_all_kits_dispatched")
                    }
                }
            }

            // =========================================================================
            // File Search (Directory Navigation)
            // =========================================================================
            builtins::BuiltInFeature::Webcam => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Webcam"
                );
                self.opened_from_main_menu = true;
                self.open_webcam(cx);
                Self::builtin_success(dctx, "open_webcam")
            }
            builtins::BuiltInFeature::Dictation => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Dictation"
                );

                // Preflight: on the start edge, verify we have somewhere to
                // deliver transcribed text before beginning capture.
                if !crate::dictation::is_dictation_recording() {
                    // Check that the Parakeet model is downloaded before
                    // starting capture — no silent Whisper fallback.
                    if !crate::dictation::is_parakeet_model_available() {
                        // If already downloading, reopen the progress prompt
                        // so the user can inspect the current state or hide it.
                        if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            self.open_dictation_model_prompt(cx);
                            return Self::builtin_success(
                                dctx,
                                "dictation_model_download_in_progress",
                            );
                        }
                        tracing::info!(
                            category = "DICTATION",
                            "Parakeet model not downloaded, opening consent prompt"
                        );
                        self.open_dictation_model_prompt(cx);
                        return Self::builtin_success(dctx, "dictation_model_prompt_opened");
                    }

                    if let Err(error) = self.ensure_dictation_delivery_target_available() {
                        let error_text = error.to_string();
                        tracing::error!(
                            category = "DICTATION",
                            error = %error_text,
                            "Dictation start preflight failed"
                        );
                        self.show_error_toast(format!("Dictation unavailable: {error_text}"), cx);
                        return Self::builtin_success(dctx, "dictation_preflight_failed");
                    }
                }

                // Resolve the delivery target at start time based on what
                // Script Kit surface is currently active.  On the stop edge
                // the target stored in the session is used instead.
                let dictation_target = if !crate::dictation::is_dictation_recording() {
                    self.resolve_dictation_target()
                } else {
                    // Stop edge — the target was captured at start time.
                    crate::dictation::get_dictation_target()
                        .unwrap_or(crate::dictation::DictationTarget::ExternalApp)
                };

                match crate::dictation::toggle_dictation(dictation_target) {
                    Ok(crate::dictation::DictationToggleOutcome::Started) => {
                        let _ = crate::dictation::set_dictation_target_cycle(
                            self.dictation_target_cycle_for(dictation_target),
                        );
                        // Track window state through orchestrator.
                        let orch_target =
                            crate::window_orchestrator::executor::to_orchestrator_target(
                                &dictation_target,
                            );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::StartDictation {
                                target: orch_target,
                            },
                            cx,
                        );
                        self.start_dictation_overlay_session(cx);
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(Some(capture))) => {
                        self.begin_dictation_transcription(capture, dictation_target, cx);
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(None)) => {
                        let _ = crate::dictation::close_dictation_overlay(cx);
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            category = "DICTATION",
                            error = %error,
                            "Failed to toggle dictation"
                        );
                        let _ = crate::dictation::update_dictation_overlay(
                            crate::dictation::DictationOverlayState {
                                phase: crate::dictation::DictationSessionPhase::Failed(
                                    error.to_string(),
                                ),
                                ..Default::default()
                            },
                            cx,
                        );
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(800),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                }
                Self::builtin_success(dctx, "dictation_toggle")
            }
            builtins::BuiltInFeature::DictationToAiHarness => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Dictation to AI Harness"
                );

                // On the start edge, run the same model-download and
                // delivery-target preflight as generic Dictation, but
                // use the target-aware validator so TabAiHarness is
                // accepted without needing the harness to be open yet.
                if !crate::dictation::is_dictation_recording() {
                    if !crate::dictation::is_parakeet_model_available() {
                        if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            self.open_dictation_model_prompt(cx);
                            return Self::builtin_success(
                                dctx,
                                "dictation_model_download_in_progress",
                            );
                        }
                        self.open_dictation_model_prompt(cx);
                        return Self::builtin_success(dctx, "dictation_model_prompt_opened");
                    }

                    let target = self.resolve_dictation_target_with_override(true);
                    if let Err(error) = self.ensure_dictation_delivery_target_available_for(target)
                    {
                        let error_text = error.to_string();
                        tracing::error!(
                            category = "DICTATION",
                            error = %error_text,
                            "Dictation-to-AI start preflight failed"
                        );
                        self.show_error_toast(format!("Dictation unavailable: {error_text}"), cx);
                        return Self::builtin_success(dctx, "dictation_preflight_failed");
                    }
                }

                // Force the target to TabAiHarness regardless of the
                // currently active view.
                let dictation_target = if !crate::dictation::is_dictation_recording() {
                    self.resolve_dictation_target_with_override(true)
                } else {
                    crate::dictation::get_dictation_target()
                        .unwrap_or(crate::dictation::DictationTarget::TabAiHarness)
                };

                match crate::dictation::toggle_dictation(dictation_target) {
                    Ok(crate::dictation::DictationToggleOutcome::Started) => {
                        let _ = crate::dictation::set_dictation_target_cycle(
                            self.dictation_target_cycle_for(dictation_target),
                        );
                        // Hide the main window SYNCHRONOUSLY before opening
                        // the overlay.  The orchestrator dispatch is deferred
                        // (cx.spawn), so relying on it for ConcealMain causes
                        // a race: the overlay opens while the main window is
                        // still visible, and macOS pushes the overlay behind
                        // other apps.
                        platform::conceal_main_window();

                        self.start_dictation_overlay_session(cx);

                        // Update orchestrator state for bookkeeping (commands
                        // are mostly no-ops since we already concealed + opened).
                        let orch_target =
                            crate::window_orchestrator::executor::to_orchestrator_target(
                                &dictation_target,
                            );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::StartDictation {
                                target: orch_target,
                            },
                            cx,
                        );
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(Some(capture))) => {
                        self.begin_dictation_transcription(capture, dictation_target, cx);
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(None)) => {
                        let _ = crate::dictation::close_dictation_overlay(cx);
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            category = "DICTATION",
                            error = %error,
                            "Failed to toggle dictation to AI harness"
                        );
                        let _ = crate::dictation::update_dictation_overlay(
                            crate::dictation::DictationOverlayState {
                                phase: crate::dictation::DictationSessionPhase::Failed(
                                    error.to_string(),
                                ),
                                ..Default::default()
                            },
                            cx,
                        );
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(800),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                }
                Self::builtin_success(dctx, "dictation_to_ai_toggle")
            }
            builtins::BuiltInFeature::DictationToFrontmostApp => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Dictation to Frontmost App"
                );

                if !crate::dictation::is_dictation_recording() {
                    if !crate::dictation::is_parakeet_model_available() {
                        if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            self.open_dictation_model_prompt(cx);
                            return Self::builtin_success(
                                dctx,
                                "dictation_model_download_in_progress",
                            );
                        }
                        self.open_dictation_model_prompt(cx);
                        return Self::builtin_success(dctx, "dictation_model_prompt_opened");
                    }

                    let target = crate::dictation::DictationTarget::ExternalApp;
                    if let Err(error) = self.ensure_dictation_delivery_target_available_for(target)
                    {
                        let error_text = error.to_string();
                        tracing::error!(
                            category = "DICTATION",
                            error = %error_text,
                            ?target,
                            "Dictation-to-app start preflight failed"
                        );
                        self.show_error_toast(format!("Dictation unavailable: {error_text}"), cx);
                        return Self::builtin_success(dctx, "dictation_preflight_failed");
                    }
                }

                let dictation_target = if !crate::dictation::is_dictation_recording() {
                    crate::dictation::DictationTarget::ExternalApp
                } else {
                    crate::dictation::get_dictation_target()
                        .unwrap_or(crate::dictation::DictationTarget::ExternalApp)
                };

                match crate::dictation::toggle_dictation(dictation_target) {
                    Ok(crate::dictation::DictationToggleOutcome::Started) => {
                        let _ = crate::dictation::set_dictation_target_cycle(
                            self.dictation_target_cycle_for(dictation_target),
                        );
                        tracing::info!(
                            category = "DICTATION",
                            ?dictation_target,
                            target_label = dictation_target.overlay_label(),
                            "Starting forced-route dictation"
                        );
                        platform::conceal_main_window();
                        self.start_dictation_overlay_session(cx);

                        let orch_target =
                            crate::window_orchestrator::executor::to_orchestrator_target(
                                &dictation_target,
                            );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::StartDictation {
                                target: orch_target,
                            },
                            cx,
                        );
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(Some(capture))) => {
                        self.begin_dictation_transcription(capture, dictation_target, cx);
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(None)) => {
                        let _ = crate::dictation::close_dictation_overlay(cx);
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            category = "DICTATION",
                            error = %error,
                            "Failed to toggle dictation to frontmost app"
                        );
                        let _ = crate::dictation::update_dictation_overlay(
                            crate::dictation::DictationOverlayState {
                                phase: crate::dictation::DictationSessionPhase::Failed(
                                    error.to_string(),
                                ),
                                ..Default::default()
                            },
                            cx,
                        );
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(800),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                }
                Self::builtin_success(dctx, "dictation_to_frontmost_app_toggle")
            }
            builtins::BuiltInFeature::DictationToNotes => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Dictation to Notes"
                );

                if !crate::dictation::is_dictation_recording() {
                    if !crate::dictation::is_parakeet_model_available() {
                        if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            self.open_dictation_model_prompt(cx);
                            return Self::builtin_success(
                                dctx,
                                "dictation_model_download_in_progress",
                            );
                        }
                        self.open_dictation_model_prompt(cx);
                        return Self::builtin_success(dctx, "dictation_model_prompt_opened");
                    }

                    let target = crate::dictation::DictationTarget::NotesEditor;
                    if let Err(error) = self.ensure_dictation_delivery_target_available_for(target)
                    {
                        let error_text = error.to_string();
                        tracing::error!(
                            category = "DICTATION",
                            error = %error_text,
                            ?target,
                            "Dictation-to-notes start preflight failed"
                        );
                        self.show_error_toast(format!("Dictation unavailable: {error_text}"), cx);
                        return Self::builtin_success(dctx, "dictation_preflight_failed");
                    }
                }

                let dictation_target = if !crate::dictation::is_dictation_recording() {
                    crate::dictation::DictationTarget::NotesEditor
                } else {
                    crate::dictation::get_dictation_target()
                        .unwrap_or(crate::dictation::DictationTarget::NotesEditor)
                };

                match crate::dictation::toggle_dictation(dictation_target) {
                    Ok(crate::dictation::DictationToggleOutcome::Started) => {
                        let _ = crate::dictation::set_dictation_target_cycle(
                            self.dictation_target_cycle_for(dictation_target),
                        );
                        tracing::info!(
                            category = "DICTATION",
                            ?dictation_target,
                            target_label = dictation_target.overlay_label(),
                            "Starting forced-route dictation"
                        );
                        self.start_dictation_overlay_session(cx);

                        let orch_target =
                            crate::window_orchestrator::executor::to_orchestrator_target(
                                &dictation_target,
                            );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::StartDictation {
                                target: orch_target,
                            },
                            cx,
                        );
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(Some(capture))) => {
                        self.begin_dictation_transcription(capture, dictation_target, cx);
                    }
                    Ok(crate::dictation::DictationToggleOutcome::Stopped(None)) => {
                        let _ = crate::dictation::close_dictation_overlay(cx);
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            category = "DICTATION",
                            error = %error,
                            "Failed to toggle dictation to notes"
                        );
                        let _ = crate::dictation::update_dictation_overlay(
                            crate::dictation::DictationOverlayState {
                                phase: crate::dictation::DictationSessionPhase::Failed(
                                    error.to_string(),
                                ),
                                ..Default::default()
                            },
                            cx,
                        );
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(800),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                    }
                }
                Self::builtin_success(dctx, "dictation_to_notes_toggle")
            }
            builtins::BuiltInFeature::FileSearch => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening File Search"
                );
                self.opened_from_main_menu = true;
                self.open_file_search(String::new(), cx);
                Self::builtin_success(dctx, "open_file_search")
            }
            // =========================================================================
            // Settings Hub
            // =========================================================================
            builtins::BuiltInFeature::Settings => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening Settings"
                );
                self.open_builtin_filterable_view(
                    AppView::SettingsView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search settings...",
                    cx,
                );
                Self::builtin_success(dctx, "open_settings")
            }
            // =========================================================================
            // ACP Conversation History
            // =========================================================================
            builtins::BuiltInFeature::AcpHistory => {
                tracing::info!(
                    category = "BUILTIN",
                    trace_id = %dctx.trace_id,
                    "Opening ACP History"
                );

                self.open_builtin_filterable_view(
                    AppView::AcpHistoryView {
                        filter: String::new(),
                        selected_index: 0,
                    },
                    "Search conversation history...",
                    cx,
                );

                Self::builtin_success(dctx, "open_acp_history")
            }
        }
    }

    // =========================================================================
    // Dictation helpers — overlay pump, transcript delivery, scheduled cleanup
    // =========================================================================

    /// Open the overlay, register the abort callback, start the pump.
    ///
    /// Shared by both `BuiltInFeature::Dictation` and
    /// `BuiltInFeature::DictationToAiHarness` so confirm/resume fixes
    /// only need one change.
    fn start_dictation_overlay_session(&mut self, cx: &mut Context<Self>) {
        let _ = crate::dictation::begin_overlay_session();
        crate::dictation::set_overlay_abort_callback(|cx| {
            if let Err(error) = crate::dictation::abort_dictation() {
                tracing::error!(
                    category = "DICTATION",
                    error = %error,
                    "Failed to abort dictation from overlay"
                );
            }
            let _ = crate::dictation::close_dictation_overlay(cx);
        });
        let _ = crate::dictation::open_dictation_overlay(cx);
        let _ = crate::dictation::update_dictation_overlay(
            crate::dictation::DictationOverlayState {
                phase: crate::dictation::DictationSessionPhase::Recording,
                ..Default::default()
            },
            cx,
        );
        self.spawn_dictation_overlay_pump(cx);
    }

    /// Transition a completed capture into the transcribing overlay state
    /// and kick off async transcription.
    ///
    /// Shared by both dictation entry points so the handoff cannot drift.
    fn begin_dictation_transcription(
        &mut self,
        capture: crate::dictation::CompletedDictationCapture,
        target: crate::dictation::DictationTarget,
        cx: &mut Context<Self>,
    ) {
        let _ = crate::dictation::update_dictation_overlay(
            crate::dictation::DictationOverlayState {
                phase: crate::dictation::DictationSessionPhase::Transcribing,
                elapsed: capture.audio_duration,
                ..Default::default()
            },
            cx,
        );
        let audio_duration = capture.audio_duration;
        let chunks = capture.chunks;
        cx.spawn(async move |this, cx| {
            let transcript_result = cx
                .background_executor()
                .spawn(async move { crate::dictation::transcribe_captured_audio(&chunks) })
                .await;
            let _ = this.update(cx, |this, cx| {
                Self::handle_dictation_transcript(
                    this,
                    transcript_result,
                    audio_duration,
                    target,
                    cx,
                );
            });
        })
        .detach();
    }

    /// Periodically snapshot the live capture session and push state to the
    /// dictation overlay.  Runs every 16 ms (~60 fps) for smooth waveform
    /// animation and stops automatically when the session ends.
    fn spawn_dictation_overlay_pump(&mut self, cx: &mut Context<Self>) {
        let gen = crate::dictation::overlay_generation();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                // Bail if a newer overlay session has started.
                if crate::dictation::overlay_generation() != gen {
                    tracing::debug!(
                        category = "DICTATION",
                        "Overlay pump detected generation change, stopping"
                    );
                    break;
                }
                let Some(state) = crate::dictation::snapshot_overlay_state() else {
                    break;
                };
                cx.update(|cx| {
                    let _ = crate::dictation::update_dictation_overlay(state, cx);
                });
            }
        })
        .detach();
    }

    /// Handle the result of background transcription: deliver the transcript
    /// to the target surface that was active when dictation started, update
    /// the overlay, and schedule cleanup timers.
    fn handle_dictation_transcript(
        &mut self,
        result: anyhow::Result<Option<String>>,
        audio_duration: std::time::Duration,
        target: crate::dictation::DictationTarget,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(transcript)) => {
                // Route delivery based on the target that was captured at
                // session start, not the current UI state.
                let delivered_internally = match target {
                    crate::dictation::DictationTarget::MainWindowFilter => {
                        if !self.can_accept_dictation_into_main_filter() {
                            self.reset_to_script_list(cx);
                        }
                        self.try_set_main_window_filter_from_dictation(transcript.clone(), cx)
                    }
                    crate::dictation::DictationTarget::MainWindowPrompt => {
                        self.try_set_prompt_input(transcript.clone(), cx)
                    }
                    crate::dictation::DictationTarget::NotesEditor => {
                        match notes::inject_text_into_notes(&mut **cx, &transcript) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error,
                                    "Notes delivery failed, falling back to frontmost app"
                                );
                                false
                            }
                        }
                    }
                    crate::dictation::DictationTarget::AiChatComposer => {
                        match ai::set_ai_input(&mut **cx, &transcript, false) {
                            Ok(()) => true,
                            Err(error) => {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error,
                                    "AI chat delivery failed, falling back to frontmost app"
                                );
                                false
                            }
                        }
                    }
                    crate::dictation::DictationTarget::TabAiHarness => {
                        self.submit_to_current_or_new_tab_ai_harness_from_text(
                            transcript.clone(),
                            crate::ai::TabAiQuickSubmitSource::Dictation,
                            cx,
                        );
                        // Let the orchestrator handle main window reveal and
                        // focus restore. The FinishDictation event emits
                        // RevealMain based on the state captured at start time.
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::FinishDictation,
                            cx,
                        );
                        true
                    }
                    crate::dictation::DictationTarget::ExternalApp => false,
                };

                if delivered_internally {
                    let destination = match target {
                        crate::dictation::DictationTarget::MainWindowFilter => {
                            crate::dictation::DictationDestination::MainWindowFilter
                        }
                        crate::dictation::DictationTarget::MainWindowPrompt => {
                            crate::dictation::DictationDestination::ActivePrompt
                        }
                        crate::dictation::DictationTarget::NotesEditor => {
                            crate::dictation::DictationDestination::NotesEditor
                        }
                        crate::dictation::DictationTarget::AiChatComposer => {
                            crate::dictation::DictationDestination::AiChatComposer
                        }
                        crate::dictation::DictationTarget::TabAiHarness => {
                            crate::dictation::DictationDestination::TabAiHarness
                        }
                        crate::dictation::DictationTarget::ExternalApp => {
                            crate::dictation::DictationDestination::FrontmostApp
                        }
                    };
                    tracing::info!(
                        category = "DICTATION",
                        ?target,
                        ?destination,
                        transcript_len = transcript.len(),
                        "Internal dictation delivery complete"
                    );

                    let _ = crate::dictation::close_dictation_overlay(cx);
                    if matches!(target, crate::dictation::DictationTarget::MainWindowFilter)
                        && !script_kit_gpui::is_main_window_visible()
                    {
                        script_kit_gpui::set_main_window_visible(true);
                        crate::platform::show_main_window_without_activation();
                    }
                    self.schedule_dictation_transcriber_cleanup(
                        cx,
                        std::time::Duration::from_secs(300),
                    );
                    // Notify orchestrator that dictation is complete.
                    // TabAiHarness dispatches this earlier (before overlay
                    // scheduling) to trigger immediate RevealMain; other
                    // targets dispatch here for state bookkeeping.
                    if !matches!(target, crate::dictation::DictationTarget::TabAiHarness) {
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::FinishDictation,
                            cx,
                        );
                    }
                } else {
                    // Guard: verify that a tracked external app target exists
                    // before attempting to paste to the frontmost app.
                    if let Err(error) = Self::ensure_dictation_frontmost_target_available() {
                        let error_text = error.to_string();
                        tracing::error!(
                            category = "DICTATION",
                            error = %error_text,
                            "Failed to resolve frontmost-app dictation target"
                        );
                        self.show_error_toast(format!("Dictation paste failed: {error_text}"), cx);
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(150),
                        );
                        self.schedule_dictation_transcriber_cleanup(
                            cx,
                            std::time::Duration::from_secs(300),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                        return;
                    }

                    let Some(target_bundle_id) =
                        crate::frontmost_app_tracker::get_last_real_app_bundle_id()
                    else {
                        tracing::error!(
                            category = "DICTATION",
                            "Frontmost-app dictation target disappeared before paste"
                        );
                        self.show_error_toast(
                            "Dictation paste failed: no tracked frontmost app is available"
                                .to_string(),
                            cx,
                        );
                        self.schedule_dictation_overlay_close(
                            cx,
                            std::time::Duration::from_millis(150),
                        );
                        self.schedule_dictation_transcriber_cleanup(
                            cx,
                            std::time::Duration::from_secs(300),
                        );
                        self.dispatch_window_event(
                            crate::window_orchestrator::WindowEvent::AbortDictation,
                            cx,
                        );
                        return;
                    };
                    tracing::info!(
                        category = "DICTATION",
                        target_bundle_id = %target_bundle_id,
                        transcript_len = transcript.len(),
                        "Preparing frontmost-app dictation paste"
                    );

                    cx.spawn(async move |this, cx| {
                        // Close overlay, hide Script Kit, and explicitly
                        // activate the tracked target app so macOS moves
                        // keyboard focus there before the CGEvent paste.
                        let yield_focus_result = match this.update(
                            cx,
                            |this, cx| this.yield_focus_for_dictation_paste(&target_bundle_id, cx),
                        ) {
                            Ok(result) => result,
                            Err(error) => Err(anyhow::anyhow!(
                                "failed to update app state before paste: {error}"
                            )),
                        };

                        if let Err(error) = yield_focus_result {
                            let error_text = error.to_string();
                            if this.update(cx, |this, cx| {
                                tracing::error!(
                                    category = "DICTATION",
                                    error = %error_text,
                                    "Failed to yield focus before dictation paste"
                                );
                                this.show_error_toast(
                                    format!(
                                        "Dictation paste failed before paste step: {error_text}"
                                    ),
                                    cx,
                                );
                                this.schedule_dictation_transcriber_cleanup(
                                    cx,
                                    std::time::Duration::from_secs(300),
                                );
                            }).is_err() {
                                tracing::warn!(
                                    category = "DICTATION",
                                    error = %error_text,
                                    "Yield-focus failure could not be surfaced (entity released)"
                                );
                            }
                            return;
                        }

                        // Let macOS settle focus back to the target app.
                        cx.background_executor()
                            .timer(Self::dictation_focus_settle_duration())
                            .await;

                        tracing::info!(
                            category = "DICTATION",
                            target_bundle_id = %target_bundle_id,
                            transcript_len = transcript.len(),
                            "Focus yielded to target app; pasting transcript"
                        );

                        let paste_result = cx
                            .background_executor()
                            .spawn({
                                let transcript = transcript.clone();
                                async move {
                                    crate::text_injector::TextInjector::new()
                                        .paste_text(&transcript)
                                }
                            })
                            .await;

                        if this.update(cx, |this, cx| {
                            match paste_result {
                                Ok(()) => {
                                    tracing::info!(
                                        category = "DICTATION",
                                        destination = ?crate::dictation::DictationDestination::FrontmostApp,
                                        transcript_len = transcript.len(),
                                        "Transcript delivered"
                                    );
                                }
                                Err(ref error) => {
                                    tracing::error!(
                                        category = "DICTATION",
                                        error = %error,
                                        "Failed to paste dictation transcript"
                                    );
                                    this.show_error_toast(
                                        format!("Dictation paste failed: {error}"),
                                        cx,
                                    );
                                }
                            }
                            this.schedule_dictation_transcriber_cleanup(
                                cx,
                                std::time::Duration::from_secs(300),
                            );
                        }).is_err() {
                            tracing::warn!(
                                category = "DICTATION",
                                transcript_len = transcript.len(),
                                "Paste result could not be reported (entity released)"
                            );
                        }
                    })
                    .detach();
                }
            }
            Ok(None) => {
                // No speech detected — close overlay quietly.
                self.schedule_dictation_overlay_close(cx, std::time::Duration::from_millis(150));
                self.schedule_dictation_transcriber_cleanup(
                    cx,
                    std::time::Duration::from_secs(300),
                );
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::FinishDictation,
                    cx,
                );
            }
            Err(error) => {
                let error_text = error.to_string();
                let model_path = crate::dictation::resolve_default_model_path();
                tracing::error!(
                    category = "DICTATION",
                    error = %error_text,
                    model_path = %model_path.display(),
                    "Transcription failed"
                );

                if error_text.contains("Parakeet model not downloaded") {
                    let _ = crate::dictation::close_dictation_overlay(cx);
                    self.dispatch_window_event(
                        crate::window_orchestrator::WindowEvent::AbortDictation,
                        cx,
                    );
                    self.open_dictation_model_prompt(cx);
                    self.schedule_dictation_transcriber_cleanup(
                        cx,
                        std::time::Duration::from_secs(300),
                    );
                    return;
                } else {
                    self.show_error_toast(
                        format!("Dictation transcription failed: {error_text}"),
                        cx,
                    );
                }

                let _ = crate::dictation::update_dictation_overlay(
                    crate::dictation::DictationOverlayState {
                        phase: crate::dictation::DictationSessionPhase::Failed(error_text),
                        elapsed: audio_duration,
                        ..Default::default()
                    },
                    cx,
                );
                self.schedule_dictation_overlay_close(cx, std::time::Duration::from_millis(800));
                self.schedule_dictation_transcriber_cleanup(
                    cx,
                    std::time::Duration::from_secs(300),
                );
                self.dispatch_window_event(
                    crate::window_orchestrator::WindowEvent::AbortDictation,
                    cx,
                );
            }
        }
    }

    const DICTATION_FOCUS_SETTLE_MS: u64 = 120;

    fn dictation_focus_settle_duration() -> std::time::Duration {
        std::time::Duration::from_millis(Self::DICTATION_FOCUS_SETTLE_MS)
    }

    /// Start downloading the Parakeet model in the background, showing
    /// progress via in-prompt updates and HUD fallback.
    fn start_parakeet_model_download(&mut self, cx: &mut Context<Self>) {
        if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_err()
        {
            self.show_hud(
                "Dictation model download already in progress".to_string(),
                Some(HUD_MEDIUM_MS),
                cx,
            );
            return;
        }

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *parakeet_model_download_cancel_slot().lock() = Some(cancel.clone());
        // Shallow channel — cosmetic updates use try_send so the download
        // thread is never blocked on UI repaints.
        let (progress_tx, progress_rx) = async_channel::bounded::<DictationModelProgressEvent>(4);
        let ui_emitter =
            std::sync::Arc::new(parking_lot::Mutex::new(DictationModelUiEmitter::default()));

        // Spawn a concurrent reader that updates the in-prompt progress
        // display as events arrive.  HUD only shows when the rich prompt
        // is not visible — they no longer compete.
        cx.spawn({
            let progress_rx = progress_rx.clone();
            async move |this, cx| {
                while let Ok(event) = progress_rx.recv().await {
                    let _ = this.update(cx, |this, cx| match event {
                        DictationModelProgressEvent::Downloading {
                            percentage,
                            downloaded_bytes,
                            total_bytes,
                            speed_bytes_per_sec,
                            eta_seconds,
                        } => {
                            let prompt_visible = this.is_dictation_model_prompt_visible();
                            this.update_dictation_model_prompt_if_visible(
                                crate::dictation::DictationModelStatus::Downloading {
                                    percentage,
                                    downloaded_bytes,
                                    total_bytes,
                                    speed_bytes_per_sec,
                                    eta_seconds,
                                },
                                cx,
                            );
                            if !prompt_visible {
                                let summary = crate::dictation::download::format_progress_summary(
                                    percentage,
                                    downloaded_bytes,
                                    total_bytes,
                                    speed_bytes_per_sec,
                                    eta_seconds,
                                );
                                this.show_hud(
                                    format!("Downloading model\u{2026} {summary}"),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                            }
                        }
                        DictationModelProgressEvent::Extracting => {
                            let prompt_visible = this.is_dictation_model_prompt_visible();
                            this.update_dictation_model_prompt_if_visible(
                                crate::dictation::DictationModelStatus::Extracting,
                                cx,
                            );
                            if !prompt_visible {
                                this.show_hud(
                                    "Extracting dictation model\u{2026}".to_string(),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                            }
                        }
                    });
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn({
                    let cancel = cancel.clone();
                    async move {
                        let speed_tracker =
                            std::sync::Arc::new(parking_lot::Mutex::new(SpeedTracker::new()));
                        let ui_emitter = ui_emitter.clone();
                        crate::dictation::download::download_parakeet_model(
                            {
                                let speed_tracker = speed_tracker.clone();
                                let ui_emitter = ui_emitter.clone();
                                let progress_tx = progress_tx;
                                move |phase, progress| {
                                    match phase {
                                        crate::dictation::download::DownloadPhase::Downloading => {
                                            let pct = progress.percentage();
                                            let speed = {
                                                let mut tracker = speed_tracker.lock();
                                                tracker.update(progress.downloaded);
                                                tracker.speed_bytes_per_sec()
                                            };
                                            let eta =
                                                crate::dictation::download::estimate_eta_seconds(
                                                    progress, speed,
                                                );

                                            let snapshot =
                                                DictationModelUiSnapshot::downloading(pct, eta);
                                            let now = std::time::Instant::now();
                                            let should_emit = {
                                                let emitter = ui_emitter.lock();
                                                emitter.should_emit(now, &snapshot)
                                            };

                                            if should_emit {
                                                let sent = progress_tx
                                                    .try_send(
                                                        DictationModelProgressEvent::Downloading {
                                                            percentage: pct,
                                                            downloaded_bytes: progress.downloaded,
                                                            total_bytes: progress.total,
                                                            speed_bytes_per_sec: speed,
                                                            eta_seconds: eta,
                                                        },
                                                    )
                                                    .is_ok();
                                                if sent {
                                                    tracing::info!(
                                                        category = "DICTATION",
                                                        pct,
                                                        downloaded = progress.downloaded,
                                                        total = progress.total,
                                                        speed,
                                                        "Model download progress"
                                                    );
                                                    let mut emitter = ui_emitter.lock();
                                                    emitter.record_emit(now, &snapshot);
                                                }
                                            }
                                        }
                                        crate::dictation::download::DownloadPhase::Extracting => {
                                            tracing::info!(
                                                category = "DICTATION",
                                                "Extracting dictation model"
                                            );
                                            let snapshot = DictationModelUiSnapshot::extracting();
                                            let now = std::time::Instant::now();
                                            // Extracting is critical — use blocking send
                                            // so it always reaches the UI.
                                            if progress_tx
                                                .send_blocking(
                                                    DictationModelProgressEvent::Extracting,
                                                )
                                                .is_ok()
                                            {
                                                let mut emitter = ui_emitter.lock();
                                                emitter.record_emit(now, &snapshot);
                                            }
                                        }
                                        crate::dictation::download::DownloadPhase::Failed(_)
                                        | crate::dictation::download::DownloadPhase::Cancelled
                                        | crate::dictation::download::DownloadPhase::Complete => {}
                                    }
                                }
                            },
                            cancel,
                        )
                    }
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                *parakeet_model_download_cancel_slot().lock() = None;
                PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS
                    .store(false, std::sync::atomic::Ordering::Release);
                match result {
                    Ok(_path) => {
                        tracing::info!(category = "DICTATION", "Parakeet model download complete");
                        this.update_dictation_model_prompt_if_visible(
                            crate::dictation::DictationModelStatus::Available,
                            cx,
                        );
                        this.show_hud(
                            "Dictation model ready \u{2014} press hotkey to dictate".to_string(),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                    }
                    Err(error) if error.to_string().contains("cancelled") => {
                        let cancelled = "model download cancelled".to_string();
                        tracing::info!(category = "DICTATION", "Parakeet model download cancelled");
                        this.update_dictation_model_prompt_if_visible(
                            crate::dictation::DictationModelStatus::DownloadFailed(cancelled),
                            cx,
                        );
                        this.show_hud(
                            "Dictation model download cancelled".to_string(),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                    }
                    Err(error) => {
                        let raw_error = error.to_string();
                        let error_text =
                            crate::dictation::download::classify_download_error(&error);
                        tracing::error!(
                            category = "DICTATION",
                            error = %raw_error,
                            user_error = %error_text,
                            "Parakeet model download failed"
                        );
                        this.update_dictation_model_prompt_if_visible(
                            crate::dictation::DictationModelStatus::DownloadFailed(
                                error_text.clone(),
                            ),
                            cx,
                        );
                        this.show_error_toast(
                            format!("Dictation model download failed: {error_text}"),
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }

    /// Build the title, placeholder, and choices for the dictation model
    /// prompt based on the current `DictationModelStatus`.  Pure function
    /// with no side effects — suitable for unit testing.
    fn build_dictation_model_prompt(
        status: crate::dictation::DictationModelStatus,
    ) -> (String, String, Vec<Choice>) {
        use crate::dictation::DictationModelStatus;

        let archive_size =
            crate::dictation::download::format_bytes(crate::dictation::PARAKEET_MODEL_ARCHIVE_SIZE);

        match status {
            DictationModelStatus::NotDownloaded => (
                "Download local dictation model".to_string(),
                format!(
                    "{archive_size} download \u{00b7} fully local transcription \u{00b7} resumable if interrupted"
                ),
                vec![
                    Choice {
                        name: format!("Download Parakeet model ({archive_size})"),
                        value: BUILTIN_DICTATION_MODEL_DOWNLOAD.to_string(),
                        description: Some("Required for local dictation".to_string()),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID, 0, BUILTIN_DICTATION_MODEL_DOWNLOAD,
                        )),
                    },
                    Choice {
                        name: "Not now".to_string(),
                        value: BUILTIN_DICTATION_MODEL_CANCEL.to_string(),
                        description: Some("Leave dictation unchanged".to_string()),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID, 1, BUILTIN_DICTATION_MODEL_CANCEL,
                        )),
                    },
                ],
            ),
            DictationModelStatus::Downloading {
                percentage,
                downloaded_bytes,
                total_bytes,
                speed_bytes_per_sec,
                eta_seconds,
            } => {
                let summary = crate::dictation::download::format_progress_summary(
                    percentage,
                    downloaded_bytes,
                    total_bytes,
                    speed_bytes_per_sec,
                    eta_seconds,
                );
                (
                    format!("Downloading local dictation model\u{2026} {percentage}%"),
                    summary,
                    vec![
                        Choice {
                            name: "Cancel download".to_string(),
                            value: BUILTIN_DICTATION_MODEL_CANCEL.to_string(),
                            description: Some(
                                "Stop now; retry resumes from the partial file".to_string(),
                            ),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID, 0, BUILTIN_DICTATION_MODEL_CANCEL,
                            )),
                        },
                        Choice {
                            name: "Hide".to_string(),
                            value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                            description: Some("Download continues in background".to_string()),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID, 1, BUILTIN_DICTATION_MODEL_HIDE,
                            )),
                        },
                    ],
                )
            }
            DictationModelStatus::Extracting => (
                "Installing local dictation model\u{2026}".to_string(),
                "Download finished. Installing model files locally.".to_string(),
                vec![Choice {
                    name: "Hide".to_string(),
                    value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                    description: Some("Extraction continues in background".to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_MODEL_PROMPT_ID, 0, BUILTIN_DICTATION_MODEL_HIDE,
                    )),
                }],
            ),
            DictationModelStatus::DownloadFailed(ref error)
                if error.to_ascii_lowercase().contains("cancelled") =>
            {
                (
                    "Download cancelled".to_string(),
                    "Partial download kept. Retry resumes from where you stopped.".to_string(),
                    vec![
                        Choice {
                            name: "Retry download".to_string(),
                            value: BUILTIN_DICTATION_MODEL_DOWNLOAD.to_string(),
                            description: Some(
                                "Resume the Parakeet model download".to_string(),
                            ),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID, 0, BUILTIN_DICTATION_MODEL_DOWNLOAD,
                            )),
                        },
                        Choice {
                            name: "Done".to_string(),
                            value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                            description: Some("Close this prompt".to_string()),
                            key: None,
                            semantic_id: Some(builtin_choice_semantic_id(
                                BUILTIN_DICTATION_MODEL_PROMPT_ID, 1, BUILTIN_DICTATION_MODEL_HIDE,
                            )),
                        },
                    ],
                )
            }
            DictationModelStatus::DownloadFailed(error) => (
                "Dictation model download failed".to_string(),
                error,
                vec![
                    Choice {
                        name: "Retry download".to_string(),
                        value: BUILTIN_DICTATION_MODEL_DOWNLOAD.to_string(),
                        description: Some(
                            "Try the Parakeet model download again".to_string(),
                        ),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID, 0, BUILTIN_DICTATION_MODEL_DOWNLOAD,
                        )),
                    },
                    Choice {
                        name: "Not now".to_string(),
                        value: BUILTIN_DICTATION_MODEL_CANCEL.to_string(),
                        description: Some("Leave dictation unchanged".to_string()),
                        key: None,
                        semantic_id: Some(builtin_choice_semantic_id(
                            BUILTIN_DICTATION_MODEL_PROMPT_ID, 1, BUILTIN_DICTATION_MODEL_CANCEL,
                        )),
                    },
                ],
            ),
            DictationModelStatus::Available => (
                "Dictation model ready".to_string(),
                "Everything is local now. Press Enter on Done or use the dictation hotkey to start recording."
                    .to_string(),
                vec![Choice {
                    name: "Done".to_string(),
                    value: BUILTIN_DICTATION_MODEL_HIDE.to_string(),
                    description: Some("Close this prompt".to_string()),
                    key: None,
                    semantic_id: Some(builtin_choice_semantic_id(
                        BUILTIN_DICTATION_MODEL_PROMPT_ID, 0, BUILTIN_DICTATION_MODEL_HIDE,
                    )),
                }],
            ),
        }
    }

    /// Render the dictation model prompt with the given status, replacing
    /// whatever is currently on screen.
    fn render_dictation_model_prompt(
        &mut self,
        status: crate::dictation::DictationModelStatus,
        cx: &mut Context<Self>,
    ) {
        *dictation_model_prompt_status().lock() = status.clone();
        let (title, placeholder, choices) = Self::build_dictation_model_prompt(status);
        self.open_builtin_filterable_view(
            AppView::MiniPrompt {
                id: BUILTIN_DICTATION_MODEL_PROMPT_ID.to_string(),
                placeholder,
                choices,
            },
            &title,
            cx,
        );
    }

    /// Returns `true` when the dictation model prompt is currently on-screen.
    fn is_dictation_model_prompt_visible(&self) -> bool {
        matches!(
            &self.current_view,
            AppView::MiniPrompt { id, .. } if id == BUILTIN_DICTATION_MODEL_PROMPT_ID
        )
    }

    /// If the dictation model prompt is currently visible, update it in-place
    /// with the new status.  Otherwise this is a no-op.
    fn update_dictation_model_prompt_if_visible(
        &mut self,
        status: crate::dictation::DictationModelStatus,
        cx: &mut Context<Self>,
    ) {
        // Always persist the latest status so reopening a hidden prompt
        // shows the current state instead of stale progress.
        *dictation_model_prompt_status().lock() = status.clone();

        let is_visible = matches!(
            &self.current_view,
            AppView::MiniPrompt { id, .. } if id == BUILTIN_DICTATION_MODEL_PROMPT_ID
        );
        if is_visible {
            self.render_dictation_model_prompt(status, cx);
        }
    }

    /// Open the dictation model prompt in its initial `NotDownloaded` state.
    fn open_dictation_model_prompt(&mut self, cx: &mut Context<Self>) {
        let status = if crate::dictation::is_parakeet_model_available() {
            crate::dictation::DictationModelStatus::Available
        } else {
            dictation_model_prompt_status().lock().clone()
        };
        self.render_dictation_model_prompt(status, cx);
    }

    /// Handle a selection from the dictation model download prompt.
    fn handle_dictation_model_selection(&mut self, value: &str, cx: &mut Context<Self>) {
        match value {
            BUILTIN_DICTATION_MODEL_DOWNLOAD => {
                tracing::info!(
                    category = "DICTATION",
                    "User accepted Parakeet model download"
                );
                // Transition the prompt to downloading state instead of closing it.
                self.render_dictation_model_prompt(
                    crate::dictation::DictationModelStatus::Downloading {
                        percentage: 0,
                        downloaded_bytes: 0,
                        total_bytes: crate::dictation::PARAKEET_MODEL_ARCHIVE_SIZE,
                        speed_bytes_per_sec: 0,
                        eta_seconds: None,
                    },
                    cx,
                );
                self.start_parakeet_model_download(cx);
            }
            BUILTIN_DICTATION_MODEL_CANCEL => {
                if PARAKEET_MODEL_DOWNLOAD_IN_PROGRESS.load(std::sync::atomic::Ordering::Acquire) {
                    tracing::info!(
                        category = "DICTATION",
                        "User requested Parakeet model download cancellation"
                    );
                    if let Some(cancel) = parakeet_model_download_cancel_slot().lock().clone() {
                        cancel.store(true, std::sync::atomic::Ordering::Release);
                    }
                    self.show_hud(
                        "Cancelling dictation model download\u{2026}".to_string(),
                        Some(HUD_SHORT_MS),
                        cx,
                    );
                } else {
                    tracing::info!(
                        category = "DICTATION",
                        "User declined Parakeet model download"
                    );
                    self.reset_to_script_list(cx);
                }
            }
            BUILTIN_DICTATION_MODEL_HIDE => {
                tracing::info!(category = "DICTATION", "User hid Parakeet model prompt");
                self.reset_to_script_list(cx);
            }
            _ => {
                tracing::info!(
                    category = "DICTATION",
                    "User declined Parakeet model download"
                );
                self.reset_to_script_list(cx);
            }
        }
    }

    /// Ensure a new dictation session has somewhere valid to send text.
    ///
    /// Allowed start conditions:
    /// - the launcher/main filter is active, or
    /// - a Script Kit prompt is active and can accept dictated text, or
    /// - the frontmost-app tracker already has a previously tracked external target.
    fn ensure_dictation_delivery_target_available(&self) -> anyhow::Result<()> {
        if self.can_accept_dictation_into_main_filter() || self.can_accept_dictation_into_prompt() {
            return Ok(());
        }
        Self::ensure_dictation_frontmost_target_available()
    }

    /// Verify that the frontmost-app tracker has a previously-tracked
    /// external app target before attempting a dictation paste.
    fn ensure_dictation_frontmost_target_available() -> anyhow::Result<()> {
        use anyhow::Context as _;
        crate::frontmost_app_tracker::get_last_real_app_bundle_id()
            .context("no previously tracked frontmost app is available for dictation paste")?;
        Ok(())
    }

    /// Validate that a specific delivery target is reachable before
    /// starting dictation.  Script Kit internal targets (harness, notes,
    /// AI composer, prompt) are always available.  `ExternalApp` requires
    /// the frontmost-app tracker to have a previously-tracked target.
    fn ensure_dictation_delivery_target_available_for(
        &self,
        target: crate::dictation::DictationTarget,
    ) -> anyhow::Result<()> {
        match target {
            crate::dictation::DictationTarget::ExternalApp => {
                Self::ensure_dictation_frontmost_target_available()
            }
            crate::dictation::DictationTarget::MainWindowFilter
            | crate::dictation::DictationTarget::MainWindowPrompt
            | crate::dictation::DictationTarget::NotesEditor
            | crate::dictation::DictationTarget::AiChatComposer
            | crate::dictation::DictationTarget::TabAiHarness => Ok(()),
        }
    }

    /// Build the small per-session destination cycle exposed by the overlay
    /// badge. Keep it intentionally tight: the primary target plus an
    /// external-app fallback, except external-app sessions which expose the
    /// main launcher filter as the alternate target.
    fn dictation_target_cycle_for(
        &self,
        target: crate::dictation::DictationTarget,
    ) -> Vec<crate::dictation::DictationTarget> {
        match target {
            crate::dictation::DictationTarget::ExternalApp => vec![
                crate::dictation::DictationTarget::ExternalApp,
                crate::dictation::DictationTarget::MainWindowFilter,
            ],
            crate::dictation::DictationTarget::MainWindowFilter => vec![
                crate::dictation::DictationTarget::MainWindowFilter,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::MainWindowPrompt => vec![
                crate::dictation::DictationTarget::MainWindowPrompt,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::NotesEditor => vec![
                crate::dictation::DictationTarget::NotesEditor,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::AiChatComposer => vec![
                crate::dictation::DictationTarget::AiChatComposer,
                crate::dictation::DictationTarget::ExternalApp,
            ],
            crate::dictation::DictationTarget::TabAiHarness => vec![
                crate::dictation::DictationTarget::TabAiHarness,
                crate::dictation::DictationTarget::ExternalApp,
            ],
        }
    }

    /// Resolve the dictation delivery target, optionally overriding to
    /// `TabAiHarness` so a dedicated "dictate to AI" action can force
    /// harness delivery even when the harness is not already on-screen.
    pub(crate) fn resolve_dictation_target_with_override(
        &self,
        force_tab_ai_harness: bool,
    ) -> crate::dictation::DictationTarget {
        if force_tab_ai_harness {
            crate::dictation::DictationTarget::TabAiHarness
        } else {
            self.resolve_dictation_target()
        }
    }

    /// Determine the delivery target for a new dictation session based on
    /// which Script Kit surface is currently active.
    ///
    /// Priority: notes editor > AI chat composer > launcher main filter >
    /// active prompt > external app.
    fn resolve_dictation_target(&self) -> crate::dictation::DictationTarget {
        let target = if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
            crate::dictation::DictationTarget::TabAiHarness
        } else if notes::is_notes_window_open() {
            crate::dictation::DictationTarget::NotesEditor
        } else if ai::is_ai_window_open() {
            crate::dictation::DictationTarget::AiChatComposer
        } else if self.can_accept_dictation_into_main_filter() {
            crate::dictation::DictationTarget::MainWindowFilter
        } else if self.can_accept_dictation_into_prompt() {
            crate::dictation::DictationTarget::MainWindowPrompt
        } else {
            crate::dictation::DictationTarget::ExternalApp
        };
        tracing::info!(
            category = "DICTATION",
            ?target,
            current_view = ?std::mem::discriminant(&self.current_view),
            notes_open = notes::is_notes_window_open(),
            ai_open = ai::is_ai_window_open(),
            accepts_main_filter = self.can_accept_dictation_into_main_filter(),
            accepts_prompt = self.can_accept_dictation_into_prompt(),
            "Resolved dictation target"
        );
        target
    }

    /// Close the dictation overlay and hide Script Kit so macOS naturally
    /// returns keyboard focus to the previously-active window before the
    /// CGEvent Cmd+V paste fires.
    ///
    /// Script Kit is a non-activating accessory app (NSPanel with
    /// NonactivatingPanel style), so when our panels close via `orderOut:`,
    /// macOS automatically restores focus to the window that was active
    /// before — no explicit `activate` call is needed.  Avoiding AppleScript
    /// `tell application id … to activate` is important because that can
    /// reorder windows within multi-window apps like Chrome, causing the
    /// paste to land in the wrong window.
    fn yield_focus_for_dictation_paste(
        &mut self,
        target_bundle_id: &str,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        use anyhow::Context as _;
        tracing::info!(
            category = "DICTATION",
            target_bundle_id = %target_bundle_id,
            "Yielding focus for dictation paste (non-activating panel dismiss)"
        );
        crate::dictation::close_dictation_overlay(cx)
            .context("failed to close dictation overlay before paste")?;
        if script_kit_gpui::is_main_window_visible() {
            script_kit_gpui::set_main_window_visible(false);
            platform::defer_hide_main_window(cx);
        }
        Ok(())
    }

    /// Schedule the overlay window to close after a delay.
    fn schedule_dictation_overlay_close(
        &mut self,
        cx: &mut Context<Self>,
        delay: std::time::Duration,
    ) {
        let gen = crate::dictation::overlay_generation();
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            // Only close if the overlay hasn't been replaced by a newer session.
            if crate::dictation::overlay_generation() != gen {
                tracing::debug!(
                    category = "DICTATION",
                    "Scheduled overlay close skipped — generation changed"
                );
                return;
            }
            cx.update(|cx| {
                let _ = crate::dictation::close_dictation_overlay(cx);
            });
        })
        .detach();
    }

    /// Bring the main window back after a delay.
    ///
    /// Used by the dictation-to-AI path: the main window is concealed so
    /// the overlay is visible, and once the overlay closes we reveal the
    /// main window with the newly-opened ACP chat view.  The delay must
    /// be slightly longer than `schedule_dictation_overlay_close` so the
    /// overlay is gone before the main window reappears.
    fn schedule_deferred_main_window_reveal(
        &mut self,
        cx: &mut Context<Self>,
        delay: std::time::Duration,
    ) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|_cx| {
                platform::show_main_window_without_activation();
            });
        })
        .detach();
    }

    /// Schedule the cached transcriber to be unloaded after an idle timeout.
    fn schedule_dictation_transcriber_cleanup(
        &mut self,
        cx: &mut Context<Self>,
        delay: std::time::Duration,
    ) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            cx.update(|_cx| {
                crate::dictation::maybe_unload_transcriber();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod builtin_execution_ai_feedback_tests {
    use super::{
        ai_capture_hide_settle_duration, ai_command_keeps_main_window_visible,
        ai_command_uses_hide_then_capture_flow, ai_open_failure_message,
        created_file_path_for_feedback, emoji_picker_label, favorites_loaded_message,
        AI_CAPTURE_HIDE_SETTLE_MS,
    };
    use crate::builtins::AiCommandType;
    use script_kit_gpui::emoji::{Emoji, EmojiCategory};
    use std::path::PathBuf;

    #[test]
    fn all_active_ai_commands_keep_main_window_visible_for_harness() {
        // All active AI commands now route to the harness terminal (a view
        // inside the main window), so they must all keep the window visible.
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::GenerateScript
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::GenerateScriptFromCurrentApp
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendScreenToAi
        ));
        assert!(ai_command_keeps_main_window_visible(&AiCommandType::OpenAi));
        assert!(ai_command_keeps_main_window_visible(&AiCommandType::MiniAi));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::NewConversation
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::ClearConversation
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendFocusedWindowToAi
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendSelectedTextToAi
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendBrowserTabToAi
        ));
        assert!(ai_command_keeps_main_window_visible(
            &AiCommandType::SendScreenAreaToAi
        ));
    }

    #[test]
    fn test_ai_open_failure_message_includes_error_details() {
        assert_eq!(
            ai_open_failure_message("window init failed"),
            "Failed to open AI: window init failed"
        );
    }

    #[test]
    fn test_favorites_loaded_message_uses_singular_for_one() {
        assert_eq!(favorites_loaded_message(1), "Loaded 1 favorite");
    }

    #[test]
    fn test_favorites_loaded_message_uses_plural_for_many() {
        assert_eq!(favorites_loaded_message(3), "Loaded 3 favorites");
    }

    #[test]
    fn no_ai_commands_use_hide_then_capture_flow_after_harness_redirect() {
        // Legacy capture flow is no longer used — all active AI commands
        // route to the harness terminal which captures context inline.
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::GenerateScriptFromCurrentApp
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendScreenToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendFocusedWindowToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendScreenAreaToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendSelectedTextToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::SendBrowserTabToAi
        ));
        assert!(!ai_command_uses_hide_then_capture_flow(
            &AiCommandType::MiniAi
        ));
    }

    #[test]
    fn test_ai_capture_hide_settle_duration_matches_constant() {
        assert_eq!(
            ai_capture_hide_settle_duration(),
            std::time::Duration::from_millis(AI_CAPTURE_HIDE_SETTLE_MS)
        );
    }

    #[test]
    fn test_ai_capture_hide_settle_duration_waits_150ms() {
        assert_eq!(AI_CAPTURE_HIDE_SETTLE_MS, 150);
        assert_eq!(
            ai_capture_hide_settle_duration(),
            std::time::Duration::from_millis(150)
        );
    }

    #[test]
    fn test_emoji_picker_label_includes_emoji_and_name() {
        let emoji = Emoji {
            emoji: "🚀",
            name: "rocket",
            keywords: &["launch", "ship"],
            category: EmojiCategory::TravelPlaces,
        };

        assert_eq!(emoji_picker_label(&emoji), "🚀  rocket");
    }

    #[test]
    fn test_created_file_path_for_feedback_returns_same_path_when_already_absolute() {
        let absolute_path = PathBuf::from("/tmp/new-script.ts");
        let feedback_path = created_file_path_for_feedback(&absolute_path);

        assert_eq!(feedback_path, absolute_path);
    }

    #[test]
    fn test_created_file_path_for_feedback_joins_current_dir_when_relative() {
        let relative_path = PathBuf::from("new-script.ts");
        let current_dir = std::env::current_dir().expect("current dir should be available");
        let feedback_path = created_file_path_for_feedback(&relative_path);

        assert_eq!(feedback_path, current_dir.join(relative_path));
    }
}

#[cfg(test)]
mod dictation_model_prompt_tests {
    use super::*;

    #[test]
    fn downloading_prompt_shows_progress_bar_with_bytes_and_speed() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Downloading {
                percentage: 35,
                downloaded_bytes: 175_000_000,
                total_bytes: 500_000_000,
                speed_bytes_per_sec: 10_485_760,
                eta_seconds: Some(31),
            },
        );
        assert!(
            title.contains("35%"),
            "title must show percentage, got: {title}"
        );
        assert!(
            placeholder.contains("166.9 MB"),
            "placeholder must show downloaded bytes, got: {placeholder}"
        );
        assert!(
            placeholder.contains("10.0 MB/s"),
            "placeholder must show speed, got: {placeholder}"
        );
        assert!(
            placeholder.contains("ETA"),
            "placeholder must show ETA, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].name, "Cancel download");
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_CANCEL);
        assert_eq!(choices[1].name, "Hide");
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn failed_prompt_offers_retry() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::DownloadFailed("network timeout".to_string()),
        );
        assert_eq!(title, "Dictation model download failed");
        assert_eq!(placeholder, "network timeout");
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_DOWNLOAD);
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_CANCEL);
    }

    #[test]
    fn cancelled_prompt_offers_retry_and_done() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::DownloadFailed(
                "model download cancelled".to_string(),
            ),
        );
        assert_eq!(title, "Download cancelled");
        assert!(
            placeholder.contains("Partial download kept"),
            "cancelled placeholder must mention partial file, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].name, "Retry download");
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_DOWNLOAD);
        assert_eq!(choices[1].name, "Done");
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn not_downloaded_prompt_offers_download_and_cancel() {
        let (title, placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::NotDownloaded,
        );
        assert_eq!(title, "Download local dictation model");
        assert!(
            placeholder.contains("fully local transcription")
                || placeholder.contains("resumable if interrupted"),
            "placeholder must mention local transcription or resumability, got: {placeholder}"
        );
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_DOWNLOAD);
        assert_eq!(choices[1].value, BUILTIN_DICTATION_MODEL_CANCEL);
    }

    #[test]
    fn extracting_prompt_offers_hide() {
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Extracting,
        );
        assert_eq!(title, "Installing local dictation model\u{2026}");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_HIDE);
    }

    #[test]
    fn available_prompt_offers_done() {
        let (title, _placeholder, choices) = ScriptListApp::build_dictation_model_prompt(
            crate::dictation::DictationModelStatus::Available,
        );
        assert_eq!(title, "Dictation model ready");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].value, BUILTIN_DICTATION_MODEL_HIDE);
    }
}
