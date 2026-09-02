// ---------------------------------------------------------------------------
// Apply-back: clipboard helpers
// ---------------------------------------------------------------------------

fn with_tab_ai_apply_back_clipboard<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)
        .map_err(|error| error.to_string())?;
    // An armed owned probe replaces only the native delegate, never the guard or
    // the production helper. A regressed guard fails the probe without touching
    // the operator clipboard.
    let intercepted = TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|probe| {
        let probe = probe.borrow();
        let Some(probe) = probe.as_ref() else {
            return false;
        };
        probe.borrow_mut().constructor_calls += 1;
        true
    });
    if intercepted {
        return Err("tab_ai_apply_back_clipboard_probe_delegate_reached".into());
    }
    operation()
}

fn read_tab_ai_apply_back_clipboard_text() -> Result<String, String> {
    with_tab_ai_apply_back_clipboard(|| {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_open_failed: {error}"))?;
    let text = clipboard
        .get_text()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_read_failed: {error}"))?;
    if text.trim().is_empty() {
        return Err("tab_ai_apply_back_clipboard_empty".to_string());
    }
    Ok(text)
    })
}

fn write_tab_ai_apply_back_clipboard_text(text: &str) -> Result<(), String> {
    with_tab_ai_apply_back_clipboard(|| {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_open_failed: {error}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| format!("tab_ai_apply_back_clipboard_write_failed: {error}"))
    })
}

#[derive(Default)]
struct TabAiApplyBackClipboardProbeState {
    constructor_calls: usize,
    read_refused: bool,
    write_refused: bool,
    terminal_no_selection: bool,
    terminal_prime_refused: bool,
    terminal_fallback_refused: bool,
    terminal_fallback_completions: usize,
    terminal_callback_scheduled: bool,
}

thread_local! {
    static TAB_AI_APPLY_BACK_CLIPBOARD_PROBE: std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<TabAiApplyBackClipboardProbeState>>>> = const { std::cell::RefCell::new(None) };
}

/// Keeps the inert native delegate armed through actual helper and terminal calls.
/// Dropping the guard restores the ordinary delegate.
pub(crate) struct TabAiApplyBackClipboardProbe {
    state: std::rc::Rc<std::cell::RefCell<TabAiApplyBackClipboardProbeState>>,
}

impl TabAiApplyBackClipboardProbe {
    pub(crate) fn observation(&self) -> serde_json::Value {
        let state = self.state.borrow();
        serde_json::json!({
            "constructorCalls": state.constructor_calls,
            "readRefused": state.read_refused,
            "writeRefused": state.write_refused,
            "terminalNoSelection": state.terminal_no_selection,
            "terminalPrimeRefused": state.terminal_prime_refused,
            "terminalFallbackCompleted": state.terminal_fallback_completions > 0,
            "terminalFallbackRefused": state.terminal_fallback_refused,
            "terminalFallbackCompletions": state.terminal_fallback_completions,
            "terminalFallbackCompletionKind": (state.terminal_fallback_completions > 0).then_some("synchronousRefusal"),
            "terminalCallbackScheduled": state.terminal_callback_scheduled,
        })
    }

    pub(crate) fn is_active() -> bool {
        TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|probe| probe.borrow().is_some())
    }
}

impl Drop for TabAiApplyBackClipboardProbe {
    fn drop(&mut self) {
        TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|probe| {
            let mut probe = probe.borrow_mut();
            if probe.as_ref().is_some_and(|state| std::rc::Rc::ptr_eq(state, &self.state)) {
                *probe = None;
            }
        });
    }
}

pub(crate) fn probe_tab_ai_apply_back_clipboard_boundary() -> anyhow::Result<TabAiApplyBackClipboardProbe> {
    // Never allow a negative probe to become a real clipboard operation when
    // invoked outside the owned evaluator.
    anyhow::ensure!(crate::runtime_policy::is_owned_evaluation(), "owned_policy_missing");
    anyhow::ensure!(!TabAiApplyBackClipboardProbe::is_active(), "clipboard_probe_already_active");
    let probe = TabAiApplyBackClipboardProbe {
        state: std::rc::Rc::new(std::cell::RefCell::new(TabAiApplyBackClipboardProbeState::default())),
    };
    TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|active| *active.borrow_mut() = Some(probe.state.clone()));
    let read = read_tab_ai_apply_back_clipboard_text();
    let write = write_tab_ai_apply_back_clipboard_text("negative-only clipboard refusal probe");
    {
        let mut state = probe.state.borrow_mut();
        state.read_refused = read.as_ref().err().map(String::as_str) == Some("system_clipboard_forbidden");
        state.write_refused = write.as_ref().err().map(String::as_str) == Some("system_clipboard_forbidden");
    }
    Ok(probe)
}

// ---------------------------------------------------------------------------
// Apply-back: entry point (⌘⏎ in QuickTerminalView)
// ---------------------------------------------------------------------------

/// Route-aware success message for the apply-back toast.
fn tab_ai_apply_back_success_message(source_type: &crate::ai::TabAiSourceType) -> &'static str {
    match source_type {
        crate::ai::TabAiSourceType::RunningCommand => "Applied result to the active prompt",
        crate::ai::TabAiSourceType::ClipboardEntry => "Copied result to the clipboard",
        crate::ai::TabAiSourceType::ScriptListItem => "Saved and ran the generated script",
        crate::ai::TabAiSourceType::DesktopSelection => "Replaced the frontmost selection",
        crate::ai::TabAiSourceType::Desktop => "Pasted into the frontmost app",
    }
}

impl ScriptListApp {
    const TAB_AI_APPLY_BACK_FOCUS_SETTLE_MS: u64 = 250;
    const TAB_AI_APPLY_BACK_CLIPBOARD_PRIME_MS: u64 = 25;
    const TAB_AI_APPLY_BACK_ROUTE_POLL_MS: u64 = 20;
    const TAB_AI_APPLY_BACK_ROUTE_TIMEOUT_MS: u64 = 750;

    /// Show a route-aware error toast when ⌘↩ is pressed but there is
    /// neither a terminal selection nor harness output available yet.
    fn toast_tab_ai_apply_back_unavailable(&mut self, cx: &mut Context<Self>) {
        let apply_label = crate::ai::tab_ai_apply_back_footer_label(
            self.tab_ai_harness_apply_back_route
                .as_ref()
                .map(|route| &route.source_type),
        );
        self.toast_manager.push(
            crate::components::toast::Toast::error(
                format!("{apply_label} failed: select terminal text or wait for output."),
                &self.theme,
            )
            .duration_ms(Some(TOAST_ERROR_MS)),
        );
        cx.notify();
    }

    /// Show a route-aware error toast when the apply-back route is still
    /// unavailable after the bounded wait expires.
    fn toast_tab_ai_apply_back_pending(&mut self, cx: &mut Context<Self>) {
        let message = match self.tab_ai_harness_apply_back_route.as_ref() {
            Some(route) => format!(
                "{} is still preparing. Try again in a moment.",
                crate::ai::tab_ai_apply_back_footer_label(Some(&route.source_type)),
            ),
            None => "Paste Back target is still preparing. Try again in a moment.".to_string(),
        };
        self.toast_manager.push(
            crate::components::toast::Toast::error(message, &self.theme)
                .duration_ms(Some(TOAST_ERROR_MS)),
        );
        cx.notify();
    }

    /// Unified apply handler — routes `text` to the correct destination
    /// based on `route.source_type`.  Called by both the terminal-selection
    /// fast path and the clipboard fallback.
    fn apply_tab_ai_result_text(
        &mut self,
        route: crate::ai::TabAiApplyBackRoute,
        text: String,
        cx: &mut Context<Self>,
    ) {
        if text.trim().is_empty() {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "No terminal selection or harness output was available".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
            return;
        }

        match route.source_type.clone() {
            crate::ai::TabAiSourceType::RunningCommand => {
                self.close_tab_ai_harness_terminal(cx);
                if self.try_set_prompt_input(text.clone(), cx) {
                    self.toast_manager.push(
                        crate::components::toast::Toast::success(
                            tab_ai_apply_back_success_message(&route.source_type).to_string(),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                } else {
                    self.toast_manager.push(
                        crate::components::toast::Toast::error(
                            "The original prompt is no longer active".to_string(),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                }
                cx.notify();
            }
            crate::ai::TabAiSourceType::ClipboardEntry => {
                self.close_tab_ai_harness_terminal(cx);
                match write_tab_ai_apply_back_clipboard_text(&text) {
                    Ok(()) => {
                        self.toast_manager.push(
                            crate::components::toast::Toast::success(
                                tab_ai_apply_back_success_message(&route.source_type).to_string(),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_SUCCESS_MS)),
                        );
                    }
                    Err(error) => {
                        self.toast_manager.push(
                            crate::components::toast::Toast::error(
                                format!("Failed to update clipboard: {error}"),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_ERROR_MS)),
                        );
                    }
                }
                cx.notify();
            }
            crate::ai::TabAiSourceType::ScriptListItem => {
                self.close_tab_ai_harness_terminal(cx);

                // Use the focused target label as the prompt for slug derivation.
                let prompt_label = route
                    .focused_target
                    .as_ref()
                    .map(|t| t.label.clone())
                    .unwrap_or_else(|| "ai generated script".to_string());

                match crate::ai::script_generation::save_generated_script_from_response(
                    &prompt_label,
                    &text,
                ) {
                    Ok(script_path) => {
                        let path_str = script_path.to_string_lossy().to_string();
                        tracing::info!(
                            target: "tab_ai",
                            source_type = "ScriptListItem",
                            script_path_sha256 = %crate::logging::log_private_user_value(&path_str),
                            script_path_bytes = path_str.len(),
                            "tab_ai_apply_back.script_saved"
                        );
                        self.toast_manager.push(
                            crate::components::toast::Toast::success(
                                format!(
                                    "Saved and running generated script: {}",
                                    script_path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("script"),
                                ),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_SUCCESS_MS)),
                        );
                        self.execute_script_by_path(&path_str, cx);
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "tab_ai",
                            error = %error,
                            "tab_ai_apply_back.script_save_failed"
                        );
                        self.toast_manager.push(
                            crate::components::toast::Toast::error(
                                format!("Failed to save generated script: {error}"),
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_ERROR_MS)),
                        );
                    }
                }
                cx.notify();
            }
            /* crate::ai::TabAiSourceType::DesktopSelection
            | crate::ai::TabAiSourceType::Desktop => */
            crate::ai::TabAiSourceType::DesktopSelection | crate::ai::TabAiSourceType::Desktop => {
                // Desktop selection / generic desktop: hide the main window first,
                // wait for focus to settle back to the previous frontmost app,
                // then apply via set_selected_text or TextInjector::paste_text.
                self.close_tab_ai_harness_terminal(cx);
                crate::platform::defer_hide_main_window(cx);

                let app_weak = cx.entity().downgrade();
                cx.spawn(async move |_this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(
                            Self::TAB_AI_APPLY_BACK_FOCUS_SETTLE_MS,
                        ))
                        .await;

                    let route_for_apply = route.clone();
                    let route_for_toast = route.clone();
                    let text_for_apply = text.clone();

                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            match route_for_apply.source_type {
                                crate::ai::TabAiSourceType::DesktopSelection => {
                                    selected_text::set_selected_text(&text_for_apply)
                                        .map_err(|error| error.to_string())
                                }
                                crate::ai::TabAiSourceType::Desktop => {
                                    let injector = text_injector::TextInjector::new();
                                    injector
                                        .paste_text(&text_for_apply)
                                        .map_err(|error| error.to_string())
                                }
                                _ => Ok(()),
                            }
                        })
                        .await;

                    cx.update(|cx| {
                        let Some(app) = app_weak.upgrade() else {
                            return;
                        };
                        app.update(cx, |this, cx| {
                            match result {
                                Ok(()) => {
                                    this.toast_manager.push(
                                        crate::components::toast::Toast::success(
                                            tab_ai_apply_back_success_message(
                                                &route_for_toast.source_type,
                                            )
                                            .to_string(),
                                            &this.theme,
                                        )
                                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                                    );
                                }
                                Err(error) => {
                                    this.toast_manager.push(
                                        crate::components::toast::Toast::error(
                                            format!("Failed to apply result: {error}"),
                                            &this.theme,
                                        )
                                        .duration_ms(Some(TOAST_ERROR_MS)),
                                    );
                                }
                            }
                            cx.notify();
                        });
                    });
                })
                .detach();
            }
        }
    }

    /// Apply `text` immediately when the route is known; otherwise poll
    /// for up to `TAB_AI_APPLY_BACK_ROUTE_TIMEOUT_MS` ms.  If the route
    /// is still unavailable after the deadline, show a route-aware error
    /// toast instead of waiting forever.  Cancels silently if the harness
    /// closes (view leaves `QuickTerminalView`) or the entity is dropped.
    fn apply_tab_ai_result_text_or_wait_for_route(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(route) = self.tab_ai_harness_apply_back_route.clone() {
            self.apply_tab_ai_result_text(route, text, cx);
            return;
        }

        let app_weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(
                    ScriptListApp::TAB_AI_APPLY_BACK_ROUTE_TIMEOUT_MS,
                );

            loop {
                enum WaitState {
                    Ready(Box<crate::ai::TabAiApplyBackRoute>),
                    Pending,
                    TimedOut,
                    Cancelled,
                }

                let state = cx.update(|cx| {
                    let Some(app) = app_weak.upgrade() else {
                        return WaitState::Cancelled;
                    };
                    app.update(cx, |this, _cx| {
                        if !matches!(this.current_view, AppView::QuickTerminalView { .. }) {
                            return WaitState::Cancelled;
                        }
                        if let Some(route) = this.tab_ai_harness_apply_back_route.clone() {
                            return WaitState::Ready(Box::new(route));
                        }
                        if std::time::Instant::now() >= deadline {
                            return WaitState::TimedOut;
                        }
                        WaitState::Pending
                    })
                });

                match state {
                    WaitState::Ready(route) => {
                        cx.update(|cx| {
                            let Some(app) = app_weak.upgrade() else {
                                return;
                            };
                            app.update(cx, |this, cx| {
                                this.apply_tab_ai_result_text(*route, text.clone(), cx);
                            });
                        });
                        break;
                    }
                    WaitState::TimedOut => {
                        cx.update(|cx| {
                            let Some(app) = app_weak.upgrade() else {
                                return;
                            };
                            app.update(cx, |this, cx| {
                                this.toast_tab_ai_apply_back_pending(cx);
                            });
                        });
                        break;
                    }
                    WaitState::Cancelled => break,
                    WaitState::Pending => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(
                                ScriptListApp::TAB_AI_APPLY_BACK_ROUTE_POLL_MS,
                            ))
                            .await;
                    }
                }
            }
        })
        .detach();
    }

    /// Apply harness output from the terminal.  Prefers the terminal selection
    /// directly (no clipboard round-trip); falls back to clipboard priming
    /// only when no selection exists.
    #[allow(dead_code)] // Called from include!() binary code (render_prompts/term.rs)
    pub(crate) fn apply_tab_ai_result_from_terminal(
        &mut self,
        entity: Entity<term_prompt::TermPrompt>,
        cx: &mut Context<Self>,
    ) {
        // Try to read the terminal selection directly — avoids the
        // clipboard prime → timer → read race entirely.
        let selected_text =
            entity.update(cx, |term_prompt, _cx| term_prompt.selected_text_for_apply());
        TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|probe| {
            if let Some(probe) = probe.borrow().as_ref() {
                probe.borrow_mut().terminal_no_selection = selected_text.is_none();
            }
        });

        if let Some(text) = selected_text {
            self.apply_tab_ai_result_text_or_wait_for_route(text, cx);
            return;
        }

        // No selection — fall back to clipboard priming (copies last output).
        let prime = with_tab_ai_apply_back_clipboard(|| {
            entity.update(cx, |term_prompt, cx| {
                term_prompt.prime_apply_clipboard(cx);
            });
            Ok(())
        });
        // A denied prime must not leave a delayed read of unrelated clipboard
        // content behind. The allowed production timer path below is unchanged.
        if let Err(error) = prime {
            tracing::warn!(%error, "Tab AI apply-back clipboard priming refused");
            self.show_error_toast(format!("Paste Back failed: {error}"), cx);
            TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|probe| {
                if let Some(probe) = probe.borrow().as_ref() {
                    let mut probe = probe.borrow_mut();
                    probe.terminal_prime_refused = error == "system_clipboard_forbidden";
                    probe.terminal_fallback_refused = probe.terminal_prime_refused;
                    probe.terminal_fallback_completions += 1;
                }
            });
            return;
        }

        TAB_AI_APPLY_BACK_CLIPBOARD_PROBE.with(|probe| {
            if let Some(probe) = probe.borrow().as_ref() {
                probe.borrow_mut().terminal_callback_scheduled = true;
            }
        });
        let app = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    Self::TAB_AI_APPLY_BACK_CLIPBOARD_PRIME_MS,
                ))
                .await;
            cx.update(|cx| {
                let Some(app) = app.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    this.apply_tab_ai_result_from_clipboard(cx);
                });
            });
        })
        .detach();
    }

    pub(crate) fn apply_tab_ai_result_from_clipboard(&mut self, cx: &mut Context<Self>) {
        let text = match read_tab_ai_apply_back_clipboard_text() {
            Ok(text) => text,
            Err(_error) => {
                self.toast_tab_ai_apply_back_unavailable(cx);
                return;
            }
        };

        self.apply_tab_ai_result_text_or_wait_for_route(text, cx);
    }
}
