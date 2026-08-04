//! External command handling via stdin.
//!
//! This module provides the ability to control the Script Kit app via stdin JSONL commands.
//! This is primarily used for testing and automation.
//!
//! # Protocol
//!
//! Commands are sent as JSON objects, one per line (JSONL format):
//!
//! ```json
//! {"type": "run", "path": "/path/to/script.ts"}
//! {"type": "show"}
//! {"type": "hide"}
//! {"type": "setFilter", "text": "search term"}
//! {"type": "triggerBuiltin", "name": "clipboardHistory"}
//! {"type": "simulateKey", "key": "enter", "modifiers": ["cmd"]}
//! ```
//!
//! # Example Usage
//!
//! ```bash
//! # Run a script via stdin
//! echo '{"type": "run", "path": "/path/to/script.ts"}' | ./script-kit-gpui
//!
//! # Show/hide the window
//! echo '{"type": "show"}' | ./script-kit-gpui
//! echo '{"type": "hide"}' | ./script-kit-gpui
//!
//! # Filter the script list (for testing)
//! echo '{"type": "setFilter", "text": "hello"}' | ./script-kit-gpui
//! ```

// --- merged from part_000.rs ---
use crate::logging;
use crate::protocol;
use crate::protocol::version::{read_wire_version, ProtocolVersion, ProtocolVersionError};
use crate::protocol::GridDepthOption;
use crate::setup;
use itertools::Itertools;
use std::io::BufRead;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use uuid::Uuid;
/// Default grid size for ShowGrid command
fn default_grid_size() -> u32 {
    8
}
/// Maximum bytes accepted for a single external stdin JSONL command.
const MAX_STDIN_COMMAND_BYTES: usize = 16 * 1024;
// Stdin JSONL protocol versions share the core protocol envelope.
// Missing `protocolVersion` fields remain legacy v1 for backward
// compatibility with the pre-v1 Bun SDK, while explicit versions in
// the core accepted range dispatch through the same typed command layer.
const CAPTURE_WINDOW_RELATIVE_ROOTS: [&str; 2] = [".test-screenshots", "test-screenshots"];
const CAPTURE_WINDOW_SCRIPTKIT_ROOT: &str = "screenshots";
/// Stdin RPC `requestId` newtype.
///
/// Verbatim-echo contract (anomaly slug `attacker-stdin-requestid-unbounded`):
/// every inbound `requestId` is accepted and echoed back on the response
/// envelope byte-for-byte, with no length cap, no truncation, and no
/// encoding transformation. The sole bound is the stdin line cap
/// [`MAX_STDIN_COMMAND_BYTES`] = 16 KiB applied at line ingest. A
/// non-default [`TryFrom<String>`] constructor, a length-bounded wrapper
/// (`Bounded<N>`, `SmallString<N>`, …), or a `.truncate(N)` / `.chars()
/// .take(N)` step anywhere along the receive→echo path silently changes
/// caller behavior for correlation ids that legitimately exceed the cap.
/// Pinned at source level in
/// `tests/stdin_requestid_verbatim_contract.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct ExternalCommandRequestId(String);
impl ExternalCommandRequestId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
impl From<String> for ExternalCommandRequestId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
impl std::fmt::Display for ExternalCommandRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl AsRef<str> for ExternalCommandRequestId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl std::ops::Deref for ExternalCommandRequestId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyModifier {
    #[serde(alias = "meta", alias = "command")]
    Cmd,
    Shift,
    #[serde(alias = "option")]
    Alt,
    #[serde(alias = "control")]
    Ctrl,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureWindowPathPolicyError {
    EmptyPath,
    PathOutsideAllowedRoots {
        resolved_path: PathBuf,
        allowed_roots: Vec<PathBuf>,
    },
    SymlinkInPath {
        resolved_path: PathBuf,
        symlink_path: PathBuf,
    },
    InvalidExtension {
        resolved_path: PathBuf,
    },
    PathResolutionIo {
        operation: &'static str,
        source: String,
    },
}
impl std::fmt::Display for CaptureWindowPathPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "captureWindow path must not be empty"),
            Self::PathOutsideAllowedRoots {
                resolved_path,
                allowed_roots,
            } => {
                let roots = allowed_roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .join(", ");
                write!(
                    f,
                    "resolved path '{}' is outside allowed roots [{}]",
                    resolved_path.display(),
                    roots
                )
            }
            Self::SymlinkInPath {
                resolved_path,
                symlink_path,
            } => write!(
                f,
                "resolved path '{}' contains symlink component '{}'",
                resolved_path.display(),
                symlink_path.display()
            ),
            Self::InvalidExtension { resolved_path } => write!(
                f,
                "resolved path '{}' must end with .png",
                resolved_path.display()
            ),
            Self::PathResolutionIo { operation, source } => {
                write!(f, "path resolution failed during {}: {}", operation, source)
            }
        }
    }
}
impl std::error::Error for CaptureWindowPathPolicyError {}
pub fn validate_capture_window_output_path(
    raw_path: &str,
) -> Result<PathBuf, CaptureWindowPathPolicyError> {
    let cwd =
        std::env::current_dir().map_err(|err| CaptureWindowPathPolicyError::PathResolutionIo {
            operation: "current_dir",
            source: err.to_string(),
        })?;
    let kit_root = setup::get_kit_path();
    validate_capture_window_output_path_with_roots(raw_path, &cwd, &kit_root)
}
fn validate_capture_window_output_path_with_roots(
    raw_path: &str,
    cwd: &Path,
    kit_root: &Path,
) -> Result<PathBuf, CaptureWindowPathPolicyError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(CaptureWindowPathPolicyError::EmptyPath);
    }

    let expanded = PathBuf::from(shellexpand::tilde(trimmed).as_ref());
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    let normalized = normalize_absolute_path(&absolute);

    let allowed_roots = capture_window_allowed_roots(cwd, kit_root);
    let is_allowed = allowed_roots
        .iter()
        .any(|allowed_root| normalized.starts_with(allowed_root));
    if !is_allowed {
        return Err(CaptureWindowPathPolicyError::PathOutsideAllowedRoots {
            resolved_path: normalized,
            allowed_roots,
        });
    }

    let has_png_extension = normalized
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("png"))
        .unwrap_or(false);
    if !has_png_extension {
        return Err(CaptureWindowPathPolicyError::InvalidExtension {
            resolved_path: normalized,
        });
    }

    match first_symlink_component(&normalized) {
        Ok(Some(symlink_path)) => Err(CaptureWindowPathPolicyError::SymlinkInPath {
            resolved_path: normalized,
            symlink_path,
        }),
        Ok(None) => Ok(normalized),
        Err(err) => Err(CaptureWindowPathPolicyError::PathResolutionIo {
            operation: "symlink_metadata",
            source: err.to_string(),
        }),
    }
}
fn capture_window_allowed_roots(cwd: &Path, kit_root: &Path) -> Vec<PathBuf> {
    let normalized_cwd = normalize_absolute_path(cwd);
    let normalized_kit_root = normalize_absolute_path(kit_root);
    let mut roots = CAPTURE_WINDOW_RELATIVE_ROOTS
        .iter()
        .map(|root| normalize_absolute_path(&normalized_cwd.join(root)))
        .collect::<Vec<_>>();
    roots.push(normalize_absolute_path(
        &normalized_kit_root.join(CAPTURE_WINDOW_SCRIPTKIT_ROOT),
    ));
    roots.sort();
    roots.dedup();
    roots
}
fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = normalized.pop();
                if !popped && !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}
fn first_symlink_component(path: &Path) -> std::io::Result<Option<PathBuf>> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());

        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Ok(Some(current.clone()));
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(None)
}
enum StdinLineRead {
    Eof,
    Line(String),
    TooLong { raw: String, raw_len: usize },
}
fn read_stdin_line_bounded<R: BufRead>(
    reader: &mut R,
    byte_buffer: &mut Vec<u8>,
    max_line_bytes: usize,
) -> std::io::Result<StdinLineRead> {
    byte_buffer.clear();
    let mut total_bytes = 0usize;
    let mut saw_any_data = false;

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if !saw_any_data {
                return Ok(StdinLineRead::Eof);
            }

            let line = String::from_utf8_lossy(byte_buffer).into_owned();
            return Ok(StdinLineRead::Line(line));
        }

        saw_any_data = true;
        let newline_pos = available.iter().position(|&byte| byte == b'\n');
        let consumed_len = newline_pos.map_or(available.len(), |idx| idx + 1);

        if byte_buffer.len() < max_line_bytes {
            let remaining = max_line_bytes - byte_buffer.len();
            let copy_len = remaining.min(consumed_len);
            byte_buffer.extend_from_slice(&available[..copy_len]);
        }

        reader.consume(consumed_len);
        total_bytes = total_bytes.saturating_add(consumed_len);

        if total_bytes > max_line_bytes {
            // Drain the rest of this oversized command to recover.
            if newline_pos.is_none() {
                loop {
                    let remaining = reader.fill_buf()?;
                    if remaining.is_empty() {
                        break;
                    }
                    if let Some(next_newline_pos) = remaining.iter().position(|&byte| byte == b'\n')
                    {
                        reader.consume(next_newline_pos + 1);
                        total_bytes = total_bytes.saturating_add(next_newline_pos + 1);
                        break;
                    }
                    let chunk_len = remaining.len();
                    reader.consume(chunk_len);
                    total_bytes = total_bytes.saturating_add(chunk_len);
                }
            }

            let raw = String::from_utf8_lossy(byte_buffer).into_owned();
            return Ok(StdinLineRead::TooLong {
                raw,
                raw_len: total_bytes,
            });
        }

        if newline_pos.is_some() {
            let line = String::from_utf8_lossy(byte_buffer).into_owned();
            return Ok(StdinLineRead::Line(line));
        }
    }
}
/// External commands that can be sent to the app via stdin
///
/// All commands support an optional `requestId` field for correlation.
/// When present, the request_id is logged with all related operations,
/// making it easy for AI agents to trace command execution through logs.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ExternalCommand {
    /// Run a script by path
    Run {
        path: String,
        /// Optional request ID for correlation in logs
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Show the window
    Show {
        /// Optional request ID for correlation in logs
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Hide the window
    Hide {
        /// Optional request ID for correlation in logs
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Set the filter text (for testing)
    SetFilter {
        text: String,
        /// Optional request ID for correlation in logs
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Update the active menu-syntax handler form field and sync it back to
    /// the canonical main input text.
    SetMenuSyntaxFormField {
        /// Field id from `getState.menuSyntaxMainHint.form.fields[].id`.
        /// When omitted, the currently focused handler form field is edited.
        #[serde(default)]
        field: Option<String>,
        value: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Trigger a built-in feature by canonical `builtinId` (v1+) or by
    /// legacy `name` alias (v1-deprecated). Exactly one of `builtinId`
    /// or `name` MUST be supplied; the receiver decides which path ran
    /// via [`ExternalCommand::trigger_builtin_ref`].
    TriggerBuiltin {
        /// Canonical `builtin/...` command id. Preferred over `name`.
        #[serde(default, rename = "builtinId")]
        builtin_id: Option<String>,
        /// Deprecated v1 fallback — legacy alias string. Resolved via
        /// the `TriggerBuiltinRegistry` legacy-alias table. Callers
        /// should migrate to `builtinId`.
        #[serde(default)]
        name: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Simulate a key press (for testing)
    /// key: Key name like "enter", "escape", "up", "down", "k", etc.
    /// modifiers: Optional array of modifiers ["cmd", "shift", "alt", "ctrl"]
    SimulateKey {
        key: String,
        #[serde(default)]
        modifiers: Vec<KeyModifier>,
        /// Optional automation target for agent-driven DevTools actions.
        /// When omitted, dispatch keeps the legacy focused-window behavior.
        #[serde(default)]
        target: Option<protocol::AutomationWindowTarget>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Inject a main launcher hotkey key-down or key-up for gesture-probe tests.
    SimulateMainHotkeyGesture {
        /// `"down"` or `"up"`
        phase: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open the Notes window (for testing)
    OpenNotes,
    /// Open the About surface in the main window (for testing)
    OpenAbout,
    /// Open the CreationFeedback surface with a deterministic local path fixture.
    OpenCreationFeedback {
        #[serde(default)]
        path: Option<String>,
        #[serde(default, rename = "receiptPath")]
        receipt_path: Option<String>,
        #[serde(default, rename = "receiptStatus")]
        receipt_status: Option<String>,
        #[serde(default, rename = "verificationStatus")]
        verification_status: Option<crate::ai::GeneratedScriptVerificationStatus>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open a deterministic in-window ConfirmPrompt fixture for visual/layout proof.
    OpenConfirmPrompt {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        body: Option<String>,
        #[serde(default, rename = "confirmText")]
        confirm_text: Option<String>,
        #[serde(default, rename = "cancelText")]
        cancel_text: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open a deterministic detached Agent Chat placeholder window for visual/layout proof.
    ///
    /// This fixture does not start an agent, acquire provider credentials, submit
    /// prompts, or mutate chat history. It only opens the detached window shell.
    OpenAgentChatDetachedFixture {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open the detached Agent Chat history popup against fixture-owned history.
    OpenAgentChatHistoryPopupFixture {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open an ordinary script-owned ChatPrompt without spawning a script process.
    /// This is the deterministic agent-facing equivalent of the SDK `chat()` message.
    OpenChatPromptFixture {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Close one exact generation-scoped PromptPopup through AppKit.
    ClosePromptPopupNatively {
        target: crate::protocol::AutomationWindowTarget,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open the Agent Chat window (for testing)
    OpenAi,
    /// Open the Mini Agent Chat window (for testing)
    OpenMiniAi,
    /// Open the Agent Chat window with mock data (for visual testing)
    /// This inserts sample conversations to test the UI layout
    OpenAiWithMockData,
    /// Open the Mini Agent Chat window with mock data (for visual testing)
    OpenMiniAiWithMockData,
    /// Open focused-text mini Agent Chat with fixture focused text and deterministic output.
    OpenFocusedTextAgentChatWithMockData {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        instruction: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Capture the currently focused external field and open focused-text mini Agent Chat
    /// with deterministic mock output. Gated by SCRIPT_KIT_FOCUSED_TEXT_LIVE_FIXTURE=1.
    OpenFocusedTextAgentChatFromFocusedFieldWithMockData {
        #[serde(default)]
        instruction: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open focused-text mini Agent Chat with fixture focused text and real Pi submission.
    OpenFocusedTextAgentChatWithPiData {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        instruction: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Show the AI command bar (Cmd+K menu) for testing the refactored ActionsDialog
    ShowAiCommandBar,
    /// Simulate a key press in the AI window (for testing command bar navigation)
    /// key: Key name like "enter", "escape", "up", "down", "k", etc.
    /// modifiers: Optional array of modifiers ["cmd", "shift", "alt", "ctrl"]
    SimulateAiKey {
        key: String,
        #[serde(default)]
        modifiers: Vec<KeyModifier>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Capture a screenshot of a window by title pattern and save to file (for testing)
    /// title: Title pattern to match (e.g., "Script Kit Agent" for the Agent Chat window)
    /// path: File path to save the PNG screenshot
    CaptureWindow {
        title: String,
        path: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Set the AI window search filter (for testing chat search)
    /// text: Search query to filter chats
    SetAiSearch {
        text: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Set the AI window input text and optionally submit (for testing streaming)
    /// text: Message text to set in the input field
    /// submit: If true, submit the message after setting (triggers streaming)
    SetAiInput {
        text: String,
        #[serde(default)]
        submit: bool,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Set the Agent Chat input text and optionally submit (for testing composer behavior)
    /// text: Message text to set in the Agent Chat input field
    /// submit: If true, submit the message after setting
    SetAgentChatInput {
        text: String,
        #[serde(default)]
        submit: bool,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Set the focused-text mini scope input text.
    SetAgentChatScopeInput {
        text: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Select a focused-text variation by index (0, 1, or 2).
    SelectAgentChatVariation {
        index: usize,
        #[serde(default)]
        edit: bool,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Get focused-text variation snapshots.
    GetAgentChatVariations {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Send an Escape key through the Agent Chat view's progressive handler.
    AgentChatEscape {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Install a no-token Agent Chat transcript fixture for devtools proof.
    ///
    /// phase accepts "awaitingFirstAssistantText", "assistantText", "idle", or "error".
    /// This mutates the active Agent Chat thread only; it never submits to an agent.
    SetAgentChatTestFixture {
        phase: String,
        #[serde(default, rename = "userText")]
        user_text: Option<String>,
        #[serde(default, rename = "assistantText")]
        assistant_text: Option<String>,
        #[serde(default, rename = "messageCount")]
        message_count: Option<usize>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Scroll the active Agent Chat transcript to a logical list offset.
    ///
    /// DevTools-only proof primitive for transcript performance scenarios.
    SetAgentChatTranscriptScroll {
        #[serde(rename = "itemIx")]
        item_ix: usize,
        #[serde(default, rename = "offsetPx")]
        offset_px: f32,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Show the debug grid overlay with options (for visual testing)
    ShowGrid {
        #[serde(default = "default_grid_size", rename = "gridSize")]
        grid_size: u32,
        #[serde(default, rename = "showBounds")]
        show_bounds: bool,
        #[serde(default, rename = "showBoxModel")]
        show_box_model: bool,
        #[serde(default, rename = "showAlignmentGuides")]
        show_alignment_guides: bool,
        #[serde(default, rename = "showDimensions")]
        show_dimensions: bool,
        #[serde(default)]
        depth: GridDepthOption,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Hide the debug grid overlay
    HideGrid,
    /// Show the shortcut recorder modal (for testing)
    /// command_id: ID of the command (e.g., "test/my-command")
    /// command_name: Display name (e.g., "My Command")
    ShowShortcutRecorder {
        #[serde(rename = "commandId")]
        command_id: String,
        #[serde(rename = "commandName")]
        command_name: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Query the AI window state as a machine-readable JSON snapshot.
    ///
    /// Returns structural metadata (mode, overlay visibility, counts) via structured
    /// tracing at info level. Never exposes conversation content or PII.
    GetAiWindowState {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Execute a fallback action (e.g., Search Google, Copy to Clipboard)
    /// This is triggered when a fallback item is selected from the UI
    ExecuteFallback {
        /// The fallback ID (e.g., "search-google", "copy-to-clipboard")
        #[serde(rename = "fallbackId")]
        fallback_id: String,
        /// The user's input text to use with the fallback action
        input: String,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Trigger an action on a shared-actions-dialog host by its action_id (for testing).
    ///
    /// Bypasses keyboard simulation so agentic-testing harnesses can fire a known
    /// action without driving Cmd+K → arrow-nav → Enter through simulateKey. Pairs
    /// with `getElements` against the actions-dialog window to discover valid ids.
    ///
    /// actionId: The action's unique id from the actions-dialog UI (e.g. the value
    ///   exposed via getElements on the automation window).
    /// host: Optional camelCase host label. Accepted values: "argPrompt",
    ///   "divPrompt", "editorPrompt", "termPrompt", "formPrompt", "chatPrompt",
    ///   "mainList", "fileSearch", "clipboardHistory", "dictationHistory",
    ///   "emojiPicker", "appLauncher", "builtinList", "webcamPrompt", "agentChatChat",
    ///   "agent_chatHistory". When omitted, resolves to the current view's host.
    TriggerAction {
        #[serde(rename = "actionId")]
        action_id: String,
        #[serde(default)]
        host: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Inject a synthetic clipboard capture without reading NSPasteboard.
    /// Large text is supplied by `textFile` under the active sandbox SK_PATH.
    InjectClipboardCaptureFixture {
        payload: crate::clipboard_history::ClipboardCaptureFixturePayload,
        #[serde(default, rename = "sourceBundleId")]
        source_bundle_id: Option<String>,
        #[serde(default, rename = "concealedTypes")]
        concealed_types: Vec<String>,
        #[serde(rename = "changeGeneration")]
        change_generation: u64,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Invoke the Agent Chat composer's `paste_text_from_clipboard` handler directly
    /// on the active `AppView::AgentChatView`, bypassing the OS Cmd+V heuristic
    /// (which routes pastes to the frontmost app — during automation runs
    /// that's the invoking terminal, not the Agent Chat composer).
    ///
    /// Pairs with the Pass #31 source-level invariants pinned in
    /// `tests/agent_chat_composer_paste_text_contract.rs`: this command is the
    /// substrate that lets live verification actually exercise the pinned
    /// paste-receiver shape (arboard read → CRLF normalize → `prepare_pasted_text`
    /// → `thread.input.insert_str`) against the current system clipboard.
    ///
    /// Returns a `Err("Agent Chat view is not active")` via the structured
    /// tracing channel when no `AgentChatView` is active; `Err("clipboard is
    /// empty or text fetch failed")` when `paste_text_from_clipboard` returns
    /// false (empty clipboard, non-text clipboard, or arboard init failure).
    PasteClipboardIntoAgentChat {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Inject a synthetic dictation transcript result into the agentic-testing
    /// surface. The handler routes through the same delivery helper used after
    /// transcription, so tests can prove Agent Chat reveal/focus behavior without
    /// depending on microphone capture or the local transcription model.
    ///
    /// transcript: Synthetic transcript text. Only `.len()` is logged — content
    ///   is not emitted at info level (real transcripts carry PII).
    /// partialTranscript: Optional partial text used only when `transcript` is
    ///   empty. This lets tests prove final-empty fallback behavior without
    ///   touching live microphone/provider code.
    /// target: Optional target label. Accepted values mirror `DictationTarget`:
    ///   "mainWindowFilter", "mainWindowPrompt", "notesEditor", "aiChatComposer",
    ///   "tabAiHarness", "externalApp", plus short aliases like "agent_chat". Unknown
    ///   or absent values fall back to the active dictation target, then the
    ///   current UI-derived target.
    PushDictationResult {
        transcript: String,
        #[serde(default, rename = "partialTranscript")]
        partial_transcript: Option<String>,
        #[serde(default)]
        target: Option<String>,
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open the Dictation overlay with synthetic visual state only.
    ///
    /// This is a DevTools visual fixture: it does not request microphone
    /// permission, start audio capture, invoke transcription, or deliver text.
    OpenDictationOverlayFixture {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Open the synthetic, no-persistence microphone selector on the active
    /// Dictation overlay fixture. Refuses production overlays.
    OpenDictationMicrophonePopupFixture {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
    /// Read-only probe exposing the current `~/.kit/config.ts`
    /// fingerprint so automation can verify a write landed on disk
    /// without shelling out to `bun` or `stat`. The handler emits a
    /// `config_fingerprint_result` tracing event carrying the receipt
    /// JSON (`path`, `len`, `modified_ms`, and an optional
    /// `fingerprintHash` reserved for a future content-hash extension).
    /// When the file is missing or stat fails the event still fires
    /// with `ok = false` and `error_code = "config_file_missing"` so
    /// automation can distinguish "no file" from "command did not
    /// land".
    GetConfigFingerprint {
        #[serde(default, rename = "requestId")]
        request_id: Option<ExternalCommandRequestId>,
    },
}
/// Result of normalizing a [`ExternalCommand::TriggerBuiltin`] payload.
/// Carries the raw string plus whether it came from the canonical or
/// the deprecated alias slot, so dispatch can bump the right counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinRef<'a> {
    /// Canonical `builtin/...` or `builtin-...` command id supplied as
    /// `builtinId`. Routed through `lookup_command_id`.
    CanonicalId(&'a str),
    /// Deprecated legacy alias supplied as `name`. Routed through
    /// `lookup_legacy_alias` and bumps the
    /// `trigger_builtin_deprecated_name_total` counter.
    LegacyAlias(&'a str),
}

impl ExternalCommand {
    /// Extract the canonical-id-or-legacy-alias pair from a
    /// `TriggerBuiltin` payload, rejecting ambiguous combinations up
    /// front (both present, neither present).
    ///
    /// Returns `Ok(None)` when the variant is not `TriggerBuiltin`.
    pub fn trigger_builtin_ref(&self) -> Result<Option<BuiltinRef<'_>>, String> {
        match self {
            Self::TriggerBuiltin {
                builtin_id: Some(id),
                name: None,
                ..
            } => Ok(Some(BuiltinRef::CanonicalId(id.as_str()))),
            Self::TriggerBuiltin {
                builtin_id: None,
                name: Some(n),
                ..
            } => Ok(Some(BuiltinRef::LegacyAlias(n.as_str()))),
            Self::TriggerBuiltin {
                builtin_id: Some(_),
                name: Some(_),
                ..
            } => Err(
                "triggerBuiltin accepts either `builtinId` or deprecated `name`, not both"
                    .to_string(),
            ),
            Self::TriggerBuiltin {
                builtin_id: None,
                name: None,
                ..
            } => Err("triggerBuiltin requires `builtinId` (or deprecated `name`)".to_string()),
            _ => Ok(None),
        }
    }

    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Run { request_id, .. }
            | Self::Show { request_id }
            | Self::Hide { request_id }
            | Self::SetFilter { request_id, .. }
            | Self::SetMenuSyntaxFormField { request_id, .. }
            | Self::TriggerBuiltin { request_id, .. }
            | Self::SimulateKey { request_id, .. }
            | Self::SimulateMainHotkeyGesture { request_id, .. }
            | Self::SimulateAiKey { request_id, .. }
            | Self::CaptureWindow { request_id, .. }
            | Self::SetAiSearch { request_id, .. }
            | Self::SetAiInput { request_id, .. }
            | Self::SetAgentChatInput { request_id, .. }
            | Self::SetAgentChatScopeInput { request_id, .. }
            | Self::SelectAgentChatVariation { request_id, .. }
            | Self::GetAgentChatVariations { request_id, .. }
            | Self::AgentChatEscape { request_id, .. }
            | Self::SetAgentChatTestFixture { request_id, .. }
            | Self::SetAgentChatTranscriptScroll { request_id, .. }
            | Self::GetAiWindowState { request_id, .. }
            | Self::ShowGrid { request_id, .. }
            | Self::ShowShortcutRecorder { request_id, .. }
            | Self::ExecuteFallback { request_id, .. }
            | Self::TriggerAction { request_id, .. }
            | Self::InjectClipboardCaptureFixture { request_id, .. }
            | Self::PushDictationResult { request_id, .. }
            | Self::OpenDictationOverlayFixture { request_id, .. }
            | Self::OpenDictationMicrophonePopupFixture { request_id, .. }
            | Self::GetConfigFingerprint { request_id, .. }
            | Self::OpenFocusedTextAgentChatWithMockData { request_id, .. }
            | Self::OpenFocusedTextAgentChatWithPiData { request_id, .. }
            | Self::OpenCreationFeedback { request_id, .. }
            | Self::OpenConfirmPrompt { request_id, .. }
            | Self::OpenAgentChatDetachedFixture { request_id, .. }
            | Self::OpenAgentChatHistoryPopupFixture { request_id, .. }
            | Self::OpenChatPromptFixture { request_id, .. }
            | Self::ClosePromptPopupNatively { request_id, .. }
            | Self::PasteClipboardIntoAgentChat { request_id, .. } => {
                request_id.as_ref().map(ExternalCommandRequestId::as_str)
            }
            _ => None,
        }
    }

    pub fn command_type(&self) -> &'static str {
        match self {
            Self::Run { .. } => "run",
            Self::Show { .. } => "show",
            Self::Hide { .. } => "hide",
            Self::SetFilter { .. } => "setFilter",
            Self::SetMenuSyntaxFormField { .. } => "setMenuSyntaxFormField",
            Self::TriggerBuiltin { .. } => "triggerBuiltin",
            Self::SimulateKey { .. } => "simulateKey",
            Self::SimulateMainHotkeyGesture { .. } => "simulateMainHotkeyGesture",
            Self::OpenNotes => "openNotes",
            Self::OpenAbout => "openAbout",
            Self::OpenCreationFeedback { .. } => "openCreationFeedback",
            Self::OpenConfirmPrompt { .. } => "openConfirmPrompt",
            Self::OpenAgentChatDetachedFixture { .. } => "openAgentChatDetachedFixture",
            Self::OpenAgentChatHistoryPopupFixture { .. } => "openAgentChatHistoryPopupFixture",
            Self::OpenChatPromptFixture { .. } => "openChatPromptFixture",
            Self::ClosePromptPopupNatively { .. } => "closePromptPopupNatively",
            Self::OpenAi => "openAi",
            Self::OpenMiniAi => "openMiniAi",
            Self::OpenAiWithMockData => "openAiWithMockData",
            Self::OpenMiniAiWithMockData => "openMiniAiWithMockData",
            Self::OpenFocusedTextAgentChatWithMockData { .. } => {
                "openFocusedTextAgentChatWithMockData"
            }
            Self::OpenFocusedTextAgentChatFromFocusedFieldWithMockData { .. } => {
                "openFocusedTextAgentChatFromFocusedFieldWithMockData"
            }
            Self::OpenFocusedTextAgentChatWithPiData { .. } => "openFocusedTextAgentChatWithPiData",
            Self::ShowAiCommandBar => "showAiCommandBar",
            Self::SimulateAiKey { .. } => "simulateAiKey",
            Self::CaptureWindow { .. } => "captureWindow",
            Self::SetAiSearch { .. } => "setAiSearch",
            Self::SetAiInput { .. } => "setAiInput",
            Self::SetAgentChatInput { .. } => "setAgentChatInput",
            Self::SetAgentChatScopeInput { .. } => "setAgentChatScopeInput",
            Self::SelectAgentChatVariation { .. } => "selectAgentChatVariation",
            Self::GetAgentChatVariations { .. } => "getAgentChatVariations",
            Self::AgentChatEscape { .. } => "agent_chatEscape",
            Self::SetAgentChatTestFixture { .. } => "setAgentChatTestFixture",
            Self::SetAgentChatTranscriptScroll { .. } => "setAgentChatTranscriptScroll",
            Self::GetAiWindowState { .. } => "getAiWindowState",
            Self::ShowGrid { .. } => "showGrid",
            Self::HideGrid => "hideGrid",
            Self::ShowShortcutRecorder { .. } => "showShortcutRecorder",
            Self::ExecuteFallback { .. } => "executeFallback",
            Self::TriggerAction { .. } => "triggerAction",
            Self::InjectClipboardCaptureFixture { .. } => "injectClipboardCaptureFixture",
            Self::PasteClipboardIntoAgentChat { .. } => "pasteClipboardIntoAgentChat",
            Self::PushDictationResult { .. } => "pushDictationResult",
            Self::OpenDictationOverlayFixture { .. } => "openDictationOverlayFixture",
            Self::OpenDictationMicrophonePopupFixture { .. } => {
                "openDictationMicrophonePopupFixture"
            }
            Self::GetConfigFingerprint { .. } => "getConfigFingerprint",
        }
    }
}

/// Every accepted stdin JSONL verb, in a stable order. This is the single
/// source of truth for drift audits that compare the `kit://stdin-commands`
/// MCP resource payload against the real `ExternalCommand` parser. The unit
/// test `external_command_verbs_cover_every_variant` pins the invariant that
/// every arm of [`ExternalCommand::command_type`] has a matching entry here.
pub const EXTERNAL_COMMAND_VERBS: &[&str] = &[
    "run",
    "show",
    "hide",
    "setFilter",
    "setMenuSyntaxFormField",
    "triggerBuiltin",
    "simulateKey",
    "simulateMainHotkeyGesture",
    "openNotes",
    "openAbout",
    "openCreationFeedback",
    "openConfirmPrompt",
    "openAgentChatDetachedFixture",
    "openAgentChatHistoryPopupFixture",
    "openChatPromptFixture",
    "closePromptPopupNatively",
    "openAi",
    "openMiniAi",
    "openAiWithMockData",
    "openMiniAiWithMockData",
    "openFocusedTextAgentChatWithMockData",
    "openFocusedTextAgentChatFromFocusedFieldWithMockData",
    "openFocusedTextAgentChatWithPiData",
    "showAiCommandBar",
    "simulateAiKey",
    "captureWindow",
    "setAiSearch",
    "setAiInput",
    "setAgentChatInput",
    "setAgentChatScopeInput",
    "selectAgentChatVariation",
    "getAgentChatVariations",
    "agent_chatEscape",
    "setAgentChatTestFixture",
    "setAgentChatTranscriptScroll",
    "getAiWindowState",
    "showGrid",
    "hideGrid",
    "showShortcutRecorder",
    "executeFallback",
    "triggerAction",
    "injectClipboardCaptureFixture",
    "pasteClipboardIntoAgentChat",
    "pushDictationResult",
    "openDictationOverlayFixture",
    "openDictationMicrophonePopupFixture",
    "getConfigFingerprint",
];

/// Accessor used by `tests/mcp_resource_drift.rs` and by the
/// `kit://stdin-commands` MCP resource so both sides read the same list.
pub fn all_external_command_verbs() -> &'static [&'static str] {
    EXTERNAL_COMMAND_VERBS
}
#[derive(Debug, Clone)]
pub enum StdinCommand {
    External(ExternalCommand),
    Protocol(Box<crate::protocol::Message>),
}

impl StdinCommand {
    pub fn command_type(&self) -> &'static str {
        match self {
            Self::External(command) => command.command_type(),
            Self::Protocol(message) => match message.as_ref() {
                crate::protocol::Message::GetState { .. } => "getState",
                crate::protocol::Message::GetElements { .. } => "getElements",
                crate::protocol::Message::GetAgentChatState { .. } => "getAgentChatState",
                crate::protocol::Message::GetAiReliabilityState { .. } => "getAiReliabilityState",
                crate::protocol::Message::SetAiReliabilityTestFixture { .. } => {
                    "setAiReliabilityTestFixture"
                }
                crate::protocol::Message::PerformAgentChatSetupAction { .. } => {
                    "performAgentChatSetupAction"
                }
                crate::protocol::Message::ResetAgentChatTestProbe { .. } => {
                    "resetAgentChatTestProbe"
                }
                crate::protocol::Message::GetAgentChatTestProbe { .. } => "getAgentChatTestProbe",
                crate::protocol::Message::GetLayoutInfo { .. } => "getLayoutInfo",
                crate::protocol::Message::InspectAutomationWindow { .. } => {
                    "inspectAutomationWindow"
                }
                crate::protocol::Message::WaitFor { .. } => "waitFor",
                crate::protocol::Message::Batch { .. } => "batch",
                crate::protocol::Message::ListAutomationWindows { .. } => "listAutomationWindows",
                crate::protocol::Message::SimulateGpuiEvent { .. } => "simulateGpuiEvent",
                _ => "protocol",
            },
        }
    }

    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::External(command) => command.request_id().map(AsRef::as_ref),
            Self::Protocol(message) => match message.as_ref() {
                crate::protocol::Message::GetState { request_id, .. }
                | crate::protocol::Message::GetElements { request_id, .. }
                | crate::protocol::Message::GetAgentChatState { request_id, .. }
                | crate::protocol::Message::GetAiReliabilityState { request_id, .. }
                | crate::protocol::Message::SetAiReliabilityTestFixture { request_id, .. }
                | crate::protocol::Message::PerformAgentChatSetupAction { request_id, .. }
                | crate::protocol::Message::ResetAgentChatTestProbe { request_id, .. }
                | crate::protocol::Message::GetAgentChatTestProbe { request_id, .. }
                | crate::protocol::Message::GetLayoutInfo { request_id, .. }
                | crate::protocol::Message::InspectAutomationWindow { request_id, .. }
                | crate::protocol::Message::WaitFor { request_id, .. }
                | crate::protocol::Message::Batch { request_id, .. }
                | crate::protocol::Message::ListAutomationWindows { request_id, .. }
                | crate::protocol::Message::SimulateGpuiEvent { request_id, .. } => {
                    Some(request_id.as_str())
                }
                _ => message.id(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct StdinCommandEnvelope {
    pub command: StdinCommand,
    pub correlation_id: String,
}

/// Read the optional `protocolVersion` field from a raw command value.
/// Missing values default to core protocol legacy v1. Explicit versions
/// inside the core accepted range dispatch; unsupported out-of-range
/// versions are rejected before typed deserialization.
fn parse_protocol_version(
    raw: &serde_json::Value,
) -> Result<ProtocolVersion, ProtocolVersionError> {
    read_wire_version(raw)
}

fn parse_stdin_command(trimmed: &str) -> anyhow::Result<StdinCommand> {
    // Two-step parse: (1) decode once as Value, (2) version-gate, (3)
    // re-deserialize into the typed enum. Avoids an extra allocation
    // of the whole JSON tree for the common v1 path while giving us
    // one clear place to reject unknown protocol versions.
    let mut raw: serde_json::Value = serde_json::from_str(trimmed)?;
    let version = parse_protocol_version(&raw).inspect_err(|err| {
        if let ProtocolVersionError::Unsupported { found } = err {
            let total = crate::protocol_stats::increment(
                &crate::protocol_stats::PROTOCOL_STATS.stdin_unsupported_protocol_version_total,
            );
            if crate::protocol_stats::should_log_occurrence(total) {
                tracing::warn!(
                    category = "STDIN",
                    event_type = "stdin_unsupported_protocol_version",
                    found_version = found,
                    occurrences_total = total,
                    "rejected stdin command with unsupported protocolVersion"
                );
            }
        }
    })?;

    // Strip the version key so the `deny_unknown_fields` typed decoders
    // do not choke on the framing envelope. The version is already
    // validated above; it has no semantic payload for the typed layer.
    if let Some(obj) = raw.as_object_mut() {
        obj.remove("protocolVersion");
    }

    // Capture the ExternalCommand error (instead of discarding via `if let Ok`)
    // so we can surface it with full field-level detail when the caller's `type`
    // names a known automation verb. Without this, a typo like
    // `{"type":"setFilter","value":"foo"}` (wrong field name — setFilter uses
    // `text`) would fall through to the Message fallback below, whose
    // "unknown variant `setFilter`, expected one of `hello`, `arg`, `submit`,
    // …" error mentions the SDK prompt-response enum instead of the real
    // payload shape problem. See Pass #8 of Run 8 AFK audit
    // (`audits/afk/log.md` entry `stdin-parse-externalcommand-field-error-masked-by-sdk-variant-list`).
    let ext_err = match serde_json::from_value::<ExternalCommand>(raw.clone()) {
        Ok(command) => {
            tracing::debug!(
                category = "STDIN",
                event_type = "stdin_protocol_version_checked",
                protocol_version = version.get(),
                command_type = command.command_type(),
                "Parsed external command"
            );
            return Ok(StdinCommand::External(command));
        }
        Err(err) => err,
    };

    if let Some(known_verb) = raw
        .get("type")
        .and_then(serde_json::Value::as_str)
        .filter(|t| EXTERNAL_COMMAND_VERBS.contains(t))
    {
        return Err(anyhow::anyhow!(
            "automation_payload_mismatch: \"{known_verb}\" is a known automation verb but the payload did not validate: {ext_err}"
        ));
    }

    let message = serde_json::from_value::<crate::protocol::Message>(raw)?;
    tracing::debug!(
        category = "STDIN",
        event_type = "stdin_protocol_version_checked",
        protocol_version = version.get(),
        "Parsed protocol message"
    );
    Ok(StdinCommand::Protocol(Box::new(message)))
}

pub fn create_stdout_response_sender() -> mpsc::SyncSender<crate::protocol::Message> {
    let (response_tx, response_rx) = mpsc::sync_channel::<crate::protocol::Message>(100);

    std::thread::spawn(move || {
        use std::io::Write;

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();

        while let Ok(response) = response_rx.recv() {
            let json: String = match protocol::serialize_message(&response) {
                Ok(json) => json,
                Err(error) => {
                    tracing::warn!(
                        category = "STDIN",
                        error = %error,
                        "Failed to serialize stdin protocol response"
                    );
                    continue;
                }
            };

            logging::log_protocol_send(1, &json);
            crate::agentic_protocol_bus::append_from_json_line(&json);

            if let Err(error) = writeln!(handle, "{}", json) {
                tracing::warn!(
                    category = "STDIN",
                    error = %error,
                    "Failed to write stdin protocol response to stdout"
                );
                break;
            }

            if let Err(error) = handle.flush() {
                tracing::warn!(
                    category = "STDIN",
                    error = %error,
                    "Failed to flush stdin protocol response stdout"
                );
                break;
            }
        }
    });

    response_tx
}
// --- merged from part_001.rs ---

/// Lenient pre-deserialization `requestId` extract used to scope
/// correlation IDs on parse failures. Mirrors the sed pattern used by
/// `scripts/agentic/session.sh` cmd_send:
///     sed -nE 's/.*"requestId"[[:space:]]*:[[:space:]]*"([^"]*)".*/\1/p'
/// + the conservative charset regex `^[A-Za-z0-9_.:/-]+$` that defends
///   the log-tail grep from attacker-controlled metachars.
///
/// Returns `Some(id)` only when the JSON contains a `"requestId":"..."`
/// pair whose value is non-empty and matches the charset; otherwise
/// `None` so callers fall back to a synthetic UUID.
///
/// Used by the `Err(_)` arm in [`start_stdin_listener`] so that
/// `stdin_parse_failed` events carry `correlation_id = "stdin:req:<id>"`
/// and can be correlated by the shell-side receipt scope exactly like
/// successful parses — closes the concurrent sad-path cross-correlation
/// anomaly filed by Run 5 Pass #12 (phase C: 5 parallel malformed sends
/// with distinct requestIds all reported the same error text).
pub(crate) fn extract_request_id_lenient(line: &str) -> Option<String> {
    let marker = "\"requestId\"";
    let start = line.find(marker)?;
    let after_key = &line[start + marker.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let inner = after_colon.strip_prefix('"')?;
    let end = inner.find('"')?;
    let id = &inner[..end];
    if id.is_empty() {
        return None;
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '/'))
    {
        return None;
    }
    Some(id.to_string())
}

fn extract_command_type_lenient(line: &str) -> Option<String> {
    let marker = "\"type\"";
    let start = line.find(marker)?;
    let after_key = &line[start + marker.len()..];
    let colon = after_key.find(':')?;
    let inner = after_key[colon + 1..].trim_start().strip_prefix('"')?;
    let end = inner.find('"')?;
    let command = &inner[..end];
    (!command.is_empty()).then(|| command.to_string())
}

fn oversized_request_error(raw: &str, raw_len: usize) -> Option<crate::protocol::Message> {
    let request_id = extract_request_id_lenient(raw)?;
    let command = extract_command_type_lenient(raw).unwrap_or_else(|| "protocol".to_string());
    Some(crate::protocol::Message::external_command_result(
        request_id,
        command,
        false,
        Some("line_too_long".to_string()),
        Some(format!(
            "stdin JSONL line exceeds {MAX_STDIN_COMMAND_BYTES} bytes (received {raw_len} bytes; retained {} bytes)",
            raw.len()
        )),
    ))
}

/// Start a thread that listens on stdin for external JSONL commands.
/// Returns an async_channel::Receiver that can be awaited without polling.
///
/// # Channel Capacity
///
/// Uses a bounded channel with capacity of 100 to prevent unbounded memory growth.
/// This is generous for stdin commands which typically arrive at < 10/sec.
///
/// # Thread Safety
///
/// Spawns a background thread that reads stdin line-by-line. When the channel
/// is closed (receiver dropped), the thread will exit gracefully.
#[tracing::instrument(skip_all)]
pub fn start_stdin_listener(
    response_sender: Option<mpsc::SyncSender<crate::protocol::Message>>,
) -> async_channel::Receiver<StdinCommandEnvelope> {
    // P1-6: Use bounded channel to prevent unbounded memory growth
    // Capacity of 100 is generous for stdin commands (typically < 10/sec)
    let (tx, rx) = async_channel::bounded(100);

    std::thread::spawn(move || {
        let listener_correlation_id = format!("stdin:listener:{}", Uuid::new_v4());
        let _listener_guard = logging::set_correlation_id(listener_correlation_id.clone());
        tracing::info!(
            category = "STDIN",
            event_type = "stdin_listener_started",
            correlation_id = %listener_correlation_id,
            "External command listener started"
        );

        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        let mut byte_buffer = Vec::with_capacity(1024);

        loop {
            match read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES) {
                Ok(StdinLineRead::Eof) => break,
                Ok(StdinLineRead::Line(line)) => {
                    let trimmed = line.trim_end_matches(['\r', '\n']);
                    if trimmed.trim().is_empty() {
                        continue;
                    }

                    let summary = logging::summarize_payload(trimmed);
                    match parse_stdin_command(trimmed) {
                        Ok(cmd) => {
                            let correlation_id = cmd
                                .request_id()
                                .filter(|id| !id.trim().is_empty())
                                .map(|id| format!("stdin:req:{}", id))
                                .unwrap_or_else(|| format!("stdin:{}", Uuid::new_v4()));
                            let _guard = logging::set_correlation_id(correlation_id.clone());

                            tracing::info!(
                                category = "STDIN",
                                event_type = "stdin_command_parsed",
                                command_type = cmd.command_type(),
                                line_len = trimmed.len(),
                                payload_summary = %summary,
                                correlation_id = %correlation_id,
                                "Parsed external command"
                            );

                            // send_blocking is used since we're in a sync thread
                            if tx
                                .send_blocking(StdinCommandEnvelope {
                                    command: cmd,
                                    correlation_id: correlation_id.clone(),
                                })
                                .is_err()
                            {
                                tracing::warn!(
                                    category = "STDIN",
                                    event_type = "stdin_channel_closed",
                                    correlation_id = %correlation_id,
                                    "Command channel closed, exiting"
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            // Scope correlation_id on the requestId when the
                            // malformed payload still carries one in a
                            // structurally-valid `"requestId":"..."` pair —
                            // symmetric with the happy-path correlation above
                            // so concurrent parse-failed events can be
                            // de-interleaved by the shell-side receipt scope
                            // (`scripts/agentic/session.sh` cmd_send uses
                            // `grep -F -- "cid=stdin:req:${req_id} "`).
                            // Fallback to a synthetic UUID when no valid
                            // requestId can be lenient-extracted — preserves
                            // the offset-first single-caller precondition.
                            let correlation_id = extract_request_id_lenient(trimmed)
                                .map(|id| format!("stdin:req:{}", id))
                                .unwrap_or_else(|| format!("stdin:parse:{}", Uuid::new_v4()));
                            let _guard = logging::set_correlation_id(correlation_id.clone());
                            tracing::warn!(
                                category = "STDIN",
                                event_type = "stdin_parse_failed",
                                line_len = trimmed.len(),
                                payload_summary = %summary,
                                error = %e,
                                correlation_id = %correlation_id,
                                "Failed to parse external command"
                            );
                            if let (Some(sender), Some(response)) = (
                                response_sender.as_ref(),
                                crate::protocol::malformed_request_error(trimmed, &e),
                            ) {
                                if sender.try_send(response).is_err() {
                                    tracing::warn!(
                                        category = "STDIN",
                                        event_type = "stdin_parse_error_response_dropped",
                                        correlation_id = %correlation_id,
                                        "Failed to enqueue malformed-payload response"
                                    );
                                }
                            }
                        }
                    }
                }
                Ok(StdinLineRead::TooLong { raw, raw_len }) => {
                    let request_id = extract_request_id_lenient(&raw);
                    let correlation_id = request_id
                        .as_deref()
                        .map(|id| format!("stdin:req:{id}"))
                        .unwrap_or_else(|| format!("stdin:oversize:{}", Uuid::new_v4()));
                    let _guard = logging::set_correlation_id(correlation_id.clone());
                    let summary = logging::summarize_payload(&raw);
                    tracing::warn!(
                        category = "STDIN",
                        event_type = "stdin_command_too_large",
                        raw_len = raw_len,
                        max_line_bytes = MAX_STDIN_COMMAND_BYTES,
                        payload_summary = %summary,
                        correlation_id = %correlation_id,
                        "Skipping oversized external command"
                    );
                    if let (Some(sender), Some(response)) = (
                        response_sender.as_ref(),
                        oversized_request_error(&raw, raw_len),
                    ) {
                        if sender.try_send(response).is_err() {
                            tracing::warn!(
                                category = "STDIN",
                                event_type = "stdin_line_too_long_response_dropped",
                                correlation_id = %correlation_id,
                                "Failed to enqueue oversized-line response"
                            );
                        }
                    }
                }
                Err(e) => {
                    let correlation_id = format!("stdin:read:{}", Uuid::new_v4());
                    let _guard = logging::set_correlation_id(correlation_id.clone());
                    tracing::error!(
                        category = "STDIN",
                        event_type = "stdin_read_error",
                        error = %e,
                        correlation_id = %correlation_id,
                        "Error reading stdin"
                    );
                    break;
                }
            }
        }
        tracing::info!(
            category = "STDIN",
            event_type = "stdin_listener_exiting",
            "External command listener exiting"
        );
    });

    rx
}
// --- merged from part_002.rs ---
// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use std::collections::BTreeSet;
    use std::io::Cursor;
    use std::path::Path;
    use tempfile::TempDir;

    static PROTOCOL_VERSION_STATS_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Pins `EXTERNAL_COMMAND_VERBS` to the exhaustive
    /// [`ExternalCommand::command_type`] match: every sample variant's verb
    /// MUST appear in the slice. Adding a new variant forces both sides to
    /// grow — the match arm below is exhaustive.
    #[test]
    fn external_command_verbs_cover_every_variant() {
        let variants: Vec<ExternalCommand> = vec![
            ExternalCommand::Run {
                path: String::new(),
                request_id: None,
            },
            ExternalCommand::Show { request_id: None },
            ExternalCommand::Hide { request_id: None },
            ExternalCommand::SetFilter {
                text: String::new(),
                request_id: None,
            },
            ExternalCommand::SetMenuSyntaxFormField {
                field: None,
                value: String::new(),
                request_id: None,
            },
            ExternalCommand::TriggerBuiltin {
                builtin_id: None,
                name: None,
                request_id: None,
            },
            ExternalCommand::SimulateKey {
                key: String::new(),
                modifiers: Vec::new(),
                target: None,
                request_id: None,
            },
            ExternalCommand::OpenNotes,
            ExternalCommand::OpenAbout,
            ExternalCommand::OpenCreationFeedback {
                path: None,
                receipt_path: None,
                receipt_status: None,
                verification_status: None,
                request_id: None,
            },
            ExternalCommand::OpenConfirmPrompt {
                title: None,
                body: None,
                confirm_text: None,
                cancel_text: None,
                request_id: None,
            },
            ExternalCommand::OpenAgentChatDetachedFixture { request_id: None },
            ExternalCommand::OpenAgentChatHistoryPopupFixture { request_id: None },
            ExternalCommand::OpenChatPromptFixture { request_id: None },
            ExternalCommand::ClosePromptPopupNatively {
                target: crate::protocol::AutomationWindowTarget::Instance {
                    id: "fixture-popup".to_string(),
                    generation: 1,
                },
                request_id: None,
            },
            ExternalCommand::OpenAi,
            ExternalCommand::OpenMiniAi,
            ExternalCommand::OpenAiWithMockData,
            ExternalCommand::OpenMiniAiWithMockData,
            ExternalCommand::OpenFocusedTextAgentChatWithMockData {
                text: None,
                instruction: None,
                request_id: None,
            },
            ExternalCommand::OpenFocusedTextAgentChatFromFocusedFieldWithMockData {
                instruction: None,
                request_id: None,
            },
            ExternalCommand::OpenFocusedTextAgentChatWithPiData {
                text: None,
                instruction: None,
                request_id: None,
            },
            ExternalCommand::ShowAiCommandBar,
            ExternalCommand::SimulateAiKey {
                key: String::new(),
                modifiers: Vec::new(),
                request_id: None,
            },
            ExternalCommand::CaptureWindow {
                title: String::new(),
                path: String::new(),
                request_id: None,
            },
            ExternalCommand::SetAiSearch {
                text: String::new(),
                request_id: None,
            },
            ExternalCommand::SetAiInput {
                text: String::new(),
                submit: false,
                request_id: None,
            },
            ExternalCommand::SetAgentChatInput {
                text: String::new(),
                submit: false,
                request_id: None,
            },
            ExternalCommand::SetAgentChatScopeInput {
                text: String::new(),
                request_id: None,
            },
            ExternalCommand::SelectAgentChatVariation {
                index: 0,
                edit: false,
                request_id: None,
            },
            ExternalCommand::GetAgentChatVariations { request_id: None },
            ExternalCommand::AgentChatEscape { request_id: None },
            ExternalCommand::SetAgentChatTestFixture {
                phase: "awaitingFirstAssistantText".to_string(),
                user_text: None,
                assistant_text: None,
                message_count: None,
                request_id: None,
            },
            ExternalCommand::GetAiWindowState { request_id: None },
            ExternalCommand::ShowGrid {
                grid_size: 8,
                show_bounds: false,
                show_box_model: false,
                show_alignment_guides: false,
                show_dimensions: false,
                depth: GridDepthOption::default(),
                request_id: None,
            },
            ExternalCommand::HideGrid,
            ExternalCommand::ShowShortcutRecorder {
                command_id: String::new(),
                command_name: String::new(),
                request_id: None,
            },
            ExternalCommand::ExecuteFallback {
                fallback_id: String::new(),
                input: String::new(),
                request_id: None,
            },
            ExternalCommand::TriggerAction {
                action_id: String::new(),
                host: None,
                request_id: None,
            },
            ExternalCommand::InjectClipboardCaptureFixture {
                payload: crate::clipboard_history::ClipboardCaptureFixturePayload::Text {
                    text: String::new(),
                },
                source_bundle_id: None,
                concealed_types: Vec::new(),
                change_generation: 0,
                request_id: None,
            },
            ExternalCommand::PasteClipboardIntoAgentChat { request_id: None },
            ExternalCommand::PushDictationResult {
                transcript: String::new(),
                partial_transcript: None,
                target: None,
                request_id: None,
            },
            ExternalCommand::OpenDictationOverlayFixture { request_id: None },
            ExternalCommand::OpenDictationMicrophonePopupFixture { request_id: None },
            ExternalCommand::GetConfigFingerprint { request_id: None },
            ExternalCommand::SimulateMainHotkeyGesture {
                phase: String::new(),
                request_id: None,
            },
            ExternalCommand::SetAgentChatTranscriptScroll {
                item_ix: 0,
                offset_px: 0.0,
                request_id: None,
            },
        ];

        let declared: BTreeSet<&str> = EXTERNAL_COMMAND_VERBS.iter().copied().collect();
        for variant in &variants {
            let verb = variant.command_type();
            assert!(
                declared.contains(verb),
                "verb {verb:?} produced by an ExternalCommand variant is not in EXTERNAL_COMMAND_VERBS"
            );
        }

        assert_eq!(
            declared.len(),
            EXTERNAL_COMMAND_VERBS.len(),
            "EXTERNAL_COMMAND_VERBS must be de-duplicated"
        );
        assert_eq!(
            declared.len(),
            variants.len(),
            "sample list in this test must cover every ExternalCommand verb"
        );
    }

    #[test]
    fn inject_clipboard_capture_fixture_deserializes_redacted_metadata_and_file_payload() {
        let command: ExternalCommand = serde_json::from_str(
            r#"{"type":"injectClipboardCaptureFixture","payload":{"type":"textFile","path":"/tmp/sandbox/.scriptkit/devtools-fixtures/oversize.txt"},"sourceBundleId":"com.apple.TextEdit","concealedTypes":["org.nspasteboard.ConcealedType"],"changeGeneration":42,"requestId":"clip-fixture-1"}"#,
        )
        .expect("clipboard fixture command parses");

        match command {
            ExternalCommand::InjectClipboardCaptureFixture {
                payload: crate::clipboard_history::ClipboardCaptureFixturePayload::TextFile { path },
                source_bundle_id,
                concealed_types,
                change_generation,
                request_id,
            } => {
                assert!(path.ends_with("oversize.txt"));
                assert_eq!(source_bundle_id.as_deref(), Some("com.apple.TextEdit"));
                assert_eq!(concealed_types, ["org.nspasteboard.ConcealedType"]);
                assert_eq!(change_generation, 42);
                assert_eq!(request_id.as_deref(), Some("clip-fixture-1"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn test_read_stdin_line_bounded_skips_oversized_line_and_recovers() -> anyhow::Result<()> {
        let oversized_payload = "x".repeat(20_000);
        let input = format!(
            r#"{{"type":"setFilter","text":"{}"}}
{{"type":"show"}}
"#,
            oversized_payload
        );

        let mut reader = Cursor::new(input);
        let mut byte_buffer = Vec::new();

        let first = read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)
            .context("Expected bounded line reader to process input")?;
        match first {
            StdinLineRead::TooLong { raw_len, .. } => {
                assert!(raw_len > MAX_STDIN_COMMAND_BYTES);
            }
            _ => panic!("Expected first line to be marked too long"),
        }

        let second =
            read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)
                .context("Expected second line to be readable")?;
        match second {
            StdinLineRead::Line(line) => {
                assert_eq!(line.trim_end(), r#"{"type":"show"}"#);
            }
            _ => panic!("Expected second line to be a valid command"),
        }

        Ok(())
    }

    #[test]
    fn oversized_line_with_early_request_id_builds_typed_error() -> anyhow::Result<()> {
        let input = format!(
            "{{\"requestId\":\"of19-early\",\"type\":\"setInput\",\"text\":\"{}\"}}\n",
            "x".repeat(20_000)
        );
        let mut reader = Cursor::new(input);
        let mut byte_buffer = Vec::new();
        let read = read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)?;
        let StdinLineRead::TooLong { raw, raw_len } = read else {
            panic!("expected oversized line");
        };
        assert_eq!(raw.len(), MAX_STDIN_COMMAND_BYTES);

        let response = oversized_request_error(&raw, raw_len)
            .expect("early requestId must survive in the retained prefix");
        match response {
            crate::protocol::Message::ExternalCommandResult {
                request_id,
                command,
                ok,
                error_code,
                error_message,
            } => {
                assert_eq!(request_id, "of19-early");
                assert_eq!(command, "setInput");
                assert!(!ok);
                assert_eq!(error_code.as_deref(), Some("line_too_long"));
                let message = error_message.expect("length diagnostic");
                assert!(message.contains("16384"));
                assert!(message.contains(&raw_len.to_string()));
            }
            other => panic!("expected externalCommandResult, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn oversized_line_with_late_request_id_remains_log_only() -> anyhow::Result<()> {
        let input = format!(
            "{{\"type\":\"setInput\",\"text\":\"{}\",\"requestId\":\"of19-late\"}}\n",
            "x".repeat(20_000)
        );
        let mut reader = Cursor::new(input);
        let mut byte_buffer = Vec::new();
        let read = read_stdin_line_bounded(&mut reader, &mut byte_buffer, MAX_STDIN_COMMAND_BYTES)?;
        let StdinLineRead::TooLong { raw, raw_len } = read else {
            panic!("expected oversized line");
        };
        assert!(!raw.contains("of19-late"));
        assert!(oversized_request_error(&raw, raw_len).is_none());
        Ok(())
    }

    #[test]
    fn test_external_command_run_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "run", "path": "/path/to/script.ts"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::Run { path, request_id } => {
                assert_eq!(path, "/path/to/script.ts");
                assert!(request_id.is_none());
            }
            _ => panic!("Expected Run command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_run_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "run", "path": "/path/to/script.ts", "requestId": "req-123"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::Run { path, request_id } => {
                assert_eq!(path, "/path/to/script.ts");
                assert_eq!(request_id, Some("req-123".to_string().into()));
            }
            _ => panic!("Expected Run command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "show"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::Show { request_id: None }));
        Ok(())
    }

    #[test]
    fn test_external_command_show_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "show", "requestId": "req-456"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::Show { request_id } => {
                assert_eq!(request_id, Some("req-456".to_string().into()));
            }
            _ => panic!("Expected Show command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_hide_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "hide"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::Hide { request_id: None }));
        Ok(())
    }

    #[test]
    fn test_external_command_set_filter_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "setFilter", "text": "hello world"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SetFilter { text, request_id } => {
                assert_eq!(text, "hello world");
                assert!(request_id.is_none());
            }
            _ => panic!("Expected SetFilter command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_set_filter_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "setFilter", "text": "hello", "requestId": "req-789"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SetFilter { text, request_id } => {
                assert_eq!(text, "hello");
                assert_eq!(request_id, Some("req-789".to_string().into()));
            }
            _ => panic!("Expected SetFilter command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_trigger_builtin_deserialization() -> anyhow::Result<()> {
        // Deprecated `name` path still parses in v1 so the pre-v1 Bun
        // SDK keeps working until callers migrate to `builtinId`.
        let json = r#"{"type": "triggerBuiltin", "name": "clipboardHistory"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match &cmd {
            ExternalCommand::TriggerBuiltin {
                name: Some(n),
                builtin_id: None,
                ..
            } => assert_eq!(n, "clipboardHistory"),
            _ => panic!("Expected TriggerBuiltin with deprecated `name` only"),
        }
        assert_eq!(
            cmd.trigger_builtin_ref().unwrap(),
            Some(BuiltinRef::LegacyAlias("clipboardHistory"))
        );
        Ok(())
    }

    #[test]
    fn trigger_builtin_prefers_canonical_builtin_id() -> anyhow::Result<()> {
        let json = r#"{"type":"triggerBuiltin","builtinId":"builtin/clipboard-history"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(
            cmd.trigger_builtin_ref().unwrap(),
            Some(BuiltinRef::CanonicalId("builtin/clipboard-history"))
        );
        Ok(())
    }

    #[test]
    fn trigger_builtin_rejects_both_fields() {
        let json = r#"{"type":"triggerBuiltin","builtinId":"builtin/clipboard-history","name":"clipboard"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        let err = cmd.trigger_builtin_ref().unwrap_err();
        assert!(
            err.contains("either `builtinId` or deprecated `name`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn trigger_builtin_rejects_neither_field() {
        let json = r#"{"type":"triggerBuiltin"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json).unwrap();
        let err = cmd.trigger_builtin_ref().unwrap_err();
        assert!(
            err.contains("requires `builtinId`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_stdin_command_defaults_missing_protocol_version_to_v1() -> anyhow::Result<()> {
        // No `protocolVersion` field → treated as v1. Preserves
        // compatibility with the pre-v1 Bun SDK.
        let parsed = parse_stdin_command(r#"{"type":"show"}"#)?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::Show { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v1_protocol_version() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(r#"{"type":"show","protocolVersion":1}"#)?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::Show { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v2_external_command_protocol_version() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(r#"{"type":"show","protocolVersion":2}"#)?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::Show { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v2_protocol_message() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"getState","requestId":"state-v2","protocolVersion":2}"#,
        )?;
        assert!(matches!(
            parsed,
            StdinCommand::Protocol(message)
                if matches!(*message, crate::protocol::Message::GetState { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_accepts_v2_trigger_builtin() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"triggerBuiltin","builtinId":"builtin/clipboard-history","protocolVersion":2}"#,
        )?;
        assert!(matches!(
            parsed,
            StdinCommand::External(ExternalCommand::TriggerBuiltin { .. })
        ));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_rejects_unsupported_protocol_version_and_counts_it() {
        let _guard = PROTOCOL_VERSION_STATS_TEST_LOCK.lock().unwrap();
        crate::protocol_stats::reset_for_test();

        let err = parse_stdin_command(r#"{"type":"show","protocolVersion":999}"#)
            .expect_err("future version must be rejected");
        assert!(
            err.to_string().contains("unsupported protocolVersion"),
            "unexpected error: {err}"
        );
        assert_eq!(
            crate::protocol_stats::PROTOCOL_STATS
                .stdin_unsupported_protocol_version_total
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn parse_stdin_command_rejects_non_integer_protocol_version_without_unsupported_count() {
        let _guard = PROTOCOL_VERSION_STATS_TEST_LOCK.lock().unwrap();
        crate::protocol_stats::reset_for_test();

        let err = parse_stdin_command(r#"{"type":"show","protocolVersion":"one"}"#)
            .expect_err("non-integer protocolVersion must be rejected");
        assert!(
            err.to_string().contains("not an unsigned integer"),
            "unexpected error: {err}"
        );
        assert_eq!(
            crate::protocol_stats::PROTOCOL_STATS
                .stdin_unsupported_protocol_version_total
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn test_external_command_simulate_key_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "simulateKey", "key": "enter", "modifiers": ["cmd", "shift"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey {
                key,
                modifiers,
                target,
                ..
            } => {
                assert_eq!(key, "enter");
                assert_eq!(modifiers, vec![KeyModifier::Cmd, KeyModifier::Shift]);
                assert!(target.is_none());
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_target_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type":"simulateKey","target":{"type":"kind","kind":"notes"},"key":"p","modifiers":["cmd","shift"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey {
                key,
                modifiers,
                target,
                ..
            } => {
                assert_eq!(key, "p");
                assert_eq!(modifiers, vec![KeyModifier::Cmd, KeyModifier::Shift]);
                match target {
                    Some(protocol::AutomationWindowTarget::Kind { kind, index }) => {
                        assert_eq!(kind, protocol::AutomationWindowKind::Notes);
                        assert_eq!(index, None);
                    }
                    other => panic!("Expected targeted Notes simulateKey, got {other:?}"),
                }
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_no_modifiers() -> anyhow::Result<()> {
        let json = r#"{"type": "simulateKey", "key": "escape"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey { key, modifiers, .. } => {
                assert_eq!(key, "escape");
                assert!(modifiers.is_empty());
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_modifier_aliases() -> anyhow::Result<()> {
        let json = r#"{"type":"simulateKey","key":"k","modifiers":["meta","option","control"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::SimulateKey { modifiers, .. } => {
                assert_eq!(
                    modifiers,
                    vec![KeyModifier::Cmd, KeyModifier::Alt, KeyModifier::Ctrl]
                );
            }
            _ => panic!("Expected SimulateKey command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_simulate_key_unknown_modifier_rejected() {
        let json = r#"{"type":"simulateKey","key":"enter","modifiers":["capslock"]}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_invalid_json_fails() {
        let json = r#"{"type": "unknown"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_rejects_unknown_fields() {
        let json = r#"{"type":"show","unexpected":"field"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_missing_required_field_fails() {
        // Run command requires path field
        let json = r#"{"type": "run"}"#;
        let result = serde_json::from_str::<ExternalCommand>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_external_command_clone() {
        let cmd = ExternalCommand::Run {
            path: "/test".to_string(),
            request_id: None,
        };
        let cloned = cmd.clone();
        match cloned {
            ExternalCommand::Run { path, .. } => assert_eq!(path, "/test"),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_external_command_debug() {
        let cmd = ExternalCommand::Show { request_id: None };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Show"));
    }

    #[test]
    fn test_external_command_request_id_accessor() {
        let cmd = ExternalCommand::SetFilter {
            text: "hello".to_string(),
            request_id: Some("req-42".to_string().into()),
        };
        assert_eq!(cmd.request_id(), Some("req-42"));
    }

    #[test]
    fn test_external_command_type_accessor() {
        let cmd = ExternalCommand::Show { request_id: None };
        assert_eq!(cmd.command_type(), "show");
    }

    #[test]
    fn test_parse_stdin_command_supports_external_commands() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(r#"{"type":"show","requestId":"show-1"}"#)?;
        assert_eq!(parsed.command_type(), "show");
        assert_eq!(parsed.request_id(), Some("show-1"));
        assert!(matches!(parsed, StdinCommand::External(_)));
        Ok(())
    }

    #[test]
    fn test_parse_stdin_command_supports_protocol_messages() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"waitFor","requestId":"wf-1","condition":"choicesRendered"}"#,
        )?;
        assert_eq!(parsed.command_type(), "waitFor");
        assert_eq!(parsed.request_id(), Some("wf-1"));
        assert!(matches!(parsed, StdinCommand::Protocol(_)));
        Ok(())
    }

    #[test]
    fn parse_stdin_command_supports_computer_see_protocol_message() -> anyhow::Result<()> {
        let parsed = parse_stdin_command(
            r#"{"type":"inspectAutomationWindow","requestId":"cu-see-1","target":{"type":"focused"},"hiDpi":false,"probes":[{"x":10,"y":20}]}"#,
        )?;

        assert_eq!(parsed.command_type(), "inspectAutomationWindow");
        assert_eq!(parsed.request_id(), Some("cu-see-1"));

        match parsed {
            StdinCommand::Protocol(message) => match message.as_ref() {
                crate::protocol::Message::InspectAutomationWindow {
                    request_id,
                    target,
                    hi_dpi,
                    probes,
                } => {
                    assert_eq!(request_id, "cu-see-1");
                    assert_eq!(
                        target,
                        &Some(crate::protocol::AutomationWindowTarget::Focused)
                    );
                    assert_eq!(hi_dpi, &Some(false));
                    assert_eq!(probes, &vec![crate::protocol::PixelProbe { x: 10, y: 20 }]);
                }
                other => panic!("expected InspectAutomationWindow, got {other:?}"),
            },
            other => panic!("expected protocol message, got {other:?}"),
        }

        Ok(())
    }

    #[test]
    fn parse_stdin_command_surfaces_external_command_error_for_known_verb_with_wrong_field() {
        let err = parse_stdin_command(r#"{"type":"setFilter","value":"foo"}"#)
            .expect_err("wrong field name should fail parse");
        let display = format!("{err:#}");
        assert!(
            display.contains("automation_payload_mismatch"),
            "expected context to tag the error as automation_payload_mismatch; got: {display}"
        );
        assert!(
            display.contains("\"setFilter\""),
            "expected context to name the attempted verb; got: {display}"
        );
        assert!(
            !display.contains("unknown variant `setFilter`"),
            "must NOT fall back to SDK Message error text; got: {display}"
        );
    }

    #[test]
    fn parse_stdin_command_surfaces_external_command_error_for_missing_required_field() {
        let err = parse_stdin_command(r#"{"type":"setFilter"}"#)
            .expect_err("missing required field should fail parse");
        let display = format!("{err:#}");
        assert!(
            display.contains("automation_payload_mismatch"),
            "expected automation_payload_mismatch context; got: {display}"
        );
    }

    #[test]
    fn parse_stdin_command_unknown_type_still_uses_sdk_message_fallback() {
        let err = parse_stdin_command(r#"{"type":"totallyFakeVerbXyz","foo":"bar"}"#)
            .expect_err("unknown verb should fail parse");
        let display = format!("{err:#}");
        assert!(
            !display.contains("automation_payload_mismatch"),
            "truly unknown verbs must NOT be tagged as automation_payload_mismatch; got: {display}"
        );
        assert!(
            display.contains("unknown variant"),
            "unknown verbs should surface the Message-enum unknown-variant error; got: {display}"
        );
    }

    #[test]
    fn test_external_command_open_notes_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openNotes"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenNotes));
        Ok(())
    }

    #[test]
    fn test_external_command_open_about_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openAbout"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenAbout));
        Ok(())
    }

    #[test]
    fn test_external_command_open_creation_feedback_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openCreationFeedback", "path": "/tmp/fixture.ts"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenCreationFeedback {
                path,
                receipt_path,
                receipt_status,
                verification_status,
                ..
            } => {
                assert_eq!(path.as_deref(), Some("/tmp/fixture.ts"));
                assert!(receipt_path.is_none());
                assert!(receipt_status.is_none());
                assert!(verification_status.is_none());
            }
            other => panic!("Expected OpenCreationFeedback, got: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_creation_feedback_receipt_fixture_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type": "openCreationFeedback", "path": "/tmp/fixture.ts", "receiptPath": "/tmp/fixture.scriptkit.json", "receiptStatus": "present", "verificationStatus": "blocked"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenCreationFeedback {
                path,
                receipt_path,
                receipt_status,
                verification_status,
                ..
            } => {
                assert_eq!(path.as_deref(), Some("/tmp/fixture.ts"));
                assert_eq!(receipt_path.as_deref(), Some("/tmp/fixture.scriptkit.json"));
                assert_eq!(receipt_status.as_deref(), Some("present"));
                assert_eq!(
                    verification_status,
                    Some(crate::ai::GeneratedScriptVerificationStatus::Blocked)
                );
            }
            other => panic!("Expected OpenCreationFeedback, got: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_confirm_prompt_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openConfirmPrompt", "title": "Delete?", "body": "This cannot be undone.", "confirmText": "Delete", "cancelText": "Keep"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenConfirmPrompt {
                title,
                body,
                confirm_text,
                cancel_text,
                ..
            } => {
                assert_eq!(title.as_deref(), Some("Delete?"));
                assert_eq!(body.as_deref(), Some("This cannot be undone."));
                assert_eq!(confirm_text.as_deref(), Some("Delete"));
                assert_eq!(cancel_text.as_deref(), Some("Keep"));
            }
            other => panic!("Expected OpenConfirmPrompt, got: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_ai_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openAi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenAi));
        Ok(())
    }

    #[test]
    fn test_external_command_open_mini_ai_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openMiniAi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenMiniAi));
        Ok(())
    }

    #[test]
    fn test_external_command_open_ai_with_mock_data_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openAiWithMockData"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenAiWithMockData));
        Ok(())
    }

    #[test]
    fn test_external_command_open_mini_ai_with_mock_data_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "openMiniAiWithMockData"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::OpenMiniAiWithMockData));
        Ok(())
    }

    #[test]
    fn test_external_command_open_focused_text_agent_chat_with_mock_data_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type":"openFocusedTextAgentChatWithMockData","text":"Hello world","instruction":"Translate","requestId":"ft-mock"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenFocusedTextAgentChatWithMockData {
                text,
                instruction,
                request_id,
            } => {
                assert_eq!(text.as_deref(), Some("Hello world"));
                assert_eq!(instruction.as_deref(), Some("Translate"));
                assert_eq!(request_id.as_deref(), Some("ft-mock"));
            }
            other => panic!("Expected OpenFocusedTextAgentChatWithMockData, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_open_focused_text_agent_chat_with_pi_data_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type":"openFocusedTextAgentChatWithPiData","text":"Hello world","instruction":"Translate","requestId":"ft-pi"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenFocusedTextAgentChatWithPiData {
                text,
                instruction,
                request_id,
            } => {
                assert_eq!(text.as_deref(), Some("Hello world"));
                assert_eq!(instruction.as_deref(), Some("Translate"));
                assert_eq!(request_id.as_deref(), Some("ft-pi"));
            }
            other => panic!("Expected OpenFocusedTextAgentChatWithPiData, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_get_ai_window_state_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "getAiWindowState"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(
            cmd,
            ExternalCommand::GetAiWindowState { request_id: None }
        ));
        assert_eq!(cmd.command_type(), "getAiWindowState");
        Ok(())
    }

    #[test]
    fn test_external_command_get_ai_window_state_with_request_id() -> anyhow::Result<()> {
        let json = r#"{"type": "getAiWindowState", "requestId": "req-42"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.request_id(), Some("req-42"));
        Ok(())
    }

    #[test]
    fn test_external_command_set_agent_chat_input_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "setAgentChatInput", "text": "hello world", "submit": true}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.command_type(), "setAgentChatInput");
        match cmd {
            ExternalCommand::SetAgentChatInput {
                text,
                submit,
                request_id,
            } => {
                assert_eq!(text, "hello world");
                assert!(submit);
                assert!(request_id.is_none());
            }
            _ => panic!("Expected SetAgentChatInput command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_set_agent_chat_input_with_request_id() -> anyhow::Result<()> {
        let json =
            r#"{"type": "setAgentChatInput", "text": "hello", "requestId": "req-agent_chat"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.request_id(), Some("req-agent_chat"));
        Ok(())
    }

    #[test]
    fn test_external_command_set_agent_chat_test_fixture_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "setAgentChatTestFixture", "phase": "awaitingFirstAssistantText", "userText": "hello", "requestId": "req-fixture"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert_eq!(cmd.command_type(), "setAgentChatTestFixture");
        assert_eq!(cmd.request_id(), Some("req-fixture"));
        match cmd {
            ExternalCommand::SetAgentChatTestFixture {
                phase,
                user_text,
                assistant_text,
                message_count,
                ..
            } => {
                assert_eq!(phase, "awaitingFirstAssistantText");
                assert_eq!(user_text.as_deref(), Some("hello"));
                assert!(assistant_text.is_none());
                assert_eq!(message_count, None);
            }
            _ => panic!("Expected SetAgentChatTestFixture command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_capture_window_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "captureWindow", "title": "Script Kit Agent Chat", "path": "/tmp/screenshot.png"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::CaptureWindow { title, path, .. } => {
                assert_eq!(title, "Script Kit Agent Chat");
                assert_eq!(path, "/tmp/screenshot.png");
            }
            _ => panic!("Expected CaptureWindow command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_grid_defaults() -> anyhow::Result<()> {
        let json = r#"{"type": "showGrid"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ShowGrid {
                grid_size,
                show_bounds,
                show_box_model,
                show_alignment_guides,
                show_dimensions,
                depth,
                ..
            } => {
                assert_eq!(grid_size, 8); // default
                assert!(!show_bounds); // default false
                assert!(!show_box_model); // default false
                assert!(!show_alignment_guides); // default false
                assert!(!show_dimensions); // default false
                assert!(matches!(depth, GridDepthOption::Preset(_))); // default
            }
            _ => panic!("Expected ShowGrid command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_grid_with_options() -> anyhow::Result<()> {
        let json = r#"{"type": "showGrid", "gridSize": 16, "showBounds": true, "showBoxModel": true, "showAlignmentGuides": true, "showDimensions": true, "depth": "all"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ShowGrid {
                grid_size,
                show_bounds,
                show_box_model,
                show_alignment_guides,
                show_dimensions,
                depth,
                ..
            } => {
                assert_eq!(grid_size, 16);
                assert!(show_bounds);
                assert!(show_box_model);
                assert!(show_alignment_guides);
                assert!(show_dimensions);
                match depth {
                    GridDepthOption::Preset(s) => assert_eq!(s, "all"),
                    _ => panic!("Expected Preset depth"),
                }
            }
            _ => panic!("Expected ShowGrid command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_show_grid_with_components() -> anyhow::Result<()> {
        let json = r#"{"type": "showGrid", "depth": ["header", "footer"]}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ShowGrid { depth, .. } => match depth {
                GridDepthOption::Components(components) => {
                    assert_eq!(components, vec!["header", "footer"]);
                }
                _ => panic!("Expected Components depth"),
            },
            _ => panic!("Expected ShowGrid command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_hide_grid_deserialization() -> anyhow::Result<()> {
        let json = r#"{"type": "hideGrid"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        assert!(matches!(cmd, ExternalCommand::HideGrid));
        Ok(())
    }

    #[test]
    fn test_external_command_execute_fallback_deserialization() -> anyhow::Result<()> {
        let json =
            r#"{"type": "executeFallback", "fallbackId": "search-google", "input": "hello world"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ExecuteFallback {
                fallback_id, input, ..
            } => {
                assert_eq!(fallback_id, "search-google");
                assert_eq!(input, "hello world");
            }
            _ => panic!("Expected ExecuteFallback command"),
        }
        Ok(())
    }

    #[test]
    fn test_external_command_execute_fallback_copy() -> anyhow::Result<()> {
        let json = r#"{"type": "executeFallback", "fallbackId": "copy-to-clipboard", "input": "test text"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::ExecuteFallback {
                fallback_id, input, ..
            } => {
                assert_eq!(fallback_id, "copy-to-clipboard");
                assert_eq!(input, "test text");
            }
            _ => panic!("Expected ExecuteFallback command"),
        }
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_allows_dot_test_screenshots() -> anyhow::Result<()>
    {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = std::fs::canonicalize(temp.path()).context("canonicalize temp dir")?;
        let kit_root = cwd.join("kit-root");
        std::fs::create_dir_all(&kit_root).context("create kit root")?;

        let resolved = validate_capture_window_output_path_with_roots(
            ".test-screenshots/shot.png",
            &cwd,
            &kit_root,
        )
        .context("path should be accepted")?;

        assert_eq!(resolved, cwd.join(".test-screenshots/shot.png"));
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_rejects_traversal() -> anyhow::Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = temp.path();
        let kit_root = cwd.join("kit-root");
        std::fs::create_dir_all(&kit_root).context("create kit root")?;

        let error = validate_capture_window_output_path_with_roots(
            ".test-screenshots/../escape.png",
            cwd,
            &kit_root,
        )
        .err()
        .context("path traversal should be rejected")?;

        assert!(matches!(
            error,
            CaptureWindowPathPolicyError::PathOutsideAllowedRoots { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_rejects_symlink_parent() -> anyhow::Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = temp.path();
        let kit_root = cwd.join("kit-root");
        std::fs::create_dir_all(&kit_root).context("create kit root")?;

        let screenshots_root = cwd.join(".test-screenshots");
        std::fs::create_dir_all(&screenshots_root).context("create screenshots root")?;

        let outside = cwd.join("outside");
        std::fs::create_dir_all(&outside).context("create outside dir")?;

        let symlink_path = screenshots_root.join("linked");
        create_symlink(&outside, &symlink_path)?;

        let error = validate_capture_window_output_path_with_roots(
            ".test-screenshots/linked/shot.png",
            cwd,
            &kit_root,
        )
        .err()
        .context("symlink target should be rejected")?;

        assert!(matches!(
            error,
            CaptureWindowPathPolicyError::SymlinkInPath { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_validate_capture_window_output_path_allows_scriptkit_screenshots_root(
    ) -> anyhow::Result<()> {
        let temp = TempDir::new().context("create temp dir")?;
        let cwd = std::fs::canonicalize(temp.path()).context("canonicalize temp dir")?;
        let kit_root = cwd.join("kit-root");
        let screenshots_root = kit_root.join("screenshots");
        std::fs::create_dir_all(&screenshots_root).context("create screenshots root")?;

        let target = screenshots_root.join("shot.png");
        let resolved = validate_capture_window_output_path_with_roots(
            target.to_string_lossy().as_ref(),
            &cwd,
            &kit_root,
        )
        .context("path should be accepted")?;

        assert_eq!(resolved, target);
        Ok(())
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
        std::os::unix::fs::symlink(target, link).context("create symlink")?;
        Ok(())
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
        std::os::windows::fs::symlink_dir(target, link).context("create symlink")?;
        Ok(())
    }

    #[test]
    fn test_external_command_open_focused_text_agent_chat_from_focused_field_with_mock_data_deserialization(
    ) -> anyhow::Result<()> {
        let json = r#"{"type":"openFocusedTextAgentChatFromFocusedFieldWithMockData","instruction":"Translate","requestId":"ft-live"}"#;
        let cmd: ExternalCommand = serde_json::from_str(json)?;
        match cmd {
            ExternalCommand::OpenFocusedTextAgentChatFromFocusedFieldWithMockData {
                instruction,
                request_id,
            } => {
                assert_eq!(instruction.as_deref(), Some("Translate"));
                assert_eq!(request_id.as_deref(), Some("ft-live"));
            }
            other => panic!(
                "Expected OpenFocusedTextAgentChatFromFocusedFieldWithMockData, got {other:?}"
            ),
        }
        Ok(())
    }
}
