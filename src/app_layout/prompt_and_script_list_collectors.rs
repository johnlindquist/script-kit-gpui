impl ScriptListApp {
    fn collect_chat_prompt_elements(
        &self,
        chat_prompt: &prompts::ChatPrompt,
        limit: usize,
    ) -> (Vec<protocol::ElementInfo>, usize) {
        let command_elements =
            crate::windows::automation_surface_collector::collect_conversation_command_elements(
                &chat_prompt.conversation_command_bindings(),
            );
        let copyable_message_indices: std::collections::HashSet<usize> = chat_prompt
            .messages
            .iter()
            .enumerate()
            .filter_map(|(index, message)| {
                use crate::components::conversation_actions::{
                    TurnCopyRole, turn_copy_eligibility,
                };
                let role = match message.role.as_ref() {
                    Some(protocol::ChatMessageRole::Assistant) => TurnCopyRole::Assistant,
                    None if !message.is_user() => TurnCopyRole::Assistant,
                    _ => TurnCopyRole::NonAnswer,
                };
                turn_copy_eligibility(
                    role,
                    message.get_content().trim().is_empty(),
                    message.streaming,
                )
                .is_present()
                .then_some(index)
            })
            .collect();
        let command_status_count = usize::from(chat_prompt.command_status_text().is_some());
        let total_count = chat_prompt.messages.len()
            + 3
            + command_elements.len()
            + copyable_message_indices.len()
            + command_status_count;
        let mut elements = Vec::with_capacity(limit.min(total_count));

        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "chat-model",
                "Model",
                chat_prompt.model.clone(),
                false,
                Some(0),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            Self::input_element(
                "chat-input",
                chat_prompt
                    .placeholder
                    .clone()
                    .unwrap_or_else(|| "Message".to_string()),
                Some(Self::preview_value(chat_prompt.input.text(), 240)),
                true,
                Some(1),
            ),
        );
        Self::push_limited_element(
            &mut elements,
            limit,
            protocol::ElementInfo::list("chat-messages", chat_prompt.messages.len()),
        );
        for command in command_elements {
            Self::push_limited_element(&mut elements, limit, command);
        }
        if let Some(status) = chat_prompt.command_status_text() {
            Self::push_limited_element(
                &mut elements,
                limit,
                protocol::ElementInfo {
                    semantic_id: "conversation.commandStatus".to_string(),
                    element_type: protocol::ElementType::Panel,
                    text: Some(status.to_string()),
                    value: None,
                    content: None,
                    selected: None,
                    focused: None,
                    index: None,
                    role: Some("conversationCommandStatus".to_string()),
                    kind: Some("disabledAcknowledgement".to_string()),
                    source: Some("ConversationCommandExecution".to_string()),
                    source_name: None,
                    selectable: Some(false),
                    status_kind: Some("disabled".to_string()),
                    action_disabled: Some(status.to_string()),
                    style: None,
                },
            );
        }

        for (index, message) in chat_prompt.messages.iter().enumerate() {
            if elements.len() >= limit {
                break;
            }
            let sender = if message.is_user() {
                "User"
            } else {
                "Assistant"
            };
            let content = message.get_content();
            let text = if content.is_empty() {
                sender.to_string()
            } else {
                format!("{sender}: {}", Self::preview_value(content, 180))
            };
            let preview = Self::preview_value(content, 180);
            let mut message_element = protocol::ElementInfo::redacted_choice(
                index,
                &text,
                &preview,
                index + 1 == chat_prompt.messages.len(),
                protocol::ElementContentKind::UserContent,
            );
            if let Some(message_id) = message.id.as_deref() {
                if let Some(sk_protocol::ai_reliability::AiOutcome::Cancelled { kind, .. }) =
                    chat_prompt.terminal_outcome_for_message(message_id)
                {
                    message_element.status_kind = Some("cancelled".to_string());
                    message_element.kind = Some(
                        match kind {
                            sk_protocol::ai_reliability::CancellationKind::UserStopped => {
                                "userStopped"
                            }
                            sk_protocol::ai_reliability::CancellationKind::UserCancelled => {
                                "userCancelled"
                            }
                            sk_protocol::ai_reliability::CancellationKind::AppShutdown => {
                                "appShutdown"
                            }
                        }
                        .to_string(),
                    );
                }
            }
            Self::push_limited_element(&mut elements, limit, message_element);

            if copyable_message_indices.contains(&index) {
                let descriptor =
                    crate::components::conversation_actions::conversation_command_descriptor(
                        crate::components::conversation_actions::ConversationCommandId::CopyTurn,
                        crate::components::conversation_actions::ConversationCommandAvailability::Enabled,
                    );
                let target_id = message
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("index-{index}"));
                Self::push_limited_element(
                    &mut elements,
                    limit,
                    protocol::ElementInfo {
                        semantic_id: format!("{}:{target_id}", descriptor.semantic_action_id),
                        element_type: protocol::ElementType::Button,
                        text: Some(descriptor.label.to_string()),
                        value: Some(target_id),
                        content: None,
                        selected: Some(false),
                        focused: Some(false),
                        index: Some(index),
                        role: Some("conversationCommand".to_string()),
                        kind: Some(descriptor.semantic_action_id.to_string()),
                        source: Some("ConversationCommandDescriptor".to_string()),
                        source_name: None,
                        selectable: Some(true),
                        status_kind: None,
                        action_disabled: None,
                        style: None,
                    },
                );
            }
        }

        (elements, total_count)
    }

    pub(crate) fn script_list_result_label(result: &scripts::SearchResult) -> String {
        match result {
            scripts::SearchResult::Script(m) => m.script.name.clone(),
            scripts::SearchResult::Scriptlet(m) => {
                let vars = crate::context_templates::ContextTemplateVars::from_frontmost_tracker();
                crate::context_templates::substitute_context_vars(&m.scriptlet.name, &vars)
                    .into_owned()
            }
            scripts::SearchResult::BuiltIn(m) => m.entry.name.clone(),
            scripts::SearchResult::App(m) => m.app.name.clone(),
            scripts::SearchResult::Window(m) => m.window.title.clone(),
            scripts::SearchResult::File(m) => m.file.name.clone(),
            scripts::SearchResult::Note(m) => m.title.clone(),
            scripts::SearchResult::BrainHit(m) => m.hit.title.clone(),
            scripts::SearchResult::BrainInboxItem(m) => m.item.title.clone(),
            scripts::SearchResult::Todo(m) => m.hit.title.clone(),
            scripts::SearchResult::AgentChatHistory(m) => m.entry.title_display().to_string(),
            scripts::SearchResult::AiVault(m) => m.hit.safe_title.clone(),
            scripts::SearchResult::ClipboardHistory(m) => m.title.clone(),
            scripts::SearchResult::DictationHistory(m) => m.preview.clone(),
            scripts::SearchResult::BrowserTab(m) => m.hit.title.clone(),
            scripts::SearchResult::BrowserHistory(m) => m.hit.title.clone(),
            scripts::SearchResult::Agent(m) => m.agent.name.clone(),
            scripts::SearchResult::Skill(m) => m.skill.title.clone(),
            scripts::SearchResult::Fallback(m) => m.display_label(),
            scripts::SearchResult::ScriptIssue(m) => m.title.clone(),
            scripts::SearchResult::SpineProjection(row) => row.title.to_string(),
            scripts::SearchResult::Flow(m) => m.display_name.clone(),
        }
    }
}
