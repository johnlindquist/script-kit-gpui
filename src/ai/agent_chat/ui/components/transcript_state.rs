//! Pure Agent Chat transcript identity and pending-response placement.

use super::super::thread::{
    AgentChatThreadMessage, AgentChatThreadMessageRole, AgentChatThreadStatus,
};
use super::super::ui_variant::AgentChatUiVariant;

fn transcript_row_role_slug(role: AgentChatThreadMessageRole) -> &'static str {
    match role {
        AgentChatThreadMessageRole::User => "user",
        AgentChatThreadMessageRole::Assistant => "assistant",
        AgentChatThreadMessageRole::Thought => "thought",
        AgentChatThreadMessageRole::Tool => "tool",
        AgentChatThreadMessageRole::System => "system",
        AgentChatThreadMessageRole::Error => "error",
    }
}

pub(super) fn transcript_row_fidelity_id(message: &AgentChatThreadMessage) -> String {
    format!(
        "agent-chat-transcript-row-{}-{}",
        transcript_row_role_slug(message.role),
        message.id
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PendingActivityPlacement {
    Hidden,
    TailSentinel,
    EmptyAssistantRow(usize),
}

pub(super) struct AssistantMessageRuntime {
    pub(super) ui_variant: AgentChatUiVariant,
    pub(super) show_pending_activity: bool,
    pub(super) thread_status: AgentChatThreadStatus,
}

pub(super) fn pending_activity_placement(
    messages: &[AgentChatThreadMessage],
    pending: bool,
) -> PendingActivityPlacement {
    if !pending {
        return PendingActivityPlacement::Hidden;
    }

    let Some(user_index) = messages
        .iter()
        .rposition(|message| matches!(message.role, AgentChatThreadMessageRole::User))
    else {
        return PendingActivityPlacement::TailSentinel;
    };

    let response = messages[user_index + 1..]
        .iter()
        .enumerate()
        .find(|(_, message)| {
            matches!(
                message.role,
                AgentChatThreadMessageRole::Assistant | AgentChatThreadMessageRole::Error
            )
        })
        .map(|(relative_index, message)| (user_index + 1 + relative_index, message));

    match response {
        None => PendingActivityPlacement::TailSentinel,
        Some((message_index, message))
            if matches!(message.role, AgentChatThreadMessageRole::Assistant)
                && message.body.trim().is_empty() =>
        {
            PendingActivityPlacement::EmptyAssistantRow(message_index)
        }
        Some(_) => PendingActivityPlacement::Hidden,
    }
}
