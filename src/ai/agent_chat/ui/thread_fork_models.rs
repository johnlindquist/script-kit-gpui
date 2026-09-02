impl AgentChatThread {
    /// Fire-and-forget: ask the Agent Chat worker to create-or-reuse the session for
    /// this thread and emit a fresh `ModelsAvailable` event. Called when the
    /// user invokes the actions dialog so the Change Model picker reflects the
    /// agent's live catalog (including models released after the hardcoded
    /// fallback was written).
    ///
    /// If the worker is unreachable the call is a no-op; the picker will fall
    /// back to whatever `available_models` already held.
    /// User messages the live session can rewind to, in conversation order.
    pub(crate) fn fork_points(&self) -> &[super::events::AgentChatForkPoint] {
        &self.fork_points
    }

    /// Resolve the Pi fork point for a transcript user message. Primary mapping
    /// is conversation ordinal: nth visible user message maps to nth Pi fork
    /// point. If the local transcript and Pi fork list are out of sync, fall
    /// back to exact user-message text matching.
    pub(crate) fn fork_point_for_message_id<'a>(
        messages: &[AgentChatThreadMessage],
        fork_points: &'a [super::events::AgentChatForkPoint],
        message_id: u64,
    ) -> Option<&'a super::events::AgentChatForkPoint> {
        let user_messages: Vec<&AgentChatThreadMessage> = messages
            .iter()
            .filter(|message| matches!(message.role, AgentChatThreadMessageRole::User))
            .collect();
        let user_ordinal = user_messages
            .iter()
            .position(|message| message.id == message_id)?;

        if user_messages.len() == fork_points.len() {
            return fork_points.get(user_ordinal);
        }

        let user_text = user_messages[user_ordinal].body.as_ref();
        fork_points.iter().find(|point| point.text == user_text)
    }

    /// Refresh the rewindable user-message list from the agent session.
    /// No-op (with a debug log) for connections without rewind support.
    pub(crate) fn refresh_fork_points(&mut self, cx: &mut Context<Self>) {
        let rx = match self.connection.fork_points() {
            Ok(rx) => rx,
            Err(error) => {
                let safe_error = crate::logging::log_private_user_value(&error.to_string());
                tracing::debug!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_fork_points_unsupported",
                    error_bytes = safe_error.raw_bytes,
                    error_sha256 = %safe_error.sha256,
                );
                return;
            }
        };
        self.spawn_fork_event_task(rx, "fork_points", cx);
    }

    /// Rewind the session to just before the given user message. On
    /// completion the transcript truncates at that message and the composer
    /// is prefilled with its text for editing. Rejected while a turn is
    /// streaming or another rewind is in flight.
    pub(crate) fn fork_to_message(&mut self, entry_id: &str, cx: &mut Context<Self>) -> bool {
        if matches!(
            self.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_fork_rejected_busy",
                status = ?self.status,
            );
            return false;
        }
        if self.pending_fork_ordinal.is_some() {
            return false;
        }
        let Some(ordinal) = self
            .fork_points
            .iter()
            .position(|point| point.entry_id == entry_id)
        else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_fork_unknown_entry",
                entry_id,
            );
            return false;
        };
        let rx = match self.connection.fork_to_entry(entry_id.to_string()) {
            Ok(rx) => rx,
            Err(error) => {
                let safe_error = crate::logging::log_private_user_value(&error.to_string());
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_fork_request_failed",
                    error_bytes = safe_error.raw_bytes,
                    error_sha256 = %safe_error.sha256,
                );
                return false;
            }
        };
        self.pending_fork_ordinal = Some(ordinal);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_fork_requested",
            entry_id,
            ordinal,
        );
        self.spawn_fork_event_task(rx, "fork", cx);
        self.notify_semantic_change(cx);
        true
    }

    /// Pump fork RPC responses into `apply_event`, downgrading failures to a
    /// system note: a failed background refresh or rewind must not flip the
    /// thread into the turn-failure error state.
    fn spawn_fork_event_task(
        &self,
        rx: AgentChatEventRx,
        context_label: &'static str,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Ok(event) = rx.recv().await {
                let Some(weak) = entity.upgrade() else {
                    break;
                };
                cx.update(|cx| {
                    weak.update(cx, |this, cx| {
                        let is_fork_event = matches!(
                            event,
                            AgentChatEvent::ForkPointsAvailable { .. }
                                | AgentChatEvent::ForkCompleted { .. }
                        );
                        if is_fork_event {
                            this.apply_event(event, cx);
                        } else if let AgentChatEvent::TurnFailed { failure } = event {
                            tracing::warn!(
                                target: "script_kit::tab_ai",
                                event = "agent_chat_fork_rpc_failed",
                                context = context_label,
                                failure_code = ?failure.failure.code,
                                diagnostic_fingerprint = ?failure.failure.diagnostic.as_ref().map(|d| &d.fingerprint.0),
                            );
                            if this.apply_fork_failed() {
                                // The runtime rejected the rewind, so the existing
                                // branch remains authoritative. Preserve transcript,
                                // draft, selection, copy target, and immutable Retry
                                // payload exactly; only release the in-flight guard.
                                this.notify_semantic_change(cx);
                            }
                        }
                    });
                });
            }
        })
        .detach();
    }

    /// Release a rejected rewind without mutating the authoritative branch.
    /// The request guard is transport state; transcript, draft, selection,
    /// copy target, and immutable Retry payload remain byte-for-byte intact.
    fn apply_fork_failed(&mut self) -> bool {
        self.pending_fork_ordinal.take().is_some()
    }

    /// Commit the state mutation for a runtime-accepted rewind. Keeping this
    /// context-free lets focused tests exercise the same production mutation.
    fn apply_fork_completed_state(&mut self, text: String) -> Option<usize> {
        let ordinal = self.pending_fork_ordinal.take()?;
        Self::truncate_messages_at_user_ordinal(&mut self.messages, ordinal);
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.transcript_generation = self.transcript_generation.wrapping_add(1);
        self.input.set_text(text.clone());
        self.input.set_cursor(text.chars().count());

        // The accepted request belonged to the abandoned branch. A successful
        // rewind must not let Retry replay it or keep its failure card alive.
        self.last_prepared_turn = None;
        self.reliability_state = AiOperationState::ready(
            self.reliability_state.identity.clone(),
            self.reliability_state.selection.clone(),
            self.current_work_snapshot(),
            self.reliability_state.retry.policy,
        );
        self.sync_status_from_reliability();
        Some(ordinal)
    }

    /// Apply a completed session rewind: truncate the transcript at the
    /// forked user message and stage its exact text in the composer for editing.
    fn apply_fork_completed(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        let prefill_chars = text.chars().count();
        let Some(ordinal) = self.apply_fork_completed_state(text) else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_fork_completed_without_request",
            );
            return false;
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_fork_completed",
            ordinal,
            message_count = self.messages.len(),
            prefill_chars,
            retry_payload_cleared = true,
        );
        // Pi rebuilt the session with fresh entry ids; refetch the list.
        self.fork_points.clear();
        self.refresh_fork_points(cx);
        true
    }

    /// Drop the `ordinal`-th user message and everything after it.
    fn truncate_messages_at_user_ordinal(
        messages: &mut Vec<AgentChatThreadMessage>,
        ordinal: usize,
    ) {
        let mut seen = 0usize;
        for index in 0..messages.len() {
            if matches!(messages[index].role, AgentChatThreadMessageRole::User) {
                if seen == ordinal {
                    messages.truncate(index);
                    return;
                }
                seen += 1;
            }
        }
    }

    pub(crate) fn refresh_models(&mut self, cx: &mut Context<Self>) {
        let rx = match self
            .connection
            .prepare_session(self.ui_thread_id.clone(), self.cwd.clone())
        {
            Ok(rx) => rx,
            Err(error) => {
                let safe_error = crate::logging::log_private_user_value(&error.to_string());
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_refresh_models_channel_closed",
                    ui_thread = %self.ui_thread_id,
                    error_bytes = safe_error.raw_bytes,
                    error_sha256 = %safe_error.sha256,
                );
                return;
            }
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_refresh_models_requested",
            ui_thread = %self.ui_thread_id,
        );

        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Ok(event) = rx.recv().await {
                let Some(weak) = entity.upgrade() else {
                    break;
                };
                cx.update(|cx| {
                    weak.update(cx, |this, cx| {
                        this.apply_event(event, cx);
                    });
                });
            }
        })
        .detach();
    }

    /// Select a model by ID. Updates the display name, persists to config, and notifies.
    pub(crate) fn select_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        if let Some(entry) = self
            .available_models
            .iter()
            .find(|m| m.id == model_id)
            .cloned()
        {
            let previous = self.reliability_state.selection.effective.clone();
            self.selected_model_id = Some(entry.id.clone());
            self.model_selection_mismatch = None;
            self.selected_model_display_name = Some(SharedString::from(
                entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| entry.id.clone()),
            ));

            // Persist selection to config.ts (non-fatal).
            let id = entry.id.clone();
            if !self.is_provider_free_fixture() {
            std::thread::Builder::new()
                .name("agent_chat-save-model".into())
                .spawn(move || {
                    let mut prefs = crate::config::load_user_preferences();
                    prefs.ai.selected_model_id = Some(id.clone());
                    if let Err(error) = crate::config::save_user_preferences(&prefs) {
                        let safe_error = crate::logging::log_private_user_value(&error.to_string());
                        tracing::warn!(
                            error_bytes = safe_error.raw_bytes,
                            error_sha256 = %safe_error.sha256,
                            "failed_to_persist_model_selection"
                        );
                    } else {
                        tracing::info!(model = %id, "model_selection_persisted");
                    }
                })
                .ok();
            }

            let recovering_selection_command = match &self.reliability_state.phase {
                AiPhase::Recovering {
                    action:
                        AiRecoveryAction::ChooseCompatibleModel { .. }
                        | AiRecoveryAction::ChooseProvider { .. }
                        | AiRecoveryAction::ChooseProfile { .. },
                    command_id,
                    ..
                } => Some(*command_id),
                AiPhase::Ready
                | AiPhase::Preflighting { .. }
                | AiPhase::Running { .. }
                | AiPhase::Cancelling { .. }
                | AiPhase::AwaitingRecovery { .. }
                | AiPhase::Recovering { .. }
                | AiPhase::Recovered { .. }
                | AiPhase::Succeeded { .. }
                | AiPhase::Cancelled { .. }
                | AiPhase::Dismissed { .. } => None,
            };
            if let Some(command_id) = recovering_selection_command {
                let applied = AiModelSelection {
                    provider_id: Self::provider_id_from_model(Some(&entry.id)),
                    model_id: Some(ModelId::from(entry.id.clone())),
                    profile_id: Some(ProfileId::from(self.profile_id.clone())),
                };
                let _ = self.transition_reliability(
                    AiOperationEvent::RecoveryCommandSucceeded {
                        command_id,
                        result: RecoveryEffectResult::SelectionApplied(SelectionChangeReceipt {
                            previous,
                            applied,
                            origin: SelectionOrigin::RecoveryChoice,
                        }),
                    },
                    cx,
                );
                if let Err(error) = self.resume_recovered_turn(cx) {
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "agent_chat_selection_recovery_resume_failed",
                        error_code = %error,
                    );
                }
            }
            self.notify_semantic_change(cx);
        }
    }

    pub(crate) fn toggle_favorite_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        match super::favorite_models::toggle_favorite_model_id(model_id) {
            Ok(_) => self.notify_semantic_change(cx),
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_favorite_model_save_failed",
                    error_kind = ?error.kind(),
                    diagnostic_fingerprint = %redacted_fingerprint(&error.to_string()),
                );
                self.push_system_message(
                    "Couldn't update your favorite model. Check Script Kit's storage permissions and try again.",
                    cx,
                );
            }
        }
    }

    pub(crate) fn cycle_favorite_model(&mut self, cx: &mut Context<Self>) {
        let favorites = super::favorite_models::load_favorite_model_ids();
        if let Some(model_id) = super::favorite_models::next_favorite_model_id(
            self.selected_model_id(),
            &favorites,
            self.available_models(),
        ) {
            self.select_model(&model_id, cx);
        }
    }
}
