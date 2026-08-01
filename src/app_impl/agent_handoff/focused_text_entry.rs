use super::*;

struct FocusedTextFixtureConnection;

impl crate::ai::agent_chat::runtime::AgentChatConnection for FocusedTextFixtureConnection {
    fn start_turn(
        &self,
        _request: crate::ai::agent_chat::runtime::AgentChatTurnRequest,
    ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
    {
        let (tx, rx) = async_channel::bounded(2);
        let _ = tx.try_send(
            crate::ai::agent_chat::events::AgentChatEvent::AgentMessageDelta(
                "Fixture focused text output.".to_string(),
            ),
        );
        let _ = tx.try_send(crate::ai::agent_chat::events::AgentChatEvent::completed(
            "fixture",
        ));
        Ok(rx)
    }

    fn cancel_turn(&self, _ui_thread_id: String) -> crate::ai::reliability::AiAdapterResult<()> {
        Ok(())
    }

    fn prepare_session(
        &self,
        _ui_thread_id: String,
        _cwd: std::path::PathBuf,
    ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
    {
        let (_tx, rx) = async_channel::bounded(1);
        Ok(rx)
    }
}

impl ScriptListApp {
    pub(crate) fn dismiss_focused_text_agent_chat_before_recapture(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let AppView::AgentChatView { entity } = self.current_view.clone() else {
            return false;
        };

        if !entity.read(cx).has_focused_text_context() {
            return false;
        }

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_recapture_dismiss_previous_session",
        );

        self.close_tab_ai_harness_terminal_impl(
            None,
            super::TabAiHarnessCloseDisposition::CloseMainWindowStateFirst,
            cx,
        );
        true
    }

    pub(crate) fn open_focused_text_agent_chat_fixture(
        &mut self,
        text: Option<String>,
        instruction: Option<String>,
        provider: &'static str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let text = text.unwrap_or_else(|| "Hello world".to_string());
        let requested_submit = instruction
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let snapshot =
            crate::platform::accessibility::focused_text::focused_text_snapshot_for_tests(text);

        if provider == "focused_text_mock_fixture" {
            self.open_focused_text_agent_chat_mock_fixture_from_snapshot(
                snapshot,
                instruction.clone(),
                provider,
                cx,
            );
        } else {
            self.open_focused_text_agent_chat_from_snapshot(
                snapshot,
                instruction.clone(),
                provider,
                cx,
            );
        }

        let AppView::AgentChatView { entity } = self.current_view.clone() else {
            return Err("focused text fixture did not open Agent Chat".to_string());
        };

        if provider == "focused_text_mock_fixture" && requested_submit {
            let user_text = instruction
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let mut fixture_error = None;
            entity.update(cx, |chat, cx| {
                let result = chat.apply_test_fixture(
                    "idle",
                    user_text,
                    Some("Fixture focused text output.".to_string()),
                    None,
                    cx,
                );
                if let Err(error) = result {
                    fixture_error = Some(error);
                }
            });
            if let Some(error) = fixture_error {
                return Err(error);
            }
        } else if requested_submit {
            entity.update(cx, |chat, cx| {
                if let Err(error) = chat.submit_focused_text_turn(
                    crate::ai::focused_text::FocusedTextEditSemantics::Replace,
                    cx,
                    None,
                ) {
                    tracing::warn!(
                        target: "script_kit::focused_text",
                        event = "focused_text_fixture_submit_failed",
                        error = %error,
                    );
                }
            });
        }

        Ok(())
    }

    pub(crate) fn open_focused_text_agent_chat_from_focused_field_mock_fixture(
        &mut self,
        instruction: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<usize, String> {
        if std::env::var("SCRIPT_KIT_FOCUSED_TEXT_LIVE_FIXTURE").as_deref() != Ok("1") {
            return Err("SCRIPT_KIT_FOCUSED_TEXT_LIVE_FIXTURE=1 is required".to_string());
        }

        let snapshot = crate::platform::accessibility::capture_focused_text_field(
            crate::platform::accessibility::CaptureFocusedTextOptions::default(),
        )
        .map_err(|error| error.to_string())?;
        let text_length = snapshot.text.len();
        let requested_submit = instruction
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        self.open_focused_text_agent_chat_mock_fixture_from_snapshot(
            snapshot,
            instruction.clone(),
            "focused_text_live_mock_fixture",
            cx,
        );

        let AppView::AgentChatView { entity } = self.current_view.clone() else {
            return Err("focused text live fixture did not open Agent Chat".to_string());
        };

        if requested_submit {
            let user_text = instruction
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let mut fixture_error = None;
            entity.update(cx, |chat, cx| {
                let result = chat.apply_test_fixture(
                    "idle",
                    user_text,
                    Some("Fixture focused text output.".to_string()),
                    None,
                    cx,
                );
                if let Err(error) = result {
                    fixture_error = Some(error);
                }
            });
            if let Some(error) = fixture_error {
                return Err(error);
            }
        }

        Ok(text_length)
    }

    fn open_focused_text_agent_chat_mock_fixture_from_snapshot(
        &mut self,
        snapshot: crate::platform::accessibility::FocusedTextSnapshot,
        instruction: Option<String>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_agent_chat_open",
            source,
            session_id = %snapshot.session_id,
            app_name = %snapshot.app.name,
            chars = snapshot.metrics.chars,
            source_view = ?source_view,
        );

        let (_broker, permission_rx) = crate::ai::agent_chat::ui::AgentChatPermissionBroker::new();
        let thread = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatThread::new(
                std::sync::Arc::new(FocusedTextFixtureConnection),
                permission_rx,
                crate::ai::agent_chat::ui::AgentChatThreadInit {
                    ui_thread_id: "focused-text-mock-fixture".to_string(),
                    cwd: std::env::temp_dir().join("script-kit-focused-text-fixture"),
                    initial_input: instruction.clone(),
                    initial_context_parts: Vec::new(),
                    display_name: "Text".into(),
                    profile_id: crate::ai::agent_chat::profiles::BUILTIN_TEXT_PROFILE_ID
                        .to_string(),
                    profile_display_name: Some("Text".into()),
                    profile_icon_name: None,
                    selected_agent: None,
                    available_agents: Vec::new(),
                    launch_requirements:
                        crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
                    available_models: Vec::new(),
                    selected_model_id: None,
                    session_policy:
                        crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::Full,
                },
                cx,
            )
        });
        thread.update(cx, |thread, cx| {
            thread.mark_context_bootstrap_ready(cx);
        });

        let view_entity = cx.new(|cx| {
            crate::ai::agent_chat::ui::AgentChatView::new(thread, cx).with_ui_variant(
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::FocusedTextMini,
            )
        });
        self.wire_embedded_agent_chat_footer_callbacks(&view_entity, cx);
        self.embedded_agent_chat = Some(view_entity.clone());
        self.tab_ai_harness_return_view = Some(source_view);
        self.tab_ai_harness_return_focus_target = Some(self.tab_ai_return_focus_target());
        self.set_main_window_mode_state_only(
            MainWindowMode::Mini,
            cx,
            "focused_text_agent_chat_open",
        );
        view_entity.update(cx, |chat, cx| {
            if let Err(error) = chat.stage_focused_text_from_host(snapshot, None, source, cx) {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "focused_text_agent_chat_stage_failed",
                    error = %error,
                );
            }
            chat.mark_focused_text_originated_from_quick_prompt();
        });
        self.enter_embedded_agent_chat_surface(view_entity, cx);
        self.request_focus(FocusTarget::ChatPrompt, cx);
        script_kit_gpui::request_show_main_window();
        crate::window_resize::resize_to_view_sync(
            crate::window_resize::ViewType::FocusedTextMini,
            0,
        );
        cx.notify();
    }

    pub(crate) fn open_focused_text_agent_chat_from_snapshot(
        &mut self,
        snapshot: crate::platform::accessibility::FocusedTextSnapshot,
        instruction: Option<String>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_agent_chat_open",
            source,
            session_id = %snapshot.session_id,
            app_name = %snapshot.app.name,
            chars = snapshot.metrics.chars,
            source_view = ?source_view,
        );

        self.set_main_window_mode_state_only(
            MainWindowMode::Mini,
            cx,
            "focused_text_agent_chat_open",
        );

        self.begin_tab_ai_harness_entry_from_source_view(
            source_view,
            None,
            AgentChatContextPolicy::SuppressFocused,
            None,
            crate::ai::TabAiCaptureKind::DefaultContext,
            // force_agent_chat_surface: focused-text apply semantics must not route to the terminal.
            true,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::FocusedTextMini,
            agent_chat_entry::AgentChatSeedPolicy::ComposerOnly,
            cx,
        );

        let AppView::AgentChatView { entity } = self.current_view.clone() else {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_agent_chat_open_failed_no_embedded_view",
            );
            return;
        };

        entity.update(cx, |chat, cx| {
            chat.set_ui_variant(
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::FocusedTextMini,
                cx,
            );
            if let Err(error) = chat.stage_focused_text_from_host(snapshot, instruction, source, cx)
            {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "focused_text_agent_chat_stage_failed",
                    error = %error,
                );
            }
            chat.mark_focused_text_originated_from_quick_prompt();
        });
        self.request_focus(FocusTarget::ChatPrompt, cx);
        script_kit_gpui::request_show_main_window();
        crate::window_resize::resize_to_view_sync(
            crate::window_resize::ViewType::FocusedTextMini,
            0,
        );
        cx.notify();
    }

    /// Capture text for the instant rewrite flow. AX field capture first
    /// (real session, whole-field Replace); when the app is AX-opaque
    /// (Chrome, Electron) or the field reads empty, fall back to the
    /// selection/Cmd+C ladder and apply-back becomes paste-over-selection.
    pub(crate) fn capture_rewrite_snapshot(
        &self,
        source: &'static str,
    ) -> Option<crate::platform::accessibility::FocusedTextSnapshot> {
        let capture_started = std::time::Instant::now();
        match crate::platform::accessibility::capture_focused_text_field(
            crate::platform::accessibility::CaptureFocusedTextOptions::default(),
        ) {
            Ok(snapshot) if !snapshot.text.trim().is_empty() => {
                tracing::info!(
                    target: "script_kit::focused_text",
                    event = "instant_rewrite_capture_complete",
                    source,
                    session_id = %snapshot.session_id,
                    app_name = %snapshot.app.name,
                    chars = snapshot.metrics.chars,
                    elapsed_ms = capture_started.elapsed().as_millis() as u64,
                );
                return Some(snapshot);
            }
            Ok(_) => {
                tracing::info!(
                    target: "script_kit::focused_text",
                    event = "instant_rewrite_ax_capture_empty",
                    source,
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "instant_rewrite_ax_capture_failed",
                    source,
                    error = %error,
                );
            }
        }

        let captured = crate::selected_text::capture_selection_or_focused_text()
            .ok()
            .flatten()
            .filter(|captured| !captured.text.trim().is_empty())?;
        tracing::info!(
            target: "script_kit::focused_text",
            event = "instant_rewrite_capture_fallback",
            source,
            kind = ?captured.kind,
            source_app = captured.source_app.as_deref().unwrap_or(""),
            chars = captured.text.chars().count(),
            elapsed_ms = capture_started.elapsed().as_millis() as u64,
        );
        Some(
            crate::platform::accessibility::focused_text::focused_text_snapshot_for_selection_paste(
                captured.text,
                captured.source_app,
            ),
        )
    }

    /// Instant rewrite flow (rewrite hotkey / "Rewrite selection" chip):
    /// capture the focused field or selection, open the FocusedTextMini, and
    /// immediately stream the three rewrite variations with the default
    /// instruction — no typing needed. When nothing is capturable, show a HUD
    /// instead of converting (shrinking) the main window into an error mini.
    pub(crate) fn open_instant_rewrite_mini(
        &mut self,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_focused_text_agent_chat_before_recapture(cx);
        match self.capture_rewrite_snapshot(source) {
            Some(snapshot) => self.open_focused_text_rewrite_from_snapshot(snapshot, source, cx),
            None => {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "instant_rewrite_nothing_to_rewrite",
                    source,
                );
                self.show_hud(
                    "Nothing to rewrite — select text or focus a text field".to_string(),
                    Some(2200),
                    cx,
                );
            }
        }
    }

    /// Open the selected-text context in the focused-text surface without
    /// submitting a rewrite. Context-chip body actions are inspect/open actions;
    /// only an explicit submit inside the destination surface may start AI work.
    pub(crate) fn open_selection_context_details(
        &mut self,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        match self.capture_rewrite_snapshot(source) {
            Some(snapshot) => {
                self.open_focused_text_agent_chat_from_snapshot(snapshot, None, source, cx)
            }
            None => self.show_hud(
                "Selected text is no longer available".to_string(),
                Some(2200),
                cx,
            ),
        }
    }

    /// Open the FocusedTextMini for `snapshot` and fire the three rewrite
    /// variations immediately with [`DEFAULT_REWRITE_INSTRUCTION`]. If the
    /// instant submit cannot start (e.g. context bootstrap still preparing),
    /// the instruction stays staged in the input so Enter retries it.
    pub(crate) fn open_focused_text_rewrite_from_snapshot(
        &mut self,
        snapshot: crate::platform::accessibility::FocusedTextSnapshot,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.open_focused_text_agent_chat_from_snapshot(
            snapshot,
            Some(crate::ai::focused_text::DEFAULT_REWRITE_INSTRUCTION.to_string()),
            source,
            cx,
        );

        let AppView::AgentChatView { entity } = self.current_view.clone() else {
            return;
        };
        entity.update(cx, |chat, cx| {
            if let Err(error) = chat.submit_instant_rewrite(cx) {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "instant_rewrite_submit_failed",
                    source,
                    error = %error,
                );
            }
        });
    }

    pub(crate) fn open_focused_text_agent_chat_from_capture_failure(
        &mut self,
        error: crate::platform::accessibility::FocusedTextError,
        instruction: Option<String>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        let reason_code = error.reason_code();
        let source_view = self.current_view.clone();
        self.seed_agent_chat_return_origin_for_view(&source_view);

        tracing::warn!(
            target: "script_kit::focused_text",
            event = "focused_text_agent_chat_open_capture_failed",
            source,
            reason_code,
            error = %error,
            source_view = ?source_view,
        );

        self.set_main_window_mode_state_only(
            MainWindowMode::Mini,
            cx,
            "focused_text_agent_chat_open_capture_failed",
        );

        self.begin_tab_ai_harness_entry_from_source_view(
            source_view,
            None,
            AgentChatContextPolicy::SuppressFocused,
            None,
            crate::ai::TabAiCaptureKind::DefaultContext,
            true,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::FocusedTextMini,
            agent_chat_entry::AgentChatSeedPolicy::ComposerOnly,
            cx,
        );

        let AppView::AgentChatView { entity } = self.current_view.clone() else {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_agent_chat_open_failed_no_embedded_view",
            );
            return;
        };

        entity.update(cx, |chat, cx| {
            chat.set_ui_variant(
                crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::FocusedTextMini,
                cx,
            );
            if let Err(stage_error) = chat.stage_focused_text_capture_failure_from_host(
                reason_code,
                instruction,
                source,
                cx,
            ) {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "focused_text_agent_chat_stage_failed",
                    error = %stage_error,
                );
            }
            chat.mark_focused_text_originated_from_quick_prompt();
        });
        self.request_focus(FocusTarget::ChatPrompt, cx);
        script_kit_gpui::request_show_main_window();
        crate::window_resize::resize_to_view_sync(
            crate::window_resize::ViewType::FocusedTextMini,
            0,
        );
        cx.notify();
    }
}
