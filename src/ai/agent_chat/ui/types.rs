use crate::ai::agent_chat::content::{ContentBlock, TextContent};
use crate::ai::context_selector::types::{ContextSelectorRow, ContextSelectorTrigger};

/// Maps a Script Kit UI session to the Agent Chat session ID returned by `session/new`.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatSessionBinding {
    /// Script Kit's internal chat identifier (from `Chat.id`).
    pub ui_session_id: String,
    /// Agent Chat session ID returned by the agent in `NewSessionResponse`.
    pub agent_session_id: String,
}

/// Convert `ProviderMessage` list into Agent Chat `ContentBlock` list.
///
/// Only emits text blocks in this cycle. Images/audio fail closed — they are
/// logged and skipped rather than being silently passed through.
pub(crate) fn build_prompt_blocks(
    messages: &[crate::ai::providers::ProviderMessage],
) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();
    for msg in messages {
        if msg.role == "system" {
            // Agent Chat agents handle system prompts via session config, not inline.
            // Emit as a labelled text block so the agent at least sees the content.
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "[system]\n{}",
                msg.content
            ))));
            continue;
        }
        if msg.has_images() {
            tracing::warn!(
                role = %msg.role,
                image_count = msg.images.len(),
                "agent_chat_prompt_images_skipped: images not supported in this Agent Chat cycle"
            );
        }
        if !msg.content.is_empty() {
            blocks.push(ContentBlock::Text(TextContent::new(&msg.content)));
        }
    }
    blocks
}

// ── Extracted Agent Chat view types ────────────────────────────────────────────

/// Agent Chat's composer popup supports command/profile triggers only.
///
/// `@` context is owned by the shared Spine/main-menu path, so it is
/// intentionally not representable as an Agent Chat popup session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatComposerPickerTrigger {
    Slash,
    Profile,
}

impl AgentChatComposerPickerTrigger {
    pub(crate) fn from_context_selector(trigger: ContextSelectorTrigger) -> Option<Self> {
        match trigger {
            ContextSelectorTrigger::Slash => Some(Self::Slash),
            ContextSelectorTrigger::Profile => Some(Self::Profile),
            ContextSelectorTrigger::Mention => None,
        }
    }

    pub(crate) fn as_context_selector(self) -> ContextSelectorTrigger {
        match self {
            Self::Slash => ContextSelectorTrigger::Slash,
            Self::Profile => ContextSelectorTrigger::Profile,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Slash => "/",
            Self::Profile => crate::ai::context_selector::types::PROFILE_TRIGGER_STR,
        }
    }
}

/// Active slash/profile composer picker state for Agent Chat.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatComposerPickerSession {
    /// Which trigger character opened this session (`/` or `|`).
    pub(crate) trigger: AgentChatComposerPickerTrigger,
    /// Character range of the trigger+query in the input text.
    pub(crate) trigger_range: std::ops::Range<usize>,
    /// Query text typed after the trigger.
    pub(crate) query: String,
    /// Currently highlighted row index.
    pub(crate) selected_index: usize,
    /// First visible row in the popup list.
    pub(crate) visible_start: usize,
    /// Ranked picker items for the current query.
    pub(crate) items: Vec<ContextSelectorRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatDismissedComposerPickerTrigger {
    pub(crate) trigger: AgentChatComposerPickerTrigger,
    pub(crate) trigger_range: std::ops::Range<usize>,
    pub(crate) query: String,
    pub(crate) cursor: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AgentChatComposerParentWindow {
    pub(crate) handle: gpui::AnyWindowHandle,
    pub(crate) bounds: gpui::Bounds<gpui::Pixels>,
    pub(crate) display_id: Option<gpui::DisplayId>,
    pub(crate) display_bounds: Option<gpui::Bounds<gpui::Pixels>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentChatFocusedMentionPreview {
    pub(crate) token: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentChatPendingPortalSession {
    pub(crate) contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
    pub(crate) composer_text: String,
    pub(crate) composer_cursor: usize,
    pub(crate) pending_context_items: Vec<crate::ai::staged_context::StagedContextItem>,
    pub(crate) pending_context_consumed: bool,
    pub(crate) state: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::providers::ProviderMessage;

    #[test]
    fn build_prompt_blocks_text_only() {
        let messages = vec![
            ProviderMessage::user("Hello"),
            ProviderMessage::assistant("Hi there"),
        ];
        let blocks = build_prompt_blocks(&messages);
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::Text(t) => assert_eq!(t.text, "Hello"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn build_prompt_blocks_system_prefix() {
        let messages = vec![ProviderMessage::system("You are helpful")];
        let blocks = build_prompt_blocks(&messages);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text(t) => {
                assert!(t.text.starts_with("[system]"));
                assert!(t.text.contains("You are helpful"));
            }
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn build_prompt_blocks_empty_content_skipped() {
        let messages = vec![ProviderMessage::user("")];
        let blocks = build_prompt_blocks(&messages);
        assert!(blocks.is_empty());
    }
}
