//! Typed protocol contracts for wait/batch deterministic transaction layer.
//!
//! These types enable AI agents to execute verifiable UI transactions:
//! - `WaitCondition`: poll until a UI state predicate is satisfied
//! - `BatchCommand`: atomic UI actions (set input, select, submit)
//! - `BatchOptions` / `BatchResultEntry`: transaction control and result reporting

use serde::{Deserialize, Serialize};

/// Specification for matching against current UI state.
///
/// All fields are optional; omitted fields are treated as "don't care".
/// A match succeeds when every present field equals the corresponding live value.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateMatchSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_visible: Option<bool>,
}

/// Simple named conditions that agents can wait on.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WaitNamedCondition {
    ChoicesRendered,
    InputEmpty,
    WindowVisible,
    WindowFocused,
}

/// Detailed conditions requiring additional parameters.
///
/// WIRE CONTRACT: the `AgentChat*` variants use explicit
/// `agent_chat*`-prefixed tags (e.g. `agent_chatReady`) because every
/// TypeScript consumer (scripts/devtools, scripts/agentic) and the
/// `agent_chatStateResult` message family use that prefix. Commit
/// 3019cb425 renamed the Rust variants `Acp*` -> `AgentChat*`, which
/// silently changed the camelCase tags to `agentChatReady` and broke
/// every live driver waitFor on these conditions; the explicit renames
/// restore the consumer-expected shape (camelCase kept as an alias).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum WaitDetailedCondition {
    ElementExists {
        #[serde(rename = "semanticId", alias = "semantic_id")]
        semantic_id: String,
    },
    ElementVisible {
        #[serde(rename = "semanticId", alias = "semantic_id")]
        semantic_id: String,
    },
    ElementFocused {
        #[serde(rename = "semanticId", alias = "semantic_id")]
        semantic_id: String,
    },
    StateMatch {
        state: StateMatchSpec,
    },
    /// Agent Chat-specific: wait until the Agent Chat view is ready (context bootstrapped, idle).
    #[serde(rename = "agent_chatReady", alias = "agentChatReady")]
    AgentChatReady,
    /// Agent Chat-specific: wait until the mention/slash picker is open.
    #[serde(rename = "agent_chatPickerOpen", alias = "agentChatPickerOpen")]
    AgentChatPickerOpen,
    /// Agent Chat-specific: wait until the mention/slash picker is closed.
    #[serde(rename = "agent_chatPickerClosed", alias = "agentChatPickerClosed")]
    AgentChatPickerClosed,
    /// Agent Chat-specific: wait until a picker item has been accepted.
    #[serde(rename = "agent_chatItemAccepted", alias = "agentChatItemAccepted")]
    AgentChatItemAccepted,
    /// Agent Chat-specific: wait until the cursor reaches a specific character index.
    #[serde(rename = "agent_chatCursorAt", alias = "agentChatCursorAt")]
    AgentChatCursorAt {
        index: usize,
    },
    /// Agent Chat-specific: wait until the Agent Chat thread reaches a specific status.
    #[serde(rename = "agent_chatStatus", alias = "agentChatStatus")]
    AgentChatStatus {
        status: String,
    },
    /// Agent Chat-specific: wait until the Agent Chat input text matches exactly.
    #[serde(rename = "agent_chatInputMatch", alias = "agentChatInputMatch")]
    AgentChatInputMatch {
        text: String,
    },
    /// Agent Chat-specific: wait until the Agent Chat input text contains a substring.
    #[serde(rename = "agent_chatInputContains", alias = "agentChatInputContains")]
    AgentChatInputContains {
        substring: String,
    },
    /// Agent Chat proof: wait until a picker item was accepted via a specific key.
    #[serde(rename = "agent_chatAcceptedViaKey", alias = "agentChatAcceptedViaKey")]
    AgentChatAcceptedViaKey {
        /// The key that must have caused acceptance: `"enter"` or `"tab"`.
        key: String,
    },
    /// Agent Chat proof: wait until a picker item with a specific label was accepted.
    #[serde(rename = "agent_chatAcceptedLabel", alias = "agentChatAcceptedLabel")]
    AgentChatAcceptedLabel {
        /// The label of the accepted item.
        label: String,
    },
    /// Agent Chat proof: wait until the cursor reaches a specific index after acceptance.
    #[serde(
        rename = "agent_chatAcceptedCursorAt",
        alias = "agentChatAcceptedCursorAt"
    )]
    AgentChatAcceptedCursorAt {
        /// Target cursor index after the accepted text was inserted.
        index: usize,
    },
    /// Agent Chat proof: wait until the input layout matches specific visibility metrics.
    #[serde(
        rename = "agent_chatInputLayoutMatch",
        alias = "agentChatInputLayoutMatch"
    )]
    AgentChatInputLayoutMatch {
        /// Visible window start (character index).
        #[serde(rename = "visibleStart")]
        visible_start: usize,
        /// Visible window end (character index, exclusive).
        #[serde(rename = "visibleEnd")]
        visible_end: usize,
        /// Cursor position within the visible window (0-based from visible_start).
        #[serde(rename = "cursorInWindow")]
        cursor_in_window: usize,
    },
    /// Agent Chat setup: wait until the setup card is visible (status == "setup" with setup payload).
    #[serde(rename = "agent_chatSetupVisible", alias = "agentChatSetupVisible")]
    AgentChatSetupVisible,
    /// Agent Chat setup: wait until the setup reason code matches.
    #[serde(
        rename = "agent_chatSetupReasonCode",
        alias = "agentChatSetupReasonCode"
    )]
    AgentChatSetupReasonCode {
        #[serde(rename = "reasonCode")]
        reason_code: String,
    },
    /// Agent Chat setup: wait until the setup primary action matches.
    #[serde(
        rename = "agent_chatSetupPrimaryAction",
        alias = "agentChatSetupPrimaryAction"
    )]
    AgentChatSetupPrimaryAction {
        action: crate::protocol::types::agent_chat_state::AgentChatSetupActionKind,
    },
    /// Agent Chat setup: wait until the setup agent picker overlay is open.
    #[serde(
        rename = "agent_chatSetupAgentPickerOpen",
        alias = "agentChatSetupAgentPickerOpen"
    )]
    AgentChatSetupAgentPickerOpen,
    /// Agent Chat setup: wait until the setup selected agent matches.
    #[serde(
        rename = "agent_chatSetupSelectedAgent",
        alias = "agentChatSetupSelectedAgent"
    )]
    AgentChatSetupSelectedAgent {
        #[serde(rename = "agentId")]
        agent_id: String,
    },
}

/// Union of named and detailed wait conditions.
///
/// Uses `#[serde(untagged)]` because the two variants are structurally
/// distinct: `WaitNamedCondition` is a bare string, while
/// `WaitDetailedCondition` is always an object with a `type` field.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum WaitCondition {
    Named(WaitNamedCondition),
    Detailed(WaitDetailedCondition),
}

/// Atomic UI commands that can be executed individually or inside a batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum BatchCommand {
    SetInput {
        text: String,
    },
    OpenActions,
    TogglePreview,
    ForceSubmit {
        value: serde_json::Value,
    },
    WaitFor {
        condition: WaitCondition,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout: Option<u64>,
        #[serde(
            rename = "pollInterval",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        poll_interval: Option<u64>,
    },
    SelectByValue {
        value: String,
        #[serde(default)]
        submit: bool,
    },
    SelectBySemanticId {
        #[serde(rename = "semanticId")]
        semantic_id: String,
        #[serde(default)]
        submit: bool,
    },
    SetThemeControl {
        control: String,
        value: String,
    },
    UndoStyleChange,
    RedoStyleChange,
    ResetStyleControls,
    SaveCurrentStyleSettings,
    FilterAndSelect {
        filter: String,
        #[serde(rename = "selectFirst", default)]
        select_first: bool,
        #[serde(default)]
        submit: bool,
    },
    TypeAndSubmit {
        text: String,
    },
}

/// Options controlling batch execution behavior.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchOptions {
    /// Stop executing commands after the first failure (default: true).
    #[serde(default = "default_stop_on_error")]
    pub stop_on_error: bool,
    /// Reserved for future use: rollback side effects on error.
    #[serde(default)]
    pub rollback_on_error: bool,
    /// Overall batch timeout in milliseconds (default: 5000).
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

/// Trace mode for transaction requests.
///
/// Controls whether trace receipts are included in results:
/// - `off`: no trace (default)
/// - `on`: always include trace
/// - `onFailure`: include trace only when the transaction fails
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum TransactionTraceMode {
    #[default]
    Off,
    On,
    OnFailure,
}

/// Machine-readable error code for transaction failures.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionErrorCode {
    WaitConditionTimeout,
    ElementNotFound,
    SelectionNotFound,
    InvalidCondition,
    RequestIdConflict,
    UnsupportedCommand,
    UnsupportedPrompt,
    ActionFailed,
}

/// Structured error with machine-readable code and actionable suggestion.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionError {
    pub code: TransactionErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TransactionError {}

impl TransactionError {
    /// Create an action-failed error from a message string.
    pub fn action_failed(message: impl Into<String>) -> Self {
        Self {
            code: TransactionErrorCode::ActionFailed,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create a wait-condition-timeout error from a message string.
    pub fn wait_timeout(message: impl Into<String>) -> Self {
        Self {
            code: TransactionErrorCode::WaitConditionTimeout,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create a selection-not-found error from a message string.
    pub fn selection_not_found(message: impl Into<String>) -> Self {
        Self {
            code: TransactionErrorCode::SelectionNotFound,
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create an element-not-found error from a semantic ID.
    pub fn element_not_found(semantic_id: &str) -> Self {
        Self {
            code: TransactionErrorCode::ElementNotFound,
            message: format!("Element not found: {semantic_id}"),
            suggestion: Some(
                "Run getElements() to discover visible semantic IDs, \
                 or waitFor elementExists before targeting."
                    .to_string(),
            ),
        }
    }

    /// Create an unsupported-prompt error from a message string.
    pub fn unsupported_prompt(message: impl Into<String>) -> Self {
        Self {
            code: TransactionErrorCode::UnsupportedPrompt,
            message: message.into(),
            suggestion: Some(
                "Check getState().promptType to verify the current prompt \
                 supports this operation."
                    .to_string(),
            ),
        }
    }
}

/// Snapshot of UI state at a point in time during a transaction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiStateSnapshot {
    pub window_visible: bool,
    pub window_focused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_semantic_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visible_semantic_ids: Vec<String>,
    #[serde(default)]
    pub choice_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_chat_status: Option<String>,
    #[serde(default)]
    pub agent_chat_context_ready: bool,
    #[serde(default)]
    pub agent_chat_picker_open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_chat_cursor_index: Option<usize>,
}

/// A single poll observation during a waitFor command.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WaitPollObservation {
    pub attempt: usize,
    pub elapsed_ms: u64,
    pub condition_satisfied: bool,
    pub snapshot: UiStateSnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_semantic_ids: Vec<String>,
}

/// Status of a completed transaction trace.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionTraceStatus {
    Ok,
    Failed,
    Timeout,
}

/// Trace data for a single command within a transaction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionCommandTrace {
    pub index: usize,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_payload: Option<BatchCommand>,
    pub started_at_ms: u64,
    pub elapsed_ms: u64,
    pub before: UiStateSnapshot,
    pub after: UiStateSnapshot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polls: Vec<WaitPollObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TransactionError>,
}

/// Full transaction trace receipt, optionally embedded in results.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTrace {
    #[serde(default = "default_transaction_trace_schema_version")]
    pub schema_version: u32,
    pub request_id: String,
    #[serde(default)]
    pub command_fingerprint: String,
    pub status: TransactionTraceStatus,
    pub started_at_ms: u64,
    pub total_elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<TransactionCommandTrace>,
}

pub const TRANSACTION_TRACE_SCHEMA_VERSION: u32 = 1;

fn default_transaction_trace_schema_version() -> u32 {
    TRANSACTION_TRACE_SCHEMA_VERSION
}

/// Result entry for a single command within a batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchResultEntry {
    /// Zero-based index of this command in the batch.
    pub index: usize,
    /// Whether this command succeeded.
    pub success: bool,
    /// The command type name (e.g., "setInput", "waitFor").
    pub command: String,
    /// Wall-clock time this command took, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed: Option<u64>,
    /// The value produced by this command, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Structured error if the command failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TransactionError>,
}

/// Helper for serde skip_serializing_if — returns true when trace mode is Off (default).
pub fn is_trace_off(mode: &TransactionTraceMode) -> bool {
    *mode == TransactionTraceMode::Off
}

fn default_stop_on_error() -> bool {
    true
}

fn default_timeout() -> u64 {
    5_000
}
