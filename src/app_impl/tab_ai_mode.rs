use super::*;

/// Resolved Tab AI context payload ready for harness submission.
#[derive(Debug, Clone)]
struct TabAiResolvedContext {
    context: crate::ai::TabAiContextBlob,
    invocation_receipt: crate::ai::TabAiInvocationReceipt,
    suggested_intents: Vec<crate::ai::TabAiSuggestedIntentSpec>,
}

/// Pre-switch snapshot of the UI state captured at the Tab interception
/// boundary, before the view flips to `QuickTerminalView`.
///
/// The deferred capture pipeline uses this to assemble context in the
/// background while the harness terminal is already visible.
#[derive(Debug, Clone)]
struct TabAiLaunchRequest {
    /// The `AppView` that was active when Tab was pressed.
    source_view: AppView,
    /// Optional user intent (from Shift+Tab typed query).
    entry_intent: Option<String>,
    /// Plain launcher Tab should submit only the current text and never
    /// translate the focused row into an ACP context chip.
    suppress_focused_part: bool,
    /// Quick-submit plan from the deterministic planner (fallback / dictation).
    quick_submit_plan: Option<crate::ai::TabAiQuickSubmitPlan>,
    /// UI snapshot taken synchronously before the view switch.
    ui_snapshot: crate::ai::TabAiUiSnapshot,
    /// Invocation receipt for logging and downstream consumption.
    invocation_receipt: crate::ai::TabAiInvocationReceipt,
    /// What kind of capture to perform (focused window, full screen, etc.).
    capture_kind: crate::ai::TabAiCaptureKind,
    /// Monotonic generation counter — used to drop stale capture results.
    capture_generation: u64,
}

/// Artifacts produced by the deferred background capture task.
#[derive(Debug, Clone, Default)]
struct TabAiDeferredCaptureArtifacts {
    /// Desktop context snapshot (frontmost app, selected text, browser URL).
    desktop: crate::context_snapshot::AiContextSnapshot,
    /// Absolute path to the focused window screenshot file, if captured.
    screenshot_path: Option<String>,
}

/// Channel receiver for deferred capture results.
type TabAiDeferredCaptureRx =
    async_channel::Receiver<Result<TabAiDeferredCaptureArtifacts, String>>;

/// Maximum visible elements captured per UI snapshot for Tab AI context.
const TAB_AI_VISIBLE_ELEMENT_LIMIT: usize = 24;

/// Maximum visible targets resolved per surface for Tab AI context.
const TAB_AI_VISIBLE_TARGET_LIMIT: usize = 10;

/// Maximum clipboard history entries included in the Tab AI context blob.
const TAB_AI_CLIPBOARD_HISTORY_LIMIT: usize = 8;

/// Maximum character length for hydrated clipboard text entries.
const TAB_AI_CLIPBOARD_TEXT_LIMIT: usize = 1000;

impl ScriptListApp {
    /// Give the ACP chat one frame to paint before deferred context staging runs.
    const ACP_CONTEXT_FIRST_PAINT_DELAY_MS: u64 = 16;

    fn wire_embedded_acp_footer_callbacks(
        &mut self,
        view_entity: &Entity<crate::ai::acp::AcpChatView>,
        cx: &mut Context<Self>,
    ) {
        let app_entity = cx.entity().clone();
        view_entity.update(cx, |view, _cx| {
            let actions_app = app_entity.clone();
            view.set_on_toggle_actions(move |window, cx| {
                actions_app.update(cx, |app, cx| {
                    app.toggle_actions(cx, window);
                });
            });

            let close_app = app_entity.clone();
            view.set_on_close_requested(move |window, cx| {
                close_app.update(cx, |app, cx| {
                    app.close_tab_ai_harness_terminal_with_window(window, cx);
                });
            });

            let history_app = app_entity.clone();
            view.set_on_open_history_command(move |window, cx| {
                history_app.update(cx, |app, cx| {
                    app.open_embedded_acp_history_popup(window, cx);
                });
            });

            let portal_app = app_entity.clone();
            view.set_on_open_portal(move |kind, cx| {
                portal_app.update(cx, |app, cx| {
                    app.open_attachment_portal(kind, cx);
                });
            });
        });
    }

    /// Open the Tab AI surface (zero-intent).
    ///
    /// Routes to the harness terminal (`QuickTerminalView`), which connects
    /// to a pre-running CLI harness (Claude Code, Codex, Gemini CLI, etc.)
    /// and injects a flat text-native context block via PTY stdin.
    pub(crate) fn open_tab_ai_chat(&mut self, cx: &mut Context<Self>) {
        self.open_tab_ai_chat_with_entry_intent(None, cx);
    }

    /// Primary Tab entry point.
    ///
    /// - `None` => open the harness and stage context only (`PasteOnly`)
    /// - `Some(intent)` => open the harness and immediately submit that intent
    pub(crate) fn open_tab_ai_chat_with_entry_intent(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_chat_with_capture_kind_and_options(
            entry_intent,
            crate::ai::TabAiCaptureKind::DefaultContext,
            false,
            cx,
        );
    }

    pub(crate) fn open_tab_ai_chat_with_entry_intent_suppressing_focused_part(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_chat_with_capture_kind_and_options(
            entry_intent,
            crate::ai::TabAiCaptureKind::DefaultContext,
            true,
            cx,
        );
    }

    /// Entry point that always routes to ACP chat, bypassing the surface
    /// preference routing that may redirect to the quick terminal.
    ///
    /// Used by the Auto Submit fallback so it always opens the ACP chat
    /// experience regardless of script-authoring detection heuristics.
    pub(crate) fn open_tab_ai_acp_with_entry_intent(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_acp_with_options(entry_intent, false, cx);
    }

    fn open_tab_ai_acp_with_options(
        &mut self,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        // If a detached chat window exists, bring it to front instead
        // of opening a new panel chat.
        if crate::ai::acp::chat_window::is_chat_window_open() {
            if let Err(e) = crate::ai::acp::chat_window::open_chat_window(cx) {
                tracing::debug!(%e, "failed to focus detached chat window");
            } else {
                tracing::info!("tab_ai_focused_detached_window");
                return;
            }
        }

        let normalized_entry_intent = entry_intent
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let has_cached_retry_request = self
            .embedded_acp_chat
            .as_ref()
            .is_some_and(|entity| entity.read(cx).has_retry_request());

        if Self::should_reuse_embedded_acp_view_for_open(
            normalized_entry_intent,
            has_cached_retry_request,
        ) && self.try_reuse_embedded_acp_view(entry_intent.clone(), cx)
        {
            return;
        }

        self.begin_tab_ai_harness_entry(
            entry_intent,
            suppress_focused_part,
            None,
            crate::ai::TabAiCaptureKind::DefaultContext,
            true,
            cx,
        );
    }

    /// Open ACP Chat with an explicit focused target, bypassing view-based
    /// auto-resolution. Used by Cmd+Enter from surfaces that own plain Enter
    /// locally (action menus, Notes) and want to hand off a canonical
    /// `FocusedTarget` chip to ACP without re-resolving from the current view.
    pub(crate) fn open_tab_ai_acp_with_explicit_target(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        let label = Self::format_tab_ai_focused_chip_label(&target);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_explicit_target_acp_open",
            item_source = %target.source,
            item_kind = %target.kind,
            semantic_id = %target.semantic_id,
            label = %label,
        );

        let focused_part =
            Some(crate::ai::message_parts::AiContextPart::FocusedTarget { target, label });

        // Minimal snapshot for the launch request — we already know the target.
        let (ui_snapshot, invocation_receipt) = self.snapshot_tab_ai_ui(cx);
        self.tab_ai_harness_capture_generation += 1;

        let request = TabAiLaunchRequest {
            source_view: self.current_view.clone(),
            entry_intent: None,
            suppress_focused_part: false,
            quick_submit_plan: None,
            ui_snapshot,
            invocation_receipt,
            capture_kind: crate::ai::TabAiCaptureKind::DefaultContext,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        // No ambient capture needed — explicit target path skips desktop snapshot.
        let (_tx, rx) = async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);

        self.open_tab_ai_acp_view_from_request_impl(
            request,
            rx,
            focused_part,
            false, // use_ask_anything_fallback
            None,  // explicit_ambient_chip_label
            true,  // force_acp_surface
            cx,
        );
    }

    /// Open ACP Chat with a selected plugin skill staged as context.
    ///
    /// The skill's SKILL.md content is read and staged as the initial prompt
    /// context, with a deterministic initial input referencing the skill.
    /// This keeps `selected_agent`/provider logic untouched.
    pub(crate) fn open_acp_with_selected_skill(
        &mut self,
        skill: &crate::plugins::PluginSkill,
        cx: &mut Context<Self>,
    ) {
        let owner = if skill.plugin_title.is_empty() {
            &skill.plugin_id
        } else {
            &skill.plugin_title
        };
        let initial_input =
            crate::ai::acp::build_staged_skill_prompt(&skill.title, owner, &skill.path);

        tracing::info!(
            event = "acp_skill_context_staged",
            plugin_id = %skill.plugin_id,
            skill_id = %skill.skill_id,
            skill_title = %skill.title,
            owner,
            path = %skill.path.display(),
            prompt_len = initial_input.len(),
            "Staged plugin skill prompt for ACP session"
        );

        self.open_tab_ai_acp_with_entry_intent(Some(initial_input), cx);
    }

    pub(crate) fn open_tab_ai_acp_with_slash_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_ai_harness_script_list_trigger = Some('/');
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_from_script_list_trigger",
            trigger = "/",
            current_view = ?self.current_view,
        );
        self.open_tab_ai_acp_with_entry_intent(None, cx);

        let detached_opened = crate::ai::acp::chat_window::open_detached_slash_picker(cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_trigger_picker_open_attempt",
            trigger = "/",
            detached_opened,
        );
        if detached_opened {
            return;
        }

        if let AppView::AcpChatView { entity } = &self.current_view {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_trigger_picker_open_embedded_deferred",
                trigger = "/",
            );
            self.schedule_embedded_acp_picker_open(window.window_handle(), entity.clone(), '/', cx);
        }
    }

    pub(crate) fn open_tab_ai_acp_with_mention_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tab_ai_harness_script_list_trigger = Some('@');
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_from_script_list_trigger",
            trigger = "@",
            current_view = ?self.current_view,
        );
        self.open_tab_ai_acp_with_entry_intent(None, cx);

        let detached_opened = crate::ai::acp::chat_window::open_detached_mention_picker(cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_trigger_picker_open_attempt",
            trigger = "@",
            detached_opened,
        );
        if detached_opened {
            return;
        }

        if let AppView::AcpChatView { entity } = &self.current_view {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_trigger_picker_open_embedded_deferred",
                trigger = "@",
            );
            self.schedule_embedded_acp_picker_open(window.window_handle(), entity.clone(), '@', cx);
        }
    }

    fn schedule_embedded_acp_picker_open(
        &self,
        window_handle: gpui::AnyWindowHandle,
        entity: gpui::Entity<crate::ai::acp::AcpChatView>,
        trigger: char,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    Self::ACP_CONTEXT_FIRST_PAINT_DELAY_MS,
                ))
                .await;

            let _ = window_handle.update(cx, |_root, window, cx| {
                entity.update(cx, |view, cx| match trigger {
                    '/' => view.open_slash_picker_in_window(window, cx),
                    '@' => view.open_mention_picker_in_window(window, cx),
                    _ => {}
                });
            });
        })
        .detach();
    }

    fn try_reuse_embedded_acp_view(
        &mut self,
        entry_intent: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(entity) = self.embedded_acp_chat.as_ref().cloned() else {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_embedded_acp_cache_miss",
            );
            return false;
        };

        let is_setup_mode = entity.read(cx).is_setup_mode();
        let normalized_intent = entry_intent
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let source_view = self.current_view.clone();

        self.tab_ai_harness_return_view = Some(source_view.clone());
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
        self.current_view = AppView::AcpChatView {
            entity: entity.clone(),
        };
        self.focused_input = FocusedInput::None;
        self.show_actions_popup = false;
        self.actions_dialog = None;
        self.pending_focus = Some(FocusTarget::ChatPrompt);

        if let Some(intent) = normalized_intent.clone().filter(|_| !is_setup_mode) {
            entity.update(cx, |chat, cx| {
                chat.submit_reused_entry_intent(intent.clone(), cx);
            });
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_embedded_acp_reused",
            source_view = ?source_view,
            auto_submit = normalized_intent.is_some(),
            is_setup_mode,
        );
        cx.notify();
        true
    }

    fn should_reuse_embedded_acp_view_for_open(
        entry_intent: Option<&str>,
        has_cached_retry_request: bool,
    ) -> bool {
        entry_intent.is_some() && !has_cached_retry_request
    }

    /// Entry point with explicit capture kind.
    ///
    /// Used by `SendScreenToAi`, `SendFocusedWindowToAi`, etc. so each
    /// command gets the appropriate screenshot/context capture behaviour
    /// instead of always defaulting to focused-window.
    pub(crate) fn open_tab_ai_chat_with_capture_kind(
        &mut self,
        entry_intent: Option<String>,
        capture_kind: crate::ai::TabAiCaptureKind,
        cx: &mut Context<Self>,
    ) {
        self.open_tab_ai_chat_with_capture_kind_and_options(entry_intent, capture_kind, false, cx);
    }

    fn open_tab_ai_chat_with_capture_kind_and_options(
        &mut self,
        entry_intent: Option<String>,
        capture_kind: crate::ai::TabAiCaptureKind,
        suppress_focused_part: bool,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }

        // If a detached chat window exists, bring it to front instead
        // of opening a new panel chat.
        if crate::ai::acp::chat_window::is_chat_window_open() {
            if let Err(e) = crate::ai::acp::chat_window::open_chat_window(cx) {
                tracing::debug!(%e, "failed to focus detached chat window");
            } else {
                tracing::info!("tab_ai_focused_detached_window");
                return;
            }
        }

        self.begin_tab_ai_harness_entry(
            entry_intent,
            suppress_focused_part,
            None,
            capture_kind,
            false,
            cx,
        );
    }

    /// Open the harness with a pre-computed quick-submit plan.
    ///
    /// The plan's `submission_intent()` becomes the entry intent and the
    /// plan's `capture_kind` selects the right screenshot/context profile.
    pub(crate) fn open_tab_ai_chat_with_quick_submit_plan(
        &mut self,
        plan: crate::ai::TabAiQuickSubmitPlan,
        cx: &mut Context<Self>,
    ) {
        if self.tab_ai_save_offer_state.is_some() {
            return;
        }
        let capture_kind = plan.capture_kind_enum();
        let intent = Some(plan.submission_intent().to_string());
        self.begin_tab_ai_harness_entry(intent, false, Some(plan), capture_kind, false, cx);
    }

    /// Route raw text (from Auto Submit fallback or dictation) through the
    /// quick-submit planner and into the harness — either an existing live
    /// session or a fresh one.
    pub(crate) fn submit_to_current_or_new_tab_ai_harness_from_text(
        &mut self,
        raw_text: String,
        source: crate::ai::TabAiQuickSubmitSource,
        cx: &mut Context<Self>,
    ) {
        let Some(plan) = crate::ai::plan_tab_ai_quick_submit(source, &raw_text) else {
            // Empty input — open the harness without intent.
            self.open_tab_ai_acp_with_entry_intent(None, cx);
            return;
        };

        // If the ACP chat view is active, route through the shared
        // verification-input builder so script-authoring guidance is appended.
        if let AppView::AcpChatView { ref entity } = self.current_view {
            self.submit_live_acp_tab_ai_from_plan(entity.clone(), plan, cx);
            return;
        }

        // If the PTY harness is already the active surface and alive, route through
        // the full structured submission builder so live-session quick submits
        // get fresh context and quick-submit metadata.
        if let Some(session) = self
            .tab_ai_harness
            .as_ref()
            .filter(|_| matches!(self.current_view, AppView::QuickTerminalView { .. }))
            .filter(|session| session.entity.read(cx).is_alive())
        {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_quick_submit_live_session",
                source = ?plan.source,
                kind = ?plan.kind,
                capture_kind = %plan.capture_kind,
                input_len = plan.raw_query.len(),
            );

            self.submit_live_tab_ai_harness_from_plan(session.entity.clone(), plan, cx);
            return;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_quick_submit_new_session",
            source = ?plan.source,
            kind = ?plan.kind,
            capture_kind = %plan.capture_kind,
            input_len = plan.raw_query.len(),
        );

        self.open_tab_ai_chat_with_quick_submit_plan(plan, cx);
    }

    /// Submit a quick-submit plan into an already-open, live ACP chat session.
    ///
    /// Routes through `build_tab_ai_acp_initial_input_for_prompt` so that
    /// script-authoring guidance (including mandatory Bun verification) is
    /// appended when the intent matches, keeping live ACP sessions aligned
    /// with the new-session ACP path.
    fn submit_live_acp_tab_ai_from_plan(
        &mut self,
        entity: gpui::Entity<crate::ai::acp::AcpChatView>,
        plan: crate::ai::TabAiQuickSubmitPlan,
        cx: &mut Context<Self>,
    ) {
        let source_view = self
            .tab_ai_harness_return_view
            .clone()
            .unwrap_or(AppView::ScriptList);
        let prompt_type = app_view_to_prompt_type_str(&source_view);

        let surface_preference = crate::ai::harness::tab_ai_surface_preference_for_prompt(
            prompt_type,
            Some(plan.submission_intent()),
            crate::ai::TabAiHarnessSubmissionMode::Submit,
        );

        if surface_preference.use_quick_terminal {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_quick_submit_acp_live_rerouted",
                surface = "quick_terminal",
                reason = "script_verification_required",
                prompt_type = prompt_type,
                source = ?plan.source,
                kind = ?plan.kind,
                capture_kind = %plan.capture_kind,
                includes_script_authoring_skill = surface_preference.includes_script_authoring_skill,
                includes_bun_build_verification = surface_preference.includes_bun_build_verification,
                includes_bun_execute_verification = surface_preference.includes_bun_execute_verification,
            );

            let capture_kind = plan.capture_kind_enum();
            let entry_intent = Some(plan.submission_intent().to_string());
            self.begin_tab_ai_harness_entry_from_source_view(
                source_view,
                entry_intent,
                false,
                Some(plan),
                capture_kind,
                false,
                cx,
            );
            return;
        }

        let initial_input = crate::ai::harness::build_tab_ai_acp_initial_input_for_prompt(
            prompt_type,
            plan.submission_intent(),
        );

        let submission_text = if initial_input.text.is_empty() {
            plan.submission_intent().to_string()
        } else {
            initial_input.text
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_quick_submit_acp_live",
            prompt_type = prompt_type,
            source = ?plan.source,
            kind = ?plan.kind,
            input_len = plan.raw_query.len(),
            submission_len = submission_text.len(),
            guidance_appended = initial_input.guidance_appended,
            forced_by_script_list_submit = initial_input.forced_by_script_list_submit,
            includes_script_authoring_skill = initial_input.includes_script_authoring_skill,
            includes_bun_build_verification = initial_input.includes_bun_build_verification,
            includes_bun_execute_verification = initial_input.includes_bun_execute_verification,
        );

        entity.update(cx, |chat, cx| {
            chat.live_thread().update(cx, |thread, cx| {
                thread.set_input(submission_text, cx);
                if let Err(error) = thread.submit_input(cx) {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_quick_submit_acp_live_submit_failed",
                        prompt_type = prompt_type,
                        error = %error,
                    );
                }
            });
        });
    }

    /// Submit a structured turn into an already-open, live harness session.
    ///
    /// Captures fresh desktop context, rebuilds the full flat-line context
    /// submission with quick-submit metadata, and injects via Submit mode.
    /// On build failure, shows an error toast instead of falling back to
    /// raw intent-only PTY injection.
    fn submit_live_tab_ai_harness_from_plan(
        &mut self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        plan: crate::ai::TabAiQuickSubmitPlan,
        cx: &mut Context<Self>,
    ) {
        let capture_kind = plan.capture_kind_enum();
        let source_view = self
            .tab_ai_harness_return_view
            .clone()
            .unwrap_or(AppView::ScriptList);

        let (ui_snapshot, invocation_receipt) = self.snapshot_tab_ai_ui(cx);
        self.tab_ai_harness_capture_generation += 1;

        let entry_intent = plan.submission_intent().to_string();
        let request = TabAiLaunchRequest {
            source_view,
            entry_intent: Some(entry_intent),
            suppress_focused_part: false,
            quick_submit_plan: Some(plan),
            ui_snapshot,
            invocation_receipt,
            capture_kind,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        let wait_for_readiness = Self::tab_ai_harness_needs_readiness_wait(&entity, cx);
        let capture_rx = self.spawn_tab_ai_pre_switch_capture(&request);
        let app_weak = cx.entity().downgrade();
        let capture_gen = request.capture_generation;

        cx.spawn(async move |_this, cx| {
            let capture_result = match capture_rx.recv().await {
                Ok(result) => result,
                Err(_) => Err("deferred capture channel closed".to_string()),
            };

            let artifacts = match capture_result {
                Ok(artifacts) => artifacts,
                Err(error) => {
                    tracing::warn!(
                        event = "tab_ai_live_quick_submit_capture_failed",
                        error = %error,
                    );
                    TabAiDeferredCaptureArtifacts::default()
                }
            };

            let _ = cx.update(|cx| {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    if this.tab_ai_harness_capture_generation != capture_gen {
                        tracing::debug!(
                            event = "tab_ai_live_quick_submit_stale",
                            expected = capture_gen,
                            current = this.tab_ai_harness_capture_generation,
                        );
                        return;
                    }

                    let resolved = this.build_tab_ai_context_from(
                        request.entry_intent.clone().unwrap_or_default(),
                        request.source_view.clone(),
                        request.ui_snapshot.clone(),
                        artifacts.desktop,
                        request.invocation_receipt.clone(),
                        cx,
                    );

                    let source_type = detect_tab_ai_source_type(
                        &request.source_view,
                        &resolved.context.desktop,
                        resolved.context.focused_target.as_ref(),
                    );
                    let apply_back_hint = build_tab_ai_apply_back_hint(source_type.as_ref());

                    this.tab_ai_harness_apply_back_route = source_type
                        .clone()
                        .zip(apply_back_hint.clone())
                        .map(|(source_type, hint)| crate::ai::TabAiApplyBackRoute {
                            source_type,
                            hint,
                            focused_target: resolved.context.focused_target.clone(),
                        });

                    let context = resolved.context.with_deferred_capture_fields(
                        source_type,
                        artifacts.screenshot_path,
                        apply_back_hint,
                    );

                    match crate::ai::build_tab_ai_harness_submission(
                        &context,
                        request.entry_intent.as_deref(),
                        crate::ai::TabAiHarnessSubmissionMode::Submit,
                        request.quick_submit_plan.as_ref(),
                        Some(&resolved.invocation_receipt),
                        &resolved.suggested_intents,
                    ) {
                        Ok(submission) => {
                            this.inject_tab_ai_harness_submission(
                                entity.clone(),
                                submission,
                                wait_for_readiness,
                                true,
                                cx,
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "tab_ai_live_quick_submit_build_failed",
                                error = %error,
                            );
                            this.toast_manager.push(
                                crate::components::toast::Toast::error(
                                    format!("Failed to build quick-submit context: {error}"),
                                    &this.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }

    /// Map a target kind to its human-readable chip prefix.
    pub(crate) fn tab_ai_chip_prefix_for_kind(kind: &str) -> &'static str {
        match kind {
            "file" => "File",
            "directory" => "Folder",
            "search_query" => "Search",
            "input" => "Input",
            "clipboard_entry" => "Clipboard",
            "script" | "scriptlet" | "builtin" => "Command",
            "window" => "Window",
            "app" => "App",
            "process" => "Process",
            "menu_command" => "Menu Command",
            "action" => "Action",
            "note" => "Note",
            "agent" => "Agent",
            "fallback" => "Suggestion",
            _ => "Selection",
        }
    }

    /// Format a canonical chip label from a resolved target.
    ///
    /// Delegates to the shared `format_explicit_target_chip_label` so Notes,
    /// actions, and the main-window ACP openings all produce the same text.
    fn format_tab_ai_focused_chip_label(target: &crate::ai::TabAiTargetContext) -> String {
        crate::ai::format_explicit_target_chip_label(target)
    }

    /// Resolve targets for a view and emit a structured audit log.
    fn resolve_tab_ai_targets_with_audit_for_view(
        &self,
        view: &AppView,
        ui: &crate::ai::TabAiUiSnapshot,
        phase: &str,
    ) -> (
        Option<crate::ai::TabAiTargetContext>,
        Vec<crate::ai::TabAiTargetContext>,
    ) {
        let (focused_target, visible_targets) =
            self.resolve_tab_ai_surface_targets_for_view(view, ui);
        crate::ai::TabAiTargetAudit::from_targets(
            &ui.prompt_type,
            &focused_target,
            &visible_targets,
        )
        .emit_with_phase(phase);
        (focused_target, visible_targets)
    }

    /// Route a plain Tab press from Script List into ACP.
    ///
    /// Non-empty launcher input is forwarded as raw ACP composer text and
    /// auto-submitted. Empty launcher input only opens ACP and waits.
    pub(crate) fn try_route_plain_tab_to_acp_context_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.show_actions_popup || self.tab_ai_save_offer_state.is_some() {
            return false;
        }

        let source_view = self.app_view_name();
        let entry_intent = matches!(self.current_view, AppView::ScriptList)
            .then(|| self.filter_text.trim())
            .filter(|text| !text.is_empty())
            .map(str::to_string);

        if let Some(intent) = entry_intent.clone() {
            if let Some(entity) = crate::ai::acp::chat_window::get_detached_acp_view_entity() {
                if let Err(error) = crate::ai::acp::chat_window::open_chat_window(cx) {
                    tracing::debug!(%error, "failed to focus detached chat window");
                } else {
                    entity.update(cx, |chat, cx| {
                        chat.live_thread().update(cx, |thread, cx| {
                            thread.set_input(intent.clone(), cx);
                            if let Err(error) = thread.submit_input(cx) {
                                tracing::warn!(
                                    target: "script_kit::tab_ai",
                                    event = "tab_ai_plain_tab_detached_submit_failed",
                                    error = %error,
                                );
                            }
                        });
                    });

                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_plain_tab_submitted_to_detached_acp",
                        source_view = %source_view,
                        input_len = intent.len(),
                    );
                    return true;
                }
            }
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_plain_tab_routed_to_acp",
            source_view = %source_view,
            auto_submit = entry_intent.is_some(),
            input_len = entry_intent.as_ref().map(|text| text.len()).unwrap_or(0),
        );

        self.open_tab_ai_acp_with_options(entry_intent, true, cx);
        true
    }

    /// Returns `true` when the current view should treat global `Cmd+Enter`
    /// as launcher-style "send this context to AI".
    fn supports_global_cmd_enter_ai_entry(view: &AppView) -> bool {
        matches!(
            view,
            AppView::ScriptList
                | AppView::ClipboardHistoryView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::DesignGalleryView { .. }
                | AppView::ThemeChooserView { .. }
                | AppView::EmojiPickerView { .. }
                | AppView::BrowseKitsView { .. }
                | AppView::InstalledKitsView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::CreateAiPresetView { .. }
                | AppView::SettingsView { .. }
                | AppView::FavoritesBrowseView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::FileSearchView { .. }
        )
    }

    /// Route a launcher-style `Cmd+Enter` press from the current non-AI source
    /// surface into ACP.
    ///
    /// Returns `true` when the keypress was consumed and ACP launch began.
    pub(crate) fn try_route_global_cmd_enter_to_acp_context_capture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.show_actions_popup || self.tab_ai_save_offer_state.is_some() {
            return false;
        }
        if !Self::supports_global_cmd_enter_ai_entry(&self.current_view) {
            return false;
        }

        let source_view = self.app_view_name();
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_global_cmd_enter_routed_to_acp",
            source_view = %source_view,
        );

        self.open_tab_ai_acp_with_entry_intent(None, cx);
        true
    }

    /// Build a focused-target `AiContextPart` from the current view's
    /// resolved focus, if any. Returns `None` when the active surface has
    /// no resolvable focused item (e.g. empty list, generic prompt).
    fn build_tab_ai_focused_part_for_view(
        &self,
        source_view: &AppView,
        ui_snapshot: &crate::ai::TabAiUiSnapshot,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        let (focused_target, _visible_targets) =
            self.resolve_tab_ai_targets_with_audit_for_view(source_view, ui_snapshot, "pre_open");
        focused_target.map(|target| {
            let label = Self::format_tab_ai_focused_chip_label(&target);
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_focused_chip_label_built",
                item_source = %target.source,
                item_kind = %target.kind,
                label_chars = label.chars().count(),
            );
            crate::ai::message_parts::AiContextPart::FocusedTarget { target, label }
        })
    }

    /// Returns `true` when the Tab press should route to the Ask Anything
    /// fallback (ambient desktop context) instead of the focused-target chip
    /// path. This happens when no focused item is resolvable from the active
    /// surface.
    fn should_use_tab_ai_ask_anything_fallback(
        &self,
        source_view: &AppView,
        ui_snapshot: &crate::ai::TabAiUiSnapshot,
    ) -> bool {
        self.build_tab_ai_focused_part_for_view(source_view, ui_snapshot)
            .is_none()
    }

    /// Deferred-capture entry point: build a launch request from pre-switch
    /// state, start background capture, then immediately open the harness.
    ///
    /// The harness terminal appears within one frame of the Tab keypress.
    /// Context capture (desktop snapshot, screenshot-to-file) runs in the
    /// background and is injected into the live PTY once complete.
    fn begin_tab_ai_harness_entry(
        &mut self,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        quick_submit_plan: Option<crate::ai::TabAiQuickSubmitPlan>,
        capture_kind: crate::ai::TabAiCaptureKind,
        force_acp_surface: bool,
        cx: &mut Context<Self>,
    ) {
        self.begin_tab_ai_harness_entry_from_source_view(
            self.current_view.clone(),
            entry_intent,
            suppress_focused_part,
            quick_submit_plan,
            capture_kind,
            force_acp_surface,
            cx,
        );
    }

    /// Deferred-capture entry point with an explicit source view.
    ///
    /// This is the inner implementation that `begin_tab_ai_harness_entry`
    /// delegates to. The `source_view` parameter allows callers like the
    /// ACP live-submit reroute to preserve the original source surface
    /// instead of using `self.current_view` (which may already be ACP).
    fn begin_tab_ai_harness_entry_from_source_view(
        &mut self,
        source_view: AppView,
        entry_intent: Option<String>,
        suppress_focused_part: bool,
        quick_submit_plan: Option<crate::ai::TabAiQuickSubmitPlan>,
        capture_kind: crate::ai::TabAiCaptureKind,
        force_acp_surface: bool,
        cx: &mut Context<Self>,
    ) {
        let snapshot_started_at = std::time::Instant::now();
        let (ui_snapshot, invocation_receipt) = if matches!(source_view, AppView::ScriptList) {
            self.snapshot_tab_ai_ui_fast_for_script_list()
        } else {
            self.snapshot_tab_ai_ui(cx)
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_stage",
            stage = "snapshot_tab_ai_ui",
            stage_ms = snapshot_started_at.elapsed().as_millis() as u64,
            source_view = match &source_view {
                AppView::ScriptList => "ScriptList",
                _ => "Other",
            },
        );
        let entry_intent = entry_intent
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        // Emit the receipt as a standalone structured log line for agent/test consumption
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_invocation_receipt",
            prompt_type = %invocation_receipt.prompt_type,
            input_status = %invocation_receipt.input_status,
            focus_status = %invocation_receipt.focus_status,
            elements_status = %invocation_receipt.elements_status,
            has_input_text = invocation_receipt.has_input_text,
            has_focus_target = invocation_receipt.has_focus_target,
            element_count = invocation_receipt.element_count,
            warning_count = invocation_receipt.warning_count,
            rich = invocation_receipt.rich,
            degradation_reasons = ?invocation_receipt.degradation_reasons,
            receipt_json = %serde_json::to_string(&invocation_receipt).unwrap_or_default(),
        );

        self.tab_ai_harness_capture_generation += 1;

        let request = TabAiLaunchRequest {
            source_view,
            entry_intent,
            suppress_focused_part,
            quick_submit_plan,
            ui_snapshot,
            invocation_receipt,
            capture_kind,
            capture_generation: self.tab_ai_harness_capture_generation,
        };

        // Compute surface preference from the shared verification markers
        // *before* branching so the decision is logged and deterministic.
        let effective_intent = Self::tab_ai_effective_submission_intent(&request);
        let submit_now = request
            .quick_submit_plan
            .as_ref()
            .map(|plan| plan.submit)
            .unwrap_or(request.entry_intent.is_some());
        let submission_mode = if submit_now {
            crate::ai::TabAiHarnessSubmissionMode::Submit
        } else {
            crate::ai::TabAiHarnessSubmissionMode::PasteOnly
        };
        let surface_preference = crate::ai::harness::tab_ai_surface_preference_for_prompt(
            &request.ui_snapshot.prompt_type,
            effective_intent.as_deref(),
            submission_mode,
        );

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_surface_selected",
            surface = if surface_preference.use_quick_terminal { "quick_terminal" } else { "acp_chat" },
            reason = if surface_preference.use_quick_terminal { "script_verification_required" } else { "default_acp" },
            prompt_type = %request.ui_snapshot.prompt_type,
            has_entry_intent = request.entry_intent.is_some(),
            submit_now,
            includes_script_authoring_skill = surface_preference.includes_script_authoring_skill,
            includes_bun_build_verification = surface_preference.includes_bun_build_verification,
            includes_bun_execute_verification = surface_preference.includes_bun_execute_verification,
        );

        // Resolve whether we have a focused target or need the Ask Anything
        // fallback *before* spawning background capture.
        let pending_script_list_trigger = self.tab_ai_harness_script_list_trigger;
        let should_stage_focused_part = Self::should_stage_focused_part_for_request(
            &request.source_view,
            pending_script_list_trigger,
            request.suppress_focused_part,
        );
        // let use_ask_anything_fallback = self.should_use_tab_ai_ask_anything_fallback(
        let use_ask_anything_fallback = self
            .should_use_tab_ai_ask_anything_fallback(&request.source_view, &request.ui_snapshot);
        let use_ask_anything_fallback = should_stage_focused_part && use_ask_anything_fallback;

        // Explicit AI commands (screen, focused window, selected text, browser tab)
        // must force ambient capture even when the source surface has a focused item.
        let explicit_ambient_chip_label =
            Self::tab_ai_explicit_ambient_chip_label(&request.capture_kind).map(str::to_string);

        if !should_stage_focused_part {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_focus_chip_suppressed_for_script_list_trigger",
                trigger = ?pending_script_list_trigger,
            );
        }

        let focused_part = if use_ask_anything_fallback || explicit_ambient_chip_label.is_some() {
            None
        } else if !should_stage_focused_part {
            None
        } else {
            self.build_tab_ai_focused_part_for_view(&request.source_view, &request.ui_snapshot)
        };

        // Only run expensive ambient capture for the Ask Anything fallback path
        // or explicit ambient capture commands.
        // Focused-target Tab flows skip desktop snapshots and screenshots.
        let capture_rx = if use_ask_anything_fallback && explicit_ambient_chip_label.is_none() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_ask_anything_fallback",
                source_view = match &request.source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
            );
            self.spawn_tab_ai_pre_switch_capture(&request)
        } else if let Some(ref label) = explicit_ambient_chip_label {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_explicit_ambient_capture",
                source_view = match &request.source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
                chip_label = %label,
            );
            self.spawn_tab_ai_pre_switch_capture(&request)
        } else {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "tab_ai_focus_chip_staged",
                source_view = match &request.source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
            );
            // No ambient capture needed — return a dummy closed channel.
            let (_tx, rx) =
                async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);
            rx
        };

        if surface_preference.use_quick_terminal && !force_acp_surface {
            self.open_tab_ai_harness_terminal_from_request(request, capture_rx, cx);
        } else {
            self.open_tab_ai_acp_view_from_request_impl(
                request,
                capture_rx,
                focused_part,
                use_ask_anything_fallback,
                explicit_ambient_chip_label,
                force_acp_surface,
                cx,
            );
        }
    }

    /// Start background capture immediately on a dedicated OS thread.
    ///
    /// Captures the desktop context snapshot and (best-effort) focused window
    /// screenshot. Returns a channel receiver that delivers the results.
    ///
    /// Uses `std::thread::spawn` instead of `cx.spawn` + background executor
    /// so the expensive AX/screenshot work begins *immediately* — before the
    /// view switch can steal focus from the frontmost app.
    fn spawn_tab_ai_pre_switch_capture(
        &self,
        request: &TabAiLaunchRequest,
    ) -> TabAiDeferredCaptureRx {
        let capture_kind = request.capture_kind;
        let (tx, rx) = async_channel::bounded::<Result<TabAiDeferredCaptureArtifacts, String>>(1);

        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                // Capture desktop context (text-safe, no screenshots in the blob)
                let desktop = crate::context_snapshot::capture_context_snapshot(
                    &crate::context_snapshot::CaptureContextOptions::tab_ai_submit(),
                );

                // Best-effort screenshot-to-file capture, branching on the
                // requested capture kind so explicit AI commands get the right
                // screenshot type instead of always defaulting to focused-window.
                let screenshot_path = match capture_kind {
                    crate::ai::TabAiCaptureKind::DefaultContext
                    | crate::ai::TabAiCaptureKind::FocusedWindow => {
                        match crate::ai::harness::capture_tab_ai_focused_window_screenshot_file() {
                            Ok(Some(file)) => Some(file.path),
                            Ok(None) => None,
                            Err(error) => {
                                tracing::debug!(
                                    event = "tab_ai_deferred_screenshot_failed",
                                    capture_kind = "focused_window",
                                    error = %error,
                                );
                                None
                            }
                        }
                    }
                    crate::ai::TabAiCaptureKind::FullScreen => {
                        match crate::ai::harness::capture_tab_ai_screen_screenshot_file() {
                            Ok(Some(file)) => Some(file.path),
                            Ok(None) => None,
                            Err(error) => {
                                tracing::debug!(
                                    event = "tab_ai_deferred_screenshot_failed",
                                    capture_kind = "full_screen",
                                    error = %error,
                                );
                                None
                            }
                        }
                    }
                    // Text-only captures (selected text, browser tab) skip screenshots.
                    crate::ai::TabAiCaptureKind::SelectedText
                    | crate::ai::TabAiCaptureKind::BrowserTab => None,
                };

                TabAiDeferredCaptureArtifacts {
                    desktop,
                    screenshot_path,
                }
            })
            .map_err(|_| "tab_ai_deferred_capture_panicked".to_string());

            let send_result = match result {
                Ok(artifacts) => tx.send_blocking(Ok(artifacts)),
                Err(error) => tx.send_blocking(Err(error)),
            };
            if send_result.is_err() {
                tracing::debug!(event = "tab_ai_deferred_capture_receiver_dropped");
            }
        });

        rx
    }

    /// Open the ACP chat view immediately, then spawn a task that waits
    /// Compute the canonical submission intent for ACP launches, matching the
    /// PTY path's normalization: prefer `quick_submit_plan.submission_intent()`
    /// over raw `entry_intent`, trim whitespace, and drop empty strings.
    fn tab_ai_effective_submission_intent(request: &TabAiLaunchRequest) -> Option<String> {
        request
            .quick_submit_plan
            .as_ref()
            .map(|plan| plan.submission_intent())
            .or(request.entry_intent.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    /// Build the ACP composer text for the first render of a new launch.
    ///
    /// ScriptList-triggered `@` and `/` routes prefill the raw trigger so the
    /// ACP handoff never paints an empty composer before the picker opens.
    fn tab_ai_acp_initial_input_for_launch(
        prompt_type: &str,
        effective_intent: Option<&str>,
        pending_script_list_trigger: Option<char>,
        force_acp_surface: bool,
    ) -> Option<String> {
        if let Some(intent) = effective_intent {
            if force_acp_surface {
                return Some(intent.to_string());
            }

            return Some(
                crate::ai::harness::build_tab_ai_acp_initial_input_for_prompt(prompt_type, intent)
                    .text,
            );
        }

        match (prompt_type, pending_script_list_trigger) {
            ("ScriptList", Some(trigger @ ('/' | '@'))) => Some(trigger.to_string()),
            _ => None,
        }
    }

    /// Return a chip label for entry modes that must always use ambient capture,
    /// even if the source view has a resolvable focused target.
    fn tab_ai_explicit_ambient_chip_label(
        capture_kind: &crate::ai::TabAiCaptureKind,
    ) -> Option<&'static str> {
        match capture_kind {
            crate::ai::TabAiCaptureKind::DefaultContext => None,
            crate::ai::TabAiCaptureKind::FullScreen => Some("Full Screen"),
            crate::ai::TabAiCaptureKind::FocusedWindow => Some("Focused Window"),
            crate::ai::TabAiCaptureKind::SelectedText => Some("Selected Text"),
            crate::ai::TabAiCaptureKind::BrowserTab => Some("Browser Tab"),
        }
    }

    fn should_stage_focused_part_for_request(
        source_view: &AppView,
        pending_script_list_trigger: Option<char>,
        suppress_focused_part: bool,
    ) -> bool {
        if suppress_focused_part {
            return false;
        }

        if !matches!(source_view, AppView::ScriptList) {
            return true;
        }

        !matches!(pending_script_list_trigger, Some('/' | '@'))
    }

    /// Extract a `TabAiTargetContext` from an `AiContextPart::FocusedTarget`,
    /// returning `None` for any other variant or `None` input.
    fn tab_ai_focused_target_from_part(
        part: Option<&crate::ai::message_parts::AiContextPart>,
    ) -> Option<crate::ai::TabAiTargetContext> {
        match part {
            Some(crate::ai::message_parts::AiContextPart::FocusedTarget { target, .. }) => {
                Some(target.clone())
            }
            _ => None,
        }
    }

    /// Seed `tab_ai_harness_apply_back_route` synchronously from the source
    /// view, UI snapshot, and an optional focused part.  When `focused_part`
    /// is `AiContextPart::FocusedTarget`, the concrete target metadata is
    /// preserved in the route so downstream apply-back consumers can identify
    /// what the user's chip refers to.
    fn seed_tab_ai_apply_back_route(
        &mut self,
        source_view: &AppView,
        ui_snapshot: &crate::ai::TabAiUiSnapshot,
        focused_part: Option<&crate::ai::message_parts::AiContextPart>,
    ) {
        let early_source_type = detect_tab_ai_source_type_early(source_view, ui_snapshot);
        let focused_target = Self::tab_ai_focused_target_from_part(focused_part);

        self.tab_ai_harness_apply_back_route = early_source_type.clone().and_then(|source_type| {
            build_tab_ai_apply_back_hint(Some(&source_type)).map(|hint| {
                crate::ai::TabAiApplyBackRoute {
                    source_type,
                    hint,
                    focused_target: focused_target.clone(),
                }
            })
        });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_apply_back_route_seeded",
            source_view = match source_view {
                AppView::ScriptList => "ScriptList",
                _ => "Other",
            },
            has_early_source_type = early_source_type.is_some(),
            has_focused_target = focused_target.is_some(),
            focused_target_kind = ?focused_target.as_ref().map(|t| t.kind.as_str()),
            focused_target_source = ?focused_target.as_ref().map(|t| t.source.as_str()),
        );
    }

    /// Open the ACP chat view and stage context.
    ///
    /// When `focused_part` is `Some`, stages it as a visible chip on the
    /// composer. When `use_ask_anything_fallback` is `true`, the deferred
    /// capture result is consumed as ambient context. Otherwise, deferred
    /// capture is skipped and only the focused chip is staged.
    ///
    /// Extract a pending retry request from the current ACP chat view.
    ///
    /// Returns `None` if the current view is not an `AcpChatView` or if no
    /// retry request has been queued.
    fn take_acp_retry_request_for_open(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<crate::ai::acp::AcpRetryRequest> {
        if let AppView::AcpChatView { entity } = &self.current_view {
            return entity.update(cx, |view, _cx| view.take_retry_request());
        }

        self.embedded_acp_chat
            .as_ref()
            .cloned()
            .and_then(|entity| entity.update(cx, |view, _cx| view.take_retry_request()))
    }

    pub(crate) fn open_embedded_acp_history_popup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let AppView::AcpChatView { entity } = &self.current_view else {
            return false;
        };

        let parent_handle = window.window_handle();
        let parent_bounds = window.bounds();
        let display_id = window.display(cx).map(|display| display.id());
        entity.update(cx, |view, cx| {
            view.open_history_popup_from_host(parent_handle, parent_bounds, display_id, cx);
        });
        true
    }

    /// **Contract:** `AppView::AcpChatView` and `cx.notify()` happen
    /// *before* any deferred-capture await. The user sees the chat surface
    /// within one frame.
    fn open_tab_ai_acp_view_from_request_impl(
        &mut self,
        request: TabAiLaunchRequest,
        capture_rx: TabAiDeferredCaptureRx,
        focused_part: Option<crate::ai::message_parts::AiContextPart>,
        use_ask_anything_fallback: bool,
        explicit_ambient_chip_label: Option<String>,
        force_acp_surface: bool,
        cx: &mut Context<Self>,
    ) {
        let open_started_at = std::time::Instant::now();
        let source_view = request.source_view.clone();
        let had_harness_session = self.tab_ai_harness.is_some();
        let pending_script_list_trigger = self.tab_ai_harness_script_list_trigger;

        // Compute canonical effective intent once, matching PTY path's normalization.
        let effective_intent = Self::tab_ai_effective_submission_intent(&request);
        let auto_submit = effective_intent.is_some();

        // Build ACP initial input via the shared helper, ensuring the same
        // verification contract as the PTY submission path.
        // When force_acp_surface is set (Auto Submit fallback), use the raw
        // intent without script-authoring guidance — the query may be general.
        let acp_initial_input = effective_intent
            .clone()
            .map(|intent| {
                if force_acp_surface {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_acp_initial_input_built",
                        prompt_type = %request.ui_snapshot.prompt_type,
                        guidance_appended = false,
                        forced_by_script_list_submit = false,
                        force_acp_surface = true,
                    );
                    intent
                } else {
                    let initial_input = crate::ai::harness::build_tab_ai_acp_initial_input_for_prompt(
                        &request.ui_snapshot.prompt_type,
                        &intent,
                    );

                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_acp_initial_input_built",
                        prompt_type = %request.ui_snapshot.prompt_type,
                        guidance_appended = initial_input.guidance_appended,
                        forced_by_script_list_submit = initial_input.forced_by_script_list_submit,
                        includes_script_authoring_skill = initial_input.includes_script_authoring_skill,
                        includes_bun_build_verification = initial_input.includes_bun_build_verification,
                        includes_bun_execute_verification = initial_input.includes_bun_execute_verification,
                    );

                    initial_input.text
                }
            })
            .or_else(|| {
                Self::tab_ai_acp_initial_input_for_launch(
                    &request.ui_snapshot.prompt_type,
                    None,
                    pending_script_list_trigger,
                    force_acp_surface,
                )
            });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_begin",
            auto_submit,
            has_entry_intent = request.entry_intent.is_some(),
            had_harness_session,
            pending_script_list_trigger = ?pending_script_list_trigger,
            prefilled_len = acp_initial_input.as_ref().map(|text| text.len()).unwrap_or(0),
        );

        // --- Permission broker + ACP connection ---
        let stage_started_at = std::time::Instant::now();
        let (broker, permission_rx) = crate::ai::acp::AcpPermissionBroker::new();
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_stage",
            stage = "permission_broker_new",
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        // --- ACP agent catalog + preflight resolution ---
        let stage_started_at = std::time::Instant::now();
        let catalog = match crate::ai::acp::load_acp_agent_catalog_entries() {
            Ok(entries) => entries,
            Err(error) => {
                tracing::error!(
                    target: "script_kit::tab_ai",
                    event = "acp_catalog_load_failed",
                    error = %error,
                );
                let setup = crate::ai::acp::AcpInlineSetupState {
                    reason_code: "catalogLoadFailed",
                    title: "Failed to load ACP catalog".into(),
                    body: format!("{error}").into(),
                    primary_action: crate::ai::acp::AcpSetupAction::OpenCatalog,
                    secondary_action: Some(crate::ai::acp::AcpSetupAction::Retry),
                    selected_agent: None,
                    catalog_entries: Vec::new(),
                    launch_requirements: crate::ai::acp::AcpLaunchRequirements::default(),
                };
                let view_entity = cx.new(|cx| crate::ai::acp::AcpChatView::new_setup(setup, cx));
                self.wire_embedded_acp_footer_callbacks(&view_entity, cx);
                self.tab_ai_harness_return_view = Some(source_view.clone());
                self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
                self.current_view = AppView::AcpChatView {
                    entity: view_entity,
                };
                self.focused_input = FocusedInput::None;
                self.show_actions_popup = false;
                self.actions_dialog = None;
                self.pending_focus = Some(FocusTarget::ChatPrompt);
                cx.notify();
                return;
            }
        };

        // Check for an explicit retry payload from a setup card before
        // falling back to persisted preference and entry-path derivation.
        let retry_request = self.take_acp_retry_request_for_open(cx);

        let preferred_agent_id = retry_request
            .as_ref()
            .and_then(|req| req.preferred_agent_id.clone())
            .or_else(crate::ai::acp::load_preferred_acp_agent_id);

        let requirements = retry_request
            .as_ref()
            .map(|req| req.launch_requirements)
            .unwrap_or_else(|| {
                let has_context_parts = focused_part.is_some();
                let needs_image = focused_part
                    .as_ref()
                    .map(|part| part.source().contains("screenshot=1"))
                    .unwrap_or(false);
                crate::ai::acp::AcpLaunchRequirements {
                    needs_embedded_context: has_context_parts,
                    needs_image,
                }
            });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_retry_request_consumed",
            had_retry_request = retry_request.is_some(),
            preferred_agent_id = ?preferred_agent_id,
            needs_embedded_context = requirements.needs_embedded_context,
            needs_image = requirements.needs_image,
        );

        let acp_launch_resolution = crate::ai::acp::resolve_acp_launch_with_requirements(
            &catalog,
            preferred_agent_id.as_deref(),
            requirements,
        );
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_launch_resolution",
            preferred_agent_id = ?preferred_agent_id,
            selected_agent_id = ?acp_launch_resolution.selected_agent_id(),
            blocker = ?acp_launch_resolution.blocker,
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        if !acp_launch_resolution.is_ready() {
            let setup = crate::ai::acp::AcpInlineSetupState::from_resolution(
                &acp_launch_resolution,
                requirements,
            );
            let view_entity = cx.new(|cx| crate::ai::acp::AcpChatView::new_setup(setup, cx));
            self.wire_embedded_acp_footer_callbacks(&view_entity, cx);
            self.tab_ai_harness_return_view = Some(source_view.clone());
            self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
            self.current_view = AppView::AcpChatView {
                entity: view_entity,
            };
            self.focused_input = FocusedInput::None;
            self.show_actions_popup = false;
            self.actions_dialog = None;
            self.pending_focus = Some(FocusTarget::ChatPrompt);
            cx.notify();
            return;
        }

        let agent = match acp_launch_resolution
            .selected_agent
            .as_ref()
            .and_then(|entry| entry.config.clone())
        {
            Some(config) => config,
            None => {
                tracing::error!(
                    target: "script_kit::tab_ai",
                    event = "acp_resolution_missing_config",
                );
                cx.notify();
                return;
            }
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_stage",
            stage = "acp_agent_resolved",
            agent_id = %agent.id,
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        let agent_display_name = agent.display_name().to_string();
        // Extract model info before `agent` is moved into spawn_with_approval.
        let agent_models = agent.models.clone();
        // Use the config-backed preferred model, falling back to the first model.
        let persisted_model = crate::config::load_user_preferences().ai.selected_model_id;
        let default_model_id = persisted_model
            .filter(|id| agent_models.iter().any(|m| m.id == *id))
            .or_else(|| agent_models.first().map(|m| m.id.clone()));

        let stage_started_at = std::time::Instant::now();
        let connection = match crate::ai::acp::AcpConnection::spawn_with_approval(
            agent,
            Some(broker.approval_fn()),
        ) {
            Ok(conn) => std::sync::Arc::new(conn),
            Err(error) => {
                tracing::error!(
                    event = "tab_ai_acp_spawn_failed",
                    error = %error,
                );
                self.toast_manager.push(
                    crate::components::toast::Toast::error(
                        format!("Failed to start ACP connection: {error}"),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
                cx.notify();
                return;
            }
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_stage",
            stage = "acp_connection_spawn_with_approval",
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        // Use ~/.scriptkit as cwd so Claude Code discovers CLAUDE.md and skills/.
        let cwd = crate::setup::get_kit_path();

        let stage_started_at = std::time::Instant::now();
        let thread = cx.new(|cx| {
            crate::ai::acp::AcpThread::new(
                connection,
                permission_rx,
                crate::ai::acp::AcpThreadInit {
                    ui_thread_id: uuid::Uuid::new_v4().to_string(),
                    cwd,
                    initial_input: acp_initial_input.clone(),
                    initial_context_parts: Vec::new(),
                    display_name: agent_display_name.into(),
                    selected_agent: acp_launch_resolution.selected_agent.clone(),
                    available_agents: acp_launch_resolution.catalog_entries.clone(),
                    launch_requirements: requirements,
                    available_models: agent_models,
                    selected_model_id: default_model_id,
                },
                cx,
            )
        });
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_stage",
            stage = "acp_thread_new",
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        // Only persist the selected agent when the launch is explicit (retry
        // request / first-run) or already aligned with the saved preference.
        // Automatic capability fallback should not silently rewrite the user's
        // preferred agent.
        let selected_agent_id = acp_launch_resolution
            .selected_agent_id()
            .map(str::to_string);
        let should_persist_selected_agent = retry_request.is_some()
            || preferred_agent_id.is_none()
            || preferred_agent_id.as_deref() == selected_agent_id.as_deref();

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_preferred_agent_post_launch_persist_decision",
            had_retry_request = retry_request.is_some(),
            preferred_agent_id = ?preferred_agent_id,
            selected_agent_id = ?selected_agent_id,
            should_persist_selected_agent,
            needs_embedded_context = requirements.needs_embedded_context,
            needs_image = requirements.needs_image,
        );

        if should_persist_selected_agent {
            crate::ai::acp::persist_preferred_acp_agent_id(selected_agent_id);
        } else {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_preferred_agent_preserved_during_fallback_launch",
                preferred_agent_id = ?preferred_agent_id,
                selected_agent_id = ?acp_launch_resolution.selected_agent_id(),
            );
        }

        let stage_started_at = std::time::Instant::now();
        let view_entity = cx.new(|cx| crate::ai::acp::AcpChatView::new(thread.clone(), cx));
        self.wire_embedded_acp_footer_callbacks(&view_entity, cx);
        self.embedded_acp_chat = Some(view_entity.clone());
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_stage",
            stage = "acp_chat_view_new",
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        // Save originating surface for close/restore
        self.tab_ai_harness_return_view = Some(source_view.clone());
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());

        // Seed apply-back route synchronously so focused-chip ACP sessions
        // retain the concrete target metadata even though they skip deferred
        // context capture.
        self.seed_tab_ai_apply_back_route(
            &request.source_view,
            &request.ui_snapshot,
            focused_part.as_ref(),
        );

        // --- View switch FIRST: user sees the ACP chat surface immediately ---
        let view_entity_for_staging = view_entity.clone();
        self.current_view = AppView::AcpChatView {
            entity: view_entity,
        };
        self.focused_input = FocusedInput::None;
        self.show_actions_popup = false;
        self.actions_dialog = None;
        self.pending_focus = Some(FocusTarget::ChatPrompt);
        cx.notify();

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "acp_open_view_switched",
            elapsed_ms = open_started_at.elapsed().as_millis() as u64,
        );

        // --- Stage context part + insert inline @type:name token ---
        if let Some(part) = focused_part.clone() {
            let inline_token = crate::ai::context_mentions::part_to_inline_token(&part);
            let _ = thread.update(cx, |thread, cx| {
                if let Some(ref token) = inline_token {
                    let text = format!("{token} ");
                    thread.input.set_text(text.clone());
                    thread.input.set_cursor(text.len());
                }
                thread.add_context_part(part.clone(), cx);
            });
            if let Some(token) = inline_token {
                view_entity_for_staging.update(cx, |view, _cx| {
                    view.register_typed_alias(token.clone(), part);
                    view.register_inline_owned_token(token);
                });
            }
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_focused_chip_staged_on_thread",
                source_view = match &source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
            );
        } else if use_ask_anything_fallback && explicit_ambient_chip_label.is_none() {
            // Stage a minimal desktop context resource as the Ask Anything inline token.
            let part = crate::ai::message_parts::AiContextPart::ResourceUri {
                uri: crate::ai::message_parts::ASK_ANYTHING_RESOURCE_URI.to_string(),
                label: crate::ai::message_parts::ASK_ANYTHING_LABEL.to_string(),
            };
            // Built-in ResourceUri — uses @snapshot mention, no alias needed.
            let _ = thread.update(cx, |thread, cx| {
                thread.add_context_part(part, cx);
            });
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_ask_anything_chip_staged_on_thread",
                source_view = match &source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
            );
        } else if let Some(ref label) = explicit_ambient_chip_label {
            // Stage a labeled ambient capture inline token for explicit AI commands.
            let chip_label = label.clone();
            let part = crate::ai::message_parts::AiContextPart::ResourceUri {
                uri: crate::ai::message_parts::ASK_ANYTHING_RESOURCE_URI.to_string(),
                label: chip_label,
            };
            // Built-in ResourceUri — uses @snapshot mention, no alias needed.
            let _ = thread.update(cx, |thread, cx| {
                thread.add_context_part(part, cx);
            });
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_ambient_capture_chip_staged_on_thread",
                source_view = match &source_view {
                    AppView::ScriptList => "ScriptList",
                    _ => "Other",
                },
                chip_label = %label,
            );
        }

        // Defer harness termination to after first paint so the user sees the
        // chat surface before the synchronous teardown blocks the main thread.
        if had_harness_session {
            let app_weak_for_teardown = cx.entity().downgrade();
            let open_started_at_for_teardown = open_started_at;
            cx.spawn(async move |_this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let _ = cx.update(|cx| {
                    let Some(app) = app_weak_for_teardown.upgrade() else {
                        return;
                    };
                    app.update(cx, |this, cx| {
                        let stage_started_at = std::time::Instant::now();
                        this.terminate_tab_ai_harness_session(cx);
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "acp_open_stage",
                            stage = "terminate_tab_ai_harness_session_post_paint",
                            stage_ms = stage_started_at.elapsed().as_millis() as u64,
                            total_ms = open_started_at_for_teardown.elapsed().as_millis() as u64,
                        );
                    });
                });
            })
            .detach();
        }

        // --- Focused-target path: mark bootstrap ready immediately ---
        // No deferred capture needed; the chip is already staged.
        if !use_ask_anything_fallback && explicit_ambient_chip_label.is_none() {
            let _ = thread.update(cx, |thread, cx| {
                thread.mark_context_bootstrap_ready(cx);
                // Auto-submit if effective intent was resolved (Shift+Tab path)
                if auto_submit {
                    if let Err(e) = thread.submit_input(cx) {
                        tracing::warn!(
                            event = "tab_ai_acp_focused_auto_submit_failed",
                            error = %e,
                        );
                    }
                }
            });
            return;
        }

        // --- Ask Anything fallback: spawn deferred context injection task ---
        let app_weak = cx.entity().downgrade();
        let thread_weak = thread.downgrade();
        let capture_gen = request.capture_generation;
        let effective_intent_for_capture = effective_intent.clone();

        let first_paint_delay_ms = Self::ACP_CONTEXT_FIRST_PAINT_DELAY_MS;
        let first_paint_delay = std::time::Duration::from_millis(first_paint_delay_ms);

        cx.spawn(async move |_this, cx| {
            // Let the ACP chat render once before any heavy context staging runs.
            cx.background_executor().timer(first_paint_delay).await;
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_context_stage",
                stage = "first_paint_gate_released",
                stage_ms = first_paint_delay_ms,
                total_ms = open_started_at.elapsed().as_millis() as u64,
            );

            // Wait for deferred capture
            let capture_wait_started_at = std::time::Instant::now();
            let capture_result = match capture_rx.recv().await {
                Ok(result) => result,
                Err(_) => Err("deferred capture channel closed".to_string()),
            };
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "acp_context_stage",
                stage = "capture_rx_recv",
                stage_ms = capture_wait_started_at.elapsed().as_millis() as u64,
                total_ms = open_started_at.elapsed().as_millis() as u64,
            );

            let artifacts = match capture_result {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(
                        event = "tab_ai_acp_deferred_capture_failed",
                        error = %e,
                    );
                    TabAiDeferredCaptureArtifacts::default()
                }
            };

            // Apply the captured context
            let _ = cx.update(|cx| {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                let Some(thread_entity) = thread_weak.upgrade() else {
                    return;
                };

                app.update(cx, |this, cx| {
                    // Stale generation check
                    if this.tab_ai_harness_capture_generation != capture_gen {
                        tracing::debug!(
                            event = "tab_ai_acp_deferred_capture_stale",
                            expected = capture_gen,
                            current = this.tab_ai_harness_capture_generation,
                        );
                        return;
                    }

                    let context_stage_started_at = std::time::Instant::now();
                    let resolved = this.build_tab_ai_context_from(
                        effective_intent_for_capture.clone().unwrap_or_default(),
                        request.source_view.clone(),
                        request.ui_snapshot.clone(),
                        artifacts.desktop,
                        request.invocation_receipt.clone(),
                        cx,
                    );
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "acp_context_stage",
                        stage = "build_tab_ai_context_from",
                        stage_ms = context_stage_started_at.elapsed().as_millis() as u64,
                        total_ms = open_started_at.elapsed().as_millis() as u64,
                    );

                    let source_type = detect_tab_ai_source_type(
                        &request.source_view,
                        &resolved.context.desktop,
                        resolved.context.focused_target.as_ref(),
                    );
                    let apply_back_hint = build_tab_ai_apply_back_hint(source_type.as_ref());

                    // Persist the apply-back route
                    this.tab_ai_harness_apply_back_route = source_type
                        .clone()
                        .zip(apply_back_hint.clone())
                        .map(|(source_type, hint)| crate::ai::TabAiApplyBackRoute {
                            source_type,
                            hint,
                            focused_target: resolved.context.focused_target.clone(),
                        });

                    let context = resolved.context.with_deferred_capture_fields(
                        source_type,
                        artifacts.screenshot_path,
                        apply_back_hint,
                    );

                    // Stage context on the AcpThread
                    let stage_context_started_at = std::time::Instant::now();
                    let _ = thread_entity.update(cx, |thread, cx| {
                        if let Err(e) = thread.stage_ask_anything_context(&context, cx) {
                            tracing::warn!(
                                event = "tab_ai_acp_stage_context_failed",
                                error = %e,
                            );
                            thread.mark_context_bootstrap_failed(
                                "Some desktop context could not be attached. You can still send.",
                                cx,
                            );
                        }

                        // Auto-submit if effective intent was resolved
                        // (Shift+Tab path or quick-submit plan)
                        if auto_submit {
                            if let Err(e) = thread.submit_input(cx) {
                                tracing::warn!(
                                    event = "tab_ai_acp_auto_submit_failed",
                                    error = %e,
                                );
                            }
                        }
                    });
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "acp_context_stage",
                        stage = "thread_stage_context",
                        stage_ms = stage_context_started_at.elapsed().as_millis() as u64,
                        total_ms = open_started_at.elapsed().as_millis() as u64,
                    );
                });
            });
        })
        .detach();
    }

    /// Open the harness terminal immediately, then spawn a task that waits
    /// for the deferred capture result and injects the enriched context.
    ///
    /// **Contract:** `AppView::QuickTerminalView` and `cx.notify()` happen
    /// *before* any deferred-capture await. The user sees the terminal cursor
    /// within one frame.
    fn open_tab_ai_harness_terminal_from_request(
        &mut self,
        request: TabAiLaunchRequest,
        capture_rx: TabAiDeferredCaptureRx,
        cx: &mut Context<Self>,
    ) {
        // Reuse a silently prewarmed session exactly once; otherwise force fresh.
        let reuse_fresh_prewarm = self
            .tab_ai_harness
            .as_ref()
            .map(|session| session.is_fresh_prewarm() && session.entity.read(cx).is_alive())
            .unwrap_or(false);

        if reuse_fresh_prewarm {
            if let Some(session) = self.tab_ai_harness.as_mut() {
                session.mark_consumed();
            }
        }

        let (entity, _was_cold_start) = match self
            .ensure_tab_ai_harness_terminal(!reuse_fresh_prewarm, cx)
        {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(
                    event = "tab_ai_harness_start_failed",
                    error = %error,
                );
                self.toast_manager.push(
                        crate::components::toast::Toast::error(
                            format!("Failed to start harness: {error}. Install the configured CLI or update claudeCode.path in ~/.scriptkit/kit/config.ts"),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                cx.notify();
                return;
            }
        };

        // Determine readiness based on actual PTY output, not cold-start flag.
        let wait_for_readiness = Self::tab_ai_harness_needs_readiness_wait(&entity, cx);

        tracing::debug!(
            event = "tab_ai_harness_submission_planned",
            wait_for_readiness,
            has_entry_intent = request.entry_intent.is_some(),
        );

        // Save the originating surface so Escape and re-entry can use it
        self.tab_ai_harness_return_view = Some(request.source_view.clone());
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());

        // Seed apply-back route synchronously so the footer shows the correct
        // Cmd+Enter label on first render and the first ⌘↩ press works without
        // waiting for the deferred capture.  PTY path has no focused part.
        self.seed_tab_ai_apply_back_route(&request.source_view, &request.ui_snapshot, None);

        // --- View switch FIRST: user sees the terminal immediately ---
        self.current_view = AppView::QuickTerminalView {
            entity: entity.clone(),
        };
        self.focused_input = FocusedInput::None;
        self.show_actions_popup = false;
        self.actions_dialog = None;
        self.pending_focus = Some(FocusTarget::TermPrompt);

        // Deferred resize to avoid RefCell borrow error
        cx.spawn(async move |_this, _cx| {
            resize_to_view_sync(ViewType::TermPrompt, 0);
        })
        .detach();
        cx.notify();

        // --- Spawn deferred context injection task ---
        // This waits for the background capture, builds the full context blob
        // with source type / screenshot / apply-back hint, then injects.
        let app_weak = cx.entity().downgrade();
        let capture_gen = request.capture_generation;

        cx.spawn(async move |_this, cx| {
            // Wait for deferred capture to complete
            let capture_result = match capture_rx.recv().await {
                Ok(result) => result,
                Err(_) => Err("deferred capture channel closed".to_string()),
            };

            let artifacts = match capture_result {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(
                        event = "tab_ai_deferred_capture_failed",
                        error = %e,
                    );
                    TabAiDeferredCaptureArtifacts::default()
                }
            };

            // Apply the captured context
            let _ = cx.update(|cx| {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                app.update(cx, |this, cx| {
                    // Stale generation check
                    if this.tab_ai_harness_capture_generation != capture_gen {
                        tracing::debug!(
                            event = "tab_ai_deferred_capture_stale",
                            expected = capture_gen,
                            current = this.tab_ai_harness_capture_generation,
                        );
                        return;
                    }

                    let resolved = this.build_tab_ai_context_from(
                        request.entry_intent.clone().unwrap_or_default(),
                        request.source_view.clone(),
                        request.ui_snapshot.clone(),
                        artifacts.desktop,
                        request.invocation_receipt.clone(),
                        cx,
                    );

                    let source_type = detect_tab_ai_source_type(
                        &request.source_view,
                        &resolved.context.desktop,
                        resolved.context.focused_target.as_ref(),
                    );
                    let apply_back_hint = build_tab_ai_apply_back_hint(source_type.as_ref());

                    // Persist the apply-back route so ⌘⏎ can execute it later.
                    // Carry the focused target metadata so apply-back can route
                    // results without rediscovering UI state after the harness closes.
                    this.tab_ai_harness_apply_back_route = source_type
                        .clone()
                        .zip(apply_back_hint.clone())
                        .map(|(source_type, hint)| crate::ai::TabAiApplyBackRoute {
                            source_type,
                            hint,
                            focused_target: resolved.context.focused_target.clone(),
                        });

                    let context = resolved.context.with_deferred_capture_fields(
                        source_type,
                        artifacts.screenshot_path,
                        apply_back_hint,
                    );

                    let effective_intent = request
                        .quick_submit_plan
                        .as_ref()
                        .map(|plan| plan.submission_intent())
                        .or(request.entry_intent.as_deref());

                    let submit_now = request
                        .quick_submit_plan
                        .as_ref()
                        .map(|plan| plan.submit)
                        .unwrap_or(request.entry_intent.is_some());

                    let submission_mode = if submit_now {
                        crate::ai::TabAiHarnessSubmissionMode::Submit
                    } else {
                        crate::ai::TabAiHarnessSubmissionMode::PasteOnly
                    };

                    match crate::ai::build_tab_ai_harness_submission(
                        &context,
                        effective_intent,
                        submission_mode,
                        request.quick_submit_plan.as_ref(),
                        Some(&resolved.invocation_receipt),
                        &resolved.suggested_intents,
                    ) {
                        Ok(submission) => {
                            this.inject_tab_ai_harness_submission(
                                entity.clone(),
                                submission,
                                wait_for_readiness,
                                submit_now,
                                cx,
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "tab_ai_harness_context_build_failed",
                                error = %error,
                            );
                            this.toast_manager.push(
                                crate::components::toast::Toast::error(
                                    format!("Failed to build harness context: {error}"),
                                    &this.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        }
                    }
                });
            });
        })
        .detach();
    }

    /// Prewarm the configured harness at app startup so the first Tab press
    /// reuses a live PTY instead of paying spawn cost.
    ///
    /// This must be silent: no view switch, no focus change, no toast.
    /// User-facing errors still belong to the explicit Tab path.
    ///
    /// When `respect_startup_opt_out` is `true`, the `warmOnStartup` config
    /// flag is honoured — set for startup prewarm only.  The post-close
    /// reseed path passes `false` so the next Tab always feels instant even
    /// when startup warming is disabled.
    fn warm_tab_ai_harness_silently(
        &mut self,
        respect_startup_opt_out: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(existing) = &self.tab_ai_harness {
            if existing.entity.read(cx).is_alive() {
                tracing::debug!(
                    event = "tab_ai_harness_prewarm_skipped",
                    reason = "already_alive",
                );
                return;
            }
        }

        let config = match crate::ai::read_tab_ai_harness_config() {
            Ok(config) => config,
            Err(error) => {
                tracing::debug!(
                    event = "tab_ai_harness_prewarm_skipped",
                    reason = "config_read_failed",
                    error = %error,
                );
                return;
            }
        };

        if respect_startup_opt_out && !config.warm_on_startup {
            tracing::debug!(
                event = "tab_ai_harness_prewarm_skipped",
                reason = "disabled",
            );
            return;
        }

        if let Err(error) = crate::ai::validate_tab_ai_harness_config(&config) {
            tracing::debug!(
                event = "tab_ai_harness_prewarm_skipped",
                reason = "invalid_config",
                error = %error,
            );
            return;
        }

        match self.ensure_tab_ai_harness_terminal(false, cx) {
            Ok((_entity, was_cold_start)) => {
                // Tag newly created sessions as FreshPrewarm so the next Tab
                // reuses them exactly once instead of immediately killing them.
                if was_cold_start {
                    if let Some(session) = self.tab_ai_harness.as_mut() {
                        session.mark_fresh_prewarm();
                    }
                }
                tracing::info!(
                    event = "tab_ai_harness_prewarmed",
                    backend = ?config.backend,
                    command = %config.command,
                    was_cold_start,
                    source = if respect_startup_opt_out { "startup" } else { "post_close" },
                );
            }
            Err(error) => {
                tracing::warn!(
                    event = "tab_ai_harness_prewarm_failed",
                    error = %error,
                );
            }
        }
    }

    /// Startup prewarm: respects `warmOnStartup=false`.
    pub(crate) fn warm_tab_ai_harness_on_startup(&mut self, cx: &mut Context<Self>) {
        self.warm_tab_ai_harness_silently(true, cx);
    }

    /// Post-close prewarm: bypasses the startup opt-out so the next Tab
    /// always gets an instant fresh session after a close cycle.
    fn warm_tab_ai_harness_after_close(&mut self, cx: &mut Context<Self>) {
        self.warm_tab_ai_harness_silently(false, cx);
    }

    /// Ensure a harness terminal session exists and is alive.
    /// Returns the entity and whether this was a cold start (newly created).
    ///
    /// When `force_fresh` is `true`, any existing alive session is terminated
    /// first so the caller gets a brand-new PTY.  The startup and post-close
    /// prewarm paths pass `false` to seed a reusable session; the first
    /// explicit Tab press may reuse that `FreshPrewarm` once, and later
    /// explicit opens force a clean PTY again.
    fn ensure_tab_ai_harness_terminal(
        &mut self,
        force_fresh: bool,
        cx: &mut Context<Self>,
    ) -> Result<(gpui::Entity<crate::term_prompt::TermPrompt>, bool), String> {
        if force_fresh {
            // Kill the existing session so the user gets a clean slate.
            // Terminate FIRST, then clear the handle — if termination fails
            // the handle stays in app state so we don't lose track of a live PTY.
            if let Some(existing) = self.tab_ai_harness.as_ref() {
                existing.entity.update(cx, |term, _cx| {
                    term.terminate_session().map_err(|e| e.to_string())
                })?;
            }
            if self.tab_ai_harness.is_some() {
                self.tab_ai_harness = None;
                tracing::info!(event = "tab_ai_harness_old_session_terminated");
            }
        } else {
            // Reuse existing session if alive (prewarm path)
            if let Some(existing) = &self.tab_ai_harness {
                if existing.entity.read(cx).is_alive() {
                    return Ok((existing.entity.clone(), false));
                }
            }
        }

        let config = crate::ai::read_tab_ai_harness_config()?;
        crate::ai::validate_tab_ai_harness_config(&config)?;

        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(move |_id: String, _value: Option<String>| {});

        let term_height = crate::window_resize::layout::MAX_HEIGHT
            - gpui::px(crate::window_resize::layout::FOOTER_HEIGHT);

        let mut prompt = crate::term_prompt::TermPrompt::with_height(
            "tab-ai-harness".to_string(),
            Some(config.command_line()),
            self.focus_handle.clone(),
            submit_callback,
            std::sync::Arc::clone(&self.theme),
            std::sync::Arc::new(self.config.clone()),
            Some(term_height),
        )
        .map_err(|e| format!("tab_ai_harness_terminal_create_failed: {e}"))?;

        // Let the smart routing in scroll_to_pty() decide: when the TUI
        // enables mouse mode (Claude Code, etc.), scroll events are forwarded
        // as escape sequences so the TUI can handle scrolling internally.
        // When mouse mode is off, scroll falls back to local display buffer.
        prompt.prefer_buffer_scroll_on_wheel = false;

        let entity = cx.new(|_| prompt);

        tracing::info!(
            event = "tab_ai_harness_terminal_created",
            backend = ?config.backend,
            command = %config.command,
        );

        self.tab_ai_harness = Some(crate::ai::TabAiHarnessSessionState::new(
            config,
            entity.clone(),
            "tab-ai-harness",
        ));

        Ok((entity, true))
    }

    /// Maximum time (ms) to wait for a cold-started harness to produce its
    /// first output before injecting context anyway.
    const HARNESS_READINESS_TIMEOUT_MS: u64 = 2000;

    /// Interval (ms) between readiness polls during cold-start wait.
    const HARNESS_READINESS_POLL_MS: u64 = 20;

    /// Check whether a harness session needs to wait for prompt readiness
    /// before context paste.  Returns `true` when the PTY has not yet
    /// produced any output — regardless of whether it was cold-started or
    /// prewarmed.
    fn tab_ai_harness_needs_readiness_wait(
        entity: &gpui::Entity<crate::term_prompt::TermPrompt>,
        cx: &Context<Self>,
    ) -> bool {
        !entity.read(cx).has_received_output
    }

    /// Inject the context submission into the harness PTY, with a readiness
    /// gate that fires whenever the PTY has not yet produced output.
    ///
    /// Polls `has_received_output` on the `TermPrompt` entity up to
    /// [`HARNESS_READINESS_TIMEOUT_MS`] before injecting.  Falls back
    /// deterministically if the harness does not produce output in time.
    /// This applies to both cold-started and prewarmed sessions that have
    /// not yet rendered their first prompt.
    ///
    /// When `submit` is true, the payload is sent as a full line (appends CR).
    /// When false, the payload is pasted without a trailing CR so the user
    /// can type their intent before pressing Enter.
    fn inject_tab_ai_harness_submission(
        &self,
        entity: gpui::Entity<crate::term_prompt::TermPrompt>,
        submission: String,
        wait_for_readiness: bool,
        submit: bool,
        cx: &mut Context<Self>,
    ) {
        let app = cx.entity().downgrade();
        let entity_weak = entity.downgrade();

        cx.spawn(async move |_this, cx| {
            // Wait until the harness has produced output (its prompt/banner),
            // with a bounded timeout as fallback.  This fires for both
            // cold-started and prewarmed sessions that are not yet ready.
            if wait_for_readiness {
                let poll_interval =
                    std::time::Duration::from_millis(Self::HARNESS_READINESS_POLL_MS);
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_millis(Self::HARNESS_READINESS_TIMEOUT_MS);

                loop {
                    let is_ready = cx.update(|cx| {
                        entity_weak
                            .upgrade()
                            .map(|e| e.read(cx).has_received_output)
                            .unwrap_or(true) // entity gone → skip waiting
                    });

                    if is_ready {
                        tracing::debug!(
                            event = "tab_ai_harness_readiness_detected",
                            elapsed_ms = %std::time::Instant::now()
                                .duration_since(deadline - std::time::Duration::from_millis(Self::HARNESS_READINESS_TIMEOUT_MS))
                                .as_millis(),
                        );
                        break;
                    }
                    if std::time::Instant::now() >= deadline {
                        tracing::warn!(
                            event = "tab_ai_harness_readiness_timeout",
                            timeout_ms = Self::HARNESS_READINESS_TIMEOUT_MS,
                        );
                        break;
                    }
                    cx.background_executor().timer(poll_interval).await;
                }
            }

            let _ = cx.update(|cx| {
                let Some(entity) = entity_weak.upgrade() else {
                    return;
                };
                let result = entity.update(cx, |term, _cx| {
                    if submit {
                        term.send_line(&submission).map_err(|e| e.to_string())
                    } else {
                        term.send_text_as_paste(&submission)
                            .map_err(|e| e.to_string())
                    }
                });
                if let Err(error) = result {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| {
                            this.toast_manager.push(
                                crate::components::toast::Toast::error(
                                    format!("Failed to inject Tab AI context: {error}"),
                                    &this.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                            cx.notify();
                        });
                    }
                }
            });
        })
        .detach();
    }

    /// Terminate any active PTY harness session without restoring views.
    ///
    /// Used at ACP open to kill a stale prewarm session, and by close to
    /// tear down the PTY for `QuickTerminalView`.
    fn terminate_tab_ai_harness_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.tab_ai_harness.as_ref() {
            let result = session.entity.update(cx, |term, _cx| {
                term.terminate_session().map_err(|e| e.to_string())
            });
            match result {
                Ok(()) => {
                    self.tab_ai_harness = None;
                }
                Err(error) => {
                    tracing::warn!(
                        event = "tab_ai_harness_terminal_kill_failed",
                        error = %error,
                    );
                }
            }
        }
    }

    /// Close the Tab AI harness terminal and restore the previous view + focus.
    ///
    /// **Close semantics contract:**
    /// - `Cmd+W` closes the wrapper (handled in `render_prompts/term.rs`).
    /// - Plain `Escape` is forwarded to the PTY so the harness TUI can handle it.
    /// - The footer hint strip advertises only "⌘W Close".
    ///
    /// **Lifecycle contract:**
    /// - For `QuickTerminalView`: tears down PTY, clears harness, schedules prewarm.
    /// - For `AcpChatView`: restores view/focus without touching PTY lifecycle.
    fn clear_transient_script_list_trigger_on_return(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            self.tab_ai_harness_script_list_trigger = None;
            return;
        }

        let Some(trigger) = self.tab_ai_harness_script_list_trigger.take() else {
            return;
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "script_list_transient_trigger_cleared",
            trigger = %trigger,
            via_window = window.is_some(),
        );

        if let Some(window) = window {
            self.clear_filter(window, cx);
        } else {
            self.reset_script_list_filter_and_selection_state(cx);
        }
    }

    fn close_tab_ai_harness_terminal_impl(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let return_is_script_list = self
            .tab_ai_harness_return_view
            .as_ref()
            .map_or(true, |v| matches!(v, AppView::ScriptList));
        let pending_script_list_trigger = self.tab_ai_harness_script_list_trigger;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_close_with_window_decision",
            return_is_script_list,
            has_pending_script_list_trigger = pending_script_list_trigger.is_some(),
            pending_script_list_trigger = ?pending_script_list_trigger,
            filter_text = %self.filter_text,
            current_view = ?self.current_view,
        );
        let closing_quick_terminal = matches!(self.current_view, AppView::QuickTerminalView { .. });
        let closing_acp_chat = matches!(self.current_view, AppView::AcpChatView { .. });

        if !closing_quick_terminal && !closing_acp_chat {
            return;
        }

        // Invalidate any in-flight deferred capture so late results cannot
        // inject stale context after the surface has been closed.
        self.tab_ai_harness_capture_generation += 1;

        // Clear the apply-back route so stale targets cannot leak across sessions.
        self.tab_ai_harness_apply_back_route = None;

        // Only the legacy PTY-backed quick terminal owns `self.tab_ai_harness`.
        if closing_quick_terminal {
            self.terminate_tab_ai_harness_session(cx);
        }
        if closing_acp_chat {
            if let AppView::AcpChatView { entity } = &self.current_view {
                self.embedded_acp_chat = Some(entity.clone());
                entity.update(cx, |view, cx| {
                    view.prepare_for_host_hide(cx);
                });
            }
        }

        let return_view = self
            .tab_ai_harness_return_view
            .take()
            .unwrap_or(AppView::ScriptList);
        let return_focus_target = self
            .tab_ai_harness_return_focus_target
            .take()
            .unwrap_or(FocusTarget::MainFilter);

        tracing::info!(
            event = "tab_ai_harness_terminal_close",
            focus_target = %format!("{return_focus_target:?}"),
            session_cleared = closing_quick_terminal,
            capture_generation = self.tab_ai_harness_capture_generation,
        );

        self.current_view = return_view;
        self.pending_focus = Some(return_focus_target);
        self.focused_input = match return_focus_target {
            FocusTarget::MainFilter => FocusedInput::MainFilter,
            FocusTarget::ActionsDialog => FocusedInput::ActionsSearch,
            _ => FocusedInput::None,
        };

        if return_is_script_list {
            self.clear_transient_script_list_trigger_on_return(window, cx);
        } else {
            self.tab_ai_harness_script_list_trigger = None;
        }

        // Keep prewarm only for the actual PTY-backed quick terminal path.
        if closing_quick_terminal {
            self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);
        }
        cx.notify();
    }

    pub(crate) fn close_tab_ai_harness_terminal_with_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_tab_ai_harness_terminal_impl(Some(window), cx);
    }

    pub(crate) fn close_tab_ai_harness_terminal(&mut self, cx: &mut Context<Self>) {
        // close_tab_ai_harness_terminal_impl(None, cx) owns the real contract:
        // - it guards the restore path to AppView::QuickTerminalView / AppView::AcpChatView
        // - tab_ai_harness_capture_generation += 1
        // - self.tab_ai_harness_apply_back_route = None;
        // - self.terminate_tab_ai_harness_session(cx);
        // - self.current_view = return_view;
        // - self.pending_focus = Some(return_focus_target);
        // - the close log records session_cleared = closing_quick_terminal
        // - it requeues schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx)
        // - it ends with cx.notify()
        self.close_tab_ai_harness_terminal_impl(None, cx);
    }

    /// Close ACP chat and force the main panel back to ScriptList.
    ///
    /// Used by the hotkey detach path where the intent is always to return
    /// to the launcher, regardless of which view opened the ACP chat.
    /// This avoids the correctness bug where `close_tab_ai_harness_terminal`
    /// would restore an unrelated originating view (e.g. ClipboardHistory).
    ///
    /// When `focus_main_filter` is `false`, the main panel switches to
    /// ScriptList but does not reclaim keyboard focus — this keeps the
    /// newly-detached chat window as the active key target.
    pub(crate) fn close_acp_chat_to_script_list(
        &mut self,
        focus_main_filter: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::AcpChatView { .. }) {
            return;
        }

        // Invalidate any in-flight deferred capture so late results cannot
        // target a chat surface that has already been detached.
        self.tab_ai_harness_capture_generation += 1;
        self.tab_ai_harness_apply_back_route = None;
        if let AppView::AcpChatView { entity } = &self.current_view {
            self.embedded_acp_chat = Some(entity.clone());
            entity.update(cx, |view, cx| {
                view.prepare_for_host_hide(cx);
            });
        }

        // A hotkey detach is a deliberate mode switch back to the launcher,
        // not a normal "return to the originating surface" close.
        self.tab_ai_harness_return_view = None;
        self.tab_ai_harness_return_focus_target = None;

        self.current_view = AppView::ScriptList;
        self.pending_focus = if focus_main_filter {
            Some(FocusTarget::MainFilter)
        } else {
            None
        };
        self.focused_input = if focus_main_filter {
            FocusedInput::MainFilter
        } else {
            FocusedInput::None
        };
        self.clear_transient_script_list_trigger_on_return(None, cx);

        tracing::info!(
            event = "acp_chat_restored_to_script_list",
            capture_generation = self.tab_ai_harness_capture_generation,
            focus_main_filter,
        );
        cx.notify();
    }

    /// Schedule a deferred prewarm of the Tab AI harness so the next Tab
    /// press gets an instant fresh session after a close cycle.
    fn schedule_tab_ai_harness_prewarm(
        &mut self,
        delay: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        let app_weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = cx.update(|cx| {
                if let Some(app) = app_weak.upgrade() {
                    app.update(cx, |this, cx| {
                        this.warm_tab_ai_harness_after_close(cx);
                    });
                }
            });
        })
        .detach();
    }

    /// Build context from explicit inputs, resolving targets and clipboard
    /// against the provided `source_view` (the view that was active when Tab
    /// was pressed) rather than `self.current_view` (which may now differ).
    fn build_tab_ai_context_from(
        &self,
        intent_for_lookup: String,
        source_view: AppView,
        ui: crate::ai::TabAiUiSnapshot,
        desktop: crate::context_snapshot::AiContextSnapshot,
        invocation_receipt: crate::ai::TabAiInvocationReceipt,
        _cx: &Context<Self>,
    ) -> TabAiResolvedContext {
        let bundle_id = desktop
            .frontmost_app
            .as_ref()
            .map(|app| app.bundle_id.clone());
        let recent_inputs = self.input_history.recent_entries(5);
        let clipboard_selected_index = match &source_view {
            AppView::ClipboardHistoryView { selected_index, .. } => Some(*selected_index),
            _ => None,
        };
        let clipboard_history = self.resolve_tab_ai_clipboard_history(
            clipboard_selected_index,
            TAB_AI_CLIPBOARD_HISTORY_LIMIT,
        );
        let clipboard = clipboard_history
            .first()
            .map(|entry| crate::ai::TabAiClipboardContext {
                content_type: entry.content_type.clone(),
                preview: entry.preview.clone(),
                ocr_text: entry.ocr_text.clone(),
            });
        let prior_automations = match crate::ai::resolve_tab_ai_prior_automations_for_entry(
            &intent_for_lookup,
            bundle_id.as_deref(),
            3,
        ) {
            Ok(items) => items,
            Err(error) => {
                tracing::warn!(event = "tab_ai_prior_automation_lookup_failed", error = %error);
                Vec::new()
            }
        };
        let (focused_target, visible_targets) =
            self.resolve_tab_ai_targets_with_audit_for_view(&source_view, &ui, "submit_context");

        let suggested_intents = crate::ai::build_tab_ai_suggested_intents(
            focused_target.as_ref(),
            clipboard.as_ref(),
            &prior_automations,
        );

        let context = crate::ai::TabAiContextBlob::from_parts_with_targets(
            ui,
            focused_target,
            visible_targets,
            desktop,
            recent_inputs,
            clipboard,
            clipboard_history,
            prior_automations,
            chrono::Utc::now().to_rfc3339(),
        );

        TabAiResolvedContext {
            context,
            invocation_receipt,
            suggested_intents,
        }
    }

    /// Return the correct `FocusTarget` for the originating surface so that
    /// closing the Tab AI overlay restores focus to the right place.
    fn tab_ai_return_focus_target(&self) -> FocusTarget {
        match &self.current_view {
            AppView::ScriptList
            | AppView::ClipboardHistoryView { .. }
            | AppView::AppLauncherView { .. }
            | AppView::WindowSwitcherView { .. }
            | AppView::FileSearchView { .. }
            | AppView::ThemeChooserView { .. }
            | AppView::EmojiPickerView { .. }
            | AppView::BrowseKitsView { .. }
            | AppView::InstalledKitsView { .. }
            | AppView::ProcessManagerView { .. }
            | AppView::SearchAiPresetsView { .. }
            | AppView::CreateAiPresetView { .. }
            | AppView::SettingsView { .. }
            | AppView::FavoritesBrowseView { .. }
            | AppView::AcpHistoryView { .. }
            | AppView::NotesBrowseView { .. }
            | AppView::CurrentAppCommandsView { .. }
            | AppView::DesignGalleryView { .. }
            | AppView::CreationFeedback { .. }
            | AppView::ActionsDialog => FocusTarget::MainFilter,

            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. }
            | AppView::DivPrompt { .. }
            | AppView::WebcamView { .. } => FocusTarget::AppRoot,

            AppView::FormPrompt { .. } => FocusTarget::FormPrompt,

            AppView::EditorPrompt { .. } | AppView::ScratchPadView { .. } => {
                FocusTarget::EditorPrompt
            }

            AppView::SelectPrompt { .. } => FocusTarget::SelectPrompt,
            AppView::PathPrompt { .. } => FocusTarget::PathPrompt,
            AppView::EnvPrompt { .. } => FocusTarget::EnvPrompt,
            AppView::DropPrompt { .. } => FocusTarget::DropPrompt,
            AppView::TemplatePrompt { .. } => FocusTarget::TemplatePrompt,

            AppView::TermPrompt { .. } | AppView::QuickTerminalView { .. } => {
                FocusTarget::TermPrompt
            }

            AppView::ChatPrompt { .. } | AppView::AcpChatView { .. } => FocusTarget::ChatPrompt,
            AppView::NamingPrompt { .. } => FocusTarget::NamingPrompt,

            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => FocusTarget::AppRoot,
        }
    }

    /// Resolve a bounded clipboard history with hydrated content from the
    /// clipboard database. The selected entry (if any) is always first,
    /// followed by other recent entries in order, deduplicated by content.
    fn resolve_tab_ai_clipboard_history(
        &self,
        selected_index: Option<usize>,
        limit: usize,
    ) -> Vec<crate::ai::TabAiClipboardHistoryEntry> {
        let mut ordered: Vec<&crate::clipboard_history::ClipboardEntryMeta> = Vec::new();

        // Selected entry goes first
        if let Some(selected) =
            selected_index.and_then(|index| self.cached_clipboard_entries.get(index))
        {
            ordered.push(selected);
        }

        for entry in &self.cached_clipboard_entries {
            if ordered.iter().any(|candidate| candidate.id == entry.id) {
                continue;
            }
            ordered.push(entry);
            if ordered.len() >= limit {
                break;
            }
        }

        let mut last_dedupe_key: Option<String> = None;
        let mut result = Vec::new();

        for entry in ordered {
            // Hydrate text content from the database for text-like entries
            let full_text = match entry.content_type {
                crate::clipboard_history::ContentType::Text
                | crate::clipboard_history::ContentType::Link
                | crate::clipboard_history::ContentType::File
                | crate::clipboard_history::ContentType::Color => {
                    crate::clipboard_history::get_entry_content(&entry.id)
                        .filter(|content| !content.trim().is_empty())
                        .map(|content| {
                            crate::ai::truncate_tab_ai_text(&content, TAB_AI_CLIPBOARD_TEXT_LIMIT)
                        })
                }
                crate::clipboard_history::ContentType::Image => None,
            };

            let preview_source = full_text
                .clone()
                .or_else(|| entry.ocr_text.clone())
                .unwrap_or_else(|| entry.display_preview());

            // Deduplicate consecutive identical entries
            let dedupe_key = format!("{}::{}", entry.content_type.as_str(), preview_source);
            if last_dedupe_key.as_deref() == Some(dedupe_key.as_str()) {
                continue;
            }
            last_dedupe_key = Some(dedupe_key);

            result.push(crate::ai::TabAiClipboardHistoryEntry {
                id: entry.id.clone(),
                content_type: entry.content_type.as_str().to_string(),
                timestamp: entry.timestamp,
                preview: crate::ai::truncate_tab_ai_text(
                    &preview_source,
                    TAB_AI_CLIPBOARD_TEXT_LIMIT,
                ),
                full_text,
                ocr_text: entry
                    .ocr_text
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| {
                        crate::ai::truncate_tab_ai_text(&text, TAB_AI_CLIPBOARD_TEXT_LIMIT)
                    }),
                image_width: entry.image_width,
                image_height: entry.image_height,
            });
        }

        result
    }

    fn tab_ai_target_from_element(
        prompt_type: &str,
        element: &crate::protocol::ElementInfo,
    ) -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: prompt_type.to_string(),
            kind: format!("{:?}", element.element_type).to_lowercase(),
            semantic_id: element.semantic_id.clone(),
            label: element
                .text
                .clone()
                .or_else(|| element.value.clone())
                .unwrap_or_else(|| element.semantic_id.clone()),
            metadata: Some(serde_json::json!({
                "text": element.text.clone(),
                "value": element.value.clone(),
                "selected": element.selected,
                "focused": element.focused,
                "index": element.index,
            })),
        }
    }

    /// Convert a `SearchResult` from the Script List into a `TabAiTargetContext`
    /// with script-native metadata (name, path, description, type).
    pub(crate) fn tab_ai_target_from_search_result(
        index: usize,
        result: &scripts::SearchResult,
    ) -> crate::ai::TabAiTargetContext {
        let name = result.name().to_string();
        let kind = match result {
            scripts::SearchResult::Script(_) => "script",
            scripts::SearchResult::Scriptlet(_) => "scriptlet",
            scripts::SearchResult::BuiltIn(_) => "builtin",
            scripts::SearchResult::App(_) => "app",
            scripts::SearchResult::Window(_) => "window",
            scripts::SearchResult::Agent(_) => "agent",
            scripts::SearchResult::Skill(_) => "skill",
            scripts::SearchResult::Fallback(_) => "fallback",
        };
        let metadata = match result {
            scripts::SearchResult::Script(m) => serde_json::json!({
                "name": m.script.name,
                "path": m.script.path.to_string_lossy(),
                "description": m.script.description,
                "shortcut": m.script.shortcut,
                "alias": m.script.alias,
            }),
            scripts::SearchResult::Scriptlet(m) => serde_json::json!({
                "name": m.scriptlet.name,
                "description": m.scriptlet.description,
                "filePath": m.scriptlet.file_path,
            }),
            scripts::SearchResult::BuiltIn(m) => serde_json::json!({
                "id": m.entry.id,
                "name": m.entry.name,
                "description": m.entry.description,
            }),
            scripts::SearchResult::App(m) => serde_json::json!({
                "name": m.app.name,
                "path": m.app.path.to_string_lossy(),
                "bundleId": m.app.bundle_id,
            }),
            scripts::SearchResult::Window(m) => serde_json::json!({
                "app": m.window.app,
                "title": m.window.title,
            }),
            scripts::SearchResult::Agent(m) => serde_json::json!({
                "name": m.agent.name,
                "path": m.agent.path.to_string_lossy(),
                "description": m.agent.description,
            }),
            scripts::SearchResult::Skill(m) => serde_json::json!({
                "pluginId": m.skill.plugin_id,
                "skillId": m.skill.skill_id,
                "title": m.skill.title,
                "description": m.skill.description,
                "path": m.skill.path.to_string_lossy(),
            }),
            scripts::SearchResult::Fallback(m) => serde_json::json!({
                "name": m.fallback.display_label(),
                "description": m.fallback.display_description(),
            }),
        };
        crate::ai::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: kind.to_string(),
            semantic_id: crate::protocol::generate_semantic_id("choice", index, &name),
            label: name,
            metadata: Some(metadata),
        }
    }

    /// Return the first `limit` file search results in display order.
    fn visible_file_search_results(
        &self,
        limit: usize,
    ) -> Vec<(usize, &crate::file_search::FileResult)> {
        (0..self.file_search_display_indices.len())
            .take(limit)
            .filter_map(|display_index| {
                self.file_search_result_at_display_index(display_index)
                    .map(|result| (display_index, result))
            })
            .collect()
    }

    /// Determine the query mode label for file search AI context.
    fn file_search_query_mode(query: &str) -> &'static str {
        if crate::file_search::parse_directory_path(query).is_some() {
            "path-browse"
        } else if crate::file_search::looks_like_advanced_mdquery(query) {
            "spotlight-advanced"
        } else {
            "spotlight-basic"
        }
    }

    /// Build surface-level metadata for the file search view, shared across
    /// all targets so the AI can reason about the query and visible result set.
    fn file_search_surface_metadata(
        &self,
        query: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut metadata = serde_json::Map::new();

        let visible_results: Vec<serde_json::Value> = self
            .visible_file_search_results(TAB_AI_VISIBLE_TARGET_LIMIT)
            .into_iter()
            .map(|(display_index, entry)| {
                serde_json::json!({
                    "displayIndex": display_index,
                    "name": entry.name.clone(),
                    "path": entry.path.clone(),
                    "fileType": format!("{:?}", entry.file_type),
                })
            })
            .collect();

        metadata.insert(
            "query".to_string(),
            serde_json::Value::String(query.to_string()),
        );
        metadata.insert(
            "queryMode".to_string(),
            serde_json::Value::String(Self::file_search_query_mode(query).to_string()),
        );
        metadata.insert(
            "visibleResultCount".to_string(),
            serde_json::json!(self.file_search_display_indices.len()),
        );
        metadata.insert(
            "visibleResults".to_string(),
            serde_json::Value::Array(visible_results),
        );

        if let Some(parsed) = crate::file_search::parse_directory_path(query) {
            metadata.insert(
                "directory".to_string(),
                serde_json::Value::String(parsed.directory),
            );
            metadata.insert(
                "directoryFilter".to_string(),
                match parsed.filter {
                    Some(filter) => serde_json::Value::String(filter),
                    None => serde_json::Value::Null,
                },
            );
        }

        metadata
    }

    /// Convert a file search result into a `TabAiTargetContext`, enriched
    /// with surface-level metadata about the query mode and visible results.
    fn tab_ai_target_from_file_search_result(
        display_index: usize,
        entry: &crate::file_search::FileResult,
        surface_metadata: &serde_json::Map<String, serde_json::Value>,
    ) -> crate::ai::TabAiTargetContext {
        let mut metadata = surface_metadata.clone();
        metadata.insert("displayIndex".to_string(), serde_json::json!(display_index));
        metadata.insert(
            "path".to_string(),
            serde_json::Value::String(entry.path.clone()),
        );
        metadata.insert(
            "fileType".to_string(),
            serde_json::Value::String(format!("{:?}", entry.file_type)),
        );
        metadata.insert("size".to_string(), serde_json::json!(entry.size));
        metadata.insert("modified".to_string(), serde_json::json!(entry.modified));

        crate::ai::TabAiTargetContext {
            source: "FileSearch".to_string(),
            kind: if entry.file_type == crate::file_search::FileType::Directory {
                "directory".to_string()
            } else {
                "file".to_string()
            },
            semantic_id: crate::protocol::generate_semantic_id(
                "choice",
                display_index,
                &entry.name,
            ),
            label: entry.name.clone(),
            metadata: Some(serde_json::Value::Object(metadata)),
        }
    }

    /// Convert a search query into a `TabAiTargetContext`.
    fn tab_ai_target_from_search_query(
        source: &str,
        query: &str,
        metadata: serde_json::Value,
    ) -> crate::ai::TabAiTargetContext {
        let trimmed = query.trim();
        let label = crate::ai::truncate_tab_ai_text(trimmed, 96);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_search_query_target_resolved",
            item_source = %source,
            label_chars = label.chars().count(),
        );
        crate::ai::TabAiTargetContext {
            source: source.to_string(),
            kind: "search_query".to_string(),
            semantic_id: crate::protocol::generate_semantic_id("query", 0, trimmed),
            label,
            metadata: Some(metadata),
        }
    }

    /// Convert raw input text into a lightweight `TabAiTargetContext`.
    fn tab_ai_target_from_input_text(
        source: &str,
        input_text: &str,
    ) -> crate::ai::TabAiTargetContext {
        let trimmed = input_text.trim();
        let label = crate::ai::truncate_tab_ai_text(trimmed, 96);
        let preview = crate::ai::truncate_tab_ai_text(trimmed, 400);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_input_target_resolved",
            item_source = %source,
            label_chars = label.chars().count(),
        );
        crate::ai::TabAiTargetContext {
            source: source.to_string(),
            kind: "input".to_string(),
            semantic_id: crate::protocol::generate_semantic_id("input", 0, trimmed),
            label,
            metadata: Some(serde_json::json!({
                "inputPreview": preview,
                "inputLength": trimmed.chars().count(),
            })),
        }
    }

    /// Source-view-aware target resolution: resolves targets against an explicit view
    /// instead of `self.current_view`. Used at submit time when `current_view`
    /// has already switched to the harness terminal.
    fn resolve_tab_ai_surface_targets_for_view(
        &self,
        view: &AppView,
        ui: &crate::ai::TabAiUiSnapshot,
    ) -> (
        Option<crate::ai::TabAiTargetContext>,
        Vec<crate::ai::TabAiTargetContext>,
    ) {
        match view {
            AppView::ClipboardHistoryView { selected_index, .. } => {
                let focused_target =
                    self.cached_clipboard_entries
                        .get(*selected_index)
                        .map(|entry| {
                            let preview = if entry.content_type.as_str() == "image" {
                                entry
                                    .ocr_text
                                    .clone()
                                    .filter(|text| !text.trim().is_empty())
                                    .unwrap_or_else(|| entry.display_preview())
                            } else {
                                entry.display_preview()
                            };
                            crate::ai::TabAiTargetContext {
                                source: "ClipboardHistory".to_string(),
                                kind: "clipboard_entry".to_string(),
                                semantic_id: crate::protocol::generate_semantic_id(
                                    "choice",
                                    *selected_index,
                                    &entry.text_preview,
                                ),
                                label: preview.clone(),
                                metadata: Some(serde_json::json!({
                                    "id": entry.id.clone(),
                                    "timestamp": entry.timestamp,
                                    "contentType": entry.content_type.as_str(),
                                    "preview": preview,
                                    "ocrText": entry.ocr_text.clone(),
                                    "imageWidth": entry.image_width,
                                    "imageHeight": entry.image_height,
                                })),
                            }
                        });
                let visible_targets = self
                    .cached_clipboard_entries
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, entry)| {
                        let preview = if entry.content_type.as_str() == "image" {
                            entry
                                .ocr_text
                                .clone()
                                .filter(|text| !text.trim().is_empty())
                                .unwrap_or_else(|| entry.display_preview())
                        } else {
                            entry.display_preview()
                        };
                        crate::ai::TabAiTargetContext {
                            source: "ClipboardHistory".to_string(),
                            kind: "clipboard_entry".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice",
                                index,
                                &entry.text_preview,
                            ),
                            label: preview.clone(),
                            metadata: Some(serde_json::json!({
                                "id": entry.id.clone(),
                                "timestamp": entry.timestamp,
                                "contentType": entry.content_type.as_str(),
                                "preview": preview,
                                "ocrText": entry.ocr_text.clone(),
                                "imageWidth": entry.image_width,
                                "imageHeight": entry.image_height,
                            })),
                        }
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::FileSearchView {
                query,
                selected_index,
                ..
            } => {
                let surface_metadata = self.file_search_surface_metadata(query);
                let focused_target = self
                    .selected_file_search_result(*selected_index)
                    .map(|(display_index, entry)| {
                        Self::tab_ai_target_from_file_search_result(
                            display_index,
                            entry,
                            &surface_metadata,
                        )
                    })
                    .or_else(|| {
                        let trimmed = query.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(Self::tab_ai_target_from_search_query(
                                "FileSearch",
                                trimmed,
                                serde_json::Value::Object(surface_metadata.clone()),
                            ))
                        }
                    });
                let visible_targets = self
                    .visible_file_search_results(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .into_iter()
                    .map(|(display_index, entry)| {
                        Self::tab_ai_target_from_file_search_result(
                            display_index,
                            entry,
                            &surface_metadata,
                        )
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::WindowSwitcherView { selected_index, .. } => {
                let focused_target = self.cached_windows.get(*selected_index).map(|entry| {
                    let label = format!("{} — {}", entry.app, entry.title);
                    crate::ai::TabAiTargetContext {
                        source: "WindowSwitcher".to_string(),
                        kind: "window".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice",
                            *selected_index,
                            &label,
                        ),
                        label,
                        metadata: Some(serde_json::json!({
                            "app": entry.app.clone(),
                            "title": entry.title.clone(),
                        })),
                    }
                });
                let visible_targets = self
                    .cached_windows
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, entry)| {
                        let label = format!("{} — {}", entry.app, entry.title);
                        crate::ai::TabAiTargetContext {
                            source: "WindowSwitcher".to_string(),
                            kind: "window".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice", index, &label,
                            ),
                            label,
                            metadata: Some(serde_json::json!({
                                "app": entry.app.clone(),
                                "title": entry.title.clone(),
                            })),
                        }
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::AppLauncherView { selected_index, .. } => {
                let focused_target =
                    self.apps
                        .get(*selected_index)
                        .map(|app| crate::ai::TabAiTargetContext {
                            source: "AppLauncher".to_string(),
                            kind: "app".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice",
                                *selected_index,
                                &app.name,
                            ),
                            label: app.name.clone(),
                            metadata: Some(serde_json::json!({
                                "name": app.name.clone(),
                            })),
                        });
                let visible_targets = self
                    .apps
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, app)| crate::ai::TabAiTargetContext {
                        source: "AppLauncher".to_string(),
                        kind: "app".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice", index, &app.name,
                        ),
                        label: app.name.clone(),
                        metadata: Some(serde_json::json!({
                            "name": app.name.clone(),
                        })),
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::ProcessManagerView { selected_index, .. } => {
                let focused_target = self.cached_processes.get(*selected_index).map(|process| {
                    crate::ai::TabAiTargetContext {
                        source: "ProcessManager".to_string(),
                        kind: "process".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice",
                            *selected_index,
                            &process.script_path,
                        ),
                        label: process.script_path.clone(),
                        metadata: Some(serde_json::json!({
                            "scriptPath": process.script_path.clone(),
                        })),
                    }
                });
                let visible_targets = self
                    .cached_processes
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, process)| crate::ai::TabAiTargetContext {
                        source: "ProcessManager".to_string(),
                        kind: "process".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice",
                            index,
                            &process.script_path,
                        ),
                        label: process.script_path.clone(),
                        metadata: Some(serde_json::json!({
                            "scriptPath": process.script_path.clone(),
                        })),
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::CurrentAppCommandsView { selected_index, .. } => {
                let focused_target =
                    self.cached_current_app_entries
                        .get(*selected_index)
                        .map(|entry| crate::ai::TabAiTargetContext {
                            source: "CurrentAppCommands".to_string(),
                            kind: "menu_command".to_string(),
                            semantic_id: crate::protocol::generate_semantic_id(
                                "choice",
                                *selected_index,
                                &entry.name,
                            ),
                            label: entry.name.clone(),
                            metadata: Some(serde_json::json!({
                                "name": entry.name.clone(),
                            })),
                        });
                let visible_targets = self
                    .cached_current_app_entries
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(index, entry)| crate::ai::TabAiTargetContext {
                        source: "CurrentAppCommands".to_string(),
                        kind: "menu_command".to_string(),
                        semantic_id: crate::protocol::generate_semantic_id(
                            "choice",
                            index,
                            &entry.name,
                        ),
                        label: entry.name.clone(),
                        metadata: Some(serde_json::json!({
                            "name": entry.name.clone(),
                        })),
                    })
                    .collect();
                (focused_target, visible_targets)
            }
            AppView::ScriptList => {
                // Resolve the focused script list item through the grouped results
                // cache, which maps selected_index → flat result index → SearchResult.
                let focused_target = self
                    .cached_grouped_items
                    .get(self.selected_index)
                    .and_then(|item| match item {
                        GroupedListItem::Item(result_idx) => {
                            self.cached_grouped_flat_results.get(*result_idx)
                        }
                        _ => None,
                    })
                    .map(|result| {
                        Self::tab_ai_target_from_search_result(self.selected_index, result)
                    })
                    .or_else(|| {
                        let trimmed = self.filter_text.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(Self::tab_ai_target_from_search_query(
                                "ScriptList",
                                trimmed,
                                serde_json::json!({
                                    "query": trimmed,
                                    "visibleResultCount": self.cached_grouped_flat_results.len(),
                                }),
                            ))
                        }
                    });

                let visible_targets: Vec<crate::ai::TabAiTargetContext> = self
                    .cached_grouped_items
                    .iter()
                    .filter_map(|item| match item {
                        GroupedListItem::Item(result_idx) => {
                            self.cached_grouped_flat_results.get(*result_idx)
                        }
                        _ => None,
                    })
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .enumerate()
                    .map(|(display_index, result)| {
                        Self::tab_ai_target_from_search_result(display_index, result)
                    })
                    .collect();

                (focused_target, visible_targets)
            }
            _ => {
                let visible_targets: Vec<crate::ai::TabAiTargetContext> = ui
                    .visible_elements
                    .iter()
                    .take(TAB_AI_VISIBLE_TARGET_LIMIT)
                    .map(|element| Self::tab_ai_target_from_element(&ui.prompt_type, element))
                    .collect();

                let focused_target = ui
                    .selected_semantic_id
                    .as_deref()
                    .or(ui.focused_semantic_id.as_deref())
                    .and_then(|semantic_id| {
                        visible_targets
                            .iter()
                            .find(|target| target.semantic_id == semantic_id)
                            .cloned()
                            .or_else(|| {
                                ui.visible_elements
                                    .iter()
                                    .find(|element| element.semantic_id == semantic_id)
                                    .map(|element| {
                                        Self::tab_ai_target_from_element(&ui.prompt_type, element)
                                    })
                            })
                    })
                    .or_else(|| {
                        ui.input_text
                            .as_deref()
                            .map(str::trim)
                            .filter(|text| !text.is_empty())
                            .map(|text| Self::tab_ai_target_from_input_text(&ui.prompt_type, text))
                    });

                (focused_target, visible_targets)
            }
        }
    }

    /// Format visible file search results for AI context injection.
    fn format_file_search_ai_visible_results(
        &self,
        selected_display_index: Option<usize>,
        limit: usize,
    ) -> String {
        self.visible_file_search_results(limit)
            .into_iter()
            .map(|(display_index, entry)| {
                let marker = if Some(display_index) == selected_display_index {
                    "*"
                } else {
                    "-"
                };
                let kind = if entry.file_type == crate::file_search::FileType::Directory {
                    "directory"
                } else {
                    "file"
                };
                format!("{marker} [{}] {} ({kind})", display_index, entry.path)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build a query-level AI intent when no valid row is selected.
    /// Falls back to the current query and visible result set.
    pub(crate) fn build_file_search_ai_query_intent(
        &self,
        query: &str,
        plan_mode: bool,
    ) -> Option<String> {
        let query = query.trim();
        if query.is_empty() && self.file_search_display_indices.is_empty() {
            return None;
        }

        let nearby = self.format_file_search_ai_visible_results(None, 6);
        let nearby = if nearby.is_empty() {
            "- (no visible results yet)".to_string()
        } else {
            nearby
        };

        let presentation = match &self.current_view {
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Mini,
                ..
            } => "mini",
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Full,
                ..
            } => "full",
            _ => "unknown",
        };

        let query_mode = if query.is_empty() {
            "empty"
        } else {
            Self::file_search_query_mode(query)
        };

        Some(if plan_mode {
            format!(
                "I am browsing files in Script Kit.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Query mode: {query_mode}\n\
                 Visible results:\n\
                 {nearby}\n\n\
                 Use the current search as the primary context.\n\
                 Propose a concrete next-step plan, including which files or directories to inspect next,\n\
                 how to refine the query, and how to verify the result."
            )
        } else {
            format!(
                "I am browsing files in Script Kit.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Query mode: {query_mode}\n\
                 Visible results:\n\
                 {nearby}\n\n\
                 Use the current search as the primary context.\n\
                 Summarize what this search is likely showing, point out the most important pattern,\n\
                 and tell me the highest-leverage next search or file to inspect."
            )
        })
    }

    /// Build an entry intent string for launching the AI harness from file
    /// search.  Returns `None` when no file is selected.
    ///
    /// `plan_mode`:
    /// - `false` (⌘↵) — "explain this file/directory"
    /// - `true`  (⌘⇧↵) — "propose a plan using this selection + query"
    pub(crate) fn build_file_search_ai_entry_intent(
        &self,
        query: &str,
        selected_index: usize,
        plan_mode: bool,
    ) -> Option<String> {
        let (display_index, selected) = self.selected_file_search_result(selected_index)?;

        let subject = if selected.file_type == crate::file_search::FileType::Directory {
            "directory"
        } else {
            "file"
        };

        let nearby = self.format_file_search_ai_visible_results(Some(display_index), 6);

        let presentation = match &self.current_view {
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Mini,
                ..
            } => "mini",
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Full,
                ..
            } => "full",
            _ => "unknown",
        };

        Some(if plan_mode {
            format!(
                "I am browsing files in Script Kit.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Selected {subject}: {}\n\
                 Nearby visible results:\n\
                 {nearby}\n\n\
                 Use the selected item as the primary target. Use nearby results only as supporting context.\n\
                 Propose a concrete plan, related files to inspect, and verification steps.",
                selected.path
            )
        } else {
            format!(
                "I selected this {subject} in Script Kit file search.\n\
                 File-search presentation: {presentation}\n\
                 Current file-search query: {query}\n\
                 Selected {subject}: {}\n\
                 Nearby visible results:\n\
                 {nearby}\n\n\
                 Explain what it is, summarize why it matters, and tell me the highest-leverage next thing to inspect or change.",
                selected.path
            )
        })
    }

    /// Open the AI harness with the currently selected file-search result
    /// routed through the quick-submit plan path (richer harness hints).
    ///
    /// Returns `false` when there is no valid selection or intent, so the
    /// caller can fall through to default key handling.
    pub(crate) fn open_file_search_selection_in_tab_ai(
        &mut self,
        query: &str,
        selected_index: usize,
        plan_mode: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_display_index, selected)) = self.selected_file_search_result(selected_index)
        else {
            return false;
        };

        let Some(intent) = self.build_file_search_ai_entry_intent(query, selected_index, plan_mode)
        else {
            return false;
        };

        let plan = crate::ai::TabAiQuickSubmitPlan {
            source: crate::ai::TabAiQuickSubmitSource::FileSearch,
            kind: crate::ai::TabAiQuickSubmitKind::FileDrop,
            raw_query: selected.path.clone(),
            normalized_query: selected.path.clone(),
            synthesized_intent: intent,
            capture_kind: "defaultContext".to_string(),
            submit: true,
        };

        self.open_tab_ai_chat_with_quick_submit_plan(plan, cx);
        true
    }

    /// Open the AI harness from file search, falling back to a query-level
    /// intent when no valid row is selected.
    ///
    /// Returns `false` only when no useful context exists at all (empty query
    /// and no visible results), so `⌘↵` / `⌘⇧↵` is never a dead keypress.
    pub(crate) fn open_file_search_selection_or_query_in_tab_ai(
        &mut self,
        query: &str,
        selected_index: usize,
        plan_mode: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.open_file_search_selection_in_tab_ai(query, selected_index, plan_mode, cx) {
            return true;
        }

        let Some(intent) = self.build_file_search_ai_query_intent(query, plan_mode) else {
            return false;
        };

        self.open_tab_ai_chat_with_entry_intent(Some(intent), cx);
        true
    }

    /// Fast-path UI snapshot for `AppView::ScriptList`.
    ///
    /// Skips the expensive `collect_visible_elements()` tree walk because the
    /// downstream target resolution in `resolve_tab_ai_surface_targets_for_view`
    /// already uses `cached_grouped_items` / `cached_grouped_flat_results`.
    fn snapshot_tab_ai_ui_fast_for_script_list(
        &self,
    ) -> (
        crate::ai::TabAiUiSnapshot,
        crate::ai::TabAiInvocationReceipt,
    ) {
        let prompt_type = "ScriptList".to_string();
        let input_text = if self.filter_text.is_empty() {
            None
        } else {
            Some(self.filter_text.clone())
        };

        let focused_semantic_id =
            self.cached_grouped_items
                .get(self.selected_index)
                .and_then(|item| match item {
                    GroupedListItem::Item(result_idx) => self
                        .cached_grouped_flat_results
                        .get(*result_idx)
                        .map(|result| {
                            Self::tab_ai_target_from_search_result(self.selected_index, result)
                                .semantic_id
                        }),
                    _ => None,
                });
        let selected_semantic_id = focused_semantic_id.clone();

        let snapshot = crate::ai::TabAiUiSnapshot {
            prompt_type: prompt_type.clone(),
            input_text: input_text.clone(),
            focused_semantic_id: focused_semantic_id.clone(),
            selected_semantic_id: selected_semantic_id.clone(),
            visible_elements: Vec::new(),
        };

        let receipt = crate::ai::TabAiInvocationReceipt::from_snapshot(
            &prompt_type,
            &input_text,
            &focused_semantic_id,
            &selected_semantic_id,
            0,
            &[],
        );

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_snapshot_fast_script_list",
            has_input_text = snapshot.input_text.is_some(),
            has_focus_target = snapshot.focused_semantic_id.is_some(),
            selected_index = self.selected_index,
        );

        (snapshot, receipt)
    }

    /// Capture a snapshot of the current UI state for context assembly.
    ///
    /// Returns the snapshot and a machine-readable invocation receipt that
    /// identifies whether UI context was rich or degraded with explicit
    /// reason codes.
    #[allow(dead_code)]
    fn snapshot_tab_ai_ui(
        &self,
        cx: &Context<Self>,
    ) -> (
        crate::ai::TabAiUiSnapshot,
        crate::ai::TabAiInvocationReceipt,
    ) {
        let prompt_type = self.app_view_name();

        // Collect visible elements
        let outcome = self.collect_visible_elements(TAB_AI_VISIBLE_ELEMENT_LIMIT, cx);

        let input_text = self.current_input_text(cx);
        let focused_id = outcome.focused_semantic_id();
        let selected_id = outcome.selected_semantic_id();

        // Build the machine-readable invocation receipt
        let receipt = crate::ai::TabAiInvocationReceipt::from_snapshot(
            &prompt_type,
            &input_text,
            &focused_id,
            &selected_id,
            outcome.elements.len(),
            &outcome.warnings,
        );

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "tab_ai_snapshot_captured",
            prompt_type = %prompt_type,
            input_status = %receipt.input_status,
            focus_status = %receipt.focus_status,
            elements_status = %receipt.elements_status,
            has_input_text = receipt.has_input_text,
            has_focus_target = receipt.has_focus_target,
            element_count = receipt.element_count,
            warning_count = receipt.warning_count,
            rich = receipt.rich,
            degradation_reasons = ?receipt.degradation_reasons,
            "tab ai snapshot captured"
        );

        let snapshot = crate::ai::TabAiUiSnapshot {
            prompt_type,
            input_text,
            focused_semantic_id: focused_id,
            selected_semantic_id: selected_id,
            visible_elements: outcome.elements,
        };

        (snapshot, receipt)
    }

    /// Return a human-readable name for the current `AppView` variant.
    #[allow(dead_code)]
    fn app_view_name(&self) -> String {
        match &self.current_view {
            AppView::ScriptList => "ScriptList".to_string(),
            AppView::ArgPrompt { .. } => "ArgPrompt".to_string(),
            AppView::MiniPrompt { .. } => "MiniPrompt".to_string(),
            AppView::MicroPrompt { .. } => "MicroPrompt".to_string(),
            AppView::DivPrompt { .. } => "DivPrompt".to_string(),
            AppView::FormPrompt { .. } => "FormPrompt".to_string(),
            AppView::TermPrompt { .. } => "TermPrompt".to_string(),
            AppView::EditorPrompt { .. } => "EditorPrompt".to_string(),
            AppView::SelectPrompt { .. } => "SelectPrompt".to_string(),
            AppView::PathPrompt { .. } => "PathPrompt".to_string(),
            AppView::EnvPrompt { .. } => "EnvPrompt".to_string(),
            AppView::DropPrompt { .. } => "DropPrompt".to_string(),
            AppView::TemplatePrompt { .. } => "TemplatePrompt".to_string(),
            AppView::ChatPrompt { .. } => "ChatPrompt".to_string(),
            AppView::ClipboardHistoryView { .. } => "ClipboardHistory".to_string(),
            AppView::AppLauncherView { .. } => "AppLauncher".to_string(),
            AppView::WindowSwitcherView { .. } => "WindowSwitcher".to_string(),
            AppView::FileSearchView { .. } => "FileSearch".to_string(),
            AppView::ThemeChooserView { .. } => "ThemeChooser".to_string(),
            AppView::EmojiPickerView { .. } => "EmojiPicker".to_string(),
            AppView::WebcamView { .. } => "Webcam".to_string(),
            AppView::ScratchPadView { .. } => "ScratchPad".to_string(),
            AppView::QuickTerminalView { .. } => "QuickTerminal".to_string(),
            AppView::NamingPrompt { .. } => "NamingPrompt".to_string(),
            AppView::CreationFeedback { .. } => "CreationFeedback".to_string(),
            AppView::DesignGalleryView { .. } => "DesignGallery".to_string(),
            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => "DesignExplorer".to_string(),
            AppView::ActionsDialog => "ActionsDialog".to_string(),
            AppView::BrowseKitsView { .. } => "BrowseKits".to_string(),
            AppView::InstalledKitsView { .. } => "InstalledKits".to_string(),
            AppView::ProcessManagerView { .. } => "ProcessManager".to_string(),
            AppView::SearchAiPresetsView { .. } => "SearchAiPresets".to_string(),
            AppView::CreateAiPresetView { .. } => "CreateAiPreset".to_string(),
            AppView::SettingsView { .. } => "Settings".to_string(),
            AppView::FavoritesBrowseView { .. } => "FavoritesBrowse".to_string(),
            AppView::CurrentAppCommandsView { .. } => "CurrentAppCommands".to_string(),
            AppView::AcpHistoryView { .. } => "AcpHistoryView".to_string(),
            AppView::NotesBrowseView { .. } => "NotesBrowse".to_string(),
            AppView::AcpChatView { .. } => "AcpChatView".to_string(),
        }
    }

    /// Return the current input text from whichever view is active.
    ///
    /// Returns `Some(text)` when the view has user-editable text that is
    /// non-empty, `None` otherwise.  Entity-based prompts are read via
    /// `entity.read(cx)` so this method requires a context reference.
    #[allow(dead_code)]
    fn current_input_text(&self, cx: &Context<Self>) -> Option<String> {
        let non_empty = |s: String| if s.is_empty() { None } else { Some(s) };

        match &self.current_view {
            AppView::ScriptList => non_empty(self.filter_text.clone()),

            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => non_empty(self.arg_input.text().to_string()),

            AppView::ClipboardHistoryView { filter, .. }
            | AppView::AppLauncherView { filter, .. }
            | AppView::WindowSwitcherView { filter, .. }
            | AppView::ThemeChooserView { filter, .. }
            | AppView::EmojiPickerView { filter, .. }
            | AppView::ProcessManagerView { filter, .. }
            | AppView::SettingsView { filter, .. }
            | AppView::SearchAiPresetsView { filter, .. }
            | AppView::FavoritesBrowseView { filter, .. }
            | AppView::CurrentAppCommandsView { filter, .. }
            | AppView::DesignGalleryView { filter, .. }
            | AppView::AcpHistoryView { filter, .. }
            | AppView::NotesBrowseView { filter, .. } => non_empty(filter.clone()),

            AppView::FileSearchView { query, .. } => non_empty(query.clone()),

            AppView::BrowseKitsView { query, .. } => non_empty(query.clone()),

            // --- Entity-based prompts ---
            AppView::EditorPrompt { entity, .. } => {
                entity.read_with(cx, |editor, app| non_empty(editor.content_from_app(app)))
            }
            AppView::ScratchPadView { entity, .. } => {
                entity.read_with(cx, |editor, app| non_empty(editor.content_from_app(app)))
            }
            AppView::ChatPrompt { entity, .. } => {
                non_empty(entity.read(cx).input.text().to_string())
            }
            AppView::PathPrompt { entity, .. } => {
                let p = entity.read(cx);
                // Prefer active filter text; fall back to current directory path
                non_empty(p.filter_text.clone()).or_else(|| non_empty(p.current_path.clone()))
            }
            AppView::EnvPrompt { entity, .. } => {
                let p = entity.read(cx);
                // Return the user-entered value (masked text is still useful
                // for "is something typed?" without revealing secrets)
                if p.secret {
                    // For secret fields, report presence but not content
                    let text = p.input_text();
                    if text.is_empty() {
                        None
                    } else {
                        Some("[secret]".to_string())
                    }
                } else {
                    non_empty(p.input_text().to_string())
                }
            }
            AppView::SelectPrompt { entity, .. } => non_empty(entity.read(cx).filter_text.clone()),
            AppView::NamingPrompt { entity, .. } => {
                non_empty(entity.read(cx).friendly_name.clone())
            }
            AppView::TemplatePrompt { entity, .. } => {
                let p = entity.read(cx);
                // Return the value of the currently focused template input
                p.values
                    .get(p.current_input)
                    .and_then(|v| non_empty(v.clone()))
            }
            AppView::CreateAiPresetView {
                name,
                system_prompt,
                model,
                active_field,
            } => {
                // Return whichever field is active
                match active_field {
                    0 => non_empty(name.clone()),
                    1 => non_empty(system_prompt.clone()),
                    2 => non_empty(model.clone()),
                    _ => non_empty(name.clone()),
                }
            }

            // --- Views with no meaningful user-editable text ---
            // DivPrompt: script-rendered HTML, no user input
            // FormPrompt: multi-field form — field values are in elements,
            //   not a single "input text" (use visible_elements instead)
            // TermPrompt/QuickTerminal: terminal content, not user text input
            // DropPrompt: file drop zone, no typed text
            // WebcamView: camera feed, no text
            // CreationFeedback: read-only confirmation
            // ActionsDialog: transient overlay, not a primary surface
            // InstalledKitsView: navigation-only, no free text
            AppView::DivPrompt { .. }
            | AppView::FormPrompt { .. }
            | AppView::TermPrompt { .. }
            | AppView::QuickTerminalView { .. }
            | AppView::AcpChatView { .. }
            | AppView::DropPrompt { .. }
            | AppView::WebcamView { .. }
            | AppView::CreationFeedback { .. }
            | AppView::ActionsDialog
            | AppView::InstalledKitsView { .. } => None,

            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => None,
        }
    }

    /// Complete the pending Tab AI execution after the script actually exits.
    ///
    /// Gates memory write-back, save-offer, and temp-file cleanup on real
    /// completion status — never at dispatch time.
    ///
    /// Called from the prompt-handler `ScriptExit` / `ScriptError` paths
    /// once the ephemeral process terminates. Uses `take()` on the pending
    /// record so only the first caller does work — subsequent calls are no-ops.
    pub(crate) fn complete_tab_ai_execution(
        &mut self,
        success: bool,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.pending_tab_ai_execution.take() else {
            return;
        };

        let cleanup_attempted = true;
        let cleanup_succeeded = crate::ai::cleanup_tab_ai_temp_script(&record.temp_script_path);

        let status = if success {
            crate::ai::TabAiExecutionStatus::Succeeded
        } else {
            crate::ai::TabAiExecutionStatus::Failed
        };

        let receipt = crate::ai::build_tab_ai_execution_receipt(
            &record,
            status,
            cleanup_attempted,
            cleanup_succeeded,
            error.clone(),
        );

        if let Err(audit_error) = crate::ai::append_tab_ai_execution_receipt(&receipt) {
            tracing::warn!(
                event = "tab_ai_execution_audit_write_failed",
                error = %audit_error,
            );
        }

        if success {
            if let Err(memory_error) = crate::ai::write_tab_ai_memory_entry(&record) {
                tracing::warn!(
                    event = "tab_ai_memory_writeback_failed",
                    error = %memory_error,
                );
            }

            if crate::ai::should_offer_save(&record) {
                tracing::info!(
                    event = "tab_ai_save_offer_open",
                    slug = %record.slug,
                    prompt_type = %record.prompt_type,
                );
                self.open_tab_ai_save_offer(record, cx);
            }
        } else {
            let message = error.unwrap_or_else(|| "Tab AI script failed".to_string());
            self.toast_manager.push(
                components::toast::Toast::error(message, &self.theme)
                    .duration_ms(Some(TOAST_ERROR_MS)),
            );
            cx.notify();
        }
    }
    // ── Tab AI save-offer overlay ──────────────────────────────────────

    fn tab_ai_default_save_name(record: &crate::ai::TabAiExecutionRecord) -> String {
        let derived = super::prompt_ai::derive_script_name_from_description(&record.intent);
        if derived == "ai-generated-script" || derived.is_empty() {
            record.slug.clone()
        } else {
            derived
        }
    }

    fn open_tab_ai_save_offer(
        &mut self,
        record: crate::ai::TabAiExecutionRecord,
        cx: &mut Context<Self>,
    ) {
        let filename_stem = Self::tab_ai_default_save_name(&record);
        tracing::info!(
            event = "tab_ai_save_offer_state_set",
            filename_stem = %filename_stem,
        );
        self.tab_ai_save_offer_state = Some(TabAiSaveOfferState {
            record,
            filename_stem,
            error: None,
        });
        cx.notify();
    }

    fn close_tab_ai_save_offer(&mut self, cx: &mut Context<Self>) {
        if self.tab_ai_save_offer_state.take().is_some() {
            tracing::info!(event = "tab_ai_save_offer_dismissed");
            self.pending_focus = Some(self.tab_ai_return_focus_target());
            cx.notify();
        }
    }

    fn save_tab_ai_script(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.tab_ai_save_offer_state.clone() else {
            return;
        };

        let created_path = match crate::script_creation::create_new_script(&state.filename_stem) {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(
                    event = "tab_ai_save_create_failed",
                    error = %error,
                    filename_stem = %state.filename_stem,
                );
                if let Some(save_state) = &mut self.tab_ai_save_offer_state {
                    save_state.error = Some(format!("Failed to create script: {error}").into());
                }
                cx.notify();
                return;
            }
        };

        if let Err(error) = std::fs::write(&created_path, &state.record.generated_source) {
            tracing::warn!(
                event = "tab_ai_save_write_failed",
                error = %error,
                path = %created_path.display(),
            );
            if let Some(save_state) = &mut self.tab_ai_save_offer_state {
                save_state.error = Some(format!("Failed to write script: {error}").into());
            }
            cx.notify();
            return;
        }

        tracing::info!(
            event = "tab_ai_script_saved",
            filename_stem = %state.filename_stem,
            path = %created_path.display(),
        );

        let created_file_path = if created_path.is_absolute() {
            created_path.clone()
        } else {
            match std::env::current_dir() {
                Ok(cwd) => cwd.join(&created_path),
                Err(_) => created_path.clone(),
            }
        };

        let editor_error =
            crate::script_creation::open_in_editor(&created_path, &self.config).err();

        self.tab_ai_save_offer_state = None;

        match editor_error {
            Some(error) => {
                tracing::warn!(
                    event = "tab_ai_save_editor_open_failed",
                    error = %error,
                );
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Saved script but failed to open editor: {error}"),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
            }
            None => {
                self.toast_manager.push(
                    components::toast::Toast::success(
                        format!("Saved '{}' and opened in editor", state.filename_stem),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_SUCCESS_MS)),
                );
            }
        }

        self.current_view = AppView::CreationFeedback {
            path: created_file_path,
        };
        self.opened_from_main_menu = true;
        cx.notify();
    }

    fn handle_tab_ai_save_offer_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        if crate::ui_foundation::is_key_escape(key) {
            self.close_tab_ai_save_offer(cx);
            cx.stop_propagation();
            return;
        }

        if crate::ui_foundation::is_key_enter(key) {
            self.save_tab_ai_script(cx);
            cx.stop_propagation();
            return;
        }

        cx.propagate();
    }

    pub(crate) fn render_tab_ai_save_offer_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let state = self.tab_ai_save_offer_state.as_ref()?;
        let theme = crate::theme::get_cached_theme();

        // Ensure the main focus handle is focused so key events route here
        if !self.focus_handle.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }

        // Whisper chrome colors — same tokens as the main Tab AI overlay
        let bg_scrim = gpui::rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
            theme.colors.background.main,
            crate::theme::opacity::OPACITY_NEAR_FULL,
        ));
        let text_primary = gpui::rgb(theme.colors.text.primary);
        let error_color = gpui::rgb(theme.colors.ui.error);
        let divider_rgba = gpui::rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
            theme.colors.text.primary,
            crate::theme::opacity::OPACITY_GHOST,
        ));

        let hint_px: f32 = crate::window_resize::mini_layout::HINT_STRIP_PADDING_X;

        let message: SharedString = format!("Save as {}.ts?", state.filename_stem).into();

        // Full-width inline panel — matches main Tab AI overlay chrome,
        // not a floating card. Footer uses HintStrip with save-specific
        // hints (justified exception: this is a confirmation dialog, not
        // the primary input surface).
        let overlay = div()
            .id("tab-ai-save-offer")
            .absolute()
            .inset_0()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .bg(bg_scrim)
            // Message row — bare text, no card, no accent bar
            .child(
                div().w_full().px(px(hint_px)).py(px(10.)).child(
                    div()
                        .text_sm()
                        .font_family(crate::list_item::FONT_MONO)
                        .text_color(text_primary)
                        .child(message),
                ),
            )
            // Hairline divider — ghost opacity
            .child(div().w_full().h(px(1.)).bg(divider_rgba))
            // Error message if present — below divider, minimal
            .when_some(state.error.clone(), |d, msg| {
                d.child(
                    div()
                        .w_full()
                        .px(px(hint_px))
                        .py(px(4.))
                        .text_xs()
                        .text_color(error_color)
                        .child(msg),
                )
            })
            // Spacer pushes footer to bottom
            .child(div().flex_1())
            // Footer — save-specific hint strip via shared component
            // (justified exception: confirmation dialog uses ↵ Save / Esc Dismiss
            // instead of the canonical three-key strip)
            .child(components::HintStrip::new(vec![
                "\u{21B5} Save".into(),
                "Esc Dismiss".into(),
            ]))
            .on_key_down(cx.listener(Self::handle_tab_ai_save_offer_key_down));

        Some(overlay.into_any_element())
    }
}

/// Detect the source type from the originating view and desktop snapshot.
///
/// Priority order:
/// 1. Desktop selected text present → `DesktopSelection`
/// 2. ScriptList with a resolved focused target → `ScriptListItem`
/// 3. ClipboardHistoryView → `ClipboardEntry`
/// 4. Prompt-like active surfaces → `RunningCommand`
/// 5. Fallback → `Desktop`
/// Convert an `AppView` variant to the prompt-type string that the canonical
/// source-type detector in `crate::ai` understands.
fn app_view_to_prompt_type_str(view: &AppView) -> &'static str {
    match view {
        AppView::ScriptList => "ScriptList",
        AppView::ClipboardHistoryView { .. } => "ClipboardHistory",
        AppView::ArgPrompt { .. } => "ArgPrompt",
        AppView::MiniPrompt { .. } => "MiniPrompt",
        AppView::MicroPrompt { .. } => "MicroPrompt",
        AppView::DivPrompt { .. } => "DivPrompt",
        AppView::FormPrompt { .. } => "FormPrompt",
        AppView::EditorPrompt { .. } => "EditorPrompt",
        AppView::SelectPrompt { .. } => "SelectPrompt",
        AppView::PathPrompt { .. } => "PathPrompt",
        AppView::DropPrompt { .. } => "DropPrompt",
        AppView::TemplatePrompt { .. } => "TemplatePrompt",
        AppView::TermPrompt { .. } => "TermPrompt",
        AppView::EnvPrompt { .. } => "EnvPrompt",
        AppView::ChatPrompt { .. } => "ChatPrompt",
        AppView::NamingPrompt { .. } => "NamingPrompt",
        _ => "Other",
    }
}

/// Early source type detection using only the view and UI snapshot — no
/// desktop context required.  Returns `Some` for known-source views where
/// the prompt type alone is sufficient (ClipboardHistory, running prompts,
/// ScriptList with a focused or selected semantic ID).  Returns `None` for
/// generic desktop/selection cases that need the deferred desktop snapshot.
fn detect_tab_ai_source_type_early(
    source_view: &AppView,
    ui: &crate::ai::TabAiUiSnapshot,
) -> Option<crate::ai::TabAiSourceType> {
    let prompt_type = app_view_to_prompt_type_str(source_view);
    match prompt_type {
        "ScriptList" if ui.focused_semantic_id.is_some() || ui.selected_semantic_id.is_some() => {
            Some(crate::ai::TabAiSourceType::ScriptListItem)
        }
        "ClipboardHistory" => Some(crate::ai::TabAiSourceType::ClipboardEntry),
        "ArgPrompt" | "MiniPrompt" | "MicroPrompt" | "DivPrompt" | "FormPrompt"
        | "EditorPrompt" | "SelectPrompt" | "PathPrompt" | "DropPrompt" | "TemplatePrompt"
        | "TermPrompt" | "EnvPrompt" | "ChatPrompt" | "NamingPrompt" => {
            Some(crate::ai::TabAiSourceType::RunningCommand)
        }
        // Desktop / DesktopSelection require the deferred capture's selected_text.
        _ => None,
    }
}

/// Detect source type by delegating to the canonical mapping in
/// `crate::ai::detect_tab_ai_source_type_from_prompt` so classification
/// logic lives in one place.
fn detect_tab_ai_source_type(
    source_view: &AppView,
    desktop: &crate::context_snapshot::AiContextSnapshot,
    focused_target: Option<&crate::ai::TabAiTargetContext>,
) -> Option<crate::ai::TabAiSourceType> {
    crate::ai::detect_tab_ai_source_type_from_prompt(
        app_view_to_prompt_type_str(source_view),
        desktop,
        focused_target,
    )
}

/// Build an apply-back hint from the detected source type.
///
/// Delegates to the canonical mapping in `crate::ai::build_tab_ai_apply_back_hint_from_source`
/// so source classification and apply-back routing share a single truth.
fn build_tab_ai_apply_back_hint(
    source_type: Option<&crate::ai::TabAiSourceType>,
) -> Option<crate::ai::TabAiApplyBackHint> {
    crate::ai::build_tab_ai_apply_back_hint_from_source(source_type)
}

// ---------------------------------------------------------------------------
// Apply-back: clipboard helpers
// ---------------------------------------------------------------------------

fn read_tab_ai_apply_back_clipboard_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_open_failed: {error}"))?;
    let text = clipboard
        .get_text()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_read_failed: {error}"))?;
    if text.trim().is_empty() {
        return Err("tab_ai_apply_back_clipboard_empty".to_string());
    }
    Ok(text)
}

fn write_tab_ai_apply_back_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|error| format!("tab_ai_apply_back_clipboard_open_failed: {error}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| format!("tab_ai_apply_back_clipboard_write_failed: {error}"))
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
                            script_path = %path_str,
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

                    let _ = cx.update(|cx| {
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
                    Ready(crate::ai::TabAiApplyBackRoute),
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
                            return WaitState::Ready(route);
                        }
                        if std::time::Instant::now() >= deadline {
                            return WaitState::TimedOut;
                        }
                        WaitState::Pending
                    })
                });

                match state {
                    WaitState::Ready(route) => {
                        let _ = cx.update(|cx| {
                            let Some(app) = app_weak.upgrade() else {
                                return;
                            };
                            app.update(cx, |this, cx| {
                                this.apply_tab_ai_result_text(route, text.clone(), cx);
                            });
                        });
                        break;
                    }
                    WaitState::TimedOut => {
                        let _ = cx.update(|cx| {
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

        if let Some(text) = selected_text {
            self.apply_tab_ai_result_text_or_wait_for_route(text, cx);
            return;
        }

        // No selection — fall back to clipboard priming (copies last output).
        entity.update(cx, |term_prompt, cx| {
            term_prompt.prime_apply_clipboard(cx);
        });

        let app = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    Self::TAB_AI_APPLY_BACK_CLIPBOARD_PRIME_MS,
                ))
                .await;
            let _ = cx.update(|cx| {
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

#[cfg(test)]
mod tests {
    use crate::{AppView, ScriptListApp};

    #[test]
    fn tab_ai_user_prompt_contains_intent_and_context() {
        let prompt = crate::ai::build_tab_ai_user_prompt("force quit", r#"{"ui":{}}"#);
        assert!(prompt.contains("force quit"));
        assert!(prompt.contains(r#"{"ui":{}}"#));
        assert!(prompt.contains("Script Kit TypeScript"));
    }

    #[test]
    fn tab_ai_user_prompt_contains_code_block_instruction() {
        let prompt = crate::ai::build_tab_ai_user_prompt("test intent", "{}");
        assert!(
            prompt.contains("fenced code block"),
            "Prompt must ask for a fenced code block so extract_generated_script_source works"
        );
    }

    #[test]
    fn tab_ai_user_prompt_separates_intent_from_context() {
        let prompt = crate::ai::build_tab_ai_user_prompt("copy url", r#"{"schemaVersion":1}"#);
        // The intent appears before the context
        let intent_pos = prompt.find("copy url").expect("intent present");
        let context_pos = prompt.find("schemaVersion").expect("context present");
        assert!(
            intent_pos < context_pos,
            "Intent should appear before context JSON"
        );
    }

    #[test]
    fn tab_ai_user_prompt_with_rich_context_json() {
        let context = serde_json::to_string_pretty(&crate::ai::TabAiContextBlob::from_parts(
            crate::ai::TabAiUiSnapshot {
                prompt_type: "ScriptList".to_string(),
                input_text: Some("slack".to_string()),
                focused_semantic_id: Some("input:filter".to_string()),
                selected_semantic_id: Some("choice:0:slack".to_string()),
                visible_elements: vec![],
            },
            Default::default(),
            vec!["recent1".to_string()],
            None,
            vec![],
            vec![],
            "2026-03-28T00:00:00Z".to_string(),
        ))
        .expect("serialize");

        let prompt = crate::ai::build_tab_ai_user_prompt("force quit this app", &context);

        assert!(prompt.contains("force quit this app"));
        assert!(prompt.contains("ScriptList"));
        assert!(prompt.contains("slack"));
        assert!(prompt.contains("choice:0:slack"));
        assert!(prompt.contains("recent1"));
    }

    #[test]
    fn tab_ai_chat_uses_three_key_footer_contract() {
        const TAB_AI_SOURCE: &str = include_str!("tab_ai_mode.rs");
        assert!(
            TAB_AI_SOURCE.contains(r#""\u{21B5} Send"#),
            "tab ai chat should expose the Send hint"
        );
        assert!(
            TAB_AI_SOURCE.contains(r#""\u{2318}K Actions"#),
            "tab ai chat should expose the Actions hint"
        );
        assert!(
            TAB_AI_SOURCE.contains(r#""Esc Back"#),
            "tab ai chat should expose the Esc Back hint"
        );
    }

    #[test]
    fn tab_ai_overlay_preserves_memory_hint_rendering() {
        const TAB_AI_SOURCE: &str = include_str!("tab_ai_mode.rs");
        assert!(
            TAB_AI_SOURCE.contains("Similar prior automation:"),
            "visual cleanup must not silently remove memory-hint behavior"
        );
    }

    #[test]
    fn tab_ai_overlay_uses_named_opacity_constants() {
        const TAB_AI_SOURCE: &str = include_str!("tab_ai_mode.rs");
        // The render function should reference OPACITY_GHOST, not raw 0.06
        assert!(
            TAB_AI_SOURCE.contains("OPACITY_GHOST"),
            "tab ai overlay should use named ghost opacity constant"
        );
    }

    #[test]
    fn tab_ai_overlay_uses_shared_hint_strip_component() {
        const TAB_AI_SOURCE: &str = include_str!("tab_ai_mode.rs");
        assert!(
            TAB_AI_SOURCE.contains("HintStrip::new"),
            "tab ai overlay should use the shared HintStrip component"
        );
    }

    // ── Source-type detection tests ──────────────────────────────────

    #[test]
    fn desktop_selection_beats_internal_surface_classification() {
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("hello".to_string()),
            ..Default::default()
        };
        // Even when the source view is ScriptList with a focused target,
        // desktop selected text takes precedence.
        let focused_target = crate::ai::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, Some(&focused_target),),
            Some(crate::ai::TabAiSourceType::DesktopSelection),
            "Desktop selected text must take precedence over ScriptList classification"
        );
    }

    #[test]
    fn script_list_requires_real_focused_target() {
        let desktop = crate::context_snapshot::AiContextSnapshot::default();

        // ScriptList without a focused target falls back to Desktop
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, None),
            Some(crate::ai::TabAiSourceType::Desktop),
            "ScriptList without focused target must fall back to Desktop"
        );

        // ScriptList WITH a focused target resolves to ScriptListItem
        let focused_target = crate::ai::TabAiTargetContext {
            source: "ScriptList".to_string(),
            kind: "script".to_string(),
            semantic_id: "script:0".to_string(),
            label: "hello-world".to_string(),
            metadata: None,
        };
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, Some(&focused_target),),
            Some(crate::ai::TabAiSourceType::ScriptListItem),
            "ScriptList with focused target must resolve to ScriptListItem"
        );
    }

    #[test]
    fn desktop_selection_whitespace_only_does_not_count() {
        let desktop = crate::context_snapshot::AiContextSnapshot {
            selected_text: Some("   \n\t  ".to_string()),
            ..Default::default()
        };
        // Whitespace-only selection should NOT trigger DesktopSelection
        assert_eq!(
            super::detect_tab_ai_source_type(&AppView::ScriptList, &desktop, None),
            Some(crate::ai::TabAiSourceType::Desktop),
            "Whitespace-only selected text must not trigger DesktopSelection"
        );
    }

    #[test]
    fn source_type_computed_after_context_resolution() {
        // Structural contract: sourceType is computed after build_tab_ai_context_from
        // so it can inspect the resolved focused_target.
        const SRC: &str = include_str!("tab_ai_mode.rs");

        let build_idx = SRC
            .find("let resolved = this.build_tab_ai_context_from(")
            .expect("build_tab_ai_context_from call");
        let detect_idx = SRC
            .find("let source_type = detect_tab_ai_source_type(")
            .expect("detect_tab_ai_source_type call");

        assert!(
            build_idx < detect_idx,
            "sourceType must be computed AFTER build_tab_ai_context_from so it can inspect resolved targets"
        );
    }

    #[test]
    fn detect_source_type_passes_resolved_focused_target() {
        // Structural contract: detect_tab_ai_source_type receives focused_target from resolved context
        const SRC: &str = include_str!("tab_ai_mode.rs");
        assert!(
            SRC.contains("resolved.context.focused_target.as_ref()"),
            "detect_tab_ai_source_type must receive focused_target from the resolved context"
        );
    }

    fn tab_ai_contract_compact(input: &str) -> String {
        input.split_whitespace().collect::<String>()
    }

    fn tab_ai_extract_fn_body(source: &str, signature: &str) -> String {
        let start = source.find(signature).expect("signature must exist");
        let rest = &source[start..];
        let open = rest.find('{').expect("function body must open");
        let mut depth = 0usize;
        let mut end = None;
        for (idx, ch) in rest[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + idx + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        rest[..end.expect("function body must close")].to_string()
    }

    #[test]
    fn tab_ai_open_path_switches_view_before_waiting_for_capture_contract() {
        let source = include_str!("tab_ai_mode.rs");
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            source,
            "fn open_tab_ai_harness_terminal_from_request(",
        ));

        let view_switch = body
            .find(&tab_ai_contract_compact(
                "self.current_view = AppView::QuickTerminalView",
            ))
            .expect("QuickTerminalView switch must exist");
        let notify = body
            .find(&tab_ai_contract_compact("cx.notify();"))
            .expect("cx.notify must exist");
        let capture_wait = body
            .find(&tab_ai_contract_compact("capture_rx.recv().await"))
            .expect("deferred capture await must exist");

        assert!(
            view_switch < notify,
            "the harness view must be selected before notifying the UI"
        );
        assert!(
            notify < capture_wait,
            "the terminal must become visible before waiting for deferred capture"
        );
    }

    #[test]
    fn tab_ai_startup_prewarm_is_marked_fresh_on_cold_start_contract() {
        let source = include_str!("tab_ai_mode.rs");
        // The shared silent helper is where cold-start tagging lives.
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            source,
            "fn warm_tab_ai_harness_silently(",
        ));

        assert!(
            body.contains(&tab_ai_contract_compact("if was_cold_start {")),
            "silent prewarm helper must gate FreshPrewarm tagging on a newly created session"
        );
        assert!(
            body.contains(&tab_ai_contract_compact("session.mark_fresh_prewarm();")),
            "cold-started prewarm must be marked reusable once"
        );
    }

    #[test]
    fn tab_ai_close_path_reseeds_future_prewarm_contract() {
        let source = include_str!("tab_ai_mode.rs");
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            source,
            "fn close_tab_ai_harness_terminal_impl(",
        ));

        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.terminate_tab_ai_harness_session(cx);"
            )),
            "close path must delegate PTY session teardown"
        );
        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.schedule_tab_ai_harness_prewarm(std::time::Duration::from_millis(250), cx);"
            )),
            "close path must schedule a fresh prewarm for the next Tab press"
        );
        assert!(
            body.contains(&tab_ai_contract_compact(
                "self.clear_transient_script_list_trigger_on_return(window, cx);"
            )),
            "close path must clear transient ScriptList trigger filters when returning to the main menu"
        );
    }

    #[test]
    fn script_list_explicit_triggers_do_not_stage_focused_parts() {
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            Some('@'),
            false,
        ));
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            Some('/'),
            false,
        ));
        assert!(ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            None,
            false,
        ));
        assert!(ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ThemeChooserView {
                filter: String::new(),
                selected_index: 0,
            },
            Some('@'),
            false,
        ));
        assert!(!ScriptListApp::should_stage_focused_part_for_request(
            &AppView::ScriptList,
            None,
            true,
        ));
    }

    #[test]
    fn acp_initial_input_prefills_script_list_triggers_without_intent() {
        assert_eq!(
            ScriptListApp::tab_ai_acp_initial_input_for_launch(
                "ScriptList",
                None,
                Some('@'),
                false,
            )
            .as_deref(),
            Some("@")
        );
        assert_eq!(
            ScriptListApp::tab_ai_acp_initial_input_for_launch(
                "ScriptList",
                None,
                Some('/'),
                false,
            )
            .as_deref(),
            Some("/")
        );
    }

    #[test]
    fn acp_initial_input_does_not_prefill_non_script_list_triggers() {
        assert_eq!(
            ScriptListApp::tab_ai_acp_initial_input_for_launch(
                "ThemeChooser",
                None,
                Some('@'),
                false,
            ),
            None
        );
        assert_eq!(
            ScriptListApp::tab_ai_acp_initial_input_for_launch(
                "ScriptList",
                None,
                Some('>'),
                false,
            ),
            None
        );
    }

    #[test]
    fn acp_initial_input_prefers_effective_intent_over_script_list_trigger() {
        assert_eq!(
            ScriptListApp::tab_ai_acp_initial_input_for_launch(
                "ScriptList",
                Some("explain this code"),
                Some('@'),
                true,
            )
            .as_deref(),
            Some("explain this code")
        );
    }

    #[test]
    fn embedded_acp_reuse_requires_entry_intent_and_no_cached_retry_request() {
        assert!(!ScriptListApp::should_reuse_embedded_acp_view_for_open(
            None, false,
        ));
        assert!(ScriptListApp::should_reuse_embedded_acp_view_for_open(
            Some("explain this"),
            false,
        ));
        assert!(!ScriptListApp::should_reuse_embedded_acp_view_for_open(
            Some("switch agent"),
            true,
        ));
    }

    #[test]
    fn embedded_acp_reuse_submits_entry_intent_via_reuse_reset_helper() {
        let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(
            include_str!("tab_ai_mode.rs"),
            "fn try_reuse_embedded_acp_view(",
        ));
        assert!(
            body.contains(&tab_ai_contract_compact(
                "chat.submit_reused_entry_intent(intent.clone(), cx);",
            )),
            "reused ACP entry intents must clear stale composer state before submit"
        );
    }

    #[test]
    fn script_list_trigger_routes_stage_trigger_before_acp_open_contract() {
        let source = include_str!("tab_ai_mode.rs");
        for (signature, trigger) in [
            (
                "pub(crate) fn open_tab_ai_acp_with_slash_picker(",
                "Some('/')",
            ),
            (
                "pub(crate) fn open_tab_ai_acp_with_mention_picker(",
                "Some('@')",
            ),
        ] {
            let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(source, signature));
            let trigger_idx = body
                .find(&tab_ai_contract_compact(&format!(
                    "self.tab_ai_harness_script_list_trigger = {trigger};"
                )))
                .expect("route must stage the trigger first");
            let open_idx = body
                .find(&tab_ai_contract_compact(
                    "self.open_tab_ai_acp_with_entry_intent(None, cx);",
                ))
                .expect("route must open ACP");
            assert!(
                trigger_idx < open_idx,
                "route must stage the trigger before opening ACP"
            );
        }
    }

    #[test]
    fn script_list_trigger_routes_defer_embedded_picker_contract() {
        let source = include_str!("tab_ai_mode.rs");
        for signature in [
            "pub(crate) fn open_tab_ai_acp_with_slash_picker(",
            "pub(crate) fn open_tab_ai_acp_with_mention_picker(",
        ] {
            let body = tab_ai_contract_compact(&tab_ai_extract_fn_body(source, signature));
            assert!(
                body.contains(&tab_ai_contract_compact(
                    "self.schedule_embedded_acp_picker_open("
                )),
                "trigger route must defer embedded picker opening"
            );
        }
    }

    // ── Existing save-name tests ──────────────────────────────────

    #[test]
    fn tab_ai_default_save_name_falls_back_to_slug_when_intent_is_generic() {
        let record = crate::ai::TabAiExecutionRecord::from_parts(
            "".to_string(),
            "import \"@scriptkit/sdk\";\nawait notify(\"ok\");\n".to_string(),
            "/tmp/tab-ai.ts".to_string(),
            "tab-ai-script".to_string(),
            "ScriptList".to_string(),
            None,
            "vercel/test-model".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        assert_eq!(
            ScriptListApp::tab_ai_default_save_name(&record),
            "tab-ai-script"
        );
    }

    #[test]
    fn tab_ai_default_save_name_derives_from_intent_when_meaningful() {
        let record = crate::ai::TabAiExecutionRecord::from_parts(
            "force quit this app".to_string(),
            "code".to_string(),
            "/tmp/tab-ai.ts".to_string(),
            "force-quit-this-app".to_string(),
            "ScriptList".to_string(),
            None,
            "gpt-4.1".to_string(),
            "vercel".to_string(),
            0,
            "2026-03-28T00:00:00Z".to_string(),
        );
        let name = ScriptListApp::tab_ai_default_save_name(&record);
        assert!(
            name.contains("force") && name.contains("quit"),
            "Should derive from intent, got: {name}"
        );
    }
}
