//! JSONL persistence layer for transaction flight recorder traces.
//!
//! Appends serialized `TransactionTrace` records to an append-only JSONL file
//! and reads them back for inspection by agents and diagnostic tooling.

use crate::protocol::types::batch_wait::{
    BatchCommand, BatchResultEntry, TransactionCommandTrace, TransactionTrace,
    TransactionTraceMode, TransactionTraceStatus, UiStateSnapshot, WaitCondition,
    WaitDetailedCondition, TRANSACTION_TRACE_SCHEMA_VERSION,
};
use anyhow::{anyhow, Context, Result};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TRANSACTION_TRACE_MAX_BYTES: u64 = 10 * 1024 * 1024;
const TRANSACTION_TRACE_COMPACT_KEEP: usize = 2_000;
const PRIVATE_VALUE_PREFIX: &str = "[REDACTED sha256:";
const PRIVATE_TRANSACTION_RESULT_TTL: Duration = Duration::from_secs(120);
const PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES: usize = 128;
const PRIVATE_TRANSACTION_RESULT_MAX_BYTES: usize = 256 * 1024;
const PRIVATE_TRANSACTION_RESULT_MAX_VALUE_BYTES: usize = 32 * 1024;
const PRIVATE_TRANSACTION_RESULT_MAX_REQUEST_ID_BYTES: usize = 512;

/// Successful selection values exist only in this short-lived process-local
/// vault. Nothing implements serialization or Debug: persisted traces, logs,
/// and cold-process replay can never recover another session's private value.
struct PrivateTransactionResult {
    request_id: String,
    command_fingerprint: String,
    result_index: usize,
    value: String,
    expires_at: Instant,
}

impl PrivateTransactionResult {
    fn retained_bytes(&self) -> usize {
        self.request_id.len()
            + self.command_fingerprint.len()
            + std::mem::size_of::<usize>()
            + self.value.len()
    }

    fn matches(&self, request_id: &str, command_fingerprint: &str, result_index: usize) -> bool {
        self.request_id == request_id
            && self.command_fingerprint == command_fingerprint
            && self.result_index == result_index
    }
}

#[derive(Default)]
struct PrivateTransactionResultVault {
    entries: VecDeque<PrivateTransactionResult>,
    retained_bytes: usize,
}

impl PrivateTransactionResultVault {
    fn prune_expired(&mut self, now: Instant) {
        self.entries.retain(|entry| entry.expires_at > now);
        self.retained_bytes = self
            .entries
            .iter()
            .map(PrivateTransactionResult::retained_bytes)
            .sum();
    }

    fn remember(
        &mut self,
        request_id: &str,
        command_fingerprint: &str,
        results: &[BatchResultEntry],
        now: Instant,
    ) {
        self.prune_expired(now);
        if request_id.is_empty()
            || request_id.len() > PRIVATE_TRANSACTION_RESULT_MAX_REQUEST_ID_BYTES
            || !is_content_fingerprint(command_fingerprint)
        {
            return;
        }
        let Some(expires_at) = now.checked_add(PRIVATE_TRANSACTION_RESULT_TTL) else {
            return;
        };

        for result in results {
            if !result.success {
                continue;
            }
            let Some(value) = result.value.as_deref().filter(|value| {
                !value.is_empty() && value.len() <= PRIVATE_TRANSACTION_RESULT_MAX_VALUE_BYTES
            }) else {
                continue;
            };

            if let Some(index) = self
                .entries
                .iter()
                .position(|entry| entry.matches(request_id, command_fingerprint, result.index))
            {
                if let Some(previous) = self.entries.remove(index) {
                    self.retained_bytes = self
                        .retained_bytes
                        .saturating_sub(previous.retained_bytes());
                }
            }

            let entry = PrivateTransactionResult {
                request_id: request_id.to_owned(),
                command_fingerprint: command_fingerprint.to_owned(),
                result_index: result.index,
                value: value.to_owned(),
                expires_at,
            };
            let retained_bytes = entry.retained_bytes();
            if retained_bytes > PRIVATE_TRANSACTION_RESULT_MAX_BYTES {
                continue;
            }
            while self.entries.len() >= PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES
                || self.retained_bytes.saturating_add(retained_bytes)
                    > PRIVATE_TRANSACTION_RESULT_MAX_BYTES
            {
                let Some(expired) = self.entries.pop_front() else {
                    break;
                };
                self.retained_bytes = self.retained_bytes.saturating_sub(expired.retained_bytes());
            }
            self.retained_bytes += retained_bytes;
            self.entries.push_back(entry);
        }
    }

    fn restore(
        &mut self,
        request_id: &str,
        command_fingerprint: &str,
        result_index: usize,
        now: Instant,
    ) -> Option<String> {
        self.prune_expired(now);
        if !is_content_fingerprint(command_fingerprint) {
            return None;
        }
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.matches(request_id, command_fingerprint, result_index))
            .map(|entry| entry.value.clone())
    }
}

fn private_transaction_result_vault() -> &'static Mutex<PrivateTransactionResultVault> {
    static VAULT: OnceLock<Mutex<PrivateTransactionResultVault>> = OnceLock::new();
    VAULT.get_or_init(|| Mutex::new(PrivateTransactionResultVault::default()))
}

/// Publish only after the owning trace has successfully reached its safe log.
pub(crate) fn remember_persisted_transaction_results(
    request_id: &str,
    command_fingerprint: &str,
    results: &[BatchResultEntry],
) {
    let Ok(mut vault) = private_transaction_result_vault().lock() else {
        return;
    };
    vault.remember(request_id, command_fingerprint, results, Instant::now());
}

pub(crate) fn restore_persisted_transaction_result(
    request_id: &str,
    command_fingerprint: &str,
    result_index: usize,
) -> Option<String> {
    let Ok(mut vault) = private_transaction_result_vault().lock() else {
        return None;
    };
    vault.restore(
        request_id,
        command_fingerprint,
        result_index,
        Instant::now(),
    )
}

pub(crate) fn transaction_content_fingerprint(value: &str) -> String {
    format!(
        "sha256:{}",
        crate::logging::log_private_user_value(value).sha256
    )
}

fn is_content_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn private_value(value: &str) -> String {
    if let Some(body) = value
        .strip_prefix(PRIVATE_VALUE_PREFIX)
        .and_then(|body| body.strip_suffix(']'))
    {
        if let Some((digest, bytes)) = body.split_once(" bytes:") {
            if digest.len() == 64
                && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                && bytes.parse::<usize>().is_ok()
            {
                return value.to_owned();
            }
        }
    }

    format!(
        "{PRIVATE_VALUE_PREFIX}{} bytes:{}]",
        crate::logging::log_private_user_value(value).sha256,
        value.len()
    )
}

fn sanitize_snapshot(snapshot: &mut UiStateSnapshot) {
    for value in [
        &mut snapshot.input_value,
        &mut snapshot.selected_value,
        &mut snapshot.focused_semantic_id,
    ]
    .into_iter()
    .flatten()
    {
        *value = private_value(value);
    }
    for semantic_id in &mut snapshot.visible_semantic_ids {
        *semantic_id = private_value(semantic_id);
    }
}

fn sanitize_wait_condition(condition: &mut WaitCondition) {
    let WaitCondition::Detailed(condition) = condition else {
        return;
    };

    match condition {
        WaitDetailedCondition::ElementExists { semantic_id }
        | WaitDetailedCondition::ElementVisible { semantic_id }
        | WaitDetailedCondition::ElementFocused { semantic_id } => {
            *semantic_id = private_value(semantic_id);
        }
        WaitDetailedCondition::StateMatch { state } => {
            for value in [&mut state.input_value, &mut state.selected_value]
                .into_iter()
                .flatten()
            {
                *value = private_value(value);
            }
        }
        WaitDetailedCondition::AgentChatStatus { status: value }
        | WaitDetailedCondition::AgentChatInputMatch { text: value }
        | WaitDetailedCondition::AgentChatInputContains { substring: value }
        | WaitDetailedCondition::AgentChatAcceptedViaKey { key: value }
        | WaitDetailedCondition::AgentChatAcceptedLabel { label: value }
        | WaitDetailedCondition::AgentChatSetupReasonCode { reason_code: value }
        | WaitDetailedCondition::AgentChatSetupSelectedAgent { agent_id: value } => {
            *value = private_value(value);
        }
        WaitDetailedCondition::AgentChatReady
        | WaitDetailedCondition::AgentChatPickerOpen
        | WaitDetailedCondition::AgentChatPickerClosed
        | WaitDetailedCondition::AgentChatItemAccepted
        | WaitDetailedCondition::AgentChatCursorAt { .. }
        | WaitDetailedCondition::AgentChatAcceptedCursorAt { .. }
        | WaitDetailedCondition::AgentChatInputLayoutMatch { .. }
        | WaitDetailedCondition::AgentChatSetupVisible
        | WaitDetailedCondition::AgentChatSetupPrimaryAction { .. }
        | WaitDetailedCondition::AgentChatSetupAgentPickerOpen => {}
    }
}

fn sanitize_command_payload(command: &mut BatchCommand) {
    match command {
        BatchCommand::SetInput { text: value }
        | BatchCommand::TypeAndSubmit { text: value }
        | BatchCommand::SelectByValue { value, .. }
        | BatchCommand::SelectBySemanticId {
            semantic_id: value, ..
        }
        | BatchCommand::FilterAndSelect { filter: value, .. } => {
            *value = private_value(value);
        }
        BatchCommand::ForceSubmit { value } => {
            if let serde_json::Value::String(existing) = value {
                if private_value(existing) == *existing {
                    return;
                }
            }
            if let Ok(serialized) = serde_json::to_string(value) {
                *value = serde_json::Value::String(private_value(&serialized));
            } else {
                *value = serde_json::Value::String("[REDACTED]".to_owned());
            }
        }
        BatchCommand::WaitFor { condition, .. } => sanitize_wait_condition(condition),
        BatchCommand::SetThemeControl { control, value } => {
            *control = private_value(control);
            *value = private_value(value);
        }
        BatchCommand::OpenActions
        | BatchCommand::TogglePreview
        | BatchCommand::UndoStyleChange
        | BatchCommand::RedoStyleChange
        | BatchCommand::ResetStyleControls
        | BatchCommand::SaveCurrentStyleSettings => {}
    }
}

fn sanitize_error(error: &mut crate::protocol::types::batch_wait::TransactionError) {
    use crate::protocol::types::batch_wait::TransactionErrorCode;

    let (message, suggestion) = match &error.code {
        TransactionErrorCode::WaitConditionTimeout => (
            "The requested UI condition was not met before the timeout.",
            "Refresh the UI state and retry with an appropriate timeout.",
        ),
        TransactionErrorCode::ElementNotFound => (
            "The requested UI element could not be found.",
            "Inspect the available elements and retry with a current identifier.",
        ),
        TransactionErrorCode::SelectionNotFound => (
            "The requested selection could not be found.",
            "Refresh the available choices and select a current entry.",
        ),
        TransactionErrorCode::InvalidCondition => (
            "The requested wait condition is invalid.",
            "Check the supported condition schema before retrying.",
        ),
        TransactionErrorCode::RequestIdConflict => (
            "The request identifier conflicts with an existing transaction.",
            "Retry with a fresh request identifier.",
        ),
        TransactionErrorCode::UnsupportedCommand => (
            "The current surface does not support the requested command.",
            "Inspect the surface capabilities and choose a supported command.",
        ),
        TransactionErrorCode::UnsupportedPrompt => (
            "The current prompt does not support the requested action.",
            "Inspect the current prompt type before retrying.",
        ),
        TransactionErrorCode::ActionFailed => (
            "The requested UI action could not be completed.",
            "Refresh the current UI state and retry the action.",
        ),
    };
    error.message = message.to_owned();
    error.suggestion = error.suggestion.as_ref().map(|_| suggestion.to_owned());
}

/// Remove user-controlled payloads and snapshots before traces leave memory.
///
/// Both persistence and resource readers apply this projection so historical
/// unsanitized traces cannot leak when exposed through MCP.
pub(crate) fn sanitize_transaction_trace(trace: &TransactionTrace) -> TransactionTrace {
    let mut sanitized = trace.clone();
    if !sanitized.command_fingerprint.is_empty()
        && !is_content_fingerprint(&sanitized.command_fingerprint)
    {
        sanitized.command_fingerprint =
            transaction_content_fingerprint(&sanitized.command_fingerprint);
    }

    for command in &mut sanitized.commands {
        if let Some(payload) = &mut command.command_payload {
            sanitize_command_payload(payload);
        }
        sanitize_snapshot(&mut command.before);
        sanitize_snapshot(&mut command.after);
        for poll in &mut command.polls {
            sanitize_snapshot(&mut poll.snapshot);
            for semantic_id in &mut poll.matched_semantic_ids {
                *semantic_id = private_value(semantic_id);
            }
        }
        if let Some(error) = &mut command.error {
            sanitize_error(error);
        }
    }

    sanitized
}

fn trace_log_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Returns the current epoch time in milliseconds.
pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Returns the default path for transaction trace logs.
pub fn default_transaction_log_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".scriptkit")
        .join("logs")
        .join("transactions.jsonl")
}

/// Append a single transaction trace to the JSONL log file.
///
/// Creates parent directories if they don't exist. Returns the path written to.
pub fn append_transaction_trace(path: Option<&Path>, trace: &TransactionTrace) -> Result<PathBuf> {
    let _guard = trace_log_mutex()
        .lock()
        .map_err(|_| anyhow!("trace log mutex poisoned"))?;
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(default_transaction_log_path);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    compact_transaction_trace_log_if_needed(&path)?;

    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    ensure_jsonl_append_boundary(&mut file)?;

    let sanitized = sanitize_transaction_trace(trace);
    let line =
        serde_json::to_string(&sanitized).context("failed to serialize transaction trace")?;
    writeln!(file, "{line}").context("failed to write transaction trace")?;

    tracing::info!(
        target: "script_kit::transaction",
        log_path = %path.display(),
        request_id = %trace.request_id,
        status = ?trace.status,
        "transaction_trace_persisted"
    );

    Ok(path)
}

fn ensure_jsonl_append_boundary(file: &mut fs::File) -> Result<()> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    file.seek(SeekFrom::End(0))?;
    if last[0] != b'\n' {
        writeln!(file)?;
    }
    Ok(())
}

pub fn compact_transaction_trace_log_if_needed(path: &Path) -> Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to stat {}", path.display()));
        }
    };
    if metadata.len() <= TRANSACTION_TRACE_MAX_BYTES {
        return Ok(());
    }

    let file = OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut traces = Vec::new();
    for line in reader.lines() {
        let line = line.context("failed to read transaction trace log during compaction")?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<TransactionTrace>(&line) {
            Ok(trace) => traces.push(trace),
            Err(error) => tracing::warn!(
                target: "script_kit::transaction",
                log_path = %path.display(),
                %error,
                "Skipping malformed transaction trace log entry during compaction"
            ),
        }
    }
    let start = traces.len().saturating_sub(TRANSACTION_TRACE_COMPACT_KEEP);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("failed to compact {}", path.display()))?;
    for trace in traces.into_iter().skip(start) {
        let line = serde_json::to_string(&sanitize_transaction_trace(&trace))
            .context("failed to serialize compacted trace")?;
        writeln!(file, "{line}").context("failed to write compacted transaction trace")?;
    }
    Ok(())
}

/// Read the most recent transaction trace, optionally filtered by request_id.
pub fn read_latest_transaction_trace(
    path: Option<&Path>,
    request_id: Option<&str>,
) -> Result<Option<TransactionTrace>> {
    let _guard = trace_log_mutex()
        .lock()
        .map_err(|_| anyhow!("trace log mutex poisoned"))?;
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(default_transaction_log_path);

    if !path.exists() {
        return Ok(None);
    }

    let file = OpenOptions::new()
        .read(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;

    let reader = BufReader::new(file);
    let mut latest = None;
    for line in reader.lines() {
        let line = line.context("failed to read transaction trace log")?;
        if line.trim().is_empty() {
            continue;
        }
        let trace: TransactionTrace = match serde_json::from_str(&line) {
            Ok(trace) => trace,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::transaction",
                    log_path = %path.display(),
                    %error,
                    "Skipping malformed transaction trace log entry"
                );
                continue;
            }
        };
        if request_id.is_none() || request_id == Some(trace.request_id.as_str()) {
            latest = Some(trace);
        }
    }

    Ok(latest.map(|trace| sanitize_transaction_trace(&trace)))
}

/// Returns true when trace policy says to include the trace in the result.
pub fn should_include_trace(mode: TransactionTraceMode, success: bool) -> bool {
    matches!(mode, TransactionTraceMode::On)
        || (!success && matches!(mode, TransactionTraceMode::OnFailure))
}

#[allow(clippy::too_many_arguments)]
pub fn build_batch_trace_from_results(
    request_id: String,
    command_fingerprint: String,
    started_at_ms: u64,
    total_elapsed_ms: u64,
    success: bool,
    failed_at: Option<usize>,
    commands: &[BatchCommand],
    results: &[BatchResultEntry],
) -> TransactionTrace {
    debug_assert_ne!(
        started_at_ms, 0,
        "transaction traces must carry a real started_at_ms"
    );
    debug_assert_eq!(
        failed_at,
        results.iter().position(|entry| !entry.success),
        "failed_at must match the first failed result"
    );

    let trace = TransactionTrace {
        schema_version: TRANSACTION_TRACE_SCHEMA_VERSION,
        request_id,
        command_fingerprint,
        status: if success {
            TransactionTraceStatus::Ok
        } else {
            TransactionTraceStatus::Failed
        },
        started_at_ms,
        total_elapsed_ms,
        failed_at,
        commands: results
            .iter()
            .map(|entry| TransactionCommandTrace {
                index: entry.index,
                command: entry.command.clone(),
                command_payload: commands.get(entry.index).cloned(),
                started_at_ms,
                elapsed_ms: entry.elapsed.unwrap_or(0),
                before: UiStateSnapshot::default(),
                after: UiStateSnapshot::default(),
                polls: Vec::new(),
                error: entry.error.clone(),
            })
            .collect(),
    };
    sanitize_transaction_trace(&trace)
}

#[allow(clippy::too_many_arguments)]
pub fn maybe_persist_batch_trace_from_results(
    mode: TransactionTraceMode,
    request_id: String,
    command_fingerprint: String,
    started_at_ms: u64,
    total_elapsed_ms: u64,
    success: bool,
    failed_at: Option<usize>,
    commands: &[BatchCommand],
    results: &[BatchResultEntry],
    log_path: Option<&Path>,
) -> Result<Option<TransactionTrace>> {
    if !should_include_trace(mode, success) {
        return Ok(None);
    }

    let trace = build_batch_trace_from_results(
        request_id,
        command_fingerprint,
        started_at_ms,
        total_elapsed_ms,
        success,
        failed_at,
        commands,
        results,
    );
    append_transaction_trace(log_path, &trace)?;
    remember_persisted_transaction_results(&trace.request_id, &trace.command_fingerprint, results);
    Ok(Some(trace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::types::batch_wait::{
        StateMatchSpec, TransactionError, TransactionErrorCode, WaitPollObservation,
    };

    fn successful_private_result(index: usize, value: &str) -> BatchResultEntry {
        BatchResultEntry {
            index,
            success: true,
            command: "selectByValue".to_owned(),
            elapsed: Some(1),
            value: Some(value.to_owned()),
            error: None,
        }
    }

    fn private_trace() -> TransactionTrace {
        let snapshot = UiStateSnapshot {
            input_value: Some("private-prompt-canary".to_owned()),
            selected_value: Some("private-selection-canary".to_owned()),
            focused_semantic_id: Some("private-focused-canary".to_owned()),
            visible_semantic_ids: vec!["private-visible-canary".to_owned()],
            choice_count: 3,
            ..UiStateSnapshot::default()
        };
        let commands = vec![
            BatchCommand::SetInput {
                text: "private-input-canary".to_owned(),
            },
            BatchCommand::ForceSubmit {
                value: serde_json::json!({ "apiKey": "private-api-key-canary" }),
            },
            BatchCommand::WaitFor {
                condition: WaitCondition::Detailed(WaitDetailedCondition::StateMatch {
                    state: StateMatchSpec {
                        input_value: Some("private-wait-canary".to_owned()),
                        selected_value: Some("private-wait-selection-canary".to_owned()),
                        ..StateMatchSpec::default()
                    },
                }),
                timeout: Some(25),
                poll_interval: Some(5),
            },
        ];

        TransactionTrace {
            schema_version: TRANSACTION_TRACE_SCHEMA_VERSION,
            request_id: "safe-request-id".to_owned(),
            command_fingerprint: "{\"text\":\"private-fingerprint-canary\"}".to_owned(),
            status: TransactionTraceStatus::Failed,
            started_at_ms: 1,
            total_elapsed_ms: 25,
            failed_at: Some(0),
            commands: commands
                .into_iter()
                .enumerate()
                .map(|(index, command)| TransactionCommandTrace {
                    index,
                    command: format!("command-{index}"),
                    command_payload: Some(command),
                    started_at_ms: 1,
                    elapsed_ms: 5,
                    before: snapshot.clone(),
                    after: snapshot.clone(),
                    polls: vec![WaitPollObservation {
                        attempt: 1,
                        elapsed_ms: 5,
                        condition_satisfied: false,
                        snapshot: snapshot.clone(),
                        matched_semantic_ids: vec!["private-matched-canary".to_owned()],
                    }],
                    error: Some(TransactionError {
                        code: TransactionErrorCode::SelectionNotFound,
                        message: "could not select private-error-canary".to_owned(),
                        suggestion: Some("retry private-suggestion-canary".to_owned()),
                    }),
                })
                .collect(),
        }
    }

    fn assert_trace_has_no_private_canaries(trace: &TransactionTrace) {
        let serialized = serde_json::to_string(trace).expect("trace serializes");
        assert!(
            !serialized.contains("private-"),
            "trace leaked private input: {serialized}"
        );
        assert!(trace.command_fingerprint.starts_with("sha256:"));
        assert_eq!(trace.command_fingerprint.len(), 71);
        assert_eq!(trace.commands.len(), 3);
        assert_eq!(trace.commands[0].before.choice_count, 3);
        assert_eq!(
            trace.commands[0].error.as_ref().map(|error| &error.code),
            Some(&TransactionErrorCode::SelectionNotFound)
        );
    }

    #[test]
    fn transaction_trace_sanitizes_payload_snapshots_polls_and_errors() {
        assert_trace_has_no_private_canaries(&sanitize_transaction_trace(&private_trace()));
    }

    #[test]
    fn transaction_trace_privacy_projection_is_idempotent() {
        let once = sanitize_transaction_trace(&private_trace());
        let twice = sanitize_transaction_trace(&once);
        assert_eq!(once, twice);
        assert_trace_has_no_private_canaries(&twice);
    }

    #[test]
    fn persisted_and_legacy_transaction_traces_never_expose_private_values() {
        let directory = tempfile::tempdir().expect("temporary trace directory");
        let persisted_path = directory.path().join("persisted.jsonl");
        append_transaction_trace(Some(&persisted_path), &private_trace())
            .expect("private trace persists safely");
        let persisted = fs::read_to_string(&persisted_path).expect("persisted trace exists");
        assert!(!persisted.contains("private-"));

        let legacy_path = directory.path().join("legacy.jsonl");
        let legacy = serde_json::to_string(&private_trace()).expect("legacy trace serializes");
        fs::write(&legacy_path, format!("{legacy}\n")).expect("legacy fixture writes");
        let recovered = read_latest_transaction_trace(Some(&legacy_path), None)
            .expect("legacy trace loads")
            .expect("legacy trace exists");
        assert_trace_has_no_private_canaries(&recovered);
    }

    #[test]
    fn transaction_fingerprints_are_deterministic_hashes_not_payloads() {
        let command = BatchCommand::SetInput {
            text: "private-fingerprint-command-canary".to_owned(),
        };
        let first = crate::protocol::transaction_executor::stable_transaction_fingerprint(
            std::slice::from_ref(&command),
            None,
        )
        .expect("transaction fingerprint computes");
        let second = crate::protocol::transaction_executor::stable_transaction_fingerprint(
            std::slice::from_ref(&command),
            None,
        )
        .expect("transaction fingerprint is deterministic");
        assert_eq!(first, second);
        assert!(is_content_fingerprint(&first));
        assert!(!first.contains("canary"));

        let condition = WaitCondition::Detailed(WaitDetailedCondition::AgentChatInputMatch {
            text: "private-wait-fingerprint-canary".to_owned(),
        });
        let wait =
            crate::protocol::transaction_executor::stable_wait_fingerprint(&condition, 25, 5)
                .expect("wait fingerprint computes");
        assert!(is_content_fingerprint(&wait));
        assert!(!wait.contains("canary"));
    }

    #[test]
    fn private_transaction_fingerprints_and_redacted_values_are_process_keyed() {
        use sha2::{Digest as _, Sha256};

        let secret = "private transaction selection and password";
        let public_sha = format!("{:x}", Sha256::digest(secret.as_bytes()));
        let fingerprint = transaction_content_fingerprint(secret);
        let redacted = private_value(secret);

        assert_eq!(fingerprint, transaction_content_fingerprint(secret));
        assert_ne!(fingerprint, format!("sha256:{public_sha}"));
        assert!(!redacted.contains(secret));
        assert!(!redacted.contains(&public_sha));
        assert!(redacted.contains(&crate::logging::log_private_user_value(secret).sha256));
    }

    #[test]
    fn transaction_private_result_vault_requires_exact_request_fingerprint_and_index() {
        let now = Instant::now();
        let fingerprint = transaction_content_fingerprint("private-vault-fingerprint-canary");
        let wrong_fingerprint = transaction_content_fingerprint("private-vault-other-canary");
        let mut vault = PrivateTransactionResultVault::default();
        vault.remember(
            "safe-vault-request",
            &fingerprint,
            &[successful_private_result(
                4,
                "private-vault-selection-canary",
            )],
            now,
        );

        assert_eq!(
            vault
                .restore("safe-vault-request", &fingerprint, 4, now)
                .as_deref(),
            Some("private-vault-selection-canary")
        );
        assert_eq!(
            vault.restore("different-request", &fingerprint, 4, now),
            None
        );
        assert_eq!(
            vault.restore("safe-vault-request", &wrong_fingerprint, 4, now),
            None
        );
        assert_eq!(
            vault.restore("safe-vault-request", &fingerprint, 5, now),
            None
        );
        assert_eq!(
            vault.restore("safe-vault-request", "noncanonical", 4, now),
            None
        );
    }

    #[test]
    fn transaction_private_result_vault_expires_and_bounds_memory() {
        let now = Instant::now();
        let fingerprint = transaction_content_fingerprint("vault-bounds");
        let mut vault = PrivateTransactionResultVault::default();

        for index in 0..(PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES + 4) {
            vault.remember(
                "bounded-request",
                &fingerprint,
                &[successful_private_result(index, "bounded-value")],
                now,
            );
        }
        assert_eq!(vault.entries.len(), PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES);
        assert!(vault.retained_bytes <= PRIVATE_TRANSACTION_RESULT_MAX_BYTES);
        assert_eq!(vault.restore("bounded-request", &fingerprint, 0, now), None);
        assert!(vault
            .restore(
                "bounded-request",
                &fingerprint,
                PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES + 3,
                now
            )
            .is_some());

        let expired = now + PRIVATE_TRANSACTION_RESULT_TTL;
        assert_eq!(
            vault.restore(
                "bounded-request",
                &fingerprint,
                PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES + 3,
                expired,
            ),
            None
        );
        assert!(vault.entries.is_empty());
        assert_eq!(vault.retained_bytes, 0);

        let too_large = "x".repeat(PRIVATE_TRANSACTION_RESULT_MAX_VALUE_BYTES + 1);
        vault.remember(
            "bounded-request",
            &fingerprint,
            &[successful_private_result(1, &too_large)],
            expired,
        );
        assert!(vault.entries.is_empty());

        let retained_value = "x".repeat(PRIVATE_TRANSACTION_RESULT_MAX_VALUE_BYTES);
        for index in 0..PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES {
            vault.remember(
                "bounded-request",
                &fingerprint,
                &[successful_private_result(index, &retained_value)],
                expired,
            );
        }
        assert!(vault.retained_bytes <= PRIVATE_TRANSACTION_RESULT_MAX_BYTES);
        assert!(vault.entries.len() < PRIVATE_TRANSACTION_RESULT_MAX_ENTRIES);
    }

    #[test]
    fn transaction_warm_replay_restores_selection_without_exposing_it_in_trace() {
        let request_id = "warm-private-replay-safe-request";
        let fingerprint = transaction_content_fingerprint("warm-private-replay-payload");
        let secret = "private-warm-selection-sk-very-secret-canary";
        let results = vec![successful_private_result(2, secret)];
        remember_persisted_transaction_results(request_id, &fingerprint, &results);

        let trace = TransactionTrace {
            schema_version: TRANSACTION_TRACE_SCHEMA_VERSION,
            request_id: request_id.to_owned(),
            command_fingerprint: fingerprint.clone(),
            status: TransactionTraceStatus::Ok,
            started_at_ms: 1,
            total_elapsed_ms: 1,
            failed_at: None,
            commands: vec![TransactionCommandTrace {
                index: 2,
                command: "selectBySemanticId".to_owned(),
                command_payload: Some(BatchCommand::SelectBySemanticId {
                    semantic_id: secret.to_owned(),
                    submit: false,
                }),
                started_at_ms: 1,
                elapsed_ms: 1,
                before: UiStateSnapshot::default(),
                after: UiStateSnapshot {
                    selected_value: Some(secret.to_owned()),
                    ..UiStateSnapshot::default()
                },
                polls: Vec::new(),
                error: None,
            }],
        };
        let replay = crate::protocol::transaction_executor::BatchOutput::from_trace(trace.clone());
        assert_eq!(replay.results[0].value.as_deref(), Some(secret));
        let exposed_trace = serde_json::to_string(replay.trace.as_ref().unwrap()).unwrap();
        assert!(!exposed_trace.contains(secret));

        let wrong_fingerprint_trace = TransactionTrace {
            command_fingerprint: transaction_content_fingerprint("wrong-warm-payload"),
            ..trace.clone()
        };
        let wrong =
            crate::protocol::transaction_executor::BatchOutput::from_trace(wrong_fingerprint_trace);
        assert_eq!(wrong.results[0].value, None);

        let mut wrong_index_trace = trace;
        wrong_index_trace.commands[0].index = 3;
        let wrong =
            crate::protocol::transaction_executor::BatchOutput::from_trace(wrong_index_trace);
        assert_eq!(wrong.results[0].value, None);
    }

    #[test]
    fn transaction_private_results_publish_only_after_safe_persistence() {
        let directory = tempfile::tempdir().expect("isolated transaction trace directory");
        let request_id = "safe-private-vault-persistence-request";
        let fingerprint = transaction_content_fingerprint("private-vault-persistence-payload");
        let secret = "private-persisted-selection-sk-sensitive-canary";
        let commands = [BatchCommand::SelectByValue {
            value: secret.to_owned(),
            submit: false,
        }];
        let results = [successful_private_result(0, secret)];

        let failed = maybe_persist_batch_trace_from_results(
            TransactionTraceMode::On,
            request_id.to_owned(),
            fingerprint.clone(),
            1,
            1,
            true,
            None,
            &commands,
            &results,
            Some(directory.path()),
        );
        assert!(failed.is_err());
        assert_eq!(
            restore_persisted_transaction_result(request_id, &fingerprint, 0),
            None
        );

        let path = directory.path().join("safe-transactions.jsonl");
        let trace = maybe_persist_batch_trace_from_results(
            TransactionTraceMode::On,
            request_id.to_owned(),
            fingerprint.clone(),
            1,
            1,
            true,
            None,
            &commands,
            &results,
            Some(&path),
        )
        .expect("safe trace persists")
        .expect("trace is included");
        assert_eq!(
            restore_persisted_transaction_result(request_id, &fingerprint, 0).as_deref(),
            Some(secret)
        );
        assert!(!serde_json::to_string(&trace).unwrap().contains(secret));
        assert!(!fs::read_to_string(&path).unwrap().contains(secret));
    }

    #[test]
    fn transaction_private_result_vault_ignores_failed_empty_and_invalid_entries() {
        let now = Instant::now();
        let fingerprint = transaction_content_fingerprint("vault-invalid-entries");
        let mut failed = successful_private_result(0, "failed-secret");
        failed.success = false;
        let empty = successful_private_result(1, "");
        let mut vault = PrivateTransactionResultVault::default();

        vault.remember("valid-request", &fingerprint, &[failed, empty], now);
        vault.remember(
            "",
            &fingerprint,
            &[successful_private_result(2, "secret")],
            now,
        );
        vault.remember(
            "valid-request",
            "raw-payload-not-a-fingerprint",
            &[successful_private_result(3, "secret")],
            now,
        );

        assert!(vault.entries.is_empty());
        assert_eq!(vault.retained_bytes, 0);
    }
}
