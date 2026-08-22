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
//!     |chunk| {
//!         println!("Chunk: {}", chunk);
//!         true
//!     },
//! )?;
//!
//! // Close session when done
//! manager.close_session("chat-uuid");
//! ```

use anyhow::{anyhow, Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, BufWriter, Write};
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// Credential-related keys to extract from the user's `~/.claude/settings.json`.
/// These are converted into child-process environment variables so the CLI can
/// authenticate without exposing credential material through `--settings` argv.
const CREDENTIAL_KEYS: &[&str] = &[
    "apiKeyHelper",
    "env", // may contain ANTHROPIC_BASE_URL, ANTHROPIC_API_KEY, etc.
    "oauthAccount",
    "primaryApiKey",
];
const DEFAULT_CLAUDE_SYSTEM_PROMPT: &str = "You are a helpful AI assistant";
#[cfg(unix)]
const CLAUDE_SYSTEM_PROMPT_DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);

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

/// Safe, nonsecret metadata about the credential environment prepared for Claude.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ClaudeLaunchAuthSummary {
    pub(crate) credential_env_count: usize,
    pub(crate) api_key_configured: bool,
    pub(crate) oauth_token_configured: bool,
}

fn insert_claude_credential_env(
    environment: &mut BTreeMap<String, String>,
    key: &str,
    value: &str,
) -> Result<()> {
    if key.is_empty() || key.contains('=') || key.contains('\0') || value.contains('\0') {
        return Err(anyhow!(
            "Claude credential environment contains an invalid variable"
        ));
    }

    if let Some(existing) = environment.get(key) {
        if existing != value {
            return Err(anyhow!(
                "Claude credential settings contain conflicting authentication sources"
            ));
        }
    } else {
        environment.insert(key.to_string(), value.to_string());
    }

    Ok(())
}

fn has_usable_claude_auth(environment: &BTreeMap<String, String>) -> bool {
    [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
    ]
    .iter()
    .any(|key| {
        environment
            .get(*key)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

/// Configure Claude without placing API keys, OAuth tokens, or helpers in argv.
///
/// Claude supports `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, and
/// `CLAUDE_CODE_OAUTH_TOKEN` through its environment. Explicit settings `env`
/// entries retain their names; `primaryApiKey` maps to `ANTHROPIC_API_KEY`, and
/// an OAuth bearer token maps to `CLAUDE_CODE_OAUTH_TOKEN`. Account metadata and
/// `apiKeyHelper` have no verified safe environment equivalent, so they are only
/// ignored when another explicit usable credential is already configured;
/// otherwise configuration fails closed before a child process can be spawned.
pub(crate) fn apply_safe_claude_launch_settings(
    command: &mut Command,
    credential_settings: &serde_json::Value,
) -> Result<ClaudeLaunchAuthSummary> {
    let settings = credential_settings
        .as_object()
        .ok_or_else(|| anyhow!("Claude credential settings must be a JSON object"))?;
    let mut environment = BTreeMap::new();

    if let Some(value) = settings.get("env") {
        let entries = value
            .as_object()
            .ok_or_else(|| anyhow!("Claude credential env must contain only string values"))?;
        for (key, value) in entries {
            let value = value
                .as_str()
                .ok_or_else(|| anyhow!("Claude credential env must contain only string values"))?;
            insert_claude_credential_env(&mut environment, key, value)?;
        }
    }

    if let Some(value) = settings.get("primaryApiKey") {
        if !value.is_null() {
            let key = value
                .as_str()
                .filter(|key| !key.trim().is_empty())
                .ok_or_else(|| {
                    anyhow!("Claude primaryApiKey has an unsupported credential shape")
                })?;
            insert_claude_credential_env(&mut environment, "ANTHROPIC_API_KEY", key)?;
        }
    }

    if let Some(account) = settings.get("oauthAccount") {
        if !account.is_null() {
            let token = match account {
                serde_json::Value::String(token) => Some(token.as_str()),
                serde_json::Value::Object(account) => {
                    let mut token = None;
                    for key in ["accessToken", "oauthToken", "token"] {
                        if let Some(value) = account.get(key) {
                            let candidate = value.as_str().ok_or_else(|| {
                                anyhow!("Claude oauthAccount has an unsupported credential shape")
                            })?;
                            if token.is_some_and(|existing| existing != candidate) {
                                return Err(anyhow!(
                                    "Claude credential settings contain conflicting authentication sources"
                                ));
                            }
                            token = Some(candidate);
                        }
                    }
                    token
                }
                _ => {
                    return Err(anyhow!(
                        "Claude oauthAccount has an unsupported credential shape"
                    ));
                }
            };

            if let Some(token) = token {
                if token.trim().is_empty() {
                    return Err(anyhow!("Claude oauthAccount contains an empty OAuth token"));
                }
                insert_claude_credential_env(&mut environment, "CLAUDE_CODE_OAUTH_TOKEN", token)?;
            } else if !has_usable_claude_auth(&environment) {
                return Err(anyhow!(
                    "Claude oauthAccount cannot authenticate safely without an explicit credential environment"
                ));
            }
        }
    }

    if let Some(helper) = settings.get("apiKeyHelper") {
        if !helper.is_null() {
            let helper = helper.as_str().ok_or_else(|| {
                anyhow!("Claude apiKeyHelper has an unsupported credential shape")
            })?;
            if !helper.trim().is_empty() && !has_usable_claude_auth(&environment) {
                return Err(anyhow!(
                    "Claude apiKeyHelper cannot authenticate safely without an explicit credential environment"
                ));
            }
        }
    }

    let summary = ClaudeLaunchAuthSummary {
        credential_env_count: environment.len(),
        api_key_configured: environment
            .get("ANTHROPIC_API_KEY")
            .is_some_and(|value| !value.trim().is_empty()),
        oauth_token_configured: environment
            .get("CLAUDE_CODE_OAUTH_TOKEN")
            .is_some_and(|value| !value.trim().is_empty()),
    };

    for (key, value) in environment {
        command.env(key, value);
    }

    let safe_settings = serde_json::json!({
        "disableAllHooks": true,
        "permissions": {"allow": ["WebSearch", "WebFetch", "Read"]},
    });
    let serialized_settings = serde_json::to_string(&safe_settings)
        .context("Failed to serialize safe Claude launch settings")?;
    command.arg("--settings").arg(serialized_settings);

    Ok(summary)
}

/// Holds an anonymous, process-local system-prompt transport until the child
/// inherits its read descriptor. It intentionally has no Debug implementation:
/// user context must never be formatted into command diagnostics.
pub(crate) struct ClaudeSystemPromptTransport<'a> {
    prompt: &'a str,
    #[cfg(unix)]
    reader: Option<OwnedFd>,
    #[cfg(unix)]
    writer: Option<OwnedFd>,
}

#[cfg(unix)]
fn set_claude_prompt_descriptor_close_on_exec(descriptor: RawFd) -> std::io::Result<()> {
    // SAFETY: descriptor is borrowed from a live OwnedFd and fcntl changes
    // only its close-on-exec bit; ownership remains with that OwnedFd.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: same live descriptor; preserving all unrelated descriptor flags.
    let updated = unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if updated < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn move_claude_prompt_descriptor_above_stdio(descriptor: OwnedFd) -> std::io::Result<OwnedFd> {
    if descriptor.as_raw_fd() > libc::STDERR_FILENO {
        return Ok(descriptor);
    }

    // SAFETY: descriptor is live and F_DUPFD returns a distinct owned
    // descriptor at or above 3, outside stdin/stdout/stderr reassignment.
    let relocated = unsafe {
        libc::fcntl(
            descriptor.as_raw_fd(),
            libc::F_DUPFD,
            libc::STDERR_FILENO + 1,
        )
    };
    if relocated < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful F_DUPFD transferred a fresh unique descriptor.
    let relocated = unsafe { OwnedFd::from_raw_fd(relocated) };
    drop(descriptor);
    Ok(relocated)
}

/// Configure Claude's verified file-based prompt flag without putting the
/// prompt in argv, environment, logs, or a temporary file.
pub(crate) fn prepare_private_claude_system_prompt<'a>(
    command: &mut Command,
    system_prompt: Option<&'a str>,
) -> Result<ClaudeSystemPromptTransport<'a>> {
    let custom_prompt = system_prompt.filter(|prompt| !prompt.trim().is_empty());
    let prompt = custom_prompt.unwrap_or(DEFAULT_CLAUDE_SYSTEM_PROMPT);

    #[cfg(unix)]
    {
        let mut descriptors = [-1; 2];
        // SAFETY: the array has exactly the two writable slots pipe requires;
        // successful descriptors are immediately transferred into OwnedFd.
        if unsafe { libc::pipe(descriptors.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error())
                .context("Failed to create private Claude system-prompt pipe");
        }
        // SAFETY: pipe succeeded and returned two distinct owned descriptors.
        let reader = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
        // SAFETY: the second successful pipe descriptor has distinct ownership.
        let writer = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
        let reader = move_claude_prompt_descriptor_above_stdio(reader)
            .context("Failed to isolate Claude system-prompt reader from stdio")?;
        let writer = move_claude_prompt_descriptor_above_stdio(writer)
            .context("Failed to isolate Claude system-prompt writer from stdio")?;
        set_claude_prompt_descriptor_close_on_exec(reader.as_raw_fd())
            .context("Failed to secure Claude system-prompt reader")?;
        set_claude_prompt_descriptor_close_on_exec(writer.as_raw_fd())
            .context("Failed to secure Claude system-prompt writer")?;

        let reader_descriptor = reader.as_raw_fd();
        command
            .arg("--system-prompt-file")
            .arg(format!("/dev/fd/{reader_descriptor}"));

        // SAFETY: this child-only hook calls only async-signal-safe fcntl and
        // exposes exactly the designated reader. The write end stays CLOEXEC.
        unsafe {
            command.pre_exec(move || {
                let flags = libc::fcntl(reader_descriptor, libc::F_GETFD);
                if flags < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::fcntl(reader_descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        Ok(ClaudeSystemPromptTransport {
            prompt,
            reader: Some(reader),
            writer: Some(writer),
        })
    }

    #[cfg(not(unix))]
    {
        if custom_prompt.is_some() {
            return Err(anyhow!(
                "Custom Claude system prompts require a private Unix descriptor transport"
            ));
        }
        command.arg("--system-prompt").arg(prompt);
        Ok(ClaudeSystemPromptTransport { prompt })
    }
}

impl ClaudeSystemPromptTransport<'_> {
    #[cfg(unix)]
    fn write_prompt(&mut self) -> Result<()> {
        let writer = self
            .writer
            .take()
            .ok_or_else(|| anyhow!("Claude system-prompt writer is no longer available"))?;
        let mut writer = std::fs::File::from(writer);
        let descriptor = writer.as_raw_fd();
        // SAFETY: writer exclusively owns this live descriptor; preserve its
        // existing status flags while making delivery deadline-bounded.
        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
        if flags < 0 {
            return Err(std::io::Error::last_os_error())
                .context("Failed to inspect Claude system-prompt writer");
        }
        // SAFETY: same owned descriptor; no unrelated status flag is removed.
        if unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error())
                .context("Failed to configure nonblocking Claude system-prompt delivery");
        }

        let deadline = Instant::now()
            .checked_add(CLAUDE_SYSTEM_PROMPT_DELIVERY_TIMEOUT)
            .ok_or_else(|| anyhow!("Claude system-prompt delivery deadline is unavailable"))?;
        let bytes = self.prompt.as_bytes();
        let mut written = 0;
        while written < bytes.len() {
            match writer.write(&bytes[written..]) {
                Ok(0) => {
                    return Err(anyhow!(
                        "Claude system-prompt pipe closed before delivery completed"
                    ));
                }
                Ok(count) => written += count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(anyhow!("Private Claude system-prompt delivery timed out"));
                    }
                    let timeout_ms = remaining.as_millis().clamp(1, 250) as libc::c_int;
                    let mut poll_descriptor = libc::pollfd {
                        fd: descriptor,
                        events: libc::POLLOUT,
                        revents: 0,
                    };
                    // SAFETY: one initialized pollfd stays valid for the call;
                    // the bounded timeout prevents a blocked child deadlock.
                    let polled = unsafe { libc::poll(&mut poll_descriptor, 1, timeout_ms) };
                    if polled < 0 {
                        let error = std::io::Error::last_os_error();
                        if error.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        return Err(error)
                            .context("Failed while waiting for private Claude prompt delivery");
                    }
                }
                Err(error) => {
                    return Err(error)
                        .context("Failed to deliver the private Claude system prompt");
                }
            }
        }
        drop(writer);
        Ok(())
    }

    /// Deliver only after successful spawn. Closing the parent reader first
    /// ensures a child that exits early produces EPIPE instead of retaining a
    /// private prompt indefinitely; owned-child cleanup is fail-closed.
    pub(crate) fn deliver_after_spawn(mut self, child: &mut Child) -> Result<()> {
        #[cfg(unix)]
        let result = {
            drop(self.reader.take());
            self.write_prompt()
        };
        #[cfg(not(unix))]
        let result: Result<()> = {
            let _ = self.prompt;
            Ok(())
        };

        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error)
                .context("Claude process was stopped after private prompt delivery failed");
        }
        Ok(())
    }
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

#[derive(Debug)]
struct ClaudeProviderFailure {
    safe_detail: String,
}

impl std::fmt::Display for ClaudeProviderFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Claude session error: {}", self.safe_detail)
    }
}

impl std::error::Error for ClaudeProviderFailure {}

#[derive(Debug)]
struct ClaudeSessionCancelled;

impl std::fmt::Display for ClaudeSessionCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Claude request was cancelled")
    }
}

impl std::error::Error for ClaudeSessionCancelled {}

pub(crate) fn is_claude_session_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ClaudeSessionCancelled>().is_some()
}

pub(crate) fn should_retry_claude_session_failure(
    error: &anyhow::Error,
    streamed_chunk_count: usize,
) -> bool {
    streamed_chunk_count == 0
        && error.downcast_ref::<ClaudeProviderFailure>().is_none()
        && !is_claude_session_cancelled(error)
}

pub(crate) fn completed_claude_response(result: Option<String>) -> Result<String> {
    match result {
        Some(response) if !response.trim().is_empty() => Ok(response),
        Some(_) => Err(anyhow!("Claude Code returned an empty response")),
        None => Err(anyhow!("Claude Code ended without a final response")),
    }
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
    pub fn send_message(
        &mut self,
        content: &str,
        on_chunk: impl Fn(&str) -> bool,
    ) -> Result<String> {
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
                        if !on_chunk(&chunk) {
                            tracing::info!(
                                session_id = %self.session_id,
                                "Stopping the exact owned persistent Claude request"
                            );
                            if self
                                .child
                                .try_wait()
                                .context("Verify cancellation of the owned Claude session")?
                                .is_none()
                            {
                                self.child
                                    .kill()
                                    .context("Stop the owned cancelled Claude session")?;
                                self.child
                                    .wait()
                                    .context("Reap the owned cancelled Claude session")?;
                            }
                            return Err(ClaudeSessionCancelled.into());
                        }
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
                        let diagnostic = super::reliability::redact_diagnostic(&err);
                        tracing::error!(
                            session_id = %self.session_id,
                            diagnostic_fingerprint = %diagnostic.fingerprint.0,
                            error_bytes = err.len(),
                            elapsed_ms = start.elapsed().as_millis() as u64,
                            "Claude session received error event"
                        );
                        let safe_detail = diagnostic.copyable_detail.unwrap_or_else(|| {
                            "Provider diagnostic details were redacted".to_string()
                        });
                        return Err(ClaudeProviderFailure { safe_detail }.into());
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
        completed_claude_response(final_result)
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
        on_chunk: impl Fn(&str) -> bool,
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

        // Keep project settings isolated while passing credentials only through
        // the child environment; inline `--settings` is deliberately nonsecret.
        cmd.arg("--setting-sources").arg("");
        let launch_auth =
            apply_safe_claude_launch_settings(&mut cmd, &read_user_credential_settings())?;
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
        let system_prompt_transport =
            prepare_private_claude_system_prompt(&mut cmd, system_prompt)?;

        // Clear CLAUDECODE env var so the spawned CLI doesn't think it's a nested session.
        // When Script Kit is running inside a Claude Code session, this var is inherited
        // and causes the child `claude` process to hang or refuse to start.
        cmd.env_remove("CLAUDECODE");

        // Set up pipes
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        tracing::info!(
            session_id = %session_id,
            model_id = %model_id,
            command = "claude",
            credential_env_count = launch_auth.credential_env_count,
            api_key_configured = launch_auth.api_key_configured,
            oauth_token_configured = launch_auth.oauth_token_configured,
            custom_system_prompt = system_prompt.is_some_and(|prompt| !prompt.trim().is_empty()),
            "Spawning persistent Claude CLI process"
        );

        let mut child = cmd.spawn().context("Failed to spawn claude CLI")?;
        system_prompt_transport.deliver_after_spawn(&mut child)?;

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
                            let diagnostic = super::reliability::redact_diagnostic(&line);
                            tracing::info!(
                                session_id = %session_id_clone,
                                line_num = line_count,
                                diagnostic_fingerprint = %diagnostic.fingerprint.0,
                                stdout_bytes = line.len(),
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
                        let diagnostic = super::reliability::redact_diagnostic(&e.to_string());
                        tracing::error!(
                            session_id = %session_id_clone,
                            diagnostic_fingerprint = %diagnostic.fingerprint.0,
                            "Error reading Claude stdout"
                        );
                        let safe_detail = diagnostic.copyable_detail.unwrap_or_else(|| {
                            "Provider diagnostic details were redacted".to_string()
                        });
                        let _ = tx_clone.send(SessionEvent::Error(safe_detail));
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
                    let diagnostic = super::reliability::redact_diagnostic(&line);
                    tracing::warn!(
                        session_id = %session_id_clone2,
                        diagnostic_fingerprint = %diagnostic.fingerprint.0,
                        stderr_bytes = line.len(),
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
            let is_error = v
                .get("is_error")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || v.get("subtype")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|subtype| subtype.starts_with("error"));
            if is_error {
                let error = v
                    .get("errors")
                    .and_then(|errors| match errors {
                        serde_json::Value::String(message) => Some(message.as_str()),
                        serde_json::Value::Array(errors) => errors.iter().find_map(|error| {
                            error.as_str().or_else(|| {
                                error
                                    .get("message")
                                    .or_else(|| error.get("error"))
                                    .and_then(serde_json::Value::as_str)
                            })
                        }),
                        _ => None,
                    })
                    .or_else(|| {
                        v.get("error").and_then(|error| {
                            error.as_str().or_else(|| {
                                error.get("message").and_then(serde_json::Value::as_str)
                            })
                        })
                    })
                    .or_else(|| v.get("result").and_then(serde_json::Value::as_str))
                    .filter(|message| !message.trim().is_empty())
                    .unwrap_or("Claude Code reported an execution error");
                return Some(SessionEvent::Error(error.to_string()));
            }

            let result = v
                .get("result")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            Some(match completed_claude_response(result) {
                Ok(response) => SessionEvent::Result(response),
                Err(error) => SessionEvent::Error(error.to_string()),
            })
        }
        "error" => {
            let error = v
                .get("error")
                .and_then(|error| {
                    error
                        .as_str()
                        .or_else(|| error.get("message").and_then(serde_json::Value::as_str))
                })
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

    fn configured_claude_environment(command: &Command) -> BTreeMap<String, String> {
        command
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|value| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn claude_system_prompt_stays_out_of_arguments_environment_and_command_metadata() {
        let hostile_prompt =
            "private-system-context-canary\nBearer sk-ant-private-prompt-canary\napi_key=secret";
        let mut command = Command::new("claude-not-launched");
        let transport = prepare_private_claude_system_prompt(&mut command, Some(hostile_prompt))
            .expect("private anonymous prompt pipe");
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--system-prompt-file");
        assert!(args[1].starts_with("/dev/fd/"));
        assert!(args.iter().all(|argument| !argument.contains("private-")));
        assert!(!format!("{command:?}").contains("private-"));
        assert!(command.get_envs().all(|(key, value)| {
            !key.to_string_lossy().contains("private-")
                && value.is_none_or(|value| !value.to_string_lossy().contains("private-"))
        }));

        let reader = transport.reader.as_ref().unwrap().as_raw_fd();
        let writer = transport.writer.as_ref().unwrap().as_raw_fd();
        assert!(reader > libc::STDERR_FILENO);
        assert!(writer > libc::STDERR_FILENO);
        // SAFETY: both descriptors are still owned and alive in transport.
        let reader_flags = unsafe { libc::fcntl(reader, libc::F_GETFD) };
        // SAFETY: the writer remains live until transport delivery/drop.
        let writer_flags = unsafe { libc::fcntl(writer, libc::F_GETFD) };
        assert_ne!(reader_flags & libc::FD_CLOEXEC, 0);
        assert_ne!(writer_flags & libc::FD_CLOEXEC, 0);
    }

    #[cfg(unix)]
    #[test]
    fn claude_system_prompt_pipe_delivers_exact_private_bytes_and_closes_descriptors() {
        let hostile_prompt = "private-context-canary\nTOKEN=private-token-canary";
        let mut command = Command::new("claude-not-launched");
        let mut transport =
            prepare_private_claude_system_prompt(&mut command, Some(hostile_prompt)).unwrap();
        let descriptor_path = command
            .get_args()
            .nth(1)
            .unwrap()
            .to_string_lossy()
            .into_owned();

        transport
            .write_prompt()
            .expect("small prompt writes without spawning a child");
        assert!(transport.writer.is_none(), "writer closes immediately");
        let received = fs::read_to_string(&descriptor_path)
            .expect("live anonymous reader is available through /dev/fd");
        assert_eq!(received, hostile_prompt);

        drop(transport.reader.take());
        assert!(transport.reader.is_none(), "reader closes after delivery");
    }

    #[cfg(unix)]
    #[test]
    fn claude_default_system_prompt_uses_the_same_private_descriptor_transport() {
        let mut command = Command::new("claude-not-launched");
        let transport = prepare_private_claude_system_prompt(&mut command, None)
            .expect("default prompt uses private transport on Unix");
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(args[0], "--system-prompt-file");
        assert!(args[1].starts_with("/dev/fd/"));
        assert!(args
            .iter()
            .all(|argument| !argument.contains(DEFAULT_CLAUDE_SYSTEM_PROMPT)));
        assert_eq!(transport.prompt, DEFAULT_CLAUDE_SYSTEM_PROMPT);
    }

    #[cfg(not(unix))]
    #[test]
    fn claude_custom_system_prompt_fails_closed_without_unix_transport() {
        let secret = "private-unsupported-prompt-canary";
        let mut command = Command::new("claude-not-launched");
        let error = prepare_private_claude_system_prompt(&mut command, Some(secret))
            .err()
            .expect("private custom prompts require descriptor support");
        assert!(!error.to_string().contains(secret));
        assert_eq!(command.get_args().count(), 0);
    }

    #[test]
    fn claude_launch_credentials_stay_in_environment_and_settings_stay_nonsecret() {
        let api_key = "sk-ant-private-api-key-canary";
        let auth_token = "private-bearer-token-canary";
        let mut command = Command::new("claude-not-launched");
        let summary = apply_safe_claude_launch_settings(
            &mut command,
            &serde_json::json!({
                "env": {
                    "ANTHROPIC_API_KEY": api_key,
                    "ANTHROPIC_AUTH_TOKEN": auth_token,
                    "ANTHROPIC_BASE_URL": "https://provider.example.invalid",
                },
                "apiKeyHelper": "echo helper-private-canary",
                "oauthAccount": {"emailAddress": "account-private-canary@example.invalid"},
            }),
        )
        .expect("explicit credentials safely replace redundant helpers and account metadata");

        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--settings");
        let inline_settings: serde_json::Value =
            serde_json::from_str(&args[1]).expect("nonsecret settings JSON");
        assert_eq!(
            inline_settings,
            serde_json::json!({
                "disableAllHooks": true,
                "permissions": {"allow": ["WebSearch", "WebFetch", "Read"]},
            })
        );
        for canary in [
            api_key,
            auth_token,
            "helper-private-canary",
            "account-private-canary",
        ] {
            assert!(args.iter().all(|argument| !argument.contains(canary)));
            assert!(!format!("{summary:?}").contains(canary));
        }

        let environment = configured_claude_environment(&command);
        assert_eq!(environment["ANTHROPIC_API_KEY"], api_key);
        assert_eq!(environment["ANTHROPIC_AUTH_TOKEN"], auth_token);
        assert_eq!(summary.credential_env_count, 3);
        assert!(summary.api_key_configured);
        assert!(!summary.oauth_token_configured);
    }

    #[test]
    fn claude_primary_api_key_and_oauth_token_map_to_supported_environment() {
        let primary_key = "primary-private-key-canary";
        let oauth_token = "oauth-private-token-canary";
        let mut command = Command::new("claude-not-launched");
        let summary = apply_safe_claude_launch_settings(
            &mut command,
            &serde_json::json!({
                "primaryApiKey": primary_key,
                "oauthAccount": {"accessToken": oauth_token},
            }),
        )
        .expect("documented credential settings map to Claude environment variables");

        let environment = configured_claude_environment(&command);
        assert_eq!(environment["ANTHROPIC_API_KEY"], primary_key);
        assert_eq!(environment["CLAUDE_CODE_OAUTH_TOKEN"], oauth_token);
        assert!(summary.api_key_configured);
        assert!(summary.oauth_token_configured);
        for argument in command.get_args() {
            let argument = argument.to_string_lossy();
            assert!(!argument.contains(primary_key));
            assert!(!argument.contains(oauth_token));
        }
    }

    #[test]
    fn claude_unsupported_sole_auth_sources_fail_closed_without_exposing_canaries() {
        for (credential_settings, canary) in [
            (
                serde_json::json!({"apiKeyHelper": "echo private-helper-canary"}),
                "private-helper-canary",
            ),
            (
                serde_json::json!({
                    "oauthAccount": {"refreshToken": "private-refresh-token-canary"},
                }),
                "private-refresh-token-canary",
            ),
            (
                serde_json::json!({
                    "env": {"ANTHROPIC_API_KEY": "first-private-canary"},
                    "primaryApiKey": "conflicting-private-canary",
                }),
                "conflicting-private-canary",
            ),
            (
                serde_json::json!({"env": {"ANTHROPIC_API_KEY": {"token": "nested-canary"}}}),
                "nested-canary",
            ),
        ] {
            let mut command = Command::new("claude-not-launched");
            let error = apply_safe_claude_launch_settings(&mut command, &credential_settings)
                .expect_err("unsafe or ambiguous authentication must fail closed");
            assert!(!error.to_string().contains(canary));
            assert_eq!(command.get_args().count(), 0);
            assert_eq!(command.get_envs().count(), 0);
        }
    }

    #[test]
    fn claude_rejects_conflicting_oauth_sources_without_leaking_token_values() {
        let configured_token = "configured-oauth-private-canary";
        let account_token = "account-oauth-private-canary";
        let mut command = Command::new("claude-not-launched");
        let error = apply_safe_claude_launch_settings(
            &mut command,
            &serde_json::json!({
                "env": {"CLAUDE_CODE_OAUTH_TOKEN": configured_token},
                "oauthAccount": {"token": account_token},
            }),
        )
        .expect_err("conflicting explicit OAuth sources must fail closed");

        assert!(!error.to_string().contains(configured_token));
        assert!(!error.to_string().contains(account_token));
        assert_eq!(command.get_args().count(), 0);
        assert_eq!(command.get_envs().count(), 0);
    }

    #[test]
    fn test_parse_claude_event_result() {
        let line = r#"{"type":"result","subtype":"success","result":"Hello there!"}"#;
        let event = parse_claude_event(line);
        assert!(matches!(event, Some(SessionEvent::Result(r)) if r == "Hello there!"));
    }

    #[test]
    fn claude_result_event_honors_provider_declared_error_instead_of_reporting_success() {
        let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":[{"message":"Sign in required"}],"result":"never report success"}"#;

        assert!(matches!(
            parse_claude_event(line),
            Some(SessionEvent::Error(message)) if message == "Sign in required"
        ));
    }

    #[test]
    fn claude_result_error_subtype_is_a_failure_even_without_legacy_boolean() {
        let line =
            r#"{"type":"result","subtype":"error_max_turns","result":"Maximum turns reached"}"#;

        assert!(matches!(
            parse_claude_event(line),
            Some(SessionEvent::Error(message)) if message == "Maximum turns reached"
        ));
    }

    #[test]
    fn claude_result_missing_or_empty_response_is_never_reported_as_success() {
        for line in [
            r#"{"type":"result","subtype":"success"}"#,
            r#"{"type":"result","subtype":"success","result":""}"#,
            r#"{"type":"result","subtype":"success","result":"  \n  "}"#,
        ] {
            assert!(matches!(
                parse_claude_event(line),
                Some(SessionEvent::Error(_))
            ));
        }

        assert!(completed_claude_response(None).is_err());
        assert!(completed_claude_response(Some(" \n ".to_string())).is_err());
        assert_eq!(
            completed_claude_response(Some("  private answer  ".to_string()))
                .expect("preserve actual answer bytes"),
            "  private answer  "
        );
    }

    #[test]
    fn claude_error_event_preserves_structured_provider_failure_message() {
        let line = r#"{"type":"error","error":{"message":"Authentication required"}}"#;

        assert!(matches!(
            parse_claude_event(line),
            Some(SessionEvent::Error(message)) if message == "Authentication required"
        ));
    }

    #[test]
    fn persistent_claude_user_stop_is_typed_cancellation_and_never_retried() {
        let error: anyhow::Error = ClaudeSessionCancelled.into();

        assert!(is_claude_session_cancelled(&error));
        assert!(!should_retry_claude_session_failure(&error, 0));
        assert!(!should_retry_claude_session_failure(&error, 1));
    }

    #[test]
    fn persistent_claude_provider_failures_never_resubmit_the_accepted_request() {
        let error: anyhow::Error = ClaudeProviderFailure {
            safe_detail: "Authentication required".to_string(),
        }
        .into();

        assert!(!is_claude_session_cancelled(&error));
        assert!(!should_retry_claude_session_failure(&error, 0));
        assert!(!should_retry_claude_session_failure(&error, 3));
        assert!(error.to_string().contains("Authentication required"));
    }

    #[test]
    fn persistent_claude_transport_retries_only_before_visible_output() {
        let error = anyhow!("Session transport closed before any response");

        assert!(should_retry_claude_session_failure(&error, 0));
        assert!(!should_retry_claude_session_failure(&error, 1));
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
            .send_message("session-a", "warmup-a", "sonnet", Some("test"), |_| true)
            .expect("warmup session-a");
        manager
            .send_message("session-b", "warmup-b", "sonnet", Some("test"), |_| true)
            .expect("warmup session-b");

        let barrier = Arc::new(Barrier::new(3));
        let manager_a = Arc::clone(&manager);
        let barrier_a = Arc::clone(&barrier);
        let t1 = std::thread::spawn(move || {
            barrier_a.wait();
            manager_a
                .send_message("session-a", "msg-a", "sonnet", Some("test"), |_| true)
                .expect("send session-a")
        });

        let manager_b = Arc::clone(&manager);
        let barrier_b = Arc::clone(&barrier);
        let t2 = std::thread::spawn(move || {
            barrier_b.wait();
            manager_b
                .send_message("session-b", "msg-b", "sonnet", Some("test"), |_| true)
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
