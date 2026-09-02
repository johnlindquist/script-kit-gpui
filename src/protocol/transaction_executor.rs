//! Transaction flight recorder executor for `waitFor` and `batch` commands.
//!
//! Executes deterministic UI transactions against a [`TransactionStateProvider`],
//! producing per-command receipts with before/after snapshots, poll observations,
//! elapsed timings, and actionable failure suggestions.

use crate::protocol::transaction_trace::{
    append_transaction_trace, now_epoch_ms, remember_persisted_transaction_results,
    restore_persisted_transaction_result, sanitize_transaction_trace, should_include_trace,
    transaction_content_fingerprint,
};
use crate::protocol::types::batch_wait::{
    BatchCommand, BatchOptions, BatchResultEntry, StateMatchSpec, TransactionCommandTrace,
    TransactionError, TransactionErrorCode, TransactionTrace, TransactionTraceMode,
    TransactionTraceStatus, UiStateSnapshot, WaitCondition, WaitDetailedCondition,
    WaitNamedCondition, WaitPollObservation,
};
use anyhow::Result;
use std::path::Path;
use std::time::{Duration, Instant};

// ── Default constants ──────────────────────────────────────────────────────

const DEFAULT_WAIT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_WAIT_POLL_INTERVAL_MS: u64 = 25;
pub const MAX_BATCH_COMMANDS: usize = 256;
pub const MAX_WAIT_POLLS: usize = 4096;

// ── Provider trait ─────────────────────────────────────────────────────────

/// Abstraction over the live UI state, allowing the executor to be tested
/// without a running GPUI window.
pub trait TransactionStateProvider {
    /// Take a snapshot of the current UI state.
    fn snapshot(&self) -> UiStateSnapshot;
    /// Set the input/filter field text.
    fn set_input(&mut self, text: &str) -> Result<()>;
    /// Select a choice by value, optionally submitting. Returns the matched
    /// value or `None` if no choice matched.
    fn select_by_value(&mut self, value: &str, submit: bool) -> Result<Option<String>>;
    /// Select a choice by semantic ID, optionally submitting. Returns the
    /// matched value or `None` if no element matched the semantic ID.
    fn select_by_semantic_id(&mut self, semantic_id: &str, submit: bool) -> Result<Option<String>>;

    /// Return the most recent Agent Chat test probe snapshot for proof-level
    /// condition evaluation. Providers without Agent Chat state return a default
    /// (empty) snapshot, which causes all Agent Chat proof conditions to evaluate
    /// as not-matched.
    fn agent_chat_test_probe(&self, _tail: usize) -> crate::protocol::AgentChatTestProbeSnapshot {
        crate::protocol::AgentChatTestProbeSnapshot::default()
    }
}

// ── Condition matching ─────────────────────────────────────────────────────

/// Check whether a `UiStateSnapshot` satisfies a `StateMatchSpec`.
///
/// Exported for use by Notes (and other non-main) condition checkers
/// in the prompt handler that cannot go through the transaction executor.
pub fn matches_state_spec(snapshot: &UiStateSnapshot, spec: &StateMatchSpec) -> bool {
    matches_state(snapshot, spec)
}

/// Match production Chat state for either an embedded or detached owner.
/// Probe collection is lazy: ordinary state predicates never copy the probe tail.
pub fn matches_agent_chat_wait_condition(
    condition: &crate::protocol::WaitDetailedCondition,
    state: &crate::protocol::AgentChatStateSnapshot,
    probe_fn: impl FnOnce() -> crate::protocol::AgentChatTestProbeSnapshot,
) -> Option<bool> {
    Some(match condition {
        crate::protocol::WaitDetailedCondition::AgentChatReady => {
            state.context_ready && state.status == "idle"
        }
        crate::protocol::WaitDetailedCondition::AgentChatPickerOpen => {
            state.picker.as_ref().is_some_and(|p| p.open)
        }
        crate::protocol::WaitDetailedCondition::AgentChatPickerClosed => {
            state.picker.is_none() || state.picker.as_ref().is_some_and(|p| !p.open)
        }
        crate::protocol::WaitDetailedCondition::AgentChatItemAccepted => {
            state.last_accepted_item.is_some()
        }
        crate::protocol::WaitDetailedCondition::AgentChatCursorAt { index } => {
            state.cursor_index == *index
        }
        crate::protocol::WaitDetailedCondition::AgentChatStatus { status } => {
            state.status == *status
        }
        crate::protocol::WaitDetailedCondition::AgentChatInputMatch { text } => {
            state.input_text == *text
        }
        crate::protocol::WaitDetailedCondition::AgentChatInputContains { substring } => {
            state.input_text.contains(substring.as_str())
        }
        crate::protocol::WaitDetailedCondition::AgentChatAcceptedViaKey { key } => {
            let probe = probe_fn();
            probe
                .accepted_items
                .last()
                .is_some_and(|item| item.accepted_via_key == *key)
        }
        crate::protocol::WaitDetailedCondition::AgentChatAcceptedLabel { label } => {
            let probe = probe_fn();
            probe
                .accepted_items
                .last()
                .is_some_and(|item| item.item_label == *label)
        }
        crate::protocol::WaitDetailedCondition::AgentChatAcceptedCursorAt { index } => {
            let probe = probe_fn();
            probe
                .accepted_items
                .last()
                .is_some_and(|item| item.cursor_after == *index)
        }
        crate::protocol::WaitDetailedCondition::AgentChatInputLayoutMatch {
            visible_start,
            visible_end,
            cursor_in_window,
        } => {
            let probe = probe_fn();
            probe.input_layout.as_ref().is_some_and(|layout| {
                layout.visible_start == *visible_start
                    && layout.visible_end == *visible_end
                    && layout.cursor_in_window == *cursor_in_window
            })
        }
        crate::protocol::WaitDetailedCondition::AgentChatSetupVisible => state.setup.is_some(),
        crate::protocol::WaitDetailedCondition::AgentChatSetupReasonCode { reason_code } => state
            .setup
            .as_ref()
            .is_some_and(|s| s.reason_code == *reason_code),
        crate::protocol::WaitDetailedCondition::AgentChatSetupPrimaryAction { action } => state
            .setup
            .as_ref()
            .is_some_and(|s| s.primary_action == *action),
        crate::protocol::WaitDetailedCondition::AgentChatSetupAgentPickerOpen => {
            state.setup.as_ref().is_some_and(|s| s.agent_picker_open)
        }
        crate::protocol::WaitDetailedCondition::AgentChatSetupSelectedAgent { agent_id } => {
            state.setup.as_ref().is_some_and(|s| {
                s.selected_agent_id
                    .as_ref()
                    .is_some_and(|id| id == agent_id)
            })
        }
        // Non-Agent Chat conditions (already handled above, but required for exhaustiveness)
        _ => return None,
    })
}

/// Match conditions that are meaningful on every registered surface.
pub fn matches_ui_wait_condition(
    snapshot: &UiStateSnapshot,
    condition: &WaitCondition,
) -> Option<bool> {
    Some(match condition {
        WaitCondition::Named(WaitNamedCondition::InputEmpty) => {
            snapshot.input_value.as_deref().unwrap_or("").is_empty()
        }
        WaitCondition::Named(WaitNamedCondition::WindowVisible) => snapshot.window_visible,
        WaitCondition::Named(WaitNamedCondition::WindowFocused) => snapshot.window_focused,
        WaitCondition::Named(WaitNamedCondition::ChoicesRendered) => snapshot.choice_count > 0,
        WaitCondition::Detailed(
            WaitDetailedCondition::ElementExists { semantic_id }
            | WaitDetailedCondition::ElementVisible { semantic_id },
        ) => snapshot
            .visible_semantic_ids
            .iter()
            .any(|id| id == semantic_id),
        WaitCondition::Detailed(WaitDetailedCondition::ElementFocused { semantic_id }) => {
            snapshot.focused_semantic_id.as_deref() == Some(semantic_id.as_str())
        }
        WaitCondition::Detailed(WaitDetailedCondition::StateMatch { state }) => {
            matches_state_spec(snapshot, state)
        }
        _ => return None,
    })
}

fn matches_state(snapshot: &UiStateSnapshot, spec: &StateMatchSpec) -> bool {
    if let Some(ref expected) = spec.input_value {
        // UI snapshots omit an empty input as `None`, while automation clients
        // express a wait-for-empty predicate as `Some("")`. Treat both wire
        // representations as the same visible state.
        if snapshot.input_value.as_deref().unwrap_or("") != expected {
            return false;
        }
    }
    if let Some(ref expected) = spec.selected_value {
        if snapshot.selected_value.as_deref() != Some(expected.as_str()) {
            return false;
        }
    }
    if let Some(ref expected) = spec.prompt_type {
        if snapshot.prompt_type.as_deref() != Some(expected.as_str()) {
            return false;
        }
    }
    if let Some(expected) = spec.window_visible {
        if snapshot.window_visible != expected {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod state_match_tests {
    use super::*;

    #[test]
    fn surface_waits_observe_target_visibility_focus_and_elements() {
        let snapshot = UiStateSnapshot {
            window_visible: false,
            window_focused: false,
            visible_semantic_ids: vec!["input:notes".into()],
            focused_semantic_id: Some("input:notes".into()),
            input_value: Some("draft".into()),
            ..Default::default()
        };
        for condition in [
            WaitNamedCondition::WindowVisible,
            WaitNamedCondition::WindowFocused,
            WaitNamedCondition::ChoicesRendered,
            WaitNamedCondition::InputEmpty,
        ] {
            assert_eq!(
                matches_ui_wait_condition(&snapshot, &WaitCondition::Named(condition)),
                Some(false)
            );
        }
        assert_eq!(
            matches_ui_wait_condition(
                &snapshot,
                &WaitCondition::Detailed(WaitDetailedCondition::ElementFocused {
                    semantic_id: "input:notes".into()
                })
            ),
            Some(true)
        );
        assert_eq!(
            matches_ui_wait_condition(
                &snapshot,
                &WaitCondition::Detailed(WaitDetailedCondition::ElementExists {
                    semantic_id: "input:main".into()
                })
            ),
            Some(false)
        );
        assert_eq!(
            matches_ui_wait_condition(
                &snapshot,
                &WaitCondition::Detailed(WaitDetailedCondition::AgentChatPickerClosed)
            ),
            None
        );
    }

    #[test]
    fn chat_ready_requires_idle_context_and_does_not_collect_probe() {
        let mut state = crate::protocol::AgentChatStateSnapshot {
            context_ready: true,
            status: "streaming".into(),
            ..Default::default()
        };
        assert_eq!(
            matches_agent_chat_wait_condition(
                &WaitDetailedCondition::AgentChatReady,
                &state,
                || panic!("state predicates must not collect probes")
            ),
            Some(false)
        );
        state.status = "idle".into();
        assert_eq!(
            matches_agent_chat_wait_condition(
                &WaitDetailedCondition::AgentChatReady,
                &state,
                || panic!("state predicates must not collect probes")
            ),
            Some(true)
        );
        state.context_ready = false;
        assert_eq!(
            matches_agent_chat_wait_condition(
                &WaitDetailedCondition::AgentChatReady,
                &state,
                Default::default
            ),
            Some(false)
        );
    }

    #[test]
    fn chat_setup_waits_use_actual_setup_reason_and_selection() {
        use crate::protocol::{
            AgentChatSetupActionKind, AgentChatSetupSnapshot, AgentChatStateSnapshot,
        };
        let state = AgentChatStateSnapshot {
            setup: Some(AgentChatSetupSnapshot {
                reason_code: "agent_missing".into(),
                title: "Choose an agent".into(),
                body: String::new(),
                primary_action: AgentChatSetupActionKind::Retry,
                secondary_action: None,
                selected_agent_id: Some("fixture-agent".into()),
                catalog_agent_ids: vec![],
                compatible_agent_ids: vec![],
                needs_image: false,
                needs_embedded_context: false,
                agent_picker_open: true,
                agent_picker_selected_id: None,
            }),
            ..Default::default()
        };
        for condition in [
            WaitDetailedCondition::AgentChatSetupVisible,
            WaitDetailedCondition::AgentChatSetupReasonCode {
                reason_code: "agent_missing".into(),
            },
            WaitDetailedCondition::AgentChatSetupPrimaryAction {
                action: AgentChatSetupActionKind::Retry,
            },
            WaitDetailedCondition::AgentChatSetupAgentPickerOpen,
            WaitDetailedCondition::AgentChatSetupSelectedAgent {
                agent_id: "fixture-agent".into(),
            },
        ] {
            assert_eq!(
                matches_agent_chat_wait_condition(&condition, &state, Default::default),
                Some(true)
            );
            assert_eq!(
                matches_agent_chat_wait_condition(
                    &condition,
                    &AgentChatStateSnapshot::default(),
                    Default::default
                ),
                Some(false)
            );
        }
        assert_eq!(
            matches_agent_chat_wait_condition(
                &WaitDetailedCondition::AgentChatSetupSelectedAgent {
                    agent_id: "other-agent".into()
                },
                &state,
                Default::default
            ),
            Some(false)
        );
    }

    #[test]
    fn chat_proof_waits_match_only_latest_acceptance_and_exact_layout() {
        use crate::protocol::{
            AgentChatInputLayoutTelemetry, AgentChatPickerItemAcceptedTelemetry,
            AgentChatTestProbeSnapshot,
        };
        let mut probe = AgentChatTestProbeSnapshot::default();
        for (label, key, cursor) in [("old", "enter", 2), ("latest", "tab", 9)] {
            probe
                .accepted_items
                .push(AgentChatPickerItemAcceptedTelemetry {
                    trigger: "@".into(),
                    item_label: label.into(),
                    item_id: label.into(),
                    accepted_via_key: key.into(),
                    cursor_after: cursor,
                    caused_submit: false,
                });
        }
        probe.input_layout = Some(AgentChatInputLayoutTelemetry {
            char_count: 20,
            visible_start: 4,
            visible_end: 12,
            cursor_in_window: 5,
        });
        for (condition, expected) in [
            (
                WaitDetailedCondition::AgentChatAcceptedLabel {
                    label: "old".into(),
                },
                false,
            ),
            (
                WaitDetailedCondition::AgentChatAcceptedLabel {
                    label: "latest".into(),
                },
                true,
            ),
            (
                WaitDetailedCondition::AgentChatAcceptedViaKey { key: "tab".into() },
                true,
            ),
            (
                WaitDetailedCondition::AgentChatAcceptedCursorAt { index: 9 },
                true,
            ),
            (
                WaitDetailedCondition::AgentChatInputLayoutMatch {
                    visible_start: 4,
                    visible_end: 12,
                    cursor_in_window: 5,
                },
                true,
            ),
            (
                WaitDetailedCondition::AgentChatInputLayoutMatch {
                    visible_start: 4,
                    visible_end: 12,
                    cursor_in_window: 6,
                },
                false,
            ),
        ] {
            assert_eq!(
                matches_agent_chat_wait_condition(&condition, &probe.state, || probe.clone()),
                Some(expected)
            );
        }
    }

    #[test]
    fn shared_command_errors_retain_code_and_suggestion_through_anyhow() {
        let expected = TransactionError::element_not_found("choice:2:missing");
        let error: anyhow::Error = expected.clone().into();
        assert_eq!(error.downcast::<TransactionError>().unwrap(), expected);
    }

    #[test]
    fn expected_empty_input_matches_none_and_some_empty_snapshots() {
        let spec = StateMatchSpec {
            input_value: Some(String::new()),
            ..StateMatchSpec::default()
        };

        for input_value in [None, Some(String::new())] {
            let snapshot = UiStateSnapshot {
                input_value,
                ..UiStateSnapshot::default()
            };
            assert!(matches_state_spec(&snapshot, &spec));
        }
    }

    #[test]
    fn trace_policy_never_overrides_the_current_privacy_mode() {
        for success in [true, false] {
            assert!(!should_include_trace(TransactionTraceMode::Off, success));
        }
        assert!(!should_include_trace(TransactionTraceMode::OnFailure, true));
        assert!(should_include_trace(TransactionTraceMode::OnFailure, false));
    }

    #[test]
    fn continued_batch_failures_preserve_the_first_failed_index() {
        let mut failed_at = None;
        record_first_batch_failure(&mut failed_at, 1);
        record_first_batch_failure(&mut failed_at, 3);
        assert_eq!(failed_at, Some(1));
    }

    #[test]
    fn unsupported_batch_command_is_present_in_its_replay_trace() {
        let command = BatchCommand::OpenActions;
        let error = TransactionError {
            code: TransactionErrorCode::UnsupportedCommand,
            message: "unsupported".to_owned(),
            suggestion: None,
        };
        let trace = unsupported_command_trace(4, &command, &error, UiStateSnapshot::default());
        assert_eq!(trace.index, 4);
        assert_eq!(trace.command, "openActions");
        assert_eq!(trace.command_payload, Some(command));
        assert_eq!(trace.error, Some(error));
    }

    #[test]
    fn transaction_provider_error_preserves_a_fingerprint_without_private_text() {
        let error = anyhow::anyhow!("provider rejected private-provider-error-canary");
        let message = safe_transaction_action_error("setInput", &error);
        assert!(message.contains("setInput failed"));
        assert!(message.contains("sha256:"));
        assert!(!message.contains("private-provider-error-canary"));
    }

    #[test]
    fn wait_suggestion_never_echoes_a_private_semantic_identifier() {
        let condition = WaitCondition::Detailed(WaitDetailedCondition::ElementExists {
            semantic_id: "private-semantic-canary".to_owned(),
        });
        let suggestion = build_wait_suggestion(&condition, &UiStateSnapshot::default())
            .expect("missing element has actionable guidance");
        assert!(suggestion.contains("requested element"));
        assert!(!suggestion.contains("private-semantic-canary"));
    }
}

fn unsupported_wait_condition(condition: &WaitCondition) -> Option<TransactionError> {
    match condition {
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupVisible)
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupReasonCode { .. })
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupPrimaryAction { .. })
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupAgentPickerOpen)
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupSelectedAgent { .. }) => {
            Some(TransactionError {
                code: TransactionErrorCode::InvalidCondition,
                message: format!("Wait condition is not wired to transaction runtime state: {condition:?}"),
                suggestion: Some(
                    "Use getAgentChatState/performAgentChatSetupAction for setup-card assertions until setup wait snapshots are supported."
                        .to_string(),
                ),
            })
        }
        _ => None,
    }
}

fn matches_condition<P: TransactionStateProvider>(
    provider: &P,
    snapshot: &UiStateSnapshot,
    condition: &WaitCondition,
) -> (bool, Vec<String>) {
    match condition {
        WaitCondition::Named(WaitNamedCondition::ChoicesRendered) => {
            (snapshot.choice_count > 0, Vec::new())
        }
        WaitCondition::Named(WaitNamedCondition::InputEmpty) => (
            snapshot.input_value.as_deref().unwrap_or("").is_empty(),
            Vec::new(),
        ),
        WaitCondition::Named(WaitNamedCondition::WindowVisible) => {
            (snapshot.window_visible, Vec::new())
        }
        WaitCondition::Named(WaitNamedCondition::WindowFocused) => {
            (snapshot.window_focused, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::ElementExists { semantic_id })
        | WaitCondition::Detailed(WaitDetailedCondition::ElementVisible { semantic_id }) => {
            let matched: Vec<String> = snapshot
                .visible_semantic_ids
                .iter()
                .filter(|id| *id == semantic_id)
                .cloned()
                .collect();
            (!matched.is_empty(), matched)
        }
        WaitCondition::Detailed(WaitDetailedCondition::ElementFocused { semantic_id }) => {
            let ok = snapshot.focused_semantic_id.as_deref() == Some(semantic_id.as_str());
            (
                ok,
                if ok {
                    vec![semantic_id.clone()]
                } else {
                    Vec::new()
                },
            )
        }
        WaitCondition::Detailed(WaitDetailedCondition::StateMatch { state }) => {
            (matches_state(snapshot, state), Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatReady) => {
            (snapshot.agent_chat_context_ready, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatPickerOpen) => {
            (snapshot.agent_chat_picker_open, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatPickerClosed) => {
            (!snapshot.agent_chat_picker_open, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatItemAccepted) => {
            let probe = provider.agent_chat_test_probe(1);
            (!probe.accepted_items.is_empty(), Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatCursorAt { index }) => {
            (snapshot.agent_chat_cursor_index == Some(*index), Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatStatus { status }) => (
            snapshot.agent_chat_status.as_deref() == Some(status.as_str()),
            Vec::new(),
        ),
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputMatch { text }) => (
            snapshot.input_value.as_deref() == Some(text.as_str()),
            Vec::new(),
        ),
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputContains { substring }) => (
            snapshot
                .input_value
                .as_deref()
                .is_some_and(|value| value.contains(substring)),
            Vec::new(),
        ),

        // ── Agent Chat proof conditions (evaluated against test probe) ──────
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedViaKey { key }) => {
            let probe = provider.agent_chat_test_probe(1);
            let ok = probe
                .accepted_items
                .last()
                .is_some_and(|item| item.accepted_via_key == *key);
            (ok, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedLabel { label }) => {
            let probe = provider.agent_chat_test_probe(1);
            let ok = probe
                .accepted_items
                .last()
                .is_some_and(|item| item.item_label == *label);
            (ok, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatAcceptedCursorAt { index }) => {
            let probe = provider.agent_chat_test_probe(1);
            let ok = probe
                .accepted_items
                .last()
                .is_some_and(|item| item.cursor_after == *index);
            (ok, Vec::new())
        }
        WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputLayoutMatch {
            visible_start,
            visible_end,
            cursor_in_window,
        }) => {
            let probe = provider.agent_chat_test_probe(1);
            let ok = probe.input_layout.as_ref().is_some_and(|layout| {
                layout.visible_start == *visible_start
                    && layout.visible_end == *visible_end
                    && layout.cursor_in_window == *cursor_in_window
            });
            (ok, Vec::new())
        }

        WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupVisible)
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupReasonCode { .. })
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupPrimaryAction { .. })
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupAgentPickerOpen)
        | WaitCondition::Detailed(WaitDetailedCondition::AgentChatSetupSelectedAgent { .. }) => {
            (false, Vec::new())
        }
    }
}

fn build_wait_suggestion(condition: &WaitCondition, snapshot: &UiStateSnapshot) -> Option<String> {
    match condition {
        WaitCondition::Named(WaitNamedCondition::ChoicesRendered) if snapshot.choice_count == 0 => {
            Some(
                "No choices were visible at timeout. Verify the preceding setInput \
                 changed the filter, or inspect getAccessibilityTree before selecting."
                    .to_string(),
            )
        }
        WaitCondition::Named(WaitNamedCondition::WindowFocused) if !snapshot.window_focused => {
            Some(
                "The window never became focused. Wait for windowVisible first, \
                 then retry windowFocused."
                    .to_string(),
            )
        }
        WaitCondition::Detailed(WaitDetailedCondition::ElementExists { semantic_id })
        | WaitCondition::Detailed(WaitDetailedCondition::ElementVisible { semantic_id })
            if !snapshot
                .visible_semantic_ids
                .iter()
                .any(|id| id == semantic_id) =>
        {
            Some(
                "The requested element was not visible at timeout. Inspect \
                 getAccessibilityTree or switch to stateMatch if the exact \
                 semanticId is unstable."
                    .to_owned(),
            )
        }
        WaitCondition::Detailed(WaitDetailedCondition::ElementFocused { semantic_id })
            if snapshot.focused_semantic_id.as_deref() != Some(semantic_id.as_str()) =>
        {
            Some(
                "The requested element never received focus. Add a focus action \
                 before waiting for elementFocused."
                    .to_owned(),
            )
        }
        _ => None,
    }
}

// ── Command name helper ────────────────────────────────────────────────────

fn command_name(command: &BatchCommand) -> &'static str {
    match command {
        BatchCommand::SetInput { .. } => "setInput",
        BatchCommand::OpenActions => "openActions",
        BatchCommand::TogglePreview => "togglePreview",
        BatchCommand::ForceSubmit { .. } => "forceSubmit",
        BatchCommand::WaitFor { .. } => "waitFor",
        BatchCommand::SelectByValue { .. } => "selectByValue",
        BatchCommand::SelectBySemanticId { .. } => "selectBySemanticId",
        BatchCommand::SetThemeControl { .. } => "setThemeControl",
        BatchCommand::UndoStyleChange => "undoStyleChange",
        BatchCommand::RedoStyleChange => "redoStyleChange",
        BatchCommand::ResetStyleControls => "resetStyleControls",
        BatchCommand::SaveCurrentStyleSettings => "saveCurrentStyleSettings",
        BatchCommand::FilterAndSelect { .. } => "filterAndSelect",
        BatchCommand::TypeAndSubmit { .. } => "typeAndSubmit",
    }
}

// ── Wait-for polling loop ──────────────────────────────────────────────────

struct WaitResult {
    success: bool,
    elapsed_ms: u64,
    error: Option<TransactionError>,
    trace: TransactionCommandTrace,
}

fn run_wait_for_command<P: TransactionStateProvider>(
    provider: &mut P,
    index: usize,
    condition: &WaitCondition,
    timeout: u64,
    poll_interval: u64,
) -> WaitResult {
    let started_at_ms = now_epoch_ms();
    let started = Instant::now();
    let before = provider.snapshot();
    let mut polls = Vec::new();

    if let Some(error) = unsupported_wait_condition(condition) {
        return WaitResult {
            success: false,
            elapsed_ms: 0,
            error: Some(error.clone()),
            trace: TransactionCommandTrace {
                index,
                command: "waitFor".to_string(),
                command_payload: None,
                started_at_ms,
                elapsed_ms: 0,
                before: before.clone(),
                after: before,
                polls,
                error: Some(error),
            },
        };
    }

    tracing::info!(
        target: "script_kit::transaction",
        index = index,
        timeout_ms = timeout,
        poll_interval_ms = poll_interval,
        "transaction_wait_start"
    );

    loop {
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let snapshot = provider.snapshot();
        let (ok, matched_ids) = matches_condition(provider, &snapshot, condition);

        polls.push(WaitPollObservation {
            attempt: polls.len() + 1,
            elapsed_ms,
            condition_satisfied: ok,
            snapshot: snapshot.clone(),
            matched_semantic_ids: matched_ids,
        });

        if ok {
            tracing::info!(
                target: "script_kit::transaction",
                index = index,
                elapsed_ms = elapsed_ms,
                "transaction_wait_complete"
            );
            return WaitResult {
                success: true,
                elapsed_ms,
                error: None,
                trace: TransactionCommandTrace {
                    index,
                    command: "waitFor".to_string(),
                    command_payload: None,
                    started_at_ms,
                    elapsed_ms,
                    before,
                    after: snapshot,
                    polls,
                    error: None,
                },
            };
        }

        if elapsed_ms >= timeout || polls.len() >= MAX_WAIT_POLLS {
            let error = TransactionError {
                code: TransactionErrorCode::WaitConditionTimeout,
                message: format!(
                    "Timeout after {timeout}ms waiting for the requested UI condition"
                ),
                suggestion: build_wait_suggestion(condition, &snapshot),
            };

            tracing::warn!(
                target: "script_kit::transaction",
                index = index,
                elapsed_ms = elapsed_ms,
                timeout_ms = timeout,
                condition_fingerprint = %transaction_content_fingerprint(&format!("{condition:?}")),
                "transaction_wait_timeout"
            );

            return WaitResult {
                success: false,
                elapsed_ms,
                error: Some(error.clone()),
                trace: TransactionCommandTrace {
                    index,
                    command: "waitFor".to_string(),
                    command_payload: None,
                    started_at_ms,
                    elapsed_ms,
                    before,
                    after: snapshot,
                    polls,
                    error: Some(error),
                },
            };
        }

        std::thread::sleep(Duration::from_millis(
            poll_interval.max(1).min(timeout.saturating_sub(elapsed_ms)),
        ));
    }
}

pub fn stable_transaction_fingerprint(
    commands: &[BatchCommand],
    options: Option<&BatchOptions>,
) -> Result<String> {
    let payload = serde_json::json!({
        "commands": commands,
        "options": options,
    });
    Ok(transaction_content_fingerprint(&serde_json::to_string(
        &payload,
    )?))
}

/// A runtime transaction is scoped to one process session and exact target lifetime.
/// Persisted traces remain history, never an authority to skip a live observation.
pub fn scoped_transaction_fingerprint(
    commands: &[BatchCommand],
    options: Option<&BatchOptions>,
    target: &crate::protocol::AutomationWindowInfo,
    session_id: &str,
) -> Result<String> {
    let payload = serde_json::json!({
        "commands": commands, "options": options, "sessionId": session_id,
        "target": {"id":target.id,"generation":target.generation,"kind":target.kind,
            "parentId":target.parent_window_id,"parentGeneration":target.parent_window_generation},
    });
    Ok(transaction_content_fingerprint(&serde_json::to_string(
        &payload,
    )?))
}

pub fn stable_wait_fingerprint(
    condition: &WaitCondition,
    timeout: u64,
    poll_interval: u64,
) -> Result<String> {
    let payload = serde_json::json!({
        "condition": condition,
        "timeout": timeout,
        "pollInterval": poll_interval,
    });
    Ok(transaction_content_fingerprint(&serde_json::to_string(
        &payload,
    )?))
}

impl BatchOutput {
    pub fn from_trace(trace: TransactionTrace) -> Self {
        let trace = sanitize_transaction_trace(&trace);
        let success = trace.status == TransactionTraceStatus::Ok;
        Self {
            request_id: trace.request_id.clone(),
            success,
            results: trace
                .commands
                .iter()
                .map(|command| BatchResultEntry {
                    index: command.index,
                    success: command.error.is_none(),
                    command: command.command.clone(),
                    elapsed: Some(command.elapsed_ms),
                    value: if command.error.is_none() {
                        restore_persisted_transaction_result(
                            &trace.request_id,
                            &trace.command_fingerprint,
                            command.index,
                        )
                    } else {
                        None
                    },
                    error: command.error.clone(),
                })
                .collect(),
            failed_at: trace.failed_at,
            total_elapsed: trace.total_elapsed_ms,
            trace: Some(trace),
        }
    }
}

fn record_first_batch_failure(failed_at: &mut Option<usize>, index: usize) {
    failed_at.get_or_insert(index);
}

fn unsupported_command_trace(
    index: usize,
    command: &BatchCommand,
    error: &TransactionError,
    snapshot: UiStateSnapshot,
) -> TransactionCommandTrace {
    TransactionCommandTrace {
        index,
        command: command_name(command).to_owned(),
        command_payload: Some(command.clone()),
        started_at_ms: now_epoch_ms(),
        elapsed_ms: 0,
        before: snapshot.clone(),
        after: snapshot,
        polls: Vec::new(),
        error: Some(error.clone()),
    }
}

fn safe_transaction_action_error(action: &str, error: &anyhow::Error) -> String {
    let fingerprint = transaction_content_fingerprint(&error.to_string());
    format!("{action} failed (diagnostic {fingerprint})")
}

// ── Trace persistence helper ───────────────────────────────────────────────

fn maybe_persist_trace(
    mode: TransactionTraceMode,
    success: bool,
    trace: &TransactionTrace,
    log_path: Option<&Path>,
) -> Result<bool> {
    if !should_include_trace(mode, success) {
        return Ok(false);
    }
    append_transaction_trace(log_path, trace)?;
    Ok(true)
}

// ── Public executor entry points ───────────────────────────────────────────

/// Result of executing a single `waitFor` command.
pub struct WaitForOutput {
    pub request_id: String,
    pub success: bool,
    pub elapsed: u64,
    pub error: Option<TransactionError>,
    pub trace: Option<TransactionTrace>,
}

/// Execute a standalone `waitFor` command.
pub fn execute_wait_for<P: TransactionStateProvider>(
    provider: &mut P,
    request_id: String,
    condition: &WaitCondition,
    timeout: Option<u64>,
    poll_interval: Option<u64>,
    trace_mode: TransactionTraceMode,
) -> Result<WaitForOutput> {
    let timeout = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
    anyhow::ensure!(timeout <= 600_000, "wait_deadline_invalid");
    let poll_interval = poll_interval
        .unwrap_or(DEFAULT_WAIT_POLL_INTERVAL_MS)
        .clamp(1, 1_000);
    let command_fingerprint = stable_wait_fingerprint(condition, timeout, poll_interval)?;

    let result = run_wait_for_command(provider, 0, condition, timeout, poll_interval);

    let trace = TransactionTrace {
        schema_version: crate::protocol::types::batch_wait::TRANSACTION_TRACE_SCHEMA_VERSION,
        request_id: request_id.clone(),
        command_fingerprint,
        status: if result.success {
            TransactionTraceStatus::Ok
        } else {
            TransactionTraceStatus::Timeout
        },
        started_at_ms: result.trace.started_at_ms,
        total_elapsed_ms: result.elapsed_ms,
        failed_at: if result.success { None } else { Some(0) },
        commands: vec![result.trace],
    };

    let include_trace = maybe_persist_trace(trace_mode, result.success, &trace, None)?;

    Ok(WaitForOutput {
        request_id,
        success: result.success,
        elapsed: result.elapsed_ms,
        error: result.error,
        trace: include_trace.then(|| sanitize_transaction_trace(&trace)),
    })
}

/// Result of executing a `batch` command.
pub struct BatchOutput {
    pub request_id: String,
    pub success: bool,
    pub results: Vec<BatchResultEntry>,
    pub failed_at: Option<usize>,
    pub total_elapsed: u64,
    pub trace: Option<TransactionTrace>,
}

/// Execute a batch of commands as a transaction.
pub fn execute_batch<P: TransactionStateProvider>(
    provider: &mut P,
    request_id: String,
    commands: &[BatchCommand],
    options: Option<&BatchOptions>,
    trace_mode: TransactionTraceMode,
) -> Result<BatchOutput> {
    let stop_on_error = options.is_none_or(|o| o.stop_on_error);
    anyhow::ensure!(
        commands.len() <= MAX_BATCH_COMMANDS,
        "batch_command_budget_exhausted"
    );
    anyhow::ensure!(
        !options.is_some_and(|options| options.rollback_on_error),
        "batch_rollback_unsupported"
    );
    let timeout =
        Duration::from_millis(options.map_or(DEFAULT_WAIT_TIMEOUT_MS, |options| options.timeout));
    anyhow::ensure!(
        !timeout.is_zero() && timeout <= Duration::from_millis(600_000),
        "batch_deadline_invalid"
    );
    let command_fingerprint = stable_transaction_fingerprint(commands, options)?;
    let started_at_ms = now_epoch_ms();
    let started = Instant::now();
    let mut results = Vec::new();
    let mut command_traces = Vec::new();
    let mut failed_at: Option<usize> = None;

    tracing::info!(
        target: "script_kit::transaction",
        request_id = %request_id,
        command_count = commands.len(),
        stop_on_error = stop_on_error,
        "transaction_batch_start"
    );

    for (index, command) in commands.iter().enumerate() {
        if started.elapsed() >= timeout {
            let error = TransactionError::wait_timeout("Batch timeout exceeded");
            results.push(BatchResultEntry {
                index,
                success: false,
                command: command_name(command).into(),
                elapsed: Some(0),
                value: None,
                error: Some(error.clone()),
            });
            command_traces.push(unsupported_command_trace(
                index,
                command,
                &error,
                provider.snapshot(),
            ));
            record_first_batch_failure(&mut failed_at, index);
            break;
        }
        match command {
            BatchCommand::SetInput { text } => {
                let cmd_started_at = now_epoch_ms();
                let cmd_started = Instant::now();
                let before = provider.snapshot();
                let mut error = None;

                let success = match provider.set_input(text) {
                    Ok(()) => true,
                    Err(e) => {
                        error = Some(TransactionError {
                            code: TransactionErrorCode::ActionFailed,
                            message: safe_transaction_action_error("setInput", &e),
                            suggestion: Some(
                                "Verify the active prompt exposes a writable input \
                                 field before issuing setInput."
                                    .to_string(),
                            ),
                        });
                        false
                    }
                };

                let elapsed_ms = cmd_started.elapsed().as_millis() as u64;
                let after = provider.snapshot();

                results.push(BatchResultEntry {
                    index,
                    success,
                    command: command_name(command).to_string(),
                    elapsed: Some(elapsed_ms),
                    value: None,
                    error: error.clone(),
                });
                command_traces.push(TransactionCommandTrace {
                    index,
                    command: command_name(command).to_string(),
                    command_payload: Some(command.clone()),
                    started_at_ms: cmd_started_at,
                    elapsed_ms,
                    before,
                    after,
                    polls: Vec::new(),
                    error,
                });

                if !success {
                    record_first_batch_failure(&mut failed_at, index);
                    if stop_on_error {
                        break;
                    }
                }
            }

            BatchCommand::WaitFor {
                condition,
                timeout,
                poll_interval,
            } => {
                let t = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS).min(
                    options
                        .map_or(DEFAULT_WAIT_TIMEOUT_MS, |options| options.timeout)
                        .saturating_sub(started.elapsed().as_millis() as u64),
                );
                let pi = poll_interval.unwrap_or(DEFAULT_WAIT_POLL_INTERVAL_MS);

                let wr = run_wait_for_command(provider, index, condition, t, pi);

                results.push(BatchResultEntry {
                    index,
                    success: wr.success,
                    command: command_name(command).to_string(),
                    elapsed: Some(wr.elapsed_ms),
                    value: None,
                    error: wr.error.clone(),
                });
                let mut trace = wr.trace;
                trace.command_payload = Some(command.clone());
                command_traces.push(trace);

                if !wr.success {
                    record_first_batch_failure(&mut failed_at, index);
                    if stop_on_error {
                        break;
                    }
                }
            }

            BatchCommand::SelectByValue { value, submit } => {
                let cmd_started_at = now_epoch_ms();
                let cmd_started = Instant::now();
                let before = provider.snapshot();
                let mut error = None;

                let selected = match provider.select_by_value(value, *submit) {
                    Ok(Some(v)) => Some(v),
                    Ok(None) => {
                        error = Some(TransactionError {
                            code: TransactionErrorCode::SelectionNotFound,
                            message: format!("selectByValue could not find value '{value}'"),
                            suggestion: Some(
                                "Run waitFor choicesRendered before selecting, or \
                                 inspect getAccessibilityTree to confirm the value \
                                 is present."
                                    .to_string(),
                            ),
                        });
                        None
                    }
                    Err(e) => {
                        error = Some(TransactionError {
                            code: TransactionErrorCode::ActionFailed,
                            message: safe_transaction_action_error("selectByValue", &e),
                            suggestion: Some(
                                "Verify the current choice is selectable and the \
                                 window is focused before selecting."
                                    .to_string(),
                            ),
                        });
                        None
                    }
                };

                let elapsed_ms = cmd_started.elapsed().as_millis() as u64;
                let after = provider.snapshot();
                let success = error.is_none();

                results.push(BatchResultEntry {
                    index,
                    success,
                    command: command_name(command).to_string(),
                    elapsed: Some(elapsed_ms),
                    value: selected,
                    error: error.clone(),
                });
                command_traces.push(TransactionCommandTrace {
                    index,
                    command: command_name(command).to_string(),
                    command_payload: Some(command.clone()),
                    started_at_ms: cmd_started_at,
                    elapsed_ms,
                    before,
                    after,
                    polls: Vec::new(),
                    error,
                });

                if !success {
                    record_first_batch_failure(&mut failed_at, index);
                    if stop_on_error {
                        break;
                    }
                }
            }

            BatchCommand::SelectBySemanticId {
                semantic_id,
                submit,
            } => {
                let cmd_started_at = now_epoch_ms();
                let cmd_started = Instant::now();
                let before = provider.snapshot();
                let mut error = None;

                let selected = match provider.select_by_semantic_id(semantic_id, *submit) {
                    Ok(Some(v)) => Some(v),
                    Ok(None) => {
                        error = Some(TransactionError {
                            code: TransactionErrorCode::SelectionNotFound,
                            message: format!("selectBySemanticId could not find '{semantic_id}'"),
                            suggestion: Some(
                                "Run getElements to discover visible semantic IDs, \
                                 or waitFor elementExists before selecting."
                                    .to_string(),
                            ),
                        });
                        None
                    }
                    Err(e) => {
                        error = Some(TransactionError {
                            code: TransactionErrorCode::ActionFailed,
                            message: safe_transaction_action_error("selectBySemanticId", &e),
                            suggestion: Some(
                                "Verify the current view supports element selection \
                                 and the window is focused."
                                    .to_string(),
                            ),
                        });
                        None
                    }
                };

                let elapsed_ms = cmd_started.elapsed().as_millis() as u64;
                let after = provider.snapshot();
                let success = error.is_none();

                results.push(BatchResultEntry {
                    index,
                    success,
                    command: command_name(command).to_string(),
                    elapsed: Some(elapsed_ms),
                    value: selected,
                    error: error.clone(),
                });
                command_traces.push(TransactionCommandTrace {
                    index,
                    command: command_name(command).to_string(),
                    command_payload: Some(command.clone()),
                    started_at_ms: cmd_started_at,
                    elapsed_ms,
                    before,
                    after,
                    polls: Vec::new(),
                    error,
                });

                if !success {
                    record_first_batch_failure(&mut failed_at, index);
                    if stop_on_error {
                        break;
                    }
                }
            }

            BatchCommand::ForceSubmit { .. }
            | BatchCommand::OpenActions
            | BatchCommand::TogglePreview
            | BatchCommand::SetThemeControl { .. }
            | BatchCommand::UndoStyleChange
            | BatchCommand::RedoStyleChange
            | BatchCommand::ResetStyleControls
            | BatchCommand::SaveCurrentStyleSettings
            | BatchCommand::FilterAndSelect { .. }
            | BatchCommand::TypeAndSubmit { .. } => {
                // These compound commands are not yet wired to the executor.
                // Record as unsupported so the caller gets a clear signal.
                // `setThemeControl` is an app-owned DevTools runtime command
                // handled by ScriptListApp's batch dispatch because it mutates
                // live ThemeChooser GPUI state.
                let error = TransactionError {
                    code: TransactionErrorCode::UnsupportedCommand,
                    message: format!(
                        "{} is not yet supported by the transaction executor",
                        command_name(command)
                    ),
                    suggestion: Some(
                        "Use the equivalent primitive commands (setInput + waitFor \
                         + selectByValue) instead."
                            .to_string(),
                    ),
                };
                let snapshot = provider.snapshot();
                command_traces.push(unsupported_command_trace(index, command, &error, snapshot));
                results.push(BatchResultEntry {
                    index,
                    success: false,
                    command: command_name(command).to_string(),
                    elapsed: Some(0),
                    value: None,
                    error: Some(error),
                });
                record_first_batch_failure(&mut failed_at, index);
                if stop_on_error {
                    break;
                }
            }
        }
    }

    let success = failed_at.is_none();
    let total_elapsed_ms = started.elapsed().as_millis() as u64;

    tracing::info!(
        target: "script_kit::transaction",
        request_id = %request_id,
        success = success,
        total_elapsed_ms = total_elapsed_ms,
        failed_at = ?failed_at,
        "transaction_batch_complete"
    );

    let trace = TransactionTrace {
        schema_version: crate::protocol::types::batch_wait::TRANSACTION_TRACE_SCHEMA_VERSION,
        request_id: request_id.clone(),
        command_fingerprint,
        status: if success {
            TransactionTraceStatus::Ok
        } else {
            TransactionTraceStatus::Failed
        },
        started_at_ms,
        total_elapsed_ms,
        failed_at,
        commands: command_traces,
    };

    let include_trace = maybe_persist_trace(trace_mode, success, &trace, None)?;
    if include_trace {
        remember_persisted_transaction_results(
            &trace.request_id,
            &trace.command_fingerprint,
            &results,
        );
    }

    Ok(BatchOutput {
        request_id,
        success,
        results,
        failed_at,
        total_elapsed: total_elapsed_ms,
        trace: include_trace.then(|| sanitize_transaction_trace(&trace)),
    })
}
