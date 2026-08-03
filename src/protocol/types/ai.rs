use gpui::SharedString;
use serde::{Deserialize, Serialize};

// ============================================================
// AI CHAT SDK API
// ============================================================

/// A typed context part that can be attached to AI protocol messages.
///
/// Scripts can send these alongside `message`/`content` to inject
/// resource-backed or file-backed context into the AI composer pipeline.
/// The `parts` field is optional and defaults to an empty vec for
/// wire compatibility with existing payloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AiContextPartInput {
    /// An MCP resource URI (e.g. `kit://context?profile=minimal`)
    ResourceUri { uri: String, label: String },
    /// A local file path attachment
    FilePath { path: String, label: String },
    /// Synthetic or SDK-authored text context. This travels only inbound; the
    /// preparation receipt records kind/length/fingerprint, never these bytes.
    TextBlock {
        label: String,
        source: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

/// Chat information returned by AI SDK API responses
///
/// Used for `aiListChats()`, `aiGetActiveChat()`, and other chat-related operations.
/// Contains metadata about a chat session without the full message history.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiChatInfo {
    /// Unique chat identifier (UUID)
    pub id: SharedString,
    /// Chat title (auto-generated or user-set)
    pub title: SharedString,
    /// Model ID used for this chat (e.g., "claude-3-5-sonnet-20241022")
    pub model_id: SharedString,
    /// AI provider (e.g., "anthropic", "openai")
    pub provider: SharedString,
    /// When the chat was created (ISO 8601)
    pub created_at: String,
    /// When the chat was last updated (ISO 8601)
    pub updated_at: String,
    /// Whether this chat is soft-deleted
    pub is_deleted: bool,
    /// Preview of the last message (~60 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Number of messages in the chat
    pub message_count: usize,
}

impl AiChatInfo {
    /// Create a new AiChatInfo with required fields
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        model_id: impl Into<SharedString>,
        provider: impl Into<SharedString>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        AiChatInfo {
            id: id.into(),
            title: title.into(),
            model_id: model_id.into(),
            provider: provider.into(),
            created_at: now.clone(),
            updated_at: now,
            is_deleted: false,
            preview: None,
            message_count: 0,
        }
    }

    /// Set the preview text
    pub fn with_preview(mut self, preview: String) -> Self {
        self.preview = Some(preview);
        self
    }

    /// Set the message count
    pub fn with_message_count(mut self, count: usize) -> Self {
        self.message_count = count;
        self
    }

    /// Set timestamps
    pub fn with_timestamps(mut self, created_at: String, updated_at: String) -> Self {
        self.created_at = created_at;
        self.updated_at = updated_at;
        self
    }

    /// Mark as deleted
    pub fn deleted(mut self) -> Self {
        self.is_deleted = true;
        self
    }
}

/// Message information returned by AI SDK API responses
///
/// Used for `aiGetConversation()` and message-related operations.
/// Contains the full message content and metadata.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiMessageInfo {
    /// Unique message identifier (UUID)
    pub id: SharedString,
    /// Message role: "user", "assistant", or "system"
    pub role: SharedString,
    /// Full message content
    pub content: SharedString,
    /// When the message was created (ISO 8601)
    pub created_at: String,
    /// Number of tokens used (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u32>,
}

impl AiMessageInfo {
    /// Create a new AiMessageInfo
    pub fn new(
        id: impl Into<SharedString>,
        role: impl Into<SharedString>,
        content: impl Into<SharedString>,
    ) -> Self {
        AiMessageInfo {
            id: id.into(),
            role: role.into(),
            content: content.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            tokens_used: None,
        }
    }

    /// Create a user message
    pub fn user(id: impl Into<SharedString>, content: impl Into<SharedString>) -> Self {
        Self::new(id, "user", content)
    }

    /// Create an assistant message
    pub fn assistant(id: impl Into<SharedString>, content: impl Into<SharedString>) -> Self {
        Self::new(id, "assistant", content)
    }

    /// Create a system message
    pub fn system(id: impl Into<SharedString>, content: impl Into<SharedString>) -> Self {
        Self::new(id, "system", content)
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, timestamp: String) -> Self {
        self.created_at = timestamp;
        self
    }

    /// Set tokens used
    pub fn with_tokens(mut self, tokens: u32) -> Self {
        self.tokens_used = Some(tokens);
        self
    }
}
