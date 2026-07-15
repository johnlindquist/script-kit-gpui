use super::*;

struct StandardAgentChatMockFixtureConnection;

impl crate::ai::agent_chat::runtime::AgentChatConnection
    for StandardAgentChatMockFixtureConnection
{
    fn start_turn(
        &self,
        _request: crate::ai::agent_chat::runtime::AgentChatTurnRequest,
    ) -> anyhow::Result<crate::ai::agent_chat::events::AgentChatEventRx> {
        let (tx, rx) = async_channel::bounded(2);
        let _ = tx.try_send(
            crate::ai::agent_chat::events::AgentChatEvent::AgentMessageDelta(
                "Fixture Agent Chat response.".to_string(),
            ),
        );
        let _ = tx.try_send(
            crate::ai::agent_chat::events::AgentChatEvent::TurnFinished {
                stop_reason: "fixture".to_string(),
            },
        );
        Ok(rx)
    }

    fn cancel_turn(&self, _ui_thread_id: String) -> anyhow::Result<()> {
        Ok(())
    }

    fn prepare_session(
        &self,
        _ui_thread_id: String,
        _cwd: std::path::PathBuf,
    ) -> anyhow::Result<crate::ai::agent_chat::events::AgentChatEventRx> {
        let (_tx, rx) = async_channel::bounded(1);
        Ok(rx)
    }
}

fn agent_chat_hot_prewarm_enabled_from(disabled: Option<&str>, enabled: Option<&str>) -> bool {
    let truthy = |value: Option<&str>| {
        value
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    };

    !truthy(disabled) && truthy(enabled)
}

impl ScriptListApp {
    /// Open a deterministic, provider-free standard Agent Chat surface for
    /// DevTools and visual smoke tests. This intentionally bypasses Pi warm-up.
    pub(crate) fn open_standard_agent_chat_mock_fixture(&mut self, cx: &mut Context<Self>) {
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);

        let (_broker, permission_rx) = crate::ai::agent_chat::ui::AgentChatPermissionBroker::new();
        let thread = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatThread::new(
                std::sync::Arc::new(StandardAgentChatMockFixtureConnection),
                permission_rx,
                crate::ai::agent_chat::ui::AgentChatThreadInit {
                    ui_thread_id: "standard-agent-chat-mock-fixture".to_string(),
                    cwd: std::env::temp_dir().join("script-kit-agent-chat-fixture"),
                    initial_input: Some("Fixture follow-up".to_string()),
                    initial_context_parts: Vec::new(),
                    display_name: "Agent Chat".into(),
                    profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID
                        .to_string(),
                    profile_display_name: Some("Agent Chat".into()),
                    profile_icon_name: None,
                    selected_agent: None,
                    available_agents: Vec::new(),
                    launch_requirements:
                        crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
                    available_models: Vec::new(),
                    selected_model_id: None,
                },
                cx,
            )
        });
        thread.update(cx, |thread, cx| {
            thread.mark_context_bootstrap_ready(cx);
            let _ = thread.apply_test_fixture(
                "assistantText",
                Some("Can you summarize this fixture?".to_string()),
                Some("This is a deterministic Agent Chat fixture response.".to_string()),
                None,
                cx,
            );
        });

        let view_entity = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatView::new(thread, cx).with_ui_variant(
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            )
        });
        self.wire_embedded_agent_chat_footer_callbacks(&view_entity, cx);
        self.embedded_agent_chat = Some(view_entity.clone());
        self.tab_ai_harness_return_view = Some(source_view);
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
        self.enter_embedded_agent_chat_surface(view_entity, cx);
        self.request_focus(FocusTarget::ChatPrompt, cx);
        script_kit_gpui::set_main_window_visible(true);
        script_kit_gpui::mark_window_shown();
        cx.notify();
    }

    /// Open the deterministic Agent Chat kitchen sink transcript without
    /// starting a provider session.
    pub(crate) fn open_agent_chat_kitchen_sink_fixture(&mut self, cx: &mut Context<Self>) {
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);

        let (_broker, permission_rx) = crate::ai::agent_chat::ui::AgentChatPermissionBroker::new();
        let fixture =
            crate::ai::agent_chat::ui::kitchen_sink_fixture::agent_chat_kitchen_sink_fixture();
        // Pinned, deliberately LONG fixture cwd: reference captures must be
        // byte-reproducible across machines (the old temp_dir()-derived path
        // carried a per-machine /var/folders/<hash>/ prefix) while still
        // stress-testing header-lane truncation and non-overlap. The real
        // PathBuf → cwd_display formatting path stays exercised.
        let fixture_cwd = std::path::PathBuf::from(
            crate::ai::agent_chat::ui::style_contract::AGENT_CHAT_KITCHEN_SINK_FIXTURE_CWD,
        );
        let _ = std::fs::create_dir_all(&fixture_cwd);
        let thread = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatThread::new(
                std::sync::Arc::new(StandardAgentChatMockFixtureConnection),
                permission_rx,
                crate::ai::agent_chat::ui::AgentChatThreadInit {
                    ui_thread_id: fixture.id.to_string(),
                    cwd: fixture_cwd,
                    initial_input: Some("Tweak this kitchen sink transcript.".to_string()),
                    initial_context_parts: Vec::new(),
                    display_name: fixture.title.into(),
                    profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID
                        .to_string(),
                    profile_display_name: Some("Agent Chat Kitchen Sink".into()),
                    profile_icon_name: None,
                    selected_agent: None,
                    available_agents: Vec::new(),
                    launch_requirements:
                        crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
                    available_models: Vec::new(),
                    selected_model_id: None,
                },
                cx,
            )
        });
        thread.update(cx, |thread, cx| {
            thread.mark_context_bootstrap_ready(cx);
            thread.load_kitchen_sink_fixture(cx);
        });

        let view_entity = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatView::new(thread, cx).with_ui_variant(
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            )
        });
        self.wire_embedded_agent_chat_footer_callbacks(&view_entity, cx);
        self.embedded_agent_chat = Some(view_entity.clone());
        self.tab_ai_harness_return_view = Some(source_view);
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
        self.enter_embedded_agent_chat_surface(view_entity, cx);
        self.request_focus(FocusTarget::ChatPrompt, cx);
        script_kit_gpui::set_main_window_visible(true);
        script_kit_gpui::mark_window_shown();
        cx.notify();
    }

    /// **Contract:** `AppView::AgentChatView` and `cx.notify()` happen
    /// *before* any deferred-capture await. The user sees the chat surface
    /// within one frame.
    pub(super) fn open_tab_ai_agent_chat_view_from_request_impl(
        &mut self,
        request: TabAiLaunchRequest,
        capture_rx: TabAiDeferredCaptureRx,
        focused_part: Option<crate::ai::message_parts::AiContextPart>,
        use_ask_anything_fallback: bool,
        explicit_ambient_chip_label: Option<String>,
        force_agent_chat_surface: bool,
        cx: &mut Context<Self>,
    ) {
        let open_started_at = std::time::Instant::now();
        let source_view = request.source_view.clone();
        let had_harness_session = self.tab_ai_harness.is_some();
        let pending_script_list_trigger = self.tab_ai_harness_script_list_trigger;

        // Compute canonical effective intent once, matching PTY path's normalization.
        let effective_intent = Self::tab_ai_effective_submission_intent(&request);
        let auto_submit = effective_intent.is_some();

        // Build Agent Chat initial input via the shared helper, ensuring the same
        // verification contract as the PTY submission path.
        // When force_agent_chat_surface is set (Auto Submit fallback), use the raw
        // intent without new-script guidance — the query may be general.
        let agent_chat_initial_input = effective_intent
            .clone()
            .map(|intent| {
                if force_agent_chat_surface {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_agent_chat_initial_input_built",
                        prompt_type = %request.ui_snapshot.prompt_type,
                        guidance_appended = false,
                        forced_by_script_list_submit = false,
                        force_agent_chat_surface = true,
                    );
                    intent
                } else {
                    let initial_input = crate::ai::harness::build_tab_ai_agent_chat_initial_input_for_prompt(
                        &request.ui_snapshot.prompt_type,
                        &intent,
                    );

                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "tab_ai_agent_chat_initial_input_built",
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
                Self::tab_ai_agent_chat_initial_input_for_launch(
                    &request.ui_snapshot.prompt_type,
                    None,
                    pending_script_list_trigger,
                    force_agent_chat_surface,
                )
            });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_open_begin",
            agent_chat_ui_variant = request.ui_variant.state_id(),
            auto_submit,
            has_entry_intent = request.entry_intent.is_some(),
            had_harness_session,
            pending_script_list_trigger = ?pending_script_list_trigger,
            prefilled_len = agent_chat_initial_input.as_ref().map(|text| text.len()).unwrap_or(0),
        );

        // --- Permission broker + Agent Chat connection ---
        let stage_started_at = std::time::Instant::now();
        let (_broker, permission_rx) = crate::ai::agent_chat::ui::AgentChatPermissionBroker::new();
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_open_stage",
            stage = "permission_broker_new",
            stage_ms = stage_started_at.elapsed().as_millis() as u64,
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        let ai_preferences = crate::config::load_user_preferences().ai;
        let focused_text_mini = request.ui_variant
            == crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::FocusedTextMini;
        let quick_ai = request.ui_variant
            == crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::QuickAi;
        let cwd_override = if focused_text_mini || quick_ai {
            None
        } else {
            self.spine_cwd_for_agent_chat_launch()
        };
        let pi_launch_result = if focused_text_mini {
            crate::ai::agent_chat::launch::resolve_focused_text_pi_launch(
                &ai_preferences,
                &profile_ctx,
            )
        } else if quick_ai {
            crate::ai::agent_chat::launch::resolve_quick_ai_pi_launch(&ai_preferences, &profile_ctx)
        } else {
            crate::ai::agent_chat::launch::resolve_selected_pi_launch_with_cwd_override(
                &ai_preferences,
                &profile_ctx,
                cwd_override.clone(),
            )
        };
        match pi_launch_result {
            Ok(pi_launch) => {
                if cwd_override.is_some() {
                    let default_cwd = crate::ai::agent_chat::launch::resolve_selected_launch_cwd(
                        &ai_preferences,
                        &profile_ctx,
                    );
                    crate::ai::agent_chat::ui::record_agent_chat_cwd_recent(
                        &pi_launch.profile.id,
                        pi_launch.cwd.clone(),
                        Some(default_cwd.as_path()),
                    );
                }
                self.open_tab_ai_pi_view_from_launch(
                    pi_launch,
                    request,
                    capture_rx,
                    focused_part,
                    use_ask_anything_fallback,
                    explicit_ambient_chip_label,
                    auto_submit,
                    effective_intent,
                    agent_chat_initial_input,
                    permission_rx,
                    source_view,
                    had_harness_session,
                    pending_script_list_trigger,
                    open_started_at,
                    cx,
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "pi_agent_chat_launch_resolution_failed",
                    error = %error,
                    focused_text_mini,
                );
                if focused_text_mini {
                    self.toast_manager.push(
                        crate::components::toast::Toast::error(
                            "Pi Text profile is unavailable",
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                }
                if quick_ai {
                    self.toast_manager.push(
                        crate::components::toast::Toast::error(
                            "Quick AI is unavailable",
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                }
                self.show_pi_agent_chat_unavailable_setup_view(source_view, error.to_string(), cx);
                return;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_tab_ai_pi_view_from_launch(
        &mut self,
        pi_launch: crate::ai::agent_chat::launch::PiAgentChatLaunch,
        request: TabAiLaunchRequest,
        capture_rx: TabAiDeferredCaptureRx,
        focused_part: Option<crate::ai::message_parts::AiContextPart>,
        use_ask_anything_fallback: bool,
        explicit_ambient_chip_label: Option<String>,
        auto_submit: bool,
        effective_intent: Option<String>,
        agent_chat_initial_input: Option<String>,
        permission_rx: async_channel::Receiver<crate::ai::agent_chat::ui::AgentChatApprovalRequest>,
        source_view: AppView,
        had_harness_session: bool,
        pending_script_list_trigger: Option<char>,
        open_started_at: std::time::Instant,
        cx: &mut Context<Self>,
    ) {
        let requirements = crate::ai::agent_chat::ui::AgentChatLaunchRequirements {
            needs_embedded_context: focused_part.is_some(),
            needs_image: focused_part
                .as_ref()
                .map(|part| part.source().contains("screenshot=1"))
                .unwrap_or(false),
        };
        let warm_spec = pi_launch.warm_spec();
        let manager = crate::ai::agent_chat::launch::warm_session_manager();
        if let Some(snapshot) = manager.snapshot(&pi_launch.warm_key).filter(|snapshot| {
            snapshot.state == crate::ai::agent_chat::warm_session::AgentChatWarmSessionState::Failed
        }) {
            let error = snapshot.failure_message.unwrap_or_else(|| {
                "Pi Agent Chat model warm-up failed. Retry after fixing the provider configuration."
                    .to_string()
            });
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "pi_agent_chat_warm_failed_setup",
                profile_id = %pi_launch.profile.id,
                warm_key = %pi_launch.warm_key,
                generation = snapshot.generation,
                error = %error,
            );
            self.show_pi_agent_chat_unavailable_setup_view(source_view, error, cx);
            return;
        }
        let (lease, acquire_origin) = match manager.acquire_ready_or_spawn_cold(warm_spec) {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "pi_agent_chat_acquire_or_cold_spawn_failed",
                    profile_id = %pi_launch.profile.id,
                    warm_key = %pi_launch.warm_key,
                    error = %error,
                );
                self.show_pi_agent_chat_unavailable_setup_view(source_view, error.to_string(), cx);
                return;
            }
        };

        let connection = lease.connection.clone();
        let cwd = lease.cwd.clone();
        let ui_thread_id = lease.ui_thread_id.clone();

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "pi_agent_chat_warm_acquired",
            profile_id = %pi_launch.profile.id,
            profile_name = %pi_launch.profile.name,
            warm_key = %pi_launch.warm_key,
            acquire_origin = ?acquire_origin,
            generation = lease.generation,
            ui_thread_id = %ui_thread_id,
            cwd = %cwd.display(),
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        let thread = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatThread::new(
                connection,
                permission_rx,
                crate::ai::agent_chat::ui::AgentChatThreadInit {
                    ui_thread_id,
                    cwd,
                    initial_input: agent_chat_initial_input.clone(),
                    initial_context_parts: Vec::new(),
                    display_name: pi_launch.profile.name.clone().into(),
                    profile_id: pi_launch.profile.id.clone(),
                    profile_display_name: Some(pi_launch.profile.name.clone().into()),
                    profile_icon_name: pi_launch.profile.icon_name.clone(),
                    selected_agent: None,
                    available_agents: Vec::new(),
                    launch_requirements: requirements,
                    available_models: pi_launch.available_models.clone(),
                    selected_model_id: pi_launch.selected_model_id.clone(),
                },
                cx,
            )
        });

        let view_entity = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatView::new(thread.clone(), cx)
                .with_ui_variant(request.ui_variant)
        });

        self.active_agent_chat_warm_lease = Some(lease);
        self.wire_embedded_agent_chat_footer_callbacks(&view_entity, cx);
        self.embedded_agent_chat = Some(view_entity.clone());
        self.tab_ai_harness_return_view = Some(source_view.clone());
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
        self.seed_tab_ai_apply_back_route(
            &request.source_view,
            &request.ui_snapshot,
            focused_part.as_ref(),
        );

        let view_entity_for_staging = view_entity.clone();
        view_entity_for_staging.update(cx, |view, _cx| {
            view.opened_via_transient_trigger = pending_script_list_trigger;
        });
        self.enter_embedded_agent_chat_surface(view_entity, cx);
        cx.notify();

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "pi_agent_chat_view_switched",
            profile_id = %pi_launch.profile.id,
            agent_chat_ui_variant = request.ui_variant.state_id(),
            total_ms = open_started_at.elapsed().as_millis() as u64,
        );

        let needs_deferred = self.stage_agent_chat_initial_context_parts(
            None,
            &view_entity_for_staging,
            &thread,
            focused_part,
            use_ask_anything_fallback,
            explicit_ambient_chip_label.clone(),
            auto_submit,
            pending_script_list_trigger,
            request.suppress_focused_part,
            &source_view,
            cx,
        );

        self.schedule_agent_chat_post_paint_harness_teardown(
            had_harness_session,
            open_started_at,
            cx,
        );

        if !needs_deferred {
            return;
        }

        view_entity_for_staging.update(cx, |view, _cx| {
            view.set_context_capture_pending(true);
        });

        self.spawn_agent_chat_deferred_context_staging(
            view_entity_for_staging,
            thread,
            request,
            capture_rx,
            effective_intent,
            auto_submit,
            open_started_at,
            cx,
        );
    }

    /// Defer harness termination to after first paint so the user sees the
    /// chat surface before the synchronous teardown blocks the main thread.
    fn schedule_agent_chat_post_paint_harness_teardown(
        &mut self,
        had_harness_session: bool,
        open_started_at: std::time::Instant,
        cx: &mut Context<Self>,
    ) {
        if !had_harness_session {
            return;
        }
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
                        event = "agent_chat_open_stage",
                        stage = "terminate_tab_ai_harness_session_post_paint",
                        stage_ms = stage_started_at.elapsed().as_millis() as u64,
                        total_ms = open_started_at_for_teardown.elapsed().as_millis() as u64,
                    );
                });
            });
        })
        .detach();
    }

    /// Extract a pending retry request from the current Agent Chat chat view.
    ///
    /// Returns `None` if the current view is not an `AgentChatView` or if no
    /// retry request has been queued.
    pub(super) fn take_agent_chat_retry_request_for_open(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<crate::ai::agent_chat::ui::AgentChatRetryRequest> {
        if let AppView::AgentChatView { entity } = &self.current_view {
            return entity.update(cx, |view, _cx| view.take_retry_request());
        }

        self.embedded_agent_chat
            .as_ref()
            .cloned()
            .and_then(|entity| entity.update(cx, |view, _cx| view.take_retry_request()))
    }

    /// Build the Agent Chat composer text for the first render of a new launch.
    ///
    /// ScriptList-triggered `@`, `/`, and `|` routes prefill the raw trigger so the
    /// Agent Chat handoff never paints an empty composer before the picker opens.
    pub(super) fn tab_ai_agent_chat_initial_input_for_launch(
        prompt_type: &str,
        effective_intent: Option<&str>,
        pending_script_list_trigger: Option<char>,
        force_agent_chat_surface: bool,
    ) -> Option<String> {
        if let Some(intent) = effective_intent {
            if force_agent_chat_surface {
                return Some(intent.to_string());
            }

            return Some(
                crate::ai::harness::build_tab_ai_agent_chat_initial_input_for_prompt(
                    prompt_type,
                    intent,
                )
                .text,
            );
        }

        match (prompt_type, pending_script_list_trigger) {
            ("ScriptList", Some(trigger @ ('/' | '@' | '|'))) => Some(trigger.to_string()),
            _ => None,
        }
    }

    /// Persist the current `spine_cwd` to user preferences (`ai.cwd`) so it is
    /// restored on the next app launch. Non-fatal and off the UI thread.
    pub(crate) fn persist_spine_cwd(&self) {
        let cwd = self
            .spine_cwd
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        // Every persisted cwd change is also a flow-discovery boundary:
        // kick the roster fetch for the new effective cwd immediately so
        // the main-menu Flows section reflects it without waiting for TTL.
        crate::flows::catalog::flow_catalog().refresh(&crate::flows::resolve_flow_cwd(cwd.clone()));
        std::thread::Builder::new()
            .name("persist-spine-cwd".into())
            .spawn(move || {
                let mut prefs = crate::config::load_user_preferences();
                if prefs.ai.cwd == cwd {
                    return;
                }
                prefs.ai.cwd = cwd.clone();
                if let Err(error) = crate::config::save_user_preferences(&prefs) {
                    tracing::warn!(
                        target: "script_kit::spine",
                        event = "persist_spine_cwd_failed",
                        error = %error,
                    );
                } else {
                    tracing::info!(
                        target: "script_kit::spine",
                        event = "persist_spine_cwd",
                        cwd = ?cwd,
                    );
                }
            })
            .ok();
    }

    /// The working directory to launch the agent in, derived from the Spine cwd
    /// chip. Returns `None` (use the profile/default cwd) unless the user has
    /// *explicitly* picked a cwd (revision > 0) and it is still a directory.
    ///
    /// The startup default (`~/.scriptkit`, revision 0) intentionally does not
    /// override the profile's launch cwd, so default launches keep hitting the
    /// startup-warmed session and the General profile's scratch directory.
    pub(crate) fn spine_cwd_for_agent_chat_launch(&self) -> Option<std::path::PathBuf> {
        if self.spine_cwd_revision == 0 {
            return None;
        }
        let cwd = self.spine_cwd.as_ref()?;
        if cwd.is_dir() {
            Some(cwd.clone())
        } else {
            tracing::warn!(
                target: "script_kit::spine",
                event = "spine_cwd_for_agent_chat_launch_not_a_dir",
                cwd = %cwd.display(),
                "Spine cwd is not a directory; falling back to profile cwd"
            );
            None
        }
    }

    pub(crate) fn agent_chat_hot_prewarm_enabled() -> bool {
        let disabled = std::env::var("SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM").ok();
        let enabled = std::env::var("SCRIPT_KIT_ENABLE_AGENT_CHAT_HOT_PREWARM").ok();
        agent_chat_hot_prewarm_enabled_from(disabled.as_deref(), enabled.as_deref())
    }

    /// Start warming a Pi Agent Chat session for the current Spine cwd so a
    /// later Cmd+Enter acquires a ready warm session with the correct working
    /// directory instead of missing (which would surface the "try again"
    /// toast). Invoked when the user picks a cwd and hot prewarm is explicitly
    /// enabled; idle Pi workers are otherwise deferred until first use because
    /// they can consume multiple CPU cores and starve GPUI frame delivery.
    pub(crate) fn prewarm_agent_chat_for_spine_cwd(&self, cx: &mut Context<Self>) {
        let _ = cx;
        let ai_preferences = crate::config::load_user_preferences().ai;
        self.prewarm_selected_agent_chat_profile_for_current_cwd(
            &ai_preferences,
            "prewarm_agent_chat_for_spine_cwd",
        );
    }

    pub(crate) fn prewarm_selected_agent_chat_profile_for_current_cwd(
        &self,
        ai_preferences: &crate::config::AiPreferences,
        source: &'static str,
    ) {
        if !Self::agent_chat_hot_prewarm_enabled() {
            tracing::info!(
                target: "script_kit::spine",
                event = "prewarm_selected_agent_chat_profile_skipped",
                source,
                reason = "disabled_by_default",
            );
            return;
        }

        let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        match crate::ai::agent_chat::launch::resolve_selected_pi_launch_with_cwd_override(
            ai_preferences,
            &profile_ctx,
            self.spine_cwd_for_agent_chat_launch(),
        ) {
            Ok(pi_launch) => {
                let manager = crate::ai::agent_chat::launch::warm_session_manager();
                if let Err(error) = manager.prepare_warm_background(pi_launch.warm_spec()) {
                    tracing::warn!(
                        target: "script_kit::spine",
                        event = "prewarm_selected_agent_chat_profile_failed",
                        source,
                        warm_key = %pi_launch.warm_key,
                        profile_id = %pi_launch.profile.id,
                        error = %error,
                    );
                } else {
                    tracing::info!(
                        target: "script_kit::spine",
                        event = "prewarm_selected_agent_chat_profile",
                        source,
                        warm_key = %pi_launch.warm_key,
                        profile_id = %pi_launch.profile.id,
                        cwd = %pi_launch.cwd.display(),
                    );
                }
            }
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::spine",
                    event = "prewarm_selected_agent_chat_profile_resolution_failed",
                    source,
                    error = %error,
                );
            }
        }
    }
}

#[cfg(test)]
mod hot_prewarm_tests {
    use super::agent_chat_hot_prewarm_enabled_from;

    #[test]
    fn hot_prewarm_is_opt_in_and_disable_wins() {
        assert!(!agent_chat_hot_prewarm_enabled_from(None, None));
        assert!(!agent_chat_hot_prewarm_enabled_from(None, Some("0")));
        assert!(agent_chat_hot_prewarm_enabled_from(None, Some("true")));
        assert!(agent_chat_hot_prewarm_enabled_from(None, Some("1")));
        assert!(!agent_chat_hot_prewarm_enabled_from(
            Some("true"),
            Some("true")
        ));
    }
}
