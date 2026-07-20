//! Cold, one-shot Codex exec adapter used only by the hidden Quick AI profile.

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context as _, Result};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::ai::agent_chat::content::ContentBlock;
use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx, AgentChatModelEntry};
use crate::ai::agent_chat::runtime::{AgentChatConnection, AgentChatTurnRequest};

pub(crate) const QUICK_AI_SELECTED_MODEL_ID: &str = "openai-codex/gpt-5.3-codex-spark";
const EVENT_CHANNEL_CAPACITY: usize = 64;
const CANCEL_POLL: Duration = Duration::from_millis(20);
const CANCEL_GRACE: Duration = Duration::from_secs(2);
const QUICK_AI_OUTPUT_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"properties":{"answer":{"type":"string"},"sources":{"type":"array","minItems":1,"items":{"type":"string"}}},"required":["answer","sources"]}"#;

#[derive(Debug, Clone)]
pub(crate) struct CodexQuickAiExecSpec {
    pub(crate) binary: PathBuf,
    pub(crate) model: String,
    pub(crate) selected_model_id: String,
    pub(crate) developer_instructions: String,
    pub(crate) scratch_root: PathBuf,
    pub(crate) trace_path: Option<PathBuf>,
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
    fn start_turn(&self, request: AgentChatTurnRequest) -> Result<AgentChatEventRx> {
        let query = extract_zero_context_query(&request, &self.spec.selected_model_id)?;
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        {
            let turns = self
                .active_turns
                .lock()
                .map_err(|_| anyhow!("quick_ai_active_turn_lock_poisoned"))?;
            if turns.contains_key(&request.ui_thread_id) {
                bail!("quick_ai_turn_already_active")
            }
        }

        std::fs::create_dir_all(&self.spec.scratch_root).with_context(|| {
            format!(
                "quick_ai_scratch_root_create_failed:{}",
                self.spec.scratch_root.display()
            )
        })?;
        let run_id = format!(
            "quick-ai-{}-{generation}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let turn_cwd = self.spec.scratch_root.join(&run_id);
        std::fs::create_dir(&turn_cwd)
            .with_context(|| format!("quick_ai_turn_cwd_create_failed:{}", turn_cwd.display()))?;

        let mut command = build_codex_exec_command(&self.spec, &turn_cwd, &query)?;
        let mut child = command.spawn().with_context(|| {
            format!("quick_ai_codex_spawn_failed:{}", self.spec.binary.display())
        })?;
        let pid = child.id();
        let pgid = pid as i32;
        let registration = crate::process_manager::ChildRegistration::register(
            pid,
            &self.spec.binary.to_string_lossy(),
        );
        let stdout = child
            .stdout
            .take()
            .context("quick_ai_codex_stdout_unavailable")?;
        let stderr = child
            .stderr
            .take()
            .context("quick_ai_codex_stderr_unavailable")?;
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let trace = TraceSink::new(self.spec.trace_path.clone(), run_id.clone());
        trace.write(
            "spawned",
            json!({
                "backend": "codex-exec",
                "profileId": "quick-ai",
                "model": self.spec.model,
                "selectedModelId": self.spec.selected_model_id,
                "promptSha256": sha256_hex(&self.spec.developer_instructions),
                "allowedTools": ["web_search"],
                "nativeSearchEnabled": true,
                "sandbox": "read-only",
                "ephemeral": true,
                "ignoreUserConfig": true,
                "ignoreRules": true,
                "stdinNull": true,
                "inputBlockCount": 1,
                "textBlockCount": 1,
                "imageBlockCount": 0,
                "querySha256": sha256_hex(&query),
                "queryChars": query.chars().count(),
                "pid": pid,
                "pgid": pgid,
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
        let scratch = turn_cwd;
        self.active_turns
            .lock()
            .map_err(|_| anyhow!("quick_ai_active_turn_lock_poisoned"))?
            .insert(
                ui_thread_id.clone(),
                ActiveExecTurn {
                    generation,
                    pid,
                    pgid,
                    cancel_requested: cancel_requested.clone(),
                },
            );
        std::thread::Builder::new()
            .name(format!("quick-ai-turn-{generation}"))
            .spawn(move || {
                let _registration = registration;
                let mut accumulator = CodexExecTurnAccumulator::new(run_id.clone());
                let mut failure: Option<CodexTurnFailure> = None;
                let mut cancelled = false;
                loop {
                    if cancel_requested.load(Ordering::Acquire) || event_tx.is_closed() {
                        cancelled = true;
                        break;
                    }
                    match line_rx.recv_timeout(CANCEL_POLL) {
                        Ok(Ok(line)) => {
                            if line.trim().is_empty() {
                                failure = Some(CodexTurnFailure::protocol(
                                    "quick_ai_codex_empty_jsonl_line",
                                ));
                                break;
                            }
                            let event = match parse_codex_exec_line(&line) {
                                Ok(event) => event,
                                Err(error) => {
                                    failure = Some(CodexTurnFailure::protocol(error.to_string()));
                                    break;
                                }
                            };
                            trace_event_for_protocol(&trace, &event);
                            match apply_codex_exec_event(&mut accumulator, event) {
                                Ok(events) => {
                                    for event in events {
                                        match event_tx.try_send(event) {
                                            Ok(()) => {}
                                            Err(async_channel::TrySendError::Closed(_)) => {
                                                cancelled = true;
                                                break;
                                            }
                                            Err(async_channel::TrySendError::Full(_)) => {
                                                failure = Some(CodexTurnFailure::protocol(
                                                    "quick_ai_event_channel_backpressure",
                                                ));
                                                break;
                                            }
                                        }
                                    }
                                }
                                Err(error) => {
                                    failure = Some(error);
                                    break;
                                }
                            }
                            if cancelled || failure.is_some() {
                                break;
                            }
                        }
                        Ok(Err(error)) => {
                            failure = Some(CodexTurnFailure::protocol(format!(
                                "quick_ai_codex_stdout_read_failed:{error}"
                            )));
                            break;
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if let Ok(Some(status)) = child.try_wait() {
                                if !accumulator.terminal_seen {
                                    failure = Some(CodexTurnFailure::protocol(format!(
                                        "quick_ai_codex_eof_without_terminal:{}",
                                        status.code().unwrap_or(-1)
                                    )));
                                }
                                break;
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            if !accumulator.terminal_seen {
                                failure = Some(CodexTurnFailure::protocol(
                                    "quick_ai_codex_eof_without_terminal",
                                ));
                            }
                            break;
                        }
                    }
                }

                let teardown = terminate_and_reap_process_group(&mut child, pgid, CANCEL_GRACE)
                    .unwrap_or_else(|error| ProcessTeardownReport::failed(error.to_string()));
                let stderr_text = stderr_thread.join().unwrap_or_default();
                trace.write(
                    "teardown",
                    serde_json::to_value(&teardown).unwrap_or(Value::Null),
                );
                let _ = std::fs::remove_dir_all(&scratch);
                if let Ok(mut turns) = active_turns.lock() {
                    if turns
                        .get(&ui_thread_id)
                        .is_some_and(|turn| turn.generation == generation)
                    {
                        turns.remove(&ui_thread_id);
                    }
                }

                if cancelled {
                    emit_running_search_failures(&event_tx, &accumulator, "Cancelled", false);
                    trace.write("terminal", json!({"kind": "cancelled"}));
                    let _ = event_tx.send_blocking(AgentChatEvent::TurnFinished {
                        stop_reason: "cancelled".to_string(),
                    });
                    return;
                }

                if failure.is_none() {
                    if !teardown.child_reaped || teardown.process_group_alive {
                        failure = Some(CodexTurnFailure::protocol(
                            "quick_ai_codex_process_teardown_incomplete",
                        ));
                    } else if teardown.exit_code != Some(0) || teardown.exit_signal.is_some() {
                        failure = Some(CodexTurnFailure::protocol(format!(
                            "quick_ai_codex_nonzero_exit:code={:?}:signal={:?}",
                            teardown.exit_code, teardown.exit_signal
                        )));
                    } else if !accumulator.turn_completed_seen {
                        failure = Some(CodexTurnFailure::protocol(
                            "quick_ai_codex_eof_without_terminal",
                        ));
                    }
                }
                let final_events = failure
                    .map(Err)
                    .unwrap_or_else(|| finalize_successful_turn(&accumulator));
                match final_events {
                    Ok(events) => {
                        if let Some(AgentChatEvent::AgentMessageDelta(answer)) = events.first() {
                            trace.write(
                                "final_answer_selected",
                                json!({
                                    "answerSha256": sha256_hex(answer),
                                    "answerChars": answer.chars().count(),
                                    "answerUrls": http_urls_in_text(answer),
                                    "sourceProvenance": if accumulator.structured_urls.is_empty() {
                                        "answer_url_after_native_search"
                                    } else {
                                        "codex_web_search_action"
                                    },
                                }),
                            );
                        }
                        trace.write("terminal", json!({"kind": "completed"}));
                        for event in events {
                            let _ = event_tx.send_blocking(event);
                        }
                    }
                    Err(error) => {
                        let mut message = error.message;
                        if !stderr_text.trim().is_empty() {
                            tracing::warn!(
                                target: "script_kit::quick_ai",
                                event = "codex_quick_ai_stderr",
                                stderr = %stderr_text.trim(),
                            );
                        }
                        if message.is_empty() {
                            message = "quick_ai_codex_turn_failed".to_string();
                        }
                        trace.write("protocol_failure", json!({"error": message}));
                        emit_running_search_failures(&event_tx, &accumulator, &message, true);
                        trace.write("terminal", json!({"kind": "failed", "error": message}));
                        let _ = event_tx.send_blocking(AgentChatEvent::Failed { error: message });
                    }
                }
            })
            .context("quick_ai_worker_spawn_failed")?;
        Ok(event_rx)
    }

    fn cancel_turn(&self, ui_thread_id: String) -> Result<()> {
        if let Some(turn) = self
            .active_turns
            .lock()
            .map_err(|_| anyhow!("quick_ai_active_turn_lock_poisoned"))?
            .get(&ui_thread_id)
        {
            turn.cancel_requested.store(true, Ordering::Release);
        }
        Ok(())
    }

    fn prepare_session(&self, _ui_thread_id: String, _cwd: PathBuf) -> Result<AgentChatEventRx> {
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

#[derive(Debug, Clone, PartialEq)]
enum CodexExecEvent {
    ThreadStarted { thread_id: String },
    TurnStarted,
    Item { phase: ItemPhase, item: CodexItem },
    TurnCompleted,
    TurnFailed { message: String },
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPhase {
    Started,
    Updated,
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
enum CodexItem {
    AgentMessage { id: String, text: String },
    WebSearch(WebSearchItem),
    Diagnostic { id: String, message: String },
    Safe { id: String, item_type: String },
    Forbidden { id: String, item_type: String },
}

#[derive(Debug, Clone, PartialEq)]
struct WebSearchItem {
    id: String,
    query: String,
    action: WebSearchAction,
    raw_action: Value,
}

#[derive(Debug, Clone, PartialEq)]
enum WebSearchAction {
    Search { queries: Vec<String> },
    OpenPage { url: Option<String> },
    FindInPage { url: Option<String> },
    Other,
}

#[derive(Debug)]
pub(crate) struct CodexProtocolError(String);

impl std::fmt::Display for CodexProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn required_string(value: &Value, key: &str, context: &str) -> Result<String, CodexProtocolError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| CodexProtocolError(format!("{context}:missing_{key}")))
}

fn parse_codex_exec_line(line: &str) -> Result<CodexExecEvent, CodexProtocolError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| CodexProtocolError(format!("quick_ai_codex_malformed_json:{error}")))?;
    let event_type = required_string(&value, "type", "quick_ai_codex_event")?;
    match event_type.as_str() {
        "thread.started" => Ok(CodexExecEvent::ThreadStarted {
            thread_id: required_string(&value, "thread_id", "thread.started")?,
        }),
        "turn.started" => Ok(CodexExecEvent::TurnStarted),
        "item.started" | "item.updated" | "item.completed" => {
            let item = value
                .get("item")
                .filter(|item| item.is_object())
                .ok_or_else(|| CodexProtocolError(format!("{event_type}:missing_item")))?;
            let phase = match event_type.as_str() {
                "item.started" => ItemPhase::Started,
                "item.updated" => ItemPhase::Updated,
                _ => ItemPhase::Completed,
            };
            Ok(CodexExecEvent::Item {
                phase,
                item: parse_item(item)?,
            })
        }
        "turn.completed" => {
            if value.get("usage").is_some_and(|usage| !usage.is_object()) {
                return Err(CodexProtocolError(
                    "turn.completed:invalid_usage".to_string(),
                ));
            }
            Ok(CodexExecEvent::TurnCompleted)
        }
        "turn.failed" => {
            let error = value
                .get("error")
                .filter(|error| error.is_object())
                .ok_or_else(|| CodexProtocolError("turn.failed:missing_error".to_string()))?;
            Ok(CodexExecEvent::TurnFailed {
                message: required_string(error, "message", "turn.failed")?,
            })
        }
        "error" => Ok(CodexExecEvent::Error {
            message: required_string(&value, "message", "error")?,
        }),
        other => Err(CodexProtocolError(format!(
            "quick_ai_codex_unsupported_event:{other}"
        ))),
    }
}

fn parse_item(item: &Value) -> Result<CodexItem, CodexProtocolError> {
    let id = required_string(item, "id", "codex_item")?;
    let item_type = required_string(item, "type", "codex_item")?;
    match item_type.as_str() {
        "agent_message" => Ok(CodexItem::AgentMessage {
            id,
            text: item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        "web_search" => {
            let query = item
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let action = item
                .get("action")
                .cloned()
                .unwrap_or_else(|| json!({"type": "other"}));
            let action_type = required_string(&action, "type", "web_search.action")?;
            let parsed = match action_type.as_str() {
                "search" => {
                    let mut queries = action
                        .get("queries")
                        .and_then(Value::as_array)
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    if queries.is_empty() {
                        let fallback = action
                            .get("query")
                            .and_then(Value::as_str)
                            .unwrap_or(&query)
                            .trim();
                        if !fallback.is_empty() {
                            queries.push(fallback.to_string());
                        }
                    }
                    WebSearchAction::Search { queries }
                }
                "open_page" => WebSearchAction::OpenPage {
                    url: action
                        .get("url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                "find_in_page" => WebSearchAction::FindInPage {
                    url: action
                        .get("url")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
                "other" => WebSearchAction::Other,
                other => {
                    return Err(CodexProtocolError(format!(
                        "quick_ai_codex_unsupported_web_action:{other}"
                    )))
                }
            };
            Ok(CodexItem::WebSearch(WebSearchItem {
                id,
                query,
                action: parsed,
                raw_action: action,
            }))
        }
        "error" => Ok(CodexItem::Diagnostic {
            id,
            message: required_string(item, "message", "error_item")?,
        }),
        "reasoning" | "todo_list" => Ok(CodexItem::Safe { id, item_type }),
        "command_execution" | "file_change" | "mcp_tool_call" | "collab_tool_call"
        | "image_view" | "dynamic_tool_call" => Ok(CodexItem::Forbidden { id, item_type }),
        other => Err(CodexProtocolError(format!(
            "quick_ai_codex_unsupported_item:{other}"
        ))),
    }
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
    completed_agent_messages: Vec<CompletedAgentMessage>,
    completed_agent_ids: HashSet<String>,
    focused_search_count: usize,
    non_search_tool_count: usize,
}

#[derive(Debug, Clone)]
struct WebSearchState {
    item_id: String,
    query: String,
    action: Value,
    started: bool,
    completed: bool,
    urls: Vec<String>,
    tool_started_emitted: bool,
    tool_completed_emitted: bool,
    search_counted: bool,
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
            completed_agent_messages: Vec::new(),
            completed_agent_ids: HashSet::new(),
            focused_search_count: 0,
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

fn apply_codex_exec_event(
    accumulator: &mut CodexExecTurnAccumulator,
    event: CodexExecEvent,
) -> Result<Vec<AgentChatEvent>, CodexTurnFailure> {
    if accumulator.terminal_seen {
        return Ok(Vec::new());
    }
    match event {
        CodexExecEvent::ThreadStarted { .. } | CodexExecEvent::TurnStarted => Ok(Vec::new()),
        CodexExecEvent::TurnCompleted => {
            accumulator.terminal_seen = true;
            accumulator.turn_completed_seen = true;
            Ok(Vec::new())
        }
        CodexExecEvent::TurnFailed { message } | CodexExecEvent::Error { message } => {
            accumulator.terminal_seen = true;
            Err(CodexTurnFailure::protocol(message))
        }
        CodexExecEvent::Item { phase, item } => match item {
            CodexItem::Safe { .. } => Ok(Vec::new()),
            CodexItem::Diagnostic { id, message }
                if message.starts_with("Skill descriptions were shortened to fit the ") =>
            {
                tracing::debug!(
                    target: "script_kit::quick_ai",
                    event = "codex_quick_ai_ignored_skill_budget_diagnostic",
                    item_id = %id,
                );
                Ok(Vec::new())
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
                        .push(CompletedAgentMessage { item_id: id, text });
                }
                Ok(Vec::new())
            }
            CodexItem::WebSearch(item) => apply_web_search(accumulator, phase, item),
        },
    }
}

fn apply_web_search(
    accumulator: &mut CodexExecTurnAccumulator,
    phase: ItemPhase,
    item: WebSearchItem,
) -> Result<Vec<AgentChatEvent>, CodexTurnFailure> {
    if !accumulator.search_items.contains_key(&item.id) {
        accumulator.search_order.push(item.id.clone());
        accumulator.search_items.insert(
            item.id.clone(),
            WebSearchState {
                item_id: item.id.clone(),
                query: item.query.clone(),
                action: item.raw_action.clone(),
                started: false,
                completed: false,
                urls: Vec::new(),
                tool_started_emitted: false,
                tool_completed_emitted: false,
                search_counted: false,
            },
        );
    }
    if let WebSearchAction::Search { queries } = &item.action {
        let should_count = accumulator
            .search_items
            .get(&item.id)
            .is_some_and(|state| !state.search_counted);
        if should_count {
            let is_focused_search = queries
                .iter()
                .any(|query| canonical_http_url(query).is_none());
            accumulator.focused_search_count += usize::from(is_focused_search);
            if accumulator.focused_search_count > 2 {
                return Err(CodexTurnFailure::protocol(
                    "quick_ai_more_than_two_search_queries",
                ));
            }
        }
    }
    // Codex 0.144.x currently reports visited pages as exact `web_search`
    // items whose `query` is the URL and whose action is `other`. Prefer the
    // explicit open/find action URL when present, then accept that exact
    // item-level query URL. When Codex emits only a search action, finalization
    // may accept an answer URL only after that native search completed.
    let action_url = match &item.action {
        WebSearchAction::OpenPage { url } | WebSearchAction::FindInPage { url } => url.as_deref(),
        _ => None,
    };
    let observed_url = action_url
        .or_else(|| canonical_http_url(&item.query).map(|_| item.query.as_str()))
        .map(str::to_string);
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
    if matches!(item.action, WebSearchAction::Search { .. }) {
        state.search_counted = true;
    }
    state.query = item.query;
    state.action = item.raw_action;
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
        let action_type = state
            .action
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("other");
        events.push(AgentChatEvent::ToolCallStarted {
            tool_call_id: state.item_id.clone(),
            title: if matches!(action_type, "open_page" | "find_in_page") {
                "Web source".to_string()
            } else {
                "Web search".to_string()
            },
            status: "running".to_string(),
            tool_name: Some("web_search".to_string()),
            raw_input: Some(json!({"query": state.query, "action": state.action})),
        });
    }
    if phase == ItemPhase::Updated
        || (phase == ItemPhase::Completed && !state.tool_completed_emitted)
    {
        if phase == ItemPhase::Completed {
            state.tool_completed_emitted = true;
        }
        events.push(AgentChatEvent::ToolCallUpdated {
            tool_call_id: state.item_id.clone(),
            title: None,
            status: Some(
                if state.completed {
                    "complete"
                } else {
                    "running"
                }
                .to_string(),
            ),
            body: (!state.urls.is_empty()).then(|| state.urls.join("\n")),
            raw_input: Some(json!({
                "query": state.query,
                "action": state.action,
                "source_urls": state.urls,
            })),
            diff: None,
            is_error: false,
        });
    }
    Ok(events)
}

fn render_final_answer(raw: &str) -> Result<String, CodexTurnFailure> {
    let trimmed = raw.trim();
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(trimmed.to_string());
    };
    let answer = object
        .get("answer")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|answer| !answer.is_empty())
        .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_output_schema_answer_missing"))?;
    let sources = object
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_output_schema_sources_missing"))?;
    let mut rendered = answer.to_string();
    let mut seen = HashSet::new();
    let valid_sources: Vec<&str> = sources
        .iter()
        .filter_map(Value::as_str)
        .filter(|source| canonical_http_url(source).is_some())
        .filter(|source| seen.insert(canonical_http_url(source).unwrap_or_default()))
        .collect();
    if valid_sources.is_empty() {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_output_schema_sources_invalid",
        ));
    }
    let answer_urls = http_urls_in_text(&rendered);
    for source in valid_sources {
        let key = canonical_http_url(source).unwrap_or_default();
        if !answer_urls
            .iter()
            .any(|url| canonical_http_url(url).is_some_and(|url| url == key))
        {
            rendered.push_str("\n\nSource: ");
            rendered.push_str(source);
        }
    }
    Ok(rendered)
}

fn finalize_successful_turn(
    accumulator: &CodexExecTurnAccumulator,
) -> Result<Vec<AgentChatEvent>, CodexTurnFailure> {
    if accumulator.non_search_tool_count != 0 {
        return Err(CodexTurnFailure::protocol(
            "quick_ai_codex_non_search_tool_observed",
        ));
    }
    let answer = accumulator
        .completed_agent_messages
        .iter()
        .rev()
        .find(|message| !message.text.trim().is_empty())
        .map(|message| message.text.as_str())
        .ok_or_else(|| CodexTurnFailure::protocol("quick_ai_codex_final_answer_missing"))?;
    let mut answer = render_final_answer(answer)?;
    let answer_urls = http_urls_in_text(&answer);
    if accumulator.structured_urls.is_empty() {
        if accumulator.focused_search_count == 0 || answer_urls.is_empty() {
            return Err(CodexTurnFailure::protocol(
                "quick_ai_structured_sources_unavailable",
            ));
        }
    } else {
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
    }
    Ok(vec![
        AgentChatEvent::AgentMessageDelta(answer),
        AgentChatEvent::TurnFinished {
            stop_reason: "stop".to_string(),
        },
    ])
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
                tool_call_id: item_id.clone(),
                title: None,
                status: Some("failed".to_string()),
                body: Some(message.to_string()),
                raw_input: Some(json!({
                    "query": state.query,
                    "action": state.action,
                    "source_urls": state.urls,
                })),
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

pub(crate) fn terminate_and_reap_process_group(
    child: &mut Child,
    pgid: i32,
    grace: Duration,
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
        } else if std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(anyhow!(
                "quick_ai_sigterm_failed:{}",
                std::io::Error::last_os_error()
            ));
        }
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if status.is_none() {
            status = child.try_wait()?;
        }
        if !process_group_alive(pgid) {
            break;
        }
        std::thread::sleep(CANCEL_POLL);
    }
    if process_group_alive(pgid) {
        let result = unsafe { libc::killpg(pgid, libc::SIGKILL) };
        if result == 0 {
            report.kill_sent = true;
        } else if std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(anyhow!(
                "quick_ai_sigkill_failed:{}",
                std::io::Error::last_os_error()
            ));
        }
    }
    if status.is_none() {
        status = Some(child.wait()?);
    }
    let Some(status) = status else {
        bail!("quick_ai_codex_wait_status_missing")
    };
    report.child_reaped = true;
    report.exit_code = status.code();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        report.exit_signal = status.signal();
    }
    for _ in 0..100 {
        if !process_group_alive(pgid) {
            break;
        }
        std::thread::sleep(CANCEL_POLL);
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
}

impl TraceSink {
    fn new(path: Option<PathBuf>, run_id: String) -> Self {
        Self {
            path,
            run_id,
            seq: Arc::new(AtomicU64::new(1)),
            started: Instant::now(),
        }
    }

    fn write(&self, event: &str, details: Value) {
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
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{record}");
        }
    }
}

fn trace_event_for_protocol(trace: &TraceSink, event: &CodexExecEvent) {
    match event {
        CodexExecEvent::ThreadStarted { .. } => trace.write("thread_started", json!({})),
        CodexExecEvent::TurnStarted => trace.write("turn_started", json!({})),
        CodexExecEvent::Item {
            phase,
            item: CodexItem::WebSearch(item),
        } => {
            let event_name = match phase {
                ItemPhase::Started => "web_search_started",
                ItemPhase::Updated => "web_search_updated",
                ItemPhase::Completed => "web_search_completed",
            };
            trace.write(
                event_name,
                json!({"itemId": item.id, "action": item.raw_action}),
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
                    "structured_source_observed",
                    json!({"itemId": item.id, "action": item.raw_action, "url": url}),
                );
            }
        }
        CodexExecEvent::Item {
            phase: ItemPhase::Completed,
            item: CodexItem::AgentMessage { id, text },
        } => trace.write(
            "agent_message_buffered",
            json!({"itemId": id, "textSha256": sha256_hex(text)}),
        ),
        CodexExecEvent::Item {
            item: CodexItem::Forbidden { id, item_type },
            ..
        } => trace.write(
            "forbidden_item",
            json!({"itemId": id, "itemType": item_type}),
        ),
        CodexExecEvent::Item {
            item: CodexItem::Diagnostic { id, message },
            ..
        } => trace.write(
            "diagnostic",
            json!({"itemId": id, "messageSha256": sha256_hex(message)}),
        ),
        _ => {}
    }
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::content::{ImageContent, TextContent};

    fn accumulator() -> CodexExecTurnAccumulator {
        CodexExecTurnAccumulator::new("test".to_string())
    }

    #[cfg(unix)]
    fn fake_codex(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("fake-codex");
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn turn_request() -> AgentChatTurnRequest {
        AgentChatTurnRequest {
            ui_thread_id: "quick-ai-test-thread".into(),
            cwd: PathBuf::from("/must/not/be/used"),
            blocks: vec![ContentBlock::Text(TextContent::new("latest Rust"))],
            model_id: Some(QUICK_AI_SELECTED_MODEL_ID.into()),
        }
    }

    fn apply_line(
        acc: &mut CodexExecTurnAccumulator,
        line: &str,
    ) -> Result<Vec<AgentChatEvent>, CodexTurnFailure> {
        apply_codex_exec_event(acc, parse_codex_exec_line(line).unwrap())
    }

    #[test]
    fn codex_quick_ai_command_matches_measured_benchmark_contract() {
        let dir = tempfile::tempdir().unwrap();
        let spec = CodexQuickAiExecSpec::from_builtin_contract(dir.path().to_path_buf());
        let command = build_codex_exec_command(&spec, dir.path(), "query").unwrap();
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.first().map(String::as_str), Some("--search"));
        assert_eq!(args.last().map(String::as_str), Some("query"));
        for required in [
            "--disable",
            "plugins",
            "skills.bundled.enabled=false",
            "model_reasoning_effort=\"low\"",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
            "--output-schema",
            "--json",
        ] {
            assert!(args.contains(&required.to_string()));
        }
    }

    #[test]
    fn codex_quick_ai_requires_exact_model_and_single_text_block() {
        let request = AgentChatTurnRequest {
            ui_thread_id: "t".into(),
            cwd: PathBuf::from("/tmp"),
            blocks: vec![ContentBlock::Text(TextContent::new("query"))],
            model_id: Some(QUICK_AI_SELECTED_MODEL_ID.into()),
        };
        assert_eq!(
            extract_zero_context_query(&request, QUICK_AI_SELECTED_MODEL_ID).unwrap(),
            "query"
        );
        let mut wrong = request.clone();
        wrong.model_id = Some("other/model".into());
        assert!(extract_zero_context_query(&wrong, QUICK_AI_SELECTED_MODEL_ID).is_err());
        wrong.model_id = Some(QUICK_AI_SELECTED_MODEL_ID.into());
        wrong
            .blocks
            .push(ContentBlock::Text(TextContent::new("context")));
        assert!(extract_zero_context_query(&wrong, QUICK_AI_SELECTED_MODEL_ID).is_err());
    }

    #[test]
    fn codex_quick_ai_rejects_image_or_additional_context_blocks() {
        let request = AgentChatTurnRequest {
            ui_thread_id: "t".into(),
            cwd: PathBuf::from("/tmp"),
            blocks: vec![ContentBlock::Image(ImageContent::new("data", "image/png"))],
            model_id: Some(QUICK_AI_SELECTED_MODEL_ID.into()),
        };
        assert!(extract_zero_context_query(&request, QUICK_AI_SELECTED_MODEL_ID).is_err());
    }

    #[test]
    fn codex_quick_ai_search_start_maps_to_existing_tool_start() {
        let mut acc = accumulator();
        let events = apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}"#).unwrap();
        assert!(
            matches!(events.as_slice(), [AgentChatEvent::ToolCallStarted { tool_name: Some(name), .. }] if name == "web_search")
        );
    }

    #[test]
    fn codex_quick_ai_open_and_find_urls_are_preserved_ordered_and_deduplicated() {
        let mut acc = accumulator();
        for line in [
            r#"{"type":"item.completed","item":{"id":"a","type":"web_search","action":{"type":"open_page","url":"https://blog.rust-lang.org/a"}}}"#,
            r#"{"type":"item.completed","item":{"id":"b","type":"web_search","action":{"type":"find_in_page","url":"https://blog.rust-lang.org/b","pattern":"release"}}}"#,
            r#"{"type":"item.completed","item":{"id":"c","type":"web_search","action":{"type":"open_page","url":"https://blog.rust-lang.org/a"}}}"#,
        ] {
            apply_line(&mut acc, line).unwrap();
        }
        assert_eq!(
            acc.structured_urls,
            [
                "https://blog.rust-lang.org/a",
                "https://blog.rust-lang.org/b"
            ]
        );
    }

    #[test]
    fn codex_quick_ai_agent_message_containing_web_search_is_not_a_tool() {
        let event = parse_codex_exec_line(r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"I used web_search"}}"#).unwrap();
        assert!(matches!(
            event,
            CodexExecEvent::Item {
                item: CodexItem::AgentMessage { .. },
                ..
            }
        ));
    }

    #[test]
    fn codex_quick_ai_buffers_preamble_and_emits_only_last_agent_message() {
        let mut acc = accumulator();
        for line in [
            r#"{"type":"item.completed","item":{"id":"m1","type":"agent_message","text":"preamble"}}"#,
            r#"{"type":"item.completed","item":{"id":"s","type":"web_search","action":{"type":"open_page","url":"https://example.com/source"}}}"#,
            r#"{"type":"item.completed","item":{"id":"m2","type":"agent_message","text":"final https://example.com/source"}}"#,
            r#"{"type":"turn.completed","usage":{}}"#,
        ] {
            apply_line(&mut acc, line).unwrap();
        }
        let events = finalize_successful_turn(&acc).unwrap();
        assert!(
            matches!(&events[0], AgentChatEvent::AgentMessageDelta(text) if text.starts_with("final"))
        );
    }

    #[test]
    fn codex_quick_ai_duplicate_item_completion_and_terminal_are_idempotent() {
        let mut acc = accumulator();
        let line =
            r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"answer"}}"#;
        apply_line(&mut acc, line).unwrap();
        apply_line(&mut acc, line).unwrap();
        assert_eq!(acc.completed_agent_messages.len(), 1);
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert!(acc.terminal_seen);
    }

    #[test]
    fn codex_quick_ai_more_than_two_search_queries_fails_closed() {
        let mut acc = accumulator();
        for id in ["a", "b"] {
            apply_line(
                &mut acc,
                &format!(r#"{{"type":"item.started","item":{{"id":"{id}","type":"web_search","action":{{"type":"search","query":"{id}"}}}}}}"#),
            )
            .unwrap();
        }
        let error = apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"c","type":"web_search","action":{"type":"search","query":"c"}}}"#).unwrap_err();
        assert_eq!(error.message, "quick_ai_more_than_two_search_queries");
    }

    #[test]
    fn codex_quick_ai_forbidden_and_unknown_items_fail_closed() {
        for item_type in ["command_execution", "file_change", "mcp_tool_call"] {
            let mut acc = accumulator();
            let line =
                format!(r#"{{"type":"item.started","item":{{"id":"x","type":"{item_type}"}}}}"#);
            assert!(apply_line(&mut acc, &line).is_err());
        }
        assert!(parse_codex_exec_line(
            r#"{"type":"item.started","item":{"id":"x","type":"future_tool"}}"#
        )
        .is_err());
    }

    #[test]
    fn codex_quick_ai_current_wire_diagnostic_and_other_url_are_supported() {
        let mut acc = accumulator();
        for line in [
            r#"{"type":"item.completed","item":{"id":"warning","type":"error","message":"Skill descriptions were shortened to fit the 2% skills context budget. Codex can still see every skill, but some descriptions are shorter. Disable unused skills or plugins to leave more room for the rest."}}"#,
            r#"{"type":"item.completed","item":{"id":"source","type":"web_search","query":"https://blog.rust-lang.org/releases/latest/","action":{"type":"other"}}}"#,
            r#"{"type":"item.completed","item":{"id":"answer","type":"agent_message","text":"Rust 1.97.0: https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/"}}"#,
            r#"{"type":"turn.completed","usage":{}}"#,
        ] {
            assert!(apply_line(&mut acc, line).is_ok());
        }
        assert!(finalize_successful_turn(&acc).is_ok());
        assert_eq!(
            acc.structured_urls,
            ["https://blog.rust-lang.org/releases/latest/"]
        );
    }

    #[test]
    fn codex_quick_ai_unexpected_error_item_fails_closed() {
        let mut acc = accumulator();
        let result = apply_line(
            &mut acc,
            r#"{"type":"item.completed","item":{"id":"error","type":"error","message":"unexpected provider failure"}}"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn codex_quick_ai_final_answer_url_without_structured_source_fails() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"answer https://example.com"}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert_eq!(
            finalize_successful_turn(&acc).unwrap_err().message,
            "quick_ai_structured_sources_unavailable"
        );
    }

    #[test]
    fn codex_quick_ai_output_schema_renders_answer_and_source() {
        let rendered = render_final_answer(
            r#"{"answer":"Rust 1.97.0 was released July 9, 2026.","sources":["https://blog.rust-lang.org/releases/latest/"]}"#,
        )
        .unwrap();
        assert_eq!(
            rendered,
            "Rust 1.97.0 was released July 9, 2026.\n\nSource: https://blog.rust-lang.org/releases/latest/"
        );
    }

    #[test]
    fn codex_quick_ai_answer_url_after_native_search_is_accepted() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","action":{"type":"other"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"s","type":"web_search","action":{"type":"search","query":"Rust release"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"Rust 1.97.0: https://blog.rust-lang.org/2026/07/09/Rust-1.97.0/"}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        assert!(finalize_successful_turn(&acc).is_ok());
    }

    #[test]
    fn codex_quick_ai_missing_answer_url_appends_structured_source() {
        let mut acc = accumulator();
        apply_line(&mut acc, r#"{"type":"item.started","item":{"id":"s","type":"web_search","action":{"type":"other"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"s","type":"web_search","query":"https://blog.rust-lang.org/releases/latest/","action":{"type":"other"}}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"Rust 1.97.0 was released July 9, 2026."}}"#).unwrap();
        apply_line(&mut acc, r#"{"type":"turn.completed","usage":{}}"#).unwrap();
        let events = finalize_successful_turn(&acc).unwrap();
        assert!(matches!(
            events.first(),
            Some(AgentChatEvent::AgentMessageDelta(answer))
                if answer.ends_with("Source: https://blog.rust-lang.org/releases/latest/")
        ));
    }

    #[test]
    fn codex_quick_ai_prepare_session_does_not_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary: dir.path().join("missing-codex"),
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection
            .prepare_session("t".into(), dir.path().into())
            .unwrap();
        assert!(matches!(
            rx.recv_blocking().unwrap(),
            AgentChatEvent::ModelsAvailable { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_success_reaps_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let binary = fake_codex(
            dir.path(),
            r#"
if IFS= read -r ignored; then exit 91; fi
printf '%s\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.started","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"o","type":"web_search","action":{"type":"open_page","url":"https://blog.rust-lang.org/source"}}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"m","type":"agent_message","text":"Rust https://blog.rust-lang.org/source"}}'
printf '%s\n' '{"type":"turn.completed","usage":{}}'
sleep 1
"#,
        );
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary,
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection.start_turn(turn_request()).unwrap();
        let mut answer = false;
        let mut finished = false;
        while let Ok(event) = rx.recv_blocking() {
            answer |= matches!(event, AgentChatEvent::AgentMessageDelta(_));
            finished |= matches!(event, AgentChatEvent::TurnFinished { .. });
        }
        assert!(answer && finished);
        assert!(connection.active_turns.lock().unwrap().is_empty());
        assert_eq!(
            std::fs::read_dir(dir.path().join("scratch"))
                .unwrap()
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn codex_quick_ai_cancel_escalates_and_reaps_parent_and_grandchild() {
        let dir = tempfile::tempdir().unwrap();
        let trace_path = dir.path().join("trace.ndjson");
        let binary = fake_codex(
            dir.path(),
            r#"
trap '' TERM
(trap '' TERM; while :; do sleep 60; done) &
printf '%s\n' '{"type":"thread.started","thread_id":"t"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.started","item":{"id":"s","type":"web_search","query":"Rust","action":{"type":"search","query":"Rust"}}}'
while :; do sleep 60; done
"#,
        );
        let connection = CodexQuickAiExecConnection::new(CodexQuickAiExecSpec {
            binary,
            trace_path: Some(trace_path.clone()),
            ..CodexQuickAiExecSpec::from_builtin_contract(dir.path().join("scratch"))
        });
        let rx = connection.start_turn(turn_request()).unwrap();
        let started = rx.recv_blocking().unwrap();
        assert!(matches!(started, AgentChatEvent::ToolCallStarted { .. }));
        let pgid = connection
            .active_turns
            .lock()
            .unwrap()
            .get("quick-ai-test-thread")
            .unwrap()
            .pgid;
        connection
            .cancel_turn("quick-ai-test-thread".into())
            .unwrap();
        let mut cancelled = false;
        while let Ok(event) = rx.recv_blocking() {
            cancelled |= matches!(event, AgentChatEvent::TurnFinished { ref stop_reason } if stop_reason == "cancelled");
        }
        assert!(cancelled);
        assert!(!process_group_alive(pgid));
        let trace = std::fs::read_to_string(trace_path).unwrap();
        assert!(trace.contains("\"killSent\":true"));
        assert!(connection.active_turns.lock().unwrap().is_empty());
    }
}
