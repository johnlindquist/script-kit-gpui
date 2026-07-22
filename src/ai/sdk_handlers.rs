//! AI SDK Protocol Handlers
//!
//! This module handles AI SDK protocol messages from scripts.
//! It converts between protocol types and storage/window operations.

#![allow(clippy::result_large_err)]

use tracing::{debug, error, info};

use crate::protocol::{AiChatInfo, AiContextPartInput, AiMessageInfo, Message};

use super::model::{Chat, ChatId, ImageAttachment, Message as AiMessage, MessageRole};
use super::storage;

/// Convert a Chat from storage to AiChatInfo for protocol
fn chat_to_info(chat: &Chat, message_count: usize) -> AiChatInfo {
    AiChatInfo {
        id: chat.id.as_str().into(),
        title: chat.title.clone().into(),
        model_id: chat.model_id.clone().into(),
        provider: chat.provider.clone().into(),
        created_at: chat.created_at.to_rfc3339(),
        updated_at: chat.updated_at.to_rfc3339(),
        is_deleted: chat.deleted_at.is_some(),
        preview: None, // Could add first message preview later
        message_count,
    }
}

/// Convert a Message from storage to AiMessageInfo for protocol
fn message_to_info(msg: &AiMessage) -> AiMessageInfo {
    AiMessageInfo {
        id: msg.id.to_string().into(),
        role: msg.role.as_str().into(),
        content: msg.content.clone().into(),
        created_at: msg.created_at.to_rfc3339(),
        tokens_used: msg.tokens_used,
    }
}

fn ai_request_error(request_id: String, code: &'static str, message: impl Into<String>) -> Message {
    Message::AiError {
        subscription_id: None,
        request_id: Some(request_id),
        code: code.to_string(),
        message: message.into(),
    }
}

fn parse_chat_id_for_mutation(
    request_id: &str,
    chat_id: &str,
) -> std::result::Result<ChatId, Message> {
    ChatId::parse(chat_id).ok_or_else(|| {
        ai_request_error(
            request_id.to_string(),
            "AI_INVALID_CHAT_ID",
            format!("Invalid chat ID: {chat_id}"),
        )
    })
}

fn active_chat_for_mutation(request_id: &str, chat_id: &str) -> std::result::Result<Chat, Message> {
    let parsed_id = parse_chat_id_for_mutation(request_id, chat_id)?;
    match storage::get_chat(&parsed_id) {
        Ok(Some(chat)) if chat.deleted_at.is_none() => Ok(chat),
        Ok(Some(_)) => Err(ai_request_error(
            request_id.to_string(),
            "AI_CHAT_DELETED",
            format!("Chat is deleted: {chat_id}"),
        )),
        Ok(None) => Err(ai_request_error(
            request_id.to_string(),
            "AI_CHAT_NOT_FOUND",
            format!("Chat not found: {chat_id}"),
        )),
        Err(e) => Err(ai_request_error(
            request_id.to_string(),
            "AI_CHAT_LOOKUP_FAILED",
            format!("Failed to load chat {chat_id}: {e}"),
        )),
    }
}

fn parse_message_role(request_id: &str, role: &str) -> std::result::Result<MessageRole, Message> {
    MessageRole::parse(role).ok_or_else(|| {
        ai_request_error(
            request_id.to_string(),
            "AI_INVALID_MESSAGE_ROLE",
            format!("Invalid message role: {role}"),
        )
    })
}

/// Handle AiIsOpen request - check if a chat surface is open
pub fn handle_ai_is_open(request_id: String) -> Message {
    // The legacy AI window is gone; the detached Agent Chat window is the
    // remaining standalone chat surface.
    let is_open = crate::ai::agent_chat::ui::chat_window::is_chat_window_open();
    let active_chat_id = None;

    info!(
        request_id = %request_id,
        is_open = is_open,
        active_chat_id = ?active_chat_id,
        "ai_sdk.is_open"
    );

    Message::AiIsOpenResult {
        request_id,
        is_open,
        active_chat_id,
    }
}

/// Handle AiGetActiveChat request - get info about active chat
pub fn handle_ai_get_active_chat(request_id: String) -> Message {
    // The legacy AI window's active-chat tracking is gone; report the most
    // recently updated chat as the active one.
    let chat = match storage::get_all_chats() {
        Ok(chats) => chats.into_iter().next(),
        Err(e) => {
            error!(error = %e, "Failed to get chats for AiGetActiveChat");
            None
        }
    };

    let chat_info = chat.map(|c| {
        let msg_count = storage::get_chat_messages(&c.id)
            .map(|msgs| msgs.len())
            .unwrap_or(0);
        chat_to_info(&c, msg_count)
    });

    info!(
        request_id = %request_id,
        has_chat = chat_info.is_some(),
        "ai_sdk.get_active_chat"
    );

    Message::AiActiveChatResult {
        request_id,
        chat: chat_info,
    }
}

/// Handle AiListChats request - list all chats
pub fn handle_ai_list_chats(
    request_id: String,
    limit: Option<usize>,
    include_deleted: bool,
) -> Message {
    let mut chats = match storage::get_all_chats() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to list chats");
            return Message::AiChatListResult {
                request_id,
                chats: vec![],
                total_count: 0,
            };
        }
    };

    if include_deleted {
        if let Ok(deleted) = storage::get_deleted_chats() {
            chats.extend(deleted);
        }
    }

    let total_count = chats.len();

    // Apply limit
    if let Some(limit) = limit {
        chats.truncate(limit);
    }

    let chat_infos: Vec<AiChatInfo> = chats
        .iter()
        .map(|c| {
            let msg_count = storage::get_chat_messages(&c.id)
                .map(|msgs| msgs.len())
                .unwrap_or(0);
            chat_to_info(c, msg_count)
        })
        .collect();

    info!(
        request_id = %request_id,
        count = chat_infos.len(),
        total = total_count,
        "ai_sdk.list_chats"
    );

    Message::AiChatListResult {
        request_id,
        chats: chat_infos,
        total_count,
    }
}

/// Handle AiGetConversation request - get messages from a chat
pub fn handle_ai_get_conversation(
    request_id: String,
    chat_id: Option<String>,
    limit: Option<usize>,
) -> Message {
    // Get chat ID - use provided or try to get active
    let target_chat_id = match chat_id {
        Some(id) => match ChatId::parse(&id) {
            Some(cid) => cid,
            None => {
                error!(chat_id = %id, "Invalid chat ID format");
                return Message::AiConversationResult {
                    request_id,
                    chat_id: id,
                    messages: vec![],
                    has_more: false,
                };
            }
        },
        None => {
            // Fall back to the most recently updated chat
            match storage::get_all_chats() {
                Ok(chats) => match chats.into_iter().next() {
                    Some(c) => c.id,
                    None => {
                        return Message::AiConversationResult {
                            request_id,
                            chat_id: String::new(),
                            messages: vec![],
                            has_more: false,
                        };
                    }
                },
                Err(e) => {
                    error!(error = %e, "Failed to get chats for conversation");
                    return Message::AiConversationResult {
                        request_id,
                        chat_id: String::new(),
                        messages: vec![],
                        has_more: false,
                    };
                }
            }
        }
    };

    let messages = match limit {
        Some(lim) => storage::get_recent_messages(&target_chat_id, lim),
        None => storage::get_chat_messages(&target_chat_id),
    };

    let (message_infos, has_more) = match messages {
        Ok(msgs) => {
            let total = msgs.len();
            let infos: Vec<AiMessageInfo> = msgs.iter().map(message_to_info).collect();
            let has_more = limit.map(|l| total >= l).unwrap_or(false);
            (infos, has_more)
        }
        Err(e) => {
            error!(error = %e, chat_id = %target_chat_id, "Failed to get messages");
            (vec![], false)
        }
    };

    info!(
        request_id = %request_id,
        chat_id = %target_chat_id,
        message_count = message_infos.len(),
        "ai_sdk.get_conversation"
    );

    Message::AiConversationResult {
        request_id,
        chat_id: target_chat_id.as_str(),
        messages: message_infos,
        has_more,
    }
}

/// Handle AiDeleteChat request - delete a chat
pub fn handle_ai_delete_chat(request_id: String, chat_id: String, permanent: bool) -> Message {
    let parsed_id = match ChatId::parse(&chat_id) {
        Some(id) => id,
        None => {
            return Message::AiChatDeleted {
                request_id,
                success: false,
                error: Some(format!("Invalid chat ID: {}", chat_id)),
            };
        }
    };

    let result = if permanent {
        storage::delete_chat_permanently(&parsed_id)
    } else {
        storage::delete_chat(&parsed_id)
    };

    match result {
        Ok(()) => {
            info!(chat_id = %chat_id, permanent = permanent, "Chat deleted via SDK");
            Message::AiChatDeleted {
                request_id,
                success: true,
                error: None,
            }
        }
        Err(e) => {
            error!(error = %e, chat_id = %chat_id, "Failed to delete chat");
            Message::AiChatDeleted {
                request_id,
                success: false,
                error: Some(e.to_string()),
            }
        }
    }
}

/// Handle AiAppendMessage request - append a stored message without streaming.
pub fn handle_ai_append_message(
    request_id: String,
    chat_id: String,
    content: String,
    role: String,
) -> Message {
    let chat = match active_chat_for_mutation(&request_id, &chat_id) {
        Ok(chat) => chat,
        Err(response) => return response,
    };
    let role = match parse_message_role(&request_id, &role) {
        Ok(role) => role,
        Err(response) => return response,
    };

    let message = AiMessage::new(chat.id, role, content);
    if let Err(e) = storage::save_message(&message) {
        error!(error = %e, chat_id = %chat_id, "Failed to append AI message via SDK");
        return ai_request_error(
            request_id,
            "AI_MESSAGE_APPEND_FAILED",
            format!("Failed to append message: {e}"),
        );
    }

    info!(
        request_id = %request_id,
        chat_id = %chat_id,
        message_id = %message.id,
        role = %message.role,
        "ai_sdk.append_message"
    );

    Message::AiMessageAppended {
        request_id,
        message_id: message.id,
        chat_id,
    }
}

/// Handle AiSendMessage request - persist a user message for an existing chat.
///
/// This storage-backed path does not own an active Agent Chat turn, so it reports
/// `streamingStarted:false` instead of pretending a model response started.
pub fn handle_ai_send_message(
    request_id: String,
    chat_id: String,
    content: String,
    image: Option<String>,
    parts: Vec<AiContextPartInput>,
) -> Message {
    if !parts.is_empty() {
        return ai_request_error(
            request_id,
            "AI_CONTEXT_PARTS_UNSUPPORTED",
            "aiSendMessage context parts require the Agent Chat UI resolver and are not supported for direct existing-chat mutation yet",
        );
    }

    let chat = match active_chat_for_mutation(&request_id, &chat_id) {
        Ok(chat) => chat,
        Err(response) => return response,
    };

    let mut message = AiMessage::user(chat.id, content);
    if let Some(image) = image {
        message.images.push(ImageAttachment::png(image));
    }

    if let Err(e) = storage::save_message(&message) {
        error!(error = %e, chat_id = %chat_id, "Failed to send AI message via SDK");
        return ai_request_error(
            request_id,
            "AI_MESSAGE_SEND_FAILED",
            format!("Failed to send message: {e}"),
        );
    }

    info!(
        request_id = %request_id,
        chat_id = %chat_id,
        user_message_id = %message.id,
        has_image = !message.images.is_empty(),
        "ai_sdk.send_message"
    );

    Message::AiMessageSent {
        request_id,
        user_message_id: message.id,
        chat_id,
        streaming_started: false,
    }
}

/// Handle AiSetSystemPrompt request - insert or update the stored system prompt.
pub fn handle_ai_set_system_prompt(request_id: String, chat_id: String, prompt: String) -> Message {
    let chat = match active_chat_for_mutation(&request_id, &chat_id) {
        Ok(chat) => chat,
        Err(Message::AiError { message, code, .. }) => {
            return Message::AiSystemPromptSet {
                request_id,
                success: false,
                error: Some(format!("{code}: {message}")),
            };
        }
        Err(response) => return response,
    };

    let existing_system_message = match storage::get_chat_messages(&chat.id) {
        Ok(messages) => messages
            .into_iter()
            .find(|message| message.role == MessageRole::System),
        Err(e) => {
            error!(error = %e, chat_id = %chat_id, "Failed to load messages for system prompt mutation");
            return Message::AiSystemPromptSet {
                request_id,
                success: false,
                error: Some(format!("AI_MESSAGES_LOOKUP_FAILED: {e}")),
            };
        }
    };

    let mut message = existing_system_message
        .unwrap_or_else(|| AiMessage::new(chat.id, MessageRole::System, String::new()));
    message.content = prompt;
    message.images.clear();
    message.tokens_used = None;

    match storage::save_message(&message) {
        Ok(()) => {
            info!(
                request_id = %request_id,
                chat_id = %chat_id,
                message_id = %message.id,
                "ai_sdk.set_system_prompt"
            );
            Message::AiSystemPromptSet {
                request_id,
                success: true,
                error: None,
            }
        }
        Err(e) => {
            error!(error = %e, chat_id = %chat_id, "Failed to set system prompt via SDK");
            Message::AiSystemPromptSet {
                request_id,
                success: false,
                error: Some(format!("AI_SYSTEM_PROMPT_SET_FAILED: {e}")),
            }
        }
    }
}

/// Handle AiFocus request - focus the AI window
pub fn handle_ai_focus(request_id: String) -> Option<Message> {
    // This needs to be handled by the UI thread, return None to signal forwarding needed
    debug!(request_id = %request_id, "AiFocus needs UI thread handling");
    None
}

/// Handle AiGetStreamingStatus request
pub fn handle_ai_get_streaming_status(
    request_id: String,
    query_chat_id: Option<String>,
) -> Message {
    // The legacy AI window published a global streaming snapshot; with it gone
    // there is no cross-window streaming state to report.
    let (is_streaming, chat_id, partial_content) = (false, None, None);
    let _ = query_chat_id;

    info!(
        request_id = %request_id,
        is_streaming = is_streaming,
        chat_id = ?chat_id,
        "ai_sdk.get_streaming_status"
    );

    Message::AiStreamingStatusResult {
        request_id,
        is_streaming,
        chat_id,
        partial_content,
    }
}

/// Check if a message is an AI SDK message that can be handled directly
/// Returns Some(response) if handled, None if needs UI thread
pub fn try_handle_ai_message(msg: &Message) -> Option<Message> {
    match msg {
        Message::AiIsOpen { request_id } => Some(handle_ai_is_open(request_id.clone())),

        Message::AiGetActiveChat { request_id } => {
            Some(handle_ai_get_active_chat(request_id.clone()))
        }

        Message::AiListChats {
            request_id,
            limit,
            include_deleted,
        } => Some(handle_ai_list_chats(
            request_id.clone(),
            *limit,
            *include_deleted,
        )),

        Message::AiGetConversation {
            request_id,
            chat_id,
            limit,
        } => Some(handle_ai_get_conversation(
            request_id.clone(),
            chat_id.clone(),
            *limit,
        )),

        Message::AiDeleteChat {
            request_id,
            chat_id,
            permanent,
        } => Some(handle_ai_delete_chat(
            request_id.clone(),
            chat_id.clone(),
            *permanent,
        )),

        Message::AiGetStreamingStatus {
            request_id,
            chat_id,
        } => Some(handle_ai_get_streaming_status(
            request_id.clone(),
            chat_id.clone(),
        )),

        Message::AiAppendMessage {
            request_id,
            chat_id,
            content,
            role,
        } => Some(handle_ai_append_message(
            request_id.clone(),
            chat_id.clone(),
            content.clone(),
            role.clone(),
        )),

        Message::AiSendMessage {
            request_id,
            chat_id,
            content,
            image,
            parts,
        } => Some(handle_ai_send_message(
            request_id.clone(),
            chat_id.clone(),
            content.clone(),
            image.clone(),
            parts.clone(),
        )),

        Message::AiSetSystemPrompt {
            request_id,
            chat_id,
            prompt,
        } => Some(handle_ai_set_system_prompt(
            request_id.clone(),
            chat_id.clone(),
            prompt.clone(),
        )),

        // These need UI thread or script-owned response channels - return None
        Message::AiFocus { .. }
        | Message::AiStartChat { .. }
        | Message::AiSubscribe { .. }
        | Message::AiUnsubscribe { .. } => None,

        // Not an AI message
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_is_open_when_closed() {
        let response = handle_ai_is_open("test-123".to_string());
        match response {
            Message::AiIsOpenResult {
                request_id,
                is_open,
                ..
            } => {
                assert_eq!(request_id, "test-123");
                assert!(!is_open); // Window not open in tests
            }
            _ => panic!("Expected AiIsOpenResult"),
        }
    }
}
