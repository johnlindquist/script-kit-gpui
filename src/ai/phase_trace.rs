//! Shared AI phase trace: one NDJSON lifecycle stream for every AI surface.
//!
//! Quick AI has had a private `TraceSink` in `agent_chat/codex_exec.rs` since
//! the latency work. It answered "where did the 6 seconds go?" for exactly one
//! surface. This module generalises that instrument so Agent Chat, Text, Mini,
//! and Flows are measurable too, without duplicating the writer three times.
//!
//! # Why a common vocabulary across three different transports
//!
//! The three transports are structurally unlike each other:
//!
//! | Surface | Transport | Owner |
//! |---|---|---|
//! | Quick AI | cold one-shot `codex exec` | `agent_chat/codex_exec.rs` |
//! | Agent Chat / Text / Mini | Pi sidecar RPC | `agent_chat/pi/runtime.rs` |
//! | Flows | persistent `codex app-server` JSON-RPC | `flows/codex_client.rs` |
//!
//! What they share is a *turn shape*: something starts, the provider first
//! responds, the user first sees something, the turn ends, resources are
//! released. Those five moments are the vocabulary. Anything transport-specific
//! stays in that transport's own detail fields rather than inventing a new
//! event name, so one analyzer can compare all four surfaces.
//!
//! # Redaction posture (binding, per `rules/AI_RELIABILITY.md`)
//!
//! Raw provider text, stderr, OS errors, tool names, and adapter internals stop
//! at the diagnostic vault. This trace is a *timing* instrument, not a content
//! log. It therefore records:
//!
//! - lengths (`textChars`) and salted-free SHA-256 hex digests (`textSha256`),
//!   never the text;
//! - stable enum labels (`outcome`, `surface`, `transport`, `failureCode`),
//!   never free-form provider prose.
//!
//! `phase_trace_tests::redaction_*` proves this holds for hostile inputs.
//!
//! # Cost when disabled
//!
//! [`PhaseTrace::disabled`] stores `None`. Every method starts with a `let
//! Some(inner) = &self.inner else { return }`, so a build with the env var
//! unset does one null check per event and never touches the filesystem, the
//! clock, or the hasher. `overhead_when_disabled_is_a_single_null_check` pins
//! that the hashing closure is not even evaluated.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

/// Environment variable naming the NDJSON file every surface appends to.
///
/// One file, not one per surface: the interesting comparisons are *between*
/// surfaces, and every record carries `surface` + `runId` so a reader can
/// partition trivially. Separate files would make the common case (compare
/// Agent Chat against Flows) require a join.
pub(crate) const AI_TRACE_PATH_ENV: &str = "SCRIPT_KIT_AI_TRACE_PATH";

/// Schema version for records written by this module.
///
/// Deliberately starts at 1 independently of Quick AI's private sink: these are
/// different record shapes (this one carries `surface`/`transport`), and a
/// reader distinguishes them by the presence of those fields.
pub(crate) const AI_TRACE_SCHEMA_VERSION: u64 = 1;

/// Which AI surface produced a record.
///
/// Sourced from the already-plumbed Agent Chat `profile_id` rather than from a
/// new field or from `ui_variant.rs`, so no caller has to learn a new concept
/// and the UI layer is untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiSurface {
    QuickAi,
    AgentChat,
    Text,
    Mini,
    Flow,
}

impl AiSurface {
    /// Stable wire label. Never derived from user data.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::QuickAi => "quick-ai",
            Self::AgentChat => "agent-chat",
            Self::Text => "text",
            Self::Mini => "mini",
            Self::Flow => "flow",
        }
    }

    /// Map an Agent Chat profile id plus turn kind onto a surface.
    ///
    /// `auxiliary` is the focused-text variation path
    /// (`start_isolated_turn`): same Pi transport, but a separate process and a
    /// different user-visible surface, so it must not be pooled with the
    /// primary Text turn when computing medians.
    pub(crate) fn from_profile(profile_id: &str, auxiliary: bool) -> Self {
        use crate::ai::agent_chat::profiles::{
            BUILTIN_QUICK_AI_PROFILE_ID, BUILTIN_TEXT_PROFILE_ID,
        };
        match (profile_id, auxiliary) {
            (_, true) => Self::Mini,
            (BUILTIN_QUICK_AI_PROFILE_ID, _) => Self::QuickAi,
            (BUILTIN_TEXT_PROFILE_ID, _) => Self::Text,
            _ => Self::AgentChat,
        }
    }
}

/// Which wire protocol carried the turn. Distinguishes cost that belongs to the
/// transport (cold process spawn) from cost that belongs to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiTransport {
    /// Cold one-shot `codex exec` subprocess.
    CodexExec,
    /// Long-lived Pi sidecar over JSON-RPC on stdio.
    PiRpc,
    /// Persistent `codex app-server` JSON-RPC session.
    CodexAppServer,
}

impl AiTransport {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::CodexExec => "codex-exec",
            Self::PiRpc => "pi-rpc",
            Self::CodexAppServer => "codex-app-server",
        }
    }
}

/// How a turn ended. A user Stop is `cancelled`, never `failed` — the
/// reliability rules treat cancellation as a normal outcome, and folding it
/// into `failed` would corrupt both the failure rate and the latency medians.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnOutcome {
    Completed,
    Failed,
    Cancelled,
}

impl TurnOutcome {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Only a completed turn is a valid latency sample. A failed turn measures
    /// how fast something broke; Pi previously "finished" in 800-900ms in a
    /// sandbox purely because it could not start.
    pub(crate) fn is_latency_sample(self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// Canonical event names. Free-form strings are deliberately not accepted: a
/// typo would silently produce an event no analyzer counts.
pub(crate) mod events {
    /// The turn was accepted by the transport and work began.
    pub(crate) const TURN_START: &str = "turn_start";
    /// The first byte of any kind came back from the provider. Bounds
    /// transport + queue + model time-to-first-token.
    pub(crate) const FIRST_PROVIDER_EVENT: &str = "first_provider_event";
    /// The first token the user can actually read appeared. This is the
    /// perceived-responsiveness number.
    pub(crate) const FIRST_VISIBLE_OUTPUT: &str = "first_visible_output";
    /// The first reasoning/thought token. Present on some surfaces only; it is
    /// visible feedback but not the answer.
    pub(crate) const FIRST_THOUGHT: &str = "first_thought";
    /// A tool call started. Counted, not detailed.
    pub(crate) const TOOL_CALL_STARTED: &str = "tool_call_started";
    /// The turn reached a terminal state.
    pub(crate) const TERMINAL: &str = "terminal";
    /// Transport-side cleanup finished.
    pub(crate) const TEARDOWN: &str = "teardown";
}

struct Inner {
    path: PathBuf,
    surface: AiSurface,
    transport: AiTransport,
    run_id: String,
    seq: AtomicU64,
    started: Instant,
    first_provider_event: AtomicBool,
    first_visible_output: AtomicBool,
    first_thought: AtomicBool,
    terminal: AtomicBool,
    tool_calls: AtomicU64,
}

/// A per-turn phase trace handle.
///
/// Cheap to clone (one `Arc`), safe to share across the async tasks that read a
/// transport's stdout, and inert when tracing is off.
#[derive(Clone)]
pub(crate) struct PhaseTrace {
    inner: Option<Arc<Inner>>,
}

impl std::fmt::Debug for PhaseTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhaseTrace")
            .field("enabled", &self.is_enabled())
            .finish()
    }
}

impl PhaseTrace {
    /// A trace that records nothing. Used when the env var is unset and by every
    /// test or construction path that does not care about timing.
    pub(crate) fn disabled() -> Self {
        Self { inner: None }
    }

    /// Begin a turn, reading the output path from the environment.
    ///
    /// Returns [`PhaseTrace::disabled`] when `SCRIPT_KIT_AI_TRACE_PATH` is
    /// unset, which is the shipping default.
    pub(crate) fn begin(
        surface: AiSurface,
        transport: AiTransport,
        run_id: impl Into<String>,
    ) -> Self {
        match std::env::var_os(AI_TRACE_PATH_ENV) {
            Some(path) if !path.is_empty() => {
                Self::begin_at(PathBuf::from(path), surface, transport, run_id)
            }
            _ => Self::disabled(),
        }
    }

    /// Begin a turn writing to an explicit path. Tests use this so they never
    /// mutate process-wide environment state.
    pub(crate) fn begin_at(
        path: PathBuf,
        surface: AiSurface,
        transport: AiTransport,
        run_id: impl Into<String>,
    ) -> Self {
        Self {
            inner: Some(Arc::new(Inner {
                path,
                surface,
                transport,
                run_id: run_id.into(),
                seq: AtomicU64::new(1),
                started: Instant::now(),
                first_provider_event: AtomicBool::new(false),
                first_visible_output: AtomicBool::new(false),
                first_thought: AtomicBool::new(false),
                terminal: AtomicBool::new(false),
                tool_calls: AtomicU64::new(0),
            })),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Milliseconds since this turn began, or `None` when disabled.
    pub(crate) fn elapsed_ms(&self) -> Option<u64> {
        self.inner
            .as_ref()
            .map(|inner| inner.started.elapsed().as_millis() as u64)
    }

    /// Write one record. Private so the event name is always from [`events`].
    fn write(&self, event: &str, details: Value) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut record = json!({
            "schemaVersion": AI_TRACE_SCHEMA_VERSION,
            "runId": inner.run_id,
            "surface": inner.surface.label(),
            "transport": inner.transport.label(),
            "seq": inner.seq.fetch_add(1, Ordering::Relaxed),
            "event": event,
            "elapsedMs": inner.started.elapsed().as_millis() as u64,
        });
        if let (Some(target), Some(fields)) = (record.as_object_mut(), details.as_object()) {
            target.extend(fields.clone());
        }
        if let Some(parent) = inner.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Serialise the whole record — newline included — into one buffer and
        // issue exactly ONE `write_all`.
        //
        // This is load-bearing, not stylistic. `writeln!` against a `File`
        // drives `fmt::Write` machinery that emits several `write` syscalls per
        // record. Pi's stdout reader races its command loop, and Flows' event
        // pump races its session thread, so two records interleave *inside* a
        // line and NDJSON parsing dies on corrupted JSON. A single `write_all`
        // to an `O_APPEND` descriptor is atomic with respect to the file
        // offset, so records can be ordered arbitrarily but never spliced.
        // `concurrent_writes_produce_unique_sequence_numbers` is the regression.
        let mut line = record.to_string();
        line.push('\n');
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inner.path)
        {
            use std::io::Write as _;
            let _ = file.write_all(line.as_bytes());
        }
    }

    /// Record the turn start. Call once, as early as the transport accepts work.
    pub(crate) fn turn_start(&self, details: Value) {
        self.write(events::TURN_START, details);
    }

    /// Record the first provider byte. Idempotent — safe to call on every
    /// inbound line, which is how transports actually observe it.
    pub(crate) fn observe_provider_event(&self) {
        let Some(inner) = &self.inner else {
            return;
        };
        if !inner.first_provider_event.swap(true, Ordering::AcqRel) {
            self.write(events::FIRST_PROVIDER_EVENT, json!({}));
        }
    }

    /// Record the first user-visible token. Idempotent; later deltas are a
    /// relaxed atomic load and nothing else, so streaming stays cheap.
    pub(crate) fn observe_visible_output(&self, text: &str) {
        let Some(inner) = &self.inner else {
            return;
        };
        if !inner.first_visible_output.swap(true, Ordering::AcqRel) {
            self.write(
                events::FIRST_VISIBLE_OUTPUT,
                json!({
                    "textChars": text.chars().count(),
                    "textSha256": sha256_hex(text),
                }),
            );
        }
    }

    /// Record the first reasoning token. Idempotent.
    pub(crate) fn observe_thought(&self, text: &str) {
        let Some(inner) = &self.inner else {
            return;
        };
        if !inner.first_thought.swap(true, Ordering::AcqRel) {
            self.write(
                events::FIRST_THOUGHT,
                json!({
                    "textChars": text.chars().count(),
                    "textSha256": sha256_hex(text),
                }),
            );
        }
    }

    /// Count a tool call. The tool *name* is never written — only the ordinal,
    /// because a tool name can carry user data (a script path, a file name).
    pub(crate) fn observe_tool_call(&self) {
        let Some(inner) = &self.inner else {
            return;
        };
        let ordinal = inner.tool_calls.fetch_add(1, Ordering::Relaxed) + 1;
        self.write(events::TOOL_CALL_STARTED, json!({ "ordinal": ordinal }));
    }

    /// Record the terminal outcome. Idempotent: transports have several racing
    /// paths that can each believe they ended the turn, and a second `terminal`
    /// record would double-count the turn in any analyzer.
    ///
    /// `failure_code` must be a stable classifier code, never provider prose.
    pub(crate) fn terminal(&self, outcome: TurnOutcome, failure_code: Option<&str>) {
        let Some(inner) = &self.inner else {
            return;
        };
        if inner.terminal.swap(true, Ordering::AcqRel) {
            return;
        }
        let mut details = json!({
            "outcome": outcome.label(),
            "isLatencySample": outcome.is_latency_sample(),
            "toolCalls": inner.tool_calls.load(Ordering::Relaxed),
        });
        if let (Some(map), Some(code)) = (details.as_object_mut(), failure_code) {
            map.insert("failureCode".to_string(), json!(code));
        }
        self.write(events::TERMINAL, details);
    }

    /// Record transport cleanup.
    pub(crate) fn teardown(&self) {
        self.write(events::TEARDOWN, json!({}));
    }
}

/// Hex SHA-256. Lets a reader confirm two surfaces saw the same text without
/// the trace ever containing that text.
pub(crate) fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod phase_trace_tests {
    use super::*;

    fn read_records(path: &std::path::Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("each line is valid JSON"))
            .collect()
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("sk-phase-trace-tests")
            .join(format!("{name}-{}.ndjson", std::process::id()))
    }

    /// A disabled trace must not create its file, so an unset env var costs
    /// nothing at all rather than "only a little".
    #[test]
    fn disabled_trace_writes_nothing() {
        let trace = PhaseTrace::disabled();
        assert!(!trace.is_enabled());
        assert_eq!(trace.elapsed_ms(), None);
        trace.turn_start(json!({}));
        trace.observe_provider_event();
        trace.observe_visible_output("hello");
        trace.observe_tool_call();
        trace.terminal(TurnOutcome::Completed, None);
        trace.teardown();
        // Nothing to assert against a path because no path exists — that is
        // exactly the point.
    }

    /// The hashing/counting work must not run when tracing is off. If the
    /// argument were eagerly evaluated at a call site this would catch it.
    #[test]
    fn overhead_when_disabled_is_a_single_null_check() {
        let trace = PhaseTrace::disabled();
        let mut evaluated = false;
        if trace.is_enabled() {
            evaluated = true;
            trace.observe_visible_output(&"x".repeat(1_000_000));
        }
        assert!(!evaluated, "guarded work must not run when tracing is off");
    }

    #[test]
    fn records_carry_surface_transport_and_monotonic_seq() {
        let path = temp_path("seq");
        let _ = std::fs::remove_file(&path);
        let trace = PhaseTrace::begin_at(
            path.clone(),
            AiSurface::AgentChat,
            AiTransport::PiRpc,
            "run-1",
        );
        trace.turn_start(json!({}));
        trace.observe_provider_event();
        trace.observe_visible_output("hi");
        trace.terminal(TurnOutcome::Completed, None);
        trace.teardown();

        let records = read_records(&path);
        assert_eq!(records.len(), 5, "one record per phase");
        let seqs: Vec<u64> = records.iter().map(|r| r["seq"].as_u64().unwrap()).collect();
        assert_eq!(seqs, vec![1, 2, 3, 4, 5], "seq is monotonic");
        for record in &records {
            assert_eq!(record["surface"], "agent-chat");
            assert_eq!(record["transport"], "pi-rpc");
            assert_eq!(record["runId"], "run-1");
            assert_eq!(record["schemaVersion"], AI_TRACE_SCHEMA_VERSION);
            assert!(record["elapsedMs"].is_u64(), "every record is timestamped");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// The five milestones the premise requires must all be present and
    /// each must carry `elapsedMs`.
    #[test]
    fn emits_the_five_required_milestones() {
        let path = temp_path("milestones");
        let _ = std::fs::remove_file(&path);
        let trace = PhaseTrace::begin_at(
            path.clone(),
            AiSurface::Flow,
            AiTransport::CodexAppServer,
            "r",
        );
        trace.turn_start(json!({}));
        trace.observe_provider_event();
        trace.observe_visible_output("a");
        trace.terminal(TurnOutcome::Completed, None);
        trace.teardown();

        let records = read_records(&path);
        let names: Vec<&str> = records
            .iter()
            .map(|r| r["event"].as_str().unwrap())
            .collect();
        for required in [
            events::TURN_START,
            events::FIRST_PROVIDER_EVENT,
            events::FIRST_VISIBLE_OUTPUT,
            events::TERMINAL,
            events::TEARDOWN,
        ] {
            assert!(names.contains(&required), "missing milestone {required}");
        }
        let _ = std::fs::remove_file(&path);
    }

    /// Streaming fires these on every delta; only the first may be recorded or
    /// the trace becomes a token log (and a redaction leak).
    #[test]
    fn first_only_milestones_are_idempotent() {
        let path = temp_path("idempotent");
        let _ = std::fs::remove_file(&path);
        let trace = PhaseTrace::begin_at(path.clone(), AiSurface::Text, AiTransport::PiRpc, "r");
        for _ in 0..50 {
            trace.observe_provider_event();
            trace.observe_visible_output("delta");
            trace.observe_thought("thinking");
        }
        let records = read_records(&path);
        assert_eq!(records.len(), 3, "50 deltas produce 3 first-only records");
        let _ = std::fs::remove_file(&path);
    }

    /// Several transport paths race to end a turn; a double terminal would
    /// double-count the turn in every median.
    #[test]
    fn terminal_is_recorded_once_even_when_racing_paths_both_fire() {
        let path = temp_path("terminal-once");
        let _ = std::fs::remove_file(&path);
        let trace =
            PhaseTrace::begin_at(path.clone(), AiSurface::AgentChat, AiTransport::PiRpc, "r");
        trace.terminal(TurnOutcome::Completed, None);
        trace.terminal(TurnOutcome::Failed, Some("RuntimeClosed"));
        let records = read_records(&path);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["outcome"], "completed", "first terminal wins");
        let _ = std::fs::remove_file(&path);
    }

    /// A user Stop is cancellation, not an error, and is not a latency sample.
    #[test]
    fn cancelled_and_failed_turns_are_not_latency_samples() {
        assert!(TurnOutcome::Completed.is_latency_sample());
        assert!(!TurnOutcome::Failed.is_latency_sample());
        assert!(
            !TurnOutcome::Cancelled.is_latency_sample(),
            "a Stop measures the user's reaction time, not the provider's"
        );
    }

    /// The binding redaction rule: hostile text containing a query, a provider
    /// name, and a tool name must not survive into the trace in any form.
    #[test]
    fn redaction_holds_for_query_provider_and_tool_text() {
        let path = temp_path("redaction");
        let _ = std::fs::remove_file(&path);
        let secret_query = "what is my bank balance at Chase";
        let provider = "openai-codex-internal-endpoint";
        let tool = "read_file:/Users/private/.ssh/id_rsa";

        let trace = PhaseTrace::begin_at(path.clone(), AiSurface::Mini, AiTransport::PiRpc, "r");
        trace.turn_start(json!({}));
        trace.observe_visible_output(secret_query);
        trace.observe_thought(provider);
        trace.observe_tool_call();
        trace.terminal(TurnOutcome::Failed, Some("RuntimeClosed"));

        let raw = std::fs::read_to_string(&path).unwrap();
        for leaked in [secret_query, provider, tool, "bank", "id_rsa", "Chase"] {
            assert!(
                !raw.contains(leaked),
                "trace leaked {leaked:?}; it must record hashes and counts only"
            );
        }
        // ...while still being useful: the digest and length are present.
        assert!(raw.contains(&sha256_hex(secret_query)));
        assert!(raw.contains("\"textChars\""));
        assert!(
            raw.contains("RuntimeClosed"),
            "a stable classifier code is allowed and needed"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The surface label must come from the existing profile plumbing, and the
    /// auxiliary (focused-text variation) path must not be pooled with Text.
    #[test]
    fn surface_is_derived_from_profile_and_turn_kind() {
        use crate::ai::agent_chat::profiles::{
            BUILTIN_GENERAL_PROFILE_ID, BUILTIN_QUICK_AI_PROFILE_ID, BUILTIN_TEXT_PROFILE_ID,
        };
        assert_eq!(
            AiSurface::from_profile(BUILTIN_QUICK_AI_PROFILE_ID, false),
            AiSurface::QuickAi
        );
        assert_eq!(
            AiSurface::from_profile(BUILTIN_TEXT_PROFILE_ID, false),
            AiSurface::Text
        );
        assert_eq!(
            AiSurface::from_profile(BUILTIN_GENERAL_PROFILE_ID, false),
            AiSurface::AgentChat
        );
        assert_eq!(
            AiSurface::from_profile(BUILTIN_TEXT_PROFILE_ID, true),
            AiSurface::Mini,
            "auxiliary variation turns are their own surface"
        );
    }

    /// Concurrent writers share one `Arc`; seq must stay unique so records are
    /// never silently lost when Pi's stdout reader races the command loop.
    #[test]
    fn concurrent_writes_produce_unique_sequence_numbers() {
        let path = temp_path("concurrent");
        let _ = std::fs::remove_file(&path);
        let trace =
            PhaseTrace::begin_at(path.clone(), AiSurface::AgentChat, AiTransport::PiRpc, "r");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let trace = trace.clone();
                std::thread::spawn(move || {
                    for _ in 0..25 {
                        trace.observe_tool_call();
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let records = read_records(&path);
        assert_eq!(records.len(), 200);
        let mut seqs: Vec<u64> = records.iter().map(|r| r["seq"].as_u64().unwrap()).collect();
        seqs.sort_unstable();
        seqs.dedup();
        assert_eq!(seqs.len(), 200, "no two records shared a sequence number");
        let _ = std::fs::remove_file(&path);
    }
}
