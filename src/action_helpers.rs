//! Action helper functions for handle_action and trigger_action_by_name
//!
//! This module extracts common functionality to reduce duplication:
//! - Path extraction from search results
//! - SDK action routing
//! - pbcopy helper with proper stdin handling

use std::path::{Path, PathBuf};
use std::sync::mpsc::SyncSender;

use gpui::SharedString;

use crate::protocol::{self, ProtocolAction};
use crate::scripts::SearchResult;

/// Error type for path extraction operations
#[derive(Debug, Clone, PartialEq)]
pub enum PathExtractionError {
    /// No item is selected
    NoSelection,
    /// The selected item type doesn't support this operation
    UnsupportedType(SharedString),
}

impl PathExtractionError {
    /// Get the error message for UI display
    pub fn message(&self) -> SharedString {
        match self {
            PathExtractionError::NoSelection => SharedString::from("No item selected"),
            PathExtractionError::UnsupportedType(msg) => msg.clone(),
        }
    }
}

/// Extract the filesystem path from a SearchResult for reveal/copy operations.
///
/// Supports: Script, App, Agent
/// Not supported: Scriptlet, BuiltIn, Window, Fallback
pub fn extract_path_for_reveal(
    result: Option<&SearchResult>,
) -> Result<PathBuf, PathExtractionError> {
    match result {
        None => Err(PathExtractionError::NoSelection),
        Some(SearchResult::Script(m)) => Ok(m.script.path.clone()),
        Some(SearchResult::App(m)) => Ok(m.app.path.clone()),
        Some(SearchResult::File(m)) => Ok(PathBuf::from(&m.file.path)),
        Some(SearchResult::Agent(m)) => Ok(m.agent.path.clone()),
        Some(SearchResult::Skill(m)) => Ok(m.skill.path.clone()),
        Some(SearchResult::Scriptlet(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot reveal scriptlets in Finder"),
        )),
        Some(SearchResult::BuiltIn(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot reveal built-in features"),
        )),
        Some(SearchResult::Window(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot reveal windows in Finder"),
        )),
        Some(SearchResult::Fallback(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot reveal fallback commands in Finder"),
        )),
        Some(SearchResult::Note(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Notes row has no filesystem path"),
        )),
        Some(SearchResult::BrainHit(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Brain row has no filesystem path"),
        )),
        Some(SearchResult::BrainInboxItem(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Brain inbox row has no filesystem path"),
        )),
        Some(SearchResult::Todo(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Todos row has no filesystem path"),
        )),
        Some(SearchResult::AgentChatHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Agent Chat conversations row has no filesystem path"),
        )),
        Some(SearchResult::AiVault(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("AI Vault row has no filesystem path"),
        )),
        Some(SearchResult::ClipboardHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Clipboard History row has no filesystem path"),
        )),
        Some(SearchResult::DictationHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Dictation History row has no filesystem path"),
        )),
        Some(SearchResult::BrowserTab(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Browser Tabs row has no filesystem path"),
        )),
        Some(SearchResult::BrowserHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Browser History row has no filesystem path"),
        )),
        Some(SearchResult::ScriptIssue(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Script Issues row has no path"),
        )),
        Some(SearchResult::SpineProjection(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Spine projection row has no filesystem path"),
        )),
        Some(SearchResult::Flow(m)) => match m.flow.as_ref() {
            Some(flow) => Ok(PathBuf::from(&flow.path)),
            None => Err(PathExtractionError::UnsupportedType(SharedString::from(
                "Conversation row has no filesystem path",
            ))),
        },
    }
}

/// Extract the filesystem path from a SearchResult for copy path operations.
pub fn extract_path_for_copy(
    result: Option<&SearchResult>,
) -> Result<PathBuf, PathExtractionError> {
    match result {
        None => Err(PathExtractionError::NoSelection),
        Some(SearchResult::Script(m)) => Ok(m.script.path.clone()),
        Some(SearchResult::App(m)) => Ok(m.app.path.clone()),
        Some(SearchResult::File(m)) => Ok(PathBuf::from(&m.file.path)),
        Some(SearchResult::Agent(m)) => Ok(m.agent.path.clone()),
        Some(SearchResult::Skill(m)) => Ok(m.skill.path.clone()),
        Some(SearchResult::Scriptlet(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot copy scriptlet path"),
        )),
        Some(SearchResult::BuiltIn(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot copy built-in path"),
        )),
        Some(SearchResult::Window(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot copy window path"),
        )),
        Some(SearchResult::Fallback(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot copy fallback command path"),
        )),
        Some(SearchResult::Note(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Notes row has no filesystem path"),
        )),
        Some(SearchResult::BrainHit(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Brain row has no filesystem path"),
        )),
        Some(SearchResult::BrainInboxItem(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Brain inbox row has no filesystem path"),
        )),
        Some(SearchResult::Todo(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Todos row has no filesystem path"),
        )),
        Some(SearchResult::AgentChatHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Agent Chat conversations row has no filesystem path"),
        )),
        Some(SearchResult::AiVault(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("AI Vault row has no filesystem path"),
        )),
        Some(SearchResult::ClipboardHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Clipboard History row has no filesystem path"),
        )),
        Some(SearchResult::DictationHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Dictation History row has no filesystem path"),
        )),
        Some(SearchResult::BrowserTab(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Browser Tabs row has no filesystem path"),
        )),
        Some(SearchResult::BrowserHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Browser History row has no filesystem path"),
        )),
        Some(SearchResult::ScriptIssue(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Script Issues row has no path"),
        )),
        Some(SearchResult::SpineProjection(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Spine projection row has no filesystem path"),
        )),
        Some(SearchResult::Flow(m)) => match m.flow.as_ref() {
            Some(flow) => Ok(PathBuf::from(&flow.path)),
            None => Err(PathExtractionError::UnsupportedType(SharedString::from(
                "Conversation row has no filesystem path",
            ))),
        },
    }
}

/// Extract a candidate filesystem path for opening Quick Terminal.
///
/// Canonicalization happens later at dispatch, so pseudo paths are cheap to
/// reject and symlinks resolve before choosing file parent vs directory.
pub fn extract_path_for_quick_terminal(
    result: Option<&SearchResult>,
) -> Result<PathBuf, PathExtractionError> {
    match result {
        None => Err(PathExtractionError::NoSelection),
        Some(SearchResult::Script(m)) => Ok(m.script.path.clone()),
        Some(SearchResult::App(m)) => Ok(m.app.path.clone()),
        Some(SearchResult::File(m)) => Ok(PathBuf::from(&m.file.path)),
        Some(SearchResult::Agent(m)) => Ok(m.agent.path.clone()),
        Some(SearchResult::Skill(m)) => Ok(m.skill.path.clone()),
        Some(SearchResult::Scriptlet(m)) => m
            .scriptlet
            .file_path
            .as_ref()
            .map(|path| PathBuf::from(path.split('#').next().unwrap_or(path)))
            .ok_or_else(|| {
                PathExtractionError::UnsupportedType(SharedString::from(
                    "Scriptlet has no source file path",
                ))
            }),
        Some(SearchResult::BuiltIn(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open built-in features in Quick Terminal"),
        )),
        Some(SearchResult::Window(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open windows in Quick Terminal"),
        )),
        Some(SearchResult::Fallback(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open fallback commands in Quick Terminal"),
        )),
        Some(SearchResult::Note(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open notes in Quick Terminal"),
        )),
        Some(SearchResult::BrainHit(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open brain memories in Quick Terminal"),
        )),
        Some(SearchResult::BrainInboxItem(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open brain inbox items in Quick Terminal"),
        )),
        Some(SearchResult::Todo(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open todos in Quick Terminal"),
        )),
        Some(SearchResult::AgentChatHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open Agent Chat conversations in Quick Terminal"),
        )),
        Some(SearchResult::AiVault(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Use the AI Vault resume action to open a terminal"),
        )),
        Some(SearchResult::ClipboardHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open clipboard history in Quick Terminal"),
        )),
        Some(SearchResult::DictationHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open dictation history in Quick Terminal"),
        )),
        Some(SearchResult::BrowserTab(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open browser tabs in Quick Terminal"),
        )),
        Some(SearchResult::BrowserHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot open browser history in Quick Terminal"),
        )),
        Some(SearchResult::ScriptIssue(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Script Issues row has no path"),
        )),
        Some(SearchResult::SpineProjection(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Spine projection row has no filesystem path"),
        )),
        Some(SearchResult::Flow(m)) => match m.flow.as_ref() {
            Some(flow) => Ok(PathBuf::from(&flow.path)),
            None => Err(PathExtractionError::UnsupportedType(SharedString::from(
                "Conversation row has no filesystem path",
            ))),
        },
    }
}

/// Resolve a path-bearing action target to the cwd Quick Terminal should use.
pub fn resolve_quick_terminal_cwd(path: &Path) -> Result<PathBuf, String> {
    let resolved = std::fs::canonicalize(path).map_err(|error| {
        format!(
            "Failed to resolve Quick Terminal path {}: {}",
            path.display(),
            error
        )
    })?;

    if resolved.is_dir() {
        return Ok(resolved);
    }

    resolved.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "Failed to resolve Quick Terminal directory for {}",
            resolved.display()
        )
    })
}

/// Extract the filesystem path from a SearchResult for edit operations.
///
/// Supports: Script, Skill, Agent
/// Not supported: Scriptlet, BuiltIn, App, Window, Fallback
pub fn extract_path_for_edit(
    result: Option<&SearchResult>,
) -> Result<PathBuf, PathExtractionError> {
    match result {
        None => Err(PathExtractionError::NoSelection),
        Some(SearchResult::Script(m)) => Ok(m.script.path.clone()),
        Some(SearchResult::Agent(m)) => Ok(m.agent.path.clone()),
        Some(SearchResult::Skill(m)) => Ok(m.skill.path.clone()),
        Some(SearchResult::File(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit files from root search"),
        )),
        Some(SearchResult::Note(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit notes from root search"),
        )),
        Some(SearchResult::BrainHit(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit brain memories from root search"),
        )),
        Some(SearchResult::BrainInboxItem(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit brain inbox items from root search"),
        )),
        Some(SearchResult::Todo(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit todos from root search"),
        )),
        Some(SearchResult::BrowserTab(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit browser tabs from root search"),
        )),
        Some(SearchResult::Scriptlet(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit scriptlets"),
        )),
        Some(SearchResult::BuiltIn(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit built-in features"),
        )),
        Some(SearchResult::App(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit applications"),
        )),
        Some(SearchResult::Window(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit windows"),
        )),
        Some(SearchResult::Fallback(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit fallback commands"),
        )),
        Some(SearchResult::AgentChatHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit Agent Chat conversations"),
        )),
        Some(SearchResult::AiVault(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit AI Vault rows"),
        )),
        Some(SearchResult::ClipboardHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit clipboard history"),
        )),
        Some(SearchResult::DictationHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit dictation history"),
        )),
        Some(SearchResult::BrowserHistory(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Cannot edit browser history"),
        )),
        Some(SearchResult::ScriptIssue(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Script Issues row cannot be edited"),
        )),
        Some(SearchResult::SpineProjection(_)) => Err(PathExtractionError::UnsupportedType(
            SharedString::from("Spine projection row cannot be edited"),
        )),
        Some(SearchResult::Flow(m)) => match m.flow.as_ref() {
            Some(flow) => Ok(PathBuf::from(&flow.path)),
            None => Err(PathExtractionError::UnsupportedType(SharedString::from(
                "Conversation row has no filesystem path",
            ))),
        },
    }
}

/// Copy text to clipboard using pbcopy on macOS.
///
/// **Critical fix**: This properly closes stdin before waiting to prevent hangs.
/// pbcopy reads until EOF, so stdin must be dropped before wait() is called.
#[cfg(target_os = "macos")]
pub fn pbcopy(text: &str) -> Result<(), std::io::Error> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;

    // Take ownership of stdin, write, then drop to signal EOF
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
        // stdin is dropped here => EOF delivered to pbcopy
    }

    // Now it's safe to wait - pbcopy has received EOF
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "pbcopy exited with status: {}",
            status
        )));
    }
    Ok(())
}

// ============================================================================
// Stable Action Error Codes
// ============================================================================
//
// Machine-readable error codes for action outcomes. These are stable identifiers
// that never expose internal transport/enum names to the UI or external tooling.
//
// Convention: lowercase_snake_case, prefixed by category.

/// Channel was full when attempting to send a message — the message was dropped.
pub const ERROR_CHANNEL_FULL: &str = "channel_full";

/// Channel was disconnected — the receiving script has exited.
pub const ERROR_CHANNEL_DISCONNECTED: &str = "channel_disconnected";

/// Feature is not supported on the current platform.
pub const ERROR_UNSUPPORTED_PLATFORM: &str = "unsupported_platform";

/// External process (editor, Finder, etc.) failed to launch.
pub const ERROR_LAUNCH_FAILED: &str = "launch_failed";

pub const ROOT_FILE_OPEN_ACTION_ID: &str = "root_file_open";
pub const ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID: &str = "root_file_reveal_in_finder";
pub const ROOT_FILE_COPY_PATH_ACTION_ID: &str = "root_file_copy_path";
pub const ROOT_FILE_COPY_NAME_ACTION_ID: &str = "root_file_copy_name";
pub const ROOT_FILE_QUICK_LOOK_ACTION_ID: &str = "root_file_quick_look";
pub const ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID: &str = "root_file_search_in_folder";
pub const ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID: &str = "root_file_browse_parent_folder";

pub fn is_root_file_action_id(action_id: &str) -> bool {
    matches!(
        action_id,
        ROOT_FILE_OPEN_ACTION_ID
            | ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID
            | ROOT_FILE_COPY_PATH_ACTION_ID
            | ROOT_FILE_COPY_NAME_ACTION_ID
            | ROOT_FILE_QUICK_LOOK_ACTION_ID
            | ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID
            | ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID
    )
}

/// Reveal-in-Finder (or equivalent) operation failed.
pub const ERROR_REVEAL_FAILED: &str = "reveal_failed";

/// Moving a file or script to Trash failed.
pub const ERROR_TRASH_FAILED: &str = "trash_failed";

/// Confirmation modal could not be opened.
pub const ERROR_MODAL_FAILED: &str = "modal_failed";

/// Generic action handler failure (sync error paths in handler dispatch).
pub const ERROR_ACTION_FAILED: &str = "action_failed";

/// User explicitly cancelled the operation (e.g. dismissed a confirmation modal).
pub const ERROR_CANCELLED: &str = "cancelled";

/// No response channel available — no running script to receive the message.
pub const ERROR_NO_SENDER: &str = "no_sender";

/// The surface that initiated an action dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum DispatchSurface {
    /// Dispatched from the action dialog / handler chain.
    Action,
    /// Dispatched from builtin execution.
    Builtin,
}

/// Context threaded through action dispatch so that every log line —
/// including those emitted from async helpers — can be correlated back
/// to the originating user gesture.
#[derive(Debug, Clone)]
pub struct DispatchContext {
    /// Unique identifier for this dispatch, used as a structured tracing field.
    pub trace_id: String,
    /// Whether this dispatch originated from the action dialog or builtin execution.
    pub surface: DispatchSurface,
    /// The action or builtin ID being dispatched.
    pub action_id: String,
}

impl DispatchContext {
    /// Create a new dispatch context for an action.
    pub fn for_action(action_id: impl Into<String>) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            surface: DispatchSurface::Action,
            action_id: action_id.into(),
        }
    }

    /// Create a new dispatch context for a builtin.
    pub fn for_builtin(builtin_id: impl Into<String>) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            surface: DispatchSurface::Builtin,
            action_id: builtin_id.into(),
        }
    }
}

/// High-level outcome status for any action dispatch.
///
/// Maps the transport-specific `SdkActionResult` variants into four
/// coarse-grained buckets that callers can switch on without knowing
/// channel internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum ActionOutcomeStatus {
    /// The action completed successfully (message sent).
    Success,
    /// The action failed due to a transport or system error.
    Error,
    /// The user explicitly cancelled the operation.
    Cancelled,
    /// The action had nothing to do (no handler, no value).
    NoEffect,
}

/// Structured outcome from any action dispatch (builtin, SDK, or handler).
///
/// Every `handle_*_action` function returns this so the top-level router can
/// log consistent structured fields and derive user-facing feedback from a
/// single object.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct DispatchOutcome {
    /// Coarse-grained status bucket.
    pub status: ActionOutcomeStatus,
    /// Stable machine-readable error code (from `ERROR_*` constants), if any.
    pub error_code: Option<&'static str>,
    /// User-facing message suitable for Toast display.  `None` when no
    /// feedback is needed (e.g. success where the UI change IS the feedback).
    pub user_message: Option<String>,
    /// Optional detail for structured logging (never shown to the user).
    pub detail: Option<String>,
    /// Trace ID propagated from the originating `DispatchContext`.
    /// Set by `from_sdk_with_trace` or `with_trace_id` so that async
    /// continuations can correlate back to the originating gesture.
    pub trace_id: Option<String>,
}

impl DispatchOutcome {
    /// The action completed successfully.
    pub fn success() -> Self {
        Self {
            status: ActionOutcomeStatus::Success,
            error_code: None,
            user_message: None,
            detail: None,
            trace_id: None,
        }
    }

    /// The action had nothing to do (not handled by this handler).
    pub fn not_handled() -> Self {
        Self {
            status: ActionOutcomeStatus::NoEffect,
            error_code: None,
            user_message: None,
            detail: None,
            trace_id: None,
        }
    }

    /// The action failed.
    pub fn error(code: &'static str, msg: impl Into<String>) -> Self {
        Self {
            status: ActionOutcomeStatus::Error,
            error_code: Some(code),
            user_message: Some(msg.into()),
            detail: None,
            trace_id: None,
        }
    }

    /// The user explicitly cancelled the operation.
    pub fn cancelled() -> Self {
        Self {
            status: ActionOutcomeStatus::Cancelled,
            error_code: Some(ERROR_CANCELLED),
            user_message: None,
            detail: None,
            trace_id: None,
        }
    }

    /// Whether this outcome represents a handled action (not `NoEffect`).
    pub fn was_handled(&self) -> bool {
        self.status != ActionOutcomeStatus::NoEffect
    }

    /// Build from an `SdkActionResult`, carrying over status, error code, and
    /// user message.
    pub fn from_sdk(result: &SdkActionResult, action_name: &str) -> Self {
        Self {
            status: result.status(),
            error_code: result.error_code(),
            user_message: result.error_message(action_name),
            detail: None,
            trace_id: None,
        }
    }

    /// Build from an `SdkActionResult` with an explicit trace_id from the
    /// originating dispatch context.
    pub fn from_sdk_with_trace(
        result: &SdkActionResult,
        action_name: &str,
        trace_id: impl Into<String>,
    ) -> Self {
        Self {
            status: result.status(),
            error_code: result.error_code(),
            user_message: result.error_message(action_name),
            detail: None,
            trace_id: Some(trace_id.into()),
        }
    }

    /// Attach optional detail for logging.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Attach a trace_id to this outcome for correlation.
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }
}

/// Result of attempting to trigger an SDK action.
#[derive(Debug, Clone, PartialEq)]
pub enum SdkActionResult {
    /// Message was successfully sent to the response channel.
    Sent,
    /// No response channel available (no running script).
    NoSender,
    /// Action has no handler and no value — nothing to send.
    NoEffect,
    /// Response channel is full — message was dropped.
    ChannelFull,
    /// Response channel is disconnected — script has exited.
    ChannelDisconnected,
    /// User explicitly cancelled the operation (e.g. dismissed a confirmation modal).
    /// This is NOT an error — no toast should be shown.
    Cancelled,
}

impl SdkActionResult {
    /// Whether the action was successfully dispatched.
    pub fn is_sent(&self) -> bool {
        matches!(self, SdkActionResult::Sent)
    }

    /// Coarse-grained outcome status.
    ///
    /// Maps transport-specific variants to one of four buckets:
    /// `Success`, `Error`, `Cancelled`, or `NoEffect`.
    pub fn status(&self) -> ActionOutcomeStatus {
        match self {
            SdkActionResult::Sent => ActionOutcomeStatus::Success,
            SdkActionResult::NoEffect => ActionOutcomeStatus::NoEffect,
            SdkActionResult::Cancelled => ActionOutcomeStatus::Cancelled,
            SdkActionResult::NoSender
            | SdkActionResult::ChannelFull
            | SdkActionResult::ChannelDisconnected => ActionOutcomeStatus::Error,
        }
    }

    /// Stable error code for machine consumption, if this is a failure or cancellation.
    ///
    /// Returns `Some` for all non-success outcomes (including `Cancelled`),
    /// allowing machine consumers to distinguish cancellation from errors.
    pub fn error_code(&self) -> Option<&'static str> {
        match self {
            SdkActionResult::Sent | SdkActionResult::NoEffect => None,
            SdkActionResult::NoSender => Some(ERROR_NO_SENDER),
            SdkActionResult::ChannelFull => Some(ERROR_CHANNEL_FULL),
            SdkActionResult::ChannelDisconnected => Some(ERROR_CHANNEL_DISCONNECTED),
            SdkActionResult::Cancelled => Some(ERROR_CANCELLED),
        }
    }

    /// User-facing error message, if any.
    ///
    /// Never exposes raw transport enum variant names — uses human-readable
    /// descriptions backed by stable error codes.
    ///
    /// `Cancelled` returns `None` because cancellation is a normal user action,
    /// not an error — no toast should be shown.
    pub fn error_message(&self, action_name: &str) -> Option<String> {
        match self {
            SdkActionResult::Sent | SdkActionResult::NoEffect | SdkActionResult::Cancelled => None,
            SdkActionResult::NoSender => {
                Some(format!("Action '{}' failed: no active script", action_name))
            }
            SdkActionResult::ChannelFull => Some(format!(
                "Action '{}' failed: response channel is busy",
                action_name,
            )),
            SdkActionResult::ChannelDisconnected => {
                Some("Action failed: script has exited".to_string())
            }
        }
    }

    /// Context-free user-facing message, if any.
    ///
    /// Unlike `error_message`, this does not require an action name and returns
    /// a generic description suitable for logging or machine consumption.
    /// Returns `None` for `Sent`, `NoEffect`, and `Cancelled`.
    pub fn user_message(&self) -> Option<String> {
        match self {
            SdkActionResult::Sent | SdkActionResult::NoEffect | SdkActionResult::Cancelled => None,
            SdkActionResult::NoSender => Some("No active script to receive the action".to_string()),
            SdkActionResult::ChannelFull => {
                Some("Response channel is busy — action dropped".to_string())
            }
            SdkActionResult::ChannelDisconnected => {
                Some("Script has exited — action not delivered".to_string())
            }
        }
    }
}

/// Trigger an SDK action and send the appropriate message to the response channel.
///
/// Routes based on `action.has_action`:
/// - `true`: Sends `ActionTriggered` message
/// - `false` with value: Sends `Submit` message with the value
/// - `false` without value: Logs warning, no message sent
///
/// The `trace_id` parameter is threaded through from the originating dispatch
/// context so every log line can be correlated back to the user gesture.
///
/// Returns an `SdkActionResult` indicating what happened, so callers can
/// show appropriate feedback (e.g. Toast on error).
pub fn trigger_sdk_action(
    action_name: &str,
    action: &ProtocolAction,
    current_input: &str,
    sender: Option<&SyncSender<protocol::Message>>,
    trace_id: &str,
) -> SdkActionResult {
    let Some(sender) = sender else {
        tracing::warn!(action = %action_name, trace_id = %trace_id, "no response sender for SDK action");
        return SdkActionResult::NoSender;
    };

    let send_result = if action.has_action {
        tracing::info!(action = %action_name, trace_id = %trace_id, "SDK action with handler, sending ActionTriggered");
        let msg = protocol::Message::action_triggered(
            action_name.to_string(),
            action.value.clone(),
            current_input.to_string(),
        );
        sender.try_send(msg)
    } else if let Some(ref value) = action.value {
        tracing::info!(action = %action_name, trace_id = %trace_id, value = ?value, "SDK action without handler, submitting value");
        let msg = protocol::Message::Submit {
            id: "action".to_string(),
            value: Some(value.clone()),
        };
        sender.try_send(msg)
    } else {
        tracing::info!(action = %action_name, trace_id = %trace_id, "SDK action has no value and has_action=false");
        return SdkActionResult::NoEffect;
    };

    match send_result {
        Ok(()) => SdkActionResult::Sent,
        Err(std::sync::mpsc::TrySendError::Full(_)) => {
            tracing::warn!(action = %action_name, trace_id = %trace_id, "response channel full - SDK action dropped");
            SdkActionResult::ChannelFull
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
            tracing::info!(action = %action_name, trace_id = %trace_id, "response channel disconnected - script exited");
            SdkActionResult::ChannelDisconnected
        }
    }
}

/// Reserved built-in action IDs that SDK actions cannot shadow.
pub const RESERVED_ACTION_IDS: &[&str] = &[
    // Script context actions
    "run_script",
    "view_logs",
    "reveal_in_finder",
    "copy_path",
    "open_in_quick_terminal",
    "edit_script",
    "edit_scriptlet",
    "reveal_scriptlet_in_finder",
    "copy_scriptlet_path",
    "copy_deeplink",
    "remove_script",
    "delete_script",
    "reload_scripts",
    "reset_ranking",
    // Dynamic shortcut / alias actions
    "configure_shortcut",
    "add_shortcut",
    "update_shortcut",
    "remove_shortcut",
    "add_alias",
    "update_alias",
    "remove_alias",
    // File search context actions
    "open_file",
    "open_directory",
    "quick_look",
    "open_with",
    "show_info",
    "attach_to_ai",
    "copy_filename",
    // Path prompt context actions
    "select_file",
    "open_in_finder",
    "open_in_editor",
    "move_to_trash",
    // Root launcher file result actions
    ROOT_FILE_OPEN_ACTION_ID,
    ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID,
    ROOT_FILE_COPY_PATH_ACTION_ID,
    ROOT_FILE_COPY_NAME_ACTION_ID,
    ROOT_FILE_QUICK_LOOK_ACTION_ID,
    ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID,
    ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID,
    // Internal
    "__cancel__",
];

/// Check if an action ID is reserved by the built-in actions.
pub fn is_reserved_action_id(action_id: &str) -> bool {
    RESERVED_ACTION_IDS.contains(&action_id)
}

/// Find an SDK action by name, with optional shadowing warning.
pub fn find_sdk_action<'a>(
    actions: Option<&'a [ProtocolAction]>,
    action_name: &str,
    warn_on_shadow: bool,
) -> Option<&'a ProtocolAction> {
    let actions = actions?;

    if warn_on_shadow && is_reserved_action_id(action_name) {
        tracing::warn!(action = %action_name, "SDK action shadows a built-in action - will be ignored");
    }

    actions.iter().find(|a| a.name == action_name)
}

#[cfg(test)]
mod tests;
