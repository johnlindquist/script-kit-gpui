//! Transaction flight recorder executor for `waitFor` and `batch` commands.
//!
//! Executes deterministic UI transactions against a [`TransactionStateProvider`],
//! producing per-command receipts with before/after snapshots, poll observations,
//! elapsed timings, and actionable failure suggestions.

use crate::protocol::transaction_trace::{
    append_transaction_trace, now_epoch_ms, read_latest_transaction_trace, should_include_trace,
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
            Some(format!(
                "Element '{semantic_id}' was not visible at timeout. Inspect \
                 getAccessibilityTree or switch to stateMatch if the exact \
                 semanticId is unstable."
            ))
        }
        WaitCondition::Detailed(WaitDetailedCondition::ElementFocused { semantic_id })
            if snapshot.focused_semantic_id.as_deref() != Some(semantic_id.as_str()) =>
        {
            Some(format!(
                "Element '{semantic_id}' never received focus. Add a focus action \
                 before waiting for elementFocused."
            ))
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

        if elapsed_ms >= timeout {
            let error = TransactionError {
                code: TransactionErrorCode::WaitConditionTimeout,
                message: format!("Timeout after {timeout}ms waiting for {condition:?}"),
                suggestion: build_wait_suggestion(condition, &snapshot),
            };

            tracing::warn!(
                target: "script_kit::transaction",
                index = index,
                elapsed_ms = elapsed_ms,
                message = %error.message,
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

        std::thread::sleep(Duration::from_millis(poll_interval.max(1)));
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
    Ok(serde_json::to_string(&payload)?)
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
    Ok(serde_json::to_string(&payload)?)
}

impl BatchOutput {
    pub fn from_trace(trace: TransactionTrace) -> Self {
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
                    value: None,
                    error: command.error.clone(),
                })
                .collect(),
            failed_at: trace.failed_at,
            total_elapsed: trace.total_elapsed_ms,
            trace: Some(trace),
        }
    }
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
    let poll_interval = poll_interval.unwrap_or(DEFAULT_WAIT_POLL_INTERVAL_MS);
    let command_fingerprint = stable_wait_fingerprint(condition, timeout, poll_interval)?;

    if let Some(existing) = read_latest_transaction_trace(None, Some(&request_id))? {
        if existing.command_fingerprint.is_empty() {
            tracing::warn!(
                target: "script_kit::transaction",
                request_id = %request_id,
                "Ignoring legacy transaction trace without fingerprint"
            );
        } else if existing.command_fingerprint == command_fingerprint {
            let success = existing.status == TransactionTraceStatus::Ok;
            return Ok(WaitForOutput {
                request_id,
                success,
                elapsed: existing.total_elapsed_ms,
                error: existing
                    .commands
                    .iter()
                    .find_map(|command| command.error.clone()),
                trace: Some(existing),
            });
        } else {
            return Err(anyhow::anyhow!(
                "requestId {request_id} was already used for a different transaction payload"
            ));
        }
    }

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
        trace: if include_trace { Some(trace) } else { None },
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
    let command_fingerprint = stable_transaction_fingerprint(commands, options)?;
    if let Some(existing) = read_latest_transaction_trace(None, Some(&request_id))? {
        if existing.command_fingerprint.is_empty() {
            tracing::warn!(
                target: "script_kit::transaction",
                request_id = %request_id,
                "Ignoring legacy transaction trace without fingerprint"
            );
        } else if existing.command_fingerprint == command_fingerprint {
            return Ok(BatchOutput::from_trace(existing));
        } else {
            return Err(anyhow::anyhow!(
                "requestId {request_id} was already used for a different transaction payload"
            ));
        }
    }
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
                            message: format!("setInput failed: {e}"),
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
                    failed_at = Some(index);
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
                let t = timeout.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
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
                    failed_at = Some(index);
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
                            message: format!("selectByValue failed: {e}"),
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
                    failed_at = Some(index);
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
                            message: format!("selectBySemanticId failed: {e}"),
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
                    failed_at = Some(index);
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
                results.push(BatchResultEntry {
                    index,
                    success: false,
                    command: command_name(command).to_string(),
                    elapsed: Some(0),
                    value: None,
                    error: Some(error),
                });
                failed_at = Some(index);
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

    Ok(BatchOutput {
        request_id,
        success,
        results,
        failed_at,
        total_elapsed: total_elapsed_ms,
        trace: if include_trace { Some(trace) } else { None },
    })
}
