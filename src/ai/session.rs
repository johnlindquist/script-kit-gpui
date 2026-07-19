//! Persistent Claude Code CLI Session Manager
//!
//! This module manages persistent Claude CLI processes for efficient multi-turn conversations.
//! Instead of spawning a new process for each message, we keep a single process alive per chat
//! and send messages via the `--input-format stream-json` protocol.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  ClaudeSessionManager (global singleton)                        │
//! │  ├── sessions: HashMap<session_id, Arc<Mutex<ClaudeSession>>>  │
//! │  └── cleanup_interval: periodically removes stale sessions      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  ClaudeSession (per chat)                                       │
//! │  ├── child: Child process handle                                │
//! │  ├── stdin: BufWriter to send JSONL messages                    │
//! │  ├── response_rx: Channel to receive parsed responses           │
//! │  └── reader_thread: Background thread parsing stdout            │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! let manager = ClaudeSessionManager::global();
//!
//! // Send a message (creates session if needed)
//! manager.send_message(
//!     "chat-uuid",
//!     "Hello!",
//!     "sonnet",
//!     Some("Be helpful"),
//!     |chunk| println!("Chunk: {}", chunk),
//! )?;
//!
//! // Close session when done
//! manager.close_session("chat-uuid");
//! ```

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// Credential-related keys to extract from the user's `~/.claude/settings.json`.
/// These are merged into the `--settings` JSON so the CLI can authenticate
/// even when `--setting-sources ""` disables normal settings loading.
const CREDENTIAL_KEYS: &[&str] = &[
    "apiKeyHelper",
    "env", // may contain ANTHROPIC_BASE_URL, ANTHROPIC_API_KEY, etc.
    "oauthAccount",
    "primaryApiKey",
];

/// Read the user's `~/.claude/settings.json` and extract only credential/connection
/// fields.  Returns a `serde_json::Value::Object` with only the whitelisted keys
/// that were present, or an empty object if the file doesn't exist / can't be read.
pub fn read_user_credential_settings() -> serde_json::Value {
    let settings_path = dirs::home_dir()
        .map(|h| h.join(".claude").join("settings.json"))
        .unwrap_or_default();

    let content = match std::fs::read_to_string(&settings_path) {
        Ok(c) => c,
        Err(_) => return serde_json::json!({}),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return serde_json::json!({}),
    };

    let obj = match parsed.as_object() {
        Some(o) => o,
        None => return serde_json::json!({}),
    };

    let mut creds = serde_json::Map::new();
    for &key in CREDENTIAL_KEYS {
        if let Some(val) = obj.get(key) {
            creds.insert(key.to_string(), val.clone());
        }
    }

    let extracted_keys: Vec<&str> = creds.keys().map(|k| k.as_str()).collect();
    tracing::debug!(
        keys = ?extracted_keys,
        "Extracted credential settings from user config"
    );

    serde_json::Value::Object(creds)
}

/// Events from the stdout reader thread
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Streaming text chunk (partial response)
    TextChunk(String),
    /// Final result (response complete)
    Result(String),
    /// Error from CLI
    Error(String),
    /// Process exited
    Exited(i32),
}

/// A persistent Claude CLI session
pub struct ClaudeSession {
    /// Child process handle
    child: Child,
    /// Buffered writer to stdin
    stdin: BufWriter<ChildStdin>,
    /// Receiver for parsed events from stdout
    response_rx: Receiver<SessionEvent>,
    /// Last activity time (for cleanup)
    last_activity: Instant,
    /// Session ID
    session_id: String,
    /// Model ID
    model_id: String,
}

impl ClaudeSession {
    /// Send a user message and stream the response
    pub fn send_message(&mut self, content: &str, on_chunk: impl Fn(&str)) -> Result<String> {
        self.last_activity = Instant::now();

        // Build and send the JSONL message
        let msg = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": content
            }
        });
        let line = serde_json::to_string(&msg)?;

        tracing::info!(
            session_id = %self.session_id,
            message_len = content.len(),
            "Sending message to persistent Claude session via stdin"
        );

        self.stdin.write_all(line.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;

        tracing::info!(
            session_id = %self.session_id,
            "Message flushed to Claude stdin, waiting for response"
        );

        // Read events until we get a Result or Error
        #[allow(unused_assignments)]
        let mut final_result: Option<String> = None;
        let timeout = Duration::from_secs(120);
        let start = Instant::now();
        let mut last_logged_secs: u64 = 0;

        loop {
            // Check timeout
            if start.elapsed() > timeout {
                return Err(anyhow!("Claude session timed out after {:?}", timeout));
            }

            // Try to receive with timeout
            match self.response_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => match event {
                    SessionEvent::TextChunk(chunk) => {
                        on_chunk(&chunk);
                    }
                    SessionEvent::Result(result) => {
                        tracing::info!(
                            session_id = %self.session_id,
                            result_len = result.len(),
                            elapsed_ms = start.elapsed().as_millis() as u64,
                            "Claude session received final result"
                        );
                        final_result = Some(result);
                        break;
                    }
                    SessionEvent::Error(err) => {
                        tracing::error!(
                            session_id = %self.session_id,
                            error = %err,
                            elapsed_ms = start.elapsed().as_millis() as u64,
                            "Claude session received error event"
                        );
                        return Err(anyhow!("Claude session error: {}", err));
                    }
                    SessionEvent::Exited(code) => {
                        tracing::error!(
                            session_id = %self.session_id,
                            exit_code = code,
                            elapsed_ms = start.elapsed().as_millis() as u64,
                            "Claude session process exited unexpectedly"
                        );
                        return Err(anyhow!("Claude session exited with code: {}", code));
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Log once per 5-second boundary (not every poll tick)
                    let elapsed_secs = start.elapsed().as_secs();
                    if elapsed_secs >= last_logged_secs + 5 {
                        last_logged_secs = elapsed_secs;
                        // Check if process is still alive
                        let alive = matches!(self.child.try_wait(), Ok(None));
                        let exit_status = if !alive {
                            self.child
                                .try_wait()
                                .ok()
                                .flatten()
                                .map(|s| format!("{}", s))
                        } else {
                            None
                        };
                        tracing::info!(
                            session_id = %self.session_id,
                            elapsed_secs = elapsed_secs,
                            pid = self.child.id(),
                            process_alive = alive,
                            exit_status = ?exit_status,
                            "Claude session still waiting for response..."
                        );
                        if !alive {
                            return Err(anyhow!(
                                "Claude CLI process (PID {}) exited while waiting for response{}",
                                self.child.id(),
                                exit_status
                                    .map(|s| format!(" with status: {}", s))
                                    .unwrap_or_default()
                            ));
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(anyhow!("Claude session reader disconnected"));
                }
            }
        }

        self.last_activity = Instant::now();
        Ok(final_result.unwrap_or_default())
    }

    /// Check if the session is still alive
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false, // Exited
            Ok(None) => true,     // Still running
            Err(_) => false,      // Error checking
        }
    }

    /// Kill the session
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ClaudeSession {
    fn drop(&mut self) {
        tracing::debug!(session_id = %self.session_id, "Dropping Claude session");
        self.kill();
    }
}

/// Configuration for spawning a Claude session
#[derive(Clone)]
pub struct SessionConfig {
    pub claude_path: String,
    pub model_id: String,
    pub system_prompt: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            claude_path: "claude".to_string(),
            model_id: "sonnet".to_string(),
            system_prompt: Some("You are a helpful AI assistant".to_string()),
        }
    }
}

/// Manager for persistent Claude CLI sessions
pub struct ClaudeSessionManager {
    sessions: Mutex<HashMap<String, Arc<Mutex<ClaudeSession>>>>,
    /// Track session IDs that have been created (for --resume vs --session-id)
    created_sessions: Mutex<std::collections::HashSet<String>>,
    config: SessionConfig,
}

impl ClaudeSessionManager {
    /// Get the global session manager instance
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<ClaudeSessionManager> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let claude_path = std::env::var("SCRIPT_KIT_CLAUDE_CODE_PATH")
                .unwrap_or_else(|_| "claude".to_string());

            ClaudeSessionManager {
                sessions: Mutex::new(HashMap::new()),
                created_sessions: Mutex::new(std::collections::HashSet::new()),
                config: SessionConfig {
                    claude_path,
                    ..Default::default()
                },
            }
        })
    }

    #[cfg(test)]
    fn new_for_tests(config: SessionConfig) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            created_sessions: Mutex::new(std::collections::HashSet::new()),
            config,
        }
    }

    /// Send a message to a session (creating it if needed)
    pub fn send_message(
        &self,
        session_id: &str,
        content: &str,
        model_id: &str,
        system_prompt: Option<&str>,
        on_chunk: impl Fn(&str),
    ) -> Result<String> {
        tracing::debug!(
            session_id = %session_id,
            content_len = content.len(),
            model_id = %model_id,
            "ClaudeSessionManager.send_message called"
        );

        // Lock the session map only long enough to clone a session handle.
        // Never hold this lock while sending to Claude, because send_message can block.
        let mut session_handle = {
            let sessions = self.sessions.lock().map_err(|e| {
                anyhow!(
                    "ClaudeSessionManager sessions lock poisoned while loading session handle \
                     (attempted=load_session_handle, session_id={}, active_sessions=unknown, error={})",
                    session_id,
                    e
                )
            })?;

            tracing::debug!(
                session_id = %session_id,
                active_sessions = sessions.len(),
                "Loaded session handle snapshot"
            );
            sessions.get(session_id).cloned()
        };

        // Validate the existing session outside the map lock.
        let needs_new_session = match session_handle.as_ref() {
            Some(handle) => {
                let alive = {
                    let mut session = handle.lock().map_err(|e| {
                        anyhow!(
                            "ClaudeSession lock poisoned while checking liveness \
                             (attempted=check_liveness, session_id={}, error={})",
                            session_id,
                            e
                        )
                    })?;
                    session.is_alive()
                };
                tracing::debug!(
                    session_id = %session_id,
                    is_alive = alive,
                    "Checked existing session liveness"
                );
                !alive
            }
            None => {
                tracing::debug!(session_id = %session_id, "No existing session found");
                true
            }
        };

        if needs_new_session {
            tracing::info!(
                session_id = %session_id,
                model_id = %model_id,
                "Creating new persistent Claude session"
            );
            let new_handle = Arc::new(Mutex::new(self.spawn_session(
                session_id,
                model_id,
                system_prompt,
            )?));
            {
                let mut sessions = self.sessions.lock().map_err(|e| {
                    anyhow!(
                        "ClaudeSessionManager sessions lock poisoned while storing session handle \
                         (attempted=store_session_handle, session_id={}, active_sessions=unknown, error={})",
                        session_id,
                        e
                    )
                })?;
                sessions.insert(session_id.to_string(), Arc::clone(&new_handle));
                tracing::debug!(
                    session_id = %session_id,
                    active_sessions = sessions.len(),
                    "Session handle stored"
                );
            }
            session_handle = Some(new_handle);
        }

        // Acquire only the per-session lock for the potentially blocking send.
        let session_handle =
            session_handle.ok_or_else(|| anyhow!("Session not found after creation"))?;
        let mut session = session_handle.lock().map_err(|e| {
            anyhow!(
                "ClaudeSession lock poisoned while sending message \
                 (attempted=send_message, session_id={}, model_id={}, error={})",
                session_id,
                model_id,
                e
            )
        })?;

        tracing::debug!(session_id = %session_id, "Sending message to session");
        let result = session.send_message(content, on_chunk);
        tracing::debug!(
            session_id = %session_id,
            success = result.is_ok(),
            "Message send completed"
        );
        result
    }

    /// Spawn a new Claude CLI session
    fn spawn_session(
        &self,
        session_id: &str,
        model_id: &str,
        system_prompt: Option<&str>,
    ) -> Result<ClaudeSession> {
        // Check if this session was created before (to use --resume vs --session-id)
        let session_existed = self
            .created_sessions
            .lock()
            .map(|set| set.contains(session_id))
            .unwrap_or(false);

        let mut cmd = Command::new(&self.config.claude_path);

        // Full isolation: --setting-sources "" prevents loading project/local settings,
        // but also kills API key loading. We fix this by extracting credential fields
        // from the user's ~/.claude/settings.json and merging them into --settings.
        cmd.arg("--setting-sources").arg("");

        let mut merged_settings = read_user_credential_settings();
        if let Some(obj) = merged_settings.as_object_mut() {
            obj.insert("disableAllHooks".to_string(), serde_json::json!(true));
            obj.insert(
                "permissions".to_string(),
                serde_json::json!({"allow": ["WebSearch", "WebFetch", "Read"]}),
            );
        }
        let settings_json = serde_json::to_string(&merged_settings)
            .unwrap_or_else(|_| r#"{"disableAllHooks":true}"#.to_string());
        cmd.arg("--settings").arg(&settings_json);
        cmd.arg("--tools").arg("WebSearch, WebFetch, Read");
        cmd.arg("--no-chrome");
        cmd.arg("--disable-slash-commands");

        // Streaming mode - IMPORTANT: --verbose is required for stream-json output
        cmd.arg("--print")
            .arg("--verbose")
            .arg("--include-partial-messages")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json");

        // Session persistence:
        // - First time: use --session-id to CREATE the session
        // - If process died and we're recreating: use --resume to CONTINUE the session
        if session_existed {
            tracing::info!(
                session_id = %session_id,
                "Resuming existing Claude session (process died, recreating)"
            );
            cmd.arg("--resume").arg(session_id);
        } else {
            tracing::info!(
                session_id = %session_id,
                "Creating new Claude session"
            );
            cmd.arg("--session-id").arg(session_id);

            // Mark this session as created
            if let Ok(mut set) = self.created_sessions.lock() {
                set.insert(session_id.to_string());
            }
        }

        // Model
        if !model_id.is_empty() && model_id != "default" {
            cmd.arg("--model").arg(model_id);
        }

        // System prompt (only effective on new sessions, ignored on resume)
        let effective_prompt = system_prompt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("You are a helpful AI assistant");
        cmd.arg("--system-prompt").arg(effective_prompt);

        // Clear CLAUDECODE env var so the spawned CLI doesn't think it's a nested session.
        // When Script Kit is running inside a Claude Code session, this var is inherited
        // and causes the child `claude` process to hang or refuse to start.
        cmd.env_remove("CLAUDECODE");

        // Set up pipes
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Log the full command for manual reproduction
        let cmd_debug = format!(
            "{} {}",
            self.config.claude_path,
            cmd.get_args()
                .map(|a| {
                    let s = a.to_string_lossy();
                    if s.contains(' ') || s.contains('{') {
                        format!("'{}'", s)
                    } else {
                        s.to_string()
                    }
                })
                .join(" ")
        );
        tracing::info!(
            session_id = %session_id,
            cmd = %cmd_debug,
            "Spawning persistent Claude CLI process"
        );

        let mut child = cmd.spawn().context("Failed to spawn claude CLI")?;

        tracing::info!(
            session_id = %session_id,
            pid = child.id(),
            "Claude CLI process spawned successfully"
        );

        // Take stdin
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdin"))?;
        let stdin = BufWriter::new(stdin);

        // Take stdout and spawn reader thread
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;

        // Take stderr for logging
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stderr"))?;

        // Create channel for events
        let (tx, rx) = mpsc::channel::<SessionEvent>();

        // Spawn stdout reader thread
        let session_id_clone = session_id.to_string();
        let tx_clone = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut line_count: usize = 0;
            tracing::info!(
                session_id = %session_id_clone,
                "Claude stdout reader thread started, waiting for first output..."
            );
            for line in reader.lines() {
                match line {
                    Ok(line) if line.trim().is_empty() => continue,
                    Ok(line) => {
                        line_count += 1;
                        if line_count <= 3 {
                            // Log first few lines to understand protocol init.
                            // Use the char-boundary-safe truncator: `&line[..200]`
                            // panicked when byte 200 fell inside a multibyte char in
                            // the subprocess output.
                            let preview = crate::logging::truncate_for_log(&line, 200);
                            tracing::info!(
                                session_id = %session_id_clone,
                                line_num = line_count,
                                line = %preview,
                                "Claude stdout line received"
                            );
                        }
                        if let Some(event) = parse_claude_event(&line) {
                            if tx_clone.send(event).is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            session_id = %session_id_clone,
                            error = %e,
                            "Error reading Claude stdout"
                        );
                        let _ = tx_clone.send(SessionEvent::Error(e.to_string()));
                        break;
                    }
                }
            }
            tracing::info!(
                session_id = %session_id_clone,
                total_lines = line_count,
                "Claude stdout reader exited"
            );
        });

        // Spawn stderr reader thread - log at warn level since stderr usually means trouble
        let session_id_clone2 = session_id.to_string();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    tracing::warn!(
                        session_id = %session_id_clone2,
                        stderr_line = %line,
                        "Claude session stderr"
                    );
                }
            }
        });

        Ok(ClaudeSession {
            child,
            stdin,
            response_rx: rx,
            last_activity: Instant::now(),
            session_id: session_id.to_string(),
            model_id: model_id.to_string(),
        })
    }

    /// Close a specific session
    pub fn close_session(&self, session_id: &str) {
        let session_handle = self
            .sessions
            .lock()
            .ok()
            .and_then(|mut sessions| sessions.remove(session_id));
        if let Some(session_handle) = session_handle {
            tracing::info!(session_id = %session_id, "Closing Claude session");
            match session_handle.lock() {
                Ok(mut session) => session.kill(),
                Err(poisoned) => {
                    tracing::warn!(
                        session_id = %session_id,
                        attempted = "close_session",
                        state = "poisoned_lock",
                        "Claude session lock poisoned during close; forcing cleanup"
                    );
                    let mut session = poisoned.into_inner();
                    session.kill();
                }
            }
        }
    }

    /// Close all sessions
    pub fn close_all_sessions(&self) {
        let handles: Vec<(String, Arc<Mutex<ClaudeSession>>)> = self
            .sessions
            .lock()
            .map(|mut sessions| sessions.drain().collect())
            .unwrap_or_default();

        for (id, session_handle) in handles {
            tracing::info!(session_id = %id, "Closing Claude session (cleanup)");
            match session_handle.lock() {
                Ok(mut session) => session.kill(),
                Err(poisoned) => {
                    tracing::warn!(
                        session_id = %id,
                        attempted = "close_all_sessions",
                        state = "poisoned_lock",
                        "Claude session lock poisoned during bulk cleanup; forcing cleanup"
                    );
                    let mut session = poisoned.into_inner();
                    session.kill();
                }
            }
        }
    }

    /// Get count of active sessions
    pub fn session_count(&self) -> usize {
        self.sessions.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Cleanup stale sessions (not used recently)
    pub fn cleanup_stale_sessions(&self, max_idle: Duration) {
        let candidates: Vec<(String, Arc<Mutex<ClaudeSession>>)> = self
            .sessions
            .lock()
            .map(|sessions| {
                sessions
                    .iter()
                    .map(|(id, session)| (id.clone(), Arc::clone(session)))
                    .collect()
            })
            .unwrap_or_default();

        let stale_ids: Vec<String> = candidates
            .iter()
            .filter_map(|(id, session_handle)| {
                let session = match session_handle.lock() {
                    Ok(session) => session,
                    Err(poisoned) => {
                        tracing::warn!(
                            session_id = %id,
                            attempted = "cleanup_stale_sessions",
                            state = "poisoned_lock",
                            "Claude session lock poisoned while checking staleness; forcing cleanup"
                        );
                        poisoned.into_inner()
                    }
                };
                if session.last_activity.elapsed() > max_idle {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        if stale_ids.is_empty() {
            return;
        }

        let stale_handles: Vec<(String, Arc<Mutex<ClaudeSession>>)> = self
            .sessions
            .lock()
            .map(|mut sessions| {
                stale_ids
                    .iter()
                    .filter_map(|id| sessions.remove(id).map(|session| (id.clone(), session)))
                    .collect()
            })
            .unwrap_or_default();

        for (id, session_handle) in stale_handles {
            match session_handle.lock() {
                Ok(mut session) => {
                    tracing::info!(
                        session_id = %id,
                        idle_secs = session.last_activity.elapsed().as_secs(),
                        "Cleaning up stale Claude session"
                    );
                    session.kill();
                }
                Err(poisoned) => {
                    tracing::warn!(
                        session_id = %id,
                        attempted = "cleanup_stale_sessions_remove",
                        state = "poisoned_lock",
                        "Claude session lock poisoned while removing stale session; forcing cleanup"
                    );
                    let mut session = poisoned.into_inner();
                    session.kill();
                }
            }
        }
    }
}

/// Parse a JSONL line from Claude CLI into a SessionEvent
fn parse_claude_event(line: &str) -> Option<SessionEvent> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;

    match v.get("type")?.as_str()? {
        "stream_event" => {
            // Streaming events from --include-partial-messages
            // Format: {"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}}
            let event = v.get("event")?;
            if event.get("type")?.as_str()? == "content_block_delta" {
                let delta = event.get("delta")?;
                if delta.get("type")?.as_str()? == "text_delta" {
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        return Some(SessionEvent::TextChunk(text.to_string()));
                    }
                }
            }
            None
        }
        "assistant" => {
            // Full assistant message (also sent after streaming completes)
            // We can ignore this since we get the chunks via stream_event
            // Format: {"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
            None
        }
        "result" => {
            // Final result
            let result = v.get("result")?.as_str()?.to_string();
            Some(SessionEvent::Result(result))
        }
        "error" => {
            let error = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            Some(SessionEvent::Error(error))
        }
        other => {
            tracing::debug!(
                event_type = %other,
                "Ignoring unhandled Claude session event type"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::sync::{Arc, Barrier};
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_parse_claude_event_result() {
        let line = r#"{"type":"result","subtype":"success","result":"Hello there!"}"#;
        let event = parse_claude_event(line);
        assert!(matches!(event, Some(SessionEvent::Result(r)) if r == "Hello there!"));
    }

    #[test]
    fn test_parse_claude_event_stream_delta() {
        // Streaming events come as stream_event with content_block_delta
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#;
        let event = parse_claude_event(line);
        assert!(matches!(event, Some(SessionEvent::TextChunk(t)) if t == "Hello"));
    }

    #[test]
    fn test_parse_claude_event_assistant_ignored() {
        // Assistant messages are ignored (we get content via stream_event)
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hi"}]}}"#;
        let event = parse_claude_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_claude_event_error() {
        let line = r#"{"type":"error","error":"Something went wrong"}"#;
        let event = parse_claude_event(line);
        assert!(matches!(event, Some(SessionEvent::Error(e)) if e == "Something went wrong"));
    }

    #[test]
    fn test_parse_claude_event_unknown() {
        let line = r#"{"type":"unknown","data":"stuff"}"#;
        let event = parse_claude_event(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_session_config_default() {
        let config = SessionConfig::default();
        assert_eq!(config.claude_path, "claude");
        assert_eq!(config.model_id, "sonnet");
        assert!(config.system_prompt.is_some());
    }

    #[cfg(unix)]
    fn write_mock_claude_cli(delay_ms: u64) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mock-claude-{}.sh", nanos));
        let delay_seconds = format!("{:.3}", delay_ms as f64 / 1000.0);
        let script = format!(
            "#!/usr/bin/env bash\nwhile IFS= read -r _line; do\n  sleep {delay}\n  printf '{{\"type\":\"result\",\"result\":\"ok\"}}\\n'\ndone\n",
            delay = delay_seconds
        );
        fs::write(&path, script).expect("write mock claude script");
        let mut perms = fs::metadata(&path)
            .expect("mock claude metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("set mock claude executable bit");
        path
    }

    #[cfg(unix)]
    #[test]
    fn test_ai_sessions_do_not_serialize_when_multiple_sessions_active() {
        let mock_path = write_mock_claude_cli(800);
        let manager = Arc::new(ClaudeSessionManager::new_for_tests(SessionConfig {
            claude_path: mock_path.to_string_lossy().to_string(),
            model_id: "sonnet".to_string(),
            system_prompt: Some("test".to_string()),
        }));

        // Warm up both sessions so this test isolates send-time lock contention.
        manager
            .send_message("session-a", "warmup-a", "sonnet", Some("test"), |_| {})
            .expect("warmup session-a");
        manager
            .send_message("session-b", "warmup-b", "sonnet", Some("test"), |_| {})
            .expect("warmup session-b");

        let barrier = Arc::new(Barrier::new(3));
        let manager_a = Arc::clone(&manager);
        let barrier_a = Arc::clone(&barrier);
        let t1 = std::thread::spawn(move || {
            barrier_a.wait();
            manager_a
                .send_message("session-a", "msg-a", "sonnet", Some("test"), |_| {})
                .expect("send session-a")
        });

        let manager_b = Arc::clone(&manager);
        let barrier_b = Arc::clone(&barrier);
        let t2 = std::thread::spawn(move || {
            barrier_b.wait();
            manager_b
                .send_message("session-b", "msg-b", "sonnet", Some("test"), |_| {})
                .expect("send session-b")
        });

        let start = Instant::now();
        barrier.wait();
        let _ = t1.join().expect("join sender 1");
        let _ = t2.join().expect("join sender 2");
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_millis(1300),
            "session sends appear serialized, elapsed={elapsed:?}"
        );

        manager.close_all_sessions();
        let _ = fs::remove_file(mock_path);
    }
}
