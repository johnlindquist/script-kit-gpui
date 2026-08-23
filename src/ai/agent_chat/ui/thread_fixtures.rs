impl AgentChatThread {
    /// Install a synthetic transcript state for no-token Agent Chat UI proof.
    pub(crate) fn apply_test_fixture(
        &mut self,
        phase: &str,
        user_text: Option<String>,
        assistant_text: Option<String>,
        message_count: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.flush_streaming_text_buffer();
        let user_text = user_text.unwrap_or_else(|| "No-token activity fixture".to_string());
        let user_text = user_text.trim();
        if user_text.is_empty() {
            return Err("setAgentChatTestFixture requires non-empty userText".to_string());
        }

        self.stream_task = None;
        self.pending_permission = None;
        self.messages.clear();
        self.next_message_id = 1;
        self.active_plan_entries.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.standing_approvals.clear();
        self.active_mode_id = None;
        self.available_commands.clear();
        self.usage_tokens = None;
        self.usage_cost_usd = None;
        self.input.clear();
        self.clear_all_pending_context("set_agent_chat_test_fixture");
        self.context_receipts.clear();
        self.last_prepared_turn = None;
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        self.queued_submit_while_bootstrapping = false;
        self.reliability_state = AiOperationState::ready(
            Self::reliability_identity(
                self.session_policy,
                &self.profile_id,
                self.selected_model_id.as_deref(),
                &self.cwd,
            ),
            Self::reliability_selection(
                &self.profile_id,
                self.selected_model_id.as_deref(),
                SelectionOrigin::PersistedUserChoice,
            ),
            self.current_work_snapshot(),
            RetryPolicy {
                automatic_max: 0,
                manual_max: 2,
            },
        );

        if let Some(message_count) = message_count {
            let message_count = message_count.clamp(1, 2_000);
            let assistant_text = assistant_text
                .as_deref()
                .unwrap_or("Fixture assistant text with enough markdown to exercise layout.");
            for index in 0..message_count {
                let role = if index % 2 == 0 {
                    AgentChatThreadMessageRole::User
                } else {
                    AgentChatThreadMessageRole::Assistant
                };
                let seed = if matches!(role, AgentChatThreadMessageRole::User) {
                    user_text
                } else {
                    assistant_text
                };
                let body = format!(
                    "{seed}\n\n### Fixture turn {turn}\n\n- row: {row}\n- purpose: rapid transcript scroll performance\n- repeated detail: alpha beta gamma delta epsilon zeta eta theta\n\n```text\nfixture block {row}\nline one with enough width to require text layout\nline two with stable markdown parsing\n```\n\nThis paragraph intentionally gives TextViewState markdown a non-trivial body while keeping the fixture deterministic.",
                    turn = index / 2 + 1,
                    row = index + 1,
                );
                self.push_message(role, body);
            }
            self.set_status(AgentChatThreadStatus::Idle);
        } else {
            self.push_message(AgentChatThreadMessageRole::User, user_text.to_string());
            match phase {
                "c06Completed" | "c06-completed" => {
                    self.push_message(
                        AgentChatThreadMessageRole::Assistant,
                        assistant_text.unwrap_or_else(|| {
                            " C06 synthetic answer\nsecond line with trailing spaces \n".to_string()
                        }),
                    );
                    self.push_message(AgentChatThreadMessageRole::Assistant, " \n\t ");
                    self.set_status(AgentChatThreadStatus::Idle);
                }
                "c06StreamingPartial" | "c06-streaming-partial" => {
                    self.push_message(
                        AgentChatThreadMessageRole::Assistant,
                        assistant_text.unwrap_or_else(|| {
                            "C06 synthetic partial\nwith exact final newline\n".to_string()
                        }),
                    );
                    let command_id = self
                        .begin_reliability_turn(CapabilityDecision::Compatible, cx)?
                        .ok_or_else(|| "c06_fixture_missing_start_command".to_string())?;
                    self.transition_reliability(
                        AiOperationEvent::RuntimeStarted {
                            command_id,
                            turn: TurnRef::from("c06-fixture-partial"),
                        },
                        cx,
                    )
                    .map_err(|error| format!("c06_fixture_runtime_start:{:?}", error.reason))?;
                }
                "c06StreamingEmpty" | "c06-streaming-empty" => {
                    self.push_message(AgentChatThreadMessageRole::Assistant, String::new());
                    let command_id = self
                        .begin_reliability_turn(CapabilityDecision::Compatible, cx)?
                        .ok_or_else(|| "c06_fixture_missing_start_command".to_string())?;
                    self.transition_reliability(
                        AiOperationEvent::RuntimeStarted {
                            command_id,
                            turn: TurnRef::from("c06-fixture-empty"),
                        },
                        cx,
                    )
                    .map_err(|error| format!("c06_fixture_runtime_start:{:?}", error.reason))?;
                }
                "c06RetryableFailure" | "c06-retryable-failure" => {
                    self.store_prepared_turn_payload(
                        user_text.to_string(),
                        vec![ContentBlock::Text(TextContent::new(user_text))],
                        Vec::new(),
                    );
                    let command_id = self
                        .begin_reliability_turn(CapabilityDecision::Compatible, cx)?
                        .ok_or_else(|| "c06_fixture_missing_start_command".to_string())?;
                    self.transition_reliability(
                        AiOperationEvent::RuntimeStarted {
                            command_id,
                            turn: TurnRef::from("c06-fixture-retry"),
                        },
                        cx,
                    )
                    .map_err(|error| format!("c06_fixture_runtime_start:{:?}", error.reason))?;
                    let failure = provider_failure(
                        ProtocolComponent::Provider,
                        "C06_SYNTHETIC_RETRYABLE_FAILURE",
                    );
                    let message = failure.primary_message().to_string();
                    self.transition_reliability(AiOperationEvent::Failed(failure.failure), cx)
                        .map_err(|error| format!("c06_fixture_failure:{:?}", error.reason))?;
                    self.push_message(AgentChatThreadMessageRole::Error, message);
                }
                "awaitingFirstAssistantText"
                | "awaiting-first-assistant-text"
                | "awaiting"
                | "waitingNoAssistant"
                | "waiting-no-assistant" => {
                    self.set_status(AgentChatThreadStatus::Streaming);
                }
                "emptyAssistant" | "empty-assistant" => {
                    self.push_message(AgentChatThreadMessageRole::Assistant, String::new());
                    self.set_status(AgentChatThreadStatus::Streaming);
                }
                "assistantText"
                | "assistant-text"
                | "text"
                | "firstToken"
                | "first-token"
                | "multiTokenStreaming"
                | "multi-token-streaming" => {
                    self.push_message(
                        AgentChatThreadMessageRole::Assistant,
                        assistant_text.unwrap_or_else(|| {
                            if matches!(phase, "firstToken" | "first-token") {
                                "First".to_string()
                            } else {
                                "Fixture assistant text with multiple tokens.".to_string()
                            }
                        }),
                    );
                    self.set_status(AgentChatThreadStatus::Streaming);
                }
                "idle" | "completed" => {
                    self.push_message(
                        AgentChatThreadMessageRole::Assistant,
                        assistant_text.unwrap_or_else(|| {
                            "Fixture assistant text with multiple tokens.".to_string()
                        }),
                    );
                    self.set_status(AgentChatThreadStatus::Idle);
                }
                "terminalEmpty" | "terminal-empty" => {
                    self.push_message(AgentChatThreadMessageRole::Assistant, String::new());
                    self.set_status(AgentChatThreadStatus::Idle);
                }
                "contextLifecyclePending" | "context-lifecycle-pending" => {
                    let part = AiContextPart::TextBlock {
                        label: "Synthetic context".to_string(),
                        source: "fixture://context-lifecycle/shared".to_string(),
                        text: "synthetic context body".to_string(),
                        mime_type: Some("text/plain".to_string()),
                    };
                    stage_context_item(
                        &mut self.pending_context_items,
                        StagedContextItem::pending(
                            part.clone(),
                            ContextProvenance::ImplicitFocused,
                            ContextRole::Supplemental,
                        ),
                    );
                    stage_context_item(
                        &mut self.pending_context_items,
                        StagedContextItem::pending(
                            part,
                            ContextProvenance::AttachmentPortal,
                            ContextRole::Supplemental,
                        ),
                    );
                    self.set_status(AgentChatThreadStatus::Idle);
                }
                "contextLifecyclePartialAccepted" | "context-lifecycle-partial-accepted" => {
                    let primary = StagedContextItem::pending(
                        AiContextPart::TextBlock {
                            label: "Required synthetic context".to_string(),
                            source: "fixture://context-lifecycle/primary".to_string(),
                            text: "required synthetic body".to_string(),
                            mime_type: Some("text/plain".to_string()),
                        },
                        ContextProvenance::HostHandoff,
                        ContextRole::Primary,
                    );
                    let supplemental = StagedContextItem::pending(
                        AiContextPart::FilePath {
                            path: "/missing/CONTEXT_LIFECYCLE_PATH_CANARY".to_string(),
                            label: "Optional synthetic file".to_string(),
                        },
                        ContextProvenance::AttachmentPortal,
                        ContextRole::Supplemental,
                    );
                    let primary_id = primary.id.clone();
                    let supplemental_id = supplemental.id.clone();
                    self.pending_context_items = vec![primary, supplemental];
                    let failure = crate::ai::reliability::context_unavailable_failure(
                        "CONTEXT_LIFECYCLE_ERROR_CANARY",
                    );
                    let transition = PreparedContextTransition {
                        attempted_items: self.pending_context_items.clone(),
                        attempted_ids: vec![primary_id.clone(), supplemental_id.clone()],
                        resolved_ids: vec![primary_id],
                        failures: vec![(supplemental_id, failure)],
                    };
                    self.commit_context_after_runtime_start(&transition);
                    self.store_prepared_turn_payload(
                        user_text.to_string(),
                        vec![ContentBlock::Text(TextContent::new(user_text))],
                        Vec::new(),
                    );
                    self.set_status(AgentChatThreadStatus::Error);
                }
                "contextLifecycleFreshThread" | "context-lifecycle-fresh-thread" => {
                    self.messages.clear();
                    self.clear_all_pending_context("context_lifecycle_fresh_thread_fixture");
                    self.context_receipts.clear();
                    self.last_prepared_turn = None;
                    self.set_status(AgentChatThreadStatus::Idle);
                }
                "error" | "provider-error" => {
                    let error =
                        assistant_text.unwrap_or_else(|| "Fixture provider error".to_string());
                    let _ = self.begin_reliability_turn(CapabilityDecision::Compatible, cx);
                    let failure = provider_failure(ProtocolComponent::Provider, error);
                    let message = failure.primary_message().to_string();
                    let _ =
                        self.transition_reliability(AiOperationEvent::Failed(failure.failure), cx);
                    self.push_message(AgentChatThreadMessageRole::Error, message);
                }
                other => {
                    return Err(format!("unknown setAgentChatTestFixture phase {other:?}"));
                }
            }
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_test_fixture_applied",
            phase,
            requested_message_count = message_count.unwrap_or(0),
            message_count = self.messages.len(),
            awaiting_first_assistant_text = self.awaiting_first_assistant_text(),
        );
        cx.notify();
        Ok(())
    }
}
