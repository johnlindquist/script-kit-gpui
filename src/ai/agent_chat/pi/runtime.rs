use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx};
use crate::ai::agent_chat::runtime::{AgentChatConnection, AgentChatTurnRequest};
use crate::ai::agent_chat::ui::events::AgentChatEventTx;

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
                if let Some(model_id) = request.model_id.as_deref() {
                    let selection = match PiRpcModelSelection::parse(model_id) {
                        Ok(selection) => selection,
                        Err(error) => {
                            let _ = event_tx
                                .send(pi_failure(format!("Invalid Pi model selection: {error}")))
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
                                let _ = event_tx.send(pi_transport_failure(error)).await;
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
                        });
                        write_json(&mut stdin, &build_prompt_command(prompt_id, payload)).await?;
                    }
                    Err(error) => {
                        let _ = event_tx.send(pi_failure(error.to_string())).await;
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

    let _ = child.kill().await;
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
                let _ = event_tx
                    .send(pi_failure(format!("Invalid Pi model selection: {error}")))
                    .await;
                let _ = child.kill().await;
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
            let _ = event_tx.send(pi_transport_failure(error)).await;
            let _ = child.kill().await;
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
            let _ = event_tx.send(pi_failure(error.to_string())).await;
            let _ = child.kill().await;
            return Ok(());
        }
    };

    active_turn.lock().replace(ActiveTurnState {
        ui_thread_id: request.ui_thread_id.clone(),
        prompt_id: prompt_id.clone(),
        event_tx: event_tx.clone(),
    });

    if let Err(error) = write_json(&mut stdin, &build_prompt_command(prompt_id, payload)).await {
        let _ = event_tx.send(pi_transport_failure(&error)).await;
        let _ = child.kill().await;
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
                    let _ = event_tx.send(cancelled_outcome()).await;
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

    let _ = child.kill().await;
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

            if response.command.as_deref() == Some("prompt") && !response.success {
                send_to_active(
                    &active_turn,
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
        let event_tx = active_turn
            .lock()
            .as_ref()
            .map(|active| active.event_tx.clone());
        if let Some(event_tx) = event_tx {
            send_events(&event_tx, events).await;
        }
        if closes_turn {
            active_turn.lock().take();
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

async fn send_set_model_and_wait<W>(
    writer: &mut W,
    pending: &PendingResponses,
    id: String,
    selection: &PiRpcModelSelection,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let (response_tx, response_rx) = oneshot::channel();
    pending
        .lock()
        .insert(id.clone(), PendingResponse::Rpc(response_tx));

    if let Err(error) = write_json(writer, &build_set_model_command(id.clone(), selection)).await {
        pending.lock().remove(&id);
        return Err(error);
    }

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), response_rx)
        .await
        .context("Pi RPC set_model timed out")?
        .context("Pi RPC set_model response channel closed")?;

    if response.success {
        return Ok(());
    }

    anyhow::bail!(
        "{}",
        response
            .error
            .unwrap_or_else(|| "Pi RPC set_model failed".to_string())
    )
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

            if response.command.as_deref() == Some("prompt") && !response.success {
                send_to_active(
                    &active_turn,
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
        let event_tx = active_turn
            .lock()
            .as_ref()
            .map(|active| active.event_tx.clone());
        if let Some(event_tx) = event_tx {
            send_events(&event_tx, events).await;
        }
        if closes_turn {
            active_turn.lock().take();
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

async fn send_events(event_tx: &AgentChatEventTx, events: Vec<AgentChatEvent>) {
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
        let _ = event_tx.send(event).await;
        if sleep_after {
            tokio::time::sleep(std::time::Duration::from_millis(PI_REVEAL_CHUNK_DELAY_MS)).await;
        }
    }
}

async fn send_to_active(active_turn: &ActiveTurn, event: AgentChatEvent) {
    let event_tx = active_turn
        .lock()
        .as_ref()
        .map(|active| active.event_tx.clone());
    if let Some(event_tx) = event_tx {
        let _ = event_tx.send(event).await;
    }
    active_turn.lock().take();
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
                let _ = event_tx
                    .send(AgentChatEvent::TurnFailed {
                        failure: failure.clone(),
                    })
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
            let error = result.unwrap_err().to_string();
            assert!(error.contains("model unavailable"));
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
