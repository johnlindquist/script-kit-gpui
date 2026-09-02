impl AgentChatThread {
    pub(crate) fn dismiss_recovery(&mut self, cx: &mut Context<Self>) {
        let _ = self.transition_reliability(AiOperationEvent::DismissRequested, cx);
    }

    fn last_user_turn_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, AgentChatThreadMessageRole::User))
            .map(|message| message.body.to_string())
    }

    pub(crate) fn retry_last_user_turn(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if !matches!(
            self.reliability_state.phase,
            AiPhase::AwaitingRecovery { .. }
        ) {
            return Ok(());
        }

        let Some(prepared) = self.last_prepared_turn.clone() else {
            return Err("no_immutable_prepared_turn_to_retry".to_string());
        };
        let retry_commands = self
            .transition_reliability(
                AiOperationEvent::RecoverySelected(AiRecoveryAction::Retry),
                cx,
            )
            .map_err(|error| format!("ai_reliability_retry:{:?}", error.reason))?;
        let command_id = retry_commands
            .into_iter()
            .find_map(|command| match command {
                AiCommand::ScheduleBackoff { command_id, .. } => Some(command_id),
                _ => None,
            })
            .ok_or_else(|| "ai_reliability_missing_backoff_command".to_string())?;
        self.transition_reliability(AiOperationEvent::BackoffElapsed { command_id }, cx)
            .map_err(|error| format!("ai_reliability_backoff:{:?}", error.reason))?;
        let commands = self
            .transition_reliability(
                AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
                cx,
            )
            .map_err(|error| format!("ai_reliability_retry_preflight:{:?}", error.reason))?;
        let start_command_id = commands
            .into_iter()
            .find_map(|command| match command {
                AiCommand::StartTurn(command) => Some(command.command_id),
                _ => None,
            })
            .ok_or_else(|| "ai_reliability_missing_retry_start_command".to_string())?;
        let rx = match self
            .connection
            .start_turn(self.turn_request(prepared.blocks))
        {
            Ok(rx) => rx,
            Err(error) => {
                let safe = error.failure.primary_message().to_string();
                let error_code = error.to_string();
                let _ = self
                    .transition_reliability(AiOperationEvent::Failed(error.failure.failure), cx);
                self.push_message(AgentChatThreadMessageRole::Error, safe);
                return Err(error_code);
            }
        };
        self.transition_reliability(
            AiOperationEvent::RuntimeStarted {
                command_id: start_command_id,
                turn: TurnRef::from(format!(
                    "{}:{}",
                    self.ui_thread_id,
                    self.current_turn_id.wrapping_add(1)
                )),
            },
            cx,
        )
        .map_err(|error| format!("ai_reliability_retry_start:{:?}", error.reason))?;
        self.stream_started_at = Some(std::time::Instant::now());
        self.ttft_pending = true;
        self.current_turn_id = self.current_turn_id.wrapping_add(1);
        self.setup_state = None;
        self.bind_stream(rx, cx);
        Ok(())
    }

    pub(crate) fn select_recovery_action(
        &mut self,
        action: AiRecoveryAction,
        cx: &mut Context<Self>,
    ) -> Result<Vec<AiCommand>, String> {
        if matches!(action, AiRecoveryAction::Retry) {
            self.retry_last_user_turn(cx)?;
            return Ok(Vec::new());
        }
        self.transition_reliability(AiOperationEvent::RecoverySelected(action), cx)
            .map_err(|error| format!("ai_reliability_recovery:{:?}", error.reason))
    }

    pub(crate) fn complete_recovery_command(
        &mut self,
        command_id: sk_protocol::ai_reliability::CommandId,
        result: RecoveryEffectResult,
        cx: &mut Context<Self>,
    ) {
        let _ = self.transition_reliability(
            AiOperationEvent::RecoveryCommandSucceeded { command_id, result },
            cx,
        );
    }

    pub(crate) fn fail_recovery_command(
        &mut self,
        command_id: sk_protocol::ai_reliability::CommandId,
        failure: AiFailure,
        cx: &mut Context<Self>,
    ) {
        let _ = self.transition_reliability(
            AiOperationEvent::RecoveryCommandFailed {
                command_id,
                failure,
            },
            cx,
        );
    }

    pub(crate) fn resume_recovered_turn_from_view(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.resume_recovered_turn(cx)
    }

    fn resume_recovered_turn(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(prepared) = self.last_prepared_turn.clone() else {
            return Err("no_immutable_prepared_turn_to_resume".to_string());
        };
        let reset_commands = self
            .transition_reliability(AiOperationEvent::ResetForNextTurn, cx)
            .map_err(|error| format!("ai_reliability_recovery_reset:{:?}", error.reason))?;
        if !reset_commands
            .iter()
            .any(|command| matches!(command, AiCommand::CheckCapabilities(_)))
        {
            return Ok(());
        }
        let commands = self
            .transition_reliability(
                AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
                cx,
            )
            .map_err(|error| format!("ai_reliability_recovery_preflight:{:?}", error.reason))?;
        let start_command_id = commands
            .into_iter()
            .find_map(|command| match command {
                AiCommand::StartTurn(command) => Some(command.command_id),
                _ => None,
            })
            .ok_or_else(|| "ai_reliability_missing_recovery_start".to_string())?;
        let rx = match self
            .connection
            .start_turn(self.turn_request(prepared.blocks))
        {
            Ok(rx) => rx,
            Err(error) => {
                let safe = error.failure.primary_message().to_string();
                let code = error.to_string();
                let _ = self
                    .transition_reliability(AiOperationEvent::Failed(error.failure.failure), cx);
                self.push_message(AgentChatThreadMessageRole::Error, safe);
                return Err(code);
            }
        };
        self.transition_reliability(
            AiOperationEvent::RuntimeStarted {
                command_id: start_command_id,
                turn: TurnRef::from(format!(
                    "{}:{}",
                    self.ui_thread_id,
                    self.current_turn_id.wrapping_add(1)
                )),
            },
            cx,
        )
        .map_err(|error| format!("ai_reliability_recovery_start:{:?}", error.reason))?;
        self.stream_started_at = Some(std::time::Instant::now());
        self.ttft_pending = true;
        self.current_turn_id = self.current_turn_id.wrapping_add(1);
        self.setup_state = None;
        self.bind_stream(rx, cx);
        Ok(())
    }

    /// Resolve a pending permission request with the user's selection.
    ///
    /// Pass `None` for cancellation, or `Some(option_id)` for a selection.
    pub(crate) fn approve_pending_permission(
        &mut self,
        selected_option_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let mut had_request = false;
        let mut changed = false;

        if let Some(request) = self.pending_permission.take() {
            let note = Self::permission_resolution_message(&request, selected_option_id.as_deref());
            self.record_standing_approval(&request, selected_option_id.as_deref());
            let _ = request.reply_tx.send_blocking(selected_option_id);
            changed |= self.push_message(AgentChatThreadMessageRole::System, note);
            had_request = true;
        }

        // Stay in Streaming so submit_input() remains blocked until
        // TurnFinished or Failed arrives — prevents mid-turn double-submit.
        if had_request {
            changed |= self.set_status(AgentChatThreadStatus::Streaming);
        }

        if changed {
            self.notify_semantic_change(cx);
        }
    }

    /// Record a session-scoped grant when the chosen option is a persistent
    /// "Allow always". Deduped by (tool, subject) so repeated grants for the
    /// same tool do not stack.
    fn record_standing_approval(
        &mut self,
        request: &AgentChatApprovalRequest,
        selected_option_id: Option<&str>,
    ) {
        let Some(option) = selected_option_id
            .and_then(|id| request.options.iter().find(|opt| opt.option_id == id))
        else {
            return;
        };
        if !option.is_persistent_allow() {
            return;
        }

        let (tool_title, subject, kind_badge) = match request.preview.as_ref() {
            Some(preview) => (
                preview.tool_title.clone(),
                preview.subject.clone(),
                preview.kind.badge_label(),
            ),
            None => (
                request.title.clone(),
                None,
                super::permission_broker::AgentChatApprovalPreviewKind::Generic.badge_label(),
            ),
        };

        let already_recorded = self
            .standing_approvals
            .iter()
            .any(|grant| grant.tool_title == tool_title && grant.subject == subject);
        if already_recorded {
            return;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_standing_approval_recorded",
            ui_thread = %self.ui_thread_id,
            tool_title = %tool_title,
            has_subject = subject.is_some(),
            total = self.standing_approvals.len() + 1,
        );
        self.standing_approvals
            .push(super::permission_broker::AgentChatStandingApproval {
                tool_title,
                subject,
                kind_badge,
                option_label: option.summary_label(),
            });
    }

    /// Session-scoped "Allow always" grants recorded so far, in grant order.
    pub(crate) fn standing_approvals(
        &self,
    ) -> &[super::permission_broker::AgentChatStandingApproval] {
        &self.standing_approvals
    }

    /// Push a System transcript message listing every standing approval, so
    /// the user can review what the session will no longer ask about.
    pub(crate) fn review_standing_approvals(&mut self, cx: &mut Context<Self>) {
        let body = if self.standing_approvals.is_empty() {
            "**Auto-approvals** \u{00b7} none granted this session.".to_string()
        } else {
            let mut lines = vec![format!(
                "**Auto-approvals** \u{00b7} {} standing grant{} this session:",
                self.standing_approvals.len(),
                if self.standing_approvals.len() == 1 {
                    ""
                } else {
                    "s"
                },
            )];
            for grant in &self.standing_approvals {
                let subject = grant
                    .subject
                    .as_deref()
                    .map(|subject| format!(" \u{00b7} `{subject}`"))
                    .unwrap_or_default();
                lines.push(format!(
                    "- {} \u{00b7} {}{subject} \u{00b7} {}",
                    grant.tool_title, grant.kind_badge, grant.option_label,
                ));
            }
            lines.push(
                "Grants live in the Pi session approval cache; starting a new session resets them."
                    .to_string(),
            );
            lines.join("\n")
        };
        self.push_message(AgentChatThreadMessageRole::System, body);
        self.notify_semantic_change(cx);
    }

    fn permission_notification_body(&self, request: &AgentChatApprovalRequest) -> String {
        request
            .preview
            .as_ref()
            .map(|preview| {
                preview
                    .subject
                    .as_ref()
                    .map(|subject| format!("{} · {subject}", preview.tool_title))
                    .unwrap_or_else(|| preview.tool_title.clone())
            })
            .unwrap_or_else(|| {
                if request.body.trim().is_empty() {
                    request.title.clone()
                } else {
                    format!("{} · {}", request.title, request.body)
                }
            })
    }

    /// Build a human-readable audit message for a permission resolution.
    fn permission_resolution_message(
        request: &AgentChatApprovalRequest,
        selected_option_id: Option<&str>,
    ) -> String {
        let tool_title = request
            .preview
            .as_ref()
            .map(|p| p.tool_title.clone())
            .unwrap_or_else(|| request.title.clone());

        match selected_option_id
            .and_then(|id| request.options.iter().find(|opt| opt.option_id == id))
        {
            Some(option) => format!(
                "Permission granted \u{00b7} {} \u{00b7} {}",
                tool_title,
                option.summary_label()
            ),
            None => format!("Permission cancelled \u{00b7} {}", tool_title),
        }
    }
}
