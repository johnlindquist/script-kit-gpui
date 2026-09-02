//! Shared, thread-safe registry of flow runs.
//!
//! The registry is the single source of truth for run state across every
//! Flow UX variation and the Flow Manager window — renderers are thin views
//! over this state (the variations must stay swappable without touching run
//! lifecycle). Runner threads apply protocol events here; UI layers read
//! snapshots. Output retention is bounded per run (see
//! `model::OUTPUT_TAIL_MAX_BYTES` / `OUTPUT_TAIL_MAX_LINES`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use sk_protocol::command_contract::{CommandAvailability, CommandIdentity, CommandSource};
use sk_protocol::execution_contract::{
    ExecutionCleanupProof, ExecutionEvent, ExecutionLifecycle, ExecutionReceipt,
};

use super::model::{
    EngagementMode, FlowUxVariant, OutputTail, RunEvent, RunEventEnvelope, RunPhase, RunTimings,
    StepState,
};

static REGISTRY: Mutex<Option<Arc<FlowRunRegistry>>> = Mutex::new(None);

/// Global registry accessor (created on first use).
pub fn flow_run_registry() -> Arc<FlowRunRegistry> {
    let mut guard = REGISTRY.lock();
    guard
        .get_or_insert_with(|| Arc::new(FlowRunRegistry::new()))
        .clone()
}

/// Monotonic id for locally-created runs before the protocol `runId` arrives.
static NEXT_LOCAL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct FlowRun {
    /// App-local id, stable for the run's lifetime (protocol runId is
    /// recorded separately once known — local ids exist so a card can render
    /// in the same frame the launch is requested).
    pub local_id: u64,
    pub protocol_run_id: Option<String>,
    pub flow_id: String,
    pub flow_name: String,
    pub flow_path: String,
    pub cwd: String,
    pub variant: FlowUxVariant,
    pub phase: RunPhase,
    pub engagement: EngagementMode,
    pub exit_code: Option<i64>,
    pub failure: Option<crate::ai::reliability::AppFailureRecord>,
    /// PGID of the app-spawned `md` process (group leader via
    /// `process_group(0)`). Immutable once set — cancellation signals THIS.
    pub pid: Option<u32>,
    /// Engine/orchestrator pid as reported by `run.started`. Informational
    /// only; never a signal target (it is not a group leader).
    pub engine_pid: Option<u32>,
    /// Input override names recorded at launch (values are never stored —
    /// password redaction holds by construction).
    pub override_names: Vec<String>,
    pub stdout_tail: OutputTail,
    pub stderr_tail: OutputTail,
    /// Chronologically interleaved stdout+stderr (stderr lines prefixed
    /// "stderr· ") so late errors cannot disappear behind stdout.
    pub merged_tail: OutputTail,
    /// Append-only stdout capture for conversation turns. The bounded tails
    /// front-evict, which breaks byte cursors (2026-07-11 audit P0); this
    /// buffer never evicts — it FREEZES at
    /// `model::CONVERSATION_CAPTURE_MAX_BYTES` and sets
    /// `conversation_truncated` instead. `None` for bare runs.
    pub conversation_stdout: Option<String>,
    pub conversation_truncated: bool,
    /// True once a terminal-phase toast has been surfaced for this run
    /// (`take_unnotified_terminal`), so the UI tick can never double-notify.
    pub notified: bool,
    pub steps: Vec<(String, StepState)>,
    pub timings: RunTimings,
    pub launched_at: Instant,
    pub duration_ms: Option<u64>,
    /// Exact owner-observed process-group cleanup; absent until the full
    /// group is gone, never set from a child event or signal dispatch.
    pub cancellation_cleanup: Option<ExecutionCleanupProof>,
}

impl FlowRun {
    pub fn display_status(&self) -> String {
        match self.phase {
            RunPhase::Failed => match (self.exit_code, &self.failure) {
                (Some(code), _) => format!("Failed (exit {code})"),
                (None, Some(failure)) => format!("Failed: {}", failure.primary_message()),
                (None, None) => "Failed".to_string(),
            },
            phase => phase.label().to_string(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        match self.duration_ms {
            Some(ms) => ms,
            None => self.launched_at.elapsed().as_millis() as u64,
        }
    }

    pub fn last_output_line(&self) -> Option<&str> {
        self.merged_tail
            .last_line()
            .or_else(|| self.stdout_tail.last_line())
            .or_else(|| self.stderr_tail.last_line())
    }

    /// Project terminal Flow state through the same typed execution contract
    /// used by other command families without replacing Flow's richer reducer.
    /// A cancellation has no receipt until exact cleanup proof is present.
    pub fn execution_receipt(&self) -> Option<ExecutionReceipt> {
        if !self.phase.is_terminal() {
            return None;
        }
        let command_id = CommandIdentity::new(CommandSource::Flow, &self.flow_id).ok()?;
        let run_id = self
            .protocol_run_id
            .clone()
            .unwrap_or_else(|| format!("flow-local-{}", self.local_id));
        let mut lifecycle = ExecutionLifecycle::new(run_id, command_id).ok()?;
        lifecycle.preflight(&CommandAvailability::Ready).ok()?;
        lifecycle.apply(ExecutionEvent::Prepare).ok()?;

        match self.phase {
            RunPhase::Succeeded => {
                lifecycle.apply(ExecutionEvent::Start).ok()?;
                if let Some(process_group_id) = self.pid {
                    lifecycle.bind_process_group(process_group_id).ok()?;
                }
                lifecycle.apply(ExecutionEvent::Complete).ok()?;
            }
            RunPhase::Failed => {
                if let Some(process_group_id) = self.pid {
                    lifecycle.apply(ExecutionEvent::Start).ok()?;
                    lifecycle.bind_process_group(process_group_id).ok()?;
                }
                lifecycle.apply(ExecutionEvent::Fail).ok()?;
            }
            RunPhase::Cancelled => {
                if let Some(process_group_id) = self.pid {
                    lifecycle.apply(ExecutionEvent::Start).ok()?;
                    lifecycle.bind_process_group(process_group_id).ok()?;
                }
                lifecycle.apply(ExecutionEvent::Cancel).ok()?;
                lifecycle
                    .confirm_cancellation(self.cancellation_cleanup.clone()?)
                    .ok()?;
            }
            RunPhase::Starting | RunPhase::Running | RunPhase::Cancelling => return None,
        }

        let fingerprint = self.failure.as_ref().and_then(|failure| {
            failure
                .failure
                .diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.fingerprint.0.clone())
        });
        lifecycle.receipt(self.elapsed_ms(), fingerprint).ok()
    }
}

fn observed_cancellation_cleanup(run: &FlowRun) -> Option<ExecutionCleanupProof> {
    if run.pid.is_some_and(super::runner::process_group_alive) {
        return None;
    }

    Some(ExecutionCleanupProof {
        run_id: run
            .protocol_run_id
            .clone()
            .unwrap_or_else(|| format!("flow-local-{}", run.local_id)),
        command_id: CommandIdentity::new(CommandSource::Flow, &run.flow_id).ok()?,
        process_group_id: run.pid,
        process_group_alive: false,
        resources_released: true,
    })
}

/// Lightweight row-facing view of one run (see `run_summaries`).
#[derive(Debug, Clone)]
pub struct RunSummary {
    pub local_id: u64,
    pub flow_id: String,
    pub flow_name: String,
    pub phase: RunPhase,
    pub display_status: String,
    pub elapsed_ms: u64,
    pub last_output_line: Option<String>,
    pub is_conversation: bool,
}

#[derive(Default)]
struct RegistryState {
    runs: HashMap<u64, FlowRun>,
    /// Insertion order for stable display (newest last).
    order: Vec<u64>,
    selected: Option<u64>,
    generation: u64,
}

/// Called (from any thread) after every state mutation so the UI layer can
/// schedule a repaint. Set once at startup by the app layer.
type NotifyHook = Box<dyn Fn() + Send + Sync>;

pub struct FlowRunRegistry {
    state: Mutex<RegistryState>,
    notify: Mutex<Option<NotifyHook>>,
}

impl FlowRunRegistry {
    fn new() -> Self {
        Self {
            state: Mutex::new(RegistryState::default()),
            notify: Mutex::new(None),
        }
    }

    pub fn set_notify_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self.notify.lock() = Some(Box::new(hook));
    }

    fn bump_and_notify(&self, state: &mut RegistryState) {
        // Enforce retention on every mutation, not just inserts — a run
        // turning terminal can push the finished count over the cap too.
        Self::evict_finished_over_cap(state);
        state.generation = state.generation.wrapping_add(1);
        drop_notify(&self.notify);
    }

    /// Insert a `Starting` run BEFORE spawning the process, so the launch is
    /// acknowledged in UI state immediately (the <100ms ack receipt).
    pub fn insert_starting(
        &self,
        flow_id: &str,
        flow_name: &str,
        flow_path: &str,
        cwd: &str,
        variant: FlowUxVariant,
        engagement: EngagementMode,
    ) -> u64 {
        let local_id = NEXT_LOCAL_ID.fetch_add(1, Ordering::Relaxed);
        let run = FlowRun {
            local_id,
            protocol_run_id: None,
            flow_id: flow_id.to_string(),
            flow_name: flow_name.to_string(),
            flow_path: flow_path.to_string(),
            cwd: cwd.to_string(),
            variant,
            phase: RunPhase::Starting,
            engagement,
            exit_code: None,
            failure: None,
            pid: None,
            engine_pid: None,
            override_names: Vec::new(),
            stdout_tail: OutputTail::default(),
            stderr_tail: OutputTail::default(),
            merged_tail: OutputTail::default(),
            conversation_stdout: None,
            conversation_truncated: false,
            notified: false,
            steps: Vec::new(),
            timings: RunTimings::default(),
            launched_at: Instant::now(),
            duration_ms: None,
            cancellation_cleanup: None,
        };
        let mut state = self.state.lock();
        state.order.push(local_id);
        state.runs.insert(local_id, run);
        // Selection is presentation state: a new launch must not steal the
        // manager's focus away from a run the user is inspecting.
        if state.selected.is_none() {
            state.selected = Some(local_id);
        }
        self.bump_and_notify(&mut state);
        local_id
    }

    /// Bound completed-run retention so an all-day session cannot grow the
    /// registry (and every snapshot clone) without limit. Active runs are
    /// never evicted.
    fn evict_finished_over_cap(state: &mut RegistryState) {
        const FINISHED_RETENTION_CAP: usize = 100;
        let finished: Vec<u64> = state
            .order
            .iter()
            .copied()
            .filter(|id| {
                state
                    .runs
                    .get(id)
                    .is_some_and(|run| run.phase.is_terminal())
            })
            .collect();
        if finished.len() <= FINISHED_RETENTION_CAP {
            return;
        }
        let excess = finished.len() - FINISHED_RETENTION_CAP;
        for id in finished.into_iter().take(excess) {
            state.runs.remove(&id);
            state.order.retain(|other| *other != id);
            if state.selected == Some(id) {
                state.selected = None;
            }
        }
    }

    /// Apply one protocol event from the runner thread.
    pub fn apply_event(&self, local_id: u64, envelope: &RunEventEnvelope) {
        let mut state = self.state.lock();
        let Some(run) = state.runs.get_mut(&local_id) else {
            return;
        };
        // Terminal phases are final: once a run is Cancelled/Failed/
        // Succeeded, NO later lifecycle mutation is accepted — a late
        // `run.started` must not resurrect a cancelled run as Running, and
        // late output/exit data must not rewrite a settled outcome.
        if run.phase.is_terminal() {
            return;
        }
        if run.protocol_run_id.is_none() {
            run.protocol_run_id = Some(envelope.run_id.clone());
        }
        match &envelope.event {
            RunEvent::Protocol { .. } => {}
            RunEvent::RunStarted { pid, .. } => {
                if run.phase == RunPhase::Starting {
                    run.phase = RunPhase::Running;
                }
                // `run.pid` stays the app-spawned group leader (the ONLY
                // valid killpg target); the protocol-reported pid is the
                // engine/orchestrator, recorded separately.
                run.engine_pid = *pid;
                if run.timings.spawn_ms.is_none() {
                    run.timings.spawn_ms = Some(run.launched_at.elapsed().as_millis() as u64);
                }
            }
            RunEvent::OutputDelta { channel, text } => {
                if run.timings.first_output_ms.is_none() {
                    run.timings.first_output_ms =
                        Some(run.launched_at.elapsed().as_millis() as u64);
                }
                match channel {
                    super::model::OutputChannel::Stdout => {
                        run.stdout_tail.push_text(text);
                        run.merged_tail.push_text(text);
                        // Conversation capture is append-only and freezes at
                        // its cap (no partial appends — cursor math must
                        // never land inside a multibyte character).
                        if !run.conversation_truncated {
                            if let Some(capture) = run.conversation_stdout.as_mut() {
                                if capture.len() + text.len()
                                    <= super::model::CONVERSATION_CAPTURE_MAX_BYTES
                                {
                                    capture.push_str(text);
                                } else {
                                    run.conversation_truncated = true;
                                }
                            }
                        }
                    }
                    super::model::OutputChannel::Stderr => {
                        run.stderr_tail.push_text(text);
                        for line in text.lines() {
                            run.merged_tail.push_text(&format!("stderr· {line}\n"));
                        }
                    }
                }
            }
            RunEvent::StepStarted { step_id, .. } => {
                if !run.steps.iter().any(|(id, _)| id == step_id) {
                    run.steps.push((
                        step_id.clone(),
                        StepState {
                            completed: false,
                            exit_code: None,
                            cached: false,
                        },
                    ));
                }
            }
            RunEvent::StepCompleted {
                step_id,
                exit_code,
                cached,
            } => {
                if let Some((_, step)) = run.steps.iter_mut().find(|(id, _)| id == step_id) {
                    step.completed = true;
                    step.exit_code = *exit_code;
                    step.cached = *cached;
                }
            }
            RunEvent::RunCompleted {
                exit_code,
                duration_ms,
            } => {
                // A child completion cannot prove that its descendants are
                // gone. Preserve Cancelling until the owning group observer
                // confirms exact cleanup; retain exit details meanwhile.
                if run.phase != RunPhase::Cancelling && !run.phase.is_terminal() {
                    run.phase = if *exit_code == 0 {
                        RunPhase::Succeeded
                    } else {
                        RunPhase::Failed
                    };
                }
                run.exit_code = Some(*exit_code);
                run.duration_ms = *duration_ms;
            }
            RunEvent::RunError {
                exit_code,
                message,
                duration_ms,
            } => {
                if run.phase != RunPhase::Cancelling && !run.phase.is_terminal() {
                    run.phase = RunPhase::Failed;
                    run.failure = Some(crate::ai::reliability::provider_failure(
                        sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
                        message,
                    ));
                }
                run.exit_code = *exit_code;
                run.duration_ms = *duration_ms;
            }
            RunEvent::RunCancelled { duration_ms, .. } => {
                if !run.phase.is_terminal() {
                    if let Some(cleanup) = observed_cancellation_cleanup(run) {
                        run.cancellation_cleanup = Some(cleanup);
                        run.phase = RunPhase::Cancelled;
                    } else {
                        run.phase = RunPhase::Cancelling;
                    }
                }
                run.duration_ms = *duration_ms;
            }
            RunEvent::Unknown { .. } => {}
        }
        self.bump_and_notify(&mut state);
    }

    /// Runner-thread fallback when the process dies without a terminal event
    /// (spawn failure, protocol violation, SIGKILL escalation).
    pub fn mark_failed(&self, local_id: u64, failure: crate::ai::reliability::AppFailureRecord) {
        let mut state = self.state.lock();
        if let Some(run) = state.runs.get_mut(&local_id) {
            if !run.phase.is_terminal() && run.phase != RunPhase::Cancelling {
                run.phase = RunPhase::Failed;
                run.failure = Some(failure);
                run.duration_ms = Some(run.launched_at.elapsed().as_millis() as u64);
            }
        }
        self.bump_and_notify(&mut state);
    }

    /// Settle cancellation only after a read-only signal-0 probe verifies
    /// that this run's exact owned process group no longer exists.
    pub fn mark_cancelled(&self, local_id: u64) -> bool {
        let mut state = self.state.lock();
        let Some(run) = state.runs.get_mut(&local_id) else {
            return false;
        };
        if run.phase.is_terminal() {
            return run.phase == RunPhase::Cancelled;
        }
        let Some(cleanup) = observed_cancellation_cleanup(run) else {
            return false;
        };
        run.cancellation_cleanup = Some(cleanup);
        run.phase = RunPhase::Cancelled;
        run.duration_ms = Some(run.launched_at.elapsed().as_millis() as u64);
        self.bump_and_notify(&mut state);
        true
    }

    /// Cancel was REQUESTED: the run enters the non-terminal `Cancelling`
    /// phase. `Cancelled` is only claimed once the outcome is known — via
    /// the authoritative `run.cancelled` event, the reader-thread EOF
    /// fallback, or the kill-escalation watcher (2026-07-11 audit: the old
    /// mark-Cancelled-before-SIGTERM made the receipt a lie and locked out
    /// later corrective events).
    pub fn mark_cancelling(&self, local_id: u64) {
        let mut state = self.state.lock();
        if let Some(run) = state.runs.get_mut(&local_id) {
            if !run.phase.is_terminal() {
                run.phase = RunPhase::Cancelling;
            }
        }
        self.bump_and_notify(&mut state);
    }

    /// Turn on the append-only stdout capture for a conversation-turn run.
    /// Must be called before the run's process spawns (launch_flow does).
    pub fn enable_conversation_capture(&self, local_id: u64) {
        let mut state = self.state.lock();
        if let Some(run) = state.runs.get_mut(&local_id) {
            if run.conversation_stdout.is_none() {
                run.conversation_stdout = Some(String::new());
            }
        }
    }

    /// Raw (non-protocol) stderr from the `md` child itself — out-of-band
    /// diagnostics the protocol allows on stderr. Retained on the tails so a
    /// crashing flow leaves a visible trace instead of vanishing.
    pub fn push_raw_stderr(&self, local_id: u64, line: &str) {
        let mut state = self.state.lock();
        let Some(run) = state.runs.get_mut(&local_id) else {
            return;
        };
        if run.phase.is_terminal() {
            return;
        }
        run.stderr_tail.push_text(&format!("{line}\n"));
        run.merged_tail.push_text(&format!("stderr· {line}\n"));
        self.bump_and_notify(&mut state);
    }

    /// Terminal runs that have not yet been surfaced to the user, marking
    /// them notified under the lock (exactly-once). Conversation-turn runs
    /// are excluded — their outcome settles inside the session transcript.
    pub fn take_unnotified_terminal(&self) -> Vec<FlowRun> {
        let mut state = self.state.lock();
        let mut ready: Vec<FlowRun> = Vec::new();
        for id in state.order.clone() {
            if let Some(run) = state.runs.get_mut(&id) {
                if run.phase.is_terminal() && !run.notified && run.conversation_stdout.is_none() {
                    run.notified = true;
                    ready.push(run.clone());
                }
            }
        }
        ready
    }

    pub fn record_launch_ack(&self, local_id: u64, ack_ms: u64) {
        let mut state = self.state.lock();
        if let Some(run) = state.runs.get_mut(&local_id) {
            if run.timings.launch_ack_ms.is_none() {
                run.timings.launch_ack_ms = Some(ack_ms);
            }
        }
        self.bump_and_notify(&mut state);
    }

    /// Engagement transitions never touch phase (Esc backgrounds, never
    /// cancels — protocol §4).
    pub fn set_engagement(&self, local_id: u64, engagement: EngagementMode) {
        let mut state = self.state.lock();
        if let Some(run) = state.runs.get_mut(&local_id) {
            run.engagement = engagement;
        }
        self.bump_and_notify(&mut state);
    }

    /// Record the app-spawned process-group leader. Immutable once set.
    /// Deliberately does NOT touch phase: `Running` is claimed only on the
    /// protocol's `run.started` event — a spawned pid is not a started run
    /// (2026-07-11 audit: flipping here made "Running" optimistic). Returns
    /// the phase observed under the lock so the runner can atomically detect
    /// cancel-during-spawn.
    pub fn set_pid(&self, local_id: u64, pid: u32) -> Option<RunPhase> {
        let mut state = self.state.lock();
        let mut observed = None;
        if let Some(run) = state.runs.get_mut(&local_id) {
            if run.pid.is_none() {
                run.pid = Some(pid);
            }
            if run.timings.spawn_ms.is_none() {
                run.timings.spawn_ms = Some(run.launched_at.elapsed().as_millis() as u64);
            }
            observed = Some(run.phase);
        }
        self.bump_and_notify(&mut state);
        observed
    }

    /// Record which inputs were overridden at launch (names only — values
    /// are never stored, so password redaction holds by construction).
    pub fn record_override_names(&self, local_id: u64, names: Vec<String>) {
        let mut state = self.state.lock();
        if let Some(run) = state.runs.get_mut(&local_id) {
            run.override_names = names;
        }
    }

    pub fn select(&self, local_id: u64) {
        let mut state = self.state.lock();
        if state.runs.contains_key(&local_id) {
            state.selected = Some(local_id);
            self.bump_and_notify(&mut state);
        }
    }

    pub fn selected_id(&self) -> Option<u64> {
        self.state.lock().selected
    }

    pub fn generation(&self) -> u64 {
        self.state.lock().generation
    }

    pub fn get(&self, local_id: u64) -> Option<FlowRun> {
        self.state.lock().runs.get(&local_id).cloned()
    }

    /// Snapshot in insertion order (newest last).
    pub fn snapshot(&self) -> Vec<FlowRun> {
        let state = self.state.lock();
        state
            .order
            .iter()
            .filter_map(|id| state.runs.get(id).cloned())
            .collect()
    }

    /// Cheap per-frame view of every run (no tail clones — a full
    /// `snapshot()` copies up to 64 KiB of output per run, which is far too
    /// heavy for row building on every render).
    pub fn run_summaries(&self) -> Vec<RunSummary> {
        let state = self.state.lock();
        state
            .order
            .iter()
            .filter_map(|id| state.runs.get(id))
            .map(|run| RunSummary {
                local_id: run.local_id,
                flow_id: run.flow_id.clone(),
                flow_name: run.flow_name.clone(),
                phase: run.phase,
                display_status: run.display_status(),
                elapsed_ms: run.elapsed_ms(),
                last_output_line: run.last_output_line().map(str::to_string),
                is_conversation: run.conversation_stdout.is_some(),
            })
            .collect()
    }

    pub fn active_count(&self) -> usize {
        let state = self.state.lock();
        state
            .runs
            .values()
            .filter(|r| !r.phase.is_terminal())
            .count()
    }

    /// Remove terminal runs (manager "Clear finished" action).
    pub fn clear_finished(&self) {
        let mut state = self.state.lock();
        let keep: Vec<u64> = state
            .order
            .iter()
            .copied()
            .filter(|id| {
                state
                    .runs
                    .get(id)
                    .map(|r| !r.phase.is_terminal())
                    .unwrap_or(false)
            })
            .collect();
        state.runs.retain(|id, _| keep.contains(id));
        state.order = keep;
        if let Some(selected) = state.selected {
            if !state.runs.contains_key(&selected) {
                state.selected = state.order.last().copied();
            }
        }
        self.bump_and_notify(&mut state);
    }

    #[cfg(test)]
    fn reset_for_test(&self) {
        let mut state = self.state.lock();
        *state = RegistryState::default();
    }
}

fn drop_notify(notify: &Mutex<Option<NotifyHook>>) {
    if let Some(hook) = notify.lock().as_ref() {
        hook();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::model::parse_event_line;

    fn fresh() -> FlowRunRegistry {
        FlowRunRegistry::new()
    }

    fn insert(registry: &FlowRunRegistry) -> u64 {
        registry.insert_starting(
            "project:demo",
            "demo",
            "/tmp/p/flows/demo.md",
            "/tmp/p",
            FlowUxVariant::Flash,
            EngagementMode::Inline,
        )
    }

    fn failure(raw: &str) -> crate::ai::reliability::AppFailureRecord {
        crate::ai::reliability::provider_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            raw,
        )
    }

    #[test]
    fn starting_run_is_acknowledged_before_spawn() {
        let registry = fresh();
        let id = insert(&registry);
        let run = registry.get(id).expect("run exists");
        assert_eq!(run.phase, RunPhase::Starting);
        assert_eq!(registry.selected_id(), Some(id));
    }

    #[test]
    fn event_stream_drives_phase_transitions() {
        let registry = fresh();
        let id = insert(&registry);
        for line in [
            r#"{"protocolVersion":1,"seq":0,"runId":"r-9","ts":1,"event":"protocol","mdflowVersion":"4.1.0"}"#,
            r#"{"protocolVersion":1,"seq":1,"runId":"r-9","ts":2,"event":"run.started","flowId":"project:demo","pid":42}"#,
            r#"{"protocolVersion":1,"seq":2,"runId":"r-9","ts":3,"event":"output.delta","channel":"stdout","text":"hi\n"}"#,
        ] {
            registry.apply_event(id, &parse_event_line(line).unwrap());
        }
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Running);
        // Protocol pid is the ENGINE, recorded separately; the killpg
        // target (run.pid) only ever comes from the app's own spawn.
        assert_eq!(run.engine_pid, Some(42));
        assert_eq!(run.pid, None);
        assert_eq!(run.protocol_run_id.as_deref(), Some("r-9"));
        assert_eq!(run.last_output_line(), Some("hi"));
        assert!(run.timings.first_output_ms.is_some());

        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":3,"runId":"r-9","ts":4,"event":"run.completed","exitCode":0,"durationMs":9}"#,
            )
            .unwrap(),
        );
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Succeeded);
        assert_eq!(run.duration_ms, Some(9));
    }

    #[test]
    fn nonzero_exit_is_failed_and_error_event_records_message() {
        let registry = fresh();
        let id = insert(&registry);
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-1","ts":1,"event":"run.completed","exitCode":42,"durationMs":5}"#,
            )
            .unwrap(),
        );
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Failed);
        assert_eq!(registry.get(id).unwrap().exit_code, Some(42));

        let id2 = insert(&registry);
        registry.apply_event(
            id2,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-2","ts":1,"event":"run.error","exitCode":null,"message":"boom","durationMs":3}"#,
            )
            .unwrap(),
        );
        let run2 = registry.get(id2).unwrap();
        assert_eq!(run2.phase, RunPhase::Failed);
        assert_eq!(
            run2.failure.as_ref().map(|failure| failure.failure.code),
            Some(sk_protocol::ai_reliability::AiFailureCode::Unknown)
        );
    }

    #[test]
    fn engagement_changes_never_touch_phase() {
        let registry = fresh();
        let id = insert(&registry);
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-3","ts":1,"event":"run.started","flowId":"f","pid":7}"#,
            )
            .unwrap(),
        );
        registry.set_engagement(id, EngagementMode::Background);
        let run = registry.get(id).unwrap();
        assert_eq!(run.engagement, EngagementMode::Background);
        assert_eq!(
            run.phase,
            RunPhase::Running,
            "backgrounding must not cancel"
        );
    }

    #[test]
    fn terminal_phase_is_not_overwritten_by_late_failure() {
        let registry = fresh();
        let id = insert(&registry);
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-4","ts":1,"event":"run.completed","exitCode":0,"durationMs":2}"#,
            )
            .unwrap(),
        );
        registry.mark_failed(id, failure("reader thread exit"));
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Succeeded);
    }

    #[test]
    fn multi_run_isolation_cancel_one_leaves_others() {
        let registry = fresh();
        let a = insert(&registry);
        let b = insert(&registry);
        let c = insert(&registry);
        for id in [a, b, c] {
            // No child was spawned. Invented PIDs can alias live CI process groups.
            registry.apply_event(
                id,
                &parse_event_line(
                    r#"{"protocolVersion":1,"seq":0,"runId":"r-m","ts":1,"event":"run.started","flowId":"f","pid":7}"#,
                )
                .unwrap(),
            );
            assert_eq!(registry.get(id).unwrap().phase, RunPhase::Running);
        }
        registry.mark_cancelled(b);
        assert_eq!(registry.get(a).unwrap().phase, RunPhase::Running);
        assert_eq!(registry.get(b).unwrap().phase, RunPhase::Cancelled);
        assert_eq!(registry.get(c).unwrap().phase, RunPhase::Running);
        assert_eq!(registry.active_count(), 2);
    }

    #[test]
    fn clear_finished_retains_active_and_fixes_selection() {
        let registry = fresh();
        let a = insert(&registry);
        let b = insert(&registry);
        registry.mark_cancelled(b);
        registry.select(b);
        registry.clear_finished();
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].local_id, a);
        assert_eq!(registry.selected_id(), Some(a));
        let _ = registry; // silence unused in some cfg combinations
    }

    #[test]
    fn notify_hook_fires_on_mutation() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let registry = fresh();
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_clone = hits.clone();
        registry.set_notify_hook(move || {
            hits_clone.fetch_add(1, Ordering::SeqCst);
        });
        let id = insert(&registry);
        registry.set_engagement(id, EngagementMode::Background);
        assert!(hits.load(Ordering::SeqCst) >= 2);
        registry.reset_for_test();
    }

    // ---- Cancellation-truth regression set (fusion-ultra 2026-07-09) ----

    /// A late `run.started` after cancellation must not resurrect the run
    /// as Running — the demonstrated hole that made "Cancelled" a lie.
    #[test]
    fn late_run_started_never_resurrects_cancelled_run() {
        let registry = fresh();
        let id = insert(&registry);
        registry.mark_cancelled(id);
        let started = parse_event_line(
            r#"{"protocolVersion":1,"seq":1,"runId":"r-x","ts":1,"event":"run.started","flowId":"project:demo","pid":4242}"#,
        )
        .unwrap();
        registry.apply_event(id, &started);
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Cancelled);
        assert_eq!(run.engine_pid, None, "terminal runs accept no mutations");
    }

    /// After ANY terminal phase, late output/exit events are rejected
    /// entirely (no output growth, no exit-code rewrite).
    #[test]
    fn terminal_runs_reject_all_later_lifecycle_events() {
        let registry = fresh();
        let id = insert(&registry);
        registry.mark_cancelled(id);
        for line in [
            r#"{"protocolVersion":1,"seq":2,"runId":"r-x","ts":1,"event":"output.delta","channel":"stdout","text":"late\n"}"#,
            r#"{"protocolVersion":1,"seq":3,"runId":"r-x","ts":1,"event":"run.completed","exitCode":0,"durationMs":5}"#,
            r#"{"protocolVersion":1,"seq":4,"runId":"r-x","ts":1,"event":"run.error","exitCode":143,"message":"late","durationMs":5}"#,
        ] {
            registry.apply_event(id, &parse_event_line(line).unwrap());
        }
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Cancelled);
        assert_eq!(run.exit_code, None);
        assert_eq!(run.stdout_tail.line_count(), 0);
    }

    /// `run.started`'s pid is the engine, NOT a killpg target: the
    /// app-spawned group leader recorded via set_pid must stay immutable.
    /// A spawned pid is also not a started run — phase stays Starting until
    /// the protocol says otherwise.
    #[test]
    fn protocol_pid_never_overwrites_spawned_pgid() {
        let registry = fresh();
        let id = insert(&registry);
        let observed = registry.set_pid(id, 1111);
        assert_eq!(observed, Some(RunPhase::Starting));
        let started = parse_event_line(
            r#"{"protocolVersion":1,"seq":1,"runId":"r-x","ts":1,"event":"run.started","flowId":"project:demo","pid":2222}"#,
        )
        .unwrap();
        registry.apply_event(id, &started);
        let run = registry.get(id).unwrap();
        assert_eq!(run.pid, Some(1111), "pgid is immutable once spawned");
        assert_eq!(run.engine_pid, Some(2222));
    }

    /// Cancel between spawn and pid publication: set_pid must report the
    /// terminal phase (so the runner delivers the missed signal) and must
    /// not flip Cancelled back to Running.
    #[test]
    fn set_pid_after_cancel_reports_terminal_and_keeps_cancelled() {
        let registry = fresh();
        let id = insert(&registry);
        registry.mark_cancelled(id);
        let observed = registry.set_pid(id, 3333);
        assert_eq!(observed, Some(RunPhase::Cancelled));
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Cancelled);
    }

    /// New launches must not steal manager selection from the run the user
    /// is inspecting (selection is presentation state, not lifecycle).
    #[test]
    fn new_runs_do_not_steal_existing_selection() {
        let registry = fresh();
        let first = insert(&registry);
        assert_eq!(registry.selected_id(), Some(first));
        let _second = insert(&registry);
        assert_eq!(registry.selected_id(), Some(first));
    }

    /// Completed-run retention is bounded; active runs are never evicted.
    #[test]
    fn finished_runs_evicted_over_cap_active_preserved() {
        let registry = fresh();
        let active = insert(&registry);
        for _ in 0..120 {
            let id = insert(&registry);
            registry.mark_cancelled(id);
        }
        let snapshot = registry.snapshot();
        let finished = snapshot.iter().filter(|r| r.phase.is_terminal()).count();
        assert!(
            finished <= 100,
            "finished retention cap exceeded: {finished}"
        );
        assert!(snapshot.iter().any(|r| r.local_id == active));
        registry.reset_for_test();
    }

    // ---- Cancelling truth (2026-07-11 audit) ----

    /// Child terminal events record exit details but cannot attest that every
    /// process-group descendant has exited. Only owner-observed cleanup can
    /// settle the cancellation, and cancellation is never reclassified failure.
    #[test]
    fn cancelling_child_terminal_events_wait_for_verified_group_cleanup() {
        let registry = fresh();
        let id = insert(&registry);
        registry.mark_cancelling(id);
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Cancelling);
        assert!(!RunPhase::Cancelling.is_terminal());
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-c","ts":1,"event":"run.error","exitCode":143,"message":"terminated","durationMs":5}"#,
            )
            .unwrap(),
        );
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Cancelling);
        assert_eq!(run.exit_code, Some(143), "exit code stays for transparency");
        assert!(run.execution_receipt().is_none());
        assert!(registry.mark_cancelled(id));
        let settled = registry.get(id).unwrap();
        assert_eq!(settled.phase, RunPhase::Cancelled);
        assert!(settled.execution_receipt().unwrap().cleanup_verified);

        let mut tampered = settled.clone();
        tampered
            .cancellation_cleanup
            .as_mut()
            .expect("settled cancellation owns cleanup proof")
            .process_group_id = Some(42);
        assert!(
            tampered.execution_receipt().is_none(),
            "an in-process cancellation must reject fabricated process-group cleanup"
        );

        tampered.pid = Some(7);
        assert!(
            tampered.execution_receipt().is_none(),
            "cleanup for another process group cannot settle an owned Flow"
        );

        let id2 = insert(&registry);
        registry.mark_cancelling(id2);
        registry.apply_event(
            id2,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-c2","ts":1,"event":"run.completed","exitCode":0,"durationMs":5}"#,
            )
            .unwrap(),
        );
        assert_eq!(registry.get(id2).unwrap().phase, RunPhase::Cancelling);
        assert!(registry.mark_cancelled(id2));
        assert_eq!(registry.get(id2).unwrap().phase, RunPhase::Cancelled);
    }

    /// Cancelling still accepts output (the dying process may flush) and the
    /// authoritative run.cancelled event settles it.
    #[test]
    fn cancelling_accepts_output_and_authoritative_cancelled() {
        let registry = fresh();
        let id = insert(&registry);
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-c3","ts":1,"event":"run.started","flowId":"f","pid":7}"#,
            )
            .unwrap(),
        );
        registry.mark_cancelling(id);
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":1,"runId":"r-c3","ts":2,"event":"output.delta","channel":"stdout","text":"flush\n"}"#,
            )
            .unwrap(),
        );
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":2,"runId":"r-c3","ts":3,"event":"run.cancelled","signal":"SIGTERM","durationMs":9}"#,
            )
            .unwrap(),
        );
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Cancelled);
        assert_eq!(run.stdout_tail.last_line(), Some("flush"));
        assert!(run.execution_receipt().unwrap().cleanup_verified);
    }

    #[test]
    fn live_owned_process_group_refuses_every_optimistic_cancellation_claim() {
        let registry = fresh();
        let id = insert(&registry);
        // SAFETY: getpgrp only reads this process's group; this test never
        // signals, mutates, or terminates it.
        let live_group = unsafe { libc::getpgrp() } as u32;
        registry.set_pid(id, live_group);
        registry.mark_cancelling(id);

        assert!(super::super::runner::process_group_alive(live_group));
        assert!(!registry.mark_cancelled(id));
        assert_eq!(registry.get(id).unwrap().phase, RunPhase::Cancelling);

        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"live-group","ts":1,"event":"run.cancelled","signal":"SIGTERM","durationMs":9}"#,
            )
            .unwrap(),
        );
        let run = registry.get(id).unwrap();
        assert_eq!(run.phase, RunPhase::Cancelling);
        assert!(run.cancellation_cleanup.is_none());
        assert!(run.execution_receipt().is_none());
    }

    #[test]
    fn terminal_execution_receipts_never_expose_private_output_or_failure_text() {
        let registry = fresh();
        let id = insert(&registry);
        registry.mark_failed(id, failure("private-provider-token-super-secret"));
        let run = registry.get(id).unwrap();
        let receipt = run.execution_receipt().expect("typed terminal receipt");
        let json = serde_json::to_string(&receipt).expect("serialize safe receipt");

        assert_eq!(receipt.command_id.as_str(), "flow/project:demo");
        assert_eq!(
            receipt.outcome,
            sk_protocol::execution_contract::ExecutionPhase::Failed
        );
        assert!(!json.contains("private-provider-token"));
        assert!(!json.contains("/tmp/p/flows"));
    }

    // ---- Conversation capture (2026-07-11 audit P0: cursor corruption) ----

    /// The conversation accumulator is append-only: it never front-evicts
    /// (byte cursors into it stay valid), and at its cap it freezes and
    /// flags truncation instead of shifting earlier bytes.
    #[test]
    fn conversation_capture_is_append_only_and_freezes_at_cap() {
        let registry = fresh();
        let id = insert(&registry);
        registry.enable_conversation_capture(id);
        registry.apply_event(
            id,
            &parse_event_line(
                r#"{"protocolVersion":1,"seq":0,"runId":"r-v","ts":1,"event":"run.started","flowId":"f","pid":7}"#,
            )
            .unwrap(),
        );
        // Push far more than the display tail can hold; the capture keeps
        // every byte while the tail front-evicts.
        let chunk = "x".repeat(8 * 1024);
        for i in 0..12 {
            let text = format!("{i}:{chunk}\n");
            let event = crate::flows::model::RunEventEnvelope {
                seq: 1 + i,
                run_id: "r-v".into(),
                ts: 2,
                event: RunEvent::OutputDelta {
                    channel: crate::flows::model::OutputChannel::Stdout,
                    text,
                },
            };
            registry.apply_event(id, &event);
        }
        let run = registry.get(id).unwrap();
        let capture = run.conversation_stdout.as_ref().unwrap();
        assert!(capture.starts_with("0:"), "capture never evicts the front");
        assert!(capture.len() > crate::flows::model::OUTPUT_TAIL_MAX_BYTES);
        assert!(!run.conversation_truncated);

        // Exceed the capture cap: the buffer freezes (no partial append).
        let len_before = capture.len();
        let giant = "y".repeat(crate::flows::model::CONVERSATION_CAPTURE_MAX_BYTES);
        let event = crate::flows::model::RunEventEnvelope {
            seq: 13,
            run_id: "r-v".into(),
            ts: 3,
            event: RunEvent::OutputDelta {
                channel: crate::flows::model::OutputChannel::Stdout,
                text: giant,
            },
        };
        registry.apply_event(id, &event);
        let run = registry.get(id).unwrap();
        assert!(run.conversation_truncated);
        assert_eq!(
            run.conversation_stdout.as_ref().unwrap().len(),
            len_before,
            "capture freezes at the cap — cursors stay valid"
        );
    }

    // ---- Terminal notifications (exactly-once) ----

    #[test]
    fn unnotified_terminal_runs_surface_exactly_once_and_skip_conversations() {
        let registry = fresh();
        let bare = insert(&registry);
        let convo = insert(&registry);
        registry.enable_conversation_capture(convo);
        registry.mark_failed(bare, failure("boom"));
        registry.mark_failed(convo, failure("boom"));

        let first = registry.take_unnotified_terminal();
        assert_eq!(first.len(), 1, "conversation runs settle in-transcript");
        assert_eq!(first[0].local_id, bare);
        assert!(
            registry.take_unnotified_terminal().is_empty(),
            "second take must be empty — exactly-once"
        );

        let active = insert(&registry);
        assert!(registry.take_unnotified_terminal().is_empty());
        registry.mark_cancelled(active);
        assert_eq!(registry.take_unnotified_terminal().len(), 1);
        registry.reset_for_test();
    }
}
