//! Agent Chat style contract — **compatibility façade only**.
//!
//! The production values that used to live here now live in
//! [`crate::components::conversation_style`], because Flow's `ChatPrompt`
//! needs to read the same numbers and could not reach a module namespaced
//! under `src/ai/agent_chat/ui/`. See that module's header for the contract.
//!
//! This file deliberately contains **no production values**. It exists so the
//! existing `AgentChat*` type names, constant names, and
//! `production_agent_chat_style()` keep resolving for the renderer, the
//! `design_contract` exporter, and their checked-in artifacts. Adding a value
//! here instead of in the shared owner would re-create exactly the drift this
//! promotion removed.
//!
//! When every caller has migrated to the `Conversation*` names, delete this
//! file rather than letting it accumulate logic.

// ── Type aliases (old Agent Chat names → shared conversation types) ───────

pub use crate::components::conversation_style::{
    ConversationCollapsibleStyle as AgentChatCollapsibleStyle,
    ConversationErrorStyle as AgentChatErrorStyle,
    ConversationMarkdownStyle as AgentChatMarkdownStyle,
    ConversationMessageStyle as AgentChatMessageStyle, ConversationStyleDef as AgentChatStyleDef,
    ConversationSystemStyle as AgentChatSystemStyle,
    ConversationTranscriptStyle as AgentChatTranscriptStyle,
};

// ── Resolver + packing re-exports ─────────────────────────────────────────

pub(crate) use crate::components::conversation_style::{
    pack_rgb_alpha,
    resolved_conversation_composer_single_line_height as resolved_agent_chat_composer_single_line_height,
    resolved_conversation_markdown_body_line_height as resolved_agent_chat_markdown_body_line_height,
    resolved_conversation_send_state_chrome as resolved_agent_chat_send_state_chrome,
    resolved_conversation_transcript_colors as resolved_agent_chat_transcript_colors,
    ConversationSendStateChrome as AgentChatSendStateChrome,
    ResolvedConversationTranscriptColors as ResolvedAgentChatTranscriptColors,
};

// ── Constant aliases ──────────────────────────────────────────────────────
//
// These are `use` re-exports, NOT copied numeric literals. A copied number
// here would silently fork from the shared owner the first time either side
// changed — the exact failure mode this promotion exists to prevent.

pub(crate) use crate::components::conversation_style::{
    CONVERSATION_ACTIVITY_DOT_SIZE as AGENT_CHAT_ACTIVITY_DOT_SIZE,
    CONVERSATION_ACTIVITY_GAP as AGENT_CHAT_ACTIVITY_GAP,
    CONVERSATION_ACTIVITY_LABEL_ALPHA as AGENT_CHAT_ACTIVITY_LABEL_ALPHA,
    CONVERSATION_BLOCK_BORDER_WIDTH as AGENT_CHAT_BLOCK_BORDER_WIDTH,
    CONVERSATION_BLOCK_HEADER_GAP as AGENT_CHAT_BLOCK_HEADER_GAP,
    CONVERSATION_DIFF_CONTEXT_OPACITY as AGENT_CHAT_DIFF_CONTEXT_OPACITY,
    CONVERSATION_DIFF_TINT_ALPHA as AGENT_CHAT_DIFF_TINT_ALPHA,
    CONVERSATION_INPUT_FONT_FAMILY as AGENT_CHAT_INPUT_FONT_FAMILY,
    CONVERSATION_INPUT_FONT_SIZE as AGENT_CHAT_INPUT_FONT_SIZE,
    CONVERSATION_INPUT_LINE_HEIGHT as AGENT_CHAT_INPUT_LINE_HEIGHT,
    CONVERSATION_INPUT_PADDING_X as AGENT_CHAT_INPUT_PADDING_X,
    CONVERSATION_INPUT_PADDING_Y as AGENT_CHAT_INPUT_PADDING_Y,
    CONVERSATION_PLACEHOLDER_ASK as AGENT_CHAT_PLACEHOLDER_ASK,
    CONVERSATION_PLACEHOLDER_FOLLOW_UP as AGENT_CHAT_PLACEHOLDER_FOLLOW_UP,
    CONVERSATION_SEND_DISABLED_BG_ALPHA as AGENT_CHAT_SEND_DISABLED_BG_ALPHA,
    CONVERSATION_SEND_DISABLED_OPACITY as AGENT_CHAT_SEND_DISABLED_OPACITY,
    CONVERSATION_SEND_ENABLED_BG_ALPHA as AGENT_CHAT_SEND_ENABLED_BG_ALPHA,
    CONVERSATION_SEND_ENABLED_OPACITY as AGENT_CHAT_SEND_ENABLED_OPACITY,
    CONVERSATION_SEND_QUEUE_BG_ALPHA as AGENT_CHAT_SEND_QUEUE_BG_ALPHA,
    CONVERSATION_SEND_QUEUE_OPACITY as AGENT_CHAT_SEND_QUEUE_OPACITY,
    CONVERSATION_SEND_RADIUS as AGENT_CHAT_SEND_RADIUS,
    CONVERSATION_SEND_SIZE as AGENT_CHAT_SEND_SIZE,
    CONVERSATION_SEND_STREAMING_OPACITY as AGENT_CHAT_SEND_STREAMING_OPACITY,
    CONVERSATION_TOOL_STATUS_PENDING_ALPHA as AGENT_CHAT_TOOL_STATUS_PENDING_ALPHA,
};

// ── Forwarding production style ───────────────────────────────────────────

/// Forwards to the shared owner. Kept so the `design_contract` exporter and
/// its checked-in artifacts keep their existing entry point while the shared
/// module owns the numbers.
pub fn production_agent_chat_style() -> AgentChatStyleDef {
    crate::components::conversation_style::production_conversation_style()
}

#[cfg(test)]
mod agent_chat_style_facade_tests {
    use super::*;

    /// The façade must stay a façade. If production values were ever pasted
    /// back into this module, this equality would still pass — so the real
    /// guard is the shared module's own value test plus this identity check
    /// proving the forwarding call is wired to the shared owner.
    #[test]
    fn production_agent_chat_style_is_a_compatibility_alias() {
        assert_eq!(
            production_agent_chat_style(),
            crate::components::conversation_style::production_conversation_style()
        );
    }

    /// The aliases must name the SAME types, not structurally-identical
    /// copies. Assigning across the alias boundary only compiles if they are
    /// one type.
    #[test]
    fn agent_chat_type_names_alias_the_shared_conversation_types() {
        let shared = crate::components::conversation_style::production_conversation_style();
        let via_alias: AgentChatStyleDef = shared;
        assert_eq!(via_alias.markdown.body_font_size, 14.0);

        let markdown: AgentChatMarkdownStyle = shared.markdown;
        assert_eq!(markdown.code_block_font_size, 13.0);
    }
}
