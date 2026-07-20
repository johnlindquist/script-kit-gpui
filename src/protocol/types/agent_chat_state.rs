//! Machine-readable Agent Chat state types for agentic testing.
//!
//! Provides structured snapshots of Agent Chat chat view state — input, cursor,
//! picker, accepted items, thread status, and layout stability metrics —
//! so that autonomous agents can verify Agent Chat interactions without screenshots.

use serde::{Deserialize, Serialize};

/// Schema version for the Agent Chat state response envelope.
pub const AGENT_CHAT_STATE_SCHEMA_VERSION: u32 = 5;

/// Resolved automation target echoed back in Agent Chat state/probe responses.
///
/// Agents use this to confirm that the response came from the intended
/// Agent Chat surface (main vs detached), preventing cross-window false proof.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatResolvedTarget {
    /// Stable automation window ID (e.g. `"main"`, `"agentChatDetached:thread-1"`).
    pub window_id: String,
    /// The kind of window that was resolved.
    pub window_kind: String,
    /// Human-readable window title, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Top-level Agent Chat state snapshot returned by `getAgentChatState`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatStateSnapshot {
    /// Schema version for forward compatibility.
    pub schema_version: u32,

    /// Resolved target metadata, echoed back so agents can confirm
    /// the response came from the intended Agent Chat surface.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_target: Option<AgentChatResolvedTarget>,

    /// Current thread status: "idle", "streaming", "waitingForPermission", "error", "setup".
    pub status: String,

    /// Active Agent Chat chat UI presentation variant.
    #[serde(default)]
    pub ui_variant: String,

    /// Composer input text (redacted to length only when content logging is off).
    pub input_text: String,

    /// Cursor position as a character index (0-based).
    pub cursor_index: usize,

    /// Whether the composer has a text selection (not just a caret).
    pub has_selection: bool,

    /// Selection range as character indices `[start, end)`, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_range: Option<[usize; 2]>,

    /// Number of messages in the thread history.
    pub message_count: usize,

    /// Number of retained background threads in the live thread pool
    /// (threads kept streaming after Cmd+N / thread switching).
    #[serde(default)]
    pub retained_thread_count: usize,

    /// Number of rewindable user messages (Pi fork checkpoints) the
    /// "Rewind & Edit" action can target.
    #[serde(default)]
    pub fork_point_count: usize,

    /// True when a submitted user turn is streaming before assistant text lands.
    #[serde(default)]
    pub awaiting_first_assistant_text: bool,

    /// Picker state (None when picker is closed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picker: Option<AgentChatPickerState>,

    /// Redacted Agent Chat Spine list state when the composer grammar owns rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spine: Option<AgentChatSpineSnapshot>,

    /// The most recently accepted picker item, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accepted_item: Option<AgentChatAcceptedItem>,

    /// Context chip count (staged context parts in the composer).
    pub context_chip_count: usize,

    /// Human-readable summary of staged context chips (comma-joined labels).
    ///
    /// Present when `context_chip_count > 0`. Sharpens the bare count into a
    /// descriptive list (e.g. `"Theme Designer, Current Context"`) so
    /// automation assertions can go from "a chip is present" to "the *right*
    /// chip is present".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_summary: Option<String>,

    /// Active dictation session phase, if any (`recording`, `confirming`,
    /// `transcribing`, `delivering`, `finished`, `failed`, or `idle`).
    ///
    /// `None` when no dictation session is active. Populated from
    /// `DictationSessionPhase::as_automation_str()` so the string vocabulary
    /// stays in lockstep with the runtime enum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dictation_phase: Option<String>,

    /// Whether the context bootstrap is still in progress.
    pub context_ready: bool,

    /// Pending permission request, if any.
    pub has_pending_permission: bool,

    /// Layout stability metrics for the single-line input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_layout: Option<AgentChatInputLayoutMetrics>,

    /// Runtime transcript scroll and scrollbar-thumb metrics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_scroll: Option<AgentChatTranscriptScrollMetrics>,

    /// Runtime multiline composer scroll metrics (growth/clamp/cursor-follow).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer_scroll: Option<AgentChatComposerScrollMetrics>,

    /// Redacted focused-text capture/apply state for the mini Agent Chat mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_text: Option<AgentChatFocusedTextState>,

    /// Structured setup card state, present when `status == "setup"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<AgentChatSetupSnapshot>,

    /// Machine-readable warning codes from the canonical automation vocabulary.
    ///
    /// Present when the request resolved but execution was degraded or rejected
    /// (e.g. `target_unsupported_non_main` when a non-main target was requested).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Default for AgentChatStateSnapshot {
    fn default() -> Self {
        Self {
            schema_version: AGENT_CHAT_STATE_SCHEMA_VERSION,
            resolved_target: None,
            status: "idle".to_string(),
            ui_variant: "standard".to_string(),
            input_text: String::new(),
            cursor_index: 0,
            has_selection: false,
            selection_range: None,
            message_count: 0,
            retained_thread_count: 0,
            fork_point_count: 0,
            awaiting_first_assistant_text: false,
            picker: None,
            spine: None,
            last_accepted_item: None,
            context_chip_count: 0,
            context_summary: None,
            dictation_phase: None,
            context_ready: true,
            has_pending_permission: false,
            input_layout: None,
            transcript_scroll: None,
            composer_scroll: None,
            focused_text: None,
            setup: None,
            warnings: Vec::new(),
        }
    }
}

/// Runtime scroll and scrollbar-thumb metrics for the Agent Chat transcript.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatTranscriptScrollMetrics {
    pub row_count: usize,
    pub scroll_top_item: usize,
    pub scroll_top_offset_px: f32,
    pub viewport_height_px: f32,
    pub content_height_px: f32,
    pub scroll_top_px: f32,
    pub max_scroll_top_px: f32,
    pub can_scroll_y: bool,
    pub thumb_track_height_px: f32,
    pub thumb_height_px: f32,
    pub thumb_top_px: f32,
    pub thumb_bottom_px: f32,
    pub thumb_position_ratio: f32,
    pub measurement_source: String,
    pub follow_tail: bool,
    pub manual_scroll: bool,
    pub pending_indicator_count: usize,
    pub streaming_copy_available: bool,
    pub row_semantic_ids: Vec<String>,
}

/// Runtime scroll metrics for the multiline Agent Chat composer input,
/// read from its GPUI scroll handle after layout.
///
/// Proves the composer growth contract at runtime without screen capture:
/// `viewport_height_px` is the clamped visible text height (≤ 6 lines),
/// `max_scroll_top_px` > 0 means the measured content genuinely exceeds the
/// viewport (wrap-aware growth), and `scroll_top_px` tracks cursor-follow.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatComposerScrollMetrics {
    pub scroll_top_px: f32,
    pub max_scroll_top_px: f32,
    pub viewport_height_px: f32,
    pub can_scroll_y: bool,
}

/// Redacted focused-text mini Agent Chat state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatFocusedTextState {
    pub mode: String,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub footer_visible: bool,
    #[serde(default)]
    pub actions_visible: bool,
    #[serde(default)]
    pub can_expand_to_chat: bool,
    pub session_id: String,
    pub app_name: String,
    pub char_count: usize,
    pub word_count: usize,
    pub context_present: bool,
    #[serde(default)]
    pub context_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_failure_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_fingerprint: Option<String>,
    #[serde(default)]
    pub submitted_prompt_locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_prompt_char_count: Option<usize>,
    #[serde(default)]
    pub input_redacted: bool,
    pub can_replace: bool,
    pub can_append: bool,
    pub can_copy: bool,
    pub has_output: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_apply_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action_receipt: Option<AgentChatFocusedTextActionReceipt>,
}

/// Redacted receipt for focused-text mini Agent Chat actions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatFocusedTextActionReceipt {
    pub action: String,
    pub success: bool,
    pub changed_text: bool,
    pub copied_to_clipboard: bool,
    pub before_ui_variant: String,
    pub after_ui_variant: String,
    pub output_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// State of the inline mention/slash picker overlay.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatPickerState {
    /// Whether the picker is currently visible.
    pub open: bool,

    /// Trigger character that opened the picker: "@" or "/".
    pub trigger: String,

    /// Number of items in the picker.
    pub item_count: usize,

    /// Currently highlighted row index.
    pub selected_index: usize,

    /// Label of the currently highlighted item (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_label: Option<String>,
}

/// Redacted state of the Agent Chat Spine rows that replace the legacy picker list.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatSpineSnapshot {
    pub owns_list: bool,
    pub active_segment_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsearch_source: Option<String>,
    pub row_count: usize,
    pub selectable_row_count: usize,
    pub selected_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_row_fingerprint: Option<String>,
    pub refresh_elapsed_ms: u64,
}

/// Metadata about the most recently accepted picker item.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatAcceptedItem {
    /// The item label that was accepted.
    pub label: String,

    /// The item ID from the picker.
    pub id: String,

    /// Trigger that was used: "@" or "/".
    pub trigger: String,

    /// Cursor index after the accepted text was inserted.
    pub cursor_after: usize,
}

/// Single-line input layout metrics for visual stability verification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatInputLayoutMetrics {
    /// Total character count in the input.
    pub char_count: usize,

    /// Visible window start (character index).
    pub visible_start: usize,

    /// Visible window end (character index, exclusive).
    pub visible_end: usize,

    /// Cursor position within the visible window (0-based from visible_start).
    pub cursor_in_window: usize,
}

/// Kind of action available on the Agent Chat setup card.
///
/// Maps 1:1 to `crate::ai::agent_chat::ui::setup_state::AgentChatSetupAction` but lives in
/// the protocol layer so agents can inspect and trigger actions by name.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AgentChatSetupActionKind {
    Retry,
    Install,
    Authenticate,
    OpenCatalog,
    SelectAgent,
    /// Automation-only: open the agent picker overlay.
    OpenAgentPicker,
    /// Automation-only: close the agent picker overlay.
    CloseAgentPicker,
}

/// Machine-readable snapshot of the Agent Chat setup card state.
///
/// Populated whenever `AgentChatStateSnapshot::status == "setup"`. Agents use this
/// to inspect the setup blocker, available recovery actions, and agent picker
/// state without screenshots or log scraping.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatSetupSnapshot {
    /// Machine-readable reason code for the setup blocker.
    pub reason_code: String,

    /// Human-readable title for the setup card.
    pub title: String,

    /// Human-readable body text explaining the blocker or recovery path.
    pub body: String,

    /// Primary action the user can take to resolve the blocker.
    pub primary_action: AgentChatSetupActionKind,

    /// Optional secondary action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_action: Option<AgentChatSetupActionKind>,

    /// ID of the currently selected agent, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_agent_id: Option<String>,

    /// IDs of all agents in the catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalog_agent_ids: Vec<String>,

    /// IDs of agents that satisfy the current launch requirements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compatible_agent_ids: Vec<String>,

    /// Whether the current launch path requires image/screenshot support.
    pub needs_image: bool,

    /// Whether the current launch path requires embedded context support.
    pub needs_embedded_context: bool,

    /// Whether the agent picker overlay is currently open.
    pub agent_picker_open: bool,

    /// ID of the agent highlighted in the picker, if open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_picker_selected_id: Option<String>,
}

/// Agent Chat-specific wait condition variants.
///
/// These extend the existing `WaitDetailedCondition` enum for Agent Chat views.
///
/// WIRE CONTRACT: explicit `agent_chat*` tags — see the matching note on
/// `WaitDetailedCondition` in batch_wait.rs. These tags must stay in sync
/// with the TypeScript driver consumers (camelCase kept as an alias).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AgentChatWaitCondition {
    /// Wait until the Agent Chat view is ready (context bootstrapped, status idle).
    #[serde(rename = "agent_chatReady", alias = "agentChatReady")]
    AgentChatReady,

    /// Wait until the mention/slash picker is open.
    #[serde(rename = "agent_chatPickerOpen", alias = "agentChatPickerOpen")]
    AgentChatPickerOpen,

    /// Wait until the mention/slash picker is closed.
    #[serde(rename = "agent_chatPickerClosed", alias = "agentChatPickerClosed")]
    AgentChatPickerClosed,

    /// Wait until a picker item has been accepted.
    #[serde(rename = "agent_chatItemAccepted", alias = "agentChatItemAccepted")]
    AgentChatItemAccepted,

    /// Wait until the cursor reaches a specific character index.
    #[serde(rename = "agent_chatCursorAt", alias = "agentChatCursorAt")]
    AgentChatCursorAt {
        /// Target character index.
        index: usize,
    },

    /// Wait until the Agent Chat thread reaches a specific status.
    #[serde(rename = "agent_chatStatus", alias = "agentChatStatus")]
    AgentChatStatus {
        /// Target status: "idle", "streaming", "waitingForPermission", "error".
        status: String,
    },

    /// Wait until the Agent Chat input text matches an exact value.
    #[serde(rename = "agent_chatInputMatch", alias = "agentChatInputMatch")]
    AgentChatInputMatch {
        /// Expected input text.
        text: String,
    },

    /// Wait until the Agent Chat input text contains a substring.
    #[serde(rename = "agent_chatInputContains", alias = "agentChatInputContains")]
    AgentChatInputContains {
        /// Substring that must appear in the input.
        substring: String,
    },
}

/// Structured telemetry event for Agent Chat key routing decisions.
///
/// Emitted via `tracing::info!` on the `script_kit::agent_chat_telemetry` target.
/// Contains no user content — only indices, booleans, and route decisions.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatKeyRouteTelemetry {
    /// The key that was pressed (e.g., "enter", "tab", "up", "@").
    pub key: String,

    /// Where the key was routed to.
    pub route: AgentChatKeyRoute,

    /// Whether the picker was open at the time of the key press.
    pub picker_open: bool,

    /// Whether an Agent Chat permission approval surface was active.
    pub permission_active: bool,

    /// Cursor index before the key was processed.
    pub cursor_before: usize,

    /// Cursor index after the key was processed.
    pub cursor_after: usize,

    /// Whether the key caused a submission.
    pub caused_submit: bool,

    /// Whether the key was consumed (stop_propagation called).
    pub consumed: bool,
}

/// Routing destination for an Agent Chat key event.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentChatKeyRoute {
    /// Key was handled by the picker (navigation or accept).
    Picker,
    /// Key was handled by the permission approval surface.
    Permission,
    /// Key was handled by the search overlay.
    Search,
    /// Key was handled by the composer input.
    Composer,
    /// Key was handled by a command shortcut (Cmd+K, Cmd+F, etc.).
    Command,
    /// Key was propagated to the parent (not consumed).
    Propagated,
    /// Key was handled by the setup mode handler.
    Setup,
}

/// Structured telemetry event emitted when a picker item is accepted.
///
/// Emitted on `script_kit::agent_chat_telemetry` target alongside the key-route event.
/// Preserves the trigger, item identity, and cursor position so agents can
/// verify acceptance without fuzzy log matching.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatPickerItemAcceptedTelemetry {
    /// Trigger character that opened the picker: `@` or `/`.
    pub trigger: String,

    /// Human-readable label of the accepted item.
    pub item_label: String,

    /// Machine ID of the accepted item (e.g. `built_in:context`).
    pub item_id: String,

    /// The key that caused the accept: `"enter"` or `"tab"`.
    pub accepted_via_key: String,

    /// Cursor character index after the accepted text was inserted.
    pub cursor_after: usize,

    /// Whether the accept also caused a message submission.
    pub caused_submit: bool,
}

/// Machine-readable trace of the most recent Agent Chat interaction.
///
/// Synthesised from the latest key-route and (optional) picker-acceptance
/// telemetry events so agents can verify Enter-vs-Tab behaviour, picker
/// acceptance, and caret movement in a single record without correlating
/// multiple probe arrays.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatLastInteractionTrace {
    /// The key that was pressed (e.g. `"enter"`, `"tab"`).
    pub key: String,

    /// Where the key was routed (e.g. `"picker"`, `"composer"`).
    pub route: String,

    /// Whether the picker overlay was open before the key was processed.
    pub picker_open_before: bool,

    /// The key that caused a picker accept, if any (`"enter"` or `"tab"`).
    /// `None` when the key did not trigger a picker acceptance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_via_key: Option<String>,

    /// Human-readable label of the accepted picker item, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_label: Option<String>,

    /// Cursor character index before the key was processed.
    pub cursor_before: usize,

    /// Cursor character index after the key was processed.
    pub cursor_after: usize,

    /// Whether the key caused a message submission.
    pub caused_submit: bool,
}

/// Schema version for the Agent Chat test probe response envelope.
pub const AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION: u32 = 2;

/// Maximum number of events stored per category in the test probe ring buffer.
pub const AGENT_CHAT_TEST_PROBE_MAX_EVENTS: usize = 32;

/// Top-level Agent Chat test probe snapshot returned by `getAgentChatTestProbe`.
///
/// Contains a bounded tail of recent key-route, picker-acceptance, and
/// input-layout telemetry events so agents can verify native Agent Chat
/// interactions without grepping logs or inferring from screenshots.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatTestProbeSnapshot {
    /// Schema version for forward compatibility.
    pub schema_version: u32,

    /// Monotonically increasing event sequence counter.
    pub event_seq: u64,

    /// Recent key-route telemetry events (bounded ring buffer tail).
    pub key_routes: Vec<AgentChatKeyRouteTelemetry>,

    /// Recent picker-acceptance telemetry events (bounded ring buffer tail).
    pub accepted_items: Vec<AgentChatPickerItemAcceptedTelemetry>,

    /// Most recent input-layout telemetry snapshot, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_layout: Option<AgentChatInputLayoutTelemetry>,

    /// Synthesised trace of the most recent interaction (key-route + optional
    /// picker acceptance merged into one record). `None` when no key events
    /// have been recorded since the last probe reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_interaction_trace: Option<AgentChatLastInteractionTrace>,

    /// Current Agent Chat state snapshot at the time the probe was queried.
    pub state: AgentChatStateSnapshot,

    /// Machine-readable warning codes from the canonical automation vocabulary.
    ///
    /// Present when the request resolved but execution was degraded or rejected
    /// (e.g. `target_unsupported_non_main` when a non-main target was requested).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Default for AgentChatTestProbeSnapshot {
    fn default() -> Self {
        Self {
            schema_version: AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION,
            event_seq: 0,
            key_routes: Vec::new(),
            accepted_items: Vec::new(),
            input_layout: None,
            last_interaction_trace: None,
            state: AgentChatStateSnapshot::default(),
            warnings: Vec::new(),
        }
    }
}

/// Structured telemetry event for single-line input layout after mutations.
///
/// Emitted on `script_kit::agent_chat_telemetry` target after acceptance or cursor
/// moves that may shift the visible window.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatInputLayoutTelemetry {
    /// Total character count in the input.
    pub char_count: usize,

    /// Visible window start (character index).
    pub visible_start: usize,

    /// Visible window end (character index, exclusive).
    pub visible_end: usize,

    /// Cursor position within the visible window (0-based from visible_start).
    pub cursor_in_window: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AgentChatStateSnapshot serde ──────────────────────────────────

    #[test]
    fn agent_chat_state_snapshot_default_round_trips() {
        let snap = AgentChatStateSnapshot::default();
        let json = serde_json::to_value(&snap).expect("serialize default snapshot");
        assert_eq!(json["schemaVersion"], AGENT_CHAT_STATE_SCHEMA_VERSION);
        assert_eq!(json["status"], "idle");
        assert_eq!(json["cursorIndex"], 0);
        assert!(!json["hasSelection"].as_bool().unwrap_or(true));
        assert!(json["picker"].is_null());
        assert!(json["spine"].is_null());
        assert!(json["lastAcceptedItem"].is_null());

        let back: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize default snapshot");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_with_picker_round_trips() {
        let snap = AgentChatStateSnapshot {
            picker: Some(AgentChatPickerState {
                open: true,
                trigger: "@".to_string(),
                item_count: 5,
                selected_index: 2,
                selected_label: Some("file.rs".to_string()),
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize with picker");
        assert_eq!(json["picker"]["open"], true);
        assert_eq!(json["picker"]["trigger"], "@");
        assert_eq!(json["picker"]["itemCount"], 5);
        assert_eq!(json["picker"]["selectedIndex"], 2);
        assert_eq!(json["picker"]["selectedLabel"], "file.rs");

        let back: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize with picker");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_spine_snapshot_round_trips() {
        let state = AgentChatSpineSnapshot {
            owns_list: true,
            active_segment_kind: "contextMention".to_string(),
            subsearch_source: Some("file".to_string()),
            row_count: 10,
            selectable_row_count: 9,
            selected_index: 1,
            row_fingerprint: Some("fnv1a64:0000000000000001".to_string()),
            selected_row_fingerprint: Some("fnv1a64:0000000000000002".to_string()),
            refresh_elapsed_ms: 7,
        };
        let json = serde_json::to_value(&state).expect("serialize spine state");
        assert_eq!(json["ownsList"], true);
        assert_eq!(json["activeSegmentKind"], "contextMention");
        assert_eq!(json["subsearchSource"], "file");
        assert_eq!(json["rowCount"], 10);
        assert_eq!(json["selectableRowCount"], 9);
        assert_eq!(json["selectedIndex"], 1);
        assert_eq!(json["rowFingerprint"], "fnv1a64:0000000000000001");
        assert_eq!(json["selectedRowFingerprint"], "fnv1a64:0000000000000002");
        assert_eq!(json["refreshElapsedMs"], 7);

        let back: AgentChatSpineSnapshot =
            serde_json::from_value(json).expect("deserialize spine state");
        assert_eq!(back, state);
    }

    #[test]
    fn agent_chat_state_snapshot_with_spine_round_trips() {
        let snap = AgentChatStateSnapshot {
            spine: Some(AgentChatSpineSnapshot {
                owns_list: true,
                active_segment_kind: "contextMention".to_string(),
                subsearch_source: Some("clipboard".to_string()),
                row_count: 4,
                selectable_row_count: 3,
                selected_index: 0,
                row_fingerprint: Some("fnv1a64:0000000000000003".to_string()),
                selected_row_fingerprint: Some("fnv1a64:0000000000000004".to_string()),
                refresh_elapsed_ms: 0,
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize with spine");
        assert!(json["spine"].is_object());
        assert_eq!(json["spine"]["ownsList"], true);
        assert_eq!(json["spine"]["subsearchSource"], "clipboard");
        assert!(json["spine"]["rowFingerprint"].is_string());

        let back: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize with spine");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_default_omits_spine() {
        let snap = AgentChatStateSnapshot::default();
        let json = serde_json::to_value(&snap).expect("serialize");
        assert!(
            json.get("spine").is_none(),
            "spine should be omitted when None"
        );
    }

    #[test]
    fn agent_chat_state_snapshot_with_accepted_item_round_trips() {
        let snap = AgentChatStateSnapshot {
            last_accepted_item: Some(AgentChatAcceptedItem {
                label: "context".to_string(),
                id: "built_in:context".to_string(),
                trigger: "@".to_string(),
                cursor_after: 9,
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize with accepted item");
        assert_eq!(json["lastAcceptedItem"]["label"], "context");
        assert_eq!(json["lastAcceptedItem"]["cursorAfter"], 9);

        let back: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize with accepted item");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_with_layout_metrics_round_trips() {
        let snap = AgentChatStateSnapshot {
            input_layout: Some(AgentChatInputLayoutMetrics {
                char_count: 42,
                visible_start: 10,
                visible_end: 35,
                cursor_in_window: 5,
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize with layout");
        assert_eq!(json["inputLayout"]["charCount"], 42);
        assert_eq!(json["inputLayout"]["visibleStart"], 10);

        let back: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize with layout");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_with_transcript_scroll_metrics_round_trips() {
        let snap = AgentChatStateSnapshot {
            transcript_scroll: Some(AgentChatTranscriptScrollMetrics {
                row_count: 160,
                scroll_top_item: 80,
                scroll_top_offset_px: 4.0,
                viewport_height_px: 500.0,
                content_height_px: 2000.0,
                scroll_top_px: 750.0,
                max_scroll_top_px: 1500.0,
                can_scroll_y: true,
                thumb_track_height_px: 500.0,
                thumb_height_px: 125.0,
                thumb_top_px: 187.5,
                thumb_bottom_px: 312.5,
                thumb_position_ratio: 0.5,
                measurement_source: "listState".to_string(),
                follow_tail: false,
                manual_scroll: true,
                pending_indicator_count: 0,
                streaming_copy_available: false,
                row_semantic_ids: vec!["agent-chat-transcript-row-user-1".to_string()],
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize with transcript scroll metrics");
        assert_eq!(json["transcriptScroll"]["rowCount"], 160);
        assert_eq!(json["transcriptScroll"]["scrollTopItem"], 80);
        assert_eq!(json["transcriptScroll"]["thumbPositionRatio"], 0.5);
        assert_eq!(json["transcriptScroll"]["measurementSource"], "listState");

        let back: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize with transcript scroll metrics");
        assert_eq!(back, snap);
    }

    // ── AgentChatResolvedTarget serde ──────────────────────────────────

    #[test]
    fn agent_chat_resolved_target_round_trips() {
        let target = AgentChatResolvedTarget {
            window_id: "agentChatDetached:thread-1".to_string(),
            window_kind: "agentChatDetached".to_string(),
            title: Some("Script Kit Agent Chat".to_string()),
        };
        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(json["windowId"], "agentChatDetached:thread-1");
        assert_eq!(json["windowKind"], "agentChatDetached");
        assert_eq!(json["title"], "Script Kit Agent Chat");

        let back: AgentChatResolvedTarget = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, target);
    }

    #[test]
    fn agent_chat_state_snapshot_with_resolved_target_round_trips() {
        let snap = AgentChatStateSnapshot {
            resolved_target: Some(AgentChatResolvedTarget {
                window_id: "agentChatDetached:thread-1".to_string(),
                window_kind: "agentChatDetached".to_string(),
                title: Some("Script Kit Agent Chat".to_string()),
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize");
        assert_eq!(
            json["resolvedTarget"]["windowId"],
            "agentChatDetached:thread-1"
        );
        assert_eq!(json["resolvedTarget"]["windowKind"], "agentChatDetached");
        assert_eq!(json["schemaVersion"], AGENT_CHAT_STATE_SCHEMA_VERSION);

        let back: AgentChatStateSnapshot = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_without_resolved_target_omits_field() {
        let snap = AgentChatStateSnapshot::default();
        let json = serde_json::to_value(&snap).expect("serialize");
        assert!(
            json.get("resolvedTarget").is_none() || json["resolvedTarget"].is_null(),
            "resolvedTarget should be omitted when None"
        );
    }

    // ── AgentChatWaitCondition serde ──────────────────────────────────

    #[test]
    fn agent_chat_wait_condition_ready_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatReady;
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatReady");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_picker_open_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatPickerOpen;
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatPickerOpen");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_picker_closed_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatPickerClosed;
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatPickerClosed");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_item_accepted_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatItemAccepted;
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatItemAccepted");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_cursor_at_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatCursorAt { index: 15 };
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatCursorAt");
        assert_eq!(json["index"], 15);

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_status_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatStatus {
            status: "streaming".to_string(),
        };
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatStatus");
        assert_eq!(json["status"], "streaming");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_input_match_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatInputMatch {
            text: "@context ".to_string(),
        };
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatInputMatch");
        assert_eq!(json["text"], "@context ");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    #[test]
    fn agent_chat_wait_condition_input_contains_round_trips() {
        let cond = AgentChatWaitCondition::AgentChatInputContains {
            substring: "hello".to_string(),
        };
        let json = serde_json::to_value(&cond).expect("serialize");
        assert_eq!(json["type"], "agent_chatInputContains");
        assert_eq!(json["substring"], "hello");

        let back: AgentChatWaitCondition = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, cond);
    }

    // ─�� AgentChatKeyRouteTelemetry serde ��─────────────────────────────

    #[test]
    fn agent_chat_key_route_telemetry_round_trips() {
        let telemetry = AgentChatKeyRouteTelemetry {
            key: "enter".to_string(),
            route: AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 5,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        };
        let json = serde_json::to_value(&telemetry).expect("serialize telemetry");
        assert_eq!(json["key"], "enter");
        assert_eq!(json["route"], "picker");
        assert!(json["pickerOpen"].as_bool().unwrap_or(false));
        assert_eq!(json["cursorBefore"], 5);
        assert_eq!(json["cursorAfter"], 9);

        let back: AgentChatKeyRouteTelemetry =
            serde_json::from_value(json).expect("deserialize telemetry");
        assert_eq!(back, telemetry);
    }

    #[test]
    fn agent_chat_key_route_variants_serialize_correctly() {
        let routes = vec![
            (AgentChatKeyRoute::Picker, "picker"),
            (AgentChatKeyRoute::Permission, "permission"),
            (AgentChatKeyRoute::Search, "search"),
            (AgentChatKeyRoute::Composer, "composer"),
            (AgentChatKeyRoute::Command, "command"),
            (AgentChatKeyRoute::Propagated, "propagated"),
            (AgentChatKeyRoute::Setup, "setup"),
        ];
        for (route, expected) in routes {
            let json = serde_json::to_value(&route).expect("serialize route");
            assert_eq!(json.as_str().unwrap_or(""), expected);
        }
    }

    // ── AgentChatPickerState serde ────────────────────────────────────

    #[test]
    fn agent_chat_picker_state_round_trips() {
        let state = AgentChatPickerState {
            open: true,
            trigger: "/".to_string(),
            item_count: 3,
            selected_index: 1,
            selected_label: Some("context-full".to_string()),
        };
        let json = serde_json::to_value(&state).expect("serialize picker state");
        assert_eq!(json["trigger"], "/");
        assert_eq!(json["itemCount"], 3);

        let back: AgentChatPickerState =
            serde_json::from_value(json).expect("deserialize picker state");
        assert_eq!(back, state);
    }

    // ── Full snapshot JSON shape ────────────────────────────────

    #[test]
    fn agent_chat_state_snapshot_full_json_shape() {
        let snap = AgentChatStateSnapshot {
            schema_version: AGENT_CHAT_STATE_SCHEMA_VERSION,
            resolved_target: None,
            status: "streaming".to_string(),
            ui_variant: "standard".to_string(),
            input_text: "hello @context".to_string(),
            cursor_index: 14,
            has_selection: false,
            selection_range: None,
            message_count: 3,
            retained_thread_count: 0,
            fork_point_count: 0,
            awaiting_first_assistant_text: true,
            picker: None,
            spine: Some(AgentChatSpineSnapshot {
                owns_list: true,
                active_segment_kind: "contextMention".to_string(),
                subsearch_source: Some("file".to_string()),
                row_count: 2,
                selectable_row_count: 2,
                selected_index: 0,
                row_fingerprint: Some("fnv1a64:0000000000000005".to_string()),
                selected_row_fingerprint: Some("fnv1a64:0000000000000006".to_string()),
                refresh_elapsed_ms: 1,
            }),
            last_accepted_item: Some(AgentChatAcceptedItem {
                label: "context".to_string(),
                id: "built_in:context".to_string(),
                trigger: "@".to_string(),
                cursor_after: 14,
            }),
            context_chip_count: 1,
            context_summary: Some("context".to_string()),
            dictation_phase: Some("recording".to_string()),
            context_ready: true,
            has_pending_permission: false,
            input_layout: Some(AgentChatInputLayoutMetrics {
                char_count: 14,
                visible_start: 0,
                visible_end: 14,
                cursor_in_window: 14,
            }),
            transcript_scroll: None,
            composer_scroll: None,
            focused_text: None,
            setup: None,
            warnings: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&snap).expect("serialize full snapshot");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("parse full snapshot JSON");

        // Verify top-level keys are present
        assert!(parsed["schemaVersion"].is_number());
        assert!(parsed["status"].is_string());
        assert_eq!(parsed["uiVariant"], "standard");
        assert!(parsed["inputText"].is_string());
        assert!(parsed["cursorIndex"].is_number());
        assert!(parsed["hasSelection"].is_boolean());
        assert!(parsed["messageCount"].is_number());
        assert_eq!(parsed["awaitingFirstAssistantText"], true);
        assert!(parsed["lastAcceptedItem"].is_object());
        assert!(parsed["spine"].is_object());
        assert_eq!(parsed["spine"]["ownsList"], true);
        assert!(parsed["spine"]["rowFingerprint"].is_string());
        assert!(parsed["contextChipCount"].is_number());
        assert_eq!(parsed["contextSummary"], "context");
        assert_eq!(parsed["dictationPhase"], "recording");
        assert!(parsed["contextReady"].is_boolean());
        assert!(parsed["hasPendingPermission"].is_boolean());
        assert!(parsed["inputLayout"].is_object());
    }

    #[test]
    fn agent_chat_state_snapshot_default_omits_dictation_phase() {
        let snap = AgentChatStateSnapshot::default();
        let json = serde_json::to_value(&snap).expect("serialize");
        assert!(
            json.get("dictationPhase").is_none(),
            "dictationPhase should be omitted when None"
        );
    }

    // ── Deserialization from external JSON ──────────────────────

    #[test]
    fn agent_chat_state_snapshot_deserializes_from_minimal_json() {
        let json = serde_json::json!({
            "schemaVersion": 1,
            "status": "idle",
            "inputText": "",
            "cursorIndex": 0,
            "hasSelection": false,
            "messageCount": 0,
            "contextChipCount": 0,
            "contextReady": true,
            "hasPendingPermission": false,
        });
        let snap: AgentChatStateSnapshot =
            serde_json::from_value(json).expect("deserialize minimal JSON");
        assert_eq!(snap.status, "idle");
        assert!(!snap.awaiting_first_assistant_text);
        assert!(snap.picker.is_none());
        assert!(snap.last_accepted_item.is_none());
        assert!(snap.input_layout.is_none());
    }

    #[test]
    fn agent_chat_wait_condition_deserializes_from_external_json() {
        let json = serde_json::json!({
            "type": "agent_chatCursorAt",
            "index": 42,
        });
        let cond: AgentChatWaitCondition =
            serde_json::from_value(json).expect("deserialize external JSON");
        assert_eq!(
            cond,
            AgentChatWaitCondition::AgentChatCursorAt { index: 42 }
        );
    }

    // ── AgentChatPickerItemAcceptedTelemetry serde ───────────────────

    #[test]
    fn agent_chat_picker_item_accepted_telemetry_round_trips() {
        let telemetry = AgentChatPickerItemAcceptedTelemetry {
            trigger: "@".to_string(),
            item_label: "context".to_string(),
            item_id: "built_in:context".to_string(),
            accepted_via_key: "enter".to_string(),
            cursor_after: 9,
            caused_submit: false,
        };
        let json = serde_json::to_value(&telemetry).expect("serialize");
        assert_eq!(json["trigger"], "@");
        assert_eq!(json["itemLabel"], "context");
        assert_eq!(json["itemId"], "built_in:context");
        assert_eq!(json["acceptedViaKey"], "enter");
        assert_eq!(json["cursorAfter"], 9);
        assert_eq!(json["causedSubmit"], false);

        let back: AgentChatPickerItemAcceptedTelemetry =
            serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, telemetry);
    }

    #[test]
    fn agent_chat_picker_item_accepted_telemetry_tab_vs_enter_distinct() {
        let enter = AgentChatPickerItemAcceptedTelemetry {
            trigger: "@".to_string(),
            item_label: "context".to_string(),
            item_id: "built_in:context".to_string(),
            accepted_via_key: "enter".to_string(),
            cursor_after: 9,
            caused_submit: false,
        };
        let tab = AgentChatPickerItemAcceptedTelemetry {
            accepted_via_key: "tab".to_string(),
            ..enter.clone()
        };
        let enter_json = serde_json::to_value(&enter).expect("serialize enter");
        let tab_json = serde_json::to_value(&tab).expect("serialize tab");

        assert_eq!(enter_json["acceptedViaKey"], "enter");
        assert_eq!(tab_json["acceptedViaKey"], "tab");
        assert_ne!(
            enter_json["acceptedViaKey"], tab_json["acceptedViaKey"],
            "enter and tab must produce distinct acceptedViaKey values"
        );
    }

    #[test]
    fn agent_chat_picker_item_accepted_telemetry_slash_trigger() {
        let telemetry = AgentChatPickerItemAcceptedTelemetry {
            trigger: "/".to_string(),
            item_label: "compact".to_string(),
            item_id: "slash:compact".to_string(),
            accepted_via_key: "enter".to_string(),
            cursor_after: 9,
            caused_submit: true,
        };
        let json = serde_json::to_value(&telemetry).expect("serialize");
        assert_eq!(json["trigger"], "/");
        assert_eq!(json["causedSubmit"], true);

        let back: AgentChatPickerItemAcceptedTelemetry =
            serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, telemetry);
    }

    // ── AgentChatInputLayoutTelemetry serde ──────────────────────────

    #[test]
    fn agent_chat_input_layout_telemetry_round_trips() {
        let telemetry = AgentChatInputLayoutTelemetry {
            char_count: 42,
            visible_start: 10,
            visible_end: 35,
            cursor_in_window: 5,
        };
        let json = serde_json::to_value(&telemetry).expect("serialize");
        assert_eq!(json["charCount"], 42);
        assert_eq!(json["visibleStart"], 10);
        assert_eq!(json["visibleEnd"], 35);
        assert_eq!(json["cursorInWindow"], 5);

        let back: AgentChatInputLayoutTelemetry =
            serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, telemetry);
    }

    #[test]
    fn agent_chat_input_layout_telemetry_matches_layout_metrics_fields() {
        // Verify the telemetry mirrors AgentChatInputLayoutMetrics field names exactly.
        let metrics = AgentChatInputLayoutMetrics {
            char_count: 24,
            visible_start: 0,
            visible_end: 24,
            cursor_in_window: 11,
        };
        let telemetry = AgentChatInputLayoutTelemetry {
            char_count: metrics.char_count,
            visible_start: metrics.visible_start,
            visible_end: metrics.visible_end,
            cursor_in_window: metrics.cursor_in_window,
        };
        let m_json = serde_json::to_value(&metrics).expect("serialize metrics");
        let t_json = serde_json::to_value(&telemetry).expect("serialize telemetry");
        assert_eq!(
            m_json, t_json,
            "telemetry and metrics must serialize to identical JSON"
        );
    }

    // ── Key route telemetry with picker-accepted context ───────

    #[test]
    fn agent_chat_key_route_telemetry_enter_picker_accept() {
        let route_event = AgentChatKeyRouteTelemetry {
            key: "enter".to_string(),
            route: AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        };
        let json = serde_json::to_value(&route_event).expect("serialize");
        assert_eq!(json["key"], "enter");
        assert_eq!(json["route"], "picker");
        assert!(json["pickerOpen"].as_bool().expect("bool"));
    }

    #[test]
    fn agent_chat_key_route_telemetry_tab_picker_accept() {
        let route_event = AgentChatKeyRouteTelemetry {
            key: "tab".to_string(),
            route: AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        };
        let json = serde_json::to_value(&route_event).expect("serialize");
        assert_eq!(json["key"], "tab");
        assert_eq!(json["route"], "picker");
    }

    #[test]
    fn agent_chat_key_route_enter_vs_tab_distinct_key_field() {
        let enter = AgentChatKeyRouteTelemetry {
            key: "enter".to_string(),
            route: AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        };
        let tab = AgentChatKeyRouteTelemetry {
            key: "tab".to_string(),
            ..enter.clone()
        };
        let e_json = serde_json::to_value(&enter).expect("serialize enter");
        let t_json = serde_json::to_value(&tab).expect("serialize tab");
        assert_ne!(
            e_json["key"], t_json["key"],
            "enter and tab key-route events must have distinct key fields"
        );
    }

    // ── AgentChatTestProbeSnapshot serde ─────────────────────────────

    #[test]
    fn agent_chat_test_probe_snapshot_default_round_trips() {
        let snap = AgentChatTestProbeSnapshot::default();
        let json = serde_json::to_value(&snap).expect("serialize default probe");
        assert_eq!(json["schemaVersion"], AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION);
        assert_eq!(json["eventSeq"], 0);
        assert!(json["keyRoutes"].as_array().expect("array").is_empty());
        assert!(json["acceptedItems"].as_array().expect("array").is_empty());
        assert!(json.get("inputLayout").is_none());

        let back: AgentChatTestProbeSnapshot =
            serde_json::from_value(json).expect("deserialize default probe");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_test_probe_snapshot_with_events_round_trips() {
        let snap = AgentChatTestProbeSnapshot {
            schema_version: AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION,
            event_seq: 14,
            key_routes: vec![AgentChatKeyRouteTelemetry {
                key: "tab".to_string(),
                route: AgentChatKeyRoute::Picker,
                picker_open: true,
                permission_active: false,
                cursor_before: 1,
                cursor_after: 17,
                caused_submit: false,
                consumed: true,
            }],
            accepted_items: vec![AgentChatPickerItemAcceptedTelemetry {
                trigger: "@".to_string(),
                item_label: "What I\u{2019}m Looking At".to_string(),
                item_id: "built_in:context".to_string(),
                accepted_via_key: "tab".to_string(),
                cursor_after: 17,
                caused_submit: false,
            }],
            input_layout: Some(AgentChatInputLayoutTelemetry {
                char_count: 27,
                visible_start: 0,
                visible_end: 27,
                cursor_in_window: 17,
            }),
            last_interaction_trace: Some(AgentChatLastInteractionTrace {
                key: "tab".to_string(),
                route: "picker".to_string(),
                picker_open_before: true,
                accepted_via_key: Some("tab".to_string()),
                accepted_label: Some("What I\u{2019}m Looking At".to_string()),
                cursor_before: 1,
                cursor_after: 17,
                caused_submit: false,
            }),
            state: AgentChatStateSnapshot {
                status: "idle".to_string(),
                cursor_index: 17,
                ..Default::default()
            },
            warnings: Vec::new(),
        };
        let json = serde_json::to_value(&snap).expect("serialize probe with events");
        assert_eq!(json["eventSeq"], 14);
        assert_eq!(json["keyRoutes"][0]["key"], "tab");
        assert_eq!(json["acceptedItems"][0]["acceptedViaKey"], "tab");
        assert_eq!(json["acceptedItems"][0]["cursorAfter"], 17);
        assert_eq!(json["inputLayout"]["cursorInWindow"], 17);
        assert_eq!(json["state"]["cursorIndex"], 17);

        let back: AgentChatTestProbeSnapshot =
            serde_json::from_value(json).expect("deserialize probe with events");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_test_probe_snapshot_schema_version_constant() {
        assert_eq!(AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION, 2);
    }

    #[test]
    fn agent_chat_test_probe_max_events_constant() {
        assert_eq!(AGENT_CHAT_TEST_PROBE_MAX_EVENTS, 32);
    }

    // ── AgentChatSetupActionKind serde ──────────────────────────────

    #[test]
    fn agent_chat_setup_action_kind_round_trips() {
        let kinds = vec![
            (AgentChatSetupActionKind::Retry, "retry"),
            (AgentChatSetupActionKind::Install, "install"),
            (AgentChatSetupActionKind::Authenticate, "authenticate"),
            (AgentChatSetupActionKind::OpenCatalog, "openCatalog"),
            (AgentChatSetupActionKind::SelectAgent, "selectAgent"),
            (AgentChatSetupActionKind::OpenAgentPicker, "openAgentPicker"),
            (
                AgentChatSetupActionKind::CloseAgentPicker,
                "closeAgentPicker",
            ),
        ];
        for (kind, expected) in kinds {
            let json = serde_json::to_value(&kind).expect("serialize");
            assert_eq!(json.as_str().unwrap_or(""), expected);
            let back: AgentChatSetupActionKind = serde_json::from_value(json).expect("deserialize");
            assert_eq!(back, kind);
        }
    }

    // ── AgentChatSetupSnapshot serde ────────────────────────────────

    #[test]
    fn agent_chat_setup_snapshot_round_trips() {
        let snap = AgentChatSetupSnapshot {
            reason_code: "capabilityMismatch".to_string(),
            title: "Agent Chat capability mismatch".to_string(),
            body: "No compatible agent".to_string(),
            primary_action: AgentChatSetupActionKind::Retry,
            secondary_action: Some(AgentChatSetupActionKind::OpenCatalog),
            selected_agent_id: Some("claude-code".to_string()),
            catalog_agent_ids: vec!["claude-code".to_string(), "opencode".to_string()],
            compatible_agent_ids: vec![],
            needs_image: true,
            needs_embedded_context: false,
            agent_picker_open: false,
            agent_picker_selected_id: None,
        };
        let json = serde_json::to_value(&snap).expect("serialize setup snapshot");
        assert_eq!(json["reasonCode"], "capabilityMismatch");
        assert_eq!(json["primaryAction"], "retry");
        assert_eq!(json["secondaryAction"], "openCatalog");
        assert_eq!(json["selectedAgentId"], "claude-code");
        assert_eq!(json["needsImage"], true);
        assert_eq!(json["agentPickerOpen"], false);

        let back: AgentChatSetupSnapshot = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_with_setup_round_trips() {
        let snap = AgentChatStateSnapshot {
            status: "setup".to_string(),
            setup: Some(AgentChatSetupSnapshot {
                reason_code: "agentNotInstalled".to_string(),
                title: "Agent install required".to_string(),
                body: "Install the agent".to_string(),
                primary_action: AgentChatSetupActionKind::Install,
                secondary_action: Some(AgentChatSetupActionKind::SelectAgent),
                selected_agent_id: Some("opencode".to_string()),
                catalog_agent_ids: vec!["opencode".to_string()],
                compatible_agent_ids: vec![],
                needs_image: false,
                needs_embedded_context: false,
                agent_picker_open: false,
                agent_picker_selected_id: None,
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize with setup");
        assert_eq!(json["status"], "setup");
        assert!(json["setup"].is_object());
        assert_eq!(json["setup"]["reasonCode"], "agentNotInstalled");
        assert_eq!(json["setup"]["primaryAction"], "install");

        let back: AgentChatStateSnapshot = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, snap);
    }

    #[test]
    fn agent_chat_state_snapshot_setup_none_omitted_in_json() {
        let snap = AgentChatStateSnapshot::default();
        let json = serde_json::to_value(&snap).expect("serialize");
        assert!(
            json.get("setup").is_none(),
            "setup field should be omitted when None"
        );
    }

    // ── AgentChatLastInteractionTrace serde ────────────────────────

    #[test]
    fn last_interaction_trace_round_trips_with_accept() {
        let trace = AgentChatLastInteractionTrace {
            key: "tab".to_string(),
            route: "picker".to_string(),
            picker_open_before: true,
            accepted_via_key: Some("tab".to_string()),
            accepted_label: Some("What I\u{2019}m Looking At".to_string()),
            cursor_before: 1,
            cursor_after: 17,
            caused_submit: false,
        };
        let json = serde_json::to_value(&trace).expect("serialize trace");
        assert_eq!(json["key"], "tab");
        assert_eq!(json["route"], "picker");
        assert_eq!(json["pickerOpenBefore"], true);
        assert_eq!(json["acceptedViaKey"], "tab");
        assert_eq!(json["acceptedLabel"], "What I\u{2019}m Looking At");
        assert_eq!(json["cursorBefore"], 1);
        assert_eq!(json["cursorAfter"], 17);
        assert_eq!(json["causedSubmit"], false);

        let back: AgentChatLastInteractionTrace =
            serde_json::from_value(json).expect("deserialize trace");
        assert_eq!(back, trace);
    }

    #[test]
    fn last_interaction_trace_without_accept_omits_optional_fields() {
        let trace = AgentChatLastInteractionTrace {
            key: "enter".to_string(),
            route: "composer".to_string(),
            picker_open_before: false,
            accepted_via_key: None,
            accepted_label: None,
            cursor_before: 5,
            cursor_after: 5,
            caused_submit: true,
        };
        let json = serde_json::to_value(&trace).expect("serialize");
        assert!(
            json.get("acceptedViaKey").is_none(),
            "acceptedViaKey should be omitted when None"
        );
        assert!(
            json.get("acceptedLabel").is_none(),
            "acceptedLabel should be omitted when None"
        );
        assert_eq!(json["causedSubmit"], true);

        let back: AgentChatLastInteractionTrace =
            serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, trace);
    }

    #[test]
    fn enter_and_tab_traces_produce_distinct_accepted_via_key() {
        let enter_trace = AgentChatLastInteractionTrace {
            key: "enter".to_string(),
            route: "picker".to_string(),
            picker_open_before: true,
            accepted_via_key: Some("enter".to_string()),
            accepted_label: Some("Context".to_string()),
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
        };
        let tab_trace = AgentChatLastInteractionTrace {
            key: "tab".to_string(),
            route: "picker".to_string(),
            picker_open_before: true,
            accepted_via_key: Some("tab".to_string()),
            accepted_label: Some("Context".to_string()),
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
        };
        assert_ne!(
            enter_trace.accepted_via_key, tab_trace.accepted_via_key,
            "enter and tab accepts must have distinct acceptedViaKey"
        );
    }

    #[test]
    fn probe_snapshot_default_has_no_trace() {
        let snap = AgentChatTestProbeSnapshot::default();
        assert!(
            snap.last_interaction_trace.is_none(),
            "default probe must have no interaction trace"
        );
        let json = serde_json::to_value(&snap).expect("serialize");
        assert!(
            json.get("lastInteractionTrace").is_none(),
            "lastInteractionTrace should be omitted when None"
        );
    }

    #[test]
    fn probe_snapshot_with_trace_round_trips() {
        let trace = AgentChatLastInteractionTrace {
            key: "tab".to_string(),
            route: "picker".to_string(),
            picker_open_before: true,
            accepted_via_key: Some("tab".to_string()),
            accepted_label: Some("What I\u{2019}m Looking At".to_string()),
            cursor_before: 1,
            cursor_after: 17,
            caused_submit: false,
        };
        let snap = AgentChatTestProbeSnapshot {
            last_interaction_trace: Some(trace.clone()),
            ..Default::default()
        };
        let json = serde_json::to_value(&snap).expect("serialize");
        assert!(json["lastInteractionTrace"].is_object());
        assert_eq!(json["lastInteractionTrace"]["key"], "tab");
        assert_eq!(json["lastInteractionTrace"]["acceptedViaKey"], "tab");
        assert_eq!(json["lastInteractionTrace"]["causedSubmit"], false);

        let back: AgentChatTestProbeSnapshot = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back.last_interaction_trace, Some(trace));
    }

    #[test]
    fn probe_snapshot_backward_compatible_without_trace_field() {
        // Simulate a JSON payload from an older producer that lacks lastInteractionTrace.
        let json = serde_json::json!({
            "schemaVersion": 1,
            "eventSeq": 0,
            "keyRoutes": [],
            "acceptedItems": [],
            "state": AgentChatStateSnapshot::default(),
        });
        let snap: AgentChatTestProbeSnapshot =
            serde_json::from_value(json).expect("deserialize old-format probe");
        assert!(
            snap.last_interaction_trace.is_none(),
            "missing field should deserialize as None"
        );
    }

    // ── Permission-active key route telemetry ────────────────────

    #[test]
    fn agent_chat_key_route_telemetry_permission_active_true() {
        let telemetry = AgentChatKeyRouteTelemetry {
            key: "enter".to_string(),
            route: AgentChatKeyRoute::Composer,
            picker_open: false,
            permission_active: true,
            cursor_before: 5,
            cursor_after: 0,
            caused_submit: true,
            consumed: true,
        };
        let json = serde_json::to_value(&telemetry).expect("serialize");
        assert!(
            json["permissionActive"].as_bool().expect("bool"),
            "permissionActive must be true when a permission approval is pending"
        );
        let back: AgentChatKeyRouteTelemetry = serde_json::from_value(json).expect("deserialize");
        assert!(back.permission_active);
    }

    #[test]
    fn agent_chat_key_route_telemetry_permission_active_false() {
        let telemetry = AgentChatKeyRouteTelemetry {
            key: "tab".to_string(),
            route: AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        };
        let json = serde_json::to_value(&telemetry).expect("serialize");
        assert!(
            !json["permissionActive"].as_bool().expect("bool"),
            "permissionActive must be false when no permission approval is pending"
        );
        let back: AgentChatKeyRouteTelemetry = serde_json::from_value(json).expect("deserialize");
        assert!(!back.permission_active);
    }

    #[test]
    fn agent_chat_key_route_telemetry_permission_active_distinguishes_states() {
        let base = AgentChatKeyRouteTelemetry {
            key: "enter".to_string(),
            route: AgentChatKeyRoute::Picker,
            picker_open: true,
            permission_active: false,
            cursor_before: 1,
            cursor_after: 9,
            caused_submit: false,
            consumed: true,
        };
        let with_permission = AgentChatKeyRouteTelemetry {
            permission_active: true,
            ..base.clone()
        };
        let json_without = serde_json::to_value(&base).expect("serialize without");
        let json_with = serde_json::to_value(&with_permission).expect("serialize with");
        assert_ne!(
            json_without["permissionActive"], json_with["permissionActive"],
            "permission_active=false and permission_active=true must produce distinct JSON"
        );
    }
}
