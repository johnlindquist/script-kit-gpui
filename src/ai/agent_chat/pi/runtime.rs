use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use serde_json::json;

use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx};
use crate::ai::agent_chat::runtime::{AgentChatConnection, AgentChatTurnRequest};
use crate::ai::agent_chat::ui::events::AgentChatEventTx;
use crate::ai::phase_trace::{AiSurface, AiTransport, PhaseTrace, TurnOutcome};

use super::events::map_rpc_line_to_events;
use super::protocol::{
    build_abort_command, build_fork_command, build_get_available_models_command,
    build_get_fork_messages_command, build_prompt_command, build_prompt_payload,
    build_set_model_command, encode_json_line, parse_rpc_line, PiRpcLaunchSpec,
    PiRpcModelSelection, PiRpcResponse,
};

type PendingResponses = Arc<Mutex<HashMap<String, PendingResponse>>>;
type ActiveTurn = Arc<Mutex<Option<ActiveTurnState>>>;
type StderrFailureHint = Arc<Mutex<Option<String>>>;

const PI_REVEAL_CHUNK_DELAY_MS: u64 = 6;

enum PendingResponse {
    Events(AgentChatEventTx),
    Rpc(oneshot::Sender<PiRpcResponse>),
}

#[derive(Clone)]
struct ActiveTurnState {
    ui_thread_id: String,
    prompt_id: String,
    event_tx: AgentChatEventTx,
    /// Phase trace for this turn.
    ///
    /// It lives on the active-turn record rather than being threaded as a
    /// parameter because `read_stdout` is a single long-lived task that serves
    /// every turn on the connection: it has no per-turn scope of its own, but it
    /// already reaches the active turn to find the event sender. Cloning is one
    /// `Arc` bump, and [`PhaseTrace::disabled`] makes the field free when
    /// tracing is off.
    trace: PhaseTrace,
}

pub(crate) enum PiRpcRuntimeCommand {
    StartTurn {
        request: AgentChatTurnRequest,
        event_tx: AgentChatEventTx,
    },
    PrepareSession {
        ui_thread_id: String,
        cwd: std::path::PathBuf,
        event_tx: AgentChatEventTx,
    },
    CancelTurn {
        ui_thread_id: String,
    },
    GetForkPoints {
        event_tx: AgentChatEventTx,
    },
    Fork {
        entry_id: String,
        event_tx: AgentChatEventTx,
    },
}

pub(crate) struct PiRpcRuntime {
    tx: async_channel::Sender<PiRpcRuntimeCommand>,
    /// Stored so focused-text variation turns can use separate Pi processes.
    ///
    /// The normal runtime still uses one worker process and one active turn.
    /// Isolated turns intentionally do not share that worker.
    spec: Option<Arc<PiRpcLaunchSpec>>,
}

impl PiRpcRuntime {
    pub(crate) fn spawn(spec: PiRpcLaunchSpec) -> Result<Self> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)?;
        let spec = Arc::new(spec);
        let worker_spec = spec.clone();
        let (tx, rx) = async_channel::bounded::<PiRpcRuntimeCommand>(8);

        std::thread::Builder::new()
            .name("pi-rpc-agent-chat".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        tracing::error!(%error, "pi_rpc_runtime_build_failed");
                        return;
                    }
                };

                runtime.block_on(async move {
                    if let Err(error) = run_pi_rpc_event_loop(worker_spec, rx).await {
                        tracing::error!(%error, "pi_rpc_event_loop_exited_with_error");
                    }
                });
            })
            .context("Failed to spawn Pi RPC worker thread")?;

        Ok(Self {
            tx,
            spec: Some(spec),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_sender(tx: async_channel::Sender<PiRpcRuntimeCommand>) -> Self {
        Self { tx, spec: None }
    }
}

impl AgentChatConnection for PiRpcRuntime {
    fn start_turn(
        &self,
        request: AgentChatTurnRequest,
    ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
        let (event_tx, event_rx) = async_channel::bounded(256);
        self.tx
            .send_blocking(PiRpcRuntimeCommand::StartTurn { request, event_tx })
            .context("Pi RPC worker channel closed")
            .map_err(crate::ai::reliability::AiAdapterError::from)?;
        Ok(event_rx)
    }

    fn start_isolated_turn(
        &self,
        request: AgentChatTurnRequest,
    ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::runtime::IsolatedTurnHandle>
    {
        let Some(spec) = self.spec.clone() else {
            return Err(anyhow::anyhow!(
                "Pi RPC isolated turns are unavailable for sender-only test runtime"
            )
            .into());
        };
        let (event_tx, event_rx) = async_channel::bounded(256);
        let cancel = spawn_single_turn_runtime(spec, request, event_tx)?;
        Ok(crate::ai::agent_chat::runtime::IsolatedTurnHandle {
            rx: event_rx,
            cancel: Some(cancel),
        })
    }

    fn cancel_turn(&self, ui_thread_id: String) -> crate::ai::reliability::AiAdapterResult<()> {
        self.tx
            .send_blocking(PiRpcRuntimeCommand::CancelTurn { ui_thread_id })
            .context("Pi RPC worker channel closed")
            .map_err(Into::into)
    }

    fn prepare_session(
        &self,
        ui_thread_id: String,
        cwd: std::path::PathBuf,
    ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
        let (event_tx, event_rx) = async_channel::bounded(8);
        self.tx
            .send_blocking(PiRpcRuntimeCommand::PrepareSession {
                ui_thread_id,
                cwd,
                event_tx,
            })
            .context("Pi RPC worker channel closed")
            .map_err(crate::ai::reliability::AiAdapterError::from)?;
        Ok(event_rx)
    }

    fn fork_points(&self) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
        let (event_tx, event_rx) = async_channel::bounded(8);
        self.tx
            .send_blocking(PiRpcRuntimeCommand::GetForkPoints { event_tx })
            .context("Pi RPC worker channel closed")
            .map_err(crate::ai::reliability::AiAdapterError::from)?;
        Ok(event_rx)
    }

    fn fork_to_entry(
        &self,
        entry_id: String,
    ) -> crate::ai::reliability::AiAdapterResult<AgentChatEventRx> {
        let (event_tx, event_rx) = async_channel::bounded(8);
        self.tx
            .send_blocking(PiRpcRuntimeCommand::Fork { entry_id, event_tx })
            .context("Pi RPC worker channel closed")
            .map_err(crate::ai::reliability::AiAdapterError::from)?;
        Ok(event_rx)
    }
}

fn pi_failure(raw: impl AsRef<str>) -> AgentChatEvent {
    AgentChatEvent::failed(sk_protocol::ai_reliability::ProtocolComponent::Pi, raw)
}

/// S12: an IO/RPC failure against the pi child is a RUNTIME CLOSED fact.
///
/// `error.to_string()` for a child that has exited is "Broken pipe (os error
/// 32)" or a closed-channel message. Neither is classifiable English, so these
/// used to reach the user as `Unknown` — "The AI request did not finish" with
/// no reconnect path, for the one failure reconnecting fixes. The cause still
/// goes to the diagnostic vault behind Copy Details.
fn pi_transport_failure(cause: impl std::fmt::Display) -> AgentChatEvent {
    AgentChatEvent::TurnFailed {
        failure: crate::ai::reliability::runtime_closed_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            &cause.to_string(),
        ),
    }
}

fn cancelled_outcome() -> AgentChatEvent {
    AgentChatEvent::TurnCompleted {
        outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Cancelled {
            kind: sk_protocol::ai_reliability::CancellationKind::UserCancelled,
            partial: sk_protocol::ai_reliability::PartialOutputState::None,
        },
    }
}

async fn run_pi_rpc_event_loop(
    spec: Arc<PiRpcLaunchSpec>,
    rx: async_channel::Receiver<PiRpcRuntimeCommand>,
) -> Result<()> {
    let mut cmd = Command::new(&spec.command);
    cmd.args(&spec.args)
        .envs(&spec.env)
        .current_dir(&spec.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group so orphan cleanup / kill_all can reap the sidecar
        // and any tools it spawned; kill_on_drop covers `?` early returns.
        .process_group(0)
        .kill_on_drop(true);

    let mut child = cmd.spawn().context("Failed to spawn Pi RPC process")?;
    let _registration = child.id().map(|pid| {
        crate::process_manager::ChildRegistration::register(pid, &spec.command.to_string_lossy())
    });
    let mut stdin = child.stdin.take().context("Pi RPC stdin unavailable")?;
    let stdout = child.stdout.take().context("Pi RPC stdout unavailable")?;
    let stderr = child.stderr.take().context("Pi RPC stderr unavailable")?;

    let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
    let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
    let stderr_failure_hint: StderrFailureHint = Arc::new(Mutex::new(None));
    let stdout_pending = pending.clone();
    let stdout_active = active_turn.clone();
    let stdout_stderr_failure_hint = stderr_failure_hint.clone();

    tokio::spawn(async move {
        read_stdout(
            stdout,
            stdout_pending,
            stdout_active,
            Some(stdout_stderr_failure_hint),
        )
        .await;
    });

    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(hint) = user_facing_pi_stderr_hint(&line) {
                stderr_failure_hint.lock().replace(hint);
            }
            log_pi_rpc_stderr_line(&line);
        }
    });

    let mut counter = 0_u64;
    // Last selection acknowledged by the pi process. set_model is a blocking
    // round trip serialized ahead of every prompt, so skip it when the model
    // is unchanged.
    let mut applied_model: Option<PiRpcModelSelection> = None;
    while let Ok(command) = rx.recv().await {
        counter += 1;
        match command {
            PiRpcRuntimeCommand::PrepareSession {
                ui_thread_id,
                cwd,
                event_tx,
            } => {
                tracing::debug!(
                    target: "script_kit::tab_ai",
                    event = "pi_rpc_prepare_session",
                    ui_thread_id = %ui_thread_id,
                    cwd = %cwd.display()
                );
                let id = format!("models-{counter}");
                pending
                    .lock()
                    .insert(id.clone(), PendingResponse::Events(event_tx));
                write_json(&mut stdin, &build_get_available_models_command(id)).await?;
            }
            PiRpcRuntimeCommand::StartTurn { request, event_tx } => {
                // Open the trace before ANY work, including the set_model round
                // trip. set_model is serialized ahead of the prompt and blocks
                // it, so a turn that pays for a model switch is genuinely slower
                // — starting the clock after it would hide that cost entirely.
                let trace =
                    PhaseTrace::begin(spec.surface, AiTransport::PiRpc, format!("pi-{counter}"));
                trace.turn_start(json!({
                    "modelSwitchPending": request.model_id.as_deref().is_some_and(|model_id| {
                        PiRpcModelSelection::parse(model_id)
                            .ok()
                            .is_some_and(|selection| applied_model.as_ref() != Some(&selection))
                    }),
                    "blockCount": request.blocks.len(),
                }));

                if let Some(model_id) = request.model_id.as_deref() {
                    let selection = match PiRpcModelSelection::parse(model_id) {
                        Ok(selection) => selection,
                        Err(error) => {
                            send_event_traced(
                                &event_tx,
                                &trace,
                                pi_failure(format!("Invalid Pi model selection: {error}")),
                            )
                            .await;
                            continue;
                        }
                    };

                    if applied_model.as_ref() != Some(&selection) {
                        let id = format!("set-model-{counter}");
                        match send_set_model_and_wait(&mut stdin, &pending, id, &selection).await {
                            Ok(()) => {
                                applied_model = Some(selection);
                            }
                            Err(error) => {
                                applied_model = None;
                                send_event_traced(
                                    &event_tx,
                                    &trace,
                                    AgentChatEvent::TurnFailed {
                                        failure: *error.failure,
                                    },
                                )
                                .await;
                                continue;
                            }
                        }
                    }
                }

                if request.cwd != spec.cwd {
                    tracing::debug!(
                        target: "script_kit::tab_ai",
                        event = "pi_rpc_cwd_mismatch",
                        requested_cwd = %request.cwd.display(),
                        launch_cwd = %spec.cwd.display(),
                        "Pi RPC runtime uses launch cwd for this connection"
                    );
                }

                let prompt_id = format!("prompt-{counter}");
                match build_prompt_payload(&request.blocks) {
                    Ok(payload) => {
                        active_turn.lock().replace(ActiveTurnState {
                            ui_thread_id: request.ui_thread_id,
                            prompt_id: prompt_id.clone(),
                            event_tx,
                            trace,
                        });
                        write_json(&mut stdin, &build_prompt_command(prompt_id, payload)).await?;
                    }
                    Err(error) => {
                        send_event_traced(&event_tx, &trace, pi_failure(error.to_string())).await;
                    }
                }
            }
            PiRpcRuntimeCommand::GetForkPoints { event_tx } => {
                let id = format!("fork-msgs-{counter}");
                pending
                    .lock()
                    .insert(id.clone(), PendingResponse::Events(event_tx));
                write_json(&mut stdin, &build_get_fork_messages_command(id)).await?;
            }
            PiRpcRuntimeCommand::Fork { entry_id, event_tx } => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "pi_rpc_fork_sent",
                    entry_id = %entry_id,
                );
                let id = format!("fork-{counter}");
                pending
                    .lock()
                    .insert(id.clone(), PendingResponse::Events(event_tx));
                write_json(&mut stdin, &build_fork_command(id, &entry_id)).await?;
            }
            PiRpcRuntimeCommand::CancelTurn { ui_thread_id } => {
                let active = active_turn.lock().clone();
                if let Some(active) = active.filter(|active| active.ui_thread_id == ui_thread_id) {
                    let id = format!("abort-{counter}");
                    // A user Stop is cancellation, not an error. Recording it as
                    // the terminal state here (rather than letting whatever
                    // event the abort produces classify it) keeps cancelled
                    // turns out of the latency medians, where they would
                    // otherwise look like impossibly fast successes.
                    active.trace.terminal(TurnOutcome::Cancelled, None);
                    write_json(&mut stdin, &build_abort_command(id)).await?;
                    tracing::debug!(
                        target: "script_kit::tab_ai",
                        event = "pi_rpc_abort_sent",
                        ui_thread_id = %ui_thread_id,
                        prompt_id = %active.prompt_id
                    );
                } else {
                    tracing::debug!(
                        target: "script_kit::tab_ai",
                        event = "pi_rpc_abort_ignored_no_active_turn",
                        ui_thread_id = %ui_thread_id
                    );
                }
            }
        }
    }

    stop_pi_child(&mut child).await?;
    Ok(())
}

pub(crate) type IsolatedTurnCancelFlag = Arc<std::sync::atomic::AtomicBool>;

fn new_cancel_flag() -> IsolatedTurnCancelFlag {
    Arc::new(std::sync::atomic::AtomicBool::new(false))
}

fn spawn_single_turn_runtime(
    spec: Arc<PiRpcLaunchSpec>,
    request: AgentChatTurnRequest,
    event_tx: AgentChatEventTx,
) -> Result<IsolatedTurnCancelFlag> {
    let cancel = new_cancel_flag();
    let cancel_inner = cancel.clone();
    std::thread::Builder::new()
        .name("pi-rpc-agent-chat-isolated-turn".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(%error, "pi_rpc_isolated_runtime_build_failed");
                    return;
                }
            };

            runtime.block_on(async move {
                if let Err(error) =
                    run_pi_rpc_single_turn(spec, request, event_tx, cancel_inner).await
                {
                    tracing::error!(%error, "pi_rpc_isolated_turn_exited_with_error");
                }
            });
        })
        .context("Failed to spawn isolated Pi RPC worker thread")?;
    Ok(cancel)
}

async fn run_pi_rpc_single_turn(
    spec: Arc<PiRpcLaunchSpec>,
    request: AgentChatTurnRequest,
    event_tx: AgentChatEventTx,
    cancel: IsolatedTurnCancelFlag,
) -> Result<()> {
    let mut cmd = Command::new(&spec.command);
    cmd.args(&spec.args)
        .envs(&spec.env)
        .current_dir(&spec.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Same reaping contract as the persistent runtime above.
        .process_group(0)
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .context("Failed to spawn isolated Pi RPC process")?;
    let _registration = child.id().map(|pid| {
        crate::process_manager::ChildRegistration::register(pid, &spec.command.to_string_lossy())
    });
    let mut stdin = child
        .stdin
        .take()
        .context("Isolated Pi RPC stdin unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("Isolated Pi RPC stdout unavailable")?;
    let stderr = child
        .stderr
        .take()
        .context("Isolated Pi RPC stderr unavailable")?;

    let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
    let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
    let stderr_failure_hint: StderrFailureHint = Arc::new(Mutex::new(None));
    let (done_tx, done_rx) = async_channel::bounded::<()>(1);

    // Isolated turns are the focused-text variation path, which is the Mini
    // surface: same Pi protocol, but its own cold process. Label it explicitly
    // rather than inheriting the connection's surface, because a cold spawn per
    // turn is a completely different latency profile from Agent Chat's warm
    // sidecar and must not be pooled with it.
    let trace = PhaseTrace::begin(AiSurface::Mini, AiTransport::PiRpc, "pi-isolated");
    trace.turn_start(json!({
        "blockCount": request.blocks.len(),
        "isolatedProcess": true,
    }));

    tokio::spawn(read_single_turn_stdout(
        stdout,
        pending.clone(),
        active_turn.clone(),
        Some(stderr_failure_hint.clone()),
        done_tx,
    ));

    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(hint) = user_facing_pi_stderr_hint(&line) {
                stderr_failure_hint.lock().replace(hint);
            }
            log_pi_rpc_stderr_line(&line);
        }
    });

    if let Some(model_id) = request.model_id.as_deref() {
        let selection = match PiRpcModelSelection::parse(model_id) {
            Ok(selection) => selection,
            Err(error) => {
                send_event_traced(
                    &event_tx,
                    &trace,
                    pi_failure(format!("Invalid Pi model selection: {error}")),
                )
                .await;
                stop_pi_child(&mut child).await?;
                return Ok(());
            }
        };

        if let Err(error) = send_set_model_and_wait(
            &mut stdin,
            &pending,
            "set-model-isolated".to_string(),
            &selection,
        )
        .await
        {
            send_event_traced(
                &event_tx,
                &trace,
                AgentChatEvent::TurnFailed {
                    failure: *error.failure,
                },
            )
            .await;
            stop_pi_child(&mut child).await?;
            return Ok(());
        }
    }

    if request.cwd != spec.cwd {
        tracing::debug!(
            target: "script_kit::tab_ai",
            event = "pi_rpc_isolated_cwd_mismatch",
            requested_cwd = %request.cwd.display(),
            launch_cwd = %spec.cwd.display(),
            "Pi RPC isolated runtime uses launch cwd for this connection"
        );
    }

    let prompt_id = "prompt-isolated".to_string();
    let payload = match build_prompt_payload(&request.blocks) {
        Ok(payload) => payload,
        Err(error) => {
            send_event_traced(&event_tx, &trace, pi_failure(error.to_string())).await;
            stop_pi_child(&mut child).await?;
            return Ok(());
        }
    };

    active_turn.lock().replace(ActiveTurnState {
        ui_thread_id: request.ui_thread_id.clone(),
        prompt_id: prompt_id.clone(),
        event_tx: event_tx.clone(),
        trace: trace.clone(),
    });

    if let Err(error) = write_json(&mut stdin, &build_prompt_command(prompt_id, payload)).await {
        send_event_traced(&event_tx, &trace, pi_transport_failure(&error)).await;
        stop_pi_child(&mut child).await?;
        return Err(error);
    }

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(600);
    loop {
        let poll_interval = std::time::Duration::from_millis(200);
        match tokio::time::timeout(poll_interval, done_rx.recv()).await {
            Ok(_) => break,
            Err(_) => {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "pi_rpc_isolated_turn_cancelled",
                    );
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        event_tx.send(cancelled_outcome()),
                    )
                    .await;
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    let _ = event_tx
                        .send(pi_failure("Pi RPC isolated turn timed out"))
                        .await;
                    break;
                }
            }
        }
    }

    stop_pi_child(&mut child).await?;
    Ok(())
}

async fn read_single_turn_stdout<R>(
    stdout: R,
    pending: PendingResponses,
    active_turn: ActiveTurn,
    stderr_failure_hint: Option<StderrFailureHint>,
    done_tx: async_channel::Sender<()>,
) where
    R: AsyncRead + Unpin,
{
    let mut terminal_event_seen = false;
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let parsed = match parse_rpc_line(&line) {
            Ok(parsed) => parsed,
            Err(error) => {
                send_to_active(
                    &active_turn,
                    pi_failure(format!("Invalid Pi RPC output: {error}")),
                )
                .await;
                terminal_event_seen = true;
                break;
            }
        };

        if let super::protocol::PiRpcLine::Response(response) = &parsed {
            if let Some(id) = response.id.as_ref() {
                let pending_response = pending.lock().remove(id);
                if let Some(pending_response) = pending_response {
                    match pending_response {
                        PendingResponse::Events(event_tx) => {
                            send_events(&event_tx, map_rpc_line_to_events(parsed)).await;
                        }
                        PendingResponse::Rpc(response_tx) => {
                            let _ = response_tx.send(response.clone());
                        }
                    }
                    continue;
                }
            }

            if let Some(active) = take_failed_prompt_response(response, &active_turn) {
                deliver_terminal_to_turn(
                    active,
                    pi_failure(
                        response
                            .error
                            .clone()
                            .unwrap_or_else(|| "Pi RPC prompt failed".to_string()),
                    ),
                )
                .await;
                terminal_event_seen = true;
                break;
            }
            continue;
        }

        let events = map_rpc_line_to_events(parsed);
        let closes_turn = events.iter().any(|event| {
            matches!(
                event,
                AgentChatEvent::TurnCompleted { .. } | AgentChatEvent::TurnFailed { .. }
            )
        });
        // Take terminal ownership before awaiting: cleanup cannot erase a new turn.
        let active = if closes_turn {
            active_turn.lock().take()
        } else {
            active_turn.lock().as_ref().cloned()
        };
        if let Some(active) = active {
            send_events_traced(&active.event_tx, &active.trace, events).await;
            if closes_turn {
                active.trace.teardown();
            }
        }
        if closes_turn {
            terminal_event_seen = true;
            break;
        }
    }

    if !terminal_event_seen {
        send_to_active(
            &active_turn,
            pi_failure(pi_rpc_process_exit_error(
                "Pi RPC isolated turn ended before completion",
                stderr_failure_hint.as_ref(),
            )),
        )
        .await;
    }

    let _ = done_tx.send(()).await;
}

async fn write_json<W>(writer: &mut W, value: &serde_json::Value) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(encode_json_line(value).as_bytes())
        .await
        .context("Failed to write Pi RPC command")?;
    writer
        .flush()
        .await
        .context("Failed to flush Pi RPC command")
}

async fn stop_pi_child(child: &mut tokio::process::Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    child.start_kill().context("Pi child kill failed")?;
    tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
        .await
        .context("Pi child teardown timed out")?
        .context("Pi child reap failed")?;
    Ok(())
}

fn validate_set_model_response(
    expected_id: &str,
    response: PiRpcResponse,
) -> crate::ai::reliability::AiAdapterResult<()> {
    use crate::ai::reliability::{AiAdapterError, ProtocolFailureFacts};
    use sk_protocol::ai_reliability::ProtocolComponent;
    if response.id.as_deref() != Some(expected_id)
        || response.command.as_deref() != Some("set_model")
    {
        return Err(AiAdapterError::from_record(
            crate::ai::reliability::protocol_failure_with_detail(
                ProtocolComponent::Pi,
                ProtocolFailureFacts::MalformedResponse,
                "Pi set_model response identity mismatch",
            ),
        ));
    }
    if !response.success {
        return Err(AiAdapterError::from_record(
            crate::ai::reliability::provider_failure(
                ProtocolComponent::Pi,
                response
                    .error
                    .as_deref()
                    .unwrap_or("Pi RPC set_model failed"),
            ),
        ));
    }
    Ok(())
}

async fn send_set_model_and_wait<W>(
    writer: &mut W,
    pending: &PendingResponses,
    id: String,
    selection: &PiRpcModelSelection,
) -> crate::ai::reliability::AiAdapterResult<()>
where
    W: AsyncWrite + Unpin,
{
    use crate::ai::reliability::AiAdapterError;
    use sk_protocol::ai_reliability::ProtocolComponent;
    let (response_tx, response_rx) = oneshot::channel();
    pending
        .lock()
        .insert(id.clone(), PendingResponse::Rpc(response_tx));
    if let Err(error) = write_json(writer, &build_set_model_command(id.clone(), selection)).await {
        pending.lock().remove(&id);
        return Err(AiAdapterError::from_record(
            crate::ai::reliability::runtime_closed_failure(
                ProtocolComponent::Pi,
                &error.to_string(),
            ),
        ));
    }
    let awaited = tokio::time::timeout(std::time::Duration::from_secs(10), response_rx).await;
    pending.lock().remove(&id);
    let response = match awaited {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            return Err(AiAdapterError::from_record(
                crate::ai::reliability::runtime_closed_failure(
                    ProtocolComponent::Pi,
                    &error.to_string(),
                ),
            ))
        }
        Err(_) => {
            return Err(AiAdapterError::from_record(
                crate::ai::reliability::provider_failure(
                    ProtocolComponent::Pi,
                    "Pi RPC set_model timed out",
                ),
            ))
        }
    };
    validate_set_model_response(&id, response)
}

/// Untagged Pi event frames still cannot prove cross-turn identity. Only RPC
/// replies carrying the exact current prompt ID may claim terminal ownership.
fn take_failed_prompt_response(
    response: &PiRpcResponse,
    active_turn: &ActiveTurn,
) -> Option<ActiveTurnState> {
    if response.success || response.command.as_deref() != Some("prompt") {
        return None;
    }
    let mut active = active_turn.lock();
    if response
        .id
        .as_deref()
        .is_some_and(|id| active.as_ref().is_some_and(|turn| turn.prompt_id == id))
    {
        active.take()
    } else {
        None
    }
}

fn log_pi_rpc_stderr_line(line: &str) {
    tracing::debug!(
        target: "script_kit::tab_ai",
        event = "pi_rpc_stderr",
        line_chars = line.chars().count(),
        line_bytes = line.len(),
        "Pi RPC stderr line suppressed"
    );
}

fn user_facing_pi_stderr_hint(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let safe_auth_hint = lower.contains("no api key")
        || lower.contains("api key found")
        || lower.contains("set env var")
        || lower.contains("missing api key");
    safe_auth_hint.then(|| trimmed.to_string())
}

fn pi_rpc_process_exit_error(
    prefix: &str,
    stderr_failure_hint: Option<&StderrFailureHint>,
) -> String {
    let Some(hint) = stderr_failure_hint.and_then(|hint| hint.lock().clone()) else {
        return prefix.to_string();
    };
    format!("{prefix}: {hint}")
}

/// How long to let the stderr reader catch up after stdout closes.
///
/// stdout EOF and the stderr line that explains it are produced by two
/// independent readers. When pi prints "No API key found for provider
/// anthropic" and exits, whichever reader is scheduled first wins the race —
/// and if stdout wins, the hint is still `None` at the moment we classify.
///
/// That race used to be harmless because a hintless exit and a hinted exit
/// both ended up as `Unknown`; only the wording differed. It stopped being
/// harmless when the hintless case started classifying as `RuntimeClosed`,
/// because the two now offer DIFFERENT actions: losing the race turns a
/// "Sign in" card into a "Reconnect" card, and reconnecting never fixes a
/// missing API key.
const PI_STDERR_HINT_GRACE_MS: u64 = 250;
const PI_STDERR_HINT_POLL_MS: u64 = 10;

/// Wait, briefly and boundedly, for the stderr reader to record why the child
/// died. Returns whether evidence arrived.
///
/// Only waits when there is a hint slot to fill and it is still empty, so a
/// caller with no stderr channel (and every unit test) pays nothing.
async fn await_stderr_hint(stderr_failure_hint: Option<&StderrFailureHint>) -> bool {
    let Some(hint) = stderr_failure_hint else {
        return false;
    };
    if hint.lock().is_some() {
        return true;
    }
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_millis(PI_STDERR_HINT_GRACE_MS);
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(PI_STDERR_HINT_POLL_MS)).await;
        if hint.lock().is_some() {
            return true;
        }
    }
    false
}

async fn read_stdout<R>(
    stdout: R,
    pending: PendingResponses,
    active_turn: ActiveTurn,
    stderr_failure_hint: Option<StderrFailureHint>,
) where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let parsed = match parse_rpc_line(&line) {
            Ok(parsed) => parsed,
            Err(error) => {
                send_to_active(
                    &active_turn,
                    pi_failure(format!("Invalid Pi RPC output: {error}")),
                )
                .await;
                continue;
            }
        };

        if let super::protocol::PiRpcLine::Response(response) = &parsed {
            if let Some(id) = response.id.as_ref() {
                let pending_response = pending.lock().remove(id);
                if let Some(pending_response) = pending_response {
                    match pending_response {
                        PendingResponse::Events(event_tx) => {
                            send_events(&event_tx, map_rpc_line_to_events(parsed)).await;
                        }
                        PendingResponse::Rpc(response_tx) => {
                            let _ = response_tx.send(response.clone());
                        }
                    }
                    continue;
                }
            }

            if let Some(active) = take_failed_prompt_response(response, &active_turn) {
                deliver_terminal_to_turn(
                    active,
                    pi_failure(
                        response
                            .error
                            .clone()
                            .unwrap_or_else(|| "Pi RPC prompt failed".to_string()),
                    ),
                )
                .await;
            }
            continue;
        }

        let events = map_rpc_line_to_events(parsed);
        let closes_turn = events.iter().any(|event| {
            matches!(
                event,
                AgentChatEvent::TurnCompleted { .. } | AgentChatEvent::TurnFailed { .. }
            )
        });
        let active = if closes_turn {
            active_turn.lock().take()
        } else {
            active_turn.lock().as_ref().cloned()
        };
        if let Some(active) = active {
            send_events_traced(&active.event_tx, &active.trace, events).await;
            if closes_turn {
                active.trace.teardown();
            }
        }
    }

    tracing::warn!(
        target: "script_kit::tab_ai",
        event = "pi_rpc_stdout_closed",
        "Pi RPC stdout closed before all pending responses completed"
    );
    let had_stderr_hint = await_stderr_hint(stderr_failure_hint.as_ref()).await;
    let error = pi_rpc_process_exit_error(
        "Pi RPC process exited before responding",
        stderr_failure_hint.as_ref(),
    );
    // S12: a pi binary that exits on launch takes this path. With no stderr
    // evidence the free-text classifier matched nothing, so Agent Chat showed
    // the generic `Unknown` card with no reconnect path — for the one failure
    // reconnecting fixes. The exit is a fact, so classify it as RuntimeClosed.
    //
    // When stderr DID say why (for example "No API key found for provider"),
    // that is real evidence about the cause and must still win: an auth
    // failure has to keep its Sign In action.
    let failure = if had_stderr_hint {
        crate::ai::reliability::provider_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            &error,
        )
    } else {
        crate::ai::reliability::runtime_closed_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            &error,
        )
    };
    // A turn that was mid-stream when the process died lives in `active_turn`,
    // not `pending` — without a terminal event its receiver waits forever.
    send_to_active(
        &active_turn,
        AgentChatEvent::TurnFailed {
            failure: failure.clone(),
        },
    )
    .await;
    fail_pending_responses(&pending, &failure, &error).await;
}

/// Map one outbound Agent Chat event onto the shared phase vocabulary.
///
/// This is the single classification point for the Pi transport. Before it
/// existed, event emission was scattered across 21 sites in this file (8 raw
/// `event_tx.send`, 4 through `send_events`, and the `send_to_active` /
/// `fail_pending_responses` terminal paths), and `AgentChatEventTx` is a bare
/// type alias rather than a newtype, so there was nothing to wrap. Rather than
/// sprinkle 21 trace calls that the next edit to this file would forget to
/// extend, every send now funnels through here.
///
/// Only *first* occurrences of the streaming milestones are recorded; the
/// `PhaseTrace` latches make the 2nd..Nth delta a single relaxed atomic load,
/// which matters because a long answer is thousands of deltas.
fn trace_agent_chat_event(trace: &PhaseTrace, event: &AgentChatEvent) {
    // Any inbound event proves the provider responded at all.
    trace.observe_provider_event();
    match event {
        // The first readable token. This is the perceived-responsiveness
        // number, and it is deliberately NOT the same as a thought delta:
        // reasoning text is feedback, but it is not the answer.
        AgentChatEvent::AgentMessageDelta(text) => trace.observe_visible_output(text),
        AgentChatEvent::AgentThoughtDelta(text) => trace.observe_thought(text),
        AgentChatEvent::ToolCallStarted { .. } => trace.observe_tool_call(),
        AgentChatEvent::TurnCompleted { .. } => trace.terminal(TurnOutcome::Completed, None),
        AgentChatEvent::TurnFailed { failure } => {
            // Carry the AppFailureRecord's ALREADY-classified code. Per
            // rules/AI_RELIABILITY.md the code must never be re-derived from
            // prose, and the raw provider text never enters the trace. The
            // serde representation is the declared wire label for the enum, so
            // it is stable in a way `Debug` output is not.
            let code = serde_json::to_value(failure.failure.code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string));
            trace.terminal(TurnOutcome::Failed, code.as_deref());
        }
        _ => {}
    }
}

/// Send one event through the trace classifier.
///
/// Used by the setup/failure paths that hold a bare sender and would otherwise
/// bypass instrumentation entirely.
async fn send_event_traced(event_tx: &AgentChatEventTx, trace: &PhaseTrace, event: AgentChatEvent) {
    trace_agent_chat_event(trace, &event);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), event_tx.send(event)).await;
}

async fn send_events(event_tx: &AgentChatEventTx, events: Vec<AgentChatEvent>) {
    send_events_traced(event_tx, &PhaseTrace::disabled(), events).await;
}

async fn send_events_traced(
    event_tx: &AgentChatEventTx,
    trace: &PhaseTrace,
    events: Vec<AgentChatEvent>,
) {
    let reveal_count = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                AgentChatEvent::AgentMessageDelta(_) | AgentChatEvent::AgentThoughtDelta(_)
            )
        })
        .count();
    let mut reveal_index = 0usize;
    for event in events {
        let reveal_chunk = matches!(
            event,
            AgentChatEvent::AgentMessageDelta(_) | AgentChatEvent::AgentThoughtDelta(_)
        );
        let sleep_after = reveal_chunk && {
            reveal_index += 1;
            reveal_index < reveal_count
        };
        trace_agent_chat_event(trace, &event);
        if tokio::time::timeout(std::time::Duration::from_secs(2), event_tx.send(event))
            .await
            .is_err()
        {
            break;
        }
        if sleep_after {
            tokio::time::sleep(std::time::Duration::from_millis(PI_REVEAL_CHUNK_DELAY_MS)).await;
        }
    }
}

/// Deliver a terminal event to whichever turn is currently live.
///
/// Every caller of this is a terminal path — a parse failure, a failed prompt
/// response, or the process exiting mid-stream — so the trace is read from the
/// active turn here rather than passed in. `teardown` fires because reaching
/// this function means the turn's transport resources are being released.
async fn send_to_active(active_turn: &ActiveTurn, event: AgentChatEvent) {
    let active = active_turn.lock().take();
    if let Some(active) = active {
        deliver_terminal_to_turn(active, event).await;
    }
}

async fn deliver_terminal_to_turn(active: ActiveTurnState, event: AgentChatEvent) {
    trace_agent_chat_event(&active.trace, &event);
    // A blocked consumer must not hold transport teardown forever.
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        active.event_tx.send(event),
    )
    .await;
    active.trace.teardown();
}

async fn fail_pending_responses(
    pending: &PendingResponses,
    failure: &crate::ai::reliability::AppFailureRecord,
    error: &str,
) {
    let pending_responses = pending.lock().drain().collect::<Vec<_>>();

    for (id, pending_response) in pending_responses {
        match pending_response {
            PendingResponse::Events(event_tx) => {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    event_tx.send(AgentChatEvent::TurnFailed {
                        failure: failure.clone(),
                    }),
                )
                .await;
            }
            PendingResponse::Rpc(response_tx) => {
                let _ = response_tx.send(PiRpcResponse {
                    id: Some(id),
                    command: None,
                    success: false,
                    data: None,
                    error: Some(error.to_string()),
                    raw: serde_json::json!({
                        "type": "response",
                        "success": false,
                        "error": error,
                    }),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::pin::Pin;
    use std::task::{Context as TaskContext, Poll};

    struct RespondingWriter {
        bytes: Vec<u8>,
        pending: PendingResponses,
        id: String,
        success: bool,
        error: Option<String>,
        responded: bool,
    }

    impl RespondingWriter {
        fn new(pending: PendingResponses, success: bool, error: Option<String>) -> Self {
            Self {
                bytes: Vec::new(),
                pending,
                id: "set-model-test".to_string(),
                success,
                error,
                responded: false,
            }
        }

        fn written(&self) -> &[u8] {
            &self.bytes
        }
    }

    impl AsyncWrite for RespondingWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut TaskContext<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.bytes.extend_from_slice(buf);
            if !self.responded {
                self.responded = true;
                let pending_response = {
                    let id = self.id.clone();
                    self.pending.lock().remove(&id)
                };
                let Some(PendingResponse::Rpc(response_tx)) = pending_response else {
                    panic!("expected pending RPC response waiter");
                };
                response_tx
                    .send(PiRpcResponse {
                        id: Some(self.id.clone()),
                        command: Some("set_model".to_string()),
                        success: self.success,
                        data: None,
                        error: self.error.clone(),
                        raw: json!({
                            "type": "response",
                            "id": self.id.clone(),
                            "command": "set_model",
                            "success": self.success,
                        }),
                    })
                    .unwrap();
            }
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut TaskContext<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut TaskContext<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn pi_rpc_runtime_implements_agent_chat_connection_trait() {
        fn accepts_connection(_: &dyn AgentChatConnection) {}
        let (tx, _rx) = async_channel::bounded::<PiRpcRuntimeCommand>(1);
        let runtime = PiRpcRuntime::from_sender(tx);
        accepts_connection(&runtime);
    }

    #[test]
    fn agent_chat_trait_start_turn_enqueues_pi_start_turn_command() {
        let (tx, rx) = async_channel::bounded::<PiRpcRuntimeCommand>(1);
        let runtime = PiRpcRuntime::from_sender(tx);

        let event_rx = runtime
            .start_turn(AgentChatTurnRequest {
                ui_thread_id: "thread-1".to_string(),
                cwd: std::path::PathBuf::from("/tmp"),
                blocks: Vec::new(),
                model_id: None,
                tool_policy: crate::ai::agent_chat::ui::capabilities::AgentChatToolPolicy::Full,
            })
            .unwrap();
        drop(event_rx);

        let command = rx.recv_blocking().unwrap();
        assert!(matches!(command, PiRpcRuntimeCommand::StartTurn { .. }));
    }

    #[test]
    fn agent_chat_trait_prepare_session_enqueues_pi_prepare_session_command() {
        let (tx, rx) = async_channel::bounded::<PiRpcRuntimeCommand>(1);
        let runtime = PiRpcRuntime::from_sender(tx);

        let event_rx = runtime
            .prepare_session("thread-1".to_string(), std::path::PathBuf::from("/tmp"))
            .unwrap();
        drop(event_rx);

        let command = rx.recv_blocking().unwrap();
        assert!(matches!(
            command,
            PiRpcRuntimeCommand::PrepareSession { ui_thread_id, .. } if ui_thread_id == "thread-1"
        ));
    }

    #[test]
    fn agent_chat_trait_cancel_turn_enqueues_pi_cancel_command() {
        let (tx, rx) = async_channel::bounded::<PiRpcRuntimeCommand>(1);
        let runtime = PiRpcRuntime::from_sender(tx);

        runtime.cancel_turn("thread-1".to_string()).unwrap();

        let command = rx.recv_blocking().unwrap();
        assert!(matches!(
            command,
            PiRpcRuntimeCommand::CancelTurn { ui_thread_id } if ui_thread_id == "thread-1"
        ));
    }

    #[test]
    fn pi_rpc_stderr_logging_suppresses_raw_line_content() {
        let source = include_str!("runtime.rs");
        assert!(source.contains("fn log_pi_rpc_stderr_line"));
        assert!(source.contains("line_chars = line.chars().count()"));
        assert!(source.contains("line_bytes = line.len()"));
        assert!(!source.contains(&format!("{}{}", "line = %", "line")));
        assert!(!source.contains(&format!("{}{}", "line = ?", "line")));
    }

    #[test]
    fn pi_rpc_stderr_auth_hint_is_user_facing_without_logging_raw_line() {
        let hint =
            user_facing_pi_stderr_hint("No API key found for provider anthropic. Set env var.");
        assert_eq!(
            hint.as_deref(),
            Some("No API key found for provider anthropic. Set env var.")
        );
        assert!(user_facing_pi_stderr_hint("debug: provider startup").is_none());
    }

    #[test]
    fn pi_rpc_reveal_delay_is_few_ms() {
        assert!(
            PI_REVEAL_CHUNK_DELAY_MS <= 8,
            "Pi reveal delay should stay in the few-ms range"
        );
    }

    #[test]
    fn set_model_wait_succeeds_only_after_pi_response() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let mut output = RespondingWriter::new(pending.clone(), true, None);
            let selection = PiRpcModelSelection {
                provider: "openai".to_string(),
                model_id: "gpt-5.4".to_string(),
            };

            send_set_model_and_wait(
                &mut output,
                &pending,
                "set-model-test".to_string(),
                &selection,
            )
            .await
            .unwrap();
            let written = String::from_utf8(output.written().to_vec()).unwrap();
            assert!(written.contains(r#""type":"set_model""#));
            assert!(written.contains(r#""provider":"openai""#));
            assert!(written.contains(r#""modelId":"gpt-5.4""#));
        });
    }

    #[test]
    fn set_model_wait_surfaces_pi_response_failure() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let mut output = RespondingWriter::new(
                pending.clone(),
                false,
                Some("model unavailable".to_string()),
            );
            let selection = PiRpcModelSelection {
                provider: "openai".to_string(),
                model_id: "missing-model".to_string(),
            };

            let result = send_set_model_and_wait(
                &mut output,
                &pending,
                "set-model-test".to_string(),
                &selection,
            )
            .await;
            let error = result.unwrap_err();
            assert_eq!(
                error.code(),
                sk_protocol::ai_reliability::AiFailureCode::ModelUnavailable
            );
        });
    }

    #[test]
    fn production_story_pi_reply_identity() {
        let response = |id: Option<&str>, command: Option<&str>, success| PiRpcResponse {
            id: id.map(str::to_string),
            command: command.map(str::to_string),
            success,
            data: None,
            error: (!success).then(|| "model unavailable".into()),
            raw: json!({}),
        };
        assert!(validate_set_model_response(
            "current",
            response(Some("current"), Some("set_model"), true)
        )
        .is_ok());
        assert_eq!(
            validate_set_model_response(
                "current",
                response(Some("current"), Some("set_model"), false)
            )
            .unwrap_err()
            .code(),
            sk_protocol::ai_reliability::AiFailureCode::ModelUnavailable
        );
        for malformed in [
            response(None, Some("set_model"), true),
            response(Some("old"), Some("set_model"), true),
            response(Some("current"), None, true),
            response(Some("current"), Some("prompt"), true),
        ] {
            assert_eq!(
                validate_set_model_response("current", malformed)
                    .unwrap_err()
                    .code(),
                sk_protocol::ai_reliability::AiFailureCode::ProtocolMalformedResponse
            );
        }
        let (tx, _rx) = async_channel::bounded(1);
        let active: ActiveTurn = Arc::new(Mutex::new(Some(ActiveTurnState {
            ui_thread_id: "thread".into(),
            prompt_id: "current".into(),
            event_tx: tx,
            trace: PhaseTrace::disabled(),
        })));
        assert!(take_failed_prompt_response(
            &response(Some("old"), Some("prompt"), false),
            &active
        )
        .is_none());
        assert!(
            take_failed_prompt_response(&response(None, Some("prompt"), false), &active).is_none()
        );
        assert!(take_failed_prompt_response(
            &response(Some("current"), Some("prompt"), true),
            &active
        )
        .is_none());
        assert!(take_failed_prompt_response(
            &response(Some("current"), Some("set_model"), false),
            &active
        )
        .is_none());
        assert!(active.lock().is_some());
        assert_eq!(
            take_failed_prompt_response(&response(Some("current"), Some("prompt"), false), &active)
                .unwrap()
                .prompt_id,
            "current"
        );
        assert!(active.lock().is_none());
    }

    #[test]
    fn read_stdout_teardown_does_not_wait_forever_for_full_pending_consumer() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
            let (event_tx, event_rx) = async_channel::bounded(1);
            event_tx.try_send(AgentChatEvent::AgentMessageDelta("occupied".into())).unwrap();
            pending.lock().insert("models-full".into(), PendingResponse::Events(event_tx));
            tokio::time::timeout(std::time::Duration::from_secs(4),
                read_stdout(tokio::io::empty(), pending.clone(), active_turn, None))
                .await.expect("Pi stdout teardown must bound pending consumer delivery");
            assert!(pending.lock().is_empty());
            assert!(matches!(event_rx.try_recv().unwrap(), AgentChatEvent::AgentMessageDelta(text) if text == "occupied"));
            assert!(event_rx.is_closed());
        });
    }

    #[test]
    fn read_stdout_fails_pending_events_when_pi_exits_before_response() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
            let (event_tx, event_rx) = async_channel::bounded(1);
            pending
                .lock()
                .insert("models-test".to_string(), PendingResponse::Events(event_tx));
            let stderr_hint: StderrFailureHint = Arc::new(Mutex::new(Some(
                "No API key found for provider anthropic. Set env var.".to_string(),
            )));

            read_stdout(
                tokio::io::empty(),
                pending.clone(),
                active_turn,
                Some(stderr_hint),
            )
            .await;

            assert!(pending.lock().is_empty());
            let event = event_rx.recv().await.unwrap();
            assert!(matches!(
                event,
                AgentChatEvent::TurnFailed { failure }
                    if failure.failure.code
                        == sk_protocol::ai_reliability::AiFailureCode::AuthenticationMissing
            ));
        });
    }

    /// The stderr reader is a separate task, so a pi child that prints its
    /// reason and dies can close stdout FIRST. Classifying at that instant saw
    /// no evidence and produced `RuntimeClosed` — a "Reconnect" card for a
    /// missing API key, which reconnecting cannot fix. Losing this race must
    /// not change what the user is offered.
    #[test]
    fn read_stdout_waits_for_late_stderr_evidence_before_calling_it_a_dead_runtime() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
            let (event_tx, event_rx) = async_channel::bounded(1);
            active_turn.lock().replace(ActiveTurnState {
                ui_thread_id: "thread-race".to_string(),
                prompt_id: "prompt-race".to_string(),
                event_tx,
                trace: PhaseTrace::disabled(),
            });
            // Empty: the hint has NOT arrived when stdout closes.
            let stderr_hint: StderrFailureHint = Arc::new(Mutex::new(None));
            let late = stderr_hint.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                late.lock()
                    .replace("No API key found for provider anthropic. Set env var.".to_string());
            });

            read_stdout(tokio::io::empty(), pending, active_turn, Some(stderr_hint)).await;

            let event = event_rx.recv().await.unwrap();
            assert!(
                matches!(
                    event,
                    AgentChatEvent::TurnFailed { failure }
                        if failure.failure.code
                            == sk_protocol::ai_reliability::AiFailureCode::AuthenticationMissing
                ),
                "stderr evidence that arrives just after stdout EOF must still decide the failure"
            );
        });
    }

    /// The Pi transport must produce a usable phase trace for a real streamed
    /// turn, not merely compile against `PhaseTrace`.
    ///
    /// This drives `read_stdout` with the actual Pi wire shapes and asserts the
    /// milestones the premise requires, in order, with monotonic `elapsedMs`.
    /// Without it, a future edit could route a send around
    /// `send_events_traced` and silently produce empty traces — which look
    /// exactly like a fast surface.
    #[test]
    fn pi_transport_emits_the_phase_trace_for_a_streamed_turn() {
        let path = std::env::temp_dir().join(format!(
            "sk-pi-phase-trace-{}-{:?}.ndjson",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
            // Bound generously: the reveal path sends every delta before the
            // reader finishes, and a full channel would deadlock the test.
            let (event_tx, _event_rx) = async_channel::bounded(64);
            let trace = PhaseTrace::begin_at(
                path.clone(),
                AiSurface::Text,
                AiTransport::PiRpc,
                "pi-trace-test",
            );
            trace.turn_start(serde_json::json!({}));
            active_turn.lock().replace(ActiveTurnState {
                ui_thread_id: "thread-trace".to_string(),
                prompt_id: "prompt-trace".to_string(),
                event_tx,
                trace,
            });

            let stdout = concat!(
                r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","delta":"pondering"}}"#,
                "\n",
                r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":"Hello"}}"#,
                "\n",
                r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","delta":" world"}}"#,
                "\n",
                r#"{"type":"agent_end"}"#,
                "\n",
            );
            read_stdout(stdout.as_bytes(), pending, active_turn, None).await;
        });

        let records: Vec<serde_json::Value> = std::fs::read_to_string(&path)
            .expect("the trace file must exist")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("every record parses as JSON"))
            .collect();
        let names: Vec<&str> = records
            .iter()
            .map(|record| record["event"].as_str().unwrap())
            .collect();

        for required in [
            crate::ai::phase_trace::events::TURN_START,
            crate::ai::phase_trace::events::FIRST_PROVIDER_EVENT,
            crate::ai::phase_trace::events::FIRST_VISIBLE_OUTPUT,
            crate::ai::phase_trace::events::TERMINAL,
            crate::ai::phase_trace::events::TEARDOWN,
        ] {
            assert!(
                names.contains(&required),
                "Pi turn trace is missing {required}; got {names:?}"
            );
        }

        // Two text deltas arrived but only the FIRST may be recorded, or a long
        // answer would write thousands of records and leak the answer text.
        assert_eq!(
            names
                .iter()
                .filter(|name| **name == crate::ai::phase_trace::events::FIRST_VISIBLE_OUTPUT)
                .count(),
            1,
            "first_visible_output must latch"
        );

        // Reasoning must not be mistaken for the answer: a surface that streams
        // thoughts early would otherwise score a misleadingly fast
        // time-to-first-visible-output.
        let thought_seq = records
            .iter()
            .position(|r| r["event"] == crate::ai::phase_trace::events::FIRST_THOUGHT)
            .expect("a reasoning delta must record first_thought");
        let visible_seq = records
            .iter()
            .position(|r| r["event"] == crate::ai::phase_trace::events::FIRST_VISIBLE_OUTPUT)
            .unwrap();
        assert!(
            thought_seq < visible_seq,
            "the thought delta arrived first and must be recorded separately"
        );

        assert_eq!(
            records
                .iter()
                .find(|r| r["event"] == crate::ai::phase_trace::events::TERMINAL)
                .map(|r| r["outcome"].clone()),
            Some(serde_json::json!("completed")),
            "agent_end is a completed turn and therefore a valid latency sample"
        );

        for record in &records {
            assert_eq!(record["surface"], "text", "surface label must survive");
            assert_eq!(record["transport"], "pi-rpc");
        }
        let elapsed: Vec<u64> = records
            .iter()
            .map(|r| r["elapsedMs"].as_u64().unwrap())
            .collect();
        assert!(
            elapsed.windows(2).all(|pair| pair[0] <= pair[1]),
            "elapsedMs must be non-decreasing: {elapsed:?}"
        );

        // The answer text must never appear, only its digest.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("Hello"), "trace leaked answer text");
        assert!(!raw.contains("pondering"), "trace leaked reasoning text");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_stdout_fails_active_turn_when_pi_exits_mid_stream() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
            let (event_tx, event_rx) = async_channel::bounded(1);
            active_turn.lock().replace(ActiveTurnState {
                ui_thread_id: "thread-test".to_string(),
                prompt_id: "prompt-test".to_string(),
                event_tx,
                trace: PhaseTrace::disabled(),
            });

            read_stdout(tokio::io::empty(), pending, active_turn.clone(), None).await;

            let event = event_rx.recv().await.unwrap();
            assert!(
                matches!(
                    event,
                    // S12: a pi child that exited is RuntimeClosed. This
                    // assertion used to demand `Unknown`, locking in the
                    // generic "did not finish" card with no reconnect path.
                    AgentChatEvent::TurnFailed { failure }
                        if failure.failure.code
                            == sk_protocol::ai_reliability::AiFailureCode::RuntimeClosed
                ),
                "active streaming turn must receive a terminal Failed event when Pi dies"
            );
            assert!(
                active_turn.lock().is_none(),
                "active turn must be cleared after the terminal event"
            );
        });
    }

    #[test]
    fn read_stdout_fails_pending_rpc_when_pi_exits_before_response() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let pending: PendingResponses = Arc::new(Mutex::new(HashMap::new()));
            let active_turn: ActiveTurn = Arc::new(Mutex::new(None));
            let (response_tx, response_rx) = oneshot::channel();
            pending.lock().insert(
                "set-model-test".to_string(),
                PendingResponse::Rpc(response_tx),
            );

            read_stdout(tokio::io::empty(), pending.clone(), active_turn, None).await;

            assert!(pending.lock().is_empty());
            let response = response_rx.await.unwrap();
            assert!(!response.success);
            assert!(response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("exited before responding"));
        });
    }
}
