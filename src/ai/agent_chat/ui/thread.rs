//! Agent Chat thread entity.
//!
//! Owns one Agent Chat conversation thread: messages, composer input, staged context
//! blocks, pending permission state, and the streaming event pump.
//!
//! Plain Tab stages context without sending a turn; the context blocks are
//! prepended to the first real user submit only. Quick-submit (Shift+Tab with
//! intent) populates `initial_input` and calls `submit_input()` after deferred
//! capture resolves.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ai::agent_chat::content::{ContentBlock, ImageContent, TextContent};
use gpui::{Context, SharedString, Task};
use smol::Timer;

use crate::components::text_input::TextInputState;
use crate::protocol::AiMessageInfo;

use crate::ai::agent_chat::runtime::{AgentChatConnection, AgentChatTurnRequest};

use super::notifications::{
    dispatch_agent_chat_notification, should_notify_agent_chat_event, truncate_notification_body,
    AgentChatNotificationDebounce, AgentChatNotificationEvent, AgentChatNotificationVisibility,
};
use super::streaming_buffer::StreamingTextBuffer;
use super::{
    build_tab_ai_agent_chat_context_blocks, AgentChatApprovalRequest, AgentChatEvent,
    AgentChatEventRx,
};

fn truncate_chars_for_title_prompt(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push('\u{2026}');
    }
    out
}

#[cfg(test)]
struct TestAgentChatConnection;

#[cfg(test)]
impl AgentChatConnection for TestAgentChatConnection {
    fn start_turn(
        &self,
        _request: AgentChatTurnRequest,
    ) -> anyhow::Result<crate::ai::agent_chat::events::AgentChatEventRx> {
        anyhow::bail!("test connection does not start turns")
    }

    fn cancel_turn(&self, _ui_thread_id: String) -> anyhow::Result<()> {
        Ok(())
    }

    fn prepare_session(
        &self,
        _ui_thread_id: String,
        _cwd: PathBuf,
    ) -> anyhow::Result<crate::ai::agent_chat::events::AgentChatEventRx> {
        anyhow::bail!("test connection does not prepare sessions")
    }
}

/// Bootstrap state for deferred context capture.
///
/// Tracks whether the Tab AI context has been assembled and staged on the
/// thread, so that the first user submit can be gated behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatContextBootstrapState {
    /// Context capture is still running in the background.
    Preparing,
    /// Context has been staged successfully.
    Ready,
    /// Context staging failed; partial or no context is available.
    Failed,
}

/// Current status of the Agent Chat thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatThreadStatus {
    /// No turn in progress; ready for input.
    Idle,
    /// Streaming events from the agent.
    Streaming,
    /// Blocked on a permission decision from the user.
    WaitingForPermission,
    /// The last turn failed.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatCwdResolutionDecision {
    Unchanged,
    RespawnNow,
    BlockInFlight,
}

pub(crate) fn decide_agent_chat_cwd_resolution(
    current_cwd: &Path,
    selected_cwd: &Path,
    status: AgentChatThreadStatus,
) -> AgentChatCwdResolutionDecision {
    if current_cwd == selected_cwd {
        return AgentChatCwdResolutionDecision::Unchanged;
    }
    match status {
        AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission => {
            AgentChatCwdResolutionDecision::BlockInFlight
        }
        AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error => {
            AgentChatCwdResolutionDecision::RespawnNow
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatHostWindowKind {
    Main,
    Detached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentChatHostWindowState {
    pub(crate) kind: AgentChatHostWindowKind,
    pub(crate) key: bool,
}

/// Severity for the active composer callout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatCalloutSeverity {
    Error,
}

/// Provider authentication recovery offered by a failed-turn callout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatAuthRecovery {
    /// The current account cannot continue because its usage allowance is exhausted.
    UsageLimitReached,
    /// The provider rejected or no longer recognizes the current credentials.
    AuthenticationRequired,
}

/// Dismissable, actionable callout rendered above the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatCallout {
    pub(crate) severity: AgentChatCalloutSeverity,
    pub(crate) title: SharedString,
    pub(crate) detail: Option<SharedString>,
    /// Original provider payload retained for diagnostics and Copy Error.
    pub(crate) raw_detail: Option<SharedString>,
    pub(crate) can_retry: bool,
    pub(crate) auth_recovery: Option<AgentChatAuthRecovery>,
}

impl AgentChatCallout {
    fn failed(error: impl Into<SharedString>, can_retry: bool) -> Self {
        let raw_error = error.into();
        let presentation = agent_chat_failure_presentation(raw_error.as_ref());
        Self {
            severity: AgentChatCalloutSeverity::Error,
            title: presentation.title.into(),
            detail: Some(presentation.message.into()),
            raw_detail: Some(raw_error),
            can_retry,
            auth_recovery: presentation.auth_recovery,
        }
    }

    fn notice(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            severity: AgentChatCalloutSeverity::Error,
            title: title.into(),
            detail: Some(detail.into()),
            raw_detail: None,
            can_retry: false,
            auth_recovery: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentChatFailurePresentation {
    title: String,
    message: String,
    auth_recovery: Option<AgentChatAuthRecovery>,
}

fn agent_chat_failure_presentation(error: &str) -> AgentChatFailurePresentation {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("usage_limit_reached")
        || normalized.contains("usage limit reached")
        || normalized.contains("usage limit has been reached")
    {
        return AgentChatFailurePresentation {
            title: "Account usage limit reached".to_string(),
            message: "This provider account cannot continue right now. Switch accounts, or sign in again after changing the account in your browser, then retry.".to_string(),
            auth_recovery: Some(AgentChatAuthRecovery::UsageLimitReached),
        };
    }

    if normalized.contains("authentication_required")
        || normalized.contains("authentication required")
        || normalized.contains("invalid_api_key")
        || normalized.contains("invalid api key")
        || normalized.contains("unauthorized")
        || normalized.contains("http 401")
    {
        return AgentChatFailurePresentation {
            title: "Provider sign-in required".to_string(),
            message: "Your provider sign-in is missing or expired. Sign in again, or switch accounts, then retry.".to_string(),
            auth_recovery: Some(AgentChatAuthRecovery::AuthenticationRequired),
        };
    }

    AgentChatFailurePresentation {
        title: "Turn failed".to_string(),
        message: error.to_string(),
        auth_recovery: None,
    }
}

/// Role for a message in the thread history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatThreadMessageRole {
    User,
    Assistant,
    Thought,
    Tool,
    System,
    Error,
}

/// A single message in the thread history.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatThreadMessage {
    pub id: u64,
    pub role: AgentChatThreadMessageRole,
    pub body: SharedString,
    /// Optional tool call ID linking this message to an `AgentChatToolCallState`.
    pub tool_call_id: Option<String>,
    /// Structured card metadata for Tool messages (kind, status, subject, diff).
    pub tool_meta: Option<super::tool_card::AgentChatToolCardMeta>,
    /// Context attached to this user message at submit (label + snippet),
    /// rendered in the transcript so the user can see WHAT text was sent and
    /// where it was grabbed from (e.g. `Selection — Safari`).
    pub attachments: Vec<AgentChatMessageAttachment>,
}

/// Visible receipt of one context part sent with a user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatMessageAttachment {
    pub label: SharedString,
    /// Short excerpt of the attached text, when the part carries text.
    pub snippet: Option<SharedString>,
}

impl AgentChatMessageAttachment {
    /// Max chars of attached text shown in the transcript receipt.
    const SNIPPET_MAX_CHARS: usize = 220;

    fn from_part(part: &crate::ai::message_parts::AiContextPart) -> Self {
        use crate::ai::message_parts::AiContextPart;
        let snippet = match part {
            AiContextPart::TextBlock { text, .. } => {
                let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let mut snippet: String = collapsed.chars().take(Self::SNIPPET_MAX_CHARS).collect();
                if collapsed.chars().count() > Self::SNIPPET_MAX_CHARS {
                    snippet.push('\u{2026}');
                }
                (!snippet.is_empty()).then(|| SharedString::from(snippet))
            }
            AiContextPart::FilePath { path, .. } => Some(SharedString::from(path.clone())),
            _ => None,
        };
        Self {
            label: SharedString::from(part.label().to_string()),
            snippet,
        }
    }
}

impl AgentChatThreadMessage {
    fn new(id: u64, role: AgentChatThreadMessageRole, body: impl Into<SharedString>) -> Self {
        Self {
            id,
            role,
            body: body.into(),
            tool_call_id: None,
            tool_meta: None,
            attachments: Vec::new(),
        }
    }

    fn with_tool_call_id(
        id: u64,
        role: AgentChatThreadMessageRole,
        body: impl Into<SharedString>,
        tool_call_id: String,
    ) -> Self {
        Self {
            id,
            role,
            body: body.into(),
            tool_call_id: Some(tool_call_id),
            tool_meta: None,
            attachments: Vec::new(),
        }
    }
}

/// Tracked state for a single tool call, kept in sync across start/update events.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatToolCallState {
    /// Agent Chat tool call identifier.
    pub tool_call_id: String,
    /// Display title (e.g. "Read file").
    pub title: String,
    /// Latest status text (e.g. "running", "completed").
    pub status: String,
    /// Latest body text (e.g. file contents, command output).
    pub body: Option<String>,
    /// Raw Pi tool name (e.g. "bash", "edit") when the event carried one.
    pub tool_name: Option<String>,
    /// One-line subject extracted from the tool args (path, command, query).
    pub subject: Option<String>,
    /// Pre-rendered diff from `result.details.diff` for edit/write tools.
    pub diff: Option<String>,
    /// Whether the tool reported an error result.
    pub is_error: bool,
    /// ID of the corresponding `AgentChatThreadMessage` so the view can correlate.
    pub message_id: u64,
    /// Index into `AgentChatThread::messages` for O(1) message lookup.
    message_index: usize,
}

impl AgentChatToolCallState {
    fn card_meta(&self) -> super::tool_card::AgentChatToolCardMeta {
        use super::tool_card::{AgentChatToolCardMeta, AgentChatToolKind, AgentChatToolStatus};
        let tool_name = self.tool_name.clone().unwrap_or_else(|| self.title.clone());
        AgentChatToolCardMeta {
            kind: AgentChatToolKind::from_tool_name(&tool_name),
            tool_name,
            status: if self.is_error {
                AgentChatToolStatus::Failed
            } else {
                AgentChatToolStatus::from_status_str(&self.status)
            },
            subject: self.subject.clone(),
            diff: self.diff.clone(),
            is_error: self.is_error,
        }
    }
}

/// Snapshot of composer draft state that can survive Agent Chat view relaunch.
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentChatThreadDraftSnapshot {
    pub input: String,
    pub input_cursor: usize,
    pub pending_context_parts: Vec<crate::ai::message_parts::AiContextPart>,
    pub pending_context_consumed: bool,
}

/// Initialization parameters for creating an `AgentChatThread`.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatThreadInit {
    /// Unique identifier for this UI thread (used to map to Agent Chat sessions).
    pub ui_thread_id: String,
    /// Working directory for the Agent Chat session.
    pub cwd: PathBuf,
    /// Optional initial input text (populated by Shift+Tab quick-submit).
    pub initial_input: Option<String>,
    /// Optional typed context parts staged before the first submit.
    pub initial_context_parts: Vec<crate::ai::message_parts::AiContextPart>,
    /// Display name for the agent (shown in toolbar, e.g. "Claude Code").
    pub display_name: SharedString,
    /// Stable identifier for the selected Agent Chat profile.
    pub profile_id: String,
    /// Display name for the selected Agent Chat profile (shown beside model).
    pub profile_display_name: Option<SharedString>,
    /// Icon name for the selected Agent Chat profile.
    pub profile_icon_name: Option<String>,
    /// Available models for this agent.
    pub available_models: Vec<super::config::AgentChatModelEntry>,
    /// Initially selected model ID (e.g. "claude-sonnet-4-6").
    pub selected_model_id: Option<String>,
    /// The resolved catalog entry for the selected agent (used for runtime
    /// setup recovery — preserves agent context when `SetupRequired` fires).
    pub selected_agent: Option<super::catalog::AgentChatAgentCatalogEntry>,
    /// Full agent catalog carried through for runtime recovery picker.
    pub available_agents: Vec<super::catalog::AgentChatAgentCatalogEntry>,
    /// Capability requirements derived when the chat was opened.
    /// Preserved through runtime recovery so agent switching stays
    /// capability-driven.
    pub launch_requirements: super::preflight::AgentChatLaunchRequirements,
}

/// One-shot context payload consumed by `prepare_turn_blocks()`.
///
/// Holds the resolved hidden blocks and the resolution receipt from typed
/// context parts. Produced by `take_pending_context_for_turn()` and consumed
/// exactly once per submission.
struct PendingContextTurn {
    blocks: Vec<ContentBlock>,
    receipt: crate::ai::message_parts::ContextResolutionReceipt,
    /// Visible label+snippet receipts for the consumed parts, attached to the
    /// transcript user message.
    attachments: Vec<AgentChatMessageAttachment>,
}

/// Resolved turn-scoped context blocks plus the receipt describing
/// resolution outcomes for the current submit.
struct ResolvedPendingContext {
    blocks: Vec<ContentBlock>,
    receipt: crate::ai::message_parts::ContextResolutionReceipt,
}

/// Return value from `prepare_turn_blocks_with_receipt()`.
///
/// Carries the content blocks for the turn AND the optional resolution
/// receipt so callers can surface partial-failure feedback.
struct PreparedTurnBlocks {
    blocks: Vec<ContentBlock>,
    receipt: Option<crate::ai::message_parts::ContextResolutionReceipt>,
    /// Visible label+snippet receipts for the transcript user message.
    attachments: Vec<AgentChatMessageAttachment>,
}

/// A user submit captured while the current turn is locked.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatQueuedMessage {
    pub(crate) text: String,
    pub(crate) context_parts: Vec<crate::ai::message_parts::AiContextPart>,
}

impl AgentChatQueuedMessage {
    fn new(text: String, context_parts: Vec<crate::ai::message_parts::AiContextPart>) -> Self {
        Self {
            text,
            context_parts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletedChatTurnIngest {
    thread_id: String,
    turn_index: usize,
    user_text: String,
    assistant_text: String,
    trace_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum SkillContextStagedBy {
    MainMenu,
    SlashPicker,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SkillContextIdentity {
    pub(crate) thread_id: String,
    pub(crate) skill_id: String,
    pub(crate) skill_file_hash: String,
    pub(crate) staged_by: SkillContextStagedBy,
}

/// GPUI entity that owns one Agent Chat conversation thread.
///
/// Holds durable message history, staged context blocks (consumed once on
/// first submit), composer input, streaming status, and pending permission
/// requests. Binds stream and permission listeners via `cx.spawn(...)`.
pub(crate) struct AgentChatThread {
    connection: Arc<dyn AgentChatConnection>,
    permission_rx: async_channel::Receiver<AgentChatApprovalRequest>,

    ui_thread_id: String,
    cwd: PathBuf,
    /// Display name for the agent (shown in toolbar).
    display_name: SharedString,

    /// Thread message history (durable across turns).
    pub(crate) messages: Vec<AgentChatThreadMessage>,
    /// Current composer input state (with cursor, selection, undo/redo).
    pub(crate) input: TextInputState,
    /// Current thread status.
    pub(crate) status: AgentChatThreadStatus,
    /// Active composer callout, rendered above the composer until dismissed or superseded.
    active_callout: Option<AgentChatCallout>,
    /// Pending permission request awaiting user decision.
    pub(crate) pending_permission: Option<AgentChatApprovalRequest>,

    /// Staged context blocks, prepended to the first user submit only.
    pending_context_blocks: Vec<ContentBlock>,
    /// Whether staged context has already been consumed.
    pending_context_consumed: bool,

    /// Typed context parts visible as chips in the composer.
    /// Resolved into prompt blocks at submit time via
    /// `resolve_context_parts_with_receipt`. Supports add/remove/dedup.
    pending_context_parts: Vec<crate::ai::message_parts::AiContextPart>,

    /// Whether the Ask Anything ambient context path is still active.
    /// Set `true` when an Ask Anything chip is staged; cleared when the
    /// chip is removed. When `false`, deferred ambient capture is suppressed.
    pending_ambient_context_enabled: bool,

    /// Whether the deferred context capture has completed.
    context_bootstrap_state: AgentChatContextBootstrapState,
    /// Whether a submit was attempted while context was still `Preparing`.
    queued_submit_while_bootstrapping: bool,
    /// Human-readable status note for the bootstrap phase.
    context_bootstrap_note: Option<SharedString>,

    /// User messages submitted while the current turn is locked.
    queued_messages: VecDeque<AgentChatQueuedMessage>,
    /// User cancellation pauses queued auto-send without discarding drafts.
    queue_paused: bool,

    // ── Structured session state (readable by the view) ──────────
    /// Current plan entries from the latest `PlanUpdated` event.
    active_plan_entries: Vec<String>,
    /// Current agent mode from the latest `ModeChanged` event.
    active_mode_id: Option<String>,
    /// Current available commands from the latest `AvailableCommandsUpdated`.
    available_commands: Vec<String>,
    /// Tracked tool calls keyed by their Agent Chat tool_call_id.
    active_tool_calls: Vec<AgentChatToolCallState>,
    /// O(1) lookup from tool_call_id to index in `active_tool_calls`.
    tool_call_lookup: HashMap<String, usize>,
    /// Session-scoped "Allow always" grants the user has made, in grant
    /// order. Pi owns the live approval cache; this mirror keeps the grants
    /// visible and reviewable in the UI. Survives `clear_messages` (same Pi
    /// session keeps its cache) and resets with the session.
    standing_approvals: Vec<super::permission_broker::AgentChatStandingApproval>,

    /// User messages the live Pi session can rewind to, in conversation
    /// order (refreshed after every finished turn via `get_fork_messages`).
    fork_points: Vec<super::events::AgentChatForkPoint>,
    /// Ordinal (index into `fork_points`) of an in-flight rewind request.
    /// Consumed when the matching `ForkCompleted` event arrives.
    pending_fork_ordinal: Option<usize>,

    /// The resolved catalog entry for the selected agent. Retained so
    /// runtime `SetupRequired` events can build recovery cards with
    /// agent-specific context.
    selected_agent: Option<super::catalog::AgentChatAgentCatalogEntry>,

    /// Full agent catalog for runtime recovery picker.
    available_agents: Vec<super::catalog::AgentChatAgentCatalogEntry>,

    /// Capability requirements from the original launch context.
    /// Used during runtime recovery to ensure agent switching stays
    /// capability-driven.
    launch_requirements: super::preflight::AgentChatLaunchRequirements,

    /// Inline setup state armed by a runtime `SetupRequired` event.
    /// When `Some`, the view renders the setup recovery card instead of
    /// the normal chat transcript.
    setup_state: Option<super::setup_state::AgentChatInlineSetupState>,

    /// Session usage: tokens used / context window size.
    pub(crate) usage_tokens: Option<(u64, u64)>,
    /// Session cost in USD (cumulative).
    pub(crate) usage_cost_usd: Option<f64>,

    /// When the current streaming turn started (for elapsed time display).
    stream_started_at: Option<std::time::Instant>,
    /// True between submit and the first assistant/thought delta of the turn;
    /// gates the one-shot time-to-first-token log.
    ttft_pending: bool,

    /// Handle to the active stream pump task.
    stream_task: Option<Task<()>>,
    /// Handle to the permission listener task.
    permission_task: Option<Task<()>>,
    /// Buffered assistant deltas waiting for typewriter-paced reveal.
    streaming_text_buffer: StreamingTextBuffer,
    /// Handle to the active assistant text drain task.
    streaming_text_drain_task: Option<Task<()>>,

    /// Generation guard for async stream delivery into the transcript.
    transcript_generation: u64,

    /// Monotonically increasing message ID counter.
    next_message_id: u64,

    /// Host-window state last observed by the AgentChatView render/activation hooks.
    host_window_state: Option<AgentChatHostWindowState>,
    notification_debounce: AgentChatNotificationDebounce,
    current_turn_id: u64,
    llm_title_attempted: bool,

    // ── Model selection ──────────────────────────────────────
    /// Available models for this agent.
    available_models: Vec<super::config::AgentChatModelEntry>,
    /// Currently selected model ID.
    selected_model_id: Option<String>,
    /// Display name for the selected model.
    selected_model_display_name: Option<SharedString>,
    /// Stable identifier for the selected Agent Chat profile.
    profile_id: String,
    /// Display name for the selected Agent Chat profile.
    profile_display_name: Option<SharedString>,
    /// Icon name for the selected Agent Chat profile.
    profile_icon_name: Option<String>,
}

impl AgentChatThread {
    pub(crate) fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    pub(crate) fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    pub(crate) fn status(&self) -> AgentChatThreadStatus {
        self.status
    }

    pub(crate) fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub(crate) fn set_notice_callout(
        &mut self,
        title: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.active_callout = Some(AgentChatCallout::notice(title, detail));
        cx.notify();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn replace_pi_session(
        &mut self,
        connection: Arc<dyn AgentChatConnection>,
        ui_thread_id: String,
        cwd: PathBuf,
        profile_id: String,
        profile_display_name: SharedString,
        profile_icon_name: Option<String>,
        available_models: Vec<super::config::AgentChatModelEntry>,
        selected_model_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.flush_streaming_text_buffer();
        self.stream_task = None;
        self.streaming_text_drain_task = None;
        self.connection = connection;
        self.ui_thread_id = ui_thread_id;
        self.cwd = cwd;
        self.profile_id = profile_id;
        self.profile_display_name = Some(profile_display_name.clone());
        self.display_name = profile_display_name;
        self.profile_icon_name = profile_icon_name;
        self.available_models = available_models;
        self.selected_model_id = selected_model_id;
        self.selected_model_display_name = None;
        self.active_plan_entries.clear();
        self.active_mode_id = None;
        self.available_commands.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.standing_approvals.clear();
        self.fork_points.clear();
        self.pending_fork_ordinal = None;
        self.pending_permission = None;
        self.setup_state = None;
        self.usage_tokens = None;
        self.usage_cost_usd = None;
        self.active_callout = None;
        self.set_status(AgentChatThreadStatus::Idle);
        self.bump_transcript_generation("cwd_respawn");
        cx.notify();
    }

    /// Create a new thread entity with optional initial input.
    ///
    /// Immediately binds the permission listener. Does NOT send an Agent Chat turn —
    /// context is staged and only consumed on the first `submit_input()`.
    pub(crate) fn new(
        connection: Arc<dyn AgentChatConnection>,
        permission_rx: async_channel::Receiver<AgentChatApprovalRequest>,
        init: AgentChatThreadInit,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            connection,
            permission_rx,
            ui_thread_id: init.ui_thread_id,
            cwd: init.cwd,
            display_name: init.display_name,
            profile_id: init.profile_id,
            messages: Vec::new(),
            input: match init.initial_input {
                Some(text) if !text.is_empty() => TextInputState::with_text(text),
                _ => TextInputState::new(),
            },
            status: AgentChatThreadStatus::Idle,
            active_callout: None,
            pending_permission: None,
            pending_context_blocks: Vec::new(),
            pending_context_consumed: false,
            pending_context_parts: init.initial_context_parts,
            pending_ambient_context_enabled: false,
            context_bootstrap_state: AgentChatContextBootstrapState::Preparing,
            queued_submit_while_bootstrapping: false,
            context_bootstrap_note: None,
            queued_messages: VecDeque::new(),
            queue_paused: false,
            active_plan_entries: Vec::new(),
            active_mode_id: None,
            available_commands: Vec::new(),
            active_tool_calls: Vec::new(),
            tool_call_lookup: HashMap::new(),
            standing_approvals: Vec::new(),
            fork_points: Vec::new(),
            pending_fork_ordinal: None,
            selected_agent: init.selected_agent,
            available_agents: init.available_agents,
            launch_requirements: init.launch_requirements,
            profile_display_name: init.profile_display_name,
            profile_icon_name: init.profile_icon_name,
            setup_state: None,
            usage_tokens: None,
            usage_cost_usd: None,
            stream_started_at: None,
            ttft_pending: false,
            stream_task: None,
            permission_task: None,
            streaming_text_buffer: StreamingTextBuffer::default(),
            streaming_text_drain_task: None,
            transcript_generation: 0,
            next_message_id: 1,
            host_window_state: None,
            notification_debounce: AgentChatNotificationDebounce::default(),
            current_turn_id: 0,
            llm_title_attempted: false,
            selected_model_display_name: {
                let id = init.selected_model_id.as_deref();
                id.and_then(|sel| {
                    init.available_models.iter().find(|m| m.id == sel).map(|m| {
                        SharedString::from(m.display_name.clone().unwrap_or_else(|| m.id.clone()))
                    })
                })
            },
            selected_model_id: init.selected_model_id,
            available_models: init.available_models,
        };
        this.bind_permission_listener(cx);
        this
    }

    fn maybe_spawn_auto_title(&mut self, conversation: &super::history::SavedConversation) {
        if self.llm_title_attempted || conversation.custom_title.is_some() {
            return;
        }
        if !conversation
            .messages
            .iter()
            .any(|message| message.role.eq_ignore_ascii_case("assistant"))
        {
            return;
        }

        let Some(first_user) = conversation
            .messages
            .iter()
            .find(|message| message.role.eq_ignore_ascii_case("user"))
            .map(|message| message.body.clone())
        else {
            return;
        };
        let Some(first_assistant) = conversation
            .messages
            .iter()
            .find(|message| message.role.eq_ignore_ascii_case("assistant"))
            .map(|message| message.body.clone())
        else {
            return;
        };

        self.llm_title_attempted = true;
        let session_id = conversation.session_id.clone();
        let user_excerpt = truncate_chars_for_title_prompt(&first_user, 400);
        let assistant_excerpt = truncate_chars_for_title_prompt(&first_assistant, 400);

        let spawn_result = std::thread::Builder::new()
            .name("agent_chat-auto-title".to_string())
            .spawn(move || {
                let registry =
                    crate::ai::providers::ProviderRegistry::from_environment_with_config(None);
                if !registry.has_any_provider() {
                    return;
                }

                let result = (|| -> anyhow::Result<()> {
                    let (model, provider) =
                        crate::ai::script_generation::select_generation_model(&registry)?;
                    let messages = vec![
                        crate::ai::providers::ProviderMessage::system(
                            "You title chat conversations. Reply with ONLY a concise 3-6 word title. No quotes, no punctuation at the end.",
                        ),
                        crate::ai::providers::ProviderMessage::user(format!(
                            "User: {user_excerpt}\nAssistant: {assistant_excerpt}"
                        )),
                    ];
                    let raw = provider.send_message(&messages, &model.id)?;
                    let title = super::history::sanitize_conversation_title(&raw);
                    if title.is_empty() {
                        return Ok(());
                    }
                    super::history::rename_conversation(&session_id, &title)?;
                    Ok(())
                })();

                if let Err(error) = result {
                    tracing::debug!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_auto_title_failed",
                        session_id = %session_id,
                        error = %error,
                    );
                }
            });

        if let Err(error) = spawn_result {
            tracing::debug!(
                target: "script_kit::tab_ai",
                event = "agent_chat_auto_title_spawn_failed",
                session_id = %conversation.session_id,
                error = %error,
            );
        }
    }

    pub(crate) fn set_host_window_state(
        &mut self,
        state: AgentChatHostWindowState,
        cx: &mut Context<Self>,
    ) {
        if self.host_window_state == Some(state) {
            return;
        }
        self.host_window_state = Some(state);
        cx.notify();
    }

    fn notification_visibility(&self) -> AgentChatNotificationVisibility {
        let Some(state) = self.host_window_state else {
            return AgentChatNotificationVisibility::Unknown;
        };
        let visible = match state.kind {
            AgentChatHostWindowKind::Main => crate::is_main_window_visible(),
            AgentChatHostWindowKind::Detached => {
                crate::ai::agent_chat::ui::chat_window::is_chat_window_open()
            }
        };
        if visible && state.key {
            AgentChatNotificationVisibility::VisibleAndKey
        } else {
            AgentChatNotificationVisibility::HiddenOrNotKey
        }
    }

    fn notifications_enabled() -> bool {
        crate::config::load_user_preferences()
            .ai
            .agent_chat_notify_when_hidden
            .unwrap_or(true)
    }

    fn maybe_notify_agent_chat_event(
        &mut self,
        event: AgentChatNotificationEvent,
        title: &'static str,
        body: String,
    ) {
        if should_notify_agent_chat_event(
            event,
            self.notification_visibility(),
            Self::notifications_enabled(),
            self.current_turn_id,
            &mut self.notification_debounce,
        ) {
            dispatch_agent_chat_notification(title, truncate_notification_body(&body));
        }
    }

    /// Stage context blocks from a `TabAiContextBlob`.
    ///
    /// Marks bootstrap as `Ready` and auto-submits any queued first turn.
    /// Calling this again before a submit replaces the staged blocks.
    pub(crate) fn stage_context(
        &mut self,
        context: &crate::ai::TabAiContextBlob,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.pending_context_blocks = build_tab_ai_agent_chat_context_blocks(context)?;
        self.pending_ambient_context_enabled = false;
        self.arm_pending_context("stage_context");
        self.finish_bootstrap(
            AgentChatContextBootstrapState::Ready,
            "Context attached",
            cx,
        );
        Ok(())
    }

    /// Stage ambient context from a deferred Ask Anything capture.
    ///
    /// If the Ask Anything chip was removed before capture finished, this
    /// is a no-op that still marks bootstrap as ready.  Otherwise it stages
    /// the context blocks and promotes the visible chip from `ResourceUri`
    /// to `AmbientContext` (display-only).
    pub(crate) fn stage_ask_anything_context(
        &mut self,
        context: &crate::ai::TabAiContextBlob,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let ambient_label = self
            .current_ambient_chip_label()
            .unwrap_or_else(|| crate::ai::message_parts::ASK_ANYTHING_LABEL.to_string());

        if !self.pending_ambient_context_enabled {
            self.clear_pending_ambient_context("ask_anything_removed_before_stage");
            self.finish_bootstrap(
                AgentChatContextBootstrapState::Ready,
                Self::ambient_capture_removed_note(&ambient_label),
                cx,
            );
            return Ok(());
        }

        self.pending_context_blocks = build_tab_ai_agent_chat_context_blocks(context)?;
        self.promote_ask_anything_chip_to_ambient();
        self.arm_pending_context("stage_ask_anything_context");

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_ask_anything_promoted_to_ambient_chip",
            block_count = self.pending_context_blocks.len(),
            chip_label = %ambient_label,
        );

        self.finish_bootstrap(
            AgentChatContextBootstrapState::Ready,
            Self::ambient_capture_ready_note(&ambient_label),
            cx,
        );
        Ok(())
    }

    fn ambient_capture_preparing_note(label: &str) -> SharedString {
        if label == crate::ai::message_parts::ASK_ANYTHING_LABEL {
            "Capturing desktop context\u{2026}".into()
        } else {
            format!("Capturing {label}\u{2026}").into()
        }
    }

    fn ambient_capture_ready_note(label: &str) -> SharedString {
        if label == crate::ai::message_parts::ASK_ANYTHING_LABEL {
            "Ask Anything ready".into()
        } else {
            format!("{label} ready").into()
        }
    }

    fn ambient_capture_removed_note(label: &str) -> SharedString {
        if label == crate::ai::message_parts::ASK_ANYTHING_LABEL {
            "Ask Anything removed".into()
        } else {
            format!("{label} removed").into()
        }
    }

    fn current_ambient_chip_label(&self) -> Option<String> {
        self.pending_context_parts
            .iter()
            .find_map(|part| part.ambient_chip_label().map(|value| value.to_string()))
    }

    /// Replace the initial ambient bootstrap `ResourceUri` chip with a
    /// display-only `AmbientContext` chip, preserving the original label.
    /// If the resource chip was already removed, pushes a new ambient chip.
    fn promote_ask_anything_chip_to_ambient(&mut self) {
        if let Some(part) = self
            .pending_context_parts
            .iter_mut()
            .find(|part| part.is_ambient_bootstrap_resource())
        {
            let label = part.label().to_string();
            *part = crate::ai::message_parts::AiContextPart::AmbientContext { label };
            return;
        }
        self.pending_context_parts
            .push(crate::ai::message_parts::AiContextPart::AmbientContext {
                label: crate::ai::message_parts::ASK_ANYTHING_LABEL.to_string(),
            });
    }

    /// Mark the context bootstrap as ready without staging ambient context
    /// blocks. Used by the focused-target Tab path where only typed context
    /// parts (chips) are staged — no hidden `TabAiContextBlob`.
    pub(crate) fn mark_context_bootstrap_ready(&mut self, cx: &mut Context<Self>) {
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        if let Err(error) = self.flush_bootstrap_queue(cx) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_bootstrap_flush_failed",
                error = %error,
            );
        }
    }

    /// Mark the context bootstrap as failed with a human-readable note.
    ///
    /// If a submit was queued, it proceeds anyway (partial context is better
    /// than dropping user input).
    pub(crate) fn mark_context_bootstrap_failed(
        &mut self,
        note: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.finish_bootstrap(AgentChatContextBootstrapState::Failed, note, cx);
    }

    /// Update the composer input text (replaces entire content, cursor at end).
    pub(crate) fn set_input(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        let value = value.into();
        let cursor = value.chars().count();
        self.input.set_text(value);
        self.input.set_cursor(cursor);
        cx.notify();
    }

    /// Recall the latest user-authored turn into the composer.
    ///
    /// Mirrors common agent prompt-history behavior: plain Up on an empty,
    /// idle composer brings back the previous user prompt with the caret at
    /// the beginning so another Up-like navigation gesture stays natural.
    /// Replace assistant transcript rows after the latest user turn.
    pub(crate) fn replace_assistant_messages_after_last_user(
        &mut self,
        bodies: Vec<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(last_user_index) = self
            .messages
            .iter()
            .rposition(|message| message.role == AgentChatThreadMessageRole::User)
        else {
            return false;
        };

        self.messages.truncate(last_user_index + 1);
        for body in bodies {
            if body.trim().is_empty() {
                continue;
            }
            self.push_message(AgentChatThreadMessageRole::Assistant, body);
        }
        cx.notify();
        true
    }

    /// Add a local assistant guidance message without starting a provider turn.
    ///
    /// Host-owned staging flows use this to explain what context was attached
    /// while still requiring the user to explicitly submit the next turn.
    pub(crate) fn push_local_assistant_message(
        &mut self,
        body: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.push_message(AgentChatThreadMessageRole::Assistant, body);
        cx.notify();
    }

    pub(crate) fn recall_last_user_message(&mut self, cx: &mut Context<Self>) -> bool {
        if !matches!(
            self.status,
            AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error
        ) || !self.input.is_empty()
        {
            return false;
        }

        let Some(body) = self
            .messages
            .iter()
            .rev()
            .find(|message| message.role == AgentChatThreadMessageRole::User)
            .map(|message| message.body.to_string())
        else {
            return false;
        };

        self.input.set_text(body);
        self.input.set_cursor(0);
        cx.notify();
        true
    }

    /// True while a user turn is streaming but no assistant text has landed yet.
    pub(crate) fn awaiting_first_assistant_text(&self) -> bool {
        if !matches!(self.status, AgentChatThreadStatus::Streaming) {
            return false;
        }

        let Some(last_user_index) = self
            .messages
            .iter()
            .rposition(|message| matches!(message.role, AgentChatThreadMessageRole::User))
        else {
            return false;
        };

        !self.messages[last_user_index + 1..].iter().any(|message| {
            matches!(message.role, AgentChatThreadMessageRole::Assistant)
                && !message.body.trim().is_empty()
        })
    }

    /// Submit the current input as a new user turn.
    ///
    /// If context is still bootstrapping (`Preparing`), the submit is queued
    /// and will fire automatically when `stage_context()` or
    /// `mark_context_bootstrap_failed()` completes.
    ///
    /// Prepends staged context blocks on the first submit, then clears them.
    /// Starts streaming events from the Agent Chat agent.
    pub(crate) fn submit_input(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let input = self.input.text().to_string();
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        self.resume_queue_for_manual_submit();

        if matches!(
            self.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) {
            self.queue_current_composer(trimmed.to_string());
            cx.notify();
            return Ok(());
        }

        // Gate on bootstrap: queue instead of sending while context is still preparing.
        if matches!(
            self.context_bootstrap_state,
            AgentChatContextBootstrapState::Preparing
        ) {
            self.queued_submit_while_bootstrapping = true;
            self.context_bootstrap_note =
                Some("Queued \u{00b7} sending when context is attached\u{2026}".into());
            cx.notify();
            return Ok(());
        }

        let prepared = self.prepare_turn_blocks_with_receipt(trimmed);
        self.set_context_resolution_note(prepared.receipt.as_ref());
        self.start_prepared_turn(
            trimmed.to_string(),
            prepared.blocks,
            prepared.attachments,
            true,
            true,
            cx,
        )
    }

    fn resume_queue_for_manual_submit(&mut self) {
        self.queue_paused = false;
    }

    fn queue_current_composer(&mut self, text: String) {
        self.queued_messages.push_back(AgentChatQueuedMessage::new(
            text,
            std::mem::take(&mut self.pending_context_parts),
        ));
        self.input.clear();
    }

    fn start_prepared_turn(
        &mut self,
        display_text: String,
        blocks: Vec<ContentBlock>,
        attachments: Vec<AgentChatMessageAttachment>,
        clear_composer: bool,
        push_user_message: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let rx = self
            .connection
            .start_turn(AgentChatTurnRequest {
                ui_thread_id: self.ui_thread_id.clone(),
                cwd: self.cwd.clone(),
                blocks,
                model_id: self.selected_model_id.clone(),
            })
            .map_err(|error| error.to_string())?;

        if push_user_message {
            let msg_id = self.alloc_id();
            let mut message = AgentChatThreadMessage::new(
                msg_id,
                AgentChatThreadMessageRole::User,
                display_text.clone(),
            );
            message.attachments = attachments;
            self.messages.push(message);
            self.publish_sdk_new_message(msg_id, AgentChatThreadMessageRole::User, display_text);
        }
        if clear_composer {
            self.input.clear();
        }
        self.stream_started_at = Some(std::time::Instant::now());
        self.ttft_pending = true;
        self.status = AgentChatThreadStatus::Streaming;
        self.active_callout = None;
        self.current_turn_id = self.current_turn_id.wrapping_add(1);

        self.setup_state = None;
        self.bind_stream(rx, cx);
        cx.notify();
        Ok(())
    }

    fn submit_queued_message(
        &mut self,
        message: AgentChatQueuedMessage,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let saved_blocks = std::mem::take(&mut self.pending_context_blocks);
        let saved_consumed = self.pending_context_consumed;
        let saved_parts = std::mem::replace(&mut self.pending_context_parts, message.context_parts);
        let saved_ambient = self.pending_ambient_context_enabled;

        self.pending_context_consumed = false;
        self.pending_ambient_context_enabled = false;
        let prepared = self.prepare_turn_blocks_with_receipt(&message.text);
        self.set_context_resolution_note(prepared.receipt.as_ref());

        self.pending_context_blocks = saved_blocks;
        self.pending_context_consumed = saved_consumed;
        self.pending_context_parts = saved_parts;
        self.pending_ambient_context_enabled = saved_ambient;

        self.start_prepared_turn(
            message.text,
            prepared.blocks,
            prepared.attachments,
            false,
            true,
            cx,
        )
    }

    fn submit_next_queued_if_ready(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.queue_paused
            || self.queued_messages.is_empty()
            || !matches!(self.status, AgentChatThreadStatus::Idle)
        {
            return Ok(());
        }
        if let Some(message) = self.queued_messages.pop_front() {
            self.submit_queued_message(message, cx)?;
        }
        Ok(())
    }

    /// Submit an already-built provider prompt while displaying a separate
    /// user-facing transcript string. Focused-text mini uses this so captured
    /// field contents stay hidden provider context instead of visible composer
    /// text.
    pub(crate) fn submit_blocks(
        &mut self,
        blocks: Vec<ContentBlock>,
        display_user_text: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if matches!(
            self.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) {
            return Ok(());
        }

        let display_user_text = display_user_text.into();
        let trimmed_display = display_user_text.trim();
        if trimmed_display.is_empty() {
            return Ok(());
        }

        if matches!(
            self.context_bootstrap_state,
            AgentChatContextBootstrapState::Preparing
        ) {
            return Err("context_bootstrap_pending".to_string());
        }

        self.clear_all_pending_context("submit_blocks");

        let msg_id = self.alloc_id();
        self.messages.push(AgentChatThreadMessage::new(
            msg_id,
            AgentChatThreadMessageRole::User,
            trimmed_display.to_string(),
        ));
        self.publish_sdk_new_message(
            msg_id,
            AgentChatThreadMessageRole::User,
            trimmed_display.to_string(),
        );
        self.input.clear();
        self.stream_started_at = Some(std::time::Instant::now());
        self.ttft_pending = true;
        self.status = AgentChatThreadStatus::Streaming;

        let rx = self
            .connection
            .start_turn(AgentChatTurnRequest {
                ui_thread_id: self.ui_thread_id.clone(),
                cwd: self.cwd.clone(),
                blocks,
                model_id: self.selected_model_id.clone(),
            })
            .map_err(|error| error.to_string())?;

        self.active_callout = None;
        self.setup_state = None;
        self.bind_stream(rx, cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn active_callout(&self) -> Option<&AgentChatCallout> {
        self.active_callout.as_ref()
    }

    pub(crate) fn dismiss_active_callout(&mut self, cx: &mut Context<Self>) {
        if self.active_callout.take().is_some() {
            cx.notify();
        }
    }

    fn last_user_turn_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, AgentChatThreadMessageRole::User))
            .map(|message| message.body.to_string())
    }

    pub(crate) fn retry_last_user_turn(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if !matches!(
            self.status,
            AgentChatThreadStatus::Error | AgentChatThreadStatus::Idle
        ) {
            return Ok(());
        }

        let Some(display_text) = self.last_user_turn_text() else {
            return Err("no_user_turn_to_retry".to_string());
        };
        let prepared = self.prepare_turn_blocks_with_receipt(display_text.trim());
        self.set_context_resolution_note(prepared.receipt.as_ref());
        self.start_prepared_turn(
            display_text,
            prepared.blocks,
            prepared.attachments,
            false,
            false,
            cx,
        )
    }

    /// Resolve a pending permission request with the user's selection.
    ///
    /// Pass `None` for cancellation, or `Some(option_id)` for a selection.
    pub(crate) fn approve_pending_permission(
        &mut self,
        selected_option_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let mut had_request = false;
        let mut changed = false;

        if let Some(request) = self.pending_permission.take() {
            let note = Self::permission_resolution_message(&request, selected_option_id.as_deref());
            self.record_standing_approval(&request, selected_option_id.as_deref());
            let _ = request.reply_tx.send_blocking(selected_option_id);
            changed |= self.push_message(AgentChatThreadMessageRole::System, note);
            had_request = true;
        }

        // Stay in Streaming so submit_input() remains blocked until
        // TurnFinished or Failed arrives — prevents mid-turn double-submit.
        if had_request {
            changed |= self.set_status(AgentChatThreadStatus::Streaming);
        }

        if changed {
            cx.notify();
        }
    }

    /// Record a session-scoped grant when the chosen option is a persistent
    /// "Allow always". Deduped by (tool, subject) so repeated grants for the
    /// same tool do not stack.
    fn record_standing_approval(
        &mut self,
        request: &AgentChatApprovalRequest,
        selected_option_id: Option<&str>,
    ) {
        let Some(option) = selected_option_id
            .and_then(|id| request.options.iter().find(|opt| opt.option_id == id))
        else {
            return;
        };
        if !option.is_persistent_allow() {
            return;
        }

        let (tool_title, subject, kind_badge) = match request.preview.as_ref() {
            Some(preview) => (
                preview.tool_title.clone(),
                preview.subject.clone(),
                preview.kind.badge_label(),
            ),
            None => (
                request.title.clone(),
                None,
                super::permission_broker::AgentChatApprovalPreviewKind::Generic.badge_label(),
            ),
        };

        let already_recorded = self
            .standing_approvals
            .iter()
            .any(|grant| grant.tool_title == tool_title && grant.subject == subject);
        if already_recorded {
            return;
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_standing_approval_recorded",
            ui_thread = %self.ui_thread_id,
            tool_title = %tool_title,
            has_subject = subject.is_some(),
            total = self.standing_approvals.len() + 1,
        );
        self.standing_approvals
            .push(super::permission_broker::AgentChatStandingApproval {
                tool_title,
                subject,
                kind_badge,
                option_label: option.summary_label(),
            });
    }

    /// Session-scoped "Allow always" grants recorded so far, in grant order.
    pub(crate) fn standing_approvals(
        &self,
    ) -> &[super::permission_broker::AgentChatStandingApproval] {
        &self.standing_approvals
    }

    /// Push a System transcript message listing every standing approval, so
    /// the user can review what the session will no longer ask about.
    pub(crate) fn review_standing_approvals(&mut self, cx: &mut Context<Self>) {
        let body = if self.standing_approvals.is_empty() {
            "**Auto-approvals** \u{00b7} none granted this session.".to_string()
        } else {
            let mut lines = vec![format!(
                "**Auto-approvals** \u{00b7} {} standing grant{} this session:",
                self.standing_approvals.len(),
                if self.standing_approvals.len() == 1 {
                    ""
                } else {
                    "s"
                },
            )];
            for grant in &self.standing_approvals {
                let subject = grant
                    .subject
                    .as_deref()
                    .map(|subject| format!(" \u{00b7} `{subject}`"))
                    .unwrap_or_default();
                lines.push(format!(
                    "- {} \u{00b7} {}{subject} \u{00b7} {}",
                    grant.tool_title, grant.kind_badge, grant.option_label,
                ));
            }
            lines.push(
                "Grants live in the Pi session approval cache; starting a new session resets them."
                    .to_string(),
            );
            lines.join("\n")
        };
        self.push_message(AgentChatThreadMessageRole::System, body);
        cx.notify();
    }

    fn permission_notification_body(&self, request: &AgentChatApprovalRequest) -> String {
        request
            .preview
            .as_ref()
            .map(|preview| {
                preview
                    .subject
                    .as_ref()
                    .map(|subject| format!("{} · {subject}", preview.tool_title))
                    .unwrap_or_else(|| preview.tool_title.clone())
            })
            .unwrap_or_else(|| {
                if request.body.trim().is_empty() {
                    request.title.clone()
                } else {
                    format!("{} · {}", request.title, request.body)
                }
            })
    }

    /// Build a human-readable audit message for a permission resolution.
    fn permission_resolution_message(
        request: &AgentChatApprovalRequest,
        selected_option_id: Option<&str>,
    ) -> String {
        let tool_title = request
            .preview
            .as_ref()
            .map(|p| p.tool_title.clone())
            .unwrap_or_else(|| request.title.clone());

        match selected_option_id
            .and_then(|id| request.options.iter().find(|opt| opt.option_id == id))
        {
            Some(option) => format!(
                "Permission granted \u{00b7} {} \u{00b7} {}",
                tool_title,
                option.summary_label()
            ),
            None => format!("Permission cancelled \u{00b7} {}", tool_title),
        }
    }

    // ── Private helpers ────────────────────────────────────────────

    /// Mark pending context as ready for the next submit.
    fn arm_pending_context(&mut self, reason: &'static str) {
        self.pending_context_consumed = false;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_context_armed",
            reason,
            pending_part_count = self.pending_context_parts.len(),
            pending_block_count = self.pending_context_blocks.len(),
            ambient_enabled = self.pending_ambient_context_enabled,
        );
    }

    /// Clear hidden ambient context blocks and disable the ambient flag.
    fn clear_pending_ambient_context(&mut self, reason: &'static str) {
        let cleared_block_count = self.pending_context_blocks.len();
        self.pending_context_blocks.clear();
        self.pending_ambient_context_enabled = false;
        self.pending_context_consumed = false;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_ambient_context_cleared",
            reason,
            cleared_block_count,
            pending_part_count = self.pending_context_parts.len(),
        );
    }

    /// Clear all pending context state (parts, blocks, flags).
    fn clear_all_pending_context(&mut self, reason: &'static str) {
        let cleared_part_count = self.pending_context_parts.len();
        let cleared_block_count = self.pending_context_blocks.len();
        self.pending_context_parts.clear();
        self.pending_context_blocks.clear();
        self.pending_context_consumed = false;
        self.pending_ambient_context_enabled = false;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_context_cleared",
            reason,
            cleared_part_count,
            cleared_block_count,
        );
    }

    /// Returns `true` for the explicit `@screenshot` resource chip.
    ///
    /// Agent Chat follow-up submits should attach this as a real image block instead
    /// of only resolving the text-only `kit://context?...` snapshot JSON.
    fn is_explicit_screenshot_part(part: &crate::ai::message_parts::AiContextPart) -> bool {
        matches!(
            part,
            crate::ai::message_parts::AiContextPart::ResourceUri { uri, label }
                if label == "Screenshot" && uri.contains("screenshot=1")
        )
    }

    /// Capture the explicit screenshot chip as an Agent Chat image block.
    ///
    /// `@screenshot` captures the active desktop — the display the Script Kit
    /// panel is on — with Script Kit's own windows excluded so the chat panel
    /// does not cover the content being asked about.
    ///
    /// Returns `Ok(None)` for non-screenshot parts so the normal prompt-block
    /// resolver can handle them. On capture failure the caller falls back to
    /// the canonical `kit://context?...` resource path.
    fn capture_special_context_block_for_part(
        part: &crate::ai::message_parts::AiContextPart,
    ) -> Result<Option<ContentBlock>, String> {
        if !Self::is_explicit_screenshot_part(part) {
            return Ok(None);
        }

        let (png_data, width, height) =
            crate::platform::capture_screen_screenshot().map_err(|error| error.to_string())?;
        if png_data.is_empty() {
            return Err("Active desktop screenshot was empty".to_string());
        }

        use base64::Engine as _;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_inline_screenshot_attachment_captured",
            width,
            height,
            title = "Active desktop",
            bytes = png_data.len(),
        );

        let base64_png = base64::engine::general_purpose::STANDARD.encode(&png_data);
        Ok(Some(ContentBlock::Image(ImageContent::new(
            base64_png,
            "image/png",
        ))))
    }

    /// Resolve pending context parts into Agent Chat blocks plus a standard receipt.
    ///
    /// Most parts resolve into text prompt blocks. Explicit screenshot chips
    /// are upgraded to real Agent Chat attachment blocks first, with the canonical
    /// resource resolver kept as a fallback if image capture fails.
    fn resolve_pending_context_parts_with<F>(
        parts: &[crate::ai::message_parts::AiContextPart],
        mut special_block_resolver: F,
    ) -> ResolvedPendingContext
    where
        F: FnMut(&crate::ai::message_parts::AiContextPart) -> Result<Option<ContentBlock>, String>,
    {
        let mut blocks = Vec::new();
        let mut prompt_blocks = Vec::new();
        let mut failures = Vec::new();

        for part in parts {
            let mut resolved_as_special_block = false;

            match special_block_resolver(part) {
                Ok(Some(block)) => {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_context_part_resolved_to_special_block",
                        source = %part.source(),
                        label = %part.label(),
                    );
                    blocks.push(block);
                    resolved_as_special_block = true;
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_context_special_block_capture_failed",
                        source = %part.source(),
                        label = %part.label(),
                        error = %error,
                    );
                }
            }

            if resolved_as_special_block {
                continue;
            }

            match crate::ai::message_parts::resolve_context_part_to_prompt_block(part, &[], &[]) {
                Ok(block) => {
                    if block.trim().is_empty() {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_context_part_prompt_block_empty",
                            source = %part.source(),
                            label = %part.label(),
                        );
                        continue;
                    }
                    prompt_blocks.push(block);
                }
                Err(err) => {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_context_part_prompt_resolution_failed",
                        source = %part.source(),
                        label = %part.label(),
                        error = %err,
                    );
                    failures.push(crate::ai::message_parts::ContextResolutionFailure {
                        label: part.label().to_string(),
                        source: part.source().to_string(),
                        error: format!("{err:#}"),
                    });
                }
            }
        }

        let resolved = blocks.len() + prompt_blocks.len();
        let prompt_prefix = prompt_blocks.join("\n\n");

        ResolvedPendingContext {
            blocks,
            receipt: crate::ai::message_parts::ContextResolutionReceipt {
                attempted: parts.len(),
                resolved,
                failures,
                prompt_prefix,
            },
        }
    }

    /// Flush a queued submit if conditions allow, otherwise just notify.
    fn flush_bootstrap_queue(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let submit_now = self.queued_submit_while_bootstrapping
            && !self.input.text().trim().is_empty()
            && !matches!(
                self.status,
                AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
            );
        self.queued_submit_while_bootstrapping = false;

        if submit_now {
            return self.submit_input(cx);
        }
        cx.notify();
        Ok(())
    }

    /// Finalize bootstrap state and flush any queued submit.
    fn finish_bootstrap(
        &mut self,
        state: AgentChatContextBootstrapState,
        note: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.context_bootstrap_state = state;
        self.context_bootstrap_note = Some(note.into());
        if let Err(error) = self.flush_bootstrap_queue(cx) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_bootstrap_flush_failed",
                error = %error,
            );
        }
    }

    /// Consume pending context for a single turn. Returns `None` if already
    /// consumed or nothing is staged. Drains both hidden blocks and typed
    /// parts, resolves parts into prompt blocks, and marks context consumed.
    fn take_pending_context_for_turn(&mut self) -> Option<PendingContextTurn> {
        self.take_pending_context_for_turn_with(Self::capture_special_context_block_for_part)
    }

    /// Variant of `take_pending_context_for_turn` that lets tests inject a
    /// deterministic special-block resolver.
    fn take_pending_context_for_turn_with<F>(
        &mut self,
        mut special_block_resolver: F,
    ) -> Option<PendingContextTurn>
    where
        F: FnMut(&crate::ai::message_parts::AiContextPart) -> Result<Option<ContentBlock>, String>,
    {
        let has_pending_parts = !self.pending_context_parts.is_empty();
        let has_pending_blocks = !self.pending_context_blocks.is_empty();

        if self.pending_context_consumed || (!has_pending_parts && !has_pending_blocks) {
            return None;
        }

        let blocks = std::mem::take(&mut self.pending_context_blocks);
        // Clone parts so the chip remains visible after submit.
        // The `pending_context_consumed` flag prevents re-resolution.
        let pending_parts = self.pending_context_parts.clone();
        let consumed_hidden_block_count = blocks.len();
        let consumed_part_count = pending_parts.len();

        let resolved_pending_context = if pending_parts.is_empty() {
            ResolvedPendingContext {
                blocks: Vec::new(),
                receipt: crate::ai::message_parts::ContextResolutionReceipt {
                    attempted: 0,
                    resolved: 0,
                    failures: Vec::new(),
                    prompt_prefix: String::new(),
                },
            }
        } else {
            Self::resolve_pending_context_parts_with(&pending_parts, |part| {
                special_block_resolver(part)
            })
        };
        let consumed_special_block_count = resolved_pending_context.blocks.len();
        let receipt = resolved_pending_context.receipt;
        let mut blocks = blocks;
        blocks.extend(resolved_pending_context.blocks);

        self.pending_context_consumed = true;
        self.pending_ambient_context_enabled = false;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_context_consumed",
            consumed_part_count,
            consumed_hidden_block_count,
            consumed_special_block_count,
            resolved_part_count = receipt.resolved,
            failed_part_count = receipt.failures.len(),
        );

        let attachments = pending_parts
            .iter()
            .map(AgentChatMessageAttachment::from_part)
            .collect();

        Some(PendingContextTurn {
            blocks,
            receipt,
            attachments,
        })
    }

    /// Build the content blocks for a turn, consuming staged context on first use.
    ///
    /// Delegates to `take_pending_context_for_turn()` for one-shot consumption.
    /// When context is present, the user's text is wrapped with a clear
    /// `--- USER REQUEST ---` marker so the agent distinguishes ambient context
    /// from the actual user intent.
    #[cfg_attr(test, allow(dead_code))]
    pub(super) fn prepare_turn_blocks(&mut self, input: &str) -> Vec<ContentBlock> {
        self.prepare_turn_blocks_with_receipt(input).blocks
    }

    fn should_stage_brain_recall(&self) -> bool {
        self.profile_id == crate::ai::agent_chat::profiles::BUILTIN_BRAIN_PROFILE_ID
    }

    /// Build the content blocks for a turn AND return the resolution receipt
    /// so callers can surface partial-failure feedback.
    fn prepare_turn_blocks_with_receipt(&mut self, input: &str) -> PreparedTurnBlocks {
        self.prepare_turn_blocks_with_receipt_using(
            input,
            |query| crate::brain::recall_context_block(query).ok().flatten(),
            crate::brain::record_ask_signals,
        )
    }

    fn prepare_turn_blocks_with_receipt_using<R, S>(
        &mut self,
        input: &str,
        recall_context: R,
        record_ask_signals: S,
    ) -> PreparedTurnBlocks
    where
        R: FnOnce(&str) -> Option<String>,
        S: FnOnce(&str),
    {
        let mut blocks = Vec::new();

        // --- Brain recall: stage relevant local memory for the Brain profile ---
        // Lexical + attention-signal retrieval over the brain store (notes,
        // past chat turns). Milliseconds on sqlite; hard-capped output so it
        // can never crowd a prompt. Empty recall stages nothing.
        if self.should_stage_brain_recall() {
            let brain_block = recall_context(input);
            if let Some(recall) = brain_block {
                tracing::info!(
                    target: "script_kit::brain",
                    event = "agent_chat_brain_recall_staged",
                    profile_id = %self.profile_id,
                    chars = recall.len(),
                );
                blocks.push(ContentBlock::Text(TextContent::new(recall)));
            }
            record_ask_signals(input);
        }

        if let Some(turn) = self.take_pending_context_for_turn() {
            let receipt = turn.receipt;
            let attachments = turn.attachments;
            blocks.extend(turn.blocks);

            if receipt.attempted > 0 {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_submit_resolved_context_parts",
                    attempted = receipt.attempted,
                    resolved = receipt.resolved,
                    failures = receipt.failures.len(),
                );
            }

            if !receipt.prompt_prefix.is_empty() {
                blocks.push(ContentBlock::Text(TextContent::new(
                    receipt.prompt_prefix.clone(),
                )));
            }

            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "--- USER REQUEST ---\n{input}"
            ))));
            return PreparedTurnBlocks {
                blocks,
                receipt: Some(receipt),
                attachments,
            };
        }

        if blocks.is_empty() {
            blocks.push(ContentBlock::Text(TextContent::new(input)));
        } else {
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "--- USER REQUEST ---\n{input}"
            ))));
        }
        PreparedTurnBlocks {
            blocks,
            receipt: None,
            attachments: Vec::new(),
        }
    }

    fn completed_chat_turn_ingest(
        &self,
        history_trace_label: Option<String>,
    ) -> Option<CompletedChatTurnIngest> {
        let user_text = self
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, AgentChatThreadMessageRole::User))
            .map(|m| m.body.to_string())?;
        let assistant_text = self
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, AgentChatThreadMessageRole::Assistant))
            .map(|m| m.body.to_string())
            .unwrap_or_default();
        let trace_label = history_trace_label.unwrap_or_else(|| {
            self.messages
                .iter()
                .find(|m| matches!(m.role, AgentChatThreadMessageRole::User))
                .map(|m| m.body.to_string())
                .unwrap_or_default()
        });
        let turn_index = self
            .messages
            .iter()
            .filter(|m| matches!(m.role, AgentChatThreadMessageRole::User))
            .count()
            .saturating_sub(1);

        Some(CompletedChatTurnIngest {
            thread_id: self.ui_thread_id.clone(),
            turn_index,
            user_text,
            assistant_text,
            trace_label,
        })
    }

    /// Update `context_bootstrap_note` with a partial-failure summary when
    /// some provider-backed mentions failed to resolve.
    fn set_context_resolution_note(
        &mut self,
        receipt: Option<&crate::ai::message_parts::ContextResolutionReceipt>,
    ) {
        let Some(receipt) = receipt else {
            self.context_bootstrap_note = None;
            return;
        };
        if receipt.failures.is_empty() {
            self.context_bootstrap_note = None;
            return;
        }

        let labels: Vec<&str> = receipt
            .failures
            .iter()
            .map(|failure| failure.label.as_str())
            .collect();
        let sources: Vec<&str> = receipt
            .failures
            .iter()
            .map(|failure| failure.source.as_str())
            .collect();

        self.context_bootstrap_note = Some(
            format!(
                "{} context attachment{} unavailable \u{00b7} {}",
                receipt.failures.len(),
                if receipt.failures.len() == 1 { "" } else { "s" },
                labels.join(", "),
            )
            .into(),
        );

        tracing::warn!(
            target: "script_kit::tab_ai",
            event = "agent_chat_context_resolution_partial_failure",
            failure_count = receipt.failures.len(),
            labels = ?labels,
            sources = ?sources,
        );
    }

    /// Spawn a task that pumps events from the Agent Chat worker into thread state.
    fn bind_stream(&mut self, rx: AgentChatEventRx, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();
        let generation = self.transcript_generation;
        self.stream_task = Some(cx.spawn(async move |_this, cx| {
            let mut terminal_event_seen = false;
            while let Ok(event) = rx.recv().await {
                let should_stop = matches!(
                    event,
                    AgentChatEvent::TurnFinished { .. } | AgentChatEvent::Failed { .. }
                );

                let entity_alive = entity.upgrade().is_some();
                if !entity_alive {
                    break;
                }

                let entity_ref = entity.clone();
                cx.update(|cx| {
                    if let Some(entity) = entity_ref.upgrade() {
                        entity.update(cx, |this, cx| {
                            if this.transcript_generation != generation {
                                tracing::debug!(
                                    target: "script_kit::tab_ai",
                                    event = "agent_chat_stream_event_discarded_stale_generation",
                                    expected_generation = generation,
                                    actual_generation = this.transcript_generation,
                                );
                                return;
                            }
                            this.apply_event(event, cx);
                        });
                    }
                });

                if should_stop {
                    terminal_event_seen = true;
                    break;
                }
            }

            if !terminal_event_seen {
                let entity_ref = entity.clone();
                cx.update(|cx| {
                    if let Some(entity) = entity_ref.upgrade() {
                        entity.update(cx, |this, cx| {
                            if this.transcript_generation != generation {
                                return;
                            }
                            if this.finish_stream_closed_without_terminal() {
                                cx.notify();
                            }
                        });
                    }
                });
            }
        }));
    }

    fn bump_transcript_generation(&mut self, reason: &'static str) {
        self.transcript_generation = self.transcript_generation.wrapping_add(1);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_thread_transcript_generation_bumped",
            reason,
            generation = self.transcript_generation,
        );
    }

    /// Spawn a long-lived task that listens for permission requests.
    fn bind_permission_listener(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().downgrade();
        let rx = self.permission_rx.clone();
        self.permission_task = Some(cx.spawn(async move |_this, cx| {
            while let Ok(request) = rx.recv().await {
                let entity_alive = entity.upgrade().is_some();
                if !entity_alive {
                    break;
                }

                let entity_ref = entity.clone();
                cx.update(|cx| {
                    if let Some(entity) = entity_ref.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.status = AgentChatThreadStatus::WaitingForPermission;
                            // Compute the notification body and id from the owned
                            // request before moving it into `pending_permission`,
                            // so we never have to re-borrow-and-expect it back out.
                            let request_id = request.id;
                            let body = this.permission_notification_body(&request);
                            this.pending_permission = Some(request);
                            this.maybe_notify_agent_chat_event(
                                AgentChatNotificationEvent::WaitingForPermission { request_id },
                                "Agent Chat — approval needed",
                                body,
                            );
                            cx.notify();
                        });
                    }
                });
            }
        }));
    }

    fn start_streaming_text_drain_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.streaming_text_buffer.is_empty() || self.streaming_text_drain_task.is_some() {
            return;
        }

        let generation = self.transcript_generation;
        self.streaming_text_drain_task = Some(cx.spawn(async move |this, cx| loop {
            Timer::after(std::time::Duration::from_millis(16)).await;

            let should_continue = this
                .update(cx, |this, cx| {
                    if this.transcript_generation != generation {
                        this.streaming_text_buffer.flush_all();
                        this.streaming_text_drain_task = None;
                        return false;
                    }

                    let changed = this.drain_streaming_text_once();
                    if changed {
                        cx.notify();
                    }

                    if this.streaming_text_buffer.is_empty() {
                        this.streaming_text_drain_task = None;
                        false
                    } else {
                        true
                    }
                })
                .unwrap_or(false);

            if !should_continue {
                break;
            }
        }));
    }

    fn drain_streaming_text_once(&mut self) -> bool {
        let budget = self.streaming_text_buffer.drain_budget_for_tick();
        let Some(delta) = self.streaming_text_buffer.drain_next(budget) else {
            return false;
        };
        self.append_assistant_stream_delta(delta)
    }

    fn append_assistant_stream_delta(&mut self, delta: String) -> bool {
        let accumulated =
            self.accumulated_text_after_append(AgentChatThreadMessageRole::Assistant, &delta);
        let changed = self.append_chunk(AgentChatThreadMessageRole::Assistant, delta.clone());
        if changed {
            crate::ai::subscriptions::publish_stream_chunk(&self.ui_thread_id, delta, accumulated);
        }
        changed
    }

    fn flush_streaming_text_buffer(&mut self) -> bool {
        let delta = self.streaming_text_buffer.flush_all();
        self.streaming_text_drain_task = None;
        if delta.is_empty() {
            return false;
        }
        self.append_assistant_stream_delta(delta)
    }

    /// Apply a single Agent Chat event to thread state.
    ///
    /// Streaming text deltas coalesce into stable messages via `append_chunk`.
    /// Plan, mode, and command updates are persisted in dedicated fields so the
    /// view can render them as first-class UI strips without reparsing text.
    /// Tool calls are tracked by ID and updated in-place.
    ///
    /// Only calls `cx.notify()` when state actually changes, avoiding redundant
    /// repaints for duplicate plan, mode, command, or tool-call updates.
    fn apply_event(&mut self, event: AgentChatEvent, cx: &mut Context<Self>) {
        let mut changed = false;

        match event {
            AgentChatEvent::UserMessageDelta(chunk) => {
                changed |= self.append_chunk(AgentChatThreadMessageRole::System, chunk);
                changed |= self.set_status(AgentChatThreadStatus::Streaming);
            }
            AgentChatEvent::AgentMessageDelta(chunk) => {
                self.log_time_to_first_token("assistant_message");
                if !chunk.is_empty() {
                    self.streaming_text_buffer.push_chunk(chunk);
                    self.start_streaming_text_drain_if_needed(cx);
                }
                changed |= self.set_status(AgentChatThreadStatus::Streaming);
            }
            AgentChatEvent::AgentThoughtDelta(chunk) => {
                self.log_time_to_first_token("agent_thought");
                changed |= self.append_chunk(AgentChatThreadMessageRole::Thought, chunk);
                changed |= self.set_status(AgentChatThreadStatus::Streaming);
            }
            AgentChatEvent::ToolCallStarted {
                tool_call_id,
                title,
                status,
                tool_name,
                raw_input,
            } => {
                changed |=
                    self.upsert_tool_call_start(tool_call_id, title, status, tool_name, raw_input);
                changed |= self.set_status(AgentChatThreadStatus::Streaming);
            }
            AgentChatEvent::ToolCallUpdated {
                tool_call_id,
                title,
                status,
                body,
                raw_input,
                diff,
                is_error,
            } => {
                changed |= self.apply_tool_call_update(
                    tool_call_id,
                    title,
                    status,
                    body,
                    raw_input,
                    diff,
                    is_error,
                );
                changed |= self.set_status(AgentChatThreadStatus::Streaming);
            }
            AgentChatEvent::PlanUpdated { entries } => {
                if self.active_plan_entries != entries {
                    self.active_plan_entries = entries;
                    changed = true;
                }
                changed |= self.set_status(AgentChatThreadStatus::Streaming);
            }
            AgentChatEvent::AvailableCommandsUpdated { command_names } => {
                if self.available_commands != command_names {
                    self.available_commands = command_names;
                    changed = true;
                }
            }
            AgentChatEvent::ModeChanged { mode_id } => {
                if self.active_mode_id.as_deref() != Some(mode_id.as_str()) {
                    self.active_mode_id = Some(mode_id);
                    changed = true;
                }
            }
            AgentChatEvent::UsageUpdated {
                used_tokens,
                context_size,
                cost_usd,
            } => {
                self.usage_tokens = Some((used_tokens, context_size));
                if let Some(cost) = cost_usd {
                    self.usage_cost_usd = Some(cost);
                }
                changed = true;
            }
            AgentChatEvent::ModelsAvailable {
                current_model_id,
                models,
            } => {
                changed |= self.apply_agent_models(current_model_id, models);
            }
            AgentChatEvent::ForkPointsAvailable { entries } => {
                if self.fork_points != entries {
                    self.fork_points = entries;
                    changed = true;
                }
            }
            AgentChatEvent::ForkCompleted { text } => {
                changed |= self.flush_streaming_text_buffer();
                changed |= self.apply_fork_completed(text, cx);
            }
            AgentChatEvent::TurnFinished { .. } => {
                changed |= self.flush_streaming_text_buffer();
                if self.pending_permission.take().is_some() {
                    changed = true;
                }
                changed |= self.set_status(AgentChatThreadStatus::Idle);
                if let Some(message) =
                    self.latest_message_with_role(AgentChatThreadMessageRole::Assistant)
                {
                    let message_id = message.id;
                    let full_content = message.body.to_string();
                    if !full_content.is_empty() {
                        self.maybe_notify_agent_chat_event(
                            AgentChatNotificationEvent::TurnFinished,
                            "Agent Chat — response ready",
                            full_content.clone(),
                        );
                        crate::ai::subscriptions::publish_stream_complete(
                            &self.ui_thread_id,
                            message_id.to_string(),
                            full_content.clone(),
                            None,
                        );
                        self.publish_sdk_new_message(
                            message_id,
                            AgentChatThreadMessageRole::Assistant,
                            full_content,
                        );
                    }
                }

                // Save conversation summary + full messages to history.
                // Build a rich index entry from the full conversation so
                // search_history() can match on later transcript content.
                let history_trace_label = if self
                    .messages
                    .iter()
                    .any(|m| matches!(m.role, AgentChatThreadMessageRole::User))
                {
                    let timestamp = chrono::Utc::now().to_rfc3339();
                    let existing_custom_title =
                        super::history::load_conversation(&self.ui_thread_id)
                            .and_then(|conversation| conversation.custom_title);
                    let conversation = super::history::SavedConversation {
                        session_id: self.ui_thread_id.clone(),
                        timestamp,
                        custom_title: existing_custom_title.clone(),
                        messages: self
                            .messages
                            .iter()
                            .map(|m| super::history::SavedMessage {
                                role: format!("{:?}", m.role),
                                body: m.body.to_string(),
                            })
                            .collect(),
                    };
                    super::history::save_conversation(&conversation);
                    self.maybe_spawn_auto_title(&conversation);

                    super::history::build_history_entry(&conversation).map(|entry| {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_history_index_entry_built",
                            session_id = %entry.session_id,
                            title = %entry.title_display(),
                            preview_len = entry.preview.len(),
                            message_count = entry.message_count,
                        );
                        super::history::save_history_entry(&entry);
                        entry.title_display().to_string()
                    })
                } else {
                    None
                };

                // The finished turn appended a user message; refresh the
                // rewind checkpoints so Cmd+K can offer it for editing.
                self.refresh_fork_points(cx);

                // --- Brain ingestion: every finished turn becomes memory ---
                // The last user/assistant exchange is written into the brain
                // store (hash-guarded, idempotent) on a background thread so
                // future turns — in any thread — can recall it.
                if let Some(payload) = self.completed_chat_turn_ingest(history_trace_label) {
                    let _ = std::thread::Builder::new()
                        .name("script-kit-brain-ingest".to_string())
                        .spawn(move || {
                            crate::brain::day_trace::maybe_append_agent_chat_trace(
                                &payload.thread_id,
                                &payload.trace_label,
                            );
                            if let Err(error) = crate::brain::ingest_chat_turn(
                                &payload.thread_id,
                                payload.turn_index,
                                &payload.user_text,
                                &payload.assistant_text,
                            ) {
                                tracing::debug!(
                                    target: "script_kit::brain",
                                    error = %error,
                                    "brain chat ingestion failed"
                                );
                            }
                        });
                }
                if let Err(error) = self.submit_next_queued_if_ready(cx) {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_queue_auto_submit_failed",
                        error = %error,
                    );
                }
            }
            AgentChatEvent::SetupRequired {
                reason,
                auth_methods,
            } => {
                let current_requirements = self.current_setup_requirements();
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_runtime_setup_session_armed",
                    reason = %reason,
                    auth_method_count = auth_methods.len(),
                    selected_agent_id = self.selected_agent.as_ref().map(|a| a.id.as_ref()),
                    available_agent_count = self.available_agents.len(),
                    needs_embedded_context = current_requirements.needs_embedded_context,
                    needs_image = current_requirements.needs_image,
                );
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_runtime_setup_requirements_preserved",
                    original_needs_embedded_context = self.launch_requirements.needs_embedded_context,
                    original_needs_image = self.launch_requirements.needs_image,
                    current_needs_embedded_context = current_requirements.needs_embedded_context,
                    current_needs_image = current_requirements.needs_image,
                );
                self.setup_state = Some(
                    super::setup_state::AgentChatInlineSetupState::from_runtime_setup_required(
                        self.selected_agent.clone(),
                        self.available_agents.clone(),
                        current_requirements,
                        &reason,
                        &auth_methods,
                    ),
                );
                changed |= self.set_status(AgentChatThreadStatus::Error);
            }
            AgentChatEvent::Failed { error } => {
                let _ = self.flush_streaming_text_buffer();
                self.maybe_notify_agent_chat_event(
                    AgentChatNotificationEvent::Failed,
                    "Agent Chat — turn failed",
                    error.clone(),
                );
                crate::ai::subscriptions::publish_error(
                    Some(&self.ui_thread_id),
                    "AGENT_CHAT_TURN_FAILED".to_string(),
                    error.clone(),
                );
                let _ = self.pending_permission.take();
                let can_retry = self.last_user_turn_text().is_some();
                let callout = AgentChatCallout::failed(error, can_retry);
                let transcript_message = callout
                    .detail
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "The provider could not complete this turn.".to_string());
                self.active_callout = Some(callout);
                changed = true;
                changed |= self.push_message(AgentChatThreadMessageRole::Error, transcript_message);
                changed |= self.set_status(AgentChatThreadStatus::Error);
            }
        }

        if changed {
            cx.notify();
        }
    }

    fn finish_stream_closed_without_terminal(&mut self) -> bool {
        if !matches!(
            self.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) {
            return false;
        }

        self.flush_streaming_text_buffer();
        let had_pending_permission = self.pending_permission.take().is_some();
        let assistant = self.latest_message_with_role(AgentChatThreadMessageRole::Assistant);
        let assistant_id = assistant.map(|message| message.id);
        let assistant_text = assistant
            .map(|message| message.body.to_string())
            .unwrap_or_default();
        let had_assistant_text = !assistant_text.trim().is_empty();

        tracing::warn!(
            target: "script_kit::tab_ai",
            event = "agent_chat_stream_closed_without_terminal_event",
            ui_thread = %self.ui_thread_id,
            had_pending_permission,
            had_assistant_text,
            message_count = self.messages.len(),
        );

        if had_assistant_text {
            self.status = AgentChatThreadStatus::Idle;
            if let Some(message_id) = assistant_id {
                crate::ai::subscriptions::publish_stream_complete(
                    &self.ui_thread_id,
                    message_id.to_string(),
                    assistant_text.clone(),
                    None,
                );
                self.publish_sdk_new_message(
                    message_id,
                    AgentChatThreadMessageRole::Assistant,
                    assistant_text,
                );
            }
        } else {
            let message = "Agent stream ended before sending a completion event.".to_string();
            self.push_message(AgentChatThreadMessageRole::Error, message);
            self.status = AgentChatThreadStatus::Error;
        }

        true
    }

    /// One-shot time-to-first-token log for the current turn. Makes warm/cold
    /// launch latency measurable from logs (`agent_chat_time_to_first_token`).
    fn log_time_to_first_token(&mut self, first_event: &'static str) {
        if !self.ttft_pending {
            return;
        }
        self.ttft_pending = false;
        let Some(started_at) = self.stream_started_at else {
            return;
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_time_to_first_token",
            ui_thread_id = %self.ui_thread_id,
            first_event,
            elapsed_ms = started_at.elapsed().as_millis() as u64,
        );
    }

    /// Set the thread status, returning `true` if it actually changed.
    fn set_status(&mut self, next: AgentChatThreadStatus) -> bool {
        if self.status == next {
            return false;
        }
        // Track streaming start time.
        if matches!(next, AgentChatThreadStatus::Streaming)
            && !matches!(self.status, AgentChatThreadStatus::Streaming)
        {
            self.stream_started_at = Some(std::time::Instant::now());
        } else if !matches!(next, AgentChatThreadStatus::Streaming) {
            self.stream_started_at = None;
        }
        self.status = next;
        true
    }

    /// Push a new message with an auto-allocated ID. Returns `true` always.
    fn push_message(
        &mut self,
        role: AgentChatThreadMessageRole,
        body: impl Into<SharedString>,
    ) -> bool {
        let id = self.alloc_id();
        self.messages
            .push(AgentChatThreadMessage::new(id, role, body));
        true
    }

    fn sdk_role(role: AgentChatThreadMessageRole) -> Option<&'static str> {
        match role {
            AgentChatThreadMessageRole::User => Some("user"),
            AgentChatThreadMessageRole::Assistant => Some("assistant"),
            AgentChatThreadMessageRole::System => Some("system"),
            AgentChatThreadMessageRole::Error => Some("system"),
            AgentChatThreadMessageRole::Thought | AgentChatThreadMessageRole::Tool => None,
        }
    }

    fn publish_sdk_new_message(&self, id: u64, role: AgentChatThreadMessageRole, content: String) {
        let Some(role) = Self::sdk_role(role) else {
            return;
        };
        crate::ai::subscriptions::publish_new_message(
            &self.ui_thread_id,
            AiMessageInfo {
                id: id.to_string().into(),
                role: role.to_string().into(),
                content: content.into(),
                created_at: chrono::Utc::now().to_rfc3339(),
                tokens_used: None,
            },
        );
    }

    fn latest_message_with_role(
        &self,
        role: AgentChatThreadMessageRole,
    ) -> Option<&AgentChatThreadMessage> {
        self.messages
            .iter()
            .rev()
            .find(|message| message.role == role)
    }

    fn accumulated_text_after_append(
        &self,
        role: AgentChatThreadMessageRole,
        chunk: &str,
    ) -> String {
        let mut accumulated = self
            .messages
            .last()
            .filter(|message| message.role == role)
            .map(|message| message.body.to_string())
            .unwrap_or_default();
        accumulated.push_str(chunk);
        accumulated
    }

    /// Append a text chunk to the last message if it has the same role,
    /// otherwise create a new message. This coalesces streaming deltas.
    /// Returns `true` if state changed (i.e. chunk was non-empty).
    fn append_chunk(&mut self, role: AgentChatThreadMessageRole, chunk: String) -> bool {
        if chunk.is_empty() {
            return false;
        }
        if let Some(last) = self.messages.last_mut() {
            if last.role == role {
                let mut text = last.body.to_string();
                text.push_str(&chunk);
                last.body = text.into();
                return true;
            }
        }
        let id = self.alloc_id();
        self.messages
            .push(AgentChatThreadMessage::new(id, role, chunk));
        true
    }

    /// Re-derive the formatted body and structured card metadata for the
    /// message backing a tool call slot.
    fn sync_tool_call_message(&mut self, slot: usize) {
        let tool_call = &self.active_tool_calls[slot];
        let new_body =
            Self::format_tool_call_body(&tool_call.title, &tool_call.status, &tool_call.body);
        let meta = tool_call.card_meta();
        let message_index = tool_call.message_index;
        if let Some(msg) = self.messages.get_mut(message_index) {
            msg.body = new_body.into();
            msg.tool_meta = Some(meta);
        }
    }

    /// Insert or update a tool call from a `ToolCallStarted` event.
    /// Uses `tool_call_lookup` for O(1) access. Returns `true` if state changed.
    fn upsert_tool_call_start(
        &mut self,
        tool_call_id: String,
        title: String,
        status: String,
        tool_name: Option<String>,
        raw_input: Option<serde_json::Value>,
    ) -> bool {
        let subject = raw_input
            .as_ref()
            .and_then(super::tool_card::subject_from_args);
        if let Some(&slot) = self.tool_call_lookup.get(&tool_call_id) {
            let existing = &mut self.active_tool_calls[slot];
            let mut changed = false;
            if existing.title != title {
                existing.title = title;
                changed = true;
            }
            if existing.status != status {
                existing.status = status;
                changed = true;
            }
            if tool_name.is_some() && existing.tool_name != tool_name {
                existing.tool_name = tool_name;
                changed = true;
            }
            if subject.is_some() && existing.subject != subject {
                existing.subject = subject;
                changed = true;
            }
            if changed {
                self.sync_tool_call_message(slot);
            }
            return changed;
        }

        let message_id = self.alloc_id();
        let message_index = self.messages.len();
        let message_body = format!("{title}\n{status}");
        self.messages
            .push(AgentChatThreadMessage::with_tool_call_id(
                message_id,
                AgentChatThreadMessageRole::Tool,
                message_body,
                tool_call_id.clone(),
            ));

        let slot = self.active_tool_calls.len();
        self.active_tool_calls.push(AgentChatToolCallState {
            tool_call_id: tool_call_id.clone(),
            title,
            status,
            body: None,
            tool_name,
            subject,
            diff: None,
            is_error: false,
            message_id,
            message_index,
        });
        self.tool_call_lookup.insert(tool_call_id, slot);
        self.sync_tool_call_message(slot);
        true
    }

    /// Apply a `ToolCallUpdated` event, updating tracked state and message in-place.
    /// Uses `tool_call_lookup` for O(1) access. Returns `true` if state changed.
    #[allow(clippy::too_many_arguments)]
    fn apply_tool_call_update(
        &mut self,
        tool_call_id: String,
        title: Option<String>,
        status: Option<String>,
        body: Option<String>,
        raw_input: Option<serde_json::Value>,
        diff: Option<String>,
        is_error: bool,
    ) -> bool {
        let subject = raw_input
            .as_ref()
            .and_then(super::tool_card::subject_from_args);
        if let Some(&slot) = self.tool_call_lookup.get(&tool_call_id) {
            let tool_call = &mut self.active_tool_calls[slot];
            let mut changed = false;

            if let Some(title) = title {
                if tool_call.title != title {
                    tool_call.title = title;
                    changed = true;
                }
            }
            if let Some(status) = status {
                if tool_call.status != status {
                    tool_call.status = status;
                    changed = true;
                }
            }
            if let Some(body) = body {
                if tool_call.body.as_deref() != Some(body.as_str()) {
                    tool_call.body = Some(body);
                    changed = true;
                }
            }
            if subject.is_some() && tool_call.subject != subject {
                tool_call.subject = subject;
                changed = true;
            }
            if diff.is_some() && tool_call.diff != diff {
                tool_call.diff = diff;
                changed = true;
            }
            if is_error && !tool_call.is_error {
                tool_call.is_error = true;
                changed = true;
            }

            if changed {
                self.sync_tool_call_message(slot);
            }
            return changed;
        }

        // Orphan update — create a standalone tool call entry.
        let title = title.unwrap_or_else(|| "Tool".to_string());
        let status = status.unwrap_or_else(|| "running".to_string());
        let message_id = self.alloc_id();
        let message_index = self.messages.len();
        let message_body = Self::format_tool_call_body(&title, &status, &body);
        self.messages
            .push(AgentChatThreadMessage::with_tool_call_id(
                message_id,
                AgentChatThreadMessageRole::Tool,
                message_body,
                tool_call_id.clone(),
            ));

        let slot = self.active_tool_calls.len();
        self.active_tool_calls.push(AgentChatToolCallState {
            tool_call_id: tool_call_id.clone(),
            title,
            status,
            body,
            tool_name: None,
            subject,
            diff,
            is_error,
            message_id,
            message_index,
        });
        self.tool_call_lookup.insert(tool_call_id, slot);
        self.sync_tool_call_message(slot);
        true
    }

    /// Allocate a unique message ID.
    fn alloc_id(&mut self) -> u64 {
        let id = self.next_message_id;
        self.next_message_id += 1;
        id
    }

    /// Format a tool call message body from tracked state.
    fn format_tool_call_body(title: &str, status: &str, body: &Option<String>) -> String {
        match body {
            Some(b) => format!("{title}\n{status}\n{b}"),
            None => format!("{title}\n{status}"),
        }
    }

    // ── Public accessors for structured session state ──────────────

    /// Capability requirements preserved from the original launch context.
    pub(crate) fn launch_requirements(&self) -> super::preflight::AgentChatLaunchRequirements {
        self.launch_requirements
    }

    /// Currently selected Agent Chat agent ID for this live thread.
    pub(crate) fn selected_agent_id(&self) -> Option<&str> {
        self.selected_agent.as_ref().map(|agent| agent.id.as_ref())
    }

    /// Replace the selected agent on the live thread (used during runtime
    /// recovery when the user picks a different agent in the setup card).
    pub(crate) fn replace_selected_agent(
        &mut self,
        next: Option<super::catalog::AgentChatAgentCatalogEntry>,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_thread_selected_agent_replaced",
            selected_agent_id = next.as_ref().map(|a| a.id.as_ref()),
            needs_embedded_context = self.launch_requirements.needs_embedded_context,
            needs_image = self.launch_requirements.needs_image,
        );
        self.selected_agent = next;
        cx.notify();
    }

    /// Runtime setup state armed by `AgentChatEvent::SetupRequired`.
    /// When `Some`, the view should render the inline setup recovery card.
    pub(crate) fn setup_state(&self) -> Option<&super::setup_state::AgentChatInlineSetupState> {
        self.setup_state.as_ref()
    }

    /// Derive runtime setup requirements from the live thread state.
    ///
    /// Unions the original `launch_requirements` with the current pending
    /// context parts and blocks so that later-added `@screenshot` or context
    /// chips are reflected when the thread re-enters `SetupRequired`.
    pub(crate) fn current_setup_requirements(
        &self,
    ) -> super::preflight::AgentChatLaunchRequirements {
        let needs_embedded_context = self.launch_requirements.needs_embedded_context
            || !self.pending_context_parts.is_empty()
            || !self.pending_context_blocks.is_empty();

        let needs_image = self.launch_requirements.needs_image
            || self
                .pending_context_parts
                .iter()
                .any(Self::is_explicit_screenshot_part);

        super::preflight::AgentChatLaunchRequirements {
            needs_embedded_context,
            needs_image,
        }
    }

    /// Replace the runtime setup state (used by the view after agent re-selection).
    pub(crate) fn replace_setup_state(
        &mut self,
        next: super::setup_state::AgentChatInlineSetupState,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_thread_setup_state_replaced",
            title = %next.title,
            selected_agent_id = next.selected_agent.as_ref().map(|a| a.id.as_ref()),
            catalog_count = next.catalog_entries.len(),
        );
        self.setup_state = Some(next);
        cx.notify();
    }

    /// Full agent catalog for runtime recovery.
    pub(crate) fn available_agents(&self) -> &[super::catalog::AgentChatAgentCatalogEntry] {
        &self.available_agents
    }

    /// Current plan entries from the latest `PlanUpdated` event.
    pub(crate) fn active_plan_entries(&self) -> &[String] {
        &self.active_plan_entries
    }

    /// Current agent mode ID (e.g. "code", "architect").
    pub(crate) fn active_mode_id(&self) -> Option<&str> {
        self.active_mode_id.as_deref()
    }

    /// Current available commands from the agent.
    pub(crate) fn available_commands(&self) -> &[String] {
        &self.available_commands
    }

    /// Unique UI thread identifier (used for automation window targeting).
    pub(crate) fn ui_thread_id(&self) -> &str {
        &self.ui_thread_id
    }

    /// Display name for the agent (e.g. "Claude Code").
    pub(crate) fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Display name for the selected Agent Chat profile, or the agent name.
    pub(crate) fn profile_display(&self) -> &str {
        self.profile_display_name
            .as_deref()
            .unwrap_or(&self.display_name)
    }

    pub(crate) fn profile_icon_name(&self) -> Option<&str> {
        self.profile_icon_name.as_deref()
    }

    pub(crate) fn set_profile_display(
        &mut self,
        profile_id: String,
        profile_display_name: SharedString,
        profile_icon_name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.profile_id = profile_id;
        self.profile_display_name = Some(profile_display_name);
        self.profile_icon_name = profile_icon_name;
        cx.notify();
    }

    /// Short display name for the currently selected model, or the agent name if none selected.
    pub(crate) fn selected_model_display(&self) -> &str {
        self.selected_model_display_name
            .as_deref()
            .unwrap_or(&self.display_name)
    }

    /// Available models for this agent.
    pub(crate) fn available_models(&self) -> &[super::config::AgentChatModelEntry] {
        &self.available_models
    }

    /// Replace the available model list with the agent's live advertisement
    /// from `session/new`. Preserves the user's current selection if it is
    /// still in the new list; otherwise falls back to the agent's declared
    /// current model, or the first entry. Returns `true` if anything changed.
    fn apply_agent_models(
        &mut self,
        current_model_id: Option<String>,
        models: Vec<super::config::AgentChatModelEntry>,
    ) -> bool {
        if models.is_empty() {
            return false;
        }

        let mut changed = self.available_models != models;
        self.available_models = models;

        let selection_still_valid = self
            .selected_model_id
            .as_deref()
            .map(|sel| self.available_models.iter().any(|m| m.id == sel))
            .unwrap_or(false);

        if !selection_still_valid {
            let fallback = current_model_id
                .as_deref()
                .and_then(|id| self.available_models.iter().find(|m| m.id == id))
                .or_else(|| self.available_models.first());
            if let Some(entry) = fallback {
                self.selected_model_id = Some(entry.id.clone());
                self.selected_model_display_name = Some(SharedString::from(
                    entry
                        .display_name
                        .clone()
                        .unwrap_or_else(|| entry.id.clone()),
                ));
                changed = true;
            }
        }

        changed
    }

    /// Currently selected model ID, if any.
    pub(crate) fn selected_model_id(&self) -> Option<&str> {
        self.selected_model_id.as_deref()
    }

    /// Start a view-owned auxiliary turn that must not mutate this thread's
    /// transcript, status, permissions, or stream_task.
    ///
    /// Focused-text variation generation uses this for conservative/creative
    /// candidates while the balanced candidate remains the canonical thread turn.
    pub(crate) fn start_auxiliary_turn(
        &self,
        ui_thread_id: String,
        blocks: Vec<ContentBlock>,
    ) -> Result<crate::ai::agent_chat::runtime::IsolatedTurnHandle, String> {
        self.connection
            .start_isolated_turn(AgentChatTurnRequest {
                ui_thread_id,
                cwd: self.cwd.clone(),
                blocks,
                model_id: self.selected_model_id.clone(),
            })
            .map_err(|error| error.to_string())
    }

    /// Fire-and-forget: ask the Agent Chat worker to create-or-reuse the session for
    /// this thread and emit a fresh `ModelsAvailable` event. Called when the
    /// user invokes the actions dialog so the Change Model picker reflects the
    /// agent's live catalog (including models released after the hardcoded
    /// fallback was written).
    ///
    /// If the worker is unreachable the call is a no-op; the picker will fall
    /// back to whatever `available_models` already held.
    /// User messages the live session can rewind to, in conversation order.
    pub(crate) fn fork_points(&self) -> &[super::events::AgentChatForkPoint] {
        &self.fork_points
    }

    /// Resolve the Pi fork point for a transcript user message. Primary mapping
    /// is conversation ordinal: nth visible user message maps to nth Pi fork
    /// point. If the local transcript and Pi fork list are out of sync, fall
    /// back to exact user-message text matching.
    pub(crate) fn fork_point_for_message_id<'a>(
        messages: &[AgentChatThreadMessage],
        fork_points: &'a [super::events::AgentChatForkPoint],
        message_id: u64,
    ) -> Option<&'a super::events::AgentChatForkPoint> {
        let user_messages: Vec<&AgentChatThreadMessage> = messages
            .iter()
            .filter(|message| matches!(message.role, AgentChatThreadMessageRole::User))
            .collect();
        let user_ordinal = user_messages
            .iter()
            .position(|message| message.id == message_id)?;

        if user_messages.len() == fork_points.len() {
            return fork_points.get(user_ordinal);
        }

        let user_text = user_messages[user_ordinal].body.as_ref();
        fork_points.iter().find(|point| point.text == user_text)
    }

    /// Refresh the rewindable user-message list from the agent session.
    /// No-op (with a debug log) for connections without rewind support.
    pub(crate) fn refresh_fork_points(&mut self, cx: &mut Context<Self>) {
        let rx = match self.connection.fork_points() {
            Ok(rx) => rx,
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_fork_points_unsupported",
                    error = %error,
                );
                return;
            }
        };
        self.spawn_fork_event_task(rx, "fork_points", cx);
    }

    /// Rewind the session to just before the given user message. On
    /// completion the transcript truncates at that message and the composer
    /// is prefilled with its text for editing. Rejected while a turn is
    /// streaming or another rewind is in flight.
    pub(crate) fn fork_to_message(&mut self, entry_id: &str, cx: &mut Context<Self>) -> bool {
        if matches!(
            self.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_fork_rejected_busy",
                status = ?self.status,
            );
            return false;
        }
        if self.pending_fork_ordinal.is_some() {
            return false;
        }
        let Some(ordinal) = self
            .fork_points
            .iter()
            .position(|point| point.entry_id == entry_id)
        else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_fork_unknown_entry",
                entry_id,
            );
            return false;
        };
        let rx = match self.connection.fork_to_entry(entry_id.to_string()) {
            Ok(rx) => rx,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_fork_request_failed",
                    error = %error,
                );
                return false;
            }
        };
        self.pending_fork_ordinal = Some(ordinal);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_fork_requested",
            entry_id,
            ordinal,
        );
        self.spawn_fork_event_task(rx, "fork", cx);
        cx.notify();
        true
    }

    /// Pump fork RPC responses into `apply_event`, downgrading failures to a
    /// system note: a failed background refresh or rewind must not flip the
    /// thread into the turn-failure error state.
    fn spawn_fork_event_task(
        &self,
        rx: AgentChatEventRx,
        context_label: &'static str,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Ok(event) = rx.recv().await {
                let Some(weak) = entity.upgrade() else {
                    break;
                };
                cx.update(|cx| {
                    weak.update(cx, |this, cx| {
                        let is_fork_event = matches!(
                            event,
                            AgentChatEvent::ForkPointsAvailable { .. }
                                | AgentChatEvent::ForkCompleted { .. }
                        );
                        if is_fork_event {
                            this.apply_event(event, cx);
                        } else if let AgentChatEvent::Failed { error } = event {
                            tracing::warn!(
                                target: "script_kit::tab_ai",
                                event = "agent_chat_fork_rpc_failed",
                                context = context_label,
                                error = %error,
                            );
                            if this.pending_fork_ordinal.take().is_some() {
                                this.push_system_message(format!("Rewind failed: {error}"), cx);
                            }
                        }
                    });
                });
            }
        })
        .detach();
    }

    /// Apply a completed session rewind: truncate the transcript at the
    /// forked user message and stage its text in the composer for editing.
    fn apply_fork_completed(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        let Some(ordinal) = self.pending_fork_ordinal.take() else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_fork_completed_without_request",
            );
            return false;
        };
        Self::truncate_messages_at_user_ordinal(&mut self.messages, ordinal);
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.transcript_generation = self.transcript_generation.wrapping_add(1);
        self.input.set_text(text.clone());
        self.input.set_cursor(text.chars().count());
        self.set_status(AgentChatThreadStatus::Idle);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_fork_completed",
            ordinal,
            message_count = self.messages.len(),
            prefill_chars = text.chars().count(),
        );
        // Pi rebuilt the session with fresh entry ids; refetch the list.
        self.fork_points.clear();
        self.refresh_fork_points(cx);
        true
    }

    /// Drop the `ordinal`-th user message and everything after it.
    fn truncate_messages_at_user_ordinal(
        messages: &mut Vec<AgentChatThreadMessage>,
        ordinal: usize,
    ) {
        let mut seen = 0usize;
        for index in 0..messages.len() {
            if matches!(messages[index].role, AgentChatThreadMessageRole::User) {
                if seen == ordinal {
                    messages.truncate(index);
                    return;
                }
                seen += 1;
            }
        }
    }

    pub(crate) fn refresh_models(&mut self, cx: &mut Context<Self>) {
        let rx = match self
            .connection
            .prepare_session(self.ui_thread_id.clone(), self.cwd.clone())
        {
            Ok(rx) => rx,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_refresh_models_channel_closed",
                    ui_thread = %self.ui_thread_id,
                    error = %error,
                );
                return;
            }
        };
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_refresh_models_requested",
            ui_thread = %self.ui_thread_id,
        );

        let entity = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            while let Ok(event) = rx.recv().await {
                let Some(weak) = entity.upgrade() else {
                    break;
                };
                cx.update(|cx| {
                    weak.update(cx, |this, cx| {
                        this.apply_event(event, cx);
                    });
                });
            }
        })
        .detach();
    }

    /// Select a model by ID. Updates the display name, persists to config, and notifies.
    pub(crate) fn select_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        if let Some(entry) = self.available_models.iter().find(|m| m.id == model_id) {
            self.selected_model_id = Some(entry.id.clone());
            self.selected_model_display_name = Some(SharedString::from(
                entry
                    .display_name
                    .clone()
                    .unwrap_or_else(|| entry.id.clone()),
            ));

            // Persist selection to config.ts (non-fatal).
            let id = entry.id.clone();
            std::thread::Builder::new()
                .name("agent_chat-save-model".into())
                .spawn(move || {
                    let mut prefs = crate::config::load_user_preferences();
                    prefs.ai.selected_model_id = Some(id.clone());
                    if let Err(e) = crate::config::save_user_preferences(&prefs) {
                        tracing::warn!(error = %e, "failed_to_persist_model_selection");
                    } else {
                        tracing::info!(model = %id, "model_selection_persisted");
                    }
                })
                .ok();

            cx.notify();
        }
    }

    pub(crate) fn toggle_favorite_model(&mut self, model_id: &str, cx: &mut Context<Self>) {
        super::favorite_models::toggle_favorite_model_id(model_id);
        cx.notify();
    }

    pub(crate) fn cycle_favorite_model(&mut self, cx: &mut Context<Self>) {
        let favorites = super::favorite_models::load_favorite_model_ids();
        if let Some(model_id) = super::favorite_models::next_favorite_model_id(
            self.selected_model_id(),
            &favorites,
            self.available_models(),
        ) {
            self.select_model(&model_id, cx);
        }
    }

    /// Elapsed seconds since streaming started, or `None` if not streaming.
    pub(crate) fn stream_elapsed_secs(&self) -> Option<u64> {
        self.stream_started_at.map(|t| t.elapsed().as_secs())
    }

    /// Add a system message to the thread (visible in the chat, not sent to the agent).
    pub(crate) fn push_system_message(
        &mut self,
        body: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.push_message(AgentChatThreadMessageRole::System, body);
        cx.notify();
    }

    /// Clear all messages for a fresh conversation within the same session.
    /// Also clears all pending context state so no stale chips or hidden
    /// blocks leak into the next conversation.
    pub(crate) fn clear_messages(&mut self, cx: &mut Context<Self>) {
        self.flush_streaming_text_buffer();
        self.messages.clear();
        self.active_plan_entries.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.clear_all_pending_context("clear_messages");
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_thread_cleared",
        );
        cx.notify();
    }

    /// Install a synthetic transcript state for no-token Agent Chat UI proof.
    pub(crate) fn apply_test_fixture(
        &mut self,
        phase: &str,
        user_text: Option<String>,
        assistant_text: Option<String>,
        message_count: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.flush_streaming_text_buffer();
        let user_text = user_text.unwrap_or_else(|| "No-token activity fixture".to_string());
        let user_text = user_text.trim();
        if user_text.is_empty() {
            return Err("setAgentChatTestFixture requires non-empty userText".to_string());
        }

        self.stream_task = None;
        self.pending_permission = None;
        self.active_callout = None;
        self.messages.clear();
        self.active_plan_entries.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.standing_approvals.clear();
        self.active_mode_id = None;
        self.available_commands.clear();
        self.usage_tokens = None;
        self.usage_cost_usd = None;
        self.input.clear();
        self.clear_all_pending_context("set_agent_chat_test_fixture");
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        self.queued_submit_while_bootstrapping = false;

        if let Some(message_count) = message_count {
            let message_count = message_count.clamp(1, 2_000);
            let assistant_text = assistant_text
                .as_deref()
                .unwrap_or("Fixture assistant text with enough markdown to exercise layout.");
            for index in 0..message_count {
                let role = if index % 2 == 0 {
                    AgentChatThreadMessageRole::User
                } else {
                    AgentChatThreadMessageRole::Assistant
                };
                let seed = if matches!(role, AgentChatThreadMessageRole::User) {
                    user_text
                } else {
                    assistant_text
                };
                let body = format!(
                    "{seed}\n\n### Fixture turn {turn}\n\n- row: {row}\n- purpose: rapid transcript scroll performance\n- repeated detail: alpha beta gamma delta epsilon zeta eta theta\n\n```text\nfixture block {row}\nline one with enough width to require text layout\nline two with stable markdown parsing\n```\n\nThis paragraph intentionally gives TextViewState markdown a non-trivial body while keeping the fixture deterministic.",
                    turn = index / 2 + 1,
                    row = index + 1,
                );
                self.push_message(role, body);
            }
            self.set_status(AgentChatThreadStatus::Idle);
        } else {
            self.push_message(AgentChatThreadMessageRole::User, user_text.to_string());
            match phase {
                "awaitingFirstAssistantText" | "awaiting-first-assistant-text" | "awaiting" => {
                    self.set_status(AgentChatThreadStatus::Streaming);
                }
                "assistantText" | "assistant-text" | "text" => {
                    self.push_message(
                        AgentChatThreadMessageRole::Assistant,
                        assistant_text.unwrap_or_else(|| "Fixture assistant text.".to_string()),
                    );
                    self.set_status(AgentChatThreadStatus::Streaming);
                }
                "idle" => {
                    if let Some(text) = assistant_text {
                        self.push_message(AgentChatThreadMessageRole::Assistant, text);
                    }
                    self.set_status(AgentChatThreadStatus::Idle);
                }
                "error" | "provider-error" => {
                    let error =
                        assistant_text.unwrap_or_else(|| "Fixture provider error".to_string());
                    let callout = AgentChatCallout::failed(error, true);
                    let transcript_message = callout
                        .detail
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| {
                            "The provider could not complete this turn.".to_string()
                        });
                    self.active_callout = Some(callout);
                    self.push_message(AgentChatThreadMessageRole::Error, transcript_message);
                    self.set_status(AgentChatThreadStatus::Error);
                }
                other => {
                    return Err(format!(
                        "unknown setAgentChatTestFixture phase {other:?}; expected awaitingFirstAssistantText, assistantText, idle, or error"
                    ));
                }
            }
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_test_fixture_applied",
            phase,
            requested_message_count = message_count.unwrap_or(0),
            message_count = self.messages.len(),
            awaiting_first_assistant_text = self.awaiting_first_assistant_text(),
        );
        cx.notify();
        Ok(())
    }

    pub(crate) fn load_kitchen_sink_fixture(&mut self, cx: &mut Context<Self>) {
        use crate::ai::agent_chat::ui::kitchen_sink_fixture::{
            agent_chat_kitchen_sink_fixture, AgentChatKitchenSinkFixtureRole,
        };

        self.flush_streaming_text_buffer();
        let fixture = agent_chat_kitchen_sink_fixture();
        self.stream_task = None;
        self.pending_permission = None;
        self.messages.clear();
        self.active_plan_entries.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.standing_approvals.clear();
        self.active_mode_id = None;
        self.available_commands.clear();
        self.usage_tokens = None;
        self.usage_cost_usd = None;
        self.input.clear();
        self.next_message_id = 1;
        self.clear_all_pending_context("load_kitchen_sink_fixture");
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        self.queued_submit_while_bootstrapping = false;

        for message in fixture.messages {
            let role = match message.role {
                AgentChatKitchenSinkFixtureRole::User => AgentChatThreadMessageRole::User,
                AgentChatKitchenSinkFixtureRole::Assistant => AgentChatThreadMessageRole::Assistant,
                AgentChatKitchenSinkFixtureRole::Thought => AgentChatThreadMessageRole::Thought,
                AgentChatKitchenSinkFixtureRole::Tool => AgentChatThreadMessageRole::Tool,
                AgentChatKitchenSinkFixtureRole::System => AgentChatThreadMessageRole::System,
                AgentChatKitchenSinkFixtureRole::Error => AgentChatThreadMessageRole::Error,
            };
            let body = message.body.to_string();
            if let Some(tool_call_id) = message.tool_call_id {
                // Tool rows with structured fixture meta go through the real
                // tool-call event path so the kitchen sink exercises the
                // production card pipeline (kind, status, subject, diff).
                if let Some(meta) =
                    crate::ai::agent_chat::ui::kitchen_sink_fixture::kitchen_sink_tool_meta(
                        tool_call_id,
                    )
                {
                    let mut lines = body.lines();
                    let title = lines.next().unwrap_or("Tool").to_string();
                    let status = lines.next().unwrap_or("running").to_string();
                    let output = lines.collect::<Vec<_>>().join("\n").trim().to_string();
                    self.upsert_tool_call_start(
                        tool_call_id.to_string(),
                        title,
                        status.clone(),
                        Some(meta.tool_name.to_string()),
                        serde_json::from_str(meta.args_json).ok(),
                    );
                    self.apply_tool_call_update(
                        tool_call_id.to_string(),
                        None,
                        Some(status),
                        (!output.is_empty()).then_some(output),
                        None,
                        meta.diff.map(str::to_string),
                        meta.is_error,
                    );
                    continue;
                }
                let id = self.alloc_id();
                self.messages
                    .push(AgentChatThreadMessage::with_tool_call_id(
                        id,
                        role,
                        body,
                        tool_call_id.to_string(),
                    ));
            } else {
                let id = self.alloc_id();
                self.messages
                    .push(AgentChatThreadMessage::new(id, role, body));
            }
        }
        self.set_status(AgentChatThreadStatus::Idle);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_kitchen_sink_fixture_loaded",
            fixture_id = fixture.id,
            message_count = self.messages.len(),
        );
        cx.notify();
    }

    /// Clear composer-attached context state before a fresh external entry
    /// intent reuses this thread.
    ///
    /// Preserves transcript history, but removes stale chips, hidden context
    /// blocks, and queued bootstrap state so a launcher-initiated submit
    /// starts from the new intent alone.
    pub(crate) fn clear_pending_context_for_new_entry_intent(&mut self, cx: &mut Context<Self>) {
        self.reset_pending_context_for_new_entry_intent();
        cx.notify();
    }

    pub(crate) fn cancel_streaming(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.status, AgentChatThreadStatus::Streaming) {
            return;
        }
        self.flush_streaming_text_buffer();
        self.queue_paused = true;
        if let Err(error) = self.connection.cancel_turn(self.ui_thread_id.clone()) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_cancel_turn_enqueue_failed",
                error = %error,
            );
        }
        self.stream_task = None;
        self.stream_started_at = None;
        self.status = AgentChatThreadStatus::Idle;
        cx.notify();
    }

    /// Load saved messages from a conversation history file.
    /// Replaces current messages with the saved ones (read-only view).
    /// Clears all pending context state so loaded history does not inherit
    /// stale chips from the previous conversation.
    pub(crate) fn load_saved_messages(
        &mut self,
        saved: &[super::history::SavedMessage],
        cx: &mut Context<Self>,
    ) {
        self.bump_transcript_generation("load_saved_messages");
        self.flush_streaming_text_buffer();
        self.stream_task = None;
        self.stream_started_at = None;
        self.pending_permission = None;
        self.status = AgentChatThreadStatus::Idle;
        self.active_plan_entries.clear();
        self.active_tool_calls.clear();
        self.tool_call_lookup.clear();
        self.standing_approvals.clear();
        self.active_mode_id = None;
        self.available_commands.clear();
        self.usage_tokens = None;
        self.usage_cost_usd = None;
        self.next_message_id = 1;
        self.clear_all_pending_context("load_saved_messages");
        self.messages.clear();
        for msg in saved {
            let role = match msg.role.as_str() {
                "User" => AgentChatThreadMessageRole::User,
                "Assistant" => AgentChatThreadMessageRole::Assistant,
                "Thought" => AgentChatThreadMessageRole::Thought,
                "Tool" => AgentChatThreadMessageRole::Tool,
                "System" => AgentChatThreadMessageRole::System,
                "Error" => AgentChatThreadMessageRole::Error,
                _ => AgentChatThreadMessageRole::System,
            };
            let id = self.alloc_id();
            self.messages
                .push(AgentChatThreadMessage::new(id, role, msg.body.clone()));
        }
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        cx.notify();
    }

    pub(crate) fn draft_snapshot(&self) -> AgentChatThreadDraftSnapshot {
        AgentChatThreadDraftSnapshot {
            input: self.input.text().to_string(),
            input_cursor: self.input.cursor(),
            pending_context_parts: self.pending_context_parts.clone(),
            pending_context_consumed: self.pending_context_consumed,
        }
    }

    pub(crate) fn restore_draft_snapshot(
        &mut self,
        snapshot: AgentChatThreadDraftSnapshot,
        cx: &mut Context<Self>,
    ) {
        let input_len = snapshot.input.chars().count();
        let cursor = snapshot.input_cursor.min(input_len);
        self.input.set_text(snapshot.input);
        self.input.set_cursor(cursor);
        self.pending_context_parts = snapshot.pending_context_parts;
        self.pending_context_consumed = snapshot.pending_context_consumed;
        self.pending_context_blocks.clear();
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        self.queued_submit_while_bootstrapping = false;
        cx.notify();
    }

    fn reset_pending_context_for_new_entry_intent(&mut self) {
        self.clear_all_pending_context("new_entry_intent");
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = None;
        self.queued_submit_while_bootstrapping = false;
    }

    /// Tracked tool calls, ordered by creation.
    pub(crate) fn active_tool_calls(&self) -> &[AgentChatToolCallState] {
        &self.active_tool_calls
    }

    /// Current bootstrap state for deferred context capture.
    pub(crate) fn context_bootstrap_state(&self) -> AgentChatContextBootstrapState {
        self.context_bootstrap_state
    }

    /// Whether deferred ambient context capture is still expected.
    pub(crate) fn pending_ambient_context_enabled(&self) -> bool {
        self.pending_ambient_context_enabled
    }

    /// Whether a submit is queued waiting for context bootstrap.
    pub(crate) fn queued_submit_while_bootstrapping(&self) -> bool {
        self.queued_submit_while_bootstrapping
    }

    pub(crate) fn queued_messages(&self) -> &VecDeque<AgentChatQueuedMessage> {
        &self.queued_messages
    }

    pub(crate) fn queue_paused(&self) -> bool {
        self.queue_paused
    }

    pub(crate) fn remove_queued_message(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.queued_messages.len() {
            self.queued_messages.remove(index);
            cx.notify();
        }
    }

    pub(crate) fn clear_queued_messages(&mut self, cx: &mut Context<Self>) {
        if !self.queued_messages.is_empty() {
            self.queued_messages.clear();
            cx.notify();
        }
    }

    /// Human-readable bootstrap status note, if any.
    pub(crate) fn context_bootstrap_note(&self) -> Option<&str> {
        self.context_bootstrap_note.as_ref().map(|s| s.as_ref())
    }

    // ── Typed context parts (composer chips) ─────────────────────

    /// Read the pending context parts (visible as chips in the composer).
    pub(crate) fn pending_context_parts(&self) -> &[crate::ai::message_parts::AiContextPart] {
        &self.pending_context_parts
    }

    /// Add a typed context part. Deduplicates by value equality — if an
    /// identical part already exists the call is a no-op.
    ///
    /// When an Ask Anything part is added, stale hidden ambient blocks are
    /// cleared and the bootstrap state is set to `Preparing` so the deferred
    /// capture path knows to arm.
    pub(crate) fn add_context_part(
        &mut self,
        part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        let already_present = self
            .pending_context_parts
            .iter()
            .any(|existing| existing == &part);

        if already_present {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_context_part_add_skipped_duplicate",
                source = %part.source(),
                label = %part.label(),
            );
            return;
        }

        let is_ambient_bootstrap = part.is_ambient_bootstrap_resource();
        let ambient_label = part.ambient_chip_label().map(|value| value.to_string());
        let label = part.label().to_string();
        let source = part.source().to_string();

        if is_ambient_bootstrap {
            self.clear_pending_ambient_context("add_ambient_bootstrap_resource");
            self.pending_ambient_context_enabled = true;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
            self.context_bootstrap_note = ambient_label
                .as_deref()
                .map(Self::ambient_capture_preparing_note);
        } else if !self.pending_ambient_context_enabled
            && matches!(
                self.context_bootstrap_state,
                AgentChatContextBootstrapState::Preparing
            )
        {
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = None;
        }

        self.pending_context_parts.push(part);
        self.arm_pending_context(if is_ambient_bootstrap {
            "add_ambient_bootstrap_part"
        } else {
            "add_context_part"
        });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_context_part_added",
            source = %source,
            label = %label,
            is_ambient_bootstrap,
            ambient_label = ?ambient_label,
            pending_part_count = self.pending_context_parts.len(),
            pending_block_count = self.pending_context_blocks.len(),
        );
        cx.notify();
    }

    pub(crate) fn add_or_replace_skill_context(
        &mut self,
        identity: SkillContextIdentity,
        part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        let crate::ai::message_parts::AiContextPart::SkillFile {
            path, slash_name, ..
        } = &part
        else {
            self.add_context_part(part, cx);
            return;
        };

        self.pending_context_parts.retain(|existing| {
            !matches!(
                existing,
                crate::ai::message_parts::AiContextPart::SkillFile {
                    path: existing_path,
                    slash_name: existing_slash,
                    ..
                } if existing_path == path || existing_slash == slash_name
            )
        });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_skill_context_bound_to_thread",
            thread_id = %identity.thread_id,
            skill_id = %identity.skill_id,
            skill_file_hash = %identity.skill_file_hash,
            staged_by = ?identity.staged_by,
        );
        self.add_context_part(part, cx);
    }

    /// Replace all pending typed context parts in one host-owned handoff.
    ///
    /// Used by host surfaces that stage a fresh context payload on an
    /// existing Agent Chat view and must not append onto stale chips from a prior
    /// entry path.
    pub(crate) fn replace_pending_context_parts(
        &mut self,
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.replace_pending_context_parts_inner(parts, reason);

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_context_parts_replaced",
            reason,
            part_count = self.pending_context_parts.len(),
            ambient_enabled = self.pending_ambient_context_enabled,
        );
        cx.notify();
    }

    fn replace_pending_context_parts_inner(
        &mut self,
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        reason: &'static str,
    ) {
        self.clear_all_pending_context(reason);
        self.pending_context_parts = parts;
        self.pending_context_consumed = false;
        self.queued_submit_while_bootstrapping = false;

        let ambient_label = self
            .pending_context_parts
            .iter()
            .find_map(|part| part.ambient_chip_label().map(|value| value.to_string()));
        let has_ambient_bootstrap = self
            .pending_context_parts
            .iter()
            .any(|part| part.is_ambient_bootstrap_resource());
        let has_promoted_ambient_chip = self
            .pending_context_parts
            .iter()
            .any(|part| part.is_ambient_context_chip());

        self.pending_ambient_context_enabled = has_ambient_bootstrap || has_promoted_ambient_chip;

        if has_ambient_bootstrap {
            self.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
            self.context_bootstrap_note = ambient_label
                .as_deref()
                .map(Self::ambient_capture_preparing_note);
        } else if has_promoted_ambient_chip {
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = ambient_label
                .as_deref()
                .map(Self::ambient_capture_ready_note);
        } else {
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = None;
        }
    }

    /// Remove a typed context part by index.
    ///
    /// When an Ask Anything or AmbientContext chip is removed, clears the
    /// staged ambient blocks, disables ambient staging, updates the bootstrap
    /// state/note, and prevents deferred ambient context from being submitted.
    /// If a submit was queued while bootstrapping and the chip is removed,
    /// re-evaluates whether to submit now (without ambient context).
    pub(crate) fn remove_context_part(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.pending_context_parts.len() {
            return;
        }
        let removed = self.pending_context_parts.remove(index);
        let removed_ambient_label = removed.ambient_chip_label().map(|value| value.to_string());

        if let Some(ref ambient_label) = removed_ambient_label {
            self.clear_pending_ambient_context("remove_ambient_context_part");
            self.finish_bootstrap(
                AgentChatContextBootstrapState::Ready,
                Self::ambient_capture_removed_note(ambient_label),
                cx,
            );
        } else {
            self.arm_pending_context("remove_context_part");
            cx.notify();
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_context_part_removed",
            index,
            source = %removed.source(),
            label = %removed.label(),
            removed_ambient = removed_ambient_label.is_some(),
            ambient_label = ?removed_ambient_label,
            pending_part_count = self.pending_context_parts.len(),
            pending_block_count = self.pending_context_blocks.len(),
        );
    }
}

/// Test-only helpers exposed to sibling modules in `src/ai/agent_chat/ui/`.
#[cfg(test)]
impl AgentChatThread {
    /// Build a test thread without a real connection or GPUI context.
    pub(super) fn test_new(
        context_blocks: Vec<ContentBlock>,
        initial_input: Option<String>,
    ) -> Self {
        let (_perm_tx, perm_rx) = async_channel::bounded(1);
        let dummy_connection: Arc<dyn AgentChatConnection> = Arc::new(TestAgentChatConnection);

        Self {
            connection: dummy_connection,
            permission_rx: perm_rx,
            ui_thread_id: "test-thread".to_string(),
            cwd: PathBuf::from("/tmp/test"),
            display_name: "Test Agent".into(),
            profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID.to_string(),
            messages: Vec::new(),
            input: match initial_input {
                Some(text) if !text.is_empty() => TextInputState::with_text(text),
                _ => TextInputState::new(),
            },
            status: AgentChatThreadStatus::Idle,
            active_callout: None,
            pending_permission: None,
            pending_context_blocks: context_blocks,
            pending_context_consumed: false,
            pending_context_parts: Vec::new(),
            pending_ambient_context_enabled: false,
            context_bootstrap_state: AgentChatContextBootstrapState::Ready,
            queued_submit_while_bootstrapping: false,
            context_bootstrap_note: None,
            queued_messages: VecDeque::new(),
            queue_paused: false,
            active_plan_entries: Vec::new(),
            active_mode_id: None,
            available_commands: Vec::new(),
            active_tool_calls: Vec::new(),
            tool_call_lookup: HashMap::new(),
            standing_approvals: Vec::new(),
            fork_points: Vec::new(),
            pending_fork_ordinal: None,
            selected_agent: None,
            available_agents: Vec::new(),
            launch_requirements: crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
            setup_state: None,
            usage_tokens: None,
            usage_cost_usd: None,
            stream_started_at: None,
            ttft_pending: false,
            stream_task: None,
            permission_task: None,
            streaming_text_buffer: StreamingTextBuffer::default(),
            streaming_text_drain_task: None,
            transcript_generation: 0,
            next_message_id: 1,
            host_window_state: None,
            notification_debounce: AgentChatNotificationDebounce::default(),
            current_turn_id: 0,
            llm_title_attempted: false,
            available_models: Vec::new(),
            selected_model_id: None,
            selected_model_display_name: None,
            profile_display_name: None,
            profile_icon_name: None,
        }
    }

    pub(super) fn dismiss_active_callout_test(&mut self) {
        self.active_callout = None;
    }

    pub(super) fn retry_last_user_turn_test(&mut self) -> Result<(), String> {
        if !matches!(
            self.status,
            AgentChatThreadStatus::Error | AgentChatThreadStatus::Idle
        ) {
            return Ok(());
        }

        let Some(display_text) = self.last_user_turn_text() else {
            return Err("no_user_turn_to_retry".to_string());
        };
        let prepared = self.prepare_turn_blocks_with_receipt(display_text.trim());
        self.set_context_resolution_note(prepared.receipt.as_ref());
        let _request = AgentChatTurnRequest {
            ui_thread_id: self.ui_thread_id.clone(),
            cwd: self.cwd.clone(),
            blocks: prepared.blocks,
            model_id: self.selected_model_id.clone(),
        };
        self.stream_started_at = Some(std::time::Instant::now());
        self.ttft_pending = true;
        self.status = AgentChatThreadStatus::Streaming;
        self.active_callout = None;
        self.setup_state = None;
        Ok(())
    }

    /// Add a context part without a GPUI context (skips `cx.notify()`).
    pub(super) fn add_context_part_test(&mut self, part: crate::ai::message_parts::AiContextPart) {
        let already_present = self
            .pending_context_parts
            .iter()
            .any(|existing| existing == &part);
        if already_present {
            return;
        }

        let is_ambient_bootstrap = part.is_ambient_bootstrap_resource();
        self.pending_context_consumed = false;

        if is_ambient_bootstrap {
            self.pending_context_blocks.clear();
            self.pending_ambient_context_enabled = true;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Preparing;
            self.context_bootstrap_note = part
                .ambient_chip_label()
                .map(Self::ambient_capture_preparing_note);
        } else if !self.pending_ambient_context_enabled
            && matches!(
                self.context_bootstrap_state,
                AgentChatContextBootstrapState::Preparing
            )
        {
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = None;
        }

        self.pending_context_parts.push(part);
    }

    /// Remove a context part by index without a GPUI context (skips `cx.notify()`).
    pub(super) fn remove_context_part_test(&mut self, index: usize) {
        if index >= self.pending_context_parts.len() {
            return;
        }
        let removed = self.pending_context_parts.remove(index);
        let removed_ambient_label = removed.ambient_chip_label().map(|value| value.to_string());

        if let Some(ref ambient_label) = removed_ambient_label {
            self.pending_ambient_context_enabled = false;
            self.pending_context_blocks.clear();
            self.pending_context_consumed = false;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = Some(Self::ambient_capture_removed_note(ambient_label));
        }
    }

    pub(super) fn replace_pending_context_parts_test(
        &mut self,
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        reason: &'static str,
    ) {
        self.replace_pending_context_parts_inner(parts, reason);
    }

    /// Stage Ask Anything context without GPUI context (skips `cx.notify()`).
    pub(super) fn stage_ask_anything_context_test(
        &mut self,
        context: &crate::ai::TabAiContextBlob,
    ) -> Result<(), String> {
        let ambient_label = self
            .current_ambient_chip_label()
            .unwrap_or_else(|| crate::ai::message_parts::ASK_ANYTHING_LABEL.to_string());

        if !self.pending_ambient_context_enabled {
            self.pending_context_blocks.clear();
            self.pending_context_consumed = false;
            self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
            self.context_bootstrap_note = Some(Self::ambient_capture_removed_note(&ambient_label));
            return Ok(());
        }

        self.pending_context_blocks = build_tab_ai_agent_chat_context_blocks(context)?;
        self.pending_context_consumed = false;
        self.promote_ask_anything_chip_to_ambient();
        self.context_bootstrap_state = AgentChatContextBootstrapState::Ready;
        self.context_bootstrap_note = Some(Self::ambient_capture_ready_note(&ambient_label));
        Ok(())
    }

    /// Apply an event without a GPUI context (for testing pure logic).
    /// Reuses the same helper methods as `apply_event` but skips `cx.notify()`.
    pub(super) fn apply_event_test(&mut self, event: super::AgentChatEvent) {
        match event {
            super::AgentChatEvent::UserMessageDelta(chunk) => {
                self.append_chunk(AgentChatThreadMessageRole::System, chunk);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::AgentMessageDelta(chunk) => {
                self.append_chunk(AgentChatThreadMessageRole::Assistant, chunk);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::AgentThoughtDelta(chunk) => {
                self.append_chunk(AgentChatThreadMessageRole::Thought, chunk);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::ToolCallStarted {
                tool_call_id,
                title,
                status,
                tool_name,
                raw_input,
            } => {
                self.upsert_tool_call_start(tool_call_id, title, status, tool_name, raw_input);
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::ToolCallUpdated {
                tool_call_id,
                title,
                status,
                body,
                raw_input,
                diff,
                is_error,
            } => {
                self.apply_tool_call_update(
                    tool_call_id,
                    title,
                    status,
                    body,
                    raw_input,
                    diff,
                    is_error,
                );
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::PlanUpdated { entries } => {
                self.active_plan_entries = entries;
                self.set_status(AgentChatThreadStatus::Streaming);
            }
            super::AgentChatEvent::AvailableCommandsUpdated { command_names } => {
                self.available_commands = command_names;
            }
            super::AgentChatEvent::ModeChanged { mode_id } => {
                self.active_mode_id = Some(mode_id);
            }
            super::AgentChatEvent::UsageUpdated {
                used_tokens,
                context_size,
                cost_usd,
            } => {
                self.usage_tokens = Some((used_tokens, context_size));
                if let Some(cost) = cost_usd {
                    self.usage_cost_usd = Some(cost);
                }
            }
            super::AgentChatEvent::ModelsAvailable {
                current_model_id,
                models,
            } => {
                self.apply_agent_models(current_model_id, models);
            }
            super::AgentChatEvent::ForkPointsAvailable { entries } => {
                self.fork_points = entries;
            }
            super::AgentChatEvent::ForkCompleted { text } => {
                // Test path mirrors `apply_fork_completed` minus the GPUI
                // refresh: truncate locally and stage the text for editing.
                if let Some(ordinal) = self.pending_fork_ordinal.take() {
                    Self::truncate_messages_at_user_ordinal(&mut self.messages, ordinal);
                    self.active_tool_calls.clear();
                    self.tool_call_lookup.clear();
                    self.input.set_text(text.clone());
                    self.input.set_cursor(text.chars().count());
                    self.set_status(AgentChatThreadStatus::Idle);
                }
            }
            super::AgentChatEvent::TurnFinished { .. } => {
                self.set_status(AgentChatThreadStatus::Idle);
                if !self.queue_paused {
                    if let Some(message) = self.queued_messages.pop_front() {
                        self.push_message(AgentChatThreadMessageRole::User, message.text);
                        self.set_status(AgentChatThreadStatus::Streaming);
                    }
                }
            }
            super::AgentChatEvent::SetupRequired {
                reason,
                auth_methods,
            } => {
                let current_requirements = self.current_setup_requirements();
                self.setup_state = Some(
                    super::setup_state::AgentChatInlineSetupState::from_runtime_setup_required(
                        self.selected_agent.clone(),
                        self.available_agents.clone(),
                        current_requirements,
                        &reason,
                        &auth_methods,
                    ),
                );
                self.set_status(AgentChatThreadStatus::Error);
            }
            super::AgentChatEvent::Failed { error } => {
                let can_retry = self.last_user_turn_text().is_some();
                let callout = AgentChatCallout::failed(error, can_retry);
                let transcript_message = callout
                    .detail
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "The provider could not complete this turn.".to_string());
                self.active_callout = Some(callout);
                self.push_message(AgentChatThreadMessageRole::Error, transcript_message);
                self.set_status(AgentChatThreadStatus::Error);
            }
        }
    }
}

#[cfg(test)]
#[path = "thread/tests.rs"]
mod tests;
