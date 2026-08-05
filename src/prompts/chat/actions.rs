use super::*;

const TRANSFER_TO_AGENT_CHAT_READY_RETRY_DELAY_MS: u64 = 16;
const TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatPromptDismissalKind {
    CloseInline,
    TransferToAgentChat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferToAgentChatReadyBarrierStep {
    Ready,
    Wait,
    TimedOut,
}

fn should_persist_chat_before_prompt_dismissal(
    save_history: bool,
    dismissal_kind: ChatPromptDismissalKind,
) -> bool {
    save_history && dismissal_kind == ChatPromptDismissalKind::CloseInline
}

fn next_transfer_to_agent_chat_ready_barrier_step(
    is_ready: bool,
    waits_completed: usize,
) -> TransferToAgentChatReadyBarrierStep {
    if is_ready {
        TransferToAgentChatReadyBarrierStep::Ready
    } else if waits_completed < TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS {
        TransferToAgentChatReadyBarrierStep::Wait
    } else {
        TransferToAgentChatReadyBarrierStep::TimedOut
    }
}

impl ChatPrompt {
    fn acknowledge_conversation_command_execution(
        &mut self,
        execution: crate::components::conversation_actions::ConversationCommandExecution,
        cx: &mut Context<Self>,
    ) -> crate::components::conversation_actions::ConversationCommandExecution {
        use crate::components::conversation_actions::ConversationCommandExecution;

        let status_changed = match execution {
            ConversationCommandExecution::Executed => self.command_status.take().is_some(),
            ConversationCommandExecution::Disabled(reason) => {
                let status = reason.as_str();
                if self.command_status.as_deref() == Some(status) {
                    false
                } else {
                    self.command_status = Some(status.to_string());
                    true
                }
            }
            ConversationCommandExecution::Unsupported => false,
        };
        if status_changed {
            cx.notify();
        }
        execution
    }

    pub(crate) fn execute_conversation_key_command(
        &mut self,
        key: &str,
        platform: bool,
        shift: bool,
        cx: &mut Context<Self>,
    ) -> Option<crate::components::conversation_actions::ConversationCommandExecution> {
        use crate::components::conversation_actions::{
            ChatPromptConversationCommand, ConversationDismissTrigger,
        };
        match resolve_chat_input_key_action_with_commands(
            key,
            platform,
            shift,
            &self.conversation_command_bindings(),
        ) {
            ChatInputKeyAction::Escape => {
                Some(self.request_dismiss(ConversationDismissTrigger::Escape, cx))
            }
            ChatInputKeyAction::Submit => {
                Some(self.execute_conversation_command(ChatPromptConversationCommand::Send, cx))
            }
            ChatInputKeyAction::StopStreaming => {
                Some(self.execute_conversation_command(ChatPromptConversationCommand::Stop, cx))
            }
            ChatInputKeyAction::Retry => {
                Some(self.execute_conversation_command(ChatPromptConversationCommand::Retry, cx))
            }
            ChatInputKeyAction::CopyLastResponse => {
                Some(self.execute_conversation_command(
                    ChatPromptConversationCommand::CopyLastResponse,
                    cx,
                ))
            }
            ChatInputKeyAction::DismissCommandW => {
                Some(self.request_dismiss(ConversationDismissTrigger::CommandW, cx))
            }
            _ => None,
        }
    }

    pub(crate) fn execute_conversation_command(
        &mut self,
        command: crate::components::conversation_actions::ChatPromptConversationCommand,
        cx: &mut Context<Self>,
    ) -> crate::components::conversation_actions::ConversationCommandExecution {
        use crate::components::conversation_actions::{
            ChatPromptConversationCommand, ConversationCommandAvailability,
            ConversationCommandExecution, ConversationDismissTrigger,
        };
        let Some(binding) = self
            .conversation_command_bindings()
            .into_iter()
            .find(|binding| binding.handler == command)
        else {
            return self.acknowledge_conversation_command_execution(
                ConversationCommandExecution::Unsupported,
                cx,
            );
        };
        if let ConversationCommandAvailability::Disabled { reason } =
            binding.descriptor.availability
        {
            return self.acknowledge_conversation_command_execution(
                ConversationCommandExecution::Disabled(reason),
                cx,
            );
        }
        match command {
            ChatPromptConversationCommand::Send => self.handle_submit(cx),
            ChatPromptConversationCommand::Stop => self.stop_streaming(cx),
            ChatPromptConversationCommand::Retry => {
                if !self.retry_latest_failed_response(cx) {
                    return self.acknowledge_conversation_command_execution(
                        ConversationCommandExecution::Unsupported,
                        cx,
                    );
                }
            }
            ChatPromptConversationCommand::Dismiss => {
                return self.request_dismiss(ConversationDismissTrigger::CloseButton, cx);
            }
            ChatPromptConversationCommand::CopyLastResponse => {
                if !self.handle_copy_last_response(cx) {
                    return self.acknowledge_conversation_command_execution(
                        ConversationCommandExecution::Unsupported,
                        cx,
                    );
                }
            }
        }
        self.acknowledge_conversation_command_execution(ConversationCommandExecution::Executed, cx)
    }

    pub(crate) fn request_dismiss(
        &mut self,
        trigger: crate::components::conversation_actions::ConversationDismissTrigger,
        cx: &mut Context<Self>,
    ) -> crate::components::conversation_actions::ConversationCommandExecution {
        use crate::components::conversation_actions::{
            ConversationCommandExecution, ConversationDismissDecision, ConversationDismissFacts,
            ConversationOverlayFacts,
        };
        let Some(binding) = self.dismiss_binding.clone() else {
            return self.acknowledge_conversation_command_execution(
                ConversationCommandExecution::Unsupported,
                cx,
            );
        };
        let execution =
            match crate::components::conversation_actions::resolve_conversation_dismissal(
                ConversationDismissFacts {
                    overlays: ConversationOverlayFacts::default(),
                    response_in_progress: self.is_streaming(),
                    active_work: binding.active_work,
                },
                trigger,
            ) {
                ConversationDismissDecision::DismissConversation => {
                    if should_persist_chat_before_prompt_dismissal(
                        self.save_history,
                        ChatPromptDismissalKind::CloseInline,
                    ) {
                        self.save_to_database();
                    }
                    (binding.callback)(ChatPromptDismissRequest {
                        prompt_id: Arc::from(self.id.as_str()),
                        trigger,
                    });
                    ConversationCommandExecution::Executed
                }
                ConversationDismissDecision::Blocked(reason) => {
                    ConversationCommandExecution::Disabled(reason)
                }
                ConversationDismissDecision::DismissOverlay(_) => {
                    // ChatPrompt owns no overlay. Parent hosts resolve their own
                    // Actions/portal/modal stack before forwarding a route here.
                    ConversationCommandExecution::Unsupported
                }
            };
        self.acknowledge_conversation_command_execution(execution, cx)
    }

    pub(crate) fn handle_escape(&mut self, cx: &mut Context<Self>) {
        let _ = self.request_dismiss(
            crate::components::conversation_actions::ConversationDismissTrigger::Escape,
            cx,
        );
    }

    /// Save the current conversation to the AI chats database
    pub(super) fn save_to_database(&self) {
        // Only save if we have messages
        if self.messages.is_empty() {
            logging::log("CHAT", "No messages to save");
            return;
        }

        // Initialize the AI database if needed
        if let Err(e) = ai::init_ai_db() {
            logging::log("CHAT", &format!("Failed to init AI db: {}", e));
            return;
        }

        // Generate title from first user message
        let title = self
            .messages
            .iter()
            .find(|m| m.is_user())
            .map(|m| Chat::generate_title_from_content(m.get_content()))
            .unwrap_or_else(|| "Chat Prompt Conversation".to_string());

        // Determine the model and provider
        let model_id = self.model.clone().unwrap_or_else(|| "unknown".to_string());
        let provider = self
            .models
            .iter()
            .find(|m| m.name == model_id || m.id == model_id)
            .map(|m| m.provider.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Create the chat record with ChatPrompt source
        let chat = Chat::new(&model_id, &provider).with_source(ChatSource::ChatPrompt);
        let mut chat = chat;
        chat.set_title(&title);

        // Save the chat
        if let Err(e) = ai::create_chat(&chat) {
            logging::log("CHAT", &format!("Failed to save chat: {}", e));
            return;
        }

        // Save all messages
        for (i, msg) in self.messages.iter().enumerate() {
            let role = if msg.is_user() {
                MessageRole::User
            } else {
                MessageRole::Assistant
            };

            let message = Message::new(chat.id, role, msg.get_content());
            if let Err(e) = ai::save_message(&message) {
                logging::log("CHAT", &format!("Failed to save message {}: {}", i, e));
            }
        }

        logging::log(
            "CHAT",
            &format!(
                "Saved conversation with {} messages (id: {})",
                self.messages.len(),
                chat.id
            ),
        );
    }

    pub fn handle_continue_in_chat(&mut self, cx: &mut Context<Self>) {
        self.transfer_to_agent_chat(cx);
    }

    pub fn handle_expand_full_chat(&mut self, cx: &mut Context<Self>) {
        self.transfer_to_agent_chat(cx);
    }

    /// Shared handoff: collect the transcript, reset inline state, dismiss,
    /// then open Agent Chat with the transcript staged in the composer.
    fn transfer_to_agent_chat(&mut self, cx: &mut Context<Self>) {
        let transfer_start = std::time::Instant::now();
        tracing::info!(
            action = "transfer_to_agent_chat",
            "=== BEACHBALL TRACE: transfer_to_agent_chat START ==="
        );
        logging::log("CHAT", "Transfer conversation to Agent Chat");

        // Collect conversation history as a role-labeled transcript so the
        // user can continue the thread from the Agent Chat composer. Image
        // attachments have no composer equivalent and are not transferred.
        let transcript = self
            .messages
            .iter()
            .map(|m| {
                let role = if m.is_user() { "User" } else { "Assistant" };
                format!("{role}: {}", m.get_content())
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let message_count = self.messages.len();
        let image_count = self.messages.iter().filter(|m| m.image.is_some()).count();
        tracing::info!(
            action = "transfer_to_agent_chat",
            message_count = message_count,
            image_count = image_count,
            "Transferring conversation transcript to Agent Chat"
        );

        if should_persist_chat_before_prompt_dismissal(
            self.save_history,
            ChatPromptDismissalKind::TransferToAgentChat,
        ) {
            self.save_to_database();
        } else if self.save_history {
            tracing::info!(
                action = "transfer_to_agent_chat",
                persistence = "transcript_staged_in_composer",
                message_count,
                image_count,
                "Skipping inline save_to_database before Agent Chat handoff"
            );
        }

        // Reset the inline prompt to empty state BEFORE the deferred Agent Chat open
        self.messages.clear();
        self.streaming_message_id = None;
        self.pending_prepared_request = None;
        self.prepared_requests_by_assistant_id.clear();
        self.builtin_replay_payloads.clear();
        self.terminal_outcomes.clear();
        self.builtin_cancel_signal = None;
        self.user_has_scrolled_up = false;
        self.input.clear();
        self.pending_image = None;
        self.pending_image_render = None;
        self.image_render_cache.clear();
        self.mark_conversation_turns_dirty();
        self.ensure_conversation_turns_cache();
        cx.notify();

        tracing::info!(
            action = "transfer_to_agent_chat",
            elapsed_ms = transfer_start.elapsed().as_millis(),
            "BEACHBALL TRACE: state reset done, about to dismiss"
        );

        // Dismiss the main prompt window through the same typed route used by
        // Back/Close; transfer never invents an ambiguous escape fallback.
        if let Some(ref callback) = self.on_continue {
            callback(self.id.clone());
        } else {
            let _ = self.request_dismiss(
                crate::components::conversation_actions::ConversationDismissTrigger::CloseButton,
                cx,
            );
        }

        tracing::info!(
            action = "transfer_to_agent_chat",
            elapsed_ms = transfer_start.elapsed().as_millis(),
            "BEACHBALL TRACE: dismiss done, spawning async open"
        );

        // Defer Agent Chat open so the inline prompt dismisses first,
        // avoiding synchronous transcript work on the original prompt path.
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;

            let open_start = std::time::Instant::now();
            tracing::info!(
                action = "transfer_to_agent_chat",
                "BEACHBALL TRACE: async open starting"
            );

            let open_result = cx.update(|cx| {
                crate::ai::agent_chat::ui::chat_window::open_chat_window(cx).map_err(|error| {
                    format!("failed to open Agent Chat for chat transfer: {error}")
                })
            });

            tracing::info!(
                action = "transfer_to_agent_chat",
                open_elapsed_ms = open_start.elapsed().as_millis(),
                "BEACHBALL TRACE: Agent Chat open complete"
            );

            let handoff_result = match open_result {
                Ok(()) => {
                    tracing::info!(
                        action = "transfer_to_agent_chat",
                        max_waits = TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS,
                        retry_delay_ms = TRANSFER_TO_AGENT_CHAT_READY_RETRY_DELAY_MS,
                        "Waiting for Agent Chat readiness before transcript handoff"
                    );

                    let mut waits_completed = 0usize;
                    loop {
                        let ready_now = cx.update(|_cx| {
                            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
                                .is_some()
                        });
                        match next_transfer_to_agent_chat_ready_barrier_step(
                            ready_now,
                            waits_completed,
                        ) {
                            TransferToAgentChatReadyBarrierStep::Ready => {
                                break cx.update(|cx| {
                                    let Some(entity) = crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity() else {
                                        return Err(
                                            "Agent Chat view unavailable after ready barrier"
                                                .to_string(),
                                        );
                                    };
                                    entity.update(cx, |chat, cx| {
                                        if chat.is_setup_mode() {
                                            return Err(
                                                "Agent Chat is in setup mode".to_string()
                                            );
                                        }
                                        chat.set_input(transcript.clone(), cx);
                                        Ok::<(), String>(())
                                    })
                                });
                            }
                            TransferToAgentChatReadyBarrierStep::Wait => {
                                waits_completed += 1;
                                tracing::debug!(
                                    action = "transfer_to_agent_chat",
                                    waits_completed,
                                    max_waits = TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS,
                                    retry_delay_ms = TRANSFER_TO_AGENT_CHAT_READY_RETRY_DELAY_MS,
                                    "Agent Chat not ready yet; retrying transcript handoff"
                                );
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(
                                        TRANSFER_TO_AGENT_CHAT_READY_RETRY_DELAY_MS,
                                    ))
                                    .await;
                            }
                            TransferToAgentChatReadyBarrierStep::TimedOut => {
                                break Err(format!(
                                    "Agent Chat not ready after open; cannot hand off transcript (waits_completed={waits_completed}, max_waits={}, retry_delay_ms={}, message_count={message_count}, image_count={image_count})",
                                    TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS,
                                    TRANSFER_TO_AGENT_CHAT_READY_RETRY_DELAY_MS,
                                ));
                            }
                        }
                    }
                }
                Err(error) => Err(error),
            };

            match handoff_result {
                Ok(()) => {
                    tracing::info!(
                        action = "transfer_to_agent_chat",
                        message_count,
                        image_count,
                        "Agent Chat opened with staged transcript"
                    );
                }
                Err(error) => {
                    tracing::error!(
                        error = %error,
                        message_count,
                        image_count,
                        "Failed to open Agent Chat for chat transfer"
                    );
                }
            }
        })
        .detach();
    }

    pub fn handle_copy_last_response(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(content) =
            crate::components::conversation_actions::resolve_latest_copyable_assistant_response(
                self.messages
                    .iter()
                    .filter(|message| !message.is_user())
                    .map(|message| message.get_content()),
            )
        else {
            return false;
        };
        self.last_copy_receipt = Some(
            crate::components::conversation_actions::write_exact_conversation_copy(content, cx),
        );
        true
    }

    pub(super) fn handle_clear(&mut self, cx: &mut Context<Self>) {
        logging::log("CHAT", "Clearing conversation (⌘+⌫)");
        self.clear_messages(cx);
    }

    pub(super) fn handle_script_generation_action(
        &mut self,
        action: ScriptGenerationAction,
        cx: &mut Context<Self>,
    ) {
        let Some((prompt_description, raw_response)) = self.latest_script_generation_draft() else {
            self.set_script_generation_status(true, "No generated script to save yet.", cx);
            return;
        };

        logging::log(
            "CHAT_SCRIPT_GEN",
            &format!(
                "state=save_requested action={:?} prompt_len={} response_len={}",
                action,
                prompt_description.len(),
                raw_response.len()
            ),
        );

        let script_path = match crate::ai::script_generation::save_generated_script_from_response(
            &prompt_description,
            &raw_response,
        ) {
            Ok(path) => path,
            Err(error) => {
                self.set_script_generation_status(
                    true,
                    format!("Failed to save script: {}", error),
                    cx,
                );
                return;
            }
        };

        let script_name = script_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "generated script".to_string());

        if action.should_run_after_save() {
            self.set_script_generation_status(false, format!("Running {}...", script_name), cx);
            if let Some(ref callback) = self.on_run_script {
                callback(script_path.clone(), cx);
                self.set_script_generation_status(
                    false,
                    format!("Saved and running {}", script_name),
                    cx,
                );
                logging::log(
                    "CHAT_SCRIPT_GEN",
                    &format!(
                        "state=run_dispatched action={:?} path={}",
                        action,
                        script_path.display()
                    ),
                );
            } else {
                self.set_script_generation_status(
                    true,
                    format!("Saved {} but run action is unavailable", script_name),
                    cx,
                );
                logging::log(
                    "CHAT_SCRIPT_GEN",
                    &format!(
                        "state=run_dispatch_failed action={:?} path={} reason=missing_callback",
                        action,
                        script_path.display()
                    ),
                );
            }
            return;
        }

        self.set_script_generation_status(false, format!("Saved {}", script_name), cx);
        logging::log(
            "CHAT_SCRIPT_GEN",
            &format!(
                "state=saved_only action={:?} path={}",
                action,
                script_path.display()
            ),
        );

        // Notify parent to show CreationFeedback panel
        if let Some(ref callback) = self.on_script_saved {
            callback(script_path, cx);
        }
    }

    // ============================================
    // Actions Menu Methods
    // ============================================

    pub(super) fn toggle_actions_menu(&mut self, _cx: &mut Context<Self>) {
        // Delegate to parent via callback to open standard ActionsDialog
        if let Some(ref callback) = self.on_show_actions {
            tracing::info!(
                event = "toggle_actions_menu.delegated",
                id = %self.id,
                mini_mode = self.mini_mode(),
                "ChatPrompt delegating actions toggle to parent via callback"
            );
            callback(self.id.clone());
        } else {
            tracing::warn!(
                event = "toggle_actions_menu.no_callback",
                id = %self.id,
                "No on_show_actions callback set — actions toggle request dropped"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        next_transfer_to_agent_chat_ready_barrier_step,
        should_persist_chat_before_prompt_dismissal, ChatPromptDismissalKind,
        TransferToAgentChatReadyBarrierStep, TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS,
    };

    #[test]
    fn test_should_persist_chat_before_prompt_dismissal_when_closing_inline_with_history_enabled() {
        assert!(should_persist_chat_before_prompt_dismissal(
            true,
            ChatPromptDismissalKind::CloseInline
        ));
    }

    #[test]
    fn test_should_not_persist_chat_before_prompt_dismissal_when_transferring_to_agent_chat() {
        assert!(!should_persist_chat_before_prompt_dismissal(
            true,
            ChatPromptDismissalKind::TransferToAgentChat
        ));
    }

    #[test]
    fn test_should_not_persist_chat_before_prompt_dismissal_when_history_is_disabled() {
        assert!(!should_persist_chat_before_prompt_dismissal(
            false,
            ChatPromptDismissalKind::CloseInline
        ));
    }

    #[test]
    fn test_next_transfer_to_agent_chat_ready_barrier_step_returns_ready_immediately() {
        assert_eq!(
            next_transfer_to_agent_chat_ready_barrier_step(true, 0),
            TransferToAgentChatReadyBarrierStep::Ready
        );
        assert_eq!(
            next_transfer_to_agent_chat_ready_barrier_step(
                true,
                TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS
            ),
            TransferToAgentChatReadyBarrierStep::Ready
        );
    }

    #[test]
    fn test_next_transfer_to_agent_chat_ready_barrier_step_retries_before_timeout() {
        assert_eq!(
            next_transfer_to_agent_chat_ready_barrier_step(false, 0),
            TransferToAgentChatReadyBarrierStep::Wait
        );
        assert_eq!(
            next_transfer_to_agent_chat_ready_barrier_step(
                false,
                TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS - 1
            ),
            TransferToAgentChatReadyBarrierStep::Wait
        );
        assert_eq!(
            next_transfer_to_agent_chat_ready_barrier_step(
                false,
                TRANSFER_TO_AGENT_CHAT_READY_MAX_WAITS
            ),
            TransferToAgentChatReadyBarrierStep::TimedOut
        );
    }
}
