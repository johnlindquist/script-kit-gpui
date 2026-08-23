impl ScriptListApp {
    fn capture_flow_session_draft(&mut self, session_id: u64) {
        let draft = self.filter_text.clone();
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let meta = &mut self.conversations.flow_sessions[index].0;
        if meta.selected_is_archived() || meta.active_draft == draft {
            return;
        }
        meta.active_draft = draft;
        meta.draft_generation = meta.draft_generation.saturating_add(1);
    }

    fn restore_flow_session_draft(&mut self, session_id: u64) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let draft = self.conversations.flow_sessions[index]
            .0
            .active_draft
            .clone();
        self.filter_text = draft;
        self.pending_filter_sync = true;
    }

    fn clear_flow_session_draft(&mut self, session_id: u64) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let meta = &mut self.conversations.flow_sessions[index].0;
        if !meta.active_draft.is_empty() {
            meta.active_draft.clear();
            meta.draft_generation = meta.draft_generation.saturating_add(1);
        }
        self.filter_text.clear();
        self.pending_filter_sync = true;
    }

    fn flow_input_return_state(&self, cx: &App) -> FlowInputReturnState {
        FlowInputReturnState {
            value: self.filter_text.clone(),
            selection: self.gpui_input_state.read(cx).selection(),
            focused_input: self.focused_input,
            pending_focus: self.pending_focus,
        }
    }

    fn capture_flow_conversation_return_route(
        &mut self,
        cx: &mut Context<Self>,
    ) -> FlowConversationReturnRoute {
        let view = self.current_view.clone();
        let input = self.flow_input_return_state(cx);
        match &view {
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                let selected_semantic_id = self
                    .flow_desk_rows(filter)
                    .get(*selected_index)
                    .map(|row| self.flow_desk_row_descriptor(row).semantic_id);
                FlowConversationReturnRoute::Desk(FlowDeskReturnState {
                    view,
                    selected_semantic_id,
                    input,
                })
            }
            AppView::FlowSessionView { .. } => self.flow_session_return_route.clone(),
            _ if self.opened_from_main_menu => {
                let interaction = self.main_menu_interaction_snapshot();
                FlowConversationReturnRoute::Main(FlowMainReturnState {
                    view,
                    raw_filter_text: self.filter_text.clone(),
                    computed_filter_text: self.computed_filter_text.clone(),
                    interaction,
                    input,
                    pending_placeholder: self.pending_placeholder.clone(),
                })
            }
            _ => FlowConversationReturnRoute::Direct,
        }
    }

    fn restore_flow_input_return_state(
        &mut self,
        state: &FlowInputReturnState,
        placeholder: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.filter_text = state.value.clone();
        self.pending_filter_sync = false;
        let value = state.value.clone();
        let selection = state.selection.clone();
        let placeholder = placeholder.to_string();
        self.gpui_input_state.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            input.set_selection(selection.start, selection.end, window, cx);
            input.set_placeholder(placeholder, window, cx);
        });
        self.focused_input = state.focused_input;
        self.pending_focus = state.pending_focus.or_else(|| {
            (state.focused_input == FocusedInput::MainFilter).then_some(FocusTarget::MainFilter)
        });
    }

    pub(crate) fn set_flow_conversation_return_route_fixture(
        &mut self,
        origin: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !matches!(self.current_view, AppView::FlowSessionView { .. }) {
            return Err("FlowSession view is not active".to_string());
        }
        self.flow_session_return_route = match origin {
            "desk" => {
                if matches!(
                    self.flow_session_return_route,
                    FlowConversationReturnRoute::Desk(_)
                ) {
                    return Ok(());
                }
                return Err("Flow Desk route was not captured by the real entry path".to_string());
            }
            "main" => {
                let value = "c06-main-route".to_string();
                FlowConversationReturnRoute::Main(FlowMainReturnState {
                    view: AppView::ScriptList,
                    raw_filter_text: value.clone(),
                    computed_filter_text: value.clone(),
                    interaction: self.main_menu_interaction_snapshot(),
                    input: FlowInputReturnState {
                        value,
                        selection: 0..0,
                        focused_input: FocusedInput::MainFilter,
                        pending_focus: Some(FocusTarget::MainFilter),
                    },
                    pending_placeholder: Some(crate::ROOT_LAUNCHER_PLACEHOLDER.to_string()),
                })
            }
            "direct" => FlowConversationReturnRoute::Direct,
            other => return Err(format!("unsupported Flow return-route fixture {other:?}")),
        };
        cx.notify();
        Ok(())
    }

    pub(crate) fn apply_flow_conversation_test_fixture(
        &mut self,
        phase: &str,
        user_text: Option<String>,
        assistant_text: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let AppView::FlowSessionView { session_id } = self.current_view else {
            return Err("FlowSession view is not active".to_string());
        };
        let Some(index) = self.flow_session_index(session_id) else {
            return Err("FlowSession entity is not active".to_string());
        };
        let entity = self.conversations.flow_sessions[index].1.clone();
        entity.update(cx, |chat, cx| {
            chat.apply_transcript_geometry_fixture(
                phase,
                user_text.clone(),
                assistant_text.clone(),
                cx,
            )
        })?;

        // Flow's host-level Copy Last Response intentionally reads the durable
        // session model, while per-turn copy reads ChatPrompt. Keep this narrow
        // runtime fixture truthful at both owners so the real ⇧⌘C path can be
        // exercised without a provider run.
        if matches!(phase, "c06Completed" | "c06-completed") {
            let user = user_text.unwrap_or_else(|| "C06 accepted request".to_string());
            let assistant = assistant_text.unwrap_or_else(|| {
                " C06 synthetic answer\nsecond line with trailing spaces \n".to_string()
            });
            let meta = &mut self.conversations.flow_sessions[index].0;
            meta.turns = vec![
                crate::flows::session::SessionTurn {
                    user,
                    assistant,
                    outcome: crate::flows::session::PersistedTurnOutcome::Ok,
                    failure: None,
                },
                crate::flows::session::SessionTurn {
                    user: String::new(),
                    assistant: " \n\t ".to_string(),
                    outcome: crate::flows::session::PersistedTurnOutcome::Ok,
                    failure: None,
                },
            ];
            meta.active_turn = None;
            meta.state = crate::flows::session::SessionState::NeedsYou;
        }
        cx.notify();
        Ok(())
    }

    fn apply_flow_conversation_return_route(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.flow_session_return_route.clone() {
            FlowConversationReturnRoute::Desk(state) => {
                let mut view = state.view.clone();
                if let AppView::FlowUxView {
                    filter,
                    selected_index,
                    ..
                } = &mut view
                {
                    if let Some(semantic_id) = state.selected_semantic_id.as_deref() {
                        if let Some(restored_index) =
                            self.flow_desk_rows(filter).iter().position(|row| {
                                self.flow_desk_row_descriptor(row).semantic_id == semantic_id
                            })
                        {
                            *selected_index = restored_index;
                        }
                    }
                    self.flow_ux_scroll_handle
                        .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                }
                self.current_view = view;
                self.pending_placeholder = Some("Search flows...".to_string());
                self.restore_flow_input_return_state(&state.input, "Search flows...", window, cx);
                cx.notify();
            }
            FlowConversationReturnRoute::Main(state) => {
                self.current_view = state.view.clone();
                self.filter_text = state.raw_filter_text.clone();
                self.computed_filter_text = state.computed_filter_text.clone();
                self.pending_placeholder = state.pending_placeholder.clone();
                self.invalidate_grouped_cache();
                let _ = self
                    .restore_main_menu_selection_from_snapshot(state.interaction.selection.clone());
                self.sync_list_state_for_filter_replacement(
                    MainListReplacementPolicy::PreserveViewport(state.interaction.viewport.clone()),
                );
                self.restore_flow_input_return_state(
                    &state.input,
                    crate::ROOT_LAUNCHER_PLACEHOLDER,
                    window,
                    cx,
                );
                self.opened_from_main_menu = false;
                self.clear_actions_popup_state();
                self.update_window_size_deferred(window, cx);
                cx.notify();
            }
            FlowConversationReturnRoute::Direct => self.close_and_reset_window(cx),
        }
    }

    pub(crate) fn open_flow_session(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let friendly = self.conversations.flow_sessions[index]
            .0
            .friendly_name
            .clone();
        // Explicit open/resume is semantic activity (Oracle step 5): returning
        // to an older session moves it back to the top of Active Flows.
        self.conversations.flow_sessions[index].0.touch_now();
        self.flow_session_return_route = self.capture_flow_conversation_return_route(cx);
        self.current_view = AppView::FlowSessionView { session_id };
        // The main input is only the visible projection of the active
        // session-owned draft. Archive browsing keeps the draft hidden.
        if self.conversations.flow_sessions[index]
            .0
            .selected_is_archived()
        {
            self.filter_text.clear();
            self.pending_filter_sync = true;
        } else {
            self.restore_flow_session_draft(session_id);
        }
        self.pending_placeholder = Some(format!("Message {friendly}…"));
        self.focused_input = FocusedInput::MainFilter;
        self.pending_focus = Some(FocusTarget::MainFilter);
        cx.spawn(async move |_this, _cx| {
            crate::window_resize::resize_to_view_sync(
                crate::window_resize::ViewType::MainWindow,
                0,
            );
        })
        .detach();
        cx.notify();
    }

    /// Leave the session view without touching the process. The session
    /// stays in `flow_sessions` and reappears under Active.
    ///
    /// ESCAPE LADDER CONTRACT: Escape returns exactly ONE step, to the
    /// surface the session was actually entered from. Desk-entered sessions
    /// return to the desk; main-menu-launched (and every other) session
    /// routes through `go_back_or_close`, so the next Escape on an empty
    /// main menu hides the window. Detouring through a surface the user
    /// never visited reads as a swallowed Escape — locked by
    /// `flow_session_escape_origin` tests and
    /// `scripts/agentic/flow-session-escape-ladder-probe.ts`.
    pub(crate) fn background_flow_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let AppView::FlowSessionView { session_id } = self.current_view else {
            return;
        };
        self.capture_flow_session_draft(session_id);
        let return_route = flow_conversation_return_route_kind(&self.flow_session_return_route);
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_session_backgrounded",
            session_id,
            return_route,
            "Backgrounding flow session (process stays alive)"
        );
        self.apply_flow_conversation_return_route(window, cx);
    }

    fn render_flow_selected_transcript(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let turns = self.conversations.flow_sessions[index]
            .0
            .selected_turns()
            .to_vec();
        let entity = self.conversations.flow_sessions[index].1.clone();
        entity.update(cx, |chat, cx| {
            chat.clear_messages(cx);
            for (turn_index, turn) in turns.iter().enumerate() {
                chat.add_message(
                    crate::protocol::ChatPromptMessage::user(turn.user.clone()),
                    cx,
                );
                let display = flow_turn_display_assistant(turn);
                let failed = turn.outcome == crate::flows::session::PersistedTurnOutcome::Failed;
                if !display.is_empty() || failed {
                    chat.add_message(
                        crate::protocol::ChatPromptMessage::assistant(display)
                            .with_id(format!("flow-{session_id}-selected-turn-{turn_index}")),
                        cx,
                    );
                }
            }
        });
        cx.notify();
    }

    pub(crate) fn show_flow_archive(
        &mut self,
        session_id: u64,
        archive_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.flow_session_index(session_id) else {
            return false;
        };
        if self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some()
        {
            return false;
        }
        self.capture_flow_session_draft(session_id);
        if !self.conversations.flow_sessions[index]
            .0
            .select_archive(archive_id)
        {
            return false;
        }
        self.filter_text.clear();
        self.pending_filter_sync = true;
        self.render_flow_selected_transcript(session_id, cx);
        true
    }

    pub(crate) fn show_current_flow_conversation(
        &mut self,
        session_id: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        self.conversations.flow_sessions[index].0.select_active();
        self.render_flow_selected_transcript(session_id, cx);
        self.restore_flow_session_draft(session_id);
    }

    pub(crate) fn continue_flow_archive_as_new(
        &mut self,
        session_id: u64,
        archive_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.flow_session_index(session_id) else {
            return false;
        };
        if self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some()
            || !self.conversations.flow_sessions[index]
                .0
                .active_draft
                .is_empty()
            || !self.conversations.flow_sessions[index]
                .0
                .continue_archive_as_new(archive_id)
        {
            return false;
        }
        crate::flows::codex_client::codex_app_server().forget_session(session_id);
        let meta = &mut self.conversations.flow_sessions[index].0;
        meta.thread_ready = false;
        meta.needs_rethread = true;
        meta.runtime_generation = meta.runtime_generation.saturating_add(1);
        let snapshot = meta.next_persisted_snapshot();
        crate::flows::session::conversation_store().persist_snapshot(snapshot);
        self.render_flow_selected_transcript(session_id, cx);
        true
    }

    pub(crate) fn delete_selected_flow_conversation(
        &mut self,
        session_id: u64,
        _confirmed: ConfirmedFlowThreadDeletion,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.flow_session_index(session_id) else {
            return false;
        };
        if self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some()
        {
            return false;
        }
        let deleting_active = !self.conversations.flow_sessions[index]
            .0
            .selected_is_archived();
        let deleted = self.conversations.flow_sessions[index]
            .0
            .delete_selected_thread();
        if deleting_active {
            crate::flows::codex_client::codex_app_server().forget_session(session_id);
            let meta = &mut self.conversations.flow_sessions[index].0;
            meta.thread_ready = false;
            meta.needs_rethread = true;
            meta.runtime_generation = meta.runtime_generation.saturating_add(1);
            if !meta.active_draft.is_empty() {
                meta.active_draft.clear();
                meta.draft_generation = meta.draft_generation.saturating_add(1);
            }
            meta.reliability = crate::flows::session::FlowReliability::new(
                &meta.flow_id,
                &meta.flow_path,
                &meta.engine,
            );
        }
        let snapshot = self.conversations.flow_sessions[index]
            .0
            .next_persisted_snapshot();
        crate::flows::session::conversation_store().persist_selected_deletion(snapshot, deleted.id);
        self.render_flow_selected_transcript(session_id, cx);
        if deleting_active {
            self.clear_flow_session_draft(session_id);
        } else {
            self.restore_flow_session_draft(session_id);
        }
        true
    }

    /// Explicit stop (⌘K verb): cancel the in-flight turn only. The
    /// conversation survives and the composer stays usable.
    /// Put a flow session on a FRESH protocol thread.
    ///
    /// The one owner of both re-thread entry points: the failure-recovery
    /// "Start a new thread" action and the user's "New Conversation". They
    /// share the transport bookkeeping and differ only on whether the
    /// transcript survives — expressed as
    /// [`FlowConversationResetCause::preserves_transcript`], not as duplicated
    /// branches at two call sites.
    ///
    /// Returns `false` when the reset was refused, so a caller can decline to
    /// clear a composer or close a popup for a reset that did not happen.
    ///
    /// Transaction order matters. The only step that can refuse is the active
    /// turn guard, so it runs FIRST, before any mutation. Everything after it
    /// succeeds unconditionally, which is what makes "the old conversation is
    /// intact if this returns false" true rather than aspirational.
    pub(crate) fn start_fresh_flow_conversation(
        &mut self,
        session_id: u64,
        cause: crate::flows::session::FlowConversationResetCause,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::flows::session::{
            resolve_flow_conversation_reset_guard, FlowConversationResetGuard,
        };

        let Some(index) = self.flow_session_index(session_id) else {
            return false;
        };

        let has_active_turn = self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some();
        if matches!(
            resolve_flow_conversation_reset_guard(cause, has_active_turn),
            FlowConversationResetGuard::BlockedByActiveTurn
        ) {
            // Neutral and informational. A running turn is not an error, and
            // this must never read as though something was cancelled — the
            // point of refusing is that nothing was.
            self.toast_manager.push(
                crate::components::toast::Toast::info(
                    "Stop the current turn before starting a new conversation".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(2500)),
            );
            cx.notify();
            tracing::info!(
                target: "script_kit::flows",
                event = "flow_conversation_reset_refused",
                session_id,
                cause = ?cause,
                reason = "active_turn",
                "Flow conversation reset refused"
            );
            return false;
        }

        // Shared transport bookkeeping: the next submit re-resolves the flow
        // contract and lands on a fresh thread.
        {
            let meta = &mut self.conversations.flow_sessions[index].0;
            meta.thread_ready = false;
            meta.needs_rethread = true;
        }

        if !cause.preserves_transcript() {
            let entity = self.conversations.flow_sessions[index].1.clone();
            // Drop the engine-side thread as well. Without this the fresh
            // thread would be started against a server session that still
            // holds the old conversation, and "new conversation" would be a
            // UI-only illusion.
            crate::flows::codex_client::codex_app_server().forget_session(session_id);
            entity.update(cx, |chat, cx| {
                chat.clear_messages(cx);
            });
            let (flow_id, flow_path, engine, snapshot) = {
                let meta = &mut self.conversations.flow_sessions[index].0;
                meta.archive_active_and_start_empty();
                meta.state = crate::flows::session::SessionState::NeedsYou;
                meta.thread_ready = false;
                meta.needs_rethread = true;
                meta.runtime_generation = meta.runtime_generation.saturating_add(1);
                if !meta.active_draft.is_empty() {
                    meta.active_draft.clear();
                    meta.draft_generation = meta.draft_generation.saturating_add(1);
                }
                (
                    meta.flow_id.clone(),
                    meta.flow_path.clone(),
                    meta.engine.clone(),
                    meta.next_persisted_snapshot(),
                )
            };
            // Recovery state is per-conversation. Carrying the old failure
            // forward would leave a recovery card offering to repair a
            // conversation that no longer exists.
            self.conversations.flow_sessions[index].0.reliability =
                crate::flows::session::FlowReliability::new(&flow_id, &flow_path, &engine);
            // Persist the empty ACTIVE thread plus immutable archive through
            // the FIFO. Empty active state is real conversation metadata, not
            // a deletion signal.
            crate::flows::session::conversation_store().persist_snapshot(snapshot);
            self.clear_flow_session_draft(session_id);
        }

        tracing::info!(
            target: "script_kit::flows",
            event = "flow_conversation_reset",
            session_id,
            cause = ?cause,
            preserved_transcript = cause.preserves_transcript(),
            "Flow conversation re-threaded"
        );
        cx.notify();
        true
    }

    /// Copy the newest assistant answer in a flow session to the clipboard.
    ///
    /// The single owner for BOTH the ⌘K `Copy Last Response` action and the
    /// ⇧⌘C chord it advertises. Agent Chat's equivalent lives at
    /// `ai/agent_chat/ui/view.rs` (`Cmd+Shift+C`); the two surfaces must feel
    /// identical, and identical here means one rule about what counts as an
    /// answer — [`resolve_last_copyable_response`] — not two lookups that
    /// happen to agree today.
    ///
    /// Returns `true` when something reached the clipboard, so a caller can
    /// tell a real copy from the empty-transcript case.
    pub(crate) fn try_handle_flow_session_copy_shortcut(
        &mut self,
        key: &str,
        platform: bool,
        shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let AppView::FlowSessionView { session_id } = self.current_view else {
            return false;
        };
        let action = resolve_flow_session_key_action(
            key,
            platform,
            shift,
            self.flow_conversation_command_facts(session_id),
            self.show_actions_popup,
        );
        if action != FlowSessionKeyAction::CopyLastResponse {
            return false;
        }
        self.copy_flow_session_last_response(session_id, cx);
        true
    }

    pub(crate) fn copy_flow_session_last_response(
        &mut self,
        session_id: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let response = self
            .conversations
            .flow_sessions
            .iter()
            .find(|(meta, _)| meta.id == session_id)
            .and_then(|(meta, _)| {
                crate::flows::session::resolve_last_copyable_response(
                    meta.turns.iter().map(|turn| turn.assistant.as_str()),
                )
                .map(|assistant| assistant.to_string())
            });

        match response {
            Some(response) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(response));
                true
            }
            None => {
                // Neutral, not an error: an unanswered conversation is a normal
                // state. What must NOT happen is silence — a chord that writes
                // nothing and says nothing is indistinguishable from a chord
                // that is not bound at all, which is the exact bug that left
                // ⇧⌘C advertised-but-dead in this surface for a release.
                self.toast_manager.push(
                    crate::components::toast::Toast::info(
                        "No response to copy yet".to_string(),
                        &self.theme,
                    )
                    .duration_ms(Some(1500)),
                );
                false
            }
        }
    }

    pub(crate) fn stop_flow_session(&mut self, session_id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        let Some(active) = self.conversations.flow_sessions[index]
            .0
            .active_turn
            .clone()
        else {
            return;
        };
        match self.conversations.flow_sessions[index].0.transport {
            crate::flows::session::SessionTransport::CodexThread => {
                crate::flows::codex_client::codex_app_server().interrupt(session_id);
                // turn/completed {status: interrupted} settles the turn.
            }
            crate::flows::session::SessionTransport::MdflowTurns => {
                if let Some(run_id) = active.run_id {
                    crate::flows::runner::cancel_run(run_id);
                    // The registry's Cancelled phase settles the turn.
                }
            }
        }
        cx.notify();
    }

    /// Stop and forget only the runtime. Transcript, draft, archives, and
    /// persistence survive; active work settles before the runtime is forgotten.
    pub(crate) fn terminate_flow_session(
        &mut self,
        session_id: u64,
        _confirmed: ConfirmedFlowRuntimeTermination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.flow_session_index(session_id) else {
            return;
        };
        self.capture_flow_session_draft(session_id);
        if self.conversations.flow_sessions[index]
            .0
            .active_turn
            .is_some()
        {
            // Termination waits for the authoritative terminal event. The turn
            // settles as Stopped, persistence is updated, and only then is the
            // runtime forgotten.
            self.conversations.flow_sessions[index]
                .0
                .pending_runtime_termination = true;
            self.stop_flow_session(session_id, cx);
            return;
        }

        crate::flows::codex_client::codex_app_server().forget_session(session_id);
        let meta = &mut self.conversations.flow_sessions[index].0;
        meta.thread_ready = false;
        meta.needs_rethread = true;
        meta.pending_runtime_termination = false;
        meta.runtime_generation = meta.runtime_generation.saturating_add(1);
        crate::flows::session::conversation_store()
            .persist_snapshot(meta.next_persisted_snapshot());
        self.toast_manager.push(
            crate::components::toast::Toast::success(
                "Runtime terminated — conversation history is preserved".to_string(),
                &self.theme,
            )
            .duration_ms(Some(1800)),
        );
        let _ = window;
        cx.notify();
    }

    /// Run once in the background via the run registry (`--events`).
    pub(crate) fn flow_desk_run_once(
        &mut self,
        flow: &crate::flows::model::FlowDescriptor,
        cx: &mut Context<Self>,
    ) -> u64 {
        let cwd = self.flow_ux_cwd();
        crate::flows::remember_flow_cwd(&cwd);
        let run_id = crate::flows::runner::launch_flow(
            &flow.id,
            &flow.name,
            &flow.path,
            &cwd,
            crate::flows::model::FlowUxVariant::Flash,
            crate::flows::model::EngagementMode::Background,
            Vec::new(),
            std::time::Instant::now(),
            false,
        );
        self.start_flow_ux_tick(cx);
        self.toast_manager.push(
            crate::components::toast::Toast::success(
                format!(
                    "{} running in background — watch it in the desk list",
                    flow.friendly_name()
                ),
                &self.theme,
            )
            .duration_ms(Some(1800)),
        );
        cx.notify();
        run_id
    }

    /// Start the plain-English creation path. `md create` is a genuinely
    /// interactive CLI wizard, so it runs in the shared Quick Terminal
    /// (honest transport) rather than being faked into a chat surface.
    pub(crate) fn start_flow_create_session(&mut self, cx: &mut Context<Self>) {
        // Pre-check: opening a terminal that immediately fails with
        // "command not found" is a dead end — point at the install
        // affordance instead.
        if crate::flows::catalog::mdflow_binary().is_none() {
            self.toast_manager.push(
                crate::components::toast::Toast::error(
                    "mdflow isn't installed — use the Install mdflow row first".to_string(),
                    &self.theme,
                )
                .duration_ms(Some(3000)),
            );
            cx.notify();
            return;
        }
        let cwd = self.flow_ux_cwd();
        tracing::info!(
            target: "script_kit::flows",
            event = "flow_create_open",
            "Opening md create in Quick Terminal"
        );
        self.open_quick_terminal_with_command(
            Some(std::path::PathBuf::from(cwd)),
            "md create".to_string(),
            cx,
        );
    }

    // ------------------------------------------------------------------
    // Tab flow router entry (from the main menu input)
    // ------------------------------------------------------------------

    /// Route free text typed in the main menu to a flow (Tab). Confident →
    /// start the conversation; the text rides along as the first message
    /// ONLY when it reads like a request rather than the flow's own name
    /// (a lookup query like "githu" must never become the agent's task).
    /// Otherwise → open the desk with the text as the filter so the user
    /// picks (the Create Flow row is always present for the no-match case).
    ///
    /// 2026-07-10: no longer wired to Tab — Tab-with-text is Quick AI again
    /// (`open_quick_ai_from_launcher`); flows stay reachable as main-menu rows.
    /// Kept for a future explicit flow-routing entry point.
    #[allow(dead_code)]
    pub(crate) fn route_text_to_flow(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let corpus = self.flow_desk_corpus();
        let decision = crate::flows::router::route(&text, &corpus);
        match decision {
            crate::flows::router::RouteDecision::AutoStart { flow } => {
                let trimmed = text.trim();
                let lowered = trimmed.to_lowercase();
                let is_name_lookup = !trimmed.contains(' ')
                    || flow.name.to_lowercase() == lowered
                    || flow.friendly_name().to_lowercase() == lowered;
                let first_message = (!is_name_lookup).then(|| trimmed.to_string());
                tracing::info!(
                    target: "script_kit::flows",
                    event = "flow_router_auto_start",
                    flow_id = %flow.id,
                    query_len = text.len(),
                    carries_message = first_message.is_some(),
                    "Tab router: confident match, starting conversation"
                );
                self.resume_or_start_flow_session(&flow, first_message, cx);
            }
            crate::flows::router::RouteDecision::Candidates { .. }
            | crate::flows::router::RouteDecision::NoMatch => {
                tracing::info!(
                    target: "script_kit::flows",
                    event = "flow_router_candidates",
                    query_len = text.len(),
                    "Tab router: opening desk with candidates"
                );
                let cwd = self.flow_ux_cwd();
                crate::flows::catalog::flow_catalog().refresh(&cwd);
                self.open_builtin_filterable_view(
                    AppView::FlowUxView {
                        variant: crate::flows::model::FlowUxVariant::Flash,
                        filter: text.clone(),
                        selected_index: 0,
                        inline_run: None,
                    },
                    "Search flows...",
                    false,
                    cx,
                );
                // Seed the visible input with the routed text so the desk
                // filter and the header input agree (cwd-pick pattern).
                self.suppress_filter_events = true;
                self.gpui_input_state.update(cx, |state, cx| {
                    state.set_value(text.clone(), window, cx);
                    let len = text.len();
                    state.set_selection(len, len, window, cx);
                });
                self.suppress_filter_events = false;
                self.start_flow_ux_tick(cx);
            }
        }
    }
}
