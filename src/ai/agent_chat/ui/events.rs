//! Typed Agent Chat turn/event primitives.
//!
//! These replace the one-shot `StreamCallback` model with a structured
//! event stream that the `AgentChatThread` entity can consume for durable
//! per-thread history, pending tool state, and permission UX.

/// Channel types for Agent Chat event streaming.
pub(crate) type AgentChatEventTx = async_channel::Sender<AgentChatEvent>;
pub(crate) type AgentChatEventRx = async_channel::Receiver<AgentChatEvent>;

/// A user message the live session can rewind to (Pi fork checkpoint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatForkPoint {
    /// Pi session entry id for the user message.
    pub entry_id: String,
    /// The user message text (used for switcher labels and composer prefill).
    pub text: String,
}

/// Typed events emitted by the Agent Chat worker for a single turn.
///
/// Each variant maps to a specific `SessionUpdate` notification from
/// the Agent Chat protocol, plus lifecycle events (`TurnFinished`, `Failed`).
#[derive(Debug, Clone)]
pub(crate) enum AgentChatEvent {
    /// A chunk of user message text echoed back by the agent.
    UserMessageDelta(String),
    /// A chunk of assistant message text.
    AgentMessageDelta(String),
    /// A chunk of agent internal reasoning / thought.
    AgentThoughtDelta(String),
    /// A new tool call has started.
    ToolCallStarted {
        tool_call_id: String,
        title: String,
        status: String,
        /// Raw Pi tool name (e.g. "bash", "edit") for kind derivation.
        tool_name: Option<String>,
        /// Raw tool input args, used to derive the card subject line.
        raw_input: Option<serde_json::Value>,
    },
    /// An existing tool call has been updated.
    ToolCallUpdated {
        tool_call_id: String,
        title: Option<String>,
        status: Option<String>,
        body: Option<String>,
        /// Raw tool input args (present on Pi execution updates; backfills
        /// orphan tool calls whose start event was missed).
        raw_input: Option<serde_json::Value>,
        /// Pre-rendered diff from `result.details.diff` for edit/write tools.
        diff: Option<String>,
        /// Whether the tool reported an error result.
        is_error: bool,
    },
    /// The agent's plan has been updated.
    PlanUpdated { entries: Vec<String> },
    /// Available commands have changed.
    AvailableCommandsUpdated { command_names: Vec<String> },
    /// The agent's mode has changed.
    ModeChanged { mode_id: String },
    /// Session usage metrics updated.
    UsageUpdated {
        used_tokens: u64,
        context_size: u64,
        cost_usd: Option<f64>,
    },
    /// The agent advertised its available models (and optional current model)
    /// on `session/new`. Emitted once per session so the UI can replace the
    /// hardcoded fallback model list with the agent's live list.
    ModelsAvailable {
        current_model_id: Option<String>,
        models: Vec<super::config::AgentChatModelEntry>,
    },
    /// The session advertised the user messages it can rewind to.
    ForkPointsAvailable { entries: Vec<AgentChatForkPoint> },
    /// The session was rewound to just before a user message; `text` is that
    /// message's original text, returned so the composer can prefill it for
    /// editing.
    ForkCompleted { text: String },
    /// The turn reached a typed non-failure terminal outcome.
    TurnCompleted {
        outcome: crate::ai::reliability::AiTurnRuntimeOutcome,
    },
    /// Agent requires setup (authentication, install, etc.).
    SetupRequired {
        reason: String,
        auth_methods: Vec<String>,
    },
    /// The turn failed with a classified, redacted record.
    TurnFailed {
        failure: crate::ai::reliability::AppFailureRecord,
    },
}

impl AgentChatEvent {
    pub(crate) fn completed(stop_reason: impl Into<String>) -> Self {
        Self::TurnCompleted {
            outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Completed {
                stop_reason: Some(stop_reason.into()),
            },
        }
    }

    pub(crate) fn failed(
        component: sk_protocol::ai_reliability::ProtocolComponent,
        raw: impl AsRef<str>,
    ) -> Self {
        Self::TurnFailed {
            failure: crate::ai::reliability::provider_failure(component, raw),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_chat_event_variants_are_clone() {
        let event = AgentChatEvent::AgentMessageDelta("hello".to_string());
        let _cloned = event.clone();
    }
}
