use serde::{Deserialize, Serialize};

use super::ProtocolAction;

// ============================================================
// CHAT PROMPT TYPES (AI SDK Compatible)
// ============================================================

/// Role of a chat message - compatible with AI SDK's CoreMessage
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageRole {
    /// System prompt/instructions
    System,
    /// User message
    User,
    /// Assistant response
    #[default]
    Assistant,
    /// Tool/function call result
    Tool,
}

impl ChatMessageRole {
    /// Convert role to display position (left/right alignment)
    pub fn to_position(&self) -> ChatMessagePosition {
        match self {
            ChatMessageRole::User => ChatMessagePosition::Right,
            _ => ChatMessagePosition::Left,
        }
    }
}

/// A chat message displayed in the ChatPrompt
///
/// Supports both AI SDK format (role/content) and Script Kit format (position/text).
/// When using AI SDK format, role/content take precedence.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatPromptMessage {
    /// Unique message identifier (UUID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    // === AI SDK Compatible Fields ===
    /// Message role (AI SDK format) - takes precedence over position if set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<ChatMessageRole>,
    /// Message content (AI SDK format) - alias for text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    // === Script Kit Fields (backwards compatible) ===
    /// Message text content (supports markdown) - use content for AI SDK compat
    #[serde(default)]
    pub text: String,
    /// Position in the chat: "left" (assistant/other) or "right" (user)
    #[serde(default)]
    pub position: ChatMessagePosition,

    // === Metadata ===
    /// Optional name/label for the message sender
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Model that generated this message (assistant only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Whether this message is currently streaming (partial content)
    #[serde(default)]
    pub streaming: bool,
    /// Error message if generation failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Typed terminal failure. `error` remains a temporary SDK compatibility
    /// projection and must contain only safe primary copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<sk_protocol::ai_reliability::AiFailure>,
    /// Creation timestamp (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Optional image attachment (base64 data URI or file path)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

impl ChatPromptMessage {
    /// Create a new user message (AI SDK compatible)
    pub fn user(content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            role: Some(ChatMessageRole::User),
            content: Some(content_str.clone()),
            text: content_str,
            position: ChatMessagePosition::Right,
            name: None,
            model: None,
            streaming: false,
            error: None,
            failure: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            image: None,
        }
    }

    /// Create a new assistant message (AI SDK compatible)
    pub fn assistant(content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            role: Some(ChatMessageRole::Assistant),
            content: Some(content_str.clone()),
            text: content_str,
            position: ChatMessagePosition::Left,
            name: None,
            model: None,
            streaming: false,
            error: None,
            failure: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            image: None,
        }
    }

    /// Create a system message (AI SDK compatible)
    pub fn system(content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            role: Some(ChatMessageRole::System),
            content: Some(content_str.clone()),
            text: content_str,
            position: ChatMessagePosition::Left,
            name: Some("System".to_string()),
            model: None,
            streaming: false,
            error: None,
            failure: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            image: None,
        }
    }

    /// Create a streaming assistant message (content still arriving)
    pub fn streaming(content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            role: Some(ChatMessageRole::Assistant),
            content: Some(content_str.clone()),
            text: content_str,
            position: ChatMessagePosition::Left,
            name: None,
            model: None,
            streaming: true,
            error: None,
            failure: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            image: None,
        }
    }

    /// Create an error message
    pub fn error(error_msg: impl Into<String>) -> Self {
        let error_str = error_msg.into();
        Self {
            id: Some(uuid::Uuid::new_v4().to_string()),
            role: Some(ChatMessageRole::Assistant),
            content: None,
            text: String::new(),
            position: ChatMessagePosition::Left,
            name: None,
            model: None,
            streaming: false,
            error: Some(error_str),
            failure: None,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            image: None,
        }
    }

    /// Get the effective content (prefers content field, falls back to text)
    pub fn get_content(&self) -> &str {
        self.content.as_deref().unwrap_or(&self.text)
    }

    /// Get the effective position (prefers role-based, falls back to position)
    pub fn get_position(&self) -> ChatMessagePosition {
        self.role
            .as_ref()
            .map(|r| r.to_position())
            .unwrap_or_else(|| self.position.clone())
    }

    /// Check if this is a user message
    pub fn is_user(&self) -> bool {
        matches!(self.role, Some(ChatMessageRole::User))
            || self.position == ChatMessagePosition::Right
    }

    /// Set the message ID
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the sender name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Mark as streaming
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Append content (for streaming)
    pub fn append_content(&mut self, chunk: &str) {
        self.text.push_str(chunk);
        if let Some(ref mut content) = self.content {
            content.push_str(chunk);
        } else {
            self.content = Some(self.text.clone());
        }
    }

    /// Set content (replaces existing content)
    pub fn set_content(&mut self, new_content: &str) {
        self.text = new_content.to_string();
        self.content = Some(new_content.to_string());
    }

    /// Convert to AI SDK CoreMessage format
    pub fn to_core_message(&self) -> serde_json::Value {
        serde_json::json!({
            "role": self.role.as_ref().map(|r| match r {
                ChatMessageRole::System => "system",
                ChatMessageRole::User => "user",
                ChatMessageRole::Assistant => "assistant",
                ChatMessageRole::Tool => "tool",
            }).unwrap_or(if self.is_user() { "user" } else { "assistant" }),
            "content": self.get_content()
        })
    }
}

/// Position of a chat message (alignment)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessagePosition {
    /// Left-aligned (assistant/other messages)
    #[default]
    Left,
    /// Right-aligned (user messages)
    Right,
}

/// Configuration options for the chat prompt
///
/// Used with the Chat message to configure the chat interface.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatPromptConfig {
    /// Placeholder text for the input field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Initial messages to display
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<ChatPromptMessage>,
    /// Whether to focus the input on open
    #[serde(default)]
    pub auto_focus: bool,
    /// Hint text (shown in header)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Footer text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    /// Optional actions for the actions panel (Cmd+K)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ProtocolAction>,
    /// Default model to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Available models in actions menu
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    /// Save conversation to database (default: true)
    #[serde(default = "default_save_history")]
    pub save_history: bool,
    /// Use built-in AI mode (app handles AI calls instead of SDK)
    #[serde(default)]
    pub use_builtin_ai: bool,
}

/// Default value for save_history (true)
fn default_save_history() -> bool {
    true
}

impl Default for ChatPromptConfig {
    fn default() -> Self {
        Self {
            placeholder: None,
            messages: Vec::new(),
            auto_focus: false,
            hint: None,
            footer: None,
            actions: Vec::new(),
            model: None,
            models: Vec::new(),
            save_history: true,
            use_builtin_ai: false,
        }
    }
}

impl ChatPromptConfig {
    /// Create a new ChatPromptConfig with a placeholder
    pub fn with_placeholder(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: Some(placeholder.into()),
            auto_focus: true,
            ..Default::default()
        }
    }

    /// Add an initial message
    pub fn add_message(mut self, msg: ChatPromptMessage) -> Self {
        self.messages.push(msg);
        self
    }

    /// Set hint text
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Set footer text
    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// Add actions for the actions panel
    pub fn with_actions(mut self, actions: Vec<ProtocolAction>) -> Self {
        self.actions = actions;
        self
    }

    /// Set the default model
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set available models
    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    /// Set whether to save conversation to database
    pub fn with_save_history(mut self, save: bool) -> Self {
        self.save_history = save;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatMessagePosition, ChatMessageRole, ChatPromptMessage};

    #[test]
    fn test_append_content_preserves_existing_text_when_content_is_none() {
        let mut message = ChatPromptMessage {
            id: None,
            role: Some(ChatMessageRole::Assistant),
            content: None,
            text: "existing".to_string(),
            position: ChatMessagePosition::Left,
            name: None,
            model: None,
            streaming: true,
            error: None,
            failure: None,
            created_at: None,
            image: None,
        };

        message.append_content(" chunk");

        assert_eq!(message.text, "existing chunk");
        assert_eq!(message.content.as_deref(), Some("existing chunk"));
    }
}
