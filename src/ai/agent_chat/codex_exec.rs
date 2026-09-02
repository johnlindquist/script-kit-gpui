//! Cold, one-shot Codex exec adapter used only by the hidden Quick AI profile.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context as _, Result};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use super::codex_exec_protocol::{
    parse_codex_exec_line, CodexExecEvent, CodexItem, ItemPhase, WebSearchAction, WebSearchItem,
};
use crate::ai::agent_chat::content::ContentBlock;
use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx, AgentChatModelEntry};
use crate::ai::agent_chat::runtime::{AgentChatConnection, AgentChatTurnRequest};

pub(crate) const QUICK_AI_SELECTED_MODEL_ID: &str = "openai-codex/gpt-5.3-codex-spark";
const EVENT_CHANNEL_CAPACITY: usize = 64;
const CANCEL_POLL: Duration = Duration::from_millis(20);
const QUICK_AI_TOTAL_COMPLETION_BUDGET: Duration = Duration::from_millis(12_000);
const QUICK_AI_TEARDOWN_RESERVE: Duration = Duration::from_millis(350);
const QUICK_AI_WORK_DEADLINE: Duration = Duration::from_millis(
    QUICK_AI_TOTAL_COMPLETION_BUDGET.as_millis() as u64
        - QUICK_AI_TEARDOWN_RESERVE.as_millis() as u64,
);
const QUICK_AI_MAX_ANSWER_CHARS: usize = 1_200;
const QUICK_AI_MAX_SOURCES: usize = 3;
// Codex structured output supports this strict subset. Length, count,
// canonical-URL, and uniqueness constraints are enforced below by the app.
const QUICK_AI_OUTPUT_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"properties":{"answer":{"type":"string"},"sources":{"type":"array","items":{"type":"string"}}},"required":["answer","sources"]}"#;
const QUICK_AI_WEB_ROW_ID: &str = "quick-ai-web-result";

#[derive(Debug, Clone)]
pub(crate) struct CodexQuickAiExecSpec {
    pub(crate) binary: PathBuf,
    pub(crate) model: String,
    pub(crate) selected_model_id: String,
    pub(crate) developer_instructions: String,
    pub(crate) scratch_root: PathBuf,
    pub(crate) trace_path: Option<PathBuf>,
    pub(crate) work_deadline: Duration,
}

impl CodexQuickAiExecSpec {
    pub(crate) fn from_builtin_contract(scratch_root: PathBuf) -> Self {
        Self {
            binary: codex_binary(),
            model: crate::ai::agent_chat::profiles::QUICK_AI_PI_MODEL.to_string(),
            selected_model_id: QUICK_AI_SELECTED_MODEL_ID.to_string(),
            developer_instructions: crate::ai::agent_chat::profiles::QUICK_AI_APPEND_SYSTEM_PROMPT
                .to_string(),
            scratch_root,
            trace_path: std::env::var_os("SCRIPT_KIT_QUICK_AI_TRACE_PATH").map(PathBuf::from),
            work_deadline: QUICK_AI_WORK_DEADLINE,
        }
    }
}

pub(crate) struct CodexQuickAiExecConnection {
    spec: CodexQuickAiExecSpec,
    next_generation: AtomicU64,
    active_turns: Arc<Mutex<HashMap<String, ActiveExecTurn>>>,
}

#[derive(Clone)]
struct ActiveExecTurn {
    generation: u64,
    #[allow(dead_code)]
    pid: u32,
    #[allow(dead_code)]
    pgid: i32,
    cancel_requested: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuickAiStartupCleanupPlan {
    terminate_owned_group: bool,
    remove_owned_scratch: bool,
    release_active_turn: bool,
}

fn plan_quick_ai_startup_cleanup(
    expected_generation: u64,
    active_generation: Option<u64>,
    owns_child: bool,
    cleanup_verified: bool,
    worker_owns_process: bool,
) -> QuickAiStartupCleanupPlan {
    if worker_owns_process {
        return QuickAiStartupCleanupPlan {
            terminate_owned_group: false,
            remove_owned_scratch: false,
            release_active_turn: false,
        };
    }

    QuickAiStartupCleanupPlan {
        terminate_owned_group: owns_child && !cleanup_verified,
        remove_owned_scratch: cleanup_verified,
        release_active_turn: cleanup_verified && active_generation == Some(expected_generation),
    }
}

fn release_owned_quick_ai_turn(
    turns: &mut HashMap<String, ActiveExecTurn>,
    ui_thread_id: &str,
    generation: u64,
    cleanup_verified: bool,
) -> bool {
    let active_generation = turns.get(ui_thread_id).map(|turn| turn.generation);
    let plan = plan_quick_ai_startup_cleanup(
        generation,
        active_generation,
        false,
        cleanup_verified,
        false,
    );
    if plan.release_active_turn {
        turns.remove(ui_thread_id);
        return true;
    }
    false
}

struct QuickAiStartupGuard {
    active_turns: Arc<Mutex<HashMap<String, ActiveExecTurn>>>,
    ui_thread_id: String,
    generation: u64,
    scratch: Option<PathBuf>,
    child: Option<Child>,
    registration: Option<crate::process_manager::ChildRegistration>,
    pgid: Option<i32>,
    worker_owns_process: bool,
}

struct QuickAiWorkerOwnership {
    child: Child,
    registration: crate::process_manager::ChildRegistration,
    scratch: PathBuf,
}

impl QuickAiStartupGuard {
    fn reserve(
        active_turns: Arc<Mutex<HashMap<String, ActiveExecTurn>>>,
        ui_thread_id: String,
        generation: u64,
        cancel_requested: Arc<AtomicBool>,
    ) -> Result<Self> {
        {
            let mut turns = active_turns
                .lock()
                .map_err(|_| anyhow!("quick_ai_active_turn_lock_poisoned"))?;
            if turns.contains_key(&ui_thread_id) {
                bail!("quick_ai_turn_already_active");
            }
            turns.insert(
                ui_thread_id.clone(),
                ActiveExecTurn {
                    generation,
                    pid: 0,
                    pgid: 0,
                    cancel_requested,
                },
            );
        }

        Ok(Self {
            active_turns,
            ui_thread_id,
            generation,
            scratch: None,
            child: None,
            registration: None,
            pgid: None,
            worker_owns_process: false,
        })
    }

    fn adopt_child(
        &mut self,
        child: Child,
        registration: crate::process_manager::ChildRegistration,
        pgid: i32,
    ) -> Result<()> {
        let pid = child.id();
        self.child = Some(child);
        self.registration = Some(registration);
        self.pgid = Some(pgid);

        let mut turns = self
            .active_turns
            .lock()
            .map_err(|_| anyhow!("quick_ai_active_turn_lock_poisoned"))?;
        let Some(turn) = turns.get_mut(&self.ui_thread_id) else {
            bail!("quick_ai_startup_reservation_missing");
        };
        if turn.generation != self.generation {
            bail!("quick_ai_startup_reservation_replaced");
        }
        turn.pid = pid;
        turn.pgid = pgid;
        Ok(())
    }

    fn child_mut(&mut self) -> Result<&mut Child> {
        self.child
            .as_mut()
            .ok_or_else(|| anyhow!("quick_ai_startup_child_missing"))
    }

    fn transfer_to_worker(mut self) -> Option<QuickAiWorkerOwnership> {
        match (
            self.child.take(),
            self.registration.take(),
            self.scratch.take(),
        ) {
            (Some(child), Some(registration), Some(scratch)) => {
                self.worker_owns_process = true;
                Some(QuickAiWorkerOwnership {
                    child,
                    registration,
                    scratch,
                })
            }
            (child, registration, scratch) => {
                self.child = child;
                self.registration = registration;
                self.scratch = scratch;
                None
            }
        }
    }
}

impl Drop for QuickAiStartupGuard {
    fn drop(&mut self) {
        if self.worker_owns_process {
            return;
        }

        let active_generation = self
            .active_turns
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&self.ui_thread_id)
            .map(|turn| turn.generation);
        let initial_plan = plan_quick_ai_startup_cleanup(
            self.generation,
            active_generation,
            self.child.is_some(),
            false,
            false,
        );
        let cleanup_verified = if initial_plan.terminate_owned_group {
            match (self.child.as_mut(), self.pgid) {
                (Some(child), Some(pgid))
                    if pgid > 0 && u32::try_from(pgid).ok() == Some(child.id()) =>
                {
                    terminate_and_reap_process_group(child, pgid, QUICK_AI_FAST_TEARDOWN)
                        .is_ok_and(|report| report.child_reaped && !report.process_group_alive)
                }
                _ => false,
            }
        } else {
            self.child.is_none()
        };
        let plan = plan_quick_ai_startup_cleanup(
            self.generation,
            active_generation,
            self.child.is_some(),
            cleanup_verified,
            false,
        );

        if plan.remove_owned_scratch {
            if let Some(path) = self.scratch.take() {
                if let Err(error) = std::fs::remove_dir_all(&path) {
                    if error.kind() != std::io::ErrorKind::NotFound {
                        let safe_error = crate::logging::log_private_user_value(&error.to_string());
                        tracing::warn!(
                            target: "script_kit::quick_ai",
                            event = "quick_ai_startup_scratch_cleanup_failed",
                            generation = self.generation,
                            error_bytes = safe_error.raw_bytes,
                            error_sha256 = %safe_error.sha256,
                        );
                    }
                }
            }
        }

        if plan.release_active_turn {
            let mut turns = self
                .active_turns
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            release_owned_quick_ai_turn(
                &mut turns,
                &self.ui_thread_id,
                self.generation,
                cleanup_verified,
            );
        } else if !cleanup_verified {
            tracing::error!(
                target: "script_kit::quick_ai",
                event = "quick_ai_startup_cleanup_unverified",
                generation = self.generation,
            );
        }
    }
}

impl CodexQuickAiExecConnection {
    pub(crate) fn new(spec: CodexQuickAiExecSpec) -> Self {
        Self {
            spec,
            next_generation: AtomicU64::new(1),
            active_turns: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Drop for CodexQuickAiExecConnection {
    fn drop(&mut self) {
        if let Ok(turns) = self.active_turns.lock() {
            for turn in turns.values() {
                turn.cancel_requested.store(true, Ordering::Release);
            }
        }
    }
}

impl AgentChatConnection for CodexQuickAiExecConnection {
    fn start_turn(
        &self,
        request: AgentChatTurnRequest,
    ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
        (|| -> Result<AgentChatEventRx> {
            let turn_started = Instant::now();
            // WP-B2: this cold adapter serves ONLY the web-search-only Quick AI
            // profile. Refuse any turn whose session policy would grant broader
            // tools — the backend allowlist is web-search-only and must never be
            // driven by a Full-policy request reaching this path.
            if request.tool_policy
                != crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy::WebSearchOnly
            {
                bail!("quick_ai_requires_web_search_only_tool_policy")
            }
            let query = extract_zero_context_query(&request, &self.spec.selected_model_id)?;
            let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
            let cancel_requested = Arc::new(AtomicBool::new(false));
            let mut startup = QuickAiStartupGuard::reserve(
                Arc::clone(&self.active_turns),
                request.ui_thread_id.clone(),
                generation,
                cancel_requested.clone(),
            )?;

            let run_id = format!(
                "quick-ai-{}-{generation}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            let trace = TraceSink::new(
                self.spec.trace_path.clone(),
                run_id.clone(),
                turn_started,
            );
            trace.write(
                "start_turn_entered",
                json!({
                    "backend": "codex-direct",
                    "profileId": "quick-ai",
                    "modelClass": "spark",
                }),
            );
            std::fs::create_dir_all(&self.spec.scratch_root).with_context(|| {
                format!(
                    "quick_ai_scratch_root_create_failed:{}",
                    self.spec.scratch_root.display()
                )
            })?;
            let turn_cwd = self.spec.scratch_root.join(&run_id);
            std::fs::create_dir(&turn_cwd).with_context(|| {
                format!("quick_ai_turn_cwd_create_failed:{}", turn_cwd.display())
            })?;
            startup.scratch = Some(turn_cwd.clone());
            trace.write("scratch_prepared", json!({}));

            let mut command = build_codex_exec_command(&self.spec, &turn_cwd, &query)?;
            trace.write("spawn_started", json!({}));
            let child = command.spawn().with_context(|| {
                format!("quick_ai_codex_spawn_failed:{}", self.spec.binary.display())
            })?;
            let pid = child.id();
            let pgid = pid as i32;
            let registration = crate::process_manager::ChildRegistration::register(
                pid,
                &self.spec.binary.to_string_lossy(),
            );
            startup.adopt_child(child, registration, pgid)?;
            let stdout = startup
                .child_mut()?
                .stdout
                .take()
                .context("quick_ai_codex_stdout_unavailable")?;
            let stderr = startup
                .child_mut()?
                .stderr
                .take()
                .context("quick_ai_codex_stderr_unavailable")?;
            trace.write(
                "spawned",
                json!({
                    "backend": "codex-exec",
                    "profileId": "quick-ai",
                    "model": self.spec.model,
                    "selectedModelId": self.spec.selected_model_id,
                    "promptSha256": sha256_hex(&self.spec.developer_instructions),
                    "allowedCapabilities": ["public-web-retrieval"],
                    "nativeSearchEnabled": true,
                    "sandbox": "read-only",
                    "ephemeral": true,
                    "ignoreUserConfig": true,
                    "ignoreRules": true,
                    "stdinNull": true,
                    "inputBlockCount": 1,
                    "textBlockCount": 1,
                    "imageBlockCount": 0,
                    "querySha256": crate::logging::log_private_user_value(&query).sha256,
                    "queryChars": query.chars().count(),
                    "pid": pid,
                    "pgid": pgid,
                    "startTurnToSpawnMs": turn_started.elapsed().as_millis() as u64,
                }),
            );

            let (line_tx, line_rx) = std::sync::mpsc::sync_channel::<Result<String, String>>(128);
            std::thread::Builder::new()
                .name(format!("quick-ai-jsonl-{generation}"))
                .spawn(move || {
                    for line in BufReader::new(stdout).lines() {
                        let value = line.map_err(|error| error.to_string());
                        if line_tx.send(value).is_err() {
                            break;
                        }
                    }
                })
                .context("quick_ai_stdout_reader_spawn_failed")?;
            let stderr_thread = std::thread::Builder::new()
                .name(format!("quick-ai-stderr-{generation}"))
                .spawn(move || {
                    let mut text = String::new();
                    let _ = BufReader::new(stderr)
                        .take(64 * 1024)
                        .read_to_string(&mut text);
                    text
                })
                .context("quick_ai_stderr_reader_spawn_failed")?;

            let (event_tx, event_rx) = async_channel::bounded(EVENT_CHANNEL_CAPACITY);
            let active_turns = Arc::clone(&self.active_turns);
            let ui_thread_id = request.ui_thread_id;
            let work_deadline = self.spec.work_deadline;
            std::thread::Builder::new()
                .name(format!("quick-ai-turn-{generation}"))
                .spawn(move || {
                    let Some(QuickAiWorkerOwnership {
                        mut child,
                        registration,
                        scratch,
                    }) = startup.transfer_to_worker()
                    else {
                        return;
                    };
                    let _registration = registration;
                    let mut accumulator = CodexExecTurnAccumulator::new(run_id.clone());
                    let deadline = turn_started + work_deadline;
                    let mut parent_exit_teardown = None;
                    let mut stop_reason = loop {
                        if cancel_requested.load(Ordering::Acquire) || event_tx.is_closed() {
                            break QuickAiTurnStop::Cancelled;
                        }
                        let now = Instant::now();
                        if now >= deadline {
                            let failure = crate::ai::reliability::quick_ai_deadline_failure(
                                work_deadline.as_millis().min(u32::MAX as u128) as u32,
                                completed_focused_search_count(&accumulator),
                                partial_answer(&accumulator).is_some(),
                                all_recovery_source_count(&accumulator),
                            );
                            trace.write(
                                "deadline_expired",
                                json!({
                                    "deadlineMs": work_deadline.as_millis() as u64,
                                    "completedSearches": completed_focused_search_count(&accumulator),
                                    "partialAnswerAvailable": partial_answer(&accumulator).is_some(),
                                    "sourceCount": all_recovery_source_count(&accumulator),
                                }),
                            );
                            break QuickAiTurnStop::DeadlineRecovery(failure);
                        }
                        let poll = CANCEL_POLL.min(deadline.saturating_duration_since(now));
                        match line_rx.recv_timeout(poll) {
                            Ok(Ok(line)) => {
                                if line.trim().is_empty() {
                                    break QuickAiTurnStop::Failed(CodexTurnFailure::protocol(
                                        "quick_ai_codex_empty_jsonl_line",
                                    ));
                                }
                                let event = match parse_codex_exec_line(&line) {
                                    Ok(event) => event,
                                    Err(error) => {
                                        break QuickAiTurnStop::Failed(CodexTurnFailure::protocol(
                                            error.to_string(),
                                        ));
                                    }
                                };
                                trace_event_for_protocol(&trace, &event);
                                let permit_before = accumulator.web_budget.permit_reserved;
                                let completed_before = accumulator.web_budget.search_completed;
                                match apply_codex_exec_event(&mut accumulator, event) {
                                    Ok(CodexEventDecision::Continue(events)) => {
                                        if !permit_before && accumulator.web_budget.permit_reserved {
                                            trace.write("search_permit_reserved", json!({"permit": 1}));
                                        }
                                        if !completed_before && accumulator.web_budget.search_completed {
                                            trace.write("search_completed", json!({"permit": 1}));
                                        }
                                        let mut send_stop = None;
                                        for event in events {
                                            match event_tx.try_send(event) {
                                                Ok(()) => {}
                                                Err(async_channel::TrySendError::Closed(_)) => {
                                                    send_stop = Some(QuickAiTurnStop::Cancelled);
                                                    break;
                                                }
                                                Err(async_channel::TrySendError::Full(_)) => {
                                                    send_stop = Some(QuickAiTurnStop::Failed(
                                                        CodexTurnFailure::protocol(
                                                            "quick_ai_event_channel_backpressure",
                                                        ),
                                                    ));
                                                    break;
                                                }
                                            }
                                        }
                                        if let Some(stop) = send_stop {
                                            break stop;
                                        }
                                        if accumulator.terminal_seen {
                                            break QuickAiTurnStop::ProviderTerminal;
                                        }
                                    }
                                    Ok(CodexEventDecision::CompleteEarly(answer)) => {
                                        trace.write(
                                            "answer_candidate",
                                            json!({
                                                "answerSha256": private_trace_fingerprint(&answer.rendered),
                                                "answerChars": answer.rendered.chars().count(),
                                                "sourceCount": answer.source_count,
                                            }),
                                        );
                                        trace.write("early_finalization_selected", json!({}));
                                        break QuickAiTurnStop::EarlySuccess(answer);
                                    }
                                    Ok(CodexEventDecision::StopForRecovery(record)) => {
                                        trace.write(
                                            "excess_web_action_observed",
                                            json!({
                                                "actionOrdinal": accumulator.web_budget.excess_action_count.saturating_add(1),
                                                "completedSearches": completed_focused_search_count(&accumulator),
                                            }),
                                        );
                                        break QuickAiTurnStop::PolicyRecovery(record);
                                    }
                                    Err(error) => break QuickAiTurnStop::Failed(error),
                                }
                            }
                            Ok(Err(error)) => {
                                break QuickAiTurnStop::Failed(CodexTurnFailure::protocol(format!(
                                    "quick_ai_codex_stdout_read_failed:{error}"
                                )));
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                // A descendant can retain stdout after its parent exits.
                                // Close those writers through the owned-group teardown,
                                // but let the reader deliver buffered JSON before EOF.
                                if parent_exit_teardown.is_none()
                                    && matches!(child.try_wait(), Ok(Some(_)))
                                {
                                    trace.write("teardown_started", json!({}));
                                    parent_exit_teardown = Some(
                                        terminate_and_reap_process_group(
                                            &mut child,
                                            pgid,
                                            USER_CANCEL_TEARDOWN,
                                        )
                                        .unwrap_or_else(|error| {
                                            ProcessTeardownReport::failed(error.to_string())
                                        }),
                                    );
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                if accumulator.terminal_seen {
                                    break QuickAiTurnStop::ProviderTerminal;
                                }
                                break QuickAiTurnStop::Failed(CodexTurnFailure::protocol(
                                    "quick_ai_codex_eof_without_terminal",
                                ));
                            }
                        }
                    };

                    let teardown = parent_exit_teardown.unwrap_or_else(|| {
                        let teardown_policy = match &stop_reason {
                            QuickAiTurnStop::EarlySuccess(_)
                            | QuickAiTurnStop::PolicyRecovery(_)
                            | QuickAiTurnStop::DeadlineRecovery(_) => QUICK_AI_FAST_TEARDOWN,
                            QuickAiTurnStop::ProviderTerminal
                            | QuickAiTurnStop::Cancelled
                            | QuickAiTurnStop::Failed(_) => USER_CANCEL_TEARDOWN,
                        };
                        trace.write("teardown_started", json!({}));
                        terminate_and_reap_process_group(&mut child, pgid, teardown_policy)
                            .unwrap_or_else(|error| ProcessTeardownReport::failed(error.to_string()))
                    });
                    trace.write(
                        "teardown",
                        serde_json::to_value(&teardown).unwrap_or(Value::Null),
                    );
                    let cleanup_verified = teardown.child_reaped && !teardown.process_group_alive;
                    // A refused signal can leave stderr open in a live child.
                    let stderr_text = if cleanup_verified || stderr_thread.is_finished() {
                        stderr_thread.join().unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if !cleanup_verified {
                        stop_reason = QuickAiTurnStop::Failed(CodexTurnFailure::protocol(
                            "quick_ai_codex_process_teardown_incomplete",
                        ));
                    } else if matches!(stop_reason, QuickAiTurnStop::ProviderTerminal)
                        && (teardown.exit_code != Some(0) || teardown.exit_signal.is_some())
                    {
                        stop_reason = QuickAiTurnStop::Failed(CodexTurnFailure::protocol(format!(
                            "quick_ai_codex_nonzero_exit:code={:?}:signal={:?}",
                            teardown.exit_code, teardown.exit_signal
                        )));
                    }
                    if cleanup_verified {
                        let _ = std::fs::remove_dir_all(&scratch);
                        if let Ok(mut turns) = active_turns.lock() {
                            release_owned_quick_ai_turn(
                                &mut turns,
                                &ui_thread_id,
                                generation,
                                true,
                            );
                        }
                    }

                    match stop_reason {
                        QuickAiTurnStop::Cancelled => {
                            emit_running_search_failures(
                                &event_tx,
                                &accumulator,
                                "Cancelled",
                                false,
                            );
                            trace.write("terminal", json!({"kind": "cancelled"}));
                            let _ = event_tx.send_blocking(AgentChatEvent::TurnCompleted {
                                outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Cancelled {
                                    kind: sk_protocol::ai_reliability::CancellationKind::UserCancelled,
                                    partial: sk_protocol::ai_reliability::PartialOutputState::None,
                                },
                            });
                        }
                        QuickAiTurnStop::PolicyRecovery(recovery)
                        | QuickAiTurnStop::DeadlineRecovery(recovery) => {
                            if let Some(answer) = partial_answer(&accumulator) {
                                let _ = event_tx
                                    .send_blocking(AgentChatEvent::AgentMessageDelta(answer));
                            }
                            emit_running_search_failures(
                                &event_tx,
                                &accumulator,
                                recovery.primary_message(),
                                false,
                            );
                            let failure_code = format!("{:?}", recovery.failure.code);
                            trace.write(
                                "policy_recovery",
                                json!({
                                    "failureCode": failure_code,
                                    "completedSearches": completed_focused_search_count(&accumulator),
                                    "searchBudget": crate::ai::agent_chat::profiles::QUICK_AI_FOCUSED_SEARCH_BUDGET,
                                    "partialAnswerAvailable": partial_answer(&accumulator).is_some(),
                                    "sourceCount": all_recovery_source_count(&accumulator),
                                }),
                            );
                            trace.write(
                                "terminal",
                                json!({"kind": "recovery", "failureCode": failure_code}),
                            );
                            let _ = event_tx
                                .send_blocking(AgentChatEvent::TurnFailed { failure: recovery });
                        }
                        QuickAiTurnStop::EarlySuccess(answer) => {
                            emit_successful_answer(
                                &event_tx,
                                &trace,
                                &accumulator,
                                answer.rendered,
                                "early-structured-answer",
                            );
                        }
                        QuickAiTurnStop::ProviderTerminal => {
                            match finalize_successful_turn(&accumulator) {
                                Ok(events) => emit_successful_events(
                                    &event_tx,
                                    &trace,
                                    &accumulator,
                                    events,
                                    "provider-terminal",
                                ),
                                Err(error) => emit_codex_failure(
                                    &event_tx,
                                    &trace,
                                    &accumulator,
                                    error,
                                    &stderr_text,
                                ),
                            }
                        }
                        QuickAiTurnStop::Failed(error) => emit_codex_failure(
                            &event_tx,
                            &trace,
                            &accumulator,
                            error,
                            &stderr_text,
                        ),
                    }
                })
                .context("quick_ai_worker_spawn_failed")?;
            Ok(event_rx)
        })()
        .map_err(Into::into)
    }

    fn cancel_turn(&self, ui_thread_id: String) -> crate::ai::reliability::AiAdapterResult<()> {
        (|| -> Result<()> {
            if let Some(turn) = self
                .active_turns
                .lock()
                .map_err(|_| anyhow!("quick_ai_active_turn_lock_poisoned"))?
                .get(&ui_thread_id)
            {
                turn.cancel_requested.store(true, Ordering::Release);
            }
            Ok(())
        })()
        .map_err(Into::into)
    }

    fn prepare_session(
        &self,
        _ui_thread_id: String,
        _cwd: PathBuf,
    ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
        let (tx, rx) = async_channel::bounded(1);
        let _ = tx.try_send(AgentChatEvent::ModelsAvailable {
            current_model_id: Some(self.spec.selected_model_id.clone()),
            models: vec![AgentChatModelEntry {
                id: self.spec.selected_model_id.clone(),
                display_name: Some(self.spec.model.clone()),
                context_window: None,
            }],
        });
        Ok(rx)
    }
}

pub(crate) fn codex_binary() -> PathBuf {
    std::env::var_os("SCRIPT_KIT_CODEX_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"))
}

pub(crate) fn build_codex_exec_command(
    spec: &CodexQuickAiExecSpec,
    turn_cwd: &Path,
    query: &str,
) -> Result<Command> {
    if spec.model != crate::ai::agent_chat::profiles::QUICK_AI_PI_MODEL
        || spec.selected_model_id != QUICK_AI_SELECTED_MODEL_ID
        || spec.developer_instructions
            != crate::ai::agent_chat::profiles::QUICK_AI_APPEND_SYSTEM_PROMPT
        || query.trim().is_empty()
    {
        bail!("quick_ai_codex_command_contract_mismatch")
    }
    let output_schema_path = turn_cwd.join("quick-ai-output-schema.json");
    std::fs::write(&output_schema_path, QUICK_AI_OUTPUT_SCHEMA).with_context(|| {
        format!(
            "quick_ai_output_schema_write_failed:{}",
            output_schema_path.display()
        )
    })?;
    let mut command = Command::new(&spec.binary);
    command
        .arg("--search")
        .arg("--model")
        .arg(&spec.model)
        .arg("--sandbox")
        .arg("read-only")
        .arg("--cd")
        .arg(turn_cwd)
        .arg("--disable")
        .arg("plugins")
        .arg("--config")
        .arg("skills.bundled.enabled=false")
        .arg("--config")
        .arg("model_reasoning_effort=\"low\"")
        .arg("--config")
        .arg("tools.web_search.context_size=\"low\"")
        // Quick AI accepts exactly one tool: web_search. Everything else is a
        // `CodexItem::Forbidden` that kills the turn, so the tools must be
        // taken away rather than rejected after the fact.
        //
        // `--sandbox read-only` is not enough: a read-only sandbox happily
        // runs shell commands that only read. A captured production stream
        // (`testdata/quick-ai-streams/rust-release-2.ndjson`) shows the model
        // running `/bin/zsh -lc 'recall context'` — the user's shared agent
        // memory — while answering a question about Rust releases.
        //
        // Measured with `scripts/agentic/quick-ai-shell-tool-gate-probe.ts`
        // against a query written to provoke a shell command, 4 reps per arm:
        //   control                 4/4 turns ran a forbidden tool, median 5.6s
        //   + features.shell_tool   4/4 (shell gone; model switched to an
        //                           `mcp_tool_call`), median 11.5s
        //   + apps/MCP surfaces too 0/4, median 4.9s — and the only item type
        //                           left in the stream is `agent_message`
        // Gating only the shell is worse than gating nothing: the model reaches
        // for the next forbidden tool and pays 3x the latency to do it.
        //
        // The MCP call is Codex's own `list_mcp_resources` over `codex_apps`
        // connectors, so `mcp_servers={}` alone cannot remove it. Note
        // `features.connectors=false` is deliberately absent: Codex answers it
        // with a deprecation `error` item, which Quick AI treats as a protocol
        // failure. `features.apps` is its replacement.
        .arg("--config")
        .arg("features.shell_tool=false")
        .arg("--config")
        .arg("mcp_servers={}")
        .arg("--config")
        .arg("features.enable_mcp_apps=false")
        .arg("--config")
        .arg("features.apps=false")
        .arg("--config")
        .arg("features.tool_search=false")
        .arg("--config")
        .arg(format!(
            "developer_instructions={}",
            serde_json::to_string(&spec.developer_instructions)?
        ))
        .arg("exec")
        .arg("--ephemeral")
        .arg("--ignore-user-config")
        .arg("--ignore-rules")
        .arg("--skip-git-repo-check")
        .arg("--output-schema")
        .arg(&output_schema_path)
        .arg("--json")
        .arg(query)
        .current_dir(turn_cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    Ok(command)
}

pub(crate) fn extract_zero_context_query(
    request: &AgentChatTurnRequest,
    expected_model_id: &str,
) -> Result<String> {
    if request.model_id.as_deref() != Some(expected_model_id) {
        bail!("quick_ai_requires_exact_model")
    }
    let [ContentBlock::Text(text)] = request.blocks.as_slice() else {
        bail!("quick_ai_requires_single_text_block")
    };
    let query = text.text.trim();
    if query.is_empty() {
        bail!("quick_ai_requires_nonempty_text")
    }
    Ok(query.to_string())
}

#[derive(Debug)]
struct CodexExecTurnAccumulator {
    #[allow(dead_code)]
    run_id: String,
    terminal_seen: bool,
    turn_completed_seen: bool,
    search_items: HashMap<String, WebSearchState>,
    search_order: Vec<String>,
    structured_urls: Vec<String>,
    structured_url_keys: HashSet<String>,
    recovery_only_urls: Vec<String>,
    recovery_only_url_keys: HashSet<String>,
    completed_agent_messages: Vec<CompletedAgentMessage>,
    completed_agent_ids: HashSet<String>,
    web_budget: QuickAiWebBudgetState,
    non_search_tool_count: usize,
}

#[derive(Debug, Clone)]
struct WebSearchState {
    started: bool,
    completed: bool,
    urls: Vec<String>,
    tool_started_emitted: bool,
    tool_completed_emitted: bool,
}

#[derive(Debug, Default)]
struct QuickAiWebBudgetState {
    admitted_item_id: Option<String>,
    admitted_query_key: Option<String>,
    permit_reserved: bool,
    search_started: bool,
    search_completed: bool,
    excess_action_count: u8,
}

#[derive(Debug)]
struct CompletedAgentMessage {
    #[allow(dead_code)]
    item_id: String,
    text: String,
}

impl CodexExecTurnAccumulator {
    fn new(run_id: String) -> Self {
        Self {
            run_id,
            terminal_seen: false,
            turn_completed_seen: false,
            search_items: HashMap::new(),
            search_order: Vec::new(),
            structured_urls: Vec::new(),
            structured_url_keys: HashSet::new(),
            recovery_only_urls: Vec::new(),
            recovery_only_url_keys: HashSet::new(),
            completed_agent_messages: Vec::new(),
            completed_agent_ids: HashSet::new(),
            web_budget: QuickAiWebBudgetState::default(),
            non_search_tool_count: 0,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CodexTurnFailure {
    message: String,
}

impl CodexTurnFailure {
    fn protocol(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug)]
enum CodexEventDecision {
    Continue(Vec<AgentChatEvent>),
    CompleteEarly(ValidatedQuickAiAnswer),
    StopForRecovery(crate::ai::reliability::AppFailureRecord),
}

#[derive(Debug)]
enum QuickAiTurnStop {
    ProviderTerminal,
    EarlySuccess(ValidatedQuickAiAnswer),
    PolicyRecovery(crate::ai::reliability::AppFailureRecord),
    DeadlineRecovery(crate::ai::reliability::AppFailureRecord),
    Cancelled,
    Failed(CodexTurnFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedQuickAiAnswer {
    rendered: String,
    source_count: usize,
    /// Canonical http(s) URLs from the schema `sources` array, already
    /// deduped and validated. These are the ONLY URLs allowed to reach the
    /// reader, so the provenance gate needs the list and not just its length.
    validated_sources: Vec<String>,
}

fn apply_codex_exec_event(
    accumulator: &mut CodexExecTurnAccumulator,
    event: CodexExecEvent,
) -> Result<CodexEventDecision, CodexTurnFailure> {
    if accumulator.terminal_seen {
        return Ok(CodexEventDecision::Continue(Vec::new()));
    }
    match event {
        CodexExecEvent::ThreadStarted { .. } | CodexExecEvent::TurnStarted => {
            Ok(CodexEventDecision::Continue(Vec::new()))
        }
        CodexExecEvent::TurnCompleted => {
            accumulator.terminal_seen = true;
            accumulator.turn_completed_seen = true;
            Ok(CodexEventDecision::Continue(Vec::new()))
        }
        CodexExecEvent::TurnFailed { message } | CodexExecEvent::Error { message } => {
            accumulator.terminal_seen = true;
            Err(CodexTurnFailure::protocol(message))
        }
        CodexExecEvent::Item { phase, item } => match item {
            CodexItem::Safe { .. } => Ok(CodexEventDecision::Continue(Vec::new())),
            CodexItem::Diagnostic { id, message }
                if message.starts_with("Skill descriptions were shortened to fit the ") =>
            {
                tracing::debug!(
                    target: "script_kit::quick_ai",
                    event = "codex_quick_ai_ignored_skill_budget_diagnostic",
                    item_id = %id,
                );
                Ok(CodexEventDecision::Continue(Vec::new()))
            }
            CodexItem::Diagnostic { id, message } => Err(CodexTurnFailure::protocol(format!(
                "quick_ai_codex_error_item:{id}:{message}"
            ))),
            CodexItem::Forbidden { id, item_type } => {
                accumulator.non_search_tool_count += 1;
                Err(CodexTurnFailure::protocol(format!(
                    "quick_ai_codex_forbidden_item:{item_type}:{id}"
                )))
            }
            CodexItem::AgentMessage { id, text } => {
                if phase == ItemPhase::Completed
                    && !text.trim().is_empty()
                    && accumulator.completed_agent_ids.insert(id.clone())
                {
                    accumulator
                        .completed_agent_messages
                        .push(CompletedAgentMessage {
                            item_id: id,
                            text: text.clone(),
                        });
                    if accumulator.web_budget.search_completed {
                        if let Some(mut candidate) = parse_structured_answer_candidate(&text)? {
                            // Early completion must clear the SAME provenance
                            // gate as `finalize_successful_turn`. Before this,
                            // a prompt answer skipped every source check, so
                            // the strict rules only ever ran on the slow path.
                            let sources = candidate.validated_sources.clone();
                            enforce_answer_provenance(
                                accumulator,
                                Some(&sources),
                                &mut candidate.rendered,
                            )?;
                            return Ok(CodexEventDecision::CompleteEarly(candidate));
                        }
                    }
                }
                Ok(CodexEventDecision::Continue(Vec::new()))
            }
            CodexItem::WebSearch(item) => apply_web_search(accumulator, phase, item),
        },
    }
}

fn apply_web_search(
    accumulator: &mut CodexExecTurnAccumulator,
    phase: ItemPhase,
    item: WebSearchItem,
) -> Result<CodexEventDecision, CodexTurnFailure> {
    if observe_web_item(&mut accumulator.web_budget, phase, &item) == WebBudgetDecision::Stop {
        if let Some(url) = observed_action_url(&item) {
            if let Some(key) = canonical_http_url(&url) {
                if accumulator.recovery_only_url_keys.insert(key) {
                    accumulator.recovery_only_urls.push(url);
                }
            }
        }
        return Ok(CodexEventDecision::StopForRecovery(
            crate::ai::reliability::quick_ai_search_budget_failure(
                completed_focused_search_count(accumulator),
                crate::ai::agent_chat::profiles::QUICK_AI_FOCUSED_SEARCH_BUDGET,
                partial_answer(accumulator).is_some(),
                all_recovery_source_count(accumulator),
            ),
        ));
    }
    if !accumulator.web_budget.search_started {
        return Ok(CodexEventDecision::Continue(Vec::new()));
    }
    if !accumulator.search_items.contains_key(&item.id) {
        accumulator.search_order.push(item.id.clone());
        accumulator.search_items.insert(
            item.id.clone(),
            WebSearchState {
                started: false,
                completed: false,
                urls: Vec::new(),
                tool_started_emitted: false,
                tool_completed_emitted: false,
            },
        );
    }
    // Codex 0.144.x currently reports visited pages as exact `web_search`
    // items whose `query` is the URL and whose action is `other`. Prefer the
    // explicit open/find action URL when present, then accept that exact
    // item-level query URL. When Codex emits only a search action, finalization
    // may accept an answer URL only after that native search completed.
    let observed_url = observed_action_url(&item);
    if let Some(raw) = observed_url.as_ref() {
        if let Some(key) = canonical_http_url(raw) {
            if accumulator.structured_url_keys.insert(key) {
                accumulator.structured_urls.push(raw.clone());
            }
        }
    }
    let Some(state) = accumulator.search_items.get_mut(&item.id) else {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_codex_search_state_missing",
        ));
    };
    if let Some(url) = observed_url {
        if !state.urls.contains(&url) {
            state.urls.push(url);
        }
    }
    state.started = true;
    state.completed |= phase == ItemPhase::Completed;
    let mut events = Vec::new();
    if !state.tool_started_emitted {
        state.tool_started_emitted = true;
        events.push(AgentChatEvent::ToolCallStarted {
            tool_call_id: QUICK_AI_WEB_ROW_ID.to_string(),
            title: "Searching the web".to_string(),
            status: "running".to_string(),
            tool_name: None,
            raw_input: None,
        });
    }
    if phase == ItemPhase::Updated
        || (phase == ItemPhase::Completed && !state.tool_completed_emitted)
    {
        if phase == ItemPhase::Completed {
            state.tool_completed_emitted = true;
        }
        events.push(AgentChatEvent::ToolCallUpdated {
            tool_call_id: QUICK_AI_WEB_ROW_ID.to_string(),
            title: Some("Web results".to_string()),
            status: Some(
                if state.completed {
                    "complete"
                } else {
                    "running"
                }
                .to_string(),
            ),
            body: (!state.urls.is_empty()).then(|| state.urls.join("\n")),
            raw_input: None,
            diff: None,
            is_error: false,
        });
    }
    Ok(CodexEventDecision::Continue(events))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebBudgetDecision {
    Admitted,
    LifecycleOnly,
    Stop,
}

fn normalize_focused_queries(action: &WebSearchAction) -> Option<Vec<String>> {
    let WebSearchAction::Search { queries } = action else {
        return Some(Vec::new());
    };
    let mut normalized = queries
        .iter()
        .map(|query| {
            query
                .split_ascii_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        })
        .filter(|query| !query.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    if normalized.is_empty()
        || normalized.len() > 1
        || normalized
            .iter()
            .any(|query| canonical_http_url(query).is_some())
    {
        return None;
    }
    Some(normalized)
}

fn observe_web_item(
    budget: &mut QuickAiWebBudgetState,
    phase: ItemPhase,
    item: &WebSearchItem,
) -> WebBudgetDecision {
    let is_page_follow = matches!(
        item.action,
        WebSearchAction::OpenPage { .. } | WebSearchAction::FindInPage { .. }
    ) || matches!(item.action, WebSearchAction::Other)
        && canonical_http_url(&item.query).is_some();
    let decision = match budget.admitted_item_id.as_deref() {
        None if is_page_follow => WebBudgetDecision::Stop,
        None => {
            budget.admitted_item_id = Some(item.id.clone());
            budget.permit_reserved = true;
            match normalize_focused_queries(&item.action) {
                None => WebBudgetDecision::Stop,
                Some(queries) if queries.is_empty() => WebBudgetDecision::LifecycleOnly,
                Some(queries) => {
                    budget.admitted_query_key = queries.first().cloned();
                    budget.search_started = true;
                    WebBudgetDecision::Admitted
                }
            }
        }
        Some(admitted_id) if admitted_id != item.id => WebBudgetDecision::Stop,
        Some(_) if is_page_follow => WebBudgetDecision::Stop,
        Some(_) => match normalize_focused_queries(&item.action) {
            None => WebBudgetDecision::Stop,
            Some(queries) if queries.is_empty() && !budget.search_started => {
                WebBudgetDecision::LifecycleOnly
            }
            Some(queries) if queries.is_empty() => WebBudgetDecision::Stop,
            Some(queries) => {
                let query = queries.first().cloned();
                if budget.admitted_query_key.is_some() && budget.admitted_query_key != query {
                    WebBudgetDecision::Stop
                } else {
                    budget.admitted_query_key = query;
                    budget.search_started = true;
                    WebBudgetDecision::Admitted
                }
            }
        },
    };
    if decision == WebBudgetDecision::Stop {
        budget.excess_action_count = budget.excess_action_count.saturating_add(1);
    } else if budget.search_started && phase == ItemPhase::Completed {
        budget.search_completed = true;
    }
    decision
}

fn observed_action_url(item: &WebSearchItem) -> Option<String> {
    match &item.action {
        WebSearchAction::OpenPage { url } | WebSearchAction::FindInPage { url } => {
            url.as_deref().and_then(canonical_http_url)
        }
        WebSearchAction::Other => canonical_http_url(&item.query),
        WebSearchAction::Search { .. } => None,
    }
}

fn render_final_answer(raw: &str) -> Result<String, CodexTurnFailure> {
    let trimmed = raw.trim();
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(trimmed.to_string());
    };
    let (answer, valid_sources) = validate_structured_answer_fields(&object)?;
    let mut rendered = answer;
    let answer_urls = http_urls_in_text(&rendered);
    for source in valid_sources {
        if !answer_urls
            .iter()
            .any(|url| canonical_http_url(url).is_some_and(|url| url == source))
        {
            rendered.push_str("\n\nSource: ");
            rendered.push_str(&source);
        }
    }
    Ok(rendered)
}

fn validate_structured_answer_fields(
    object: &serde_json::Map<String, Value>,
) -> Result<(String, Vec<String>), CodexTurnFailure> {
    let answer = object
        .get("answer")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|answer| !answer.is_empty())
        .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_output_schema_answer_missing"))?;
    if answer.chars().count() > QUICK_AI_MAX_ANSWER_CHARS {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_output_schema_answer_too_long",
        ));
    }
    let sources = object
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_output_schema_sources_missing"))?;
    if sources.len() > QUICK_AI_MAX_SOURCES {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_output_schema_too_many_sources",
        ));
    }
    let mut seen = HashSet::new();
    let mut canonical_sources = Vec::with_capacity(sources.len());
    for source in sources {
        let source = source
            .as_str()
            .and_then(canonical_http_url)
            .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_output_schema_source_invalid"))?;
        if !seen.insert(source.clone()) {
            return Err(CodexTurnFailure::protocol(
                "quick_ai_output_schema_source_duplicate",
            ));
        }
        canonical_sources.push(source);
    }
    Ok((answer.to_string(), canonical_sources))
}

fn parse_structured_answer_candidate(
    raw: &str,
) -> Result<Option<ValidatedQuickAiAnswer>, CodexTurnFailure> {
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw.trim()) else {
        return Ok(None);
    };
    let (_, sources) = validate_structured_answer_fields(&object)?;
    Ok(Some(ValidatedQuickAiAnswer {
        rendered: render_final_answer(raw)?,
        source_count: sources.len(),
        validated_sources: sources,
    }))
}

fn completed_focused_search_count(accumulator: &CodexExecTurnAccumulator) -> u8 {
    u8::from(accumulator.web_budget.search_completed)
}

fn all_recovery_source_count(accumulator: &CodexExecTurnAccumulator) -> u16 {
    let mut keys = accumulator
        .structured_urls
        .iter()
        .chain(&accumulator.recovery_only_urls)
        .filter_map(|url| canonical_http_url(url))
        .collect::<HashSet<_>>();
    if let Some(answer) = partial_answer(accumulator) {
        keys.extend(
            http_urls_in_text(&answer)
                .into_iter()
                .filter_map(|url| canonical_http_url(&url)),
        );
    }
    keys.len().min(u16::MAX as usize) as u16
}

/// Return only assistant text that Codex actually completed before the policy
/// boundary. URLs alone are never promoted into an invented answer.
fn partial_answer(accumulator: &CodexExecTurnAccumulator) -> Option<String> {
    let raw = accumulator
        .completed_agent_messages
        .iter()
        .rev()
        .find(|message| !message.text.trim().is_empty())?
        .text
        .trim();
    if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw) {
        return object
            .get("answer")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|answer| !answer.is_empty())
            .map(str::to_string);
    }
    Some(raw.to_string())
}

/// The single source-provenance gate for a successful Quick AI turn.
///
/// Both completion paths run this. `finalize_successful_turn` handles the turn
/// that ends at `turn.completed`; `apply_codex_exec_event` handles the common
/// fast path where a schema-valid `agent_message` arrives right after the
/// admitted search completed. That fast path used to return straight to the
/// driver, so every rule below was unreachable exactly when Quick AI worked
/// normally — an answer could cite any host it liked. Keeping one
/// implementation makes the two paths structurally unable to drift.
///
/// `validated_sources` is the schema `sources` array when the answer parsed as
/// the output schema, and `None` when it did not.
///
/// What can actually be proven here is limited by the wire protocol. A Codex
/// `web_search` item with a `search` action reports the queries and NOTHING
/// else — no result URLs (see `testdata/quick-ai-streams/*.ndjson`). Only a
/// page visit carries a URL, and the Quick AI prompt forbids page visits to
/// stay inside the latency budget. So `structured_urls` is empty on every
/// ordinary turn, and host verification is reachable only on the paths where a
/// visit really happened. Rather than pretend otherwise, the snippets-only
/// path enforces the two things it genuinely can:
///
/// 1. any URL shown to the reader passed `validate_structured_answer_fields`;
/// 2. citing anything at all required a search to have completed.
///
/// An answer with no sources and no URLs is allowed with or without a search:
/// that is both the honest empty-result case and an ordinary question the
/// model can answer from its own knowledge.
fn enforce_answer_provenance(
    accumulator: &CodexExecTurnAccumulator,
    validated_sources: Option<&[String]>,
    answer: &mut String,
) -> Result<(), CodexTurnFailure> {
    if accumulator.non_search_tool_count != 0 {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_codex_non_search_tool_observed",
        ));
    }
    let answer_urls = http_urls_in_text(answer);
    if !accumulator.structured_urls.is_empty() {
        let structured_hosts: HashSet<String> = accumulator
            .structured_urls
            .iter()
            .filter_map(|url| canonical_http_host(url))
            .collect();
        if answer_urls.is_empty() {
            answer.push_str("\n\nSource: ");
            answer.push_str(&accumulator.structured_urls[0]);
        } else if !answer_urls
            .iter()
            .filter_map(|url| canonical_http_host(url))
            .any(|host| structured_hosts.contains(&host))
        {
            return Err(CodexTurnFailure::protocol(
                "quick_ai_final_answer_url_without_structured_source_host",
            ));
        }
        return Ok(());
    }

    let sources = validated_sources.unwrap_or_default();
    if answer_urls.is_empty() && sources.is_empty() {
        return Ok(());
    }
    if !accumulator.web_budget.search_completed {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_structured_sources_unavailable",
        ));
    }
    let allowed: HashSet<&str> = sources.iter().map(String::as_str).collect();
    if answer_urls.iter().any(|url| {
        canonical_http_url(url).is_none_or(|canonical| !allowed.contains(canonical.as_str()))
    }) {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_answer_url_not_in_validated_sources",
        ));
    }
    Ok(())
}

fn finalize_successful_turn(
    accumulator: &CodexExecTurnAccumulator,
) -> Result<Vec<AgentChatEvent>, CodexTurnFailure> {
    let answer = accumulator
        .completed_agent_messages
        .iter()
        .rev()
        .find(|message| !message.text.trim().is_empty())
        .map(|message| message.text.as_str())
        .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_codex_final_answer_missing"))?;
    let structured_candidate = parse_structured_answer_candidate(answer)?;
    let mut answer = structured_candidate
        .as_ref()
        .map(|candidate| candidate.rendered.clone())
        .unwrap_or(render_final_answer(answer)?);
    enforce_answer_provenance(
        accumulator,
        structured_candidate
            .as_ref()
            .map(|candidate| candidate.validated_sources.as_slice()),
        &mut answer,
    )?;
    Ok(vec![
        AgentChatEvent::AgentMessageDelta(answer),
        AgentChatEvent::completed("stop"),
    ])
}

fn emit_successful_answer(
    tx: &async_channel::Sender<AgentChatEvent>,
    trace: &TraceSink,
    accumulator: &CodexExecTurnAccumulator,
    answer: String,
    completion_path: &str,
) {
    emit_successful_events(
        tx,
        trace,
        accumulator,
        vec![
            AgentChatEvent::AgentMessageDelta(answer),
            AgentChatEvent::completed("stop"),
        ],
        completion_path,
    );
}

fn emit_successful_events(
    tx: &async_channel::Sender<AgentChatEvent>,
    trace: &TraceSink,
    accumulator: &CodexExecTurnAccumulator,
    events: Vec<AgentChatEvent>,
    completion_path: &str,
) {
    if let Some(AgentChatEvent::AgentMessageDelta(answer)) = events.first() {
        trace.write(
            "final_answer_selected",
            json!({
                "answerSha256": private_trace_fingerprint(answer),
                "answerChars": answer.chars().count(),
                "answerUrls": http_urls_in_text(answer),
                // `unvisited-validated-schema-source` is the ordinary case, and
                // the name says what it is: the URL passed schema validation
                // and followed a completed search, but nothing fetched it. The
                // old label ("answer-url-after-native-search") described the
                // check that had been performed rather than the confidence it
                // bought, which made the traces read like verification.
                "sourceProvenance": if accumulator.structured_urls.is_empty() {
                    "unvisited-validated-schema-source"
                } else {
                    "admitted-native-action"
                },
                "completionPath": completion_path,
            }),
        );
    }
    trace.write("terminal", json!({"kind": "completed"}));
    for event in events {
        let _ = tx.send_blocking(event);
    }
}

fn emit_codex_failure(
    tx: &async_channel::Sender<AgentChatEvent>,
    trace: &TraceSink,
    accumulator: &CodexExecTurnAccumulator,
    error: CodexTurnFailure,
    stderr_text: &str,
) {
    let message = if error.message.is_empty() {
        "quick_ai_codex_turn_failed".to_string()
    } else {
        error.message
    };
    let detail = if stderr_text.trim().is_empty() {
        message.clone()
    } else {
        format!("{message}\n{}", stderr_text.trim())
    };
    // Classify from OUR code, not from the code glued to provider stderr.
    // The concatenated form let stray words in stderr pick the failure kind —
    // "operation not permitted" from a blocked shell command read as a
    // permission problem, "unauthorized" read as "sign in to continue".
    let failure = crate::ai::reliability::quick_ai_failure(
        sk_protocol::ai_reliability::ProtocolComponent::Codex,
        &message,
        &detail,
    );
    let fingerprint = failure
        .failure
        .diagnostic
        .as_ref()
        .map(|diagnostic| diagnostic.fingerprint.0.as_str())
        .unwrap_or("unavailable");
    tracing::warn!(
        target: "script_kit::quick_ai",
        event = "codex_quick_ai_failed",
        failure_code = ?failure.failure.code,
        diagnostic_fingerprint = fingerprint,
    );
    trace.write(
        "protocol_failure",
        json!({
            "failureCode": format!("{:?}", failure.failure.code),
            "diagnosticFingerprint": fingerprint,
        }),
    );
    emit_running_search_failures(tx, accumulator, failure.primary_message(), true);
    trace.write(
        "terminal",
        json!({
            "kind": "failed",
            "failureCode": format!("{:?}", failure.failure.code),
            "diagnosticFingerprint": fingerprint,
        }),
    );
    let _ = tx.send_blocking(AgentChatEvent::TurnFailed { failure });
}

fn emit_running_search_failures(
    tx: &async_channel::Sender<AgentChatEvent>,
    accumulator: &CodexExecTurnAccumulator,
    message: &str,
    is_error: bool,
) {
    for item_id in &accumulator.search_order {
        let Some(state) = accumulator.search_items.get(item_id) else {
            continue;
        };
        if state.tool_started_emitted && !state.tool_completed_emitted {
            let _ = tx.send_blocking(AgentChatEvent::ToolCallUpdated {
                tool_call_id: QUICK_AI_WEB_ROW_ID.to_string(),
                title: None,
                status: Some("failed".to_string()),
                body: Some(message.to_string()),
                raw_input: None,
                diff: None,
                is_error,
            });
        }
    }
}

pub(crate) fn canonical_http_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches(|character: char| {
        matches!(character, '.' | ',' | ';' | ':' | ')' | ']' | '}')
    });
    canonical_http_host(trimmed)?;
    Some(trimmed.to_string())
}

fn canonical_http_host(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?.trim();
    if host.is_empty() || host.chars().any(char::is_whitespace) {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

fn http_urls_in_text(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(canonical_http_url)
        .collect()
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessTeardownReport {
    term_sent: bool,
    kill_sent: bool,
    child_reaped: bool,
    exit_code: Option<i32>,
    exit_signal: Option<i32>,
    process_group_alive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ProcessTeardownReport {
    fn failed(error: String) -> Self {
        Self {
            term_sent: false,
            kill_sent: false,
            child_reaped: false,
            exit_code: None,
            exit_signal: None,
            process_group_alive: true,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TeardownPolicy {
    term_grace: Duration,
    poll_interval: Duration,
    post_kill_verify: Duration,
}

const USER_CANCEL_TEARDOWN: TeardownPolicy = TeardownPolicy {
    term_grace: Duration::from_secs(2),
    poll_interval: Duration::from_millis(20),
    post_kill_verify: Duration::from_secs(2),
};

const QUICK_AI_FAST_TEARDOWN: TeardownPolicy = TeardownPolicy {
    term_grace: Duration::from_millis(75),
    poll_interval: Duration::from_millis(10),
    post_kill_verify: Duration::from_millis(200),
};

pub(crate) fn terminate_and_reap_process_group(
    child: &mut Child,
    pgid: i32,
    policy: TeardownPolicy,
) -> Result<ProcessTeardownReport> {
    let mut report = ProcessTeardownReport {
        term_sent: false,
        kill_sent: false,
        child_reaped: false,
        exit_code: None,
        exit_signal: None,
        process_group_alive: process_group_alive(pgid),
        error: None,
    };
    let mut status = child.try_wait()?;
    if report.process_group_alive {
        let result = unsafe { libc::killpg(pgid, libc::SIGTERM) };
        if result == 0 {
            report.term_sent = true;
        } else {
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::ESRCH) => {}
                Some(libc::EPERM) => {
                    // Darwin also refuses signals to zombie-only groups. Keep the
                    // diagnostic; only reaping plus a later ESRCH proves cleanup.
                    report.error = Some(format!("quick_ai_sigterm_failed:{error}"));
                }
                _ => return Err(anyhow!("quick_ai_sigterm_failed:{error}")),
            }
        }
    }
    let deadline = Instant::now() + policy.term_grace;
    while Instant::now() < deadline {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if !process_group_alive(pgid) {
            break;
        }
        std::thread::sleep(policy.poll_interval);
    }
    if process_group_alive(pgid) {
        let result = unsafe { libc::killpg(pgid, libc::SIGKILL) };
        if result == 0 {
            report.kill_sent = true;
        } else {
            let error = std::io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::ESRCH) => {}
                Some(libc::EPERM) => {
                    report
                        .error
                        .get_or_insert_with(|| format!("quick_ai_sigkill_failed:{error}"));
                }
                _ => return Err(anyhow!("quick_ai_sigkill_failed:{error}")),
            }
        }
    }
    // Never block waiting for a live child whose termination was refused.
    if status.is_none() && report.error.is_none() {
        status = Some(child.wait()?);
    }
    let verify_deadline = Instant::now() + policy.post_kill_verify;
    while Instant::now() < verify_deadline {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if status.is_some() && !process_group_alive(pgid) {
            break;
        }
        std::thread::sleep(policy.poll_interval);
    }
    if status.is_none() {
        status = child.try_wait()?;
    }
    if let Some(status) = status {
        report.child_reaped = true;
        report.exit_code = status.code();
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;
            report.exit_signal = status.signal();
        }
    }
    report.process_group_alive = process_group_alive(pgid);
    Ok(report)
}

pub(crate) fn process_group_alive(pgid: i32) -> bool {
    let result = unsafe { libc::killpg(pgid, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[derive(Clone)]
struct TraceSink {
    path: Option<PathBuf>,
    run_id: String,
    seq: Arc<AtomicU64>,
    started: Instant,
    first_protocol_event_seen: Arc<AtomicBool>,
    web_action_ordinals: Arc<Mutex<HashMap<String, u8>>>,
    /// Mirror of the milestones onto the shared cross-surface trace.
    ///
    /// Quick AI keeps its own rich, Quick-AI-specific vocabulary (search
    /// permits, web action classes, answer provenance) because that detail is
    /// what made its own latency work possible and no other surface has an
    /// equivalent. But without ALSO emitting the five common milestones, Quick
    /// AI would be the one surface missing from the cross-surface report — the
    /// exact inversion of the problem this work exists to fix.
    ///
    /// Mirroring happens inside `write` rather than at the ~20 call sites so
    /// that adding a Quick AI trace event can never silently skip the shared
    /// trace, and so this change threads no new parameter through the many
    /// free functions that already take a `&TraceSink`.
    shared: crate::ai::phase_trace::PhaseTrace,
}

impl TraceSink {
    fn new(path: Option<PathBuf>, run_id: String, started: Instant) -> Self {
        Self {
            path,
            run_id: run_id.clone(),
            seq: Arc::new(AtomicU64::new(1)),
            started,
            first_protocol_event_seen: Arc::new(AtomicBool::new(false)),
            web_action_ordinals: Arc::new(Mutex::new(HashMap::new())),
            shared: crate::ai::phase_trace::PhaseTrace::begin(
                crate::ai::phase_trace::AiSurface::QuickAi,
                crate::ai::phase_trace::AiTransport::CodexExec,
                run_id,
            ),
        }
    }

    /// Map one Quick AI event onto the shared vocabulary.
    ///
    /// Quick AI is NOT a streaming surface from the user's point of view: it
    /// buffers and then shows a finished answer. So its first visible output is
    /// the moment the answer is selected, not the moment tokens started
    /// arriving. Recording it any earlier would flatter Quick AI against the
    /// surfaces that genuinely stream.
    fn mirror_to_shared(&self, event: &str, details: &Value) {
        use crate::ai::phase_trace::TurnOutcome;
        match event {
            "start_turn_entered" => self.shared.turn_start(details.clone()),
            "first_protocol_event" => self.shared.observe_provider_event(),
            "native_web_action" => self.shared.observe_tool_call(),
            "final_answer_selected" | "early_finalization_selected" => {
                // The digest of the answer is recorded by the shared trace; the
                // answer text itself never leaves this process.
                self.shared.observe_visible_output(
                    details
                        .get("answerSha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                );
            }
            "terminal" => {
                let outcome = match details.get("kind").and_then(Value::as_str) {
                    Some("cancelled") => TurnOutcome::Cancelled,
                    Some("completed") => TurnOutcome::Completed,
                    _ => TurnOutcome::Failed,
                };
                self.shared.terminal(outcome, None);
            }
            "teardown" => self.shared.teardown(),
            _ => {}
        }
    }

    fn observe_first_protocol_event(&self) {
        if !self.first_protocol_event_seen.swap(true, Ordering::AcqRel) {
            self.write("first_protocol_event", json!({}));
        }
    }

    fn web_action_ordinal(&self, item_id: &str) -> u8 {
        let Ok(mut ordinals) = self.web_action_ordinals.lock() else {
            return 0;
        };
        if let Some(ordinal) = ordinals.get(item_id) {
            return *ordinal;
        }
        let ordinal = ordinals.len().saturating_add(1).min(u8::MAX as usize) as u8;
        ordinals.insert(item_id.to_string(), ordinal);
        ordinal
    }

    fn write(&self, event: &str, details: Value) {
        // Mirror BEFORE the early return below. The two traces are gated on
        // different environment variables, so a run that enables only the
        // shared cross-surface trace must still record Quick AI's milestones.
        self.mirror_to_shared(event, &details);
        let Some(path) = &self.path else { return };
        let mut record = json!({
            "schemaVersion": 1,
            "runId": self.run_id,
            "seq": self.seq.fetch_add(1, Ordering::Relaxed),
            "event": event,
            "elapsedMs": self.started.elapsed().as_millis() as u64,
        });
        if let (Some(target), Some(fields)) = (record.as_object_mut(), details.as_object()) {
            target.extend(fields.clone());
        }
        let line = record.to_string();
        let _ = crate::atomic_file::append_private_observability_record(path, line.as_bytes());
    }
}

fn trace_event_for_protocol(trace: &TraceSink, event: &CodexExecEvent) {
    trace.observe_first_protocol_event();
    match event {
        CodexExecEvent::ThreadStarted { .. } => trace.write("thread_started", json!({})),
        CodexExecEvent::TurnStarted => trace.write("turn_started", json!({})),
        CodexExecEvent::Item {
            phase,
            item: CodexItem::WebSearch(item),
        } => {
            let native_lifecycle_phase = match phase {
                ItemPhase::Started => "started",
                ItemPhase::Updated => "updated",
                ItemPhase::Completed => "completed",
            };
            let action_class = match &item.action {
                WebSearchAction::Search { .. } => "search",
                WebSearchAction::OpenPage { .. } | WebSearchAction::FindInPage { .. } => {
                    "page-follow"
                }
                WebSearchAction::Other if canonical_http_url(&item.query).is_some() => "url-visit",
                WebSearchAction::Other => "unknown",
            };
            trace.write(
                "native_web_action",
                json!({
                    "nativeLifecyclePhase": native_lifecycle_phase,
                    "actionClass": action_class,
                    "actionOrdinal": trace.web_action_ordinal(&item.id),
                    "querySha256": crate::logging::log_private_user_value(&item.query).sha256,
                    "queryChars": item.query.chars().count(),
                }),
            );
            let action_url = match &item.action {
                WebSearchAction::OpenPage { url } | WebSearchAction::FindInPage { url } => {
                    url.as_deref()
                }
                _ => None,
            };
            if let Some(url) =
                action_url.or_else(|| canonical_http_url(&item.query).map(|_| item.query.as_str()))
            {
                trace.write(
                    "source_observed",
                    json!({
                        "actionOrdinal": trace.web_action_ordinal(&item.id),
                        "sourceUrl": canonical_http_url(url),
                        "sourceClass": "native-action",
                    }),
                );
            }
        }
        CodexExecEvent::Item {
            phase: ItemPhase::Completed,
            item: CodexItem::AgentMessage { id: _, text },
        } => trace.write(
            "agent_message_buffered",
            json!({"textSha256": private_trace_fingerprint(text), "textChars": text.chars().count()}),
        ),
        CodexExecEvent::Item {
            item: CodexItem::Forbidden { id: _, item_type },
            ..
        } => trace.write(
            "forbidden_item",
            json!({"itemTypeSha256": private_trace_fingerprint(item_type)}),
        ),
        CodexExecEvent::Item {
            item: CodexItem::Diagnostic { id: _, message },
            ..
        } => trace.write(
            "diagnostic",
            json!({"messageSha256": private_trace_fingerprint(message)}),
        ),
        _ => {}
    }
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn private_trace_fingerprint(value: &str) -> String {
    crate::logging::log_private_user_value(value).sha256
}

#[cfg(test)]
include!("codex_exec_tests.rs");
