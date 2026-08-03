use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::thread::{AgentChatThread, AgentChatThreadMessageRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatExportPurpose {
    SubmitTurn,
    CopyTranscript,
    AutomationSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportedAgentChatMessage {
    pub(crate) id: u64,
    pub(crate) role: String,
    pub(crate) body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportedContextPart {
    pub(crate) stable_id: String,
    pub(crate) part: crate::ai::message_parts::AiContextPart,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentChatConversationExport {
    pub(crate) messages: Vec<ExportedAgentChatMessage>,
    pub(crate) context_parts: Vec<ExportedContextPart>,
}

pub(crate) trait StableAgentChatExportId {
    fn stable_export_id(&self) -> String;
}

impl StableAgentChatExportId for crate::ai::message_parts::AiContextPart {
    fn stable_export_id(&self) -> String {
        match self {
            Self::ResourceUri { uri, label } => format!("resource:{uri}:{label}"),
            Self::FilePath { path, label } => format!("file:{path}:{label}"),
            Self::SkillFile {
                path,
                slash_name,
                owner_label,
                ..
            } => format!("skill:{owner_label}:{slash_name}:{path}"),
            Self::FocusedTarget { target, label } => {
                format!("focused:{}:{}:{label}", target.kind, target.semantic_id)
            }
            Self::AmbientContext { label } => format!("ambient:{label}"),
            Self::TextBlock { source, label, .. } => format!("text:{source}:{label}"),
        }
    }
}

impl AgentChatThread {
    pub(crate) fn export_conversation(
        &self,
        purpose: AgentChatExportPurpose,
    ) -> AgentChatConversationExport {
        let mut seen_messages = HashSet::new();
        let messages =
            self.messages
                .iter()
                .filter(|message| match purpose {
                    AgentChatExportPurpose::CopyTranscript => !message.body.trim().is_empty(),
                    AgentChatExportPurpose::SubmitTurn
                    | AgentChatExportPurpose::AutomationSnapshot => true,
                })
                .filter(|message| {
                    let key = format!(
                        "{:?}:{}:{}",
                        message.role,
                        message.tool_call_id.as_deref().unwrap_or_default(),
                        message.body
                    );
                    seen_messages.insert(key)
                })
                .map(|message| ExportedAgentChatMessage {
                    id: message.id,
                    role: match message.role {
                        AgentChatThreadMessageRole::User => "user",
                        AgentChatThreadMessageRole::Assistant => "assistant",
                        AgentChatThreadMessageRole::Thought => "thought",
                        AgentChatThreadMessageRole::Tool => "tool",
                        AgentChatThreadMessageRole::System => "system",
                        AgentChatThreadMessageRole::Error => "error",
                    }
                    .to_string(),
                    body: message.body.to_string(),
                    tool_call_id: message.tool_call_id.clone(),
                })
                .collect();

        let mut seen_parts = HashSet::new();
        let context_parts = self
            .pending_context_items()
            .iter()
            .filter_map(|item| {
                let stable_id = item.part.stable_export_id();
                seen_parts
                    .insert(stable_id.clone())
                    .then(|| ExportedContextPart {
                        stable_id,
                        part: item.part.clone(),
                    })
            })
            .collect();

        AgentChatConversationExport {
            messages,
            context_parts,
        }
    }
}
