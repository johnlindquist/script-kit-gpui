//! Confirmed, symlink-safe deletion of detached Agent Chat history.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ConfirmedAgentChatHistoryDeletion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChatHistoryDeletionTarget {
    HistoryIndex,
    Conversations,
    PromptHistory,
    HistoryAttachments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChatHistoryDeletionError {
    ConfirmationRequired,
    UnsafeTarget(AgentChatHistoryDeletionTarget),
    InspectFailed(AgentChatHistoryDeletionTarget, std::io::ErrorKind),
    RemoveFailed(AgentChatHistoryDeletionTarget, std::io::ErrorKind),
}

impl AgentChatHistoryDeletionError {
    pub(super) const fn safe_code(self) -> &'static str {
        match self {
            Self::ConfirmationRequired => "confirmation_required",
            Self::UnsafeTarget(AgentChatHistoryDeletionTarget::HistoryIndex) => {
                "unsafe_history_index"
            }
            Self::UnsafeTarget(AgentChatHistoryDeletionTarget::Conversations) => {
                "unsafe_conversations_directory"
            }
            Self::UnsafeTarget(AgentChatHistoryDeletionTarget::PromptHistory) => {
                "unsafe_prompt_history"
            }
            Self::UnsafeTarget(AgentChatHistoryDeletionTarget::HistoryAttachments) => {
                "unsafe_history_attachments_directory"
            }
            Self::InspectFailed(AgentChatHistoryDeletionTarget::HistoryIndex, _) => {
                "history_index_inspection_failed"
            }
            Self::InspectFailed(AgentChatHistoryDeletionTarget::Conversations, _) => {
                "conversations_directory_inspection_failed"
            }
            Self::InspectFailed(AgentChatHistoryDeletionTarget::PromptHistory, _) => {
                "prompt_history_inspection_failed"
            }
            Self::InspectFailed(AgentChatHistoryDeletionTarget::HistoryAttachments, _) => {
                "history_attachments_directory_inspection_failed"
            }
            Self::RemoveFailed(AgentChatHistoryDeletionTarget::HistoryIndex, _) => {
                "history_index_removal_failed"
            }
            Self::RemoveFailed(AgentChatHistoryDeletionTarget::Conversations, _) => {
                "conversations_directory_removal_failed"
            }
            Self::RemoveFailed(AgentChatHistoryDeletionTarget::PromptHistory, _) => {
                "prompt_history_removal_failed"
            }
            Self::RemoveFailed(AgentChatHistoryDeletionTarget::HistoryAttachments, _) => {
                "history_attachments_directory_removal_failed"
            }
        }
    }

    pub(super) const fn io_error_kind(self) -> Option<std::io::ErrorKind> {
        match self {
            Self::InspectFailed(_, kind) | Self::RemoveFailed(_, kind) => Some(kind),
            Self::ConfirmationRequired | Self::UnsafeTarget(_) => None,
        }
    }
}

fn inspect_agent_chat_history_deletion_target(
    path: &std::path::Path,
    target: AgentChatHistoryDeletionTarget,
) -> Result<bool, AgentChatHistoryDeletionError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            let expected_type = match target {
                AgentChatHistoryDeletionTarget::HistoryIndex
                | AgentChatHistoryDeletionTarget::PromptHistory => metadata.is_file(),
                AgentChatHistoryDeletionTarget::Conversations
                | AgentChatHistoryDeletionTarget::HistoryAttachments => metadata.is_dir(),
            };
            if metadata.file_type().is_symlink() || !expected_type {
                Err(AgentChatHistoryDeletionError::UnsafeTarget(target))
            } else {
                Ok(true)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AgentChatHistoryDeletionError::InspectFailed(
            target,
            error.kind(),
        )),
    }
}

/// Delete only the exact detached Agent Chat history targets. All filesystem
/// types are checked without following symlinks before any target is touched.
pub(super) fn clear_agent_chat_history_at(
    kit_path: &std::path::Path,
    confirmation: Option<ConfirmedAgentChatHistoryDeletion>,
) -> Result<(), AgentChatHistoryDeletionError> {
    if confirmation.is_none() {
        return Err(AgentChatHistoryDeletionError::ConfirmationRequired);
    }

    let history_path = kit_path.join("agent_chat-history.jsonl");
    let conversations_path = kit_path.join("agent_chat-conversations");
    let prompt_history_path = kit_path.join("agent_chat-prompt-history.jsonl");
    let attachments_path = kit_path.join("agent_chat-history-attachments");
    let history_exists = inspect_agent_chat_history_deletion_target(
        &history_path,
        AgentChatHistoryDeletionTarget::HistoryIndex,
    )?;
    let conversations_exist = inspect_agent_chat_history_deletion_target(
        &conversations_path,
        AgentChatHistoryDeletionTarget::Conversations,
    )?;
    let prompt_history_exists = inspect_agent_chat_history_deletion_target(
        &prompt_history_path,
        AgentChatHistoryDeletionTarget::PromptHistory,
    )?;
    let attachments_exist = inspect_agent_chat_history_deletion_target(
        &attachments_path,
        AgentChatHistoryDeletionTarget::HistoryAttachments,
    )?;

    if history_exists {
        std::fs::remove_file(&history_path).map_err(|error| {
            AgentChatHistoryDeletionError::RemoveFailed(
                AgentChatHistoryDeletionTarget::HistoryIndex,
                error.kind(),
            )
        })?;
    }
    if conversations_exist {
        std::fs::remove_dir_all(&conversations_path).map_err(|error| {
            AgentChatHistoryDeletionError::RemoveFailed(
                AgentChatHistoryDeletionTarget::Conversations,
                error.kind(),
            )
        })?;
    }
    if prompt_history_exists {
        std::fs::remove_file(&prompt_history_path).map_err(|error| {
            AgentChatHistoryDeletionError::RemoveFailed(
                AgentChatHistoryDeletionTarget::PromptHistory,
                error.kind(),
            )
        })?;
    }
    if attachments_exist {
        std::fs::remove_dir_all(&attachments_path).map_err(|error| {
            AgentChatHistoryDeletionError::RemoveFailed(
                AgentChatHistoryDeletionTarget::HistoryAttachments,
                error.kind(),
            )
        })?;
    }

    Ok(())
}
