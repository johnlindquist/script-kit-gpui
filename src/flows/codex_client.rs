//! Persistent `codex app-server` JSON-RPC client — the native transport for
//! codex-engine flow conversations (Threadline).
//!
//! Wire contract (verified against codex-cli 0.144.0 on 2026-07-09 with a
//! live round trip; source of truth: `~/dev/codex/codex-rs/app-server-protocol`):
//! - JSONL over stdio. Envelope has NO `"jsonrpc"` field: requests are
//!   `{id, method, params}`, responses `{id, result|error}`, notifications
//!   `{method, params}`. Params are camelCase.
//! - Handshake: `initialize` → (response) → `initialized` notification. The
//!   transport is ordered, so we pipeline all three writes at spawn instead
//!   of blocking the UI thread on the response.
//! - One protocol thread per flow session: `thread/start {cwd,
//!   approvalPolicy: "never"}` plus the flow's declared contract when the
//!   frontmatter pins one (`model`, `sandbox`, and the mission body as
//!   `developerInstructions`). Unpinned fields defer to the user's
//!   `~/.codex/config.toml`, matching how `md`/mdflow runs `codex exec`.
//! - One turn per user message: `turn/start {threadId, input: [{type:
//!   "text", text}]}`. Assistant text arrives as `item/agentMessage/delta`
//!   plus an authoritative full text on `item/completed
//!   {item.type: "agentMessage"}`; the turn settles with `turn/completed
//!   {turn.status}`. Unknown notifications (`hook/*`, `skills/changed`, …)
//!   MUST be ignored — the server emits many.
//! - Server→client requests (approvals etc.) are answered with a JSON-RPC
//!   error so a turn can never hang waiting on a dialog we don't render.
//!
//! Threading: callers (GPUI main thread) only ever take a short mutex to
//! enqueue writes; a reader thread owns stdout and appends [`FlowThreadEvent`]s
//! to a queue the flow tick drains. If the child dies, every linked session
//! gets a failure event and the next `converse` respawns the server and
//! re-threads transparently (transcripts live app-side, context is lost —
//! honest, visible, recoverable).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// Events surfaced to the flow tick, keyed by app session id.
#[derive(Debug, Clone)]
pub(crate) enum FlowThreadEvent {
    /// `thread/start` answered; the session is linked to a protocol thread.
    ThreadStarted { session_id: u64, model: String },
    /// A turn is running on the server (honest "working" edge).
    TurnStarted { session_id: u64 },
    /// Streamed assistant text. `item_id` marks agentMessage item
    /// boundaries — a turn can carry several items and the renderer must
    /// separate them (paragraph break), never butt-join their text.
    AgentDelta {
        session_id: u64,
        item_id: String,
        delta: String,
    },
    /// Authoritative full text of ONE agentMessage item (`item/completed`).
    AgentMessageFinal {
        session_id: u64,
        item_id: String,
        text: String,
    },
    /// The turn settled without a failure.
    TurnCompleted {
        session_id: u64,
        outcome: crate::ai::reliability::AiTurnRuntimeOutcome,
    },
    /// Turn-level failure from a classified runtime/provider/protocol path.
    TurnFailed {
        session_id: u64,
        failure: crate::ai::reliability::AppFailureRecord,
    },
    /// RPC-level failure attributable to one session (thread start
    /// rejected, server died mid-turn, spawn failure).
    SessionFailed {
        session_id: u64,
        failure: crate::ai::reliability::AppFailureRecord,
    },
}

fn flow_failure(raw: impl AsRef<str>) -> crate::ai::reliability::AppFailureRecord {
    crate::ai::reliability::provider_failure(
        sk_protocol::ai_reliability::ProtocolComponent::Codex,
        raw,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    Initialize,
    ThreadStart { session_id: u64 },
    TurnStart { session_id: u64 },
    TurnInterrupt,
}

/// Per-session link state. `cwd` survives server restarts so re-threading is
/// transparent; queued prompts flush as soon as the thread exists.
#[derive(Debug, Default)]
struct SessionLink {
    cwd: String,
    /// Flow-declared thread contract (model/sandbox/developer
    /// instructions), applied on `thread/start`.
    profile: super::session::FlowThreadProfile,
    thread_id: Option<String>,
    thread_starting: bool,
    queued_prompts: Vec<String>,
}

#[derive(Default)]
struct Shared {
    stdin: Option<ChildStdin>,
    child: Option<Child>,
    /// Keeps the live child registered with the global process manager so
    /// quit paths and post-crash orphan cleanup can reap it (drop = unregister).
    child_registration: Option<crate::process_manager::ChildRegistration>,
    child_generation: u64,
    next_id: u64,
    pending: HashMap<u64, PendingKind>,
    /// protocol thread id → app session id
    threads: HashMap<String, u64>,
    sessions: HashMap<u64, SessionLink>,
    events: Vec<FlowThreadEvent>,
}

pub struct CodexAppServer {
    shared: Mutex<Shared>,
}

static CLIENT: OnceLock<CodexAppServer> = OnceLock::new();

pub fn codex_app_server() -> &'static CodexAppServer {
    CLIENT.get_or_init(|| CodexAppServer {
        shared: Mutex::new(Shared::default()),
    })
}

/// Binary override seam for probes (a fake app-server script keeps desk
/// receipts deterministic and token-free).
fn codex_binary() -> String {
    std::env::var("SCRIPT_KIT_CODEX_BIN").unwrap_or_else(|_| "codex".to_string())
}

impl CodexAppServer {
    /// The single public entry: send one user turn for a session, spawning
    /// the server and/or starting the session's thread as needed. Returns
    /// immediately; outcomes arrive via [`drain_events`].
    pub fn converse(
        &self,
        session_id: u64,
        cwd: &str,
        profile: Option<super::session::FlowThreadProfile>,
        prompt: String,
    ) {
        let mut shared = self.shared.lock().unwrap();
        if let Err(err) = ensure_child(&mut shared) {
            shared.events.push(FlowThreadEvent::SessionFailed {
                session_id,
                failure: flow_failure(err),
            });
            return;
        }
        let link = shared.sessions.entry(session_id).or_default();
        link.cwd = cwd.to_string();
        if let Some(profile) = profile {
            link.profile = profile;
        }
        match link.thread_id.clone() {
            Some(thread_id) => {
                send_turn_start(&mut shared, session_id, &thread_id, &prompt);
            }
            None => {
                let link = shared.sessions.get_mut(&session_id).expect("just inserted");
                link.queued_prompts.push(prompt);
                let needs_start = !link.thread_starting;
                if needs_start {
                    link.thread_starting = true;
                    let params = thread_start_params(link);
                    send_request(
                        &mut shared,
                        PendingKind::ThreadStart { session_id },
                        "thread/start",
                        params,
                    );
                }
            }
        }
    }

    /// Warm a session's protocol thread before the first message: spawn
    /// the server, initialize, and send `thread/start` with the flow's
    /// contract. By the time the user finishes typing, the first submit is
    /// usually just `turn/start` — no perceived spawn/handshake dead time.
    pub fn prepare_thread(
        &self,
        session_id: u64,
        cwd: &str,
        profile: super::session::FlowThreadProfile,
    ) {
        let mut shared = self.shared.lock().unwrap();
        if let Err(err) = ensure_child(&mut shared) {
            shared.events.push(FlowThreadEvent::SessionFailed {
                session_id,
                failure: flow_failure(err),
            });
            return;
        }
        let link = shared.sessions.entry(session_id).or_default();
        link.cwd = cwd.to_string();
        link.profile = profile;
        if link.thread_id.is_some() || link.thread_starting {
            return;
        }
        link.thread_starting = true;
        let params = thread_start_params(link);
        send_request(
            &mut shared,
            PendingKind::ThreadStart { session_id },
            "thread/start",
            params,
        );
    }

    /// Interrupt the session's in-flight turn (⌘K Stop). No-op when the
    /// session has no live thread.
    pub fn interrupt(&self, session_id: u64) {
        let mut shared = self.shared.lock().unwrap();
        let Some(thread_id) = shared
            .sessions
            .get(&session_id)
            .and_then(|link| link.thread_id.clone())
        else {
            return;
        };
        send_request(
            &mut shared,
            PendingKind::TurnInterrupt,
            "turn/interrupt",
            serde_json::json!({ "threadId": thread_id }),
        );
    }

    /// Forget a session (dismissed). The protocol thread is left to idle;
    /// the server prunes on exit.
    pub fn forget_session(&self, session_id: u64) {
        let mut shared = self.shared.lock().unwrap();
        if let Some(link) = shared.sessions.remove(&session_id) {
            if let Some(thread_id) = link.thread_id {
                shared.threads.remove(&thread_id);
            }
        }
    }

    /// Drain queued events (called from the flow tick).
    pub(crate) fn drain_events(&self) -> Vec<FlowThreadEvent> {
        let mut shared = self.shared.lock().unwrap();
        std::mem::take(&mut shared.events)
    }

    /// Test seam: push an event as if the reader thread produced it.
    #[cfg(test)]
    fn push_event_for_test(&self, event: FlowThreadEvent) {
        self.shared.lock().unwrap().events.push(event);
    }
}

/// Spawn `codex app-server` and pipeline the handshake. Ordered transport
/// means `thread/start` written right after is processed after `initialize`.
fn ensure_child(shared: &mut Shared) -> Result<(), String> {
    let alive = shared
        .child
        .as_mut()
        .map(|child| child.try_wait().map(|status| status.is_none()))
        .transpose()
        .map_err(|err| format!("codex app-server wait failed: {err}"))?
        .unwrap_or(false);
    if alive && shared.stdin.is_some() {
        return Ok(());
    }

    // (Re)spawn: invalidate every thread link but keep cwds + queued
    // prompts so sessions re-thread transparently on their next turn.
    shared.stdin = None;
    shared.child = None;
    shared.child_registration = None;
    shared.pending.clear();
    shared.threads.clear();
    for link in shared.sessions.values_mut() {
        link.thread_id = None;
        link.thread_starting = false;
    }

    let binary = codex_binary();
    let mut command = Command::new(&binary);
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Own process group so orphan cleanup / kill_all can reap the server and
    // any turn subprocesses it spawned.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to spawn {binary} app-server: {err}"))?;
    let registration = crate::process_manager::ChildRegistration::register(child.id(), &binary);
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "codex app-server stdin unavailable".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "codex app-server stdout unavailable".to_string())?;

    shared.child_generation += 1;
    let generation = shared.child_generation;
    shared.stdin = Some(stdin);
    shared.child = Some(child);
    shared.child_registration = Some(registration);
    shared.next_id = 1;

    tracing::info!(
        target: "script_kit::flows",
        event = "codex_app_server_spawned",
        generation,
        "codex app-server child started"
    );

    send_request(
        shared,
        PendingKind::Initialize,
        "initialize",
        serde_json::json!({
            "clientInfo": {
                "name": "script-kit",
                "title": "Script Kit",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    );
    send_notification(shared, "initialized", serde_json::json!({}));

    std::thread::Builder::new()
        .name(format!("codex-app-server-reader-{generation}"))
        .spawn(move || reader_loop(stdout, generation))
        .map_err(|err| format!("failed to spawn reader thread: {err}"))?;
    Ok(())
}

fn send_request(shared: &mut Shared, kind: PendingKind, method: &str, params: serde_json::Value) {
    let id = shared.next_id;
    shared.next_id += 1;
    shared.pending.insert(id, kind);
    let line = serde_json::json!({ "id": id, "method": method, "params": params });
    write_line(shared, &line, method);
}

fn send_notification(shared: &mut Shared, method: &str, params: serde_json::Value) {
    let line = serde_json::json!({ "method": method, "params": params });
    write_line(shared, &line, method);
}

fn write_line(shared: &mut Shared, line: &serde_json::Value, method: &str) {
    let Some(stdin) = shared.stdin.as_mut() else {
        return;
    };
    let mut payload = line.to_string();
    payload.push('\n');
    if let Err(err) = stdin.write_all(payload.as_bytes()) {
        tracing::warn!(
            target: "script_kit::flows",
            event = "codex_app_server_write_failed",
            method,
            %err,
            "codex app-server write failed — child presumed dead"
        );
        shared.stdin = None;
    }
}

fn send_turn_start(shared: &mut Shared, session_id: u64, thread_id: &str, prompt: &str) {
    // Log lengths only — prompts can carry anything.
    tracing::info!(
        target: "script_kit::flows",
        event = "codex_turn_start",
        session_id,
        prompt_len = prompt.len(),
        "Starting codex turn"
    );
    send_request(
        shared,
        PendingKind::TurnStart { session_id },
        "turn/start",
        serde_json::json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": prompt }],
        }),
    );
}

/// Reader thread: owns stdout for one child generation. Every mutation goes
/// through the shared state; a stale generation exits without touching it.
fn reader_loop(stdout: std::process::ChildStdout, generation: u64) {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let client = codex_app_server();
        let mut shared = client.shared.lock().unwrap();
        if shared.child_generation != generation {
            return; // superseded by a respawn
        }
        handle_message(&mut shared, &msg);
    }

    // EOF: the child died. Fail every session with work in flight so no
    // chip can lie about a dead server.
    let client = codex_app_server();
    let mut shared = client.shared.lock().unwrap();
    if shared.child_generation != generation {
        return;
    }
    tracing::warn!(
        target: "script_kit::flows",
        event = "codex_app_server_died",
        generation,
        "codex app-server exited"
    );
    shared.stdin = None;
    if let Some(mut child) = shared.child.take() {
        let _ = child.try_wait();
    }
    let session_ids: Vec<u64> = shared.sessions.keys().copied().collect();
    for session_id in session_ids {
        shared.events.push(FlowThreadEvent::SessionFailed {
            session_id,
            failure: flow_failure("codex app-server exited — send again to reconnect"),
        });
        if let Some(link) = shared.sessions.get_mut(&session_id) {
            link.thread_id = None;
            link.thread_starting = false;
            link.queued_prompts.clear();
        }
    }
    shared.threads.clear();
    shared.pending.clear();
}

fn handle_message(shared: &mut Shared, msg: &serde_json::Value) {
    // Response (id + result/error) — match against pending requests.
    if let Some(id) = msg.get("id").and_then(|id| id.as_u64()) {
        if msg.get("result").is_some() || msg.get("error").is_some() {
            let Some(kind) = shared.pending.remove(&id) else {
                return;
            };
            if let Some(error) = msg.get("error") {
                let message = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown RPC error")
                    .to_string();
                handle_rpc_error(shared, kind, message);
                return;
            }
            let result = msg.get("result").cloned().unwrap_or_default();
            handle_response(shared, kind, &result);
            return;
        }
        // Server→client request: refuse politely so turns never hang on an
        // approval dialog this surface does not render.
        let refusal = serde_json::json!({
            "id": msg.get("id"),
            "error": { "code": -32601, "message": "script-kit flow sessions do not support server-side requests" },
        });
        write_line(shared, &refusal, "server-request-refusal");
        return;
    }
    // Notification.
    let Some(method) = msg.get("method").and_then(|m| m.as_str()) else {
        return;
    };
    let params = msg.get("params").cloned().unwrap_or_default();
    let session_for_thread = |shared: &Shared, params: &serde_json::Value| {
        params
            .get("threadId")
            .and_then(|t| t.as_str())
            .and_then(|thread_id| shared.threads.get(thread_id).copied())
    };
    match method {
        "turn/started" => {
            if let Some(session_id) = session_for_thread(shared, &params) {
                shared
                    .events
                    .push(FlowThreadEvent::TurnStarted { session_id });
            }
        }
        "item/agentMessage/delta" => {
            if let Some(session_id) = session_for_thread(shared, &params) {
                if let Some(delta) = params.get("delta").and_then(|d| d.as_str()) {
                    let item_id = params
                        .get("itemId")
                        .and_then(|id| id.as_str())
                        .unwrap_or_default()
                        .to_string();
                    shared.events.push(FlowThreadEvent::AgentDelta {
                        session_id,
                        item_id,
                        delta: delta.to_string(),
                    });
                }
            }
        }
        "item/completed" => {
            let is_agent_message = params
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(|t| t.as_str())
                == Some("agentMessage");
            if is_agent_message {
                if let Some(session_id) = session_for_thread(shared, &params) {
                    let item = params.get("item");
                    let text = item
                        .and_then(|item| item.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let item_id = item
                        .and_then(|item| item.get("id"))
                        .and_then(|id| id.as_str())
                        .unwrap_or_default()
                        .to_string();
                    shared.events.push(FlowThreadEvent::AgentMessageFinal {
                        session_id,
                        item_id,
                        text,
                    });
                }
            }
        }
        "turn/completed" => {
            if let Some(session_id) = session_for_thread(shared, &params) {
                let turn = params.get("turn").cloned().unwrap_or_default();
                let status = turn
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("completed")
                    .to_string();
                let error = turn
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .map(str::to_string);
                match status.as_str() {
                    "completed" => shared.events.push(FlowThreadEvent::TurnCompleted {
                        session_id,
                        outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Completed {
                            stop_reason: Some(status),
                        },
                    }),
                    "interrupted" => shared.events.push(FlowThreadEvent::TurnCompleted {
                        session_id,
                        outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Cancelled {
                            kind: sk_protocol::ai_reliability::CancellationKind::UserStopped,
                            partial: sk_protocol::ai_reliability::PartialOutputState::None,
                        },
                    }),
                    _ => shared.events.push(FlowThreadEvent::TurnFailed {
                        session_id,
                        failure: flow_failure(error.unwrap_or_else(|| "Turn failed".to_string())),
                    }),
                }
            }
        }
        "error" => {
            // Transient errors (willRetry) do not end the turn — surface
            // only fatal ones.
            let will_retry = params
                .get("willRetry")
                .and_then(|w| w.as_bool())
                .unwrap_or(false);
            if !will_retry {
                if let Some(session_id) = session_for_thread(shared, &params) {
                    let message = params
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("codex reported an error")
                        .to_string();
                    shared.events.push(FlowThreadEvent::TurnFailed {
                        session_id,
                        failure: flow_failure(message),
                    });
                }
            }
        }
        // hook/*, skills/changed, thread/status/changed, tokenUsage,
        // rateLimits, item/started, … — deliberately ignored.
        _ => {}
    }
}

fn handle_rpc_error(shared: &mut Shared, kind: PendingKind, message: String) {
    match kind {
        PendingKind::Initialize => {
            let failure = flow_failure(format!("codex initialize failed: {message}"));
            let fingerprint = failure
                .failure
                .diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.fingerprint.0.as_str())
                .unwrap_or("unavailable");
            tracing::warn!(
                target: "script_kit::flows",
                event = "codex_initialize_failed",
                failure_code = ?failure.failure.code,
                diagnostic_fingerprint = fingerprint,
                "codex app-server initialize failed"
            );
            // Every session that queued work against this child fails.
            let session_ids: Vec<u64> = shared.sessions.keys().copied().collect();
            for session_id in session_ids {
                shared.events.push(FlowThreadEvent::SessionFailed {
                    session_id,
                    failure: failure.clone(),
                });
            }
        }
        PendingKind::ThreadStart { session_id } => {
            if let Some(link) = shared.sessions.get_mut(&session_id) {
                link.thread_starting = false;
                link.queued_prompts.clear();
            }
            shared.events.push(FlowThreadEvent::SessionFailed {
                session_id,
                failure: flow_failure(format!("thread/start failed: {message}")),
            });
        }
        PendingKind::TurnStart { session_id } => {
            shared.events.push(FlowThreadEvent::TurnFailed {
                session_id,
                failure: flow_failure(format!("turn/start failed: {message}")),
            });
        }
        PendingKind::TurnInterrupt => {}
    }
}

/// `thread/start` params for a session: cwd + never-ask approvals, plus the
/// flow's declared contract when it pins one. A flow's `model:`/`sandbox:`
/// frontmatter must reach the server — otherwise the session silently runs
/// on the user's global codex defaults wearing the flow's name.
fn thread_start_params(link: &SessionLink) -> serde_json::Value {
    let mut params = serde_json::json!({ "cwd": link.cwd, "approvalPolicy": "never" });
    if let Some(model) = &link.profile.model {
        params["model"] = serde_json::Value::String(model.clone());
    }
    if let Some(sandbox) = &link.profile.sandbox {
        params["sandbox"] = serde_json::Value::String(sandbox.clone());
    }
    if let Some(instructions) = &link.profile.developer_instructions {
        params["developerInstructions"] = serde_json::Value::String(instructions.clone());
    }
    params
}

fn handle_response(shared: &mut Shared, kind: PendingKind, result: &serde_json::Value) {
    match kind {
        PendingKind::Initialize | PendingKind::TurnInterrupt => {}
        PendingKind::ThreadStart { session_id } => {
            let thread_id = result
                .get("thread")
                .and_then(|t| t.get("id"))
                .and_then(|id| id.as_str())
                .map(str::to_string);
            let model = result
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
            let Some(thread_id) = thread_id else {
                shared.events.push(FlowThreadEvent::SessionFailed {
                    session_id,
                    failure: flow_failure("thread/start response missing thread.id"),
                });
                return;
            };
            shared.threads.insert(thread_id.clone(), session_id);
            let queued = if let Some(link) = shared.sessions.get_mut(&session_id) {
                link.thread_id = Some(thread_id.clone());
                link.thread_starting = false;
                std::mem::take(&mut link.queued_prompts)
            } else {
                Vec::new()
            };
            shared
                .events
                .push(FlowThreadEvent::ThreadStarted { session_id, model });
            for prompt in queued {
                send_turn_start(shared, session_id, &thread_id, &prompt);
            }
        }
        PendingKind::TurnStart { session_id } => {
            // turn/started also arrives as a notification; the response is
            // the authoritative ack that the turn was accepted.
            shared
                .events
                .push(FlowThreadEvent::TurnStarted { session_id });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Notification routing: agent deltas and completion map thread ids back
    /// to app session ids, and unknown notifications are ignored.
    #[test]
    fn notifications_route_by_thread_id_and_unknowns_are_ignored() {
        let mut shared = Shared::default();
        shared.threads.insert("t-1".to_string(), 7);

        handle_message(
            &mut shared,
            &serde_json::json!({
                "method": "item/agentMessage/delta",
                "params": { "threadId": "t-1", "turnId": "x", "itemId": "i", "delta": "Hello" }
            }),
        );
        handle_message(
            &mut shared,
            &serde_json::json!({ "method": "hook/started", "params": { "threadId": "t-1" } }),
        );
        handle_message(
            &mut shared,
            &serde_json::json!({
                "method": "turn/completed",
                "params": { "threadId": "t-1", "turn": { "id": "x", "status": "completed" } }
            }),
        );

        assert_eq!(shared.events.len(), 2);
        assert!(matches!(
            &shared.events[0],
            FlowThreadEvent::AgentDelta { session_id: 7, item_id, delta }
                if delta == "Hello" && item_id == "i"
        ));
        assert!(matches!(
            &shared.events[1],
            FlowThreadEvent::TurnCompleted {
                session_id: 7,
                outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Completed {
                    stop_reason: Some(status)
                }
            } if status == "completed"
        ));
    }

    /// thread/start response links the thread, flushes queued prompts as
    /// turn/start requests, and emits ThreadStarted.
    #[test]
    fn thread_start_response_flushes_queued_prompts() {
        let mut shared = Shared::default();
        shared.sessions.insert(
            3,
            SessionLink {
                cwd: "/tmp".into(),
                profile: Default::default(),
                thread_id: None,
                thread_starting: true,
                queued_prompts: vec!["first task".into()],
            },
        );

        handle_response(
            &mut shared,
            PendingKind::ThreadStart { session_id: 3 },
            &serde_json::json!({ "thread": { "id": "t-9" }, "model": "gpt-5.3" }),
        );

        assert_eq!(shared.threads.get("t-9"), Some(&3));
        assert!(matches!(
            &shared.events[0],
            FlowThreadEvent::ThreadStarted { session_id: 3, model } if model == "gpt-5.3"
        ));
        // The queued prompt became a pending turn/start (stdin is None in
        // tests, so only the pending map records it).
        assert!(shared
            .pending
            .values()
            .any(|kind| matches!(kind, PendingKind::TurnStart { session_id: 3 })));
        assert!(shared.sessions.get(&3).unwrap().queued_prompts.is_empty());
    }

    /// Transient errors with willRetry never settle a turn; fatal ones do.
    #[test]
    fn transient_errors_do_not_end_the_turn() {
        let mut shared = Shared::default();
        shared.threads.insert("t-2".to_string(), 5);
        handle_message(
            &mut shared,
            &serde_json::json!({
                "method": "error",
                "params": { "threadId": "t-2", "turnId": "x", "willRetry": true,
                             "error": { "message": "rate limited" } }
            }),
        );
        assert!(shared.events.is_empty());
        handle_message(
            &mut shared,
            &serde_json::json!({
                "method": "error",
                "params": { "threadId": "t-2", "turnId": "x", "willRetry": false,
                             "error": { "message": "boom" } }
            }),
        );
        assert!(matches!(
            &shared.events[0],
            FlowThreadEvent::TurnFailed { session_id: 5, failure }
                if failure.failure.code == sk_protocol::ai_reliability::AiFailureCode::Unknown
        ));
    }

    /// drain_events empties the queue exactly once.
    #[test]
    fn drain_events_takes_the_queue() {
        let client = CodexAppServer {
            shared: Mutex::new(Shared::default()),
        };
        client.push_event_for_test(FlowThreadEvent::TurnStarted { session_id: 1 });
        assert_eq!(client.drain_events().len(), 1);
        assert!(client.drain_events().is_empty());
    }
}
