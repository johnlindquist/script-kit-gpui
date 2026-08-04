use super::*;

impl ScriptListApp {
    pub fn execute_chat_action(&mut self, action_id: &str, cx: &mut Context<Self>) {
        logging::log("ACTIONS", &format!("execute_chat_action: {}", action_id));

        let action_id = action_id.strip_prefix("chat:").unwrap_or(action_id);

        // Handle model selection (action_id starts with "select_model_")
        if let Some(model_id) = action_id.strip_prefix("select_model_") {
            let mut selected_model_name: Option<String> = None;
            if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                let model_id_owned = model_id.to_string();
                entity.update(cx, |chat, cx| {
                    // Find model by ID and set it
                    if let Some(model) = chat.models.iter().find(|m| m.id == model_id_owned) {
                        chat.model = Some(model.name.clone());
                        selected_model_name = Some(model.name.clone());
                        logging::log("CHAT", &format!("Model changed to: {}", model.name));
                        cx.notify();
                    }
                });
            }
            if let Some(model_name) = selected_model_name {
                self.show_hud(format!("Model: {}", model_name), Some(HUD_SHORT_MS), cx);
            }
            return;
        }

        // Handle other chat actions
        match action_id {
            "continue_in_chat" => {
                tracing::info!(
                    event = "execute_chat_action",
                    action = "continue_in_chat",
                    reuses = "handle_continue_in_chat",
                    "Handoff to harness terminal via existing continue_in_chat path"
                );
                if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                    entity.update(cx, |chat, cx| {
                        chat.handle_continue_in_chat(cx);
                    });
                }
            }
            "expand_full_chat" => {
                tracing::info!(
                    event = "execute_chat_action",
                    action = "expand_full_chat",
                    reuses = "handle_expand_full_chat",
                    "Handoff to full AI window via expand_full_chat path"
                );
                if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                    entity.update(cx, |chat, cx| {
                        chat.handle_expand_full_chat(cx);
                    });
                }
            }
            "send" | "stop" | "close" => {
                let command = match action_id {
                    "send" => crate::components::conversation_actions::ChatPromptConversationCommand::Send,
                    "stop" => crate::components::conversation_actions::ChatPromptConversationCommand::Stop,
                    "close" => crate::components::conversation_actions::ChatPromptConversationCommand::Close,
                    _ => unreachable!(),
                };
                if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                    entity.update(cx, |chat, cx| {
                        let _ = chat.execute_conversation_command(command, cx);
                    });
                }
            }
            "copy_response" => {
                if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                    let copied = entity.update(cx, |chat, cx| {
                        chat.execute_conversation_command(
                            crate::components::conversation_actions::ChatPromptConversationCommand::CopyLastResponse,
                            cx,
                        )
                    });
                    if copied {
                        self.show_hud("Copied response".to_string(), Some(HUD_SHORT_MS), cx);
                    }
                }
            }
            "clear_conversation" => {
                let chat_entity = if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                    entity.clone()
                } else {
                    return;
                };

                let message = "Clear this conversation? This cannot be undone.".to_string();
                cx.spawn(async move |this, cx| {
                    let confirmed = match crate::confirm::confirm_with_parent_dialog(
                        cx,
                        crate::confirm::ParentConfirmOptions::destructive(
                            "Clear Conversation",
                            message,
                            "Clear",
                        ),
                        "clear_conversation",
                    )
                    .await
                    {
                        Ok(confirmed) => confirmed,
                        Err(e) => {
                            let _ = this.update(cx, move |this, cx| {
                                tracing::error!(
                                    event = "clear_conversation_dialog_open_failed",
                                    error = %e,
                                    "Failed to open clear conversation dialog"
                                );
                                this.show_error_toast("Failed to open confirmation dialog", cx);
                            });
                            return;
                        }
                    };

                    if !confirmed {
                        return;
                    }

                    let _ = this.update(cx, |_, cx| {
                        chat_entity.update(cx, |chat, cx| {
                            chat.clear_messages(cx);
                        });
                    });
                })
                .detach();
            }
            "capture_screen_area" => {
                if let AppView::ChatPrompt { entity, .. } = &self.current_view {
                    entity.update(cx, |chat, cx| {
                        chat.capture_screen_area_attachment(cx);
                    });
                }
            }
            _ => {
                logging::log("ACTIONS", &format!("Unknown chat action: {}", action_id));
            }
        }
    }
}
