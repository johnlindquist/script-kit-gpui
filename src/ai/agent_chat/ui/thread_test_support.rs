#[cfg(test)]
struct TestAgentChatConnection;

#[cfg(test)]
impl AgentChatConnection for TestAgentChatConnection {
    fn start_turn(
        &self,
        _request: AgentChatTurnRequest,
    ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
    {
        Err(anyhow::anyhow!("test connection does not start turns").into())
    }

    fn cancel_turn(&self, _ui_thread_id: String) -> crate::ai::reliability::AiAdapterResult<()> {
        Ok(())
    }

    fn prepare_session(
        &self,
        _ui_thread_id: String,
        _cwd: PathBuf,
    ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
    {
        Err(anyhow::anyhow!("test connection does not prepare sessions").into())
    }
}

/// Test-only helpers exposed to sibling modules in `src/ai/agent_chat/ui/`.
#[cfg(test)]
impl AgentChatThread {
    /// Build a test thread without a real connection or GPUI context.
    pub(super) fn test_new(
        context_blocks: Vec<ContentBlock>,
        initial_input: Option<String>,
    ) -> Self {
        let (_perm_tx, perm_rx) = async_channel::bounded(1);
        let dummy_connection: Arc<dyn AgentChatConnection> = Arc::new(TestAgentChatConnection);

        Self {
            connection: dummy_connection,
            permission_rx: perm_rx,
            ui_thread_id: "test-thread".to_string(),
            cwd: PathBuf::from("/tmp/test"),
            display_name: "Test Agent".into(),
            profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID.to_string(),
            messages: Vec::new(),
            input: match initial_input {
                Some(text) if !text.is_empty() => TextInputState::with_text(text),
                _ => TextInputState::new(),
            },
            status: AgentChatThreadStatus::Idle,
            reliability_state: AiOperationState::ready(
                Self::reliability_identity(
                    AgentChatSessionPolicy::Full,
                    crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID,
                    None,
                    Path::new("/tmp/test"),
                ),
                Self::reliability_selection(
                    crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID,
                    None,
                    SelectionOrigin::PersistedUserChoice,
                ),
                AiWorkSnapshot {
                    key: WorkKey::from("test-thread"),
                    transcript: PreservationReceipt::NotApplicable,
                    draft: PreservationReceipt::NotApplicable,
                    attachments: PreservationReceipt::NotApplicable,
                    partial_output: PreservationReceipt::NotApplicable,
                },
                RetryPolicy {
                    automatic_max: 0,
                    manual_max: 2,
                },
            ),
            context_resolution_id: 0,
            pending_permission: None,
            pending_context_blocks: context_blocks,
            pending_context_consumed: false,
            pending_context_items: Vec::new(),
            context_receipts: Vec::new(),
            last_prepared_turn: None,
            pending_ambient_context_enabled: false,
            context_bootstrap_state: AgentChatContextBootstrapState::Ready,
            queued_submit_while_bootstrapping: false,
            context_bootstrap_note: None,
            queued_messages: VecDeque::new(),
            queue_paused: false,
            active_plan_entries: Vec::new(),
            active_mode_id: None,
            available_commands: Vec::new(),
            active_tool_calls: Vec::new(),
            tool_call_lookup: HashMap::new(),
            standing_approvals: Vec::new(),
            fork_points: Vec::new(),
            pending_fork_ordinal: None,
            selected_agent: None,
            available_agents: Vec::new(),
            launch_requirements: crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
            setup_state: None,
            usage_tokens: None,
            usage_cost_usd: None,
            stream_started_at: None,
            ttft_pending: false,
            stream_task: None,
            permission_task: None,
            streaming_text_buffer: StreamingTextBuffer::default(),
            streaming_text_drain_task: None,
            transcript_generation: 0,
            next_message_id: 1,
            host_window_state: None,
            notification_debounce: AgentChatNotificationDebounce::default(),
            current_turn_id: 0,
            llm_title_attempted: false,
            session_policy: AgentChatSessionPolicy::Full,
            available_models: Vec::new(),
            selected_model_id: None,
            model_selection_mismatch: None,
            selected_model_display_name: None,
            profile_display_name: None,
            profile_icon_name: None,
        }
    }

    /// Test seam for the thread-owned session policy (WP-B1). Replaces the
    /// old `set_retain_history_test` retain-history boolean seam.
    pub(super) fn set_session_policy_test(&mut self, policy: AgentChatSessionPolicy) {
        self.session_policy = policy;
    }

    pub(super) fn session_policy_test(&self) -> AgentChatSessionPolicy {
        self.session_policy
    }

    fn transition_reliability_test(&mut self, event: AiOperationEvent) {
        if let Ok(next) = transition(self.reliability_state.clone(), event) {
            self.reliability_state = next.next;
            self.sync_status_from_reliability();
        }
    }

    fn finish_stream_closed_without_terminal(&mut self) -> bool {
        if !matches!(
            self.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) {
            return false;
        }
        self.flush_streaming_text_buffer();
        self.pending_permission = None;
        let has_output = self
            .messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, AgentChatThreadMessageRole::Assistant))
            .is_some_and(|message| !message.body.trim().is_empty());
        if has_output {
            self.transition_reliability_test(AiOperationEvent::Completed(CompletionKind::Partial));
            self.status = AgentChatThreadStatus::Idle;
        } else {
            let failure =
                process_failure(ProtocolComponent::Pi, ProcessFailureFacts::RuntimeClosed);
            self.transition_reliability_test(AiOperationEvent::Failed(failure.failure));
            self.push_message(
                AgentChatThreadMessageRole::Error,
                "The AI connection stopped. Your work is saved and can be recovered.",
            );
            self.status = AgentChatThreadStatus::Error;
        }
        true
    }

    pub(super) fn dismiss_recovery_test(&mut self) {
        self.transition_reliability_test(AiOperationEvent::DismissRequested);
    }

    pub(super) fn seed_last_prepared_turn_test(&mut self, text: &str) {
        self.store_prepared_turn_payload(
            text.to_string(),
            vec![ContentBlock::Text(TextContent::new(text))],
            Vec::new(),
        );
    }

    pub(super) fn retry_last_user_turn_test(&mut self) -> Result<(), String> {
        if !matches!(
            self.reliability_state.phase,
            AiPhase::AwaitingRecovery { .. }
        ) {
            return Ok(());
        }

        let Some(prepared) = self.last_prepared_turn.clone() else {
            return Err("no_immutable_prepared_turn_to_retry".to_string());
        };
        let _request = self.turn_request(prepared.blocks);
        let retry_transition = transition(
            self.reliability_state.clone(),
            AiOperationEvent::RecoverySelected(AiRecoveryAction::Retry),
        )
        .map_err(|error| format!("{:?}", error.reason))?;
        let command_id = retry_transition
            .commands
            .iter()
            .find_map(|command| match command {
                AiCommand::ScheduleBackoff { command_id, .. } => Some(*command_id),
                _ => None,
            })
            .ok_or_else(|| "missing_backoff".to_string())?;
        self.reliability_state = retry_transition.next;
        self.transition_reliability_test(AiOperationEvent::BackoffElapsed { command_id });
        let preflight_transition = transition(
            self.reliability_state.clone(),
            AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
        )
        .map_err(|error| format!("{:?}", error.reason))?;
        let start_command_id = preflight_transition
            .commands
            .iter()
            .find_map(|command| match command {
                AiCommand::StartTurn(command) => Some(command.command_id),
                _ => None,
            })
            .ok_or_else(|| "missing_start".to_string())?;
        self.reliability_state = preflight_transition.next;
        self.transition_reliability_test(AiOperationEvent::RuntimeStarted {
            command_id: start_command_id,
            turn: TurnRef::from("test-retry"),
        });
        self.stream_started_at = Some(std::time::Instant::now());
        self.ttft_pending = true;
        self.status = AgentChatThreadStatus::Streaming;
        self.setup_state = None;
        Ok(())
    }

    /// Add a context part without a GPUI context (skips `cx.notify()`).
    pub(super) fn add_context_part_test(&mut self, part: crate::ai::message_parts::AiContextPart) {
        let is_ambient_bootstrap = part.is_ambient_bootstrap_resource();
        self.pending_context_consumed = false;

        if is_ambient_bootstrap {
            self.pending_context_blocks.clear();
            self.pending_ambient_context_enabled = true;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
            self.context_bootstrap_note = part
                .ambient_chip_label()
                .map(Self::ambient_capture_preparing_note);
        } else if !self.pending_ambient_context_enabled
            && matches!(
                self.context_bootstrap_state,
                AgentChatContextBootstrapState::Preparing
            )
        {
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = None;
        }

        stage_context_item(
            &mut self.pending_context_items,
            StagedContextItem::pending(
                part,
                ContextProvenance::UserMention,
                ContextRole::Supplemental,
            ),
        );
    }

    /// Remove a context part by index without a GPUI context (skips `cx.notify()`).
    pub(super) fn remove_context_part_test(&mut self, index: usize) {
        if index >= self.pending_context_items.len() {
            return;
        }
        let removed = self.pending_context_items.remove(index);
        let removed_ambient_label = removed
            .part
            .ambient_chip_label()
            .map(|value| value.to_string());

        if let Some(ref ambient_label) = removed_ambient_label {
            self.pending_ambient_context_enabled = false;
            self.pending_context_blocks.clear();
            self.pending_context_consumed = false;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = Some(Self::ambient_capture_removed_note(ambient_label));
        }
    }

    pub(super) fn replace_pending_context_parts_test(
        &mut self,
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        reason: &'static str,
    ) {
        self.replace_pending_context_parts_inner(
            parts,
            ContextProvenance::HostHandoff,
            ContextRole::Supplemental,
            reason,
        );
    }

    /// Stage Ask Anything context without GPUI context (skips `cx.notify()`).
    pub(super) fn stage_ask_anything_context_test(
        &mut self,
        context: &crate::ai::TabAiContextBlob,
    ) -> Result<(), String> {
        let ambient_label = self
            .current_ambient_chip_label()
            .unwrap_or_else(|| crate::ai::message_parts::ASK_ANYTHING_LABEL.to_string());

        if !self.pending_ambient_context_enabled {
            self.pending_context_blocks.clear();
            self.pending_context_consumed = false;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = Some(Self::ambient_capture_removed_note(&ambient_label));
            return Ok(());
        }

        self.pending_context_blocks = build_tab_ai_agent_chat_context_blocks(context)?;
        self.pending_context_consumed = false;
        self.promote_ask_anything_chip_to_ambient();
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = Some(Self::ambient_capture_ready_note(&ambient_label));
        Ok(())
    }

    /// Apply an event without a GPUI context (for testing pure logic).
    /// Reuses the same helper methods as `apply_event` but skips `cx.notify()`.
    pub(super) fn apply_event_test(&mut self, event: super::AgentChatEvent) {
        match event {
            super::AgentChatEvent::UserMessageDelta(chunk) => {
                self.append_chunk(AgentChatThreadMessageRole::System, chunk);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::AgentMessageDelta(chunk) => {
                self.append_chunk(AgentChatThreadMessageRole::Assistant, chunk);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::AgentThoughtDelta(chunk) => {
                self.append_chunk(AgentChatThreadMessageRole::Thought, chunk);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::ToolCallStarted {
                tool_call_id,
                title,
                status,
                tool_name,
                raw_input,
            } => {
                // Mirror `apply_event`'s WP-B2 forbidden-tool guard so tests can
                // exercise the fail-closed path without a GPUI context.
                if self.tool_event_is_forbidden(tool_name.as_deref()) {
                    self.fail_turn_forbidden_tool(tool_name.as_deref(), None);
                } else {
                    self.upsert_tool_call_start(tool_call_id, title, status, tool_name, raw_input);
                    self.set_status(AgentChatThreadStatus::Streaming);
                }
            }
            super::AgentChatEvent::ToolCallUpdated {
                tool_call_id,
                title,
                status,
                body,
                raw_input,
                diff,
                is_error,
            } => {
                self.apply_tool_call_update(
                    tool_call_id,
                    title,
                    status,
                    body,
                    raw_input,
                    diff,
                    is_error,
                );
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::PlanUpdated { entries } => {
                self.active_plan_entries = entries;
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::AvailableCommandsUpdated { command_names } => {
                self.available_commands = command_names;
            }
            super::AgentChatEvent::ModeChanged { mode_id } => {
                self.active_mode_id = Some(mode_id);
            }
            super::AgentChatEvent::UsageUpdated {
                used_tokens,
                context_size,
                cost_usd,
            } => {
                self.usage_tokens = Some((used_tokens, context_size));
                if let Some(cost) = cost_usd {
                    self.usage_cost_usd = Some(cost);
                }
            }
            super::AgentChatEvent::ModelsAvailable {
                current_model_id,
                models,
            } => {
                self.apply_agent_models(current_model_id, models);
            }
            super::AgentChatEvent::ForkPointsAvailable { entries } => {
                self.fork_points = entries;
            }
            super::AgentChatEvent::ForkCompleted { text } => {
                // Production and tests share the exact accepted-rewind mutation;
                // only the GPUI fork-point refresh is omitted here.
                self.apply_fork_completed_state(text);
            }
            super::AgentChatEvent::TurnCompleted { .. } => {
                self.set_status(AgentChatThreadStatus::Idle);
                if !self.queue_paused {
                    if let Some(message) = self.queued_messages.pop_front() {
                        self.push_message(AgentChatThreadMessageRole::User, message.text);
                        self.set_status(AgentChatThreadStatus::Streaming);
                    }
                }
            }
            super::AgentChatEvent::SetupRequired {
                reason,
                auth_methods,
            } => {
                let current_requirements = self.current_setup_requirements();
                self.setup_state = Some(
                    super::setup_state::AgentChatInlineSetupState::from_runtime_setup_required(
                        self.selected_agent.clone(),
                        self.available_agents.clone(),
                        current_requirements,
                        &reason,
                        &auth_methods,
                    ),
                );
                if !matches!(
                    self.reliability_state.phase,
                    AiPhase::Preflighting { .. } | AiPhase::Running { .. }
                ) {
                    let request = TurnRequestRef::from("test-setup-request");
                    self.transition_reliability_test(AiOperationEvent::SubmitRequested {
                        request,
                        work: self.current_work_snapshot(),
                        selection: Self::reliability_selection(
                            &self.profile_id,
                            self.selected_model_id.as_deref(),
                            SelectionOrigin::PersistedUserChoice,
                        ),
                        risk: TurnRisk::MayMutate,
                    });
                    self.transition_reliability_test(AiOperationEvent::CapabilityResolved(
                        CapabilityDecision::Compatible,
                    ));
                }
                // S11/S12: `SetupRequired` is a typed fact. The prose form
                // matched no auth wording, so this classified to `Unknown` and
                // the card lost its Sign In action.
                let failure = crate::ai::reliability::setup_required_failure(
                    ProtocolComponent::Pi,
                    &reason,
                    &auth_methods,
                );
                self.transition_reliability_test(AiOperationEvent::Failed(failure.failure));
            }
            super::AgentChatEvent::TurnFailed { failure } => {
                if matches!(self.reliability_state.phase, AiPhase::Ready) {
                    let request = TurnRequestRef::from("test-request");
                    self.transition_reliability_test(AiOperationEvent::SubmitRequested {
                        request,
                        work: self.current_work_snapshot(),
                        selection: Self::reliability_selection(
                            &self.profile_id,
                            self.selected_model_id.as_deref(),
                            SelectionOrigin::PersistedUserChoice,
                        ),
                        risk: TurnRisk::MayMutate,
                    });
                    self.transition_reliability_test(AiOperationEvent::CapabilityResolved(
                        CapabilityDecision::Compatible,
                    ));
                }
                let message = failure.primary_message().to_string();
                self.transition_reliability_test(AiOperationEvent::Failed(failure.failure));
                self.push_message(AgentChatThreadMessageRole::Error, message);
            }
        }
    }
}
