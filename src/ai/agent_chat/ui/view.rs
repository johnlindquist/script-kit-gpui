//! Agent Chat chat view.
//!
//! Renders an Agent Chat conversation thread with markdown-rendered messages,
//! role-aware cards, empty/streaming/error states, and inline permission
//! approval cards. Wraps an `AgentChatThread` entity for the Tab AI surface.

use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    div, prelude::*, px, rgb, rgba, Animation, AnimationExt, App, Context, ElementId, Entity,
    FocusHandle, Focusable, FontWeight, IntoElement, ParentElement, Render, Rgba, SharedString,
    Task, WeakEntity, Window,
};

use gpui_component::scroll::ScrollableElement;
use gpui_component::tooltip::Tooltip;

use crate::ai::agent_chat::content::{ContentBlock, TextContent};
use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatEventRx};
use crate::components::text_input::{
    render_text_input_cursor_selection, TextHighlightRange, TextInlinePillRange,
    TextInputRenderConfig, TextSelection,
};
use crate::theme::{self, AppChromeColors, PromptColors};

use super::composer_state::{
    reduce_agent_chat_composer_picker, AgentChatComposerPickerDismissReason,
    AgentChatComposerPickerEvent, AgentChatComposerPickerRefreshInput,
    AgentChatComposerPickerState, AgentChatComposerPickerTransition,
};
use super::history_popup::{
    history_popup_key_intent, AgentChatHistoryPopupKeyIntent, HISTORY_POPUP_PAGE_JUMP,
    HISTORY_POPUP_SEARCH_LIMIT,
};
use super::thread::{
    decide_agent_chat_cwd_resolution, AgentChatCalloutSeverity, AgentChatContextBootstrapState,
    AgentChatCwdResolutionDecision, AgentChatHostWindowKind, AgentChatHostWindowState,
    AgentChatThread, AgentChatThreadMessage, AgentChatThreadMessageRole, AgentChatThreadStatus,
};
use super::types::{
    AgentChatComposerParentWindow, AgentChatComposerPickerSession, AgentChatComposerPickerTrigger,
    AgentChatDismissedComposerPickerTrigger, AgentChatFocusedMentionPreview,
    AgentChatPendingPortalSession,
};
use super::ui_variant::{AgentChatComposerPlacement, AgentChatUiVariant};
use super::{
    AgentChatApprovalOption, AgentChatApprovalPreview, AgentChatApprovalPreviewKind,
    AgentChatApprovalRequest,
};
use crate::ai::context_selector::types::PROFILE_TRIGGER_STR;

use crate::ai::context_selector::types::{
    ContextSelectorRow, ContextSelectorRowKind, ContextSelectorTrigger, SlashCommandPayload,
};
use crate::ai::context_selector::{
    slash_command_empty_row, slash_command_loading_row, slash_command_no_match_row,
    slash_command_rows_with_payloads,
};
use crate::ai::message_parts::AiContextPart;
use crate::list_item::{IconKind, ListItem, ListItemColors, TypeAccessory};
use crate::spine::list::{SpineListAction, SpineListRow, SpineListRowKind, SpineListSection};

use super::components::setup_card::{
    AgentChatSetupAgentPickerState, AgentChatSetupCard, AgentChatSetupCardEvent,
};
use super::components::toolbar::{AgentChatToolbar, AgentChatToolbarEvent};
use super::components::transcript::{AgentChatTranscript, AgentChatTranscriptEvent};

mod slash_and_skills;
mod types_local;

use slash_and_skills::parse_skill_description;
pub(crate) use slash_and_skills::{
    build_skill_context_part, build_skill_slash_command_text, build_staged_skill_prompt,
    SlashCommandEntry, SlashCommandSource,
};
pub(crate) use types_local::{parse_script_ready_receipt, ScriptReadyReceipt};

/// Click handler type for collapsible block toggle.
type ToggleHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;
/// Footer action callbacks use `&mut App` (not `Context<AgentChatView>`) so they can be
/// invoked without holding the AgentChatView borrow — toggle_actions needs to read the
/// entity, which panics if called from inside its own update.
type AgentChatFooterActionHandler = std::sync::Arc<dyn Fn(&mut Window, &mut App) + 'static>;
/// Portal open callback — receives the portal kind so the host can open the
/// appropriate built-in view (file search, clipboard history, etc.).
/// Takes `&mut App` (not `&mut Window`) because the handler opens a new view
/// via entity update, and this callback is invoked from contexts where
/// `Window` is not available (e.g. `accept_composer_picker_selection_impl`).
type AgentChatPortalHandler = std::sync::Arc<
    dyn Fn(crate::ai::context_selector::types::ContextPortalKind, &mut App) + 'static,
>;
type AgentChatProfileSelectionHandler = std::sync::Arc<dyn Fn(String, &mut App) + 'static>;
type AgentChatHostAppHandler = std::sync::Arc<dyn Fn(&mut App) + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortalRefusal {
    NoHost,
    UnsupportedByHost,
    OpenFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PortalOpenResult {
    Opened,
    Refused(PortalRefusal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusedTextMiniAction {
    Replace,
    Append,
    Copy,
    Expand,
    Stop,
    Retry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusedTextMiniPhase {
    InputOnly,
    Loading,
    Streaming,
    Result,
    Error,
}

impl FocusedTextMiniPhase {
    fn state_id(self) -> &'static str {
        match self {
            Self::InputOnly => "inputOnly",
            Self::Loading => "loading",
            Self::Streaming => "streaming",
            Self::Result => "result",
            Self::Error => "error",
        }
    }
}

const FOCUSED_TEXT_BALANCED_VARIATION_INDEX: usize = 1;
const AGENT_CHAT_FOOTER_LEADING_SLOT_WIDTH_PX: f32 =
    crate::components::footer_chrome::FOOTER_PASTE_RESPONSE_SLOT_WIDTH_PX;
const AGENT_CHAT_TRANSIENT_QUEUE_LANE_HEIGHT_PX: f32 = 36.0;
const AGENT_CHAT_TRANSIENT_BOOTSTRAP_LANE_HEIGHT_PX: f32 = 34.0;
const AGENT_CHAT_TRANSIENT_PLAN_LANE_HEIGHT_PX: f32 = 84.0;
const AGENT_CHAT_TRANSIENT_PERMISSION_LANE_HEIGHT_PX: f32 = 156.0;

fn agent_chat_transient_lane_height(height_px: f32, active: bool) -> f32 {
    if active {
        height_px
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusedTextVariationStatus {
    Idle,
    Streaming,
    Complete,
    Error,
}

impl FocusedTextVariationStatus {
    pub(crate) fn state_id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Streaming => "streaming",
            Self::Complete => "complete",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FocusedTextVariationSnapshot {
    pub(crate) index: usize,
    pub(crate) angle_id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) text: String,
    pub(crate) status: FocusedTextVariationStatus,
    pub(crate) selected: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FocusedTextVariationState {
    angle: crate::ai::focused_text::FocusedTextPromptAngle,
    text: String,
    status: FocusedTextVariationStatus,
    error: Option<String>,
}

impl FocusedTextVariationState {
    fn new(angle: crate::ai::focused_text::FocusedTextPromptAngle) -> Self {
        Self {
            angle,
            text: String::new(),
            status: FocusedTextVariationStatus::Idle,
            error: None,
        }
    }

    fn streaming(angle: crate::ai::focused_text::FocusedTextPromptAngle) -> Self {
        Self {
            angle,
            text: String::new(),
            status: FocusedTextVariationStatus::Streaming,
            error: None,
        }
    }

    fn snapshot(&self, index: usize, selected: bool) -> FocusedTextVariationSnapshot {
        FocusedTextVariationSnapshot {
            index,
            angle_id: self.angle.id(),
            label: self.angle.label(),
            text: self.text.clone(),
            status: self.status,
            selected,
            error: self.error.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FocusedTextContextStatus {
    Captured,
    CaptureFailed { reason_code: &'static str },
}

impl FocusedTextContextStatus {
    fn state_id(&self) -> &'static str {
        match self {
            Self::Captured => "captured",
            Self::CaptureFailed { .. } => "captureFailed",
        }
    }

    fn failure_code(&self) -> Option<String> {
        match self {
            Self::Captured => None,
            Self::CaptureFailed { reason_code } => Some((*reason_code).to_string()),
        }
    }

    fn user_message(&self) -> Option<&'static str> {
        match self {
            Self::Captured => None,
            Self::CaptureFailed { reason_code } => Some(match *reason_code {
                "accessibilityPermissionRequired" => {
                    "Accessibility permission needed. Grant access in System Settings to grab focused text."
                }
                "secureField" => "This is a secure field and can't be accessed.",
                "unsupportedTarget" => {
                    "Unable to grab text from this field. Select text and try again."
                }
                "staleSession" => "The focused text session expired. Try again.",
                "platform" => {
                    "Unable to grab text due to a system error. Select text and try again."
                }
                _ => "Unable to grab text. Select text and try again.",
            }),
        }
    }

    fn offers_open_settings(&self) -> bool {
        matches!(
            self,
            Self::CaptureFailed {
                reason_code: "accessibilityPermissionRequired"
            }
        )
    }
}

struct FocusedTextSemanticActionSpec {
    semantic_id: &'static str,
    action_value: &'static str,
    label: &'static str,
    shortcut: &'static str,
    enabled: bool,
    disabled_reason: Option<&'static str>,
}

impl FocusedTextMiniAction {
    pub(crate) fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "focused-text-action-replace" => Some(Self::Replace),
            "focused-text-action-append" => Some(Self::Append),
            "focused-text-action-copy" => Some(Self::Copy),
            "focused-text-action-expand" => Some(Self::Expand),
            "focused-text-action-collapse" => Some(Self::Expand),
            "focused-text-action-stop" => Some(Self::Stop),
            "focused-text-action-retry" => Some(Self::Retry),
            _ => None,
        }
    }

    fn trace_value(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::Append => "append",
            Self::Copy => "copy",
            Self::Expand => "expand",
            Self::Stop => "stop",
            Self::Retry => "retry",
        }
    }

    fn apply_action(self) -> Option<crate::ai::focused_text::FocusedTextApplyAction> {
        match self {
            Self::Replace => Some(crate::ai::focused_text::FocusedTextApplyAction::Replace),
            Self::Append => Some(crate::ai::focused_text::FocusedTextApplyAction::Append),
            Self::Copy => Some(crate::ai::focused_text::FocusedTextApplyAction::Copy),
            Self::Expand | Self::Stop | Self::Retry => None,
        }
    }

    fn from_footer_action(action: crate::footer_popup::FooterAction) -> Option<Self> {
        match action {
            crate::footer_popup::FooterAction::Replace => Some(Self::Replace),
            crate::footer_popup::FooterAction::Append => Some(Self::Append),
            crate::footer_popup::FooterAction::Copy | crate::footer_popup::FooterAction::Apply => {
                Some(Self::Copy)
            }
            crate::footer_popup::FooterAction::Expand => Some(Self::Expand),
            crate::footer_popup::FooterAction::Stop => Some(Self::Stop),
            crate::footer_popup::FooterAction::Retry => Some(Self::Retry),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatFooterHost {
    Inline,
    External,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentChatFooterButtonSpec {
    pub(crate) action: crate::footer_popup::FooterAction,
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) selected: bool,
    pub(crate) enabled: bool,
    pub(crate) disabled_reason: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentChatFooterSnapshot {
    pub(crate) visible: bool,
    pub(crate) dot_status: crate::footer_popup::FooterDotStatus,
    pub(crate) profile_display: String,
    pub(crate) profile_icon_name: Option<String>,
    pub(crate) model_display: String,
    pub(crate) status_text: Option<&'static str>,
    pub(crate) buttons: Vec<AgentChatFooterButtonSpec>,
    pub(crate) cwd_display: Option<String>,
}

impl AgentChatFooterSnapshot {
    pub(crate) fn agent_model_header_label(&self) -> String {
        match (
            self.profile_display.trim().is_empty(),
            self.model_display.trim().is_empty(),
        ) {
            (false, false) => format!("{} · {}", self.profile_display, self.model_display),
            (false, true) => self.profile_display.clone(),
            (true, false) => self.model_display.clone(),
            (true, true) => String::new(),
        }
    }

    pub(crate) fn model_status_label(&self) -> String {
        match self.status_text {
            Some(status) if !status.is_empty() => {
                format!("{} · {}", self.model_display, status)
            }
            _ => self.model_display.clone(),
        }
    }

    pub(crate) fn profile_left_info(&self) -> crate::footer_popup::FooterLeftInfo {
        let model_label = self.model_status_label();
        let cwd_chip = self
            .cwd_display
            .as_ref()
            .map(|cwd| crate::footer_popup::FooterCwdChip {
                label: cwd.clone(),
                icon_token: "folder".to_string(),
                key: None,
                tooltip: Some("Working directory — click to change".to_string()),
            });
        crate::footer_popup::FooterLeftInfo {
            dot_status: self.dot_status,
            model_name: model_label,
            prefer_accent_for_active_states: true,
            profile_name: Some(self.profile_display.clone()),
            icon_token: None,
            action: Some(crate::footer_popup::FooterAction::Ai),
            selected: false,
            cwd_chip,
        }
    }
}

#[derive(Clone, Debug)]
struct FocusedTextAgentChatState {
    snapshot: crate::platform::accessibility::FocusedTextSnapshot,
    session_id: crate::platform::accessibility::FocusedTextSessionId,
    app_name: String,
    app_bundle_id: Option<String>,
    char_count: usize,
    word_count: usize,
    context_status: FocusedTextContextStatus,
    capture_truncated: bool,
    can_replace: bool,
    can_append: bool,
    can_copy: bool,
    originated_from_quick_prompt: bool,
    last_apply_receipt: Option<crate::ai::focused_text::FocusedTextMutationReceipt>,
    last_action_receipt: Option<crate::protocol::AgentChatFocusedTextActionReceipt>,
}

/// Session mode for the Agent Chat chat view.
#[derive(Clone)]
pub(crate) enum AgentChatSession {
    /// Live conversation with an Agent Chat agent thread.
    Live(Entity<AgentChatThread>),
    /// Inline setup card — no launchable agent exists.
    Setup(Box<super::setup_state::AgentChatInlineSetupState>),
}

/// Explicit relaunch payload queued when setup retry is requested.
///
/// Carries the selected agent id and capability requirements from the
/// setup card so the next Agent Chat open path can consume them ahead of
/// fallback preference loading.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatRetryDraftState {
    pub input_text: String,
    pub input_cursor: usize,
    pub pending_context_parts: Vec<crate::ai::message_parts::AiContextPart>,
    pub pasted_text_tokens: Vec<crate::pasted_text::PastedTextToken>,
    pub pasted_image_tokens: Vec<crate::pasted_image::PastedImageToken>,
    pub typed_mention_aliases:
        std::collections::HashMap<String, crate::ai::message_parts::AiContextPart>,
    pub inline_owned_context_tokens: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentChatRetryRequest {
    pub preferred_agent_id: Option<String>,
    pub launch_requirements: super::preflight::AgentChatLaunchRequirements,
    pub draft_state: Option<AgentChatRetryDraftState>,
}

impl AgentChatRetryRequest {
    pub(crate) fn from_setup_state(setup: &super::setup_state::AgentChatInlineSetupState) -> Self {
        Self {
            preferred_agent_id: setup
                .selected_agent
                .as_ref()
                .map(|agent| agent.id.to_string()),
            launch_requirements: setup.launch_requirements,
            draft_state: None,
        }
    }
}

/// Explicit resume payload queued when a history item is selected for
/// re-opening. The Agent Chat open path can consume this to load a saved
/// conversation by `session_id` instead of using clipboard text or
/// markdown export.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatHistoryResumeRequest {
    pub session_id: String,
}

/// Snapshot of Agent Chat view-local draft state for host relaunches.
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentChatViewDraftSnapshot {
    pub thread: Option<super::thread::AgentChatThreadDraftSnapshot>,
    pending_portal_session: Option<AgentChatPendingPortalSession>,
    pasted_text_tokens: Vec<crate::pasted_text::PastedTextToken>,
    pasted_image_tokens: Vec<crate::pasted_image::PastedImageToken>,
    typed_mention_aliases:
        std::collections::HashMap<String, crate::ai::message_parts::AiContextPart>,
    inline_owned_context_tokens: HashSet<String>,
}

/// Structured state for the inline Agent Chat history popup.
///
/// Replaces the old `Option<(usize, String, Vec<AgentChatHistoryEntry>)>` tuple
/// so ranked search metadata (`AgentChatHistorySearchHit`) is preserved through
/// render instead of being discarded before the popup sees it.
#[derive(Debug, Clone)]
pub(crate) struct AgentChatHistoryMenuState {
    pub(crate) selected_index: usize,
    pub(crate) query: String,
    pub(crate) hits: Vec<super::history::AgentChatHistorySearchHit>,
}

/// Lightweight descriptor of a retained background thread, consumed by the
/// Cmd+K "Threads" section so the switcher can label rows without touching
/// the live thread entities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentChatThreadSummary {
    pub ui_thread_id: String,
    pub title: String,
    /// Messages appended since the user last viewed this thread.
    pub unread: usize,
    pub is_streaming: bool,
}

/// GPUI view entity wrapping an `AgentChatThread` for the Tab AI surface.
pub(crate) struct AgentChatView {
    /// The Agent Chat session — either a live thread or inline setup state.
    pub(crate) session: AgentChatSession,
    /// Live background threads retained when the user starts or switches
    /// threads. Each owns its own Pi connection and keeps streaming while
    /// inactive; the Cmd+K "Threads" section switches back to them.
    retained_threads: Vec<Entity<AgentChatThread>>,
    /// Message count last seen per `ui_thread_id`, for unread badges in the
    /// thread switcher.
    thread_last_seen: std::collections::HashMap<String, usize>,
    /// Observer subscriptions keyed by thread entity id (session + retained),
    /// so swapping the session thread never double-registers an observer.
    thread_observers: std::collections::HashMap<gpui::EntityId, gpui::Subscription>,
    host_activation_subscription: Option<gpui::Subscription>,
    focus_handle: FocusHandle,
    /// Virtualized variable-height message list state.
    permission_index: usize,
    /// Whether the inline permission options list is expanded.
    permission_options_open: bool,
    /// Cursor blink state.
    cursor_visible: bool,
    /// Handle to the cursor blink task.
    _blink_task: Task<()>,
    /// Ranked history popup state. None = hidden.
    pub(crate) history_menu: Option<AgentChatHistoryMenuState>,
    /// Most recent timestamp when the history popup was explicitly dismissed.
    history_closed_at: Option<Instant>,
    /// Whether the + attachment menu popup is open.
    attach_menu_open: bool,
    /// Whether the queued message strip is expanded to individual rows.
    message_queue_expanded: bool,
    /// Cmd+F search: (query, current_match_index). None = search hidden.
    pub(crate) search_state: Option<(String, usize)>,
    /// Cached slash commands discovered at creation, with source identity.
    cached_slash_commands: Vec<SlashCommandEntry>,
    /// Handle to the deferred slash command discovery task.
    _slash_discovery_task: Task<()>,
    /// Active slash/profile composer picker session (None = picker hidden).
    pub(crate) composer_picker_session: Option<AgentChatComposerPickerSession>,
    expanded_composer: bool,
    /// Surface-local Spine state for the Agent Chat composer. When this projection
    /// owns the conversation area, the transcript is replaced with the
    /// Spine list (context / slash / profile / style / capture / CWD rows).
    pub(crate) composer_spine:
        crate::ai::agent_chat::ui::composer_state::AgentChatComposerSpineState,
    /// Exact active trigger dismissed by pointer/escape while the input text remains unchanged.
    dismissed_mention_trigger: Option<AgentChatDismissedComposerPickerTrigger>,
    /// Cached parent window metadata for toolbar-triggered popups.
    composer_parent_window: Option<AgentChatComposerParentWindow>,
    /// Canonical inline tokens that currently own their attached context part.
    ///
    /// This preserves non-inline chip attachments during mention sync while
    /// still letting deleted inline mentions remove the parts they created.
    inline_owned_context_tokens: HashSet<String>,
    /// Session-local alias registry mapping typed `@type:name` display tokens
    /// to full `AiContextPart` values for resolution and sync.
    typed_mention_aliases:
        std::collections::HashMap<String, crate::ai::message_parts::AiContextPart>,
    /// Large pasted blocks collapsed into inline tokens for compact composer display.
    pasted_text_tokens: Vec<crate::pasted_text::PastedTextToken>,
    /// Clipboard images collapsed into inline pills while remaining attached as files.
    pasted_image_tokens: Vec<crate::pasted_image::PastedImageToken>,
    /// Setup card entity (only present during setup or runtime recovery).
    setup_card: Option<Entity<AgentChatSetupCard>>,
    toolbar: Option<Entity<AgentChatToolbar>>,
    pub(crate) transcript: Option<Entity<AgentChatTranscript>>,
    ui_variant: AgentChatUiVariant,
    focused_text: Option<FocusedTextAgentChatState>,
    focused_text_variations: Vec<FocusedTextVariationState>,
    focused_text_variation_tasks: Vec<Task<()>>,
    focused_text_variation_cancel_flags: Vec<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    /// Monotonic generation counter — incremented on each new variation submit
    /// so that stale async tasks from a previous generation are discarded.
    focused_text_variation_generation: u64,
    /// History of previous variation generations for Cmd+Left/Right navigation.
    focused_text_variation_history: Vec<Vec<FocusedTextVariationState>>,
    /// Current position in the generation history (None = latest).
    focused_text_variation_history_index: Option<usize>,
    focused_text_selected_variation: Option<usize>,
    focused_text_editing_variation: Option<usize>,
    focused_text_instruction_history: Vec<String>,
    focused_text_instruction_history_index: Option<usize>,
    focused_text_instruction_history_draft: Option<String>,

    /// Plain natural-language scope for focused-text mini edits.
    pub(crate) scope_input: String,
    /// Whether the optional scope row is visible in focused-text mini mode.
    pub(crate) scope_visible: bool,
    /// Whether focused-text mini key input is currently routed to the scope row.
    scope_focused: bool,

    /// Setup-mode agent selection picker state (managed by AgentChatView until
    /// fully migrated to AgentChatSetupCard).
    pub(crate) setup_agent_picker: Option<AgentChatSetupAgentPickerState>,
    /// The transient trigger character that initiated this session from the main menu.
    pub(crate) opened_via_transient_trigger: Option<char>,
    /// Most recently accepted picker item (for telemetry/testing).
    last_accepted_item: Option<crate::protocol::AgentChatAcceptedItem>,
    /// Bounded test probe ring buffer for agentic verification.
    test_probe: AgentChatTestProbe,
    /// Queued retry payload from setup card — consumed by the Agent Chat open path.
    pending_retry_request: Option<AgentChatRetryRequest>,
    /// Queued history resume request — consumed by the Agent Chat open path
    /// to load a saved conversation by session_id.
    pending_history_resume: Option<AgentChatHistoryResumeRequest>,
    /// Host-owned footer callback for toggling the actions popup.
    on_toggle_actions: Option<AgentChatFooterActionHandler>,
    /// Host-owned footer callback for closing the Agent Chat surface.
    on_close_requested: Option<AgentChatFooterActionHandler>,
    /// Host-owned shortcut callback for closing the host window from Agent Chat.
    on_close_window_requested: Option<AgentChatFooterActionHandler>,
    /// Host-owned callback for opening the dedicated history command surface.
    on_open_history_command: Option<AgentChatFooterActionHandler>,
    /// Host-owned callback for pasting the latest assistant response.
    on_paste_response_requested: Option<AgentChatFooterActionHandler>,
    /// Host-owned callback for expanding focused-text mini into full Agent Chat.
    on_focused_text_expand_requested: Option<AgentChatHostAppHandler>,
    /// Host-owned callback for collapsing focused-text Agent Chat back to mini mode.
    on_focused_text_collapse_requested: Option<AgentChatHostAppHandler>,
    /// Host-owned callback for opening a full built-in view as an attachment portal.
    on_open_portal: Option<AgentChatPortalHandler>,
    /// Host-owned callback for persisting an Agent Chat profile and relaunching.
    on_profile_selected: Option<AgentChatProfileSelectionHandler>,
    /// Transactional session for the currently staged attachment portal open.
    pending_portal_session: Option<AgentChatPendingPortalSession>,
    footer_host: AgentChatFooterHost,
    /// Validated script path from a `SCRIPT_READY` receipt in assistant output.
    /// When `Some`, the footer Run button dispatches this path instead of
    /// the generic `execute_selected`.
    ready_script_path: Option<std::path::PathBuf>,
    /// Pending slash-command to prime on first picker refresh (e.g. "new-script").
    pending_slash_prime: Option<String>,
    /// True while a deferred context capture is in-flight, driving the footer loading dot.
    context_capture_pending: bool,

    /// Last observed lock state for the focused-text mini instruction input.
    ///
    /// Used to detect the Loading/Streaming -> unlocked edge without enforcing
    /// focus on every render.
    focused_text_mini_input_locked: bool,

    /// One-shot focus restore requested after focused-text mini input unlocks.
    pending_focused_text_mini_focus_restore: bool,

    /// Portal kinds the host allows this Agent Chat surface to open.
    ///
    /// Defaults to all kinds. Notes-hosted Agent Chat narrows this to only
    /// `AgentChatHistory` because it cannot own file-search or clipboard views.
    /// Items for disallowed kinds are filtered from the composer picker and
    /// rejected at the portal-open dispatch as defense-in-depth.
    allowed_portal_kinds: Vec<crate::ai::context_selector::types::ContextPortalKind>,
    _footer_action_task: Option<gpui::Task<()>>,
}

/// Bounded ring buffer for Agent Chat test probe events.
///
/// Agents can reset, record, and snapshot this to verify native picker
/// acceptance without scraping logs. Storage is cheap and bounded.
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentChatTestProbe {
    /// Monotonically increasing event counter.
    pub(crate) event_seq: u64,
    /// Recent key-route events (bounded by `MAX_EVENTS`).
    pub(crate) key_routes: std::collections::VecDeque<crate::protocol::AgentChatKeyRouteTelemetry>,
    /// Recent picker-acceptance events (bounded by `MAX_EVENTS`).
    pub(crate) accepted_items:
        std::collections::VecDeque<crate::protocol::AgentChatPickerItemAcceptedTelemetry>,
    /// Most recent input-layout telemetry.
    pub(crate) input_layout: Option<crate::protocol::AgentChatInputLayoutTelemetry>,
    /// Most recent synthesised interaction trace (key-route + optional accept).
    pub(crate) last_interaction_trace: Option<crate::protocol::AgentChatLastInteractionTrace>,
}

use crate::protocol::AGENT_CHAT_TEST_PROBE_MAX_EVENTS;

impl AgentChatView {
    /// All portal kinds — the default for launcher/detached Agent Chat surfaces.
    fn all_portal_kinds() -> Vec<crate::ai::context_selector::types::ContextPortalKind> {
        use crate::ai::context_selector::types::ContextPortalKind;
        vec![
            ContextPortalKind::AgentChatHistory,
            ContextPortalKind::FileSearch,
            ContextPortalKind::BrowserHistory,
            ContextPortalKind::BrowserTabs,
            ContextPortalKind::ClipboardHistory,
            ContextPortalKind::DictationHistory,
            ContextPortalKind::ScriptSearch,
            ContextPortalKind::ScriptletSearch,
            ContextPortalKind::SkillSearch,
            ContextPortalKind::NotesBrowse,
            ContextPortalKind::Terminal,
        ]
    }

    pub(crate) fn with_ui_variant(mut self, ui_variant: AgentChatUiVariant) -> Self {
        self.ui_variant = ui_variant;
        self
    }

    pub(crate) fn set_ui_variant(
        &mut self,
        ui_variant: AgentChatUiVariant,
        cx: &mut Context<Self>,
    ) {
        if self.ui_variant == ui_variant {
            return;
        }
        self.ui_variant = ui_variant;

        self.pending_focused_text_mini_focus_restore = false;
        if ui_variant != AgentChatUiVariant::FocusedTextMini {
            self.scope_focused = false;
            self.focused_text_editing_variation = None;
        }
        if ui_variant == AgentChatUiVariant::FocusedTextMini && !self.is_setup_mode() {
            let input_locked = {
                let thread = self.live_thread().read(cx);
                self.focused_text_input_locked_for_thread(thread)
            };
            self.focused_text_mini_input_locked = input_locked;
        } else {
            self.focused_text_mini_input_locked = false;
        }

        if let Some(transcript) = &self.transcript {
            transcript.update(cx, |transcript, cx| {
                transcript.set_ui_variant(ui_variant, cx);
            });
        }
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_ui_variant_changed",
            agent_chat_ui_variant = ui_variant.state_id(),
        );
        cx.notify();
    }

    pub(crate) fn debug_ui_variant_id(&self) -> &'static str {
        self.ui_variant.state_id()
    }

    pub(crate) fn is_focused_text_mini(&self) -> bool {
        self.focused_text.is_some() && self.ui_variant == AgentChatUiVariant::FocusedTextMini
    }

    pub(crate) fn locks_main_window_resize(&self) -> bool {
        matches!(self.ui_variant, AgentChatUiVariant::FocusedTextMini)
    }

    pub(crate) fn mark_focused_text_originated_from_quick_prompt(&mut self) {
        if let Some(state) = self.focused_text.as_mut() {
            state.originated_from_quick_prompt = true;
        }
    }

    pub(crate) fn focused_text_originated_from_quick_prompt(&self) -> bool {
        self.focused_text
            .as_ref()
            .is_some_and(|state| state.originated_from_quick_prompt)
    }

    fn composer_is_active(
        window_active: bool,
        view_focused: bool,
        actions_window_open: bool,
    ) -> bool {
        window_active && view_focused && !actions_window_open
    }

    fn host_window_state_for_window(&self, window: &Window) -> AgentChatHostWindowState {
        let kind = if crate::ai::agent_chat::ui::chat_window::is_chat_window(window) {
            AgentChatHostWindowKind::Detached
        } else {
            AgentChatHostWindowKind::Main
        };
        AgentChatHostWindowState {
            kind,
            key: window.is_window_active() && self.focus_handle.is_focused(window),
        }
    }

    fn sync_host_window_state(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(thread) = self.thread() else {
            return;
        };
        let state = self.host_window_state_for_window(window);
        thread.update(cx, |thread, cx| thread.set_host_window_state(state, cx));
    }

    fn ensure_host_activation_subscription(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.host_activation_subscription.is_some() {
            return;
        }
        self.host_activation_subscription =
            Some(cx.observe_window_activation(window, |this, window, cx| {
                this.sync_host_window_state(window, cx);
            }));
    }

    fn was_history_recently_closed(&self) -> bool {
        const HISTORY_CLOSE_DEBOUNCE: Duration = Duration::from_millis(300);
        self.history_closed_at
            .map(|t| t.elapsed() < HISTORY_CLOSE_DEBOUNCE)
            .unwrap_or(false)
    }

    fn mark_history_popup_closed(&mut self, cx: &mut Context<Self>) {
        self.history_menu = None;
        self.history_closed_at = Some(Instant::now());
        cx.notify();
    }

    pub(crate) fn dismiss_history_popup(&mut self, cx: &mut Context<Self>) {
        if self.history_menu.is_none() {
            return;
        }

        let cancel_portal = self.has_pending_history_portal_session();
        self.mark_history_popup_closed(cx);
        self.sync_history_popup_window_from_cached_parent(cx);
        if cancel_portal {
            tracing::info!(
                target: "script_kit::agent_chat",
                event = "agent_chat_history_portal_dismissed_via_popup",
            );
            let _ = self.cancel_pending_portal_session(
                crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory,
                cx,
            );
        }
    }

    pub(crate) fn dismiss_history_popup_from_window(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if self.history_menu.is_none() {
            return;
        }

        let cancel_portal = self.has_pending_history_portal_session();
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_popup_closed",
            reason,
            "Closed Agent Chat history popup from detached window lifecycle"
        );
        self.mark_history_popup_closed(cx);
        if cancel_portal {
            tracing::info!(
                target: "script_kit::agent_chat",
                event = "agent_chat_history_portal_dismissed_from_window",
                reason,
            );
            let _ = self.cancel_pending_portal_session(
                crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory,
                cx,
            );
        }
    }

    fn char_to_byte_offset(text: &str, char_idx: usize) -> usize {
        text.char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(text.len())
    }

    fn telemetry_item_id(item: &ContextSelectorRow) -> String {
        match &item.kind {
            ContextSelectorRowKind::BuiltIn(_)
            | ContextSelectorRowKind::SlashCommand(_)
            | ContextSelectorRowKind::AgentChatProfile { .. } => item.id.to_string(),
            ContextSelectorRowKind::File(_) => format!("file:{}", item.label),
            ContextSelectorRowKind::Folder(_) => format!("folder:{}", item.label),
            ContextSelectorRowKind::Portal(_)
            | ContextSelectorRowKind::PortalPrefix(_)
            | ContextSelectorRowKind::PortalResult(_)
            | ContextSelectorRowKind::Inert => item.id.to_string(),
        }
    }

    fn cache_composer_parent_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let display = window.display(cx);
        let parent = AgentChatComposerParentWindow {
            handle: window.window_handle(),
            bounds: window.bounds(),
            display_id: display.as_ref().map(|display| display.id()),
            display_bounds: display.as_ref().map(|display| display.visible_bounds()),
        };
        self.composer_parent_window = Some(parent);
    }

    fn sync_agent_chat_popup_windows_from_cached_parent(&mut self, cx: &mut Context<Self>) {
        if self.is_setup_mode() {
            self.composer_picker_session = None;
            self.history_menu = None;
            crate::ai::agent_chat::ui::history_popup::close_history_popup_window(cx);
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_popup_sync_setup_mode_closed",
            );
            return;
        }

        self.sync_history_popup_window_from_cached_parent(cx);
    }

    fn profile_selector_entries(
        &self,
    ) -> Vec<crate::ai::agent_chat::profiles::AgentChatProfilePickerEntry> {
        let prefs = crate::config::load_user_preferences();
        let ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        crate::ai::agent_chat::profiles::agent_chat_profile_picker_entries(&prefs.ai, &ctx)
    }

    fn build_profile_picker_items(&self, query: &str) -> Vec<ContextSelectorRow> {
        let query_lower = query.trim().to_ascii_lowercase();
        let mut items = self
            .profile_selector_entries()
            .into_iter()
            .filter_map(|entry| {
                let haystack = format!("{} {}", entry.name, entry.id).to_ascii_lowercase();
                if !query_lower.is_empty() && !haystack.contains(&query_lower) {
                    return None;
                }
                let source = match entry.source {
                    crate::ai::agent_chat::profiles::AgentChatProfileSource::BuiltIn => "Built-in",
                    crate::ai::agent_chat::profiles::AgentChatProfileSource::User => "Custom",
                    crate::ai::agent_chat::profiles::AgentChatProfileSource::Mdflow => "Markdown",
                };
                let backend = "Pi";
                let score = if query_lower.is_empty() {
                    100
                } else if entry.name.to_ascii_lowercase().starts_with(&query_lower) {
                    200
                } else if entry.id.to_ascii_lowercase().starts_with(&query_lower) {
                    175
                } else {
                    125
                };
                Some(ContextSelectorRow {
                    id: SharedString::from(format!("agent-chat-profile:{}", entry.id)),
                    label: SharedString::from(entry.name),
                    description: SharedString::from(format!("{source} Agent Chat profile")),
                    meta: SharedString::from(format!("'{} · {backend}", entry.id)),
                    kind: ContextSelectorRowKind::AgentChatProfile {
                        profile_id: entry.id,
                        icon_name: entry.icon_name,
                    },
                    score,
                    label_highlight_indices: Vec::new(),
                    meta_highlight_indices: Vec::new(),
                })
            })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.label.to_string().cmp(&b.label.to_string()))
        });
        items
    }

    pub(crate) fn set_on_toggle_actions(
        &mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.on_toggle_actions = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_on_close_requested(
        &mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.on_close_requested = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_on_close_window_requested(
        &mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.on_close_window_requested = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_on_open_portal(
        &mut self,
        callback: impl Fn(crate::ai::context_selector::types::ContextPortalKind, &mut App) + 'static,
    ) {
        self.on_open_portal = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_on_profile_selected(
        &mut self,
        callback: impl Fn(String, &mut App) + 'static,
    ) {
        self.on_profile_selected = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_profile_display(
        &mut self,
        profile_id: String,
        profile_display_name: String,
        profile_icon_name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_set_profile_display_ignored_setup_mode",
                profile_display_name,
            );
            return;
        }

        self.live_thread().update(cx, |thread, cx| {
            thread.set_profile_display(
                profile_id,
                profile_display_name.into(),
                profile_icon_name,
                cx,
            );
        });
        cx.notify();
    }

    pub(crate) fn set_on_focused_text_expand_requested(
        &mut self,
        callback: impl Fn(&mut App) + 'static,
    ) {
        self.on_focused_text_expand_requested = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_on_focused_text_collapse_requested(
        &mut self,
        callback: impl Fn(&mut App) + 'static,
    ) {
        self.on_focused_text_collapse_requested = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_footer_host(&mut self, footer_host: AgentChatFooterHost) {
        self.footer_host = footer_host;
    }

    pub(crate) fn uses_external_footer_host(&self) -> bool {
        matches!(self.footer_host, AgentChatFooterHost::External)
    }

    fn inline_footer_height(&self) -> f32 {
        if self.uses_external_footer_host() {
            0.0
        } else {
            crate::window_resize::main_layout::HINT_STRIP_HEIGHT
        }
    }

    fn composer_height(&self) -> f32 {
        Self::composer_height_for_expanded(self.expanded_composer)
    }

    fn composer_height_for_expanded(expanded: bool) -> f32 {
        let line_count = if expanded { 6.0 } else { 1.0 };
        (Self::AGENT_CHAT_INPUT_PADDING_Y * 2.0) + (Self::AGENT_CHAT_INPUT_LINE_HEIGHT * line_count)
    }

    pub(crate) fn automation_layout_info(
        &self,
        target: &crate::protocol::AutomationWindowInfo,
        cx: &App,
    ) -> crate::protocol::LayoutInfo {
        use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
        use crate::ui::chrome as chrome_tokens;

        let (window_width, window_height) = target
            .bounds
            .as_ref()
            .map(|bounds| (bounds.width as f32, bounds.height as f32))
            .unwrap_or((480.0, 440.0));
        let composer_height = self.composer_height();
        let footer_height = self.inline_footer_height();
        let message_height = (window_height - composer_height - footer_height).max(0.0);
        let message_bounds = self.transcript_viewport_bounds_px(cx).unwrap_or((
            0.0,
            0.0,
            window_width,
            message_height,
        ));
        let composer_y = message_height;
        let footer_y = (window_height - footer_height).max(composer_y + composer_height);
        let mut components = Vec::new();

        components.push(
            LayoutComponentInfo::new("AgentChatDetachedWindow", LayoutComponentType::Container)
                .with_bounds(0.0, 0.0, window_width, window_height)
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FLOATING,
                    chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                    Some(chrome_tokens::LIQUID_GLASS_WINDOW_RADIUS_PX),
                )
                .with_visual_token("chrome.agentChatDetachedWindow")
                .with_flex_column()
                .with_depth(0)
                .with_explanation(
                    "Detached Agent Chat chat OS window root measured from the resolved automation target bounds.",
                ),
        );

        components.push(
            LayoutComponentInfo::new("AgentChatMessageViewport", LayoutComponentType::List)
                .with_bounds(
                    message_bounds.0,
                    message_bounds.1,
                    message_bounds.2,
                    message_bounds.3,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_CONTENT,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                )
                .with_visual_token("content.agent_chatMessages")
                .with_flex_grow(1.0)
                .with_depth(1)
                .with_parent("AgentChatDetachedWindow")
                .with_explanation(
                    "Scrollable Agent Chat transcript bounds measured from its ListState viewport.",
                ),
        );

        components.push(
            LayoutComponentInfo::new("AgentChatComposerBar", LayoutComponentType::Input)
                .with_bounds(0.0, composer_y, window_width, composer_height)
                .with_padding(
                    Self::AGENT_CHAT_INPUT_PADDING_Y,
                    Self::AGENT_CHAT_INPUT_PADDING_X,
                    Self::AGENT_CHAT_INPUT_PADDING_Y,
                    Self::AGENT_CHAT_INPUT_PADDING_X,
                )
                .with_visual_style(
                    chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                    chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                    Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                )
                .with_visual_token("chrome.agent_chatComposer")
                .with_depth(1)
                .with_parent("AgentChatDetachedWindow")
                .with_explanation(
                    "Agent Chat composer row: vertical padding plus the measured single-line input height.",
                ),
        );

        if footer_height > 0.0 {
            components.push(
                LayoutComponentInfo::new("AgentChatFooterRail", LayoutComponentType::Panel)
                    .with_bounds(0.0, footer_y, window_width, footer_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                    )
                    .with_visual_token("chrome.agent_chatFooterRail")
                    .with_depth(1)
                    .with_parent("AgentChatDetachedWindow")
                    .with_explanation(
                        "Inline hint/footer rail shown when Agent Chat owns its footer inside the detached window.",
                    ),
            );
        }

        if matches!(
            self.composer_picker_state(),
            crate::ai::agent_chat::ui::composer_state::AgentChatComposerPickerState::Open(_)
        ) {
            let picker_width = Self::composer_picker_width_for_window(window_width);
            components.push(
                LayoutComponentInfo::new("AgentChatComposerPicker", LayoutComponentType::Panel)
                    .with_bounds(
                        Self::AGENT_CHAT_INPUT_PADDING_X,
                        (composer_y + composer_height + Self::AGENT_CHAT_COMPOSER_PICKER_OFFSET_Y)
                            .min(window_height),
                        picker_width,
                        220.0_f32.min(window_height),
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FLOATING,
                        chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("chrome.agent_chatComposerPicker")
                    .with_depth(2)
                    .with_parent("AgentChatDetachedWindow")
                    .with_explanation(
                        "Composer slash/profile picker floating from the detached Agent Chat composer.",
                    ),
            );
        }

        LayoutInfo {
            window_width,
            window_height,
            prompt_type: "agentChatDetached".to_string(),
            components,
            handler_form: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub(crate) fn placeholder_automation_layout_info(
        target: &crate::protocol::AutomationWindowInfo,
    ) -> crate::protocol::LayoutInfo {
        use crate::protocol::{LayoutComponentInfo, LayoutComponentType, LayoutInfo};
        use crate::ui::chrome as chrome_tokens;

        let (window_width, window_height) = target
            .bounds
            .as_ref()
            .map(|bounds| (bounds.width as f32, bounds.height as f32))
            .unwrap_or((480.0, 440.0));
        let composer_height = Self::composer_height_for_expanded(false);
        let footer_height = crate::window_resize::main_layout::HINT_STRIP_HEIGHT;
        let message_height = (window_height - composer_height - footer_height).max(0.0);
        let footer_y = (window_height - footer_height).max(message_height + composer_height);

        LayoutInfo {
            window_width,
            window_height,
            prompt_type: "agentChatDetached".to_string(),
            components: vec![
                LayoutComponentInfo::new("AgentChatDetachedWindow", LayoutComponentType::Container)
                    .with_bounds(0.0, 0.0, window_width, window_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FLOATING,
                        chrome_tokens::MATERIAL_NS_VISUAL_EFFECT,
                        Some(chrome_tokens::LIQUID_GLASS_WINDOW_RADIUS_PX),
                    )
                    .with_visual_token("chrome.agentChatDetachedWindow")
                    .with_flex_column()
                    .with_depth(0)
                    .with_explanation(
                        "Detached Agent Chat placeholder window root measured from the resolved automation target bounds.",
                    ),
                LayoutComponentInfo::new("AgentChatMessageViewport", LayoutComponentType::List)
                    .with_bounds(0.0, 0.0, window_width, message_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_CONTENT,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("content.agent_chatMessages")
                    .with_flex_grow(1.0)
                    .with_depth(1)
                    .with_parent("AgentChatDetachedWindow")
                    .with_explanation(
                        "Placeholder Agent Chat transcript region above the composer and footer.",
                    ),
                LayoutComponentInfo::new("AgentChatComposerBar", LayoutComponentType::Input)
                    .with_bounds(0.0, message_height, window_width, composer_height)
                    .with_padding(
                        Self::AGENT_CHAT_INPUT_PADDING_Y,
                        Self::AGENT_CHAT_INPUT_PADDING_X,
                        Self::AGENT_CHAT_INPUT_PADDING_Y,
                        Self::AGENT_CHAT_INPUT_PADDING_X,
                    )
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX),
                    )
                    .with_visual_token("chrome.agent_chatComposer")
                    .with_depth(1)
                    .with_parent("AgentChatDetachedWindow")
                    .with_explanation(
                        "Agent Chat composer row: vertical padding plus the measured single-line input height.",
                    ),
                LayoutComponentInfo::new("AgentChatFooterRail", LayoutComponentType::Panel)
                    .with_bounds(0.0, footer_y, window_width, footer_height)
                    .with_visual_style(
                        chrome_tokens::CHROME_LAYER_FUNCTIONAL,
                        chrome_tokens::MATERIAL_SOLID_THEME_TOKEN,
                        Some(chrome_tokens::LIQUID_GLASS_COMPACT_RADIUS_PX),
                    )
                    .with_visual_token("chrome.agent_chatFooterRail")
                    .with_depth(1)
                    .with_parent("AgentChatDetachedWindow")
                    .with_explanation("Placeholder Agent Chat footer rail for window-shell proof."),
            ],
            handler_form: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub(crate) fn footer_snapshot(&self, cx: &App) -> AgentChatFooterSnapshot {
        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_footer_snapshot_hidden_setup_mode",
            );
            return AgentChatFooterSnapshot {
                visible: false,
                dot_status: crate::footer_popup::FooterDotStatus::Hidden,
                profile_display: String::new(),
                profile_icon_name: None,
                model_display: String::new(),
                status_text: None,
                buttons: Vec::new(),
                cwd_display: None,
            };
        }

        let thread = self.live_thread().read(cx);
        let visible = self.main_window_footer_visible_for_thread(thread);
        let cwd = thread.cwd().clone();
        let cwd_display = if cwd.as_os_str().is_empty() || cwd == std::path::Path::new(".") {
            None
        } else {
            let home = dirs::home_dir().unwrap_or_default();
            let display = if cwd.starts_with(&home) {
                format!("~/{}", cwd.strip_prefix(&home).unwrap_or(&cwd).display())
            } else {
                cwd.display().to_string()
            };
            Some(display)
        };
        AgentChatFooterSnapshot {
            visible,
            dot_status: self.footer_dot_status(cx),
            profile_display: thread.profile_display().to_string(),
            profile_icon_name: thread.profile_icon_name().map(str::to_string),
            model_display: thread.selected_model_display().to_string(),
            status_text: self.footer_status_text(cx),
            buttons: if visible {
                self.footer_buttons_for_thread(thread)
            } else {
                Vec::new()
            },
            cwd_display,
        }
    }

    pub(crate) fn main_window_footer_visible(&self, cx: &App) -> bool {
        if self.is_setup_mode() {
            return false;
        }

        let thread = self.live_thread().read(cx);
        self.main_window_footer_visible_for_thread(thread)
    }

    fn main_window_footer_visible_for_thread(&self, thread: &AgentChatThread) -> bool {
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini && self.focused_text.is_some() {
            return self.focused_text_mini_footer_visible_for_thread(thread);
        }
        true
    }

    pub(crate) fn agent_chat_detached_native_footer_config(
        &self,
        cx: &App,
    ) -> crate::footer_popup::MainWindowFooterConfig {
        use crate::footer_popup::{FooterButtonConfig, MainWindowFooterConfig};

        let snapshot = self.footer_snapshot(cx);
        let buttons = snapshot
            .buttons
            .iter()
            .map(|btn| {
                let mut config = FooterButtonConfig::new(btn.action, btn.key, btn.label)
                    .selected(btn.selected)
                    .enabled(btn.enabled);
                if let Some(reason) = btn.disabled_reason {
                    config = config.disabled_reason(reason);
                }
                config
            })
            .collect();

        let mut config = MainWindowFooterConfig::new("agent_chat", buttons);
        config.left_info = Some(snapshot.profile_left_info());

        config
    }

    fn ensure_native_footer_action_listener(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self._footer_action_task.is_some() {
            return;
        }

        let rx = crate::footer_popup::agent_chat_footer_action_channel()
            .1
            .clone();
        self._footer_action_task = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(action) = rx.recv().await {
                if let Err(error) = this.update_in(cx, |view, window, cx| {
                    view.dispatch_footer_button(action, window, cx);
                }) {
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "agent_chat_native_footer_action_dispatch_failed",
                        action = ?action,
                        %error,
                        "Failed to dispatch native footer action into AgentChatView"
                    );
                }
            }
        }));
    }

    fn footer_buttons_for_thread(
        &self,
        thread: &AgentChatThread,
    ) -> Vec<AgentChatFooterButtonSpec> {
        use crate::footer_popup::FooterAction;

        if self.focused_text.is_some() {
            return self.focused_text_visible_footer_buttons(thread);
        }

        let actions_selected = crate::actions::is_actions_window_open();
        let attach_picker_active = self.composer_picker_session.is_some()
            || self.agent_chat_spine_owns_list()
            || self.pending_portal_session.is_some();
        let mut buttons = Vec::new();

        match thread.status {
            AgentChatThreadStatus::Streaming => {
                buttons.push(AgentChatFooterButtonSpec {
                    action: FooterAction::Stop,
                    key: "Esc",
                    label: "Stop",
                    selected: false,
                    enabled: true,
                    disabled_reason: None,
                });
            }
            AgentChatThreadStatus::WaitingForPermission => {
                buttons.push(AgentChatFooterButtonSpec {
                    action: FooterAction::Run,
                    key: "↵",
                    label: "Send",
                    selected: false,
                    enabled: false,
                    disabled_reason: Some("waiting_for_permission"),
                });
            }
            AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error => {
                let input = thread.input.text();
                let raw_empty = input.is_empty();
                let blank = input.trim().is_empty();
                if raw_empty && Self::has_pastable_assistant_response(thread) {
                    buttons.push(AgentChatFooterButtonSpec {
                        action: FooterAction::PasteResponse,
                        key: "↵",
                        label: "Paste Response",
                        selected: false,
                        enabled: true,
                        disabled_reason: None,
                    });
                } else {
                    buttons.push(AgentChatFooterButtonSpec {
                        action: FooterAction::Run,
                        key: "↵",
                        label: if attach_picker_active {
                            "Attach"
                        } else {
                            "Send"
                        },
                        selected: false,
                        enabled: (attach_picker_active || !blank) && !self.context_capture_pending,
                        disabled_reason: if blank && !attach_picker_active {
                            Some("type_message_first")
                        } else if self.context_capture_pending {
                            Some("context_capture_pending")
                        } else {
                            None
                        },
                    });
                }
            }
        }

        buttons.push(AgentChatFooterButtonSpec {
            action: FooterAction::Actions,
            key: "⌘K",
            label: "Actions",
            selected: actions_selected,
            enabled: true,
            disabled_reason: None,
        });

        buttons
    }

    fn focused_text_visible_footer_buttons(
        &self,
        thread: &AgentChatThread,
    ) -> Vec<AgentChatFooterButtonSpec> {
        use crate::footer_popup::FooterAction;

        let Some(state) = self.focused_text.as_ref() else {
            return Vec::new();
        };

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini {
            let has_output = self.selected_focused_text_output(thread).is_some();
            let action_disabled_reason = if has_output {
                None
            } else {
                Some("assistant_output_required")
            };
            if !self.focused_text_mini_result_ready_for_thread(thread) || !has_output {
                return Vec::new();
            }
            return vec![AgentChatFooterButtonSpec {
                action: FooterAction::Replace,
                key: "↵",
                label: "Paste",
                selected: false,
                enabled: state.can_replace,
                disabled_reason: if !state.can_replace {
                    Some("replace_unavailable")
                } else {
                    action_disabled_reason
                },
            }];
        }

        let leading = match thread.status {
            AgentChatThreadStatus::Streaming => AgentChatFooterButtonSpec {
                action: FooterAction::Stop,
                key: "Esc",
                label: "Stop",
                selected: false,
                enabled: true,
                disabled_reason: None,
            },
            AgentChatThreadStatus::WaitingForPermission => AgentChatFooterButtonSpec {
                action: FooterAction::Run,
                key: "↵",
                label: "Send",
                selected: false,
                enabled: false,
                disabled_reason: Some("waiting_for_permission"),
            },
            AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error => {
                AgentChatFooterButtonSpec {
                    action: FooterAction::Run,
                    key: "↵",
                    label: "Send",
                    selected: false,
                    enabled: !thread.input.text().trim().is_empty()
                        && !self.context_capture_pending,
                    disabled_reason: if thread.input.text().trim().is_empty() {
                        Some("type_message_first")
                    } else if self.context_capture_pending {
                        Some("context_capture_pending")
                    } else {
                        None
                    },
                }
            }
        };

        vec![
            leading,
            AgentChatFooterButtonSpec {
                action: FooterAction::Actions,
                key: "⌘K",
                label: "Actions",
                selected: crate::actions::is_actions_window_open(),
                enabled: true,
                disabled_reason: None,
            },
        ]
    }

    fn focused_text_semantic_actions(
        &self,
        thread: &AgentChatThread,
    ) -> Vec<FocusedTextSemanticActionSpec> {
        let Some(state) = self.focused_text.as_ref() else {
            return Vec::new();
        };
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && !self.focused_text_mini_result_ready_for_thread(thread)
        {
            return Vec::new();
        }

        let has_output = self.selected_focused_text_output(thread).is_some();
        let streaming = matches!(thread.status, AgentChatThreadStatus::Streaming);
        let output_required = if has_output {
            None
        } else {
            Some("assistant_output_required")
        };

        let replace_disabled = if !state.can_replace {
            Some("replace_unavailable")
        } else {
            output_required
        };
        let append_disabled = if !state.can_append {
            Some("append_unavailable")
        } else {
            output_required
        };
        let copy_disabled = if !state.can_copy {
            Some("copy_unavailable")
        } else {
            output_required
        };
        let retryable = self.has_retry_request();
        let expanded = self.ui_variant != AgentChatUiVariant::FocusedTextMini;

        let mut actions = vec![
            FocusedTextSemanticActionSpec {
                semantic_id: "focused-text-action-replace",
                action_value: "focused-text-action-replace",
                label: "Replace Selected Text",
                shortcut: if expanded { "⌘R" } else { "⌘↵" },
                enabled: !streaming && state.can_replace && has_output,
                disabled_reason: if streaming {
                    Some("streaming")
                } else {
                    replace_disabled
                },
            },
            FocusedTextSemanticActionSpec {
                semantic_id: "focused-text-action-append",
                action_value: "focused-text-action-append",
                label: "Append to Selected Text",
                shortcut: "⌘A",
                enabled: !streaming && state.can_append && has_output,
                disabled_reason: if streaming {
                    Some("streaming")
                } else {
                    append_disabled
                },
            },
            FocusedTextSemanticActionSpec {
                semantic_id: "focused-text-action-copy",
                action_value: "focused-text-action-copy",
                label: "Copy Response",
                shortcut: "⌘C",
                enabled: !streaming && state.can_copy && has_output,
                disabled_reason: if streaming {
                    Some("streaming")
                } else {
                    copy_disabled
                },
            },
        ];
        if !expanded {
            actions.push(FocusedTextSemanticActionSpec {
                semantic_id: "focused-text-action-expand",
                action_value: "focused-text-action-expand",
                label: "Chat",
                shortcut: "⌘↵",
                enabled: true,
                disabled_reason: None,
            });
        }
        actions.extend([
            FocusedTextSemanticActionSpec {
                semantic_id: "focused-text-action-stop",
                action_value: "focused-text-action-stop",
                label: "Stop",
                shortcut: "Esc",
                enabled: streaming,
                disabled_reason: if streaming {
                    None
                } else {
                    Some("not_streaming")
                },
            },
            FocusedTextSemanticActionSpec {
                semantic_id: "focused-text-action-retry",
                action_value: "focused-text-action-retry",
                label: "Retry",
                shortcut: "⌘⇧R",
                enabled: retryable,
                disabled_reason: if retryable {
                    None
                } else {
                    Some("not_retryable")
                },
            },
        ]);
        actions
    }

    fn has_pastable_assistant_response(thread: &AgentChatThread) -> bool {
        thread.messages.iter().rev().any(|message| {
            matches!(message.role, AgentChatThreadMessageRole::Assistant)
                && !message.body.trim().is_empty()
        })
    }

    fn latest_assistant_response_text(thread: &AgentChatThread) -> Option<String> {
        thread
            .messages
            .iter()
            .rev()
            .find(|message| {
                matches!(message.role, AgentChatThreadMessageRole::Assistant)
                    && !message.body.trim().is_empty()
            })
            .map(|message| message.body.to_string())
    }

    fn latest_assistant_response_after_latest_user(thread: &AgentChatThread) -> Option<String> {
        Self::latest_assistant_response_after_latest_user_in_messages(&thread.messages)
    }

    fn latest_assistant_response_after_latest_user_in_messages(
        messages: &[AgentChatThreadMessage],
    ) -> Option<String> {
        let last_user_index = messages
            .iter()
            .rposition(|message| matches!(message.role, AgentChatThreadMessageRole::User))?;
        messages[last_user_index + 1..]
            .iter()
            .rev()
            .find(|message| {
                matches!(message.role, AgentChatThreadMessageRole::Assistant)
                    && !message.body.trim().is_empty()
            })
            .map(|message| message.body.to_string())
    }

    fn focused_text_variation_angles() -> [crate::ai::focused_text::FocusedTextPromptAngle; 3] {
        use crate::ai::focused_text::FocusedTextPromptAngle;
        [
            FocusedTextPromptAngle::Conservative,
            FocusedTextPromptAngle::Balanced,
            FocusedTextPromptAngle::Creative,
        ]
    }

    fn cancel_isolated_variation_processes(&mut self) {
        for flag in &self.focused_text_variation_cancel_flags {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.focused_text_variation_cancel_flags.clear();
    }

    fn reset_focused_text_variations_for_submit(&mut self) {
        self.cancel_isolated_variation_processes();
        self.focused_text_variation_tasks.clear();
        self.focused_text_variation_generation += 1;
        self.focused_text_selected_variation = None;
        self.focused_text_editing_variation = None;
        self.focused_text_variations = Self::focused_text_variation_angles()
            .iter()
            .copied()
            .map(FocusedTextVariationState::streaming)
            .collect();
    }

    fn clear_focused_text_variations(&mut self) {
        self.cancel_isolated_variation_processes();
        self.focused_text_variation_tasks.clear();
        self.focused_text_variations.clear();
        self.focused_text_variation_history.clear();
        self.focused_text_variation_history_index = None;
        self.focused_text_selected_variation = None;
        self.focused_text_editing_variation = None;
    }

    fn select_first_completed_focused_text_variation(&mut self) {
        if self.focused_text_selected_variation.is_some() {
            return;
        }
        let Some(index) = self.focused_text_variations.iter().position(|variation| {
            variation.status == FocusedTextVariationStatus::Complete
                && !variation.text.trim().is_empty()
        }) else {
            return;
        };
        self.focused_text_selected_variation = Some(index);
        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variation_auto_selected",
            index,
            angle = self.focused_text_variations[index].angle.id(),
            text_len = self.focused_text_variations[index].text.chars().count(),
        );
    }

    fn mark_focused_text_variation_failed(
        &mut self,
        index: usize,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if let Some(variation) = self.focused_text_variations.get_mut(index) {
            variation.status = FocusedTextVariationStatus::Error;
            variation.error = Some(error);
        }
        cx.notify();
    }

    fn sync_balanced_focused_text_variation(
        &mut self,
        messages: &[AgentChatThreadMessage],
        status: AgentChatThreadStatus,
        cx: &mut Context<Self>,
    ) {
        if self.focused_text.is_none()
            || self.focused_text_variations.len() <= FOCUSED_TEXT_BALANCED_VARIATION_INDEX
        {
            return;
        }

        let latest_text = Self::latest_assistant_response_after_latest_user_in_messages(messages)
            .unwrap_or_default();
        {
            let editing_balanced =
                self.focused_text_editing_variation == Some(FOCUSED_TEXT_BALANCED_VARIATION_INDEX);
            let variation =
                &mut self.focused_text_variations[FOCUSED_TEXT_BALANCED_VARIATION_INDEX];
            if editing_balanced {
                variation.status = FocusedTextVariationStatus::Complete;
                variation.error = None;
            } else {
                if !latest_text.trim().is_empty() {
                    variation.text = latest_text;
                }
                variation.status = match status {
                    AgentChatThreadStatus::Streaming
                    | AgentChatThreadStatus::WaitingForPermission => {
                        FocusedTextVariationStatus::Streaming
                    }
                    AgentChatThreadStatus::Idle if !variation.text.trim().is_empty() => {
                        FocusedTextVariationStatus::Complete
                    }
                    AgentChatThreadStatus::Error => {
                        if variation.error.is_none() {
                            variation.error = Some("balanced_turn_failed".to_string());
                        }
                        FocusedTextVariationStatus::Error
                    }
                    AgentChatThreadStatus::Idle => FocusedTextVariationStatus::Idle,
                };
            }
        }

        self.select_first_completed_focused_text_variation();
        cx.notify();
    }

    fn apply_focused_text_variation_event(
        &mut self,
        index: usize,
        generation: u64,
        event: AgentChatEvent,
        cx: &mut Context<Self>,
    ) {
        if generation != self.focused_text_variation_generation {
            return;
        }
        if index >= self.focused_text_variations.len() {
            return;
        }

        if self.focused_text_editing_variation == Some(index) {
            if matches!(
                event,
                AgentChatEvent::TurnFinished { .. }
                    | AgentChatEvent::Failed { .. }
                    | AgentChatEvent::SetupRequired { .. }
            ) {
                if let Some(variation) = self.focused_text_variations.get_mut(index) {
                    variation.status = FocusedTextVariationStatus::Complete;
                    variation.error = None;
                }
                cx.notify();
            }
            return;
        }

        match event {
            AgentChatEvent::AgentMessageDelta(chunk) => {
                let variation = &mut self.focused_text_variations[index];
                variation.text.push_str(&chunk);
                variation.status = FocusedTextVariationStatus::Streaming;
                variation.error = None;
            }
            AgentChatEvent::TurnFinished { .. } => {
                let variation = &mut self.focused_text_variations[index];
                if variation.status != FocusedTextVariationStatus::Error {
                    variation.status = FocusedTextVariationStatus::Complete;
                }
            }
            AgentChatEvent::Failed { error } => {
                let variation = &mut self.focused_text_variations[index];
                variation.status = FocusedTextVariationStatus::Error;
                variation.error = Some(error);
            }
            AgentChatEvent::SetupRequired { reason, .. } => {
                let variation = &mut self.focused_text_variations[index];
                variation.status = FocusedTextVariationStatus::Error;
                variation.error = Some(format!("setup_required:{reason}"));
            }
            AgentChatEvent::UserMessageDelta(_)
            | AgentChatEvent::AgentThoughtDelta(_)
            | AgentChatEvent::ToolCallStarted { .. }
            | AgentChatEvent::ToolCallUpdated { .. }
            | AgentChatEvent::PlanUpdated { .. }
            | AgentChatEvent::AvailableCommandsUpdated { .. }
            | AgentChatEvent::ModeChanged { .. }
            | AgentChatEvent::UsageUpdated { .. }
            | AgentChatEvent::ModelsAvailable { .. }
            | AgentChatEvent::ForkPointsAvailable { .. }
            | AgentChatEvent::ForkCompleted { .. } => {}
        }

        self.select_first_completed_focused_text_variation();
        cx.notify();
    }

    fn spawn_focused_text_variation_task(
        &mut self,
        index: usize,
        rx: AgentChatEventRx,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity().downgrade();
        let generation = self.focused_text_variation_generation;
        let task = cx.spawn(async move |_this, cx| {
            while let Ok(event) = rx.recv().await {
                let terminal = matches!(
                    event,
                    AgentChatEvent::TurnFinished { .. }
                        | AgentChatEvent::Failed { .. }
                        | AgentChatEvent::SetupRequired { .. }
                );
                let view_ref = view.clone();
                cx.update(|cx| {
                    if let Some(entity) = view_ref.upgrade() {
                        entity.update(cx, |this, cx| {
                            this.apply_focused_text_variation_event(index, generation, event, cx);
                        });
                    }
                });
                if terminal {
                    break;
                }
            }
        });
        self.focused_text_variation_tasks.push(task);
    }

    /// Text to apply or paste back into the host app. Prefers the selected
    /// focused-text variation when variations exist; otherwise the latest
    /// assistant message from the thread.
    pub(crate) fn pastable_response_text(&self, cx: &App) -> Option<String> {
        if self.is_setup_mode() {
            return None;
        }
        let thread = self.live_thread().read(cx);
        self.selected_focused_text_output(thread)
    }

    fn selected_focused_text_output(&self, thread: &AgentChatThread) -> Option<String> {
        if self.focused_text.is_some() {
            if let Some(text) = self
                .focused_text_selected_variation
                .and_then(|index| self.focused_text_variations.get(index))
                .filter(|variation| !variation.text.trim().is_empty())
                .map(|variation| variation.text.clone())
            {
                return Some(text);
            }

            if let Some(text) = self
                .focused_text_variations
                .iter()
                .find(|variation| {
                    variation.status == FocusedTextVariationStatus::Complete
                        && !variation.text.trim().is_empty()
                })
                .map(|variation| variation.text.clone())
            {
                return Some(text);
            }

            return Self::latest_assistant_response_after_latest_user(thread);
        }

        Self::latest_assistant_response_text(thread)
    }

    pub(crate) fn focused_text_variation_snapshots(&self) -> Vec<FocusedTextVariationSnapshot> {
        self.focused_text_variations
            .iter()
            .enumerate()
            .map(|(index, variation)| {
                variation.snapshot(index, self.focused_text_selected_variation == Some(index))
            })
            .collect()
    }

    pub(crate) fn select_focused_text_variation(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if index >= self.focused_text_variations.len() {
            return false;
        }
        if self.focused_text_selected_variation == Some(index) {
            return true;
        }
        self.focused_text_editing_variation = None;
        self.focused_text_selected_variation = Some(index);
        self.scope_focused = false;
        self.cursor_visible = true;
        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variation_selected",
            index,
            angle = self.focused_text_variations[index].angle.id(),
            status = self.focused_text_variations[index].status.state_id(),
            text_len = self.focused_text_variations[index].text.chars().count(),
        );
        cx.notify();
        true
    }

    /// Tab advances through the variation cards and wraps back to the first,
    /// unlike Up/Down which saturate at the edges.
    fn cycle_focused_text_variation_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(next) = Self::next_variation_index_wrapping(
            self.focused_text_selected_variation,
            self.focused_text_variations.len(),
        ) else {
            return false;
        };
        self.select_focused_text_variation(next, cx)
    }

    fn next_variation_index_wrapping(current: Option<usize>, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        Some(match current.filter(|index| *index < count) {
            Some(index) => (index + 1) % count,
            None => 0,
        })
    }

    /// Plain Tab cycles the focused-text variation cards so the user can pick
    /// the rewrite to paste. Shift+Tab stays reserved for the profile picker;
    /// the scope toggle keeps Tab while no variations exist (ask phase).
    fn handle_focused_text_variation_tab(
        &mut self,
        has_shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if has_shift
            || self.ui_variant != AgentChatUiVariant::FocusedTextMini
            || self.focused_text.is_none()
            || self.focused_text_variations.is_empty()
            || self.focused_text_editing_variation.is_some()
            || self.scope_focused
            || self.composer_picker_session.is_some()
        {
            return false;
        }
        self.cycle_focused_text_variation_selection(cx)
    }

    fn move_focused_text_variation_selection(
        &mut self,
        direction: i32,
        cx: &mut Context<Self>,
    ) -> bool {
        let count = self.focused_text_variations.len();
        if count == 0 {
            return false;
        }
        let current = self
            .focused_text_selected_variation
            .filter(|index| *index < count);
        let next = match (current, direction < 0) {
            (Some(index), true) => index.saturating_sub(1),
            (Some(index), false) => (index + 1).min(count.saturating_sub(1)),
            (None, true) => count.saturating_sub(1),
            (None, false) => 0,
        };
        self.select_focused_text_variation(next, cx)
    }

    fn save_focused_text_variation_history_slot(&mut self, index: usize) {
        if let Some(entry) = self.focused_text_variation_history.get_mut(index) {
            *entry = self.focused_text_variations.clone();
        }
    }

    fn navigate_focused_text_variation_history(
        &mut self,
        delta: i32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.focused_text_variation_history.is_empty() {
            return false;
        }

        if self.focused_text_variation_history_index.is_none() && delta < 0 {
            let should_push =
                self.focused_text_variation_history.last() != Some(&self.focused_text_variations);
            if should_push {
                self.focused_text_variation_history
                    .push(self.focused_text_variations.clone());
            }
        }

        let len = self.focused_text_variation_history.len();
        let current = self
            .focused_text_variation_history_index
            .unwrap_or(len.saturating_sub(1));
        let target = current as i32 + delta;
        if target < 0 {
            return false;
        }
        let target = target as usize;

        if target >= len {
            if delta <= 0 {
                return false;
            }
            self.save_focused_text_variation_history_slot(current);
            self.focused_text_variation_history_index = None;
            self.focused_text_selected_variation = None;
            self.focused_text_editing_variation = None;
            self.select_first_completed_focused_text_variation();
            cx.notify();
            return true;
        }

        self.save_focused_text_variation_history_slot(current);
        self.focused_text_variations = self.focused_text_variation_history[target].clone();
        self.focused_text_variation_history_index = Some(target);
        self.focused_text_selected_variation = None;
        self.focused_text_editing_variation = None;
        self.select_first_completed_focused_text_variation();
        cx.notify();
        true
    }

    fn regenerate_focused_text_variations(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.focused_text_selected_variation else {
            return;
        };
        let source_text = self
            .focused_text_variations
            .get(index)
            .map(|variation| variation.text.clone())
            .unwrap_or_default();
        if source_text.trim().is_empty() {
            return;
        }

        if !self.focused_text_variations.is_empty() {
            self.focused_text_variation_history
                .push(self.focused_text_variations.clone());
            self.focused_text_variation_history_index = None;
        }

        let semantics = {
            let thread = self.live_thread().read(cx);
            self.focused_text_enter_semantics_for_thread(thread)
        };

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variations_regenerated",
            source_index = index,
            source_text_len = source_text.chars().count(),
            history_len = self.focused_text_variation_history.len(),
        );

        if let Err(error) = self.submit_focused_text_turn(semantics, cx, Some(source_text)) {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_regenerate_failed",
                error = %error,
            );
        }
    }

    fn latest_user_prompt_for_display(thread: &AgentChatThread) -> Option<String> {
        thread
            .messages
            .iter()
            .rev()
            .find(|message| {
                matches!(message.role, AgentChatThreadMessageRole::User)
                    && !message.body.trim().is_empty()
            })
            .map(|message| message.body.to_string())
    }

    fn has_submitted_user_turn(thread: &AgentChatThread) -> bool {
        thread
            .messages
            .iter()
            .any(|message| matches!(message.role, AgentChatThreadMessageRole::User))
    }

    fn focused_text_mini_phase_for_thread(
        &self,
        thread: &AgentChatThread,
    ) -> Option<FocusedTextMiniPhase> {
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini || self.focused_text.is_none() {
            return None;
        }

        let active = matches!(
            thread.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        );
        let has_output = Self::latest_assistant_response_after_latest_user(thread).is_some();
        let has_variations = !self.focused_text_variations.is_empty();
        let all_variations_failed = has_variations
            && self
                .focused_text_variations
                .iter()
                .all(|v| v.status == FocusedTextVariationStatus::Error);

        if !active && has_variations && all_variations_failed {
            return Some(FocusedTextMiniPhase::Error);
        }

        match (active, has_output || has_variations) {
            (true, false) => Some(FocusedTextMiniPhase::Loading),
            (true, true) => Some(FocusedTextMiniPhase::Streaming),
            (false, true) => Some(FocusedTextMiniPhase::Result),
            (false, false) => Some(FocusedTextMiniPhase::InputOnly),
        }
    }

    fn focused_text_input_locked_for_thread(&self, thread: &AgentChatThread) -> bool {
        matches!(
            self.focused_text_mini_phase_for_thread(thread),
            Some(FocusedTextMiniPhase::Loading | FocusedTextMiniPhase::Streaming)
        )
    }

    fn focused_text_locked_input_allows_key(key: &str) -> bool {
        crate::ui_foundation::is_key_escape(key)
            || crate::ui_foundation::is_key_enter(key)
            || crate::ui_foundation::is_key_up(key)
            || crate::ui_foundation::is_key_down(key)
            || crate::ui_foundation::is_key_left(key)
            || crate::ui_foundation::is_key_right(key)
            || key.eq_ignore_ascii_case("home")
            || key.eq_ignore_ascii_case("end")
            || key.eq_ignore_ascii_case("pageup")
            || key.eq_ignore_ascii_case("pagedown")
    }

    fn focused_text_mini_result_ready_for_thread(&self, thread: &AgentChatThread) -> bool {
        matches!(
            self.focused_text_mini_phase_for_thread(thread),
            Some(FocusedTextMiniPhase::Result)
        )
    }

    fn focused_text_mini_footer_visible_for_thread(&self, thread: &AgentChatThread) -> bool {
        self.focused_text_mini_result_ready_for_thread(thread)
    }

    fn focused_text_state_phase_for_thread(&self, thread: &AgentChatThread) -> &'static str {
        if self.focused_text.is_some() && self.ui_variant != AgentChatUiVariant::FocusedTextMini {
            return "expanded";
        }
        self.focused_text_mini_phase_for_thread(thread)
            .map(FocusedTextMiniPhase::state_id)
            .unwrap_or("unknown")
    }

    fn focused_text_compact_count(value: usize) -> String {
        if value >= 1000 {
            format!("{:.1}K", value as f32 / 1000.0)
        } else {
            value.to_string()
        }
    }

    fn focused_text_context_fingerprint(state: &FocusedTextAgentChatState) -> String {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in state.session_id.0.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for byte in state.app_name.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= state.char_count as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= state.word_count as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        format!("fnv1a64:{hash:016x}")
    }

    fn focused_text_state_snapshot(
        &self,
        thread: &AgentChatThread,
    ) -> Option<crate::protocol::AgentChatFocusedTextState> {
        let state = self.focused_text.as_ref()?;
        let phase = self.focused_text_state_phase_for_thread(thread);
        let footer_visible = self.main_window_footer_visible_for_thread(thread);
        let submitted_prompt_locked = self.focused_text_input_locked_for_thread(thread);
        let submitted_prompt_char_count = if submitted_prompt_locked {
            Self::latest_user_prompt_for_display(thread).map(|value| value.chars().count())
        } else {
            None
        };
        let context_present = matches!(state.context_status, FocusedTextContextStatus::Captured);
        Some(crate::protocol::AgentChatFocusedTextState {
            mode: if self.ui_variant == AgentChatUiVariant::FocusedTextMini {
                "mini".to_string()
            } else {
                "expanded".to_string()
            },
            phase: phase.to_string(),
            footer_visible,
            actions_visible: footer_visible && phase != "inputOnly",
            can_expand_to_chat: self.focused_text.is_some(),
            session_id: state.session_id.to_string(),
            app_name: state.app_name.clone(),
            char_count: state.char_count,
            word_count: state.word_count,
            context_present,
            context_status: state.context_status.state_id().to_string(),
            context_failure_code: state.context_status.failure_code(),
            context_fingerprint: context_present
                .then(|| Self::focused_text_context_fingerprint(state)),
            submitted_prompt_locked,
            submitted_prompt_char_count,
            input_redacted: self.ui_variant == AgentChatUiVariant::FocusedTextMini,
            can_replace: state.can_replace,
            can_append: state.can_append,
            can_copy: state.can_copy,
            has_output: self.selected_focused_text_output(thread).is_some(),
            last_apply_action: state
                .last_apply_receipt
                .as_ref()
                .map(|receipt| format!("{:?}", receipt.action).to_lowercase()),
            last_action_receipt: state.last_action_receipt.clone(),
        })
    }

    pub(crate) fn collect_focused_text_mini_elements(
        &self,
        limit: usize,
        cx: &App,
    ) -> Vec<crate::protocol::ElementInfo> {
        if self.is_setup_mode() || self.build_setup_protocol_snapshot(cx).is_some() {
            return Vec::new();
        }

        let thread = self.live_thread().read(cx);
        let Some(focused_text) = self.focused_text_state_snapshot(thread) else {
            return Vec::new();
        };
        let result_ready = focused_text.phase == "result";
        let input_locked = focused_text.submitted_prompt_locked;
        let input_status = if input_locked {
            "submitted_prompt_locked"
        } else if thread.input.text().is_empty() {
            "empty"
        } else {
            "draft_present"
        };
        let context_status_text = if focused_text.context_status == "captured" {
            format!("{} words", focused_text.word_count)
        } else {
            "redacted".to_string()
        };

        let mut elements = vec![
            crate::protocol::ElementInfo {
                semantic_id: "focused-text-mini-root".to_string(),
                element_type: crate::protocol::ElementType::Panel,
                text: Some(format!(
                    "{} · {} chars · {} words",
                    focused_text.app_name, focused_text.char_count, focused_text.word_count
                )),
                value: Some(self.ui_variant.state_id().to_string()),
                selected: None,
                focused: None,
                index: None,
                role: Some("focused-text-mini".to_string()),
                kind: Some(focused_text.phase.clone()),
                source: Some("focusedText".to_string()),
                source_name: Some(focused_text.app_name.clone()),
                selectable: Some(false),
                status_kind: Some(Self::agent_chat_thread_status_label(thread.status).to_string()),
                action_disabled: None,
                style: None,
            },
            crate::protocol::ElementInfo {
                semantic_id: "focused-text-input".to_string(),
                element_type: crate::protocol::ElementType::Input,
                text: Some("Instruction".to_string()),
                value: None,
                selected: None,
                focused: Some(!input_locked),
                index: None,
                role: Some("composer".to_string()),
                kind: Some("focused-text-instruction".to_string()),
                source: Some("focusedText".to_string()),
                source_name: None,
                selectable: Some(!input_locked),
                status_kind: Some(input_status.to_string()),
                action_disabled: input_locked.then(|| "submitted_prompt_locked".to_string()),
                style: None,
            },
            crate::protocol::ElementInfo {
                semantic_id: "focused-text-context-badge".to_string(),
                element_type: crate::protocol::ElementType::Panel,
                text: Some("App".to_string()),
                value: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("context-badge".to_string()),
                kind: Some("redacted-context".to_string()),
                source: Some("focusedText".to_string()),
                source_name: None,
                selectable: Some(false),
                status_kind: None,
                action_disabled: None,
                style: None,
            },
            crate::protocol::ElementInfo {
                semantic_id: "focused-text-context-status".to_string(),
                element_type: crate::protocol::ElementType::Panel,
                text: Some(context_status_text),
                value: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("context-status".to_string()),
                kind: Some(focused_text.context_status.clone()),
                source: Some("focusedText".to_string()),
                source_name: None,
                selectable: Some(false),
                status_kind: Some(if focused_text.context_status == "captured" {
                    "captured".to_string()
                } else {
                    "capture_failed".to_string()
                }),
                action_disabled: None,
                style: None,
            },
            crate::protocol::ElementInfo {
                semantic_id: "focused-text-profile-icon".to_string(),
                element_type: crate::protocol::ElementType::Panel,
                text: Some("Profile".to_string()),
                value: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("profile-icon".to_string()),
                kind: Some("redacted-profile".to_string()),
                source: Some("focusedText".to_string()),
                source_name: None,
                selectable: Some(false),
                status_kind: Some(if input_locked {
                    "working".to_string()
                } else {
                    "idle".to_string()
                }),
                action_disabled: None,
                style: None,
            },
        ];

        if result_ready {
            elements.push(crate::protocol::ElementInfo {
                semantic_id: "focused-text-preview".to_string(),
                element_type: crate::protocol::ElementType::Panel,
                text: Some(format!(
                    "{} assistant output",
                    if focused_text.has_output { "has" } else { "no" }
                )),
                value: None,
                selected: None,
                focused: None,
                index: None,
                role: Some("preview".to_string()),
                kind: Some("redacted-output".to_string()),
                source: Some("focusedText".to_string()),
                source_name: None,
                selectable: Some(false),
                status_kind: Some(if focused_text.has_output {
                    "output_ready".to_string()
                } else {
                    "output_empty".to_string()
                }),
                action_disabled: None,
                style: None,
            });
        }

        for action in self.focused_text_semantic_actions(thread) {
            elements.push(crate::protocol::ElementInfo {
                semantic_id: action.semantic_id.to_string(),
                element_type: crate::protocol::ElementType::Button,
                text: Some(action.label.to_string()),
                value: Some(action.action_value.to_string()),
                selected: Some(false),
                focused: None,
                index: None,
                role: Some("focused-text-action".to_string()),
                kind: Some(action.shortcut.to_string()),
                source: Some("focusedText".to_string()),
                source_name: Some("Cmd+K".to_string()),
                selectable: Some(action.enabled),
                status_kind: None,
                action_disabled: action.disabled_reason.map(str::to_string),
                style: None,
            });
        }

        if elements.len() > limit {
            elements.truncate(limit);
        }
        elements
    }

    /// Apply-back for focused text (Cmd+Enter Replace/Append/Copy, footer
    /// Replace). Uses `selected_focused_text_output` so the selected variation
    /// is applied, not the raw thread assistant message.
    fn apply_focused_text_output(
        &mut self,
        action: crate::ai::focused_text::FocusedTextApplyAction,
        cx: &mut Context<Self>,
    ) -> crate::protocol::AgentChatFocusedTextActionReceipt {
        let before_ui_variant = self.ui_variant.state_id().to_string();
        let output = {
            let thread = self.live_thread().read(cx);
            self.selected_focused_text_output(thread)
        };
        let output_length = output
            .as_ref()
            .map(|value| value.chars().count())
            .unwrap_or(0);
        let Some(output) = output else {
            tracing::warn!(
                target: "script_kit::focused_text",
                event = "focused_text_apply_skipped_no_output",
                action = ?action,
            );
            let receipt = crate::protocol::AgentChatFocusedTextActionReceipt {
                action: format!("{action:?}").to_lowercase(),
                success: false,
                changed_text: false,
                copied_to_clipboard: false,
                before_ui_variant: before_ui_variant.clone(),
                after_ui_variant: before_ui_variant,
                output_length,
                error_code: Some("no_output".to_string()),
            };
            if let Some(state) = self.focused_text.as_mut() {
                state.last_action_receipt = Some(receipt.clone());
            }
            cx.notify();
            return receipt;
        };

        let Some(state) = self.focused_text.as_mut() else {
            return crate::protocol::AgentChatFocusedTextActionReceipt {
                action: format!("{action:?}").to_lowercase(),
                success: false,
                changed_text: false,
                copied_to_clipboard: false,
                before_ui_variant: before_ui_variant.clone(),
                after_ui_variant: before_ui_variant,
                output_length,
                error_code: Some("no_focused_text".to_string()),
            };
        };

        let mutation = match action {
            crate::ai::focused_text::FocusedTextApplyAction::Replace => {
                crate::ai::focused_text::FocusedTextMutation::Replace {
                    session_id: state.session_id.clone(),
                    text: output,
                }
            }
            crate::ai::focused_text::FocusedTextApplyAction::Append => {
                crate::ai::focused_text::FocusedTextMutation::Append {
                    session_id: state.session_id.clone(),
                    text: output,
                }
            }
            crate::ai::focused_text::FocusedTextApplyAction::Copy => {
                crate::ai::focused_text::FocusedTextMutation::Copy { text: output }
            }
        };

        let bridge = crate::ai::focused_text::SystemFocusedTextPlatformBridge;
        match crate::ai::focused_text::FocusedTextPlatformBridge::apply_text_mutation(
            &bridge, mutation,
        ) {
            Ok(receipt) => {
                let action_receipt = crate::protocol::AgentChatFocusedTextActionReceipt {
                    action: format!("{:?}", receipt.action).to_lowercase(),
                    success: receipt.success,
                    changed_text: receipt.changed_text,
                    copied_to_clipboard: receipt.copied_to_clipboard,
                    before_ui_variant: before_ui_variant.clone(),
                    after_ui_variant: self.ui_variant.state_id().to_string(),
                    output_length,
                    error_code: None,
                };
                tracing::info!(
                    target: "script_kit::focused_text",
                    event = "focused_text_apply_complete",
                    action = ?receipt.action,
                    success = receipt.success,
                    changed_text = receipt.changed_text,
                    copied_to_clipboard = receipt.copied_to_clipboard,
                    app_name = %state.app_name,
                    chars = state.char_count,
                );
                state.last_apply_receipt = Some(receipt);
                state.last_action_receipt = Some(action_receipt.clone());
                cx.notify();
                action_receipt
            }
            Err(error) => {
                let action_receipt = crate::protocol::AgentChatFocusedTextActionReceipt {
                    action: format!("{action:?}").to_lowercase(),
                    success: false,
                    changed_text: false,
                    copied_to_clipboard: false,
                    before_ui_variant: before_ui_variant.clone(),
                    after_ui_variant: self.ui_variant.state_id().to_string(),
                    output_length,
                    error_code: Some("mutation_failed".to_string()),
                };
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "focused_text_apply_failed",
                    action = ?action,
                    app_name = %state.app_name,
                    chars = state.char_count,
                    error = %error,
                );
                state.last_action_receipt = Some(action_receipt.clone());
                cx.notify();
                action_receipt
            }
        }
    }

    pub(crate) fn perform_focused_text_mini_action(
        &mut self,
        action: FocusedTextMiniAction,
        cx: &mut Context<Self>,
    ) -> crate::protocol::AgentChatFocusedTextActionReceipt {
        if let Some(apply_action) = action.apply_action() {
            return self.apply_focused_text_output(apply_action, cx);
        }

        let before_ui_variant = self.ui_variant.state_id().to_string();
        let output_length = {
            let thread = self.live_thread().read(cx);
            self.selected_focused_text_output(thread)
                .map(|value| value.chars().count())
                .unwrap_or(0)
        };

        let mut success = self.focused_text.is_some();
        let mut error_code = None;

        match action {
            FocusedTextMiniAction::Expand => {
                if success {
                    if self.ui_variant == AgentChatUiVariant::FocusedTextMini {
                        self.expand_focused_text_to_full_chat(cx);
                    } else {
                        self.set_ui_variant(AgentChatUiVariant::FocusedTextMini, cx);
                        if let Some(callback) = self.on_focused_text_collapse_requested.clone() {
                            Self::spawn_host_app_callback(callback, cx);
                        }
                    }
                }
            }
            FocusedTextMiniAction::Stop => {
                success = self.cancel_streaming_from_escape(cx);
                if !success {
                    error_code = Some("not_streaming".to_string());
                }
            }
            FocusedTextMiniAction::Retry => {
                if self.has_retry_request() {
                    self.queue_setup_retry_request(cx);
                } else {
                    success = false;
                    error_code = Some("not_retryable".to_string());
                }
            }
            FocusedTextMiniAction::Replace
            | FocusedTextMiniAction::Append
            | FocusedTextMiniAction::Copy => {}
        }

        if self.focused_text.is_none() && error_code.is_none() {
            error_code = Some("no_focused_text".to_string());
        }

        let receipt = crate::protocol::AgentChatFocusedTextActionReceipt {
            action: action.trace_value().to_string(),
            success,
            changed_text: false,
            copied_to_clipboard: false,
            before_ui_variant,
            after_ui_variant: self.ui_variant.state_id().to_string(),
            output_length,
            error_code,
        };

        if let Some(state) = self.focused_text.as_mut() {
            state.last_action_receipt = Some(receipt.clone());
        }

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_mini_action_complete",
            action = action.trace_value(),
            success = receipt.success,
            changed_text = receipt.changed_text,
            copied_to_clipboard = receipt.copied_to_clipboard,
            before_ui_variant = %receipt.before_ui_variant,
            after_ui_variant = %receipt.after_ui_variant,
            output_length = receipt.output_length,
            error_code = ?receipt.error_code,
        );

        cx.notify();
        receipt
    }

    fn expand_focused_text_to_full_chat(&mut self, cx: &mut Context<Self>) {
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini {
            return;
        }
        self.sync_focused_text_thread_for_expand(cx);
        self.set_ui_variant(AgentChatUiVariant::Standard, cx);
        if let Some(callback) = self.on_focused_text_expand_requested.clone() {
            Self::spawn_host_app_callback(callback, cx);
        }
    }

    fn sync_focused_text_thread_for_expand(&mut self, cx: &mut Context<Self>) {
        let selected_index = self.focused_text_selected_variation.or_else(|| {
            self.focused_text_variations.iter().position(|variation| {
                variation.status == FocusedTextVariationStatus::Complete
                    && !variation.text.trim().is_empty()
            })
        });

        let mut assistant_bodies = Vec::new();
        for (index, variation) in self.focused_text_variations.iter().enumerate() {
            if variation.status != FocusedTextVariationStatus::Complete {
                continue;
            }
            let text = variation.text.trim();
            if text.is_empty() {
                continue;
            }
            let selected = selected_index == Some(index);
            let label = variation.angle.label();
            assistant_bodies.push(if selected {
                format!("**Selected · {label}**\n\n{text}")
            } else {
                format!("**{label}**\n\n{text}")
            });
        }

        if assistant_bodies.is_empty() {
            if let Some(text) = self
                .selected_focused_text_output(self.live_thread().read(cx))
                .filter(|text| !text.trim().is_empty())
            {
                assistant_bodies.push(text);
            } else {
                return;
            }
        }

        self.live_thread().update(cx, |thread, cx| {
            thread.replace_assistant_messages_after_last_user(assistant_bodies, cx);
        });
    }

    fn push_focused_text_instruction_history(&mut self, instruction: &str) {
        let instruction = instruction.trim();
        if instruction.is_empty() {
            return;
        }
        if self
            .focused_text_instruction_history
            .last()
            .is_some_and(|previous| previous == instruction)
        {
            return;
        }
        const MAX_FOCUSED_TEXT_INSTRUCTION_HISTORY: usize = 20;
        if self.focused_text_instruction_history.len() >= MAX_FOCUSED_TEXT_INSTRUCTION_HISTORY {
            self.focused_text_instruction_history.remove(0);
        }
        self.focused_text_instruction_history
            .push(instruction.to_string());
    }

    fn reset_focused_text_instruction_history_navigation(&mut self) {
        self.focused_text_instruction_history_index = None;
        self.focused_text_instruction_history_draft = None;
    }

    fn recall_focused_text_instruction_history(
        &mut self,
        delta: i32,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.focused_text_instruction_history.is_empty() {
            return false;
        }

        if delta > 0 && self.focused_text_instruction_history_index.is_none() {
            return false;
        }

        let len = self.focused_text_instruction_history.len();
        if self.focused_text_instruction_history_index.is_none() && delta < 0 {
            let draft = self.live_thread().read(cx).input.text().to_string();
            self.focused_text_instruction_history_draft = Some(draft);
            self.focused_text_instruction_history_index = Some(len);
        }

        let current = self.focused_text_instruction_history_index.unwrap_or(len);
        let target = current as i32 + delta;
        if target < 0 {
            return false;
        }

        if target >= len as i32 {
            if delta <= 0 {
                return false;
            }
            self.focused_text_instruction_history_index = None;
            let text = self
                .focused_text_instruction_history_draft
                .take()
                .unwrap_or_default();
            let cursor = text.chars().count();
            self.live_thread().update(cx, |thread, cx| {
                thread.input.set_text(text);
                thread.input.set_cursor(cursor);
                cx.notify();
            });
            cx.notify();
            return true;
        }

        self.focused_text_instruction_history_index = Some(target as usize);
        let text = self.focused_text_instruction_history[target as usize].clone();
        let cursor = text.chars().count();
        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(text);
            thread.input.set_cursor(cursor);
            cx.notify();
        });
        cx.notify();
        true
    }

    fn focused_text_enter_semantics_for_thread(
        &self,
        thread: &AgentChatThread,
    ) -> crate::ai::focused_text::FocusedTextEditSemantics {
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini {
            match self.focused_text_mini_phase_for_thread(thread) {
                Some(FocusedTextMiniPhase::InputOnly)
                | Some(FocusedTextMiniPhase::Loading)
                | Some(FocusedTextMiniPhase::Streaming) => {
                    crate::ai::focused_text::FocusedTextEditSemantics::Replace
                }
                Some(FocusedTextMiniPhase::Result) | Some(FocusedTextMiniPhase::Error) | None => {
                    crate::ai::focused_text::FocusedTextEditSemantics::Chat
                }
            }
        } else {
            crate::ai::focused_text::FocusedTextEditSemantics::Chat
        }
    }

    pub(crate) fn submit_focused_text_from_enter(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (phase, has_instruction, semantics) = {
            let thread = self.live_thread().read(cx);
            (
                self.focused_text_mini_phase_for_thread(thread),
                !thread.input.text().trim().is_empty(),
                self.focused_text_enter_semantics_for_thread(thread),
            )
        };

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini {
            match phase {
                Some(FocusedTextMiniPhase::Loading) => {
                    return Ok(());
                }
                Some(FocusedTextMiniPhase::Streaming) => {
                    let _ = self.cancel_streaming_from_escape(cx);
                    return Ok(());
                }
                Some(FocusedTextMiniPhase::Result | FocusedTextMiniPhase::Error)
                    if !has_instruction =>
                {
                    return Ok(());
                }
                Some(FocusedTextMiniPhase::InputOnly)
                | Some(FocusedTextMiniPhase::Result)
                | Some(FocusedTextMiniPhase::Error)
                | None => {}
            }
        }

        if !has_instruction {
            return Ok(());
        }

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && matches!(
                phase,
                Some(FocusedTextMiniPhase::Result | FocusedTextMiniPhase::Error)
            )
        {
            // AI-edit of the selected variation: a typed instruction in the
            // result phase refines the selected variation into a fresh set of
            // variations (⌘←/⌘→ walks back to the previous set) instead of
            // expanding into the full chat surface.
            if let Some(source_text) = self.selected_focused_text_variation_text() {
                if !self.focused_text_variations.is_empty() {
                    self.focused_text_variation_history
                        .push(self.focused_text_variations.clone());
                    self.focused_text_variation_history_index = None;
                }
                tracing::info!(
                    target: "script_kit::focused_text",
                    event = "focused_text_variation_refine_submitted",
                    source_index = ?self.focused_text_selected_variation,
                    source_text_len = source_text.chars().count(),
                    history_len = self.focused_text_variation_history.len(),
                );
                return self.submit_focused_text_turn(
                    crate::ai::focused_text::FocusedTextEditSemantics::Replace,
                    cx,
                    Some(source_text),
                );
            }
            self.expand_focused_text_to_full_chat(cx);
        }
        self.submit_focused_text_turn(semantics, cx, None)
    }

    /// The non-empty text of the currently selected variation, if any.
    fn selected_focused_text_variation_text(&self) -> Option<String> {
        self.focused_text_selected_variation
            .and_then(|index| self.focused_text_variations.get(index))
            .map(|variation| variation.text.clone())
            .filter(|text| !text.trim().is_empty())
    }

    fn footer_hint_label(button: &AgentChatFooterButtonSpec) -> &'static str {
        use crate::footer_popup::FooterAction;

        match button.action {
            FooterAction::Run if button.label == "Attach" => "↵ Attach",
            FooterAction::Run => "↵ Send",
            FooterAction::PasteResponse => "↵ Paste Response",
            FooterAction::Stop => "Esc Stop",
            FooterAction::Actions => "⌘K Actions",
            FooterAction::Ai => "⌘↵ Agent Chat",
            FooterAction::Apply => "⌘↩ Apply",
            FooterAction::Replace if button.key == "↵" => "↵ Paste",
            FooterAction::Replace if button.key == "⌘↵" => "⌘↵ Replace",
            FooterAction::Replace => "⌘R Replace",
            FooterAction::Append => "⌘A Append",
            FooterAction::Copy => "⌘C Copy",
            FooterAction::Expand if button.label == "Collapse" => "⌘⇧M Collapse",
            FooterAction::Expand => "⌘↵ Chat",
            FooterAction::Retry => "⌘⇧R Retry",
            FooterAction::Close => "⌘W Close",
            FooterAction::Cwd => "📁 CWD",
            FooterAction::AgentModel => "⇧⇥ Agent",
        }
    }

    pub(crate) fn dispatch_footer_button(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::footer_popup::FooterAction;

        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_footer_action_ignored_setup_mode",
                action = ?action,
            );
            return;
        }

        if self.focused_text.is_some() {
            if matches!(action, FooterAction::Run) {
                if let Err(error) = self.submit_focused_text_from_enter(cx) {
                    tracing::warn!(
                        target: "script_kit::focused_text",
                        event = "focused_text_submit_failed",
                        error = %error,
                    );
                }
                return;
            }
            if matches!(action, FooterAction::Actions) {
                self.trigger_toggle_actions(window, cx);
                return;
            }
            if let Some(action) = FocusedTextMiniAction::from_footer_action(action) {
                self.perform_focused_text_mini_action(action, cx);
                return;
            }
        }

        match action {
            FooterAction::Run => {
                if self.agent_chat_spine_owns_list()
                    && self.accept_agent_chat_spine_projection_row(window, cx)
                {
                    return;
                }
                if self.composer_picker_session.is_some() {
                    self.accept_composer_picker_selection_impl(false, cx);
                    return;
                }
                self.submit_with_expanded_tokens(cx);
            }
            FooterAction::PasteResponse => self.trigger_paste_response_requested(window, cx),
            FooterAction::Stop => {
                let _ = self.cancel_streaming_from_escape(cx);
            }
            FooterAction::Actions => self.trigger_toggle_actions(window, cx),
            FooterAction::Close => self.trigger_close_requested(window, cx),
            FooterAction::Ai => self.open_profile_trigger_picker_in_window(window, cx),
            FooterAction::Retry => self.retry_last_user_turn(cx),
            FooterAction::Apply => {}
            FooterAction::Replace
            | FooterAction::Append
            | FooterAction::Copy
            | FooterAction::Expand => {}
            FooterAction::Cwd => {
                self.cache_composer_parent_window(window, cx);
                window.focus(&self.focus_handle, cx);
                self.insert_picker_hint_prefix(">", cx);
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_footer_cwd_chip_opened_picker",
                );
            }
            FooterAction::AgentModel => {
                // Agent/model picker is owned by the main launcher window;
                // the Agent Chat chat does not currently host it inline.
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_footer_agent_model_chip_clicked_noop",
                );
            }
        }
    }

    pub(crate) fn footer_dot_status(&self, cx: &App) -> crate::footer_popup::FooterDotStatus {
        use crate::ai::agent_chat::ui::thread::AgentChatThreadStatus;
        use crate::footer_popup::FooterDotStatus;

        if self.is_setup_mode() {
            return FooterDotStatus::Hidden;
        }

        if self.context_capture_pending {
            return FooterDotStatus::Streaming;
        }

        match self.live_thread().read(cx).status {
            AgentChatThreadStatus::Streaming => FooterDotStatus::Streaming,
            AgentChatThreadStatus::WaitingForPermission => FooterDotStatus::WaitingForPermission,
            AgentChatThreadStatus::Error => FooterDotStatus::Error,
            AgentChatThreadStatus::Idle => FooterDotStatus::Idle,
        }
    }

    pub(crate) fn footer_status_text(&self, cx: &App) -> Option<&'static str> {
        use crate::ai::agent_chat::ui::thread::AgentChatThreadStatus;

        if self.is_setup_mode() {
            return None;
        }

        if self.context_capture_pending {
            return Some("Loading context...");
        }

        match self.live_thread().read(cx).status {
            AgentChatThreadStatus::Streaming => Some("Working..."),
            AgentChatThreadStatus::WaitingForPermission => Some("Waiting for permission..."),
            AgentChatThreadStatus::Error => Some("Error"),
            AgentChatThreadStatus::Idle => None,
        }
    }

    fn render_toolbar_from_snapshot(
        snapshot: AgentChatFooterSnapshot,
        weak_view: WeakEntity<AgentChatView>,
    ) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();

        // Hint strip opacity: match main menu's OPACITY_TEXT_MUTED (0.65)
        let hint_text_hex = theme.colors.text.primary;
        let hint_opacity_byte = (crate::theme::opacity::OPACITY_TEXT_MUTED * 255.0).round() as u32;
        let hint_text_rgba = (hint_text_hex << 8) | hint_opacity_byte;

        let hint_row = Self::render_agent_chat_footer_hint_row(
            &snapshot,
            weak_view.clone(),
            true,
            hint_text_rgba,
            &theme,
        );

        div()
            .w_full()
            .h(px(crate::window_resize::main_layout::HINT_STRIP_HEIGHT))
            .px(px(crate::window_resize::main_layout::HINT_STRIP_PADDING_X))
            .py(px(crate::window_resize::main_layout::HINT_STRIP_PADDING_Y))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_t(px(1.0))
            .border_color(rgba((theme.colors.text.primary << 8) | 0x10))
            .child(Self::render_profile_status_marker_from_snapshot(
                &snapshot,
                weak_view.clone(),
                hint_text_rgba,
            ))
            .child(hint_row)
            .into_any_element()
    }

    fn render_external_host_footer_from_snapshot(
        snapshot: AgentChatFooterSnapshot,
        weak_view: WeakEntity<AgentChatView>,
    ) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();
        let hint_text_hex = theme.colors.text.primary;
        let hint_opacity_byte = (crate::theme::opacity::OPACITY_TEXT_MUTED * 255.0).round() as u32;
        let hint_text_rgba = (hint_text_hex << 8) | hint_opacity_byte;
        let hint_row = Self::render_agent_chat_footer_hint_row(
            &snapshot,
            weak_view.clone(),
            false,
            hint_text_rgba,
            &theme,
        );

        div()
            .w_full()
            .h(px(crate::window_resize::main_layout::HINT_STRIP_HEIGHT))
            .px(px(crate::window_resize::main_layout::HINT_STRIP_PADDING_X))
            .py(px(crate::window_resize::main_layout::HINT_STRIP_PADDING_Y))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .border_t(px(1.0))
            .border_color(rgba((theme.colors.text.primary << 8) | 0x10))
            .child(Self::render_profile_status_marker_from_snapshot(
                &snapshot,
                weak_view.clone(),
                hint_text_rgba,
            ))
            .child(hint_row)
            .into_any_element()
    }

    fn footer_slot_width(action: crate::footer_popup::FooterAction, leading: bool) -> f32 {
        use crate::components::footer_chrome;
        use crate::footer_popup::FooterAction;

        if leading
            && matches!(
                action,
                FooterAction::Run | FooterAction::Stop | FooterAction::PasteResponse
            )
        {
            return AGENT_CHAT_FOOTER_LEADING_SLOT_WIDTH_PX;
        }

        match action {
            FooterAction::Run => footer_chrome::FOOTER_RUN_SLOT_MIN_WIDTH_PX,
            FooterAction::Actions => footer_chrome::FOOTER_ACTIONS_SLOT_WIDTH_PX,
            FooterAction::Ai | FooterAction::Cwd | FooterAction::AgentModel => {
                footer_chrome::FOOTER_AI_SLOT_WIDTH_PX
            }
            FooterAction::Apply
            | FooterAction::Replace
            | FooterAction::Append
            | FooterAction::Copy
            | FooterAction::Expand => footer_chrome::FOOTER_APPLY_SLOT_WIDTH_PX,
            FooterAction::Retry | FooterAction::Stop => footer_chrome::FOOTER_STOP_SLOT_WIDTH_PX,
            FooterAction::PasteResponse => footer_chrome::FOOTER_PASTE_RESPONSE_SLOT_WIDTH_PX,
            FooterAction::Close => footer_chrome::FOOTER_CLOSE_SLOT_WIDTH_PX,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_agent_chat_footer_hint_button(
        id: &'static str,
        key: &'static str,
        label: &'static str,
        slot_width_px: f32,
        selected: bool,
        enabled: bool,
        theme: &crate::theme::Theme,
        on_click: Option<crate::components::hint_strip::HintClickHandler>,
    ) -> gpui::AnyElement {
        let height = crate::components::footer_chrome::footer_button_height(
            crate::window_resize::main_layout::HINT_STRIP_HEIGHT,
        );
        let mut button = crate::components::footer_chrome::render_footer_hint_action_button_frame(
            crate::components::footer_chrome::FooterHintActionButtonFrameSpec {
                id,
                label: SharedString::from(label),
                key: SharedString::from(key),
                slot_width_px,
                height_px: height,
                selected,
                key_first: true,
                justify: crate::components::footer_chrome::FooterHintContentJustify::Center,
                layout: crate::components::footer_chrome::FooterHintButtonLayoutOverrides::default(
                ),
            },
            theme,
        )
        .when(!enabled, |d| d.opacity(0.38));

        if enabled {
            if let Some(handler) = on_click {
                button = button.on_click(move |event, window, cx| handler(event, window, cx));
            }
        }

        button.into_any_element()
    }

    fn render_agent_chat_footer_hint_row(
        snapshot: &AgentChatFooterSnapshot,
        weak_view: WeakEntity<AgentChatView>,
        include_history_and_close: bool,
        _hint_text_rgba: u32,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let mut row = div().flex().flex_row().items_center().gap(px(
            crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX,
        ));

        for (index, button) in snapshot.buttons.iter().enumerate() {
            let action = button.action;
            let button_view = weak_view.clone();
            let on_click = Rc::new(
                move |_event: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    if let Some(entity) = button_view.upgrade() {
                        entity.update(cx, |chat, cx| {
                            chat.dispatch_footer_button(action, window, cx);
                        });
                    }
                },
            );
            row = row.child(Self::render_agent_chat_footer_hint_button(
                if index == 0 {
                    "agent-chat-footer-leading-slot"
                } else {
                    "agent-chat-footer-action-slot"
                },
                button.key,
                button.label,
                Self::footer_slot_width(button.action, index == 0),
                button.selected,
                button.enabled,
                theme,
                Some(on_click),
            ));
        }

        if include_history_and_close {
            let history_view = weak_view.clone();
            let history_click = Rc::new(
                move |_event: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    if let Some(entity) = history_view.upgrade() {
                        entity.update(cx, |chat, cx| {
                            tracing::info!(
                                target: "script_kit::tab_ai",
                                event = "agent_chat_toolbar_history_clicked",
                            );
                            chat.trigger_open_history_command(window, cx);
                        });
                    }
                },
            );
            row = row.child(Self::render_agent_chat_footer_hint_button(
                "agent-chat-footer-history-slot",
                "⌘P",
                "History",
                crate::components::footer_chrome::FOOTER_ACTIONS_SLOT_WIDTH_PX,
                false,
                true,
                theme,
                Some(history_click),
            ));

            let close_view = weak_view;
            let close_click = Rc::new(
                move |_event: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    if let Some(entity) = close_view.upgrade() {
                        entity.update(cx, |chat, cx| {
                            chat.trigger_close_requested(window, cx);
                        });
                    }
                },
            );
            row = row.child(Self::render_agent_chat_footer_hint_button(
                "agent-chat-footer-close-slot",
                "⌘W",
                "Close",
                crate::components::footer_chrome::FOOTER_CLOSE_SLOT_WIDTH_PX,
                false,
                true,
                theme,
                Some(close_click),
            ));
        }

        row.into_any_element()
    }

    fn render_profile_status_marker_from_snapshot(
        snapshot: &AgentChatFooterSnapshot,
        weak_view: WeakEntity<AgentChatView>,
        hint_text_rgba: u32,
    ) -> gpui::AnyElement {
        div()
            .id("agent-chat-profile-display")
            .flex()
            .items_center()
            .gap(px(6.0))
            .min_w(px(0.0))
            .overflow_hidden()
            .cursor_pointer()
            .on_click({
                let profile_view = weak_view.clone();
                move |_event, window, cx| {
                    if let Some(entity) = profile_view.upgrade() {
                        entity.update(cx, |chat, cx| {
                            chat.open_profile_trigger_picker_in_window(window, cx);
                        });
                    }
                }
            })
            .child(
                div()
                    .id("agent_chat-profile-display")
                    .flex()
                    .items_center()
                    .min_w(px(0.0))
                    .text_xs()
                    .text_color(rgba(hint_text_rgba))
                    .overflow_hidden()
                    .child(snapshot.profile_display.clone()),
            )
            .child(
                div()
                    .id("agent_chat-model-display")
                    .flex()
                    .items_center()
                    .min_w(px(0.0))
                    .text_xs()
                    .text_color(rgba(hint_text_rgba))
                    .overflow_hidden()
                    .opacity(0.72)
                    .child(snapshot.model_display.clone()),
            )
            .when_some(snapshot.status_text, |d, status| {
                d.child(div().text_xs().text_color(rgba(hint_text_rgba)).child("·"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgba(hint_text_rgba))
                            .child(status),
                    )
            })
            .into_any_element()
    }

    pub(crate) fn build_external_host_footer(
        &self,
        weak_view: WeakEntity<AgentChatView>,
        cx: &App,
    ) -> Option<gpui::AnyElement> {
        if !self.uses_external_footer_host() || self.is_setup_mode() {
            return None;
        }

        Some(Self::render_external_host_footer_from_snapshot(
            self.footer_snapshot(cx),
            weak_view,
        ))
    }

    /// Restrict portal kinds this Agent Chat surface can open.
    ///
    /// Items for disallowed kinds are filtered from the composer picker and
    /// rejected at the portal-open dispatch. Call before wiring host callbacks.
    pub(crate) fn set_allowed_portal_kinds(
        &mut self,
        kinds: Vec<crate::ai::context_selector::types::ContextPortalKind>,
    ) {
        self.allowed_portal_kinds = kinds;
    }

    /// Whether the given portal kind is allowed by the host.
    fn is_portal_kind_allowed(
        &self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
    ) -> bool {
        self.allowed_portal_kinds.contains(&kind)
    }

    /// Register an inline mention token as owned so the mention sync system
    /// will remove the corresponding context part when the token is deleted.
    pub(crate) fn register_inline_owned_token(&mut self, token: String) {
        self.inline_owned_context_tokens.insert(token);
    }

    /// Register a typed mention alias so the parser can resolve `@type:name`
    /// tokens back to full `AiContextPart` values.
    pub(crate) fn register_typed_alias(
        &mut self,
        token: String,
        part: crate::ai::message_parts::AiContextPart,
    ) {
        self.typed_mention_aliases.insert(token, part);
    }

    pub(crate) fn register_inline_owned_context_part(
        &mut self,
        token: String,
        part: crate::ai::message_parts::AiContextPart,
    ) {
        if let crate::ai::message_parts::AiContextPart::TextBlock {
            label,
            source,
            text,
            ..
        } = &part
        {
            if source.starts_with("clipboard://pasted-text/")
                && !self
                    .pasted_text_tokens
                    .iter()
                    .any(|existing| existing.token == token)
            {
                self.pasted_text_tokens
                    .push(crate::pasted_text::PastedTextToken {
                        token: token.clone(),
                        label: label.clone(),
                        text: text.clone(),
                    });
            }
        }

        if let crate::ai::message_parts::AiContextPart::FilePath { path, label } = &part {
            if crate::pasted_image::label_looks_like_pasted_image(label)
                && !self
                    .pasted_image_tokens
                    .iter()
                    .any(|existing| existing.token == token)
            {
                self.pasted_image_tokens
                    .push(crate::pasted_image::PastedImageToken {
                        token: token.clone(),
                        label: label.clone(),
                        path: path.clone(),
                    });
            }
        }

        self.register_typed_alias(token.clone(), part);
        self.register_inline_owned_token(token);
    }

    /// Read-only access to the typed mention alias registry.
    pub(crate) fn typed_aliases(
        &self,
    ) -> &std::collections::HashMap<String, crate::ai::message_parts::AiContextPart> {
        &self.typed_mention_aliases
    }

    fn sync_pasted_clipboard_tokens(&mut self, cx: &App) {
        let text = self.live_thread().read(cx).input.text().to_string();
        crate::pasted_text::sync_pasted_text_tokens(&mut self.pasted_text_tokens, &text);
        crate::pasted_image::sync_pasted_image_tokens(&mut self.pasted_image_tokens, &text);
        self.typed_mention_aliases
            .retain(|token, _| text.contains(token));
    }

    fn pasted_text_pill_ranges(
        &self,
        input_text: &str,
    ) -> Vec<crate::components::text_input::TextInlinePillRange> {
        let theme = crate::theme::get_cached_theme();
        crate::pasted_text::token_ranges(input_text, &self.pasted_text_tokens)
            .iter()
            .map(|pill| crate::components::text_input::TextInlinePillRange {
                start: pill.range.start,
                end: pill.range.end,
                label: pill.label.clone(),
                text_color: theme.colors.text.primary,
                background_color: theme.colors.accent.selected_subtle,
                border_color: theme.colors.ui.border,
            })
            .collect()
    }

    fn pasted_image_pill_ranges(
        &self,
        input_text: &str,
    ) -> Vec<crate::components::text_input::TextInlinePillRange> {
        let theme = crate::theme::get_cached_theme();
        crate::pasted_image::token_ranges(input_text, &self.pasted_image_tokens)
            .iter()
            .map(|pill| crate::components::text_input::TextInlinePillRange {
                start: pill.range.start,
                end: pill.range.end,
                label: pill.label.clone(),
                text_color: theme.colors.text.primary,
                background_color: theme.colors.accent.selected_subtle,
                border_color: theme.colors.ui.border,
            })
            .collect()
    }

    fn paste_image_from_clipboard(&mut self, cx: &mut Context<Self>) -> bool {
        use crate::prompts::chat::MAX_IMAGE_BYTES;
        use base64::Engine as _;

        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return false;
        };
        let Ok(image_data) = clipboard.get_image() else {
            return false;
        };

        let Ok(encoded) = crate::clipboard_history::encode_image_as_png(&image_data) else {
            return false;
        };
        let base64_data = encoded.strip_prefix("png:").unwrap_or(&encoded);
        let Ok(png_bytes) = base64::engine::general_purpose::STANDARD.decode(base64_data) else {
            return false;
        };

        if png_bytes.len() > MAX_IMAGE_BYTES {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_pasted_image_rejected_too_large",
                size_bytes = png_bytes.len(),
                max_bytes = MAX_IMAGE_BYTES,
            );
            return false;
        }

        let Ok(path) = crate::pasted_image::write_png_bytes_to_temp_file(&png_bytes) else {
            return false;
        };
        let prepared = crate::pasted_image::prepare_pasted_image(&path, &self.pasted_image_tokens);
        let token = prepared.token.clone();
        let insertion_text = prepared.insertion_text;

        self.live_thread().update(cx, move |thread, cx| {
            thread.input.insert_str(&insertion_text);
            cx.notify();
        });

        let part = crate::ai::message_parts::AiContextPart::FilePath {
            path,
            label: token.label.clone(),
        };
        self.pasted_image_tokens.push(token.clone());
        self.typed_mention_aliases.insert(token.token, part);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_clipboard_image_pasted",
            label = %token.label,
            width = image_data.width,
            height = image_data.height,
            size_bytes = png_bytes.len(),
        );
        self.sync_inline_mentions(cx);

        true
    }

    pub(crate) fn paste_text_from_clipboard(&mut self, cx: &mut Context<Self>) -> bool {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            return false;
        };
        let Ok(text) = clipboard.get_text() else {
            return false;
        };
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        if normalized.is_empty() {
            return false;
        }

        let prepared =
            crate::pasted_text::prepare_pasted_text(&normalized, &self.pasted_text_tokens);
        let token = prepared.token.clone();
        let insertion_text = prepared.insertion_text;

        self.live_thread().update(cx, move |thread, cx| {
            thread.input.insert_str(&insertion_text);
            cx.notify();
        });

        if let Some(token) = token {
            let part = crate::ai::message_parts::AiContextPart::TextBlock {
                label: token.label.clone(),
                source: format!(
                    "clipboard://pasted-text/{}",
                    self.pasted_text_tokens.len() + 1
                ),
                text: normalized,
                mime_type: Some("text/plain".to_string()),
            };
            self.pasted_text_tokens.push(token.clone());
            self.typed_mention_aliases.insert(token.token, part);
        } else {
            self.sync_pasted_clipboard_tokens(cx);
        }

        self.sync_inline_mentions(cx);

        true
    }

    /// Expand typed display tokens in the input text back to full paths/URIs
    /// before sending to the AI. Replaces `@file:demo.rs` (and other alias keys)
    /// with `@file:"/full/path.rs"` via `typed_mention_aliases`.
    fn expand_typed_tokens_for_submit(&self, cx: &mut Context<Self>) {
        if self.typed_mention_aliases.is_empty() {
            return;
        }
        let text = self.live_thread().read(cx).input.text().to_string();
        if text.is_empty() {
            return;
        }

        let mentions = crate::ai::context_mentions::parse_inline_context_mentions_with_aliases(
            &text,
            &self.typed_mention_aliases,
        );
        if mentions.is_empty() {
            return;
        }

        // Build the expanded text by replacing typed tokens with full source paths.
        // Process mentions in reverse order to preserve character indices.
        let mut expanded = text.clone();
        for mention in mentions.iter().rev() {
            let full_ref = match &mention.part {
                crate::ai::message_parts::AiContextPart::FilePath { path, .. } => {
                    crate::ai::context_mentions::format_inline_file_token(path)
                }
                crate::ai::message_parts::AiContextPart::FocusedTarget {
                    target, label, ..
                } => {
                    // File/directory targets expand to full @file:path
                    if let Some(path) = target
                        .metadata
                        .as_ref()
                        .and_then(|m| m.get("path"))
                        .and_then(|v| v.as_str())
                    {
                        crate::ai::context_mentions::format_inline_file_token(path)
                    } else {
                        crate::ai::context_mentions::part_to_inline_token(&mention.part)
                            .unwrap_or_else(|| format!("@cmd:{label}"))
                    }
                }
                _ => continue,
            };
            let byte_start = expanded
                .char_indices()
                .nth(mention.range.start)
                .map(|(b, _)| b)
                .unwrap_or(0);
            let byte_end = expanded
                .char_indices()
                .nth(mention.range.end)
                .map(|(b, _)| b)
                .unwrap_or(expanded.len());
            expanded.replace_range(byte_start..byte_end, &full_ref);
        }

        if expanded != text {
            self.live_thread().update(cx, |thread, _cx| {
                thread.input.set_text(expanded);
            });
        }
    }

    /// Submit the current input, expanding typed display tokens to full paths first.
    pub(crate) fn submit_with_expanded_tokens(&mut self, cx: &mut Context<Self>) {
        self.expand_typed_tokens_for_submit(cx);
        let _ = self
            .live_thread()
            .update(cx, |thread, cx| thread.submit_input(cx));
    }

    /// Invoke a footer callback outside the AgentChatView borrow by spawning an
    /// immediate async task. The host callbacks (toggle_actions, close, etc.)
    /// may need to entity.read() this view, which panics if we're inside update.
    fn spawn_footer_callback(
        callback: AgentChatFooterActionHandler,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let window_handle = window.window_handle();
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
            let _ = window_handle.update(cx, |_root, window, cx| {
                callback(window, cx);
            });
        })
        .detach();
    }

    fn spawn_host_app_callback(callback: AgentChatHostAppHandler, cx: &mut Context<Self>) {
        cx.spawn(async move |_this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
            cx.update(|cx| {
                callback(cx);
            });
        })
        .detach();
    }

    fn trigger_toggle_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_toggle_actions.clone() {
            // toggle_actions needs entity.read(cx) on AgentChatView, which panics
            // if called from within AgentChatView's own update. Spawn an immediate
            // async task to fully release the entity borrow first.
            Self::spawn_footer_callback(callback, window, cx);
        } else {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_footer_toggle_actions_no_callback",
                "Agent Chat footer actions click dropped because no host callback was installed"
            );
        }
    }

    fn trigger_toggle_actions_from_parent(
        &mut self,
        parent: AgentChatComposerParentWindow,
        cx: &mut Context<Self>,
    ) {
        if let Some(callback) = self.on_toggle_actions.clone() {
            cx.spawn(async move |_this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1))
                    .await;
                let _ = parent.handle.update(cx, |_root, window, cx| {
                    callback(window, cx);
                });
            })
            .detach();
        } else {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_toolbar_model_actions_no_callback",
                "Agent Chat model toolbar click dropped because no host actions callback was installed"
            );
        }
    }

    fn reset_agent_chat_zoom(&mut self, cx: &mut Context<Self>) {
        let mut theme = crate::theme::get_cached_theme();
        let defaults = crate::theme::FontConfig::default();
        let mut fonts = theme.fonts.clone().unwrap_or_default();
        fonts.ui_size = defaults.ui_size;
        fonts.mono_size = defaults.mono_size;
        theme.fonts = Some(fonts);

        match crate::theme::service::persist_theme_and_sync_all_windows(
            cx,
            &theme,
            "agent_chat_cmd_0_reset_agent_chat_zoom",
        ) {
            Ok(_) => {
                tracing::info!(
                    target: "script_kit::keyboard",
                    event = "agent_chat_cmd_0_reset_agent_chat_zoom",
                );
                cx.notify();
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::keyboard",
                    event = "agent_chat_cmd_0_reset_agent_chat_zoom_failed",
                    error = %error,
                );
            }
        }
    }

    fn trigger_close_requested(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_close_requested.clone() {
            Self::spawn_footer_callback(callback, window, cx);
        } else {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_footer_close_no_callback",
                "Agent Chat footer close click dropped because no host callback was installed"
            );
        }
    }

    fn trigger_close_window_requested(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_close_window_requested.clone() {
            Self::spawn_footer_callback(callback, window, cx);
        } else {
            self.trigger_close_requested(window, cx);
        }
    }

    pub(crate) fn set_on_open_history_command(
        &mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.on_open_history_command = Some(std::sync::Arc::new(callback));
    }

    pub(crate) fn set_on_paste_response_requested(
        &mut self,
        callback: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.on_paste_response_requested = Some(std::sync::Arc::new(callback));
    }

    /// Prepare the embedded Agent Chat view to be hidden behind another main-panel
    /// surface while keeping its live thread/session intact for reuse.
    pub(crate) fn prepare_for_host_hide(&mut self, cx: &mut Context<Self>) {
        self.attach_menu_open = false;
        self.permission_options_open = false;
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        self.history_menu = None;
        self.opened_via_transient_trigger = None;
        if let Some(card) = &self.setup_card {
            card.update(cx, |view, cx| view.set_agent_picker(None, cx));
        }
        // Clear a bare `@` / `/` / `|` trigger left over from a launcher-initiated
        // transient entry. Without this, the thread-change observer
        // registered at `Self::new` can re-fire on a later notify (agent
        // preflight, model discovery, etc.), see the lingering trigger
        // character still in the composer, and pop the slash/profile
        // picker back open on top of the now-visible main menu.
        if let AgentChatSession::Live(thread) = &self.session {
            let text = thread.read(cx).input.text().to_string();
            if text == "@" || text == "/" || text == PROFILE_TRIGGER_STR {
                thread.update(cx, |thread, cx| {
                    thread.input.set_text(String::new());
                    thread.input.set_cursor(0);
                    cx.notify();
                });
            }
        }
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    fn check_for_transient_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.opened_via_transient_trigger.is_some() {
            let is_empty = if let AgentChatSession::Live(thread) = &self.session {
                let thread_ref = thread.read(cx);
                thread_ref.messages.is_empty() && thread_ref.input.text().is_empty()
            } else {
                false
            };
            if is_empty {
                self.opened_via_transient_trigger = None;
                self.trigger_close_requested(window, cx);
                return true;
            }
        }
        false
    }

    pub(crate) fn prepare_for_attachment_portal_open(&mut self, cx: &mut Context<Self>) {
        self.attach_menu_open = false;
        self.permission_options_open = false;
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::PortalStaged, cx);
        self.history_menu = None;
        if let Some(card) = &self.setup_card {
            card.update(cx, |view, cx| view.set_agent_picker(None, cx));
        }

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_attachment_portal_prepare",
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn resume_after_attachment_portal_close(&mut self, cx: &mut Context<Self>) {
        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_attachment_portal_resume",
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    fn trigger_open_history_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_open_history_command.clone() {
            Self::spawn_footer_callback(callback, window, cx);
        } else {
            tracing::info!(
                target: "script_kit::agent_chat",
                event = "agent_chat_history_command_no_callback",
                "Cmd+P history command request dropped — no host callback installed"
            );
        }
    }

    fn export_thread_to_downloads(&mut self, cx: &mut Context<Self>) {
        let (markdown, session_id) = {
            let thread = self.live_thread();
            let thread_ref = thread.read(cx);
            (
                super::export::build_agent_chat_conversation_markdown_from_thread(thread_ref),
                thread_ref.ui_thread_id().to_string(),
            )
        };

        let result = markdown
            .ok_or_else(|| "Nothing to export yet".to_string())
            .and_then(|markdown| {
                let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
                std::fs::create_dir_all(&dir)
                    .map_err(|error| format!("Create export directory failed: {error}"))?;
                let safe_session_id = session_id
                    .chars()
                    .map(|ch| {
                        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                            ch
                        } else {
                            '-'
                        }
                    })
                    .collect::<String>();
                let path = dir.join(format!("agent-chat-export-{safe_session_id}.md"));
                std::fs::write(&path, markdown)
                    .map_err(|error| format!("Write export failed: {error}"))?;
                if let Err(error) = crate::platform::reveal_in_finder(&path) {
                    tracing::warn!(
                        target: "script_kit::agent_chat",
                        event = "agent_chat_export_reveal_failed",
                        path = %path.display(),
                        error = %error,
                    );
                }
                Ok(path)
            });

        self.live_thread().update(cx, |thread, cx| match result {
            Ok(path) => thread.push_system_message(
                format!("Exported Agent Chat thread to {}", path.display()),
                cx,
            ),
            Err(error) => {
                thread.push_system_message(format!("Agent Chat export failed: {error}"), cx)
            }
        });
    }

    pub(crate) fn has_focused_text_context(&self) -> bool {
        self.focused_text.is_some()
    }

    pub(crate) fn focused_text_actions_expanded(&self) -> bool {
        self.focused_text.is_some() && self.ui_variant != AgentChatUiVariant::FocusedTextMini
    }

    fn trigger_paste_response_requested(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(callback) = self.on_paste_response_requested.clone() {
            Self::spawn_footer_callback(callback, window, cx);
        } else {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_footer_paste_response_no_callback",
                "Agent Chat footer Paste Response request dropped because no host callback was installed"
            );
        }
    }

    pub(crate) fn toggle_expanded_composer(&mut self, cx: &mut Context<Self>) {
        self.expanded_composer = !self.expanded_composer;
        cx.notify();
    }

    pub(super) fn refresh_composer_picker_state_after_parent_change(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
    }

    /// Convert recent history entries into neutral hits (score 0, Title field).
    fn recent_history_hits() -> Vec<super::history::AgentChatHistorySearchHit> {
        super::history::load_history()
            .into_iter()
            .map(|entry| super::history::AgentChatHistorySearchHit {
                entry,
                score: 0,
                matched_field: super::history::AgentChatHistorySearchField::Title,
            })
            .collect()
    }

    fn history_popup_snapshot(
        &self,
    ) -> Option<crate::ai::agent_chat::ui::history_popup::AgentChatHistoryPopupSnapshot> {
        let menu = self.history_menu.as_ref()?;
        let entries = menu
            .hits
            .iter()
            .cloned()
            .map(crate::ai::agent_chat::ui::history_popup::AgentChatHistoryPopupEntry::from_hit)
            .collect::<Vec<_>>();
        let selected_index = if entries.is_empty() {
            0
        } else {
            menu.selected_index.min(entries.len().saturating_sub(1))
        };

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_popup_snapshot_built",
            query = %menu.query,
            hit_count = menu.hits.len(),
            visible_count = entries.len(),
            selected_index,
        );

        Some(
            crate::ai::agent_chat::ui::history_popup::AgentChatHistoryPopupSnapshot {
                title: if menu.query.trim().is_empty() {
                    SharedString::from("Recent Conversations (⌘P)")
                } else {
                    SharedString::from(format!("History matches \u{201c}{}\u{201d}", menu.query))
                },
                query: SharedString::from(menu.query.clone()),
                selected_index,
                entries,
            },
        )
    }

    pub(super) fn sync_history_popup_window_from_cached_parent(&mut self, cx: &mut Context<Self>) {
        let Some(parent) = self.composer_parent_window else {
            crate::ai::agent_chat::ui::history_popup::close_history_popup_window(cx);
            return;
        };

        let source_view = cx.entity().downgrade();
        if let Some(snapshot) = self.history_popup_snapshot() {
            if let Err(error) = crate::ai::agent_chat::ui::history_popup::sync_history_popup_window(
                cx,
                crate::ai::agent_chat::ui::history_popup::AgentChatHistoryPopupRequest {
                    parent_window_handle: parent.handle,
                    parent_bounds: parent.bounds,
                    display_id: parent.display_id,
                    source_view,
                    snapshot,
                },
            ) {
                tracing::error!(error = %error, "agent_chat_history_popup_sync_failed");
            }
        } else {
            crate::ai::agent_chat::ui::history_popup::close_history_popup_window(cx);
        }
    }

    pub(crate) fn select_profile_from_popup(&mut self, profile_id: &str, cx: &mut Context<Self>) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_profile_selector_selected",
            profile_id,
            "Selected Agent Chat profile from Menu Search"
        );
        if let Some(callback) = self.on_profile_selected.clone() {
            let selected_profile_id = profile_id.to_string();
            cx.defer(move |cx| {
                callback(selected_profile_id.clone(), cx);
            });
        }
        cx.notify();
    }

    pub(crate) fn select_history_from_popup(
        &mut self,
        entry: &super::history::AgentChatHistoryEntry,
        cx: &mut Context<Self>,
    ) {
        self.history_menu = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        let had_pending_history_portal = self.has_pending_history_portal_session();
        if had_pending_history_portal {
            if let Err(error) = self.attach_history_session(
                &entry.session_id,
                super::history_attachment::AgentChatHistoryAttachMode::Summary,
                cx,
            ) {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_history_popup_attach_failed",
                    session_id = %entry.session_id,
                    mode = "summary",
                    error = %error,
                );
                let _ = self.cancel_pending_portal_session(
                    crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory,
                    cx,
                );
                return;
            } else {
                return;
            }
        }
        if let Some(conv) = super::history::load_conversation(&entry.session_id) {
            self.live_thread().update(cx, |thread, cx| {
                thread.load_saved_messages(&conv.messages, cx);
            });
            if let Some(transcript) = &self.transcript {
                transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
            }
        } else {
            self.live_thread().update(cx, |thread, cx| {
                thread.input.set_text(entry.first_message.clone());
                cx.notify();
            });
        }
        cx.notify();
    }

    pub(crate) fn select_history_session_by_id(
        &mut self,
        session_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(entry) = super::history::load_history()
            .into_iter()
            .find(|entry| entry.session_id == session_id)
        else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_actions_select_missing",
                session_id = %session_id,
            );
            return false;
        };

        self.select_history_from_popup(&entry, cx);
        true
    }

    fn set_history_popup_query(&mut self, query: String, cx: &mut Context<Self>) {
        let hits = super::history::search_history(&query, HISTORY_POPUP_SEARCH_LIMIT);
        self.history_closed_at = None;
        self.history_menu = Some(AgentChatHistoryMenuState {
            selected_index: 0,
            query,
            hits,
        });
        self.sync_history_popup_window_from_cached_parent(cx);
        cx.notify();
    }

    fn navigate_history_popup_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };
        if menu.hits.is_empty() {
            return;
        }

        let len = menu.hits.len();
        let current = menu.selected_index;
        menu.selected_index = if delta < 0 {
            current.saturating_sub((-delta) as usize)
        } else {
            (current + delta as usize).min(len.saturating_sub(1))
        };
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        cx.notify();
    }

    fn jump_history_popup_selection(&mut self, end: bool, cx: &mut Context<Self>) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };
        if menu.hits.is_empty() {
            return;
        }

        menu.selected_index = if end {
            menu.hits.len().saturating_sub(1)
        } else {
            0
        };
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        cx.notify();
    }

    fn page_history_popup_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };
        if menu.hits.is_empty() {
            return;
        }

        let len = menu.hits.len();
        menu.selected_index = if delta < 0 {
            menu.selected_index.saturating_sub(HISTORY_POPUP_PAGE_JUMP)
        } else {
            (menu.selected_index + HISTORY_POPUP_PAGE_JUMP).min(len.saturating_sub(1))
        };
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);
        cx.notify();
    }

    fn execute_history_popup_selection(
        &mut self,
        modifiers: &gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .history_menu
            .as_ref()
            .and_then(|menu| menu.hits.get(menu.selected_index))
            .map(|hit| hit.entry.clone())
        else {
            return;
        };

        self.history_menu = None;
        self.history_closed_at = None;
        self.sync_history_popup_window_from_cached_parent(cx);

        if modifiers.platform {
            self.select_history_from_popup(&entry, cx);
            return;
        }

        let mode = if modifiers.shift {
            super::history_attachment::AgentChatHistoryAttachMode::Transcript
        } else {
            super::history_attachment::AgentChatHistoryAttachMode::Summary
        };

        if let Err(error) = self.attach_history_session(&entry.session_id, mode, cx) {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_popup_attach_failed",
                session_id = %entry.session_id,
                mode = ?mode,
                error = %error,
            );
        }
        cx.notify();
    }

    fn has_pending_history_portal_session(&self) -> bool {
        matches!(
            self.pending_portal_session.as_ref(),
            Some(session)
                if session.contract.portal_kind
                    == crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory
        )
    }

    fn build_history_attachment_part(
        &self,
        session_id: &str,
        mode: super::history_attachment::AgentChatHistoryAttachMode,
    ) -> anyhow::Result<AiContextPart> {
        let (path, label) = super::history_attachment::write_history_attachment(session_id, mode)?;
        Ok(AiContextPart::FilePath {
            path: path.to_string_lossy().to_string(),
            label,
        })
    }

    /// Attach a prior conversation as a context chip via the existing file attachment path.
    pub(crate) fn attach_history_session(
        &mut self,
        session_id: &str,
        mode: super::history_attachment::AgentChatHistoryAttachMode,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let part = self.build_history_attachment_part(session_id, mode)?;
        let (display_path, label) = match &part {
            AiContextPart::FilePath { path, label } => (path.clone(), label.clone()),
            _ => unreachable!("history attachments must be file-backed"),
        };

        if self.has_pending_history_portal_session() {
            tracing::info!(
                target: "script_kit::agent_chat",
                event = "agent_chat_history_portal_selection_attached_via_contract",
                session_id = %session_id,
                mode = ?mode,
            );
            self.attach_portal_part(part, cx);
            return Ok(());
        }

        self.live_thread().update(cx, |thread, cx| {
            thread.add_context_part(part.clone(), cx);
        });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_attachment_added",
            session_id = %session_id,
            mode = ?mode,
            path = %display_path,
            label = %label,
        );

        cx.notify();
        Ok(())
    }

    /// Read the staged portal query for `kind`.
    pub(crate) fn portal_query_for(
        &self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
    ) -> Option<String> {
        self.pending_portal_session
            .as_ref()
            .filter(|session| session.contract.portal_kind == kind)
            .map(|session| {
                crate::ai::agent_chat::ui::portal_contract::picker_portal_query(
                    kind,
                    &session.contract.query,
                )
            })
    }

    /// Backward-compatible helper for the Agent Chat history host flow.
    pub(crate) fn take_pending_history_portal_query(&mut self) -> Option<String> {
        self.portal_query_for(
            crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory,
        )
    }

    /// Open the history popup pre-seeded with search hits from the portal.
    pub(crate) fn open_history_portal_with_entries(
        &mut self,
        query: String,
        hits: Vec<super::history::AgentChatHistorySearchHit>,
        cx: &mut Context<Self>,
    ) -> bool {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_portal_opened",
            query = %query,
            hit_count = hits.len(),
        );
        self.attach_menu_open = false;
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        self.history_closed_at = None;
        self.history_menu = Some(AgentChatHistoryMenuState {
            selected_index: 0,
            query,
            hits,
        });
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
        true
    }

    pub(crate) fn sync_history_popup_state_from_window(
        &mut self,
        query: String,
        hits: Vec<super::history::AgentChatHistorySearchHit>,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.history_menu.is_none() {
            return;
        }

        let clamped_selected_index = if hits.is_empty() {
            0
        } else {
            selected_index.min(hits.len().saturating_sub(1))
        };

        self.history_closed_at = None;
        self.history_menu = Some(AgentChatHistoryMenuState {
            selected_index: clamped_selected_index,
            query,
            hits,
        });
        cx.notify();
    }

    pub(crate) fn sync_history_popup_selection_from_window(
        &mut self,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.history_menu.as_mut() else {
            return;
        };

        menu.selected_index = if menu.hits.is_empty() {
            0
        } else {
            selected_index.min(menu.hits.len().saturating_sub(1))
        };
        self.history_closed_at = None;
        cx.notify();
    }

    pub(crate) fn open_history_popup_from_host(
        &mut self,
        parent_handle: gpui::AnyWindowHandle,
        parent_bounds: gpui::Bounds<gpui::Pixels>,
        display_id: Option<gpui::DisplayId>,
        cx: &mut Context<Self>,
    ) {
        let display_bounds = display_id.and_then(|id| {
            cx.displays()
                .into_iter()
                .find(|d| d.id() == id)
                .map(|d| d.visible_bounds())
        });
        self.composer_parent_window = Some(AgentChatComposerParentWindow {
            handle: parent_handle,
            bounds: parent_bounds,
            display_id,
            display_bounds,
        });

        if self.history_menu.is_none() {
            let hits = Self::recent_history_hits();
            if hits.is_empty() {
                self.sync_history_popup_window_from_cached_parent(cx);
                cx.notify();
                return;
            }

            self.attach_menu_open = false;
            self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
            self.history_closed_at = None;
            self.history_menu = Some(AgentChatHistoryMenuState {
                selected_index: 0,
                query: String::new(),
                hits,
            });
        }

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    fn toggle_history_popup_from_cached_parent(&mut self, cx: &mut Context<Self>) {
        if self.history_menu.is_some() {
            self.dismiss_history_popup(cx);
            return;
        }

        if self.was_history_recently_closed() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_popup_toggle_suppressed_recent_close",
                "Suppressed Agent Chat history popup reopen because it was just closed"
            );
            return;
        } else {
            let hits = Self::recent_history_hits();
            if !hits.is_empty() {
                self.attach_menu_open = false;
                self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
                self.history_closed_at = None;
                self.history_menu = Some(AgentChatHistoryMenuState {
                    selected_index: 0,
                    query: String::new(),
                    hits,
                });
            }
        }
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn toggle_history_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cache_composer_parent_window(window, cx);
        self.toggle_history_popup_from_cached_parent(cx);
    }

    pub(crate) fn dismiss_escape_popup(&mut self, cx: &mut Context<Self>) -> bool {
        if self.exit_focused_text_variation_editor(cx) {
            return true;
        }

        if self.dismiss_composer_picker(cx) {
            return true;
        }

        if self.history_menu.is_some() {
            self.dismiss_history_popup(cx);
            return true;
        }

        if self.attach_menu_open {
            self.attach_menu_open = false;
            cx.notify();
            return true;
        }

        false
    }

    /// Cancel an active Agent Chat turn from an Escape shortcut.
    ///
    /// Returns true when Escape was consumed by cancellation. Host-level
    /// interceptors call this before falling back to "return to main menu",
    /// because focused child routing is not guaranteed for every Escape path.
    pub(crate) fn cancel_streaming_from_escape(&mut self, cx: &mut Context<Self>) -> bool {
        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::keyboard",
                event = "agent_chat_escape_cancel_ignored_setup_mode",
            );
            return false;
        }

        let is_streaming = matches!(
            self.live_thread().read(cx).status,
            AgentChatThreadStatus::Streaming
        );
        if !is_streaming {
            return false;
        }

        tracing::info!(
            target: "script_kit::keyboard",
            event = "agent_chat_escape_cancel_streaming_requested",
            variation_generation = self.focused_text_variation_generation,
        );
        self.focused_text_variation_generation += 1;
        self.cancel_isolated_variation_processes();
        self.focused_text_variation_tasks.clear();
        for variation in &mut self.focused_text_variations {
            if variation.status == FocusedTextVariationStatus::Streaming {
                variation.status = FocusedTextVariationStatus::Error;
                variation.error = Some("cancelled".to_string());
            }
        }
        self.live_thread()
            .update(cx, |thread, cx| thread.cancel_streaming(cx));
        true
    }

    pub(crate) fn has_escape_dismissible_popup(&self) -> bool {
        self.focused_text_editing_variation.is_some()
            || self.composer_picker_session.is_some()
            || self.history_menu.is_some()
            || self.attach_menu_open
    }

    fn composer_picker_state(&self) -> AgentChatComposerPickerState {
        if let Some(session) = self.composer_picker_session.clone() {
            AgentChatComposerPickerState::Open(session)
        } else if let Some(trigger) = self.dismissed_mention_trigger.clone() {
            AgentChatComposerPickerState::Dismissed(trigger)
        } else {
            AgentChatComposerPickerState::Closed
        }
    }

    fn apply_composer_picker_transition(
        &mut self,
        transition: AgentChatComposerPickerTransition,
        cx: &mut Context<Self>,
    ) -> Option<AgentChatComposerPickerSession> {
        let AgentChatComposerPickerTransition {
            state,
            sync_popup,
            notify,
            close_competing_popups,
            clear_last_accepted_item,
            log_visible_reason,
            accepted_session,
            insert_slash_input,
            clear_slash_input,
        } = transition;

        match state {
            AgentChatComposerPickerState::Closed => {
                self.composer_picker_session.take();
                self.dismissed_mention_trigger = None;
            }
            AgentChatComposerPickerState::Open(session) => {
                self.composer_picker_session = Some(session);
                self.dismissed_mention_trigger = None;
            }
            AgentChatComposerPickerState::Dismissed(trigger) => {
                self.composer_picker_session.take();
                self.dismissed_mention_trigger = Some(trigger);
            }
        }

        if clear_last_accepted_item {
            self.last_accepted_item = None;
        }
        if close_competing_popups {
            self.attach_menu_open = false;
            self.history_menu = None;
        }
        if !self.is_setup_mode() {
            if clear_slash_input {
                self.live_thread().update(cx, |thread, cx| {
                    let text = thread.input.text().to_string();
                    if text.starts_with('/') {
                        thread.input.set_text(String::new());
                        thread.input.set_cursor(0);
                    }
                    cx.notify();
                });
            }
            if insert_slash_input {
                self.live_thread().update(cx, |thread, cx| {
                    thread.input.set_text("/".to_string());
                    thread.input.set_cursor(1);
                    cx.notify();
                });
            }
        }
        if let Some(reason) = log_visible_reason {
            self.log_composer_picker_visible_range(reason);
        }
        if sync_popup {
            self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        }
        if notify {
            cx.notify();
        }

        accepted_session
    }

    fn clear_composer_picker(
        &mut self,
        reason: AgentChatComposerPickerDismissReason,
        cx: &mut Context<Self>,
    ) {
        let transition = reduce_agent_chat_composer_picker(
            self.composer_picker_state(),
            AgentChatComposerPickerEvent::Dismiss { reason, cursor: 0 },
        );
        self.apply_composer_picker_transition(transition, cx);
    }

    pub(crate) fn dismiss_composer_picker(&mut self, cx: &mut Context<Self>) -> bool {
        if self.composer_picker_session.is_none() {
            return false;
        };
        let cursor = self.live_thread().read(cx).input.cursor();
        let transition = reduce_agent_chat_composer_picker(
            self.composer_picker_state(),
            AgentChatComposerPickerEvent::Dismiss {
                reason: AgentChatComposerPickerDismissReason::Outside,
                cursor,
            },
        );
        let trigger = match &transition.state {
            AgentChatComposerPickerState::Dismissed(trigger) => Some(trigger.clone()),
            _ => None,
        };
        self.apply_composer_picker_transition(transition, cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_composer_picker_dismissed",
            trigger = ?trigger.as_ref().map(|trigger| trigger.trigger),
            query = %trigger.as_ref().map(|trigger| trigger.query.as_str()).unwrap_or(""),
        );
        true
    }

    /// Access the live thread entity, if in live mode.
    pub(crate) fn thread(&self) -> Option<Entity<AgentChatThread>> {
        match &self.session {
            AgentChatSession::Live(t) => Some(t.clone()),
            AgentChatSession::Setup(_) => None,
        }
    }

    /// Whether this view is in setup mode (no live thread).
    pub(crate) fn is_setup_mode(&self) -> bool {
        matches!(self.session, AgentChatSession::Setup(_))
    }

    /// Returns the validated script path if a `SCRIPT_READY` receipt exists.
    pub(crate) fn ready_script_path(&self) -> Option<std::path::PathBuf> {
        self.ready_script_path.clone()
    }

    /// Whether a deferred context capture is in-flight (drives footer loading dot).
    pub(crate) fn is_context_capture_pending(&self) -> bool {
        self.context_capture_pending
    }

    /// Set the context capture pending state (drives footer loading dot).
    pub(crate) fn set_context_capture_pending(&mut self, pending: bool) {
        self.context_capture_pending = pending;
    }

    /// Prime the slash command picker to show `/{slash_name}` on first open.
    ///
    /// Sets the input text to `/{slash_name}` and triggers a composer session
    /// refresh so the picker row for that skill is pre-selected.
    pub(crate) fn prime_slash_entry(&mut self, slash_name: &str, cx: &mut Context<Self>) {
        let prefill = format!("/{slash_name}");
        self.pending_slash_prime = Some(slash_name.to_string());
        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(prefill.clone());
            thread.input.set_cursor(prefill.chars().count());
            cx.notify();
        });
        self.refresh_agent_chat_spine_from_composer(cx);
        self.refresh_composer_picker_session(cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_slash_entry_primed",
            slash_name,
        );
    }

    /// Internal accessor returning a reference to the live thread entity.
    ///
    /// Only called from code paths guarded by `render()` and `handle_key_down()`
    /// early-returns in setup mode.
    #[inline]
    pub(crate) fn live_thread(&self) -> &Entity<AgentChatThread> {
        match &self.session {
            AgentChatSession::Live(t) => t,
            AgentChatSession::Setup(_) => unreachable!("live_thread called in setup mode"),
        }
    }

    /// Summaries of retained background threads for the Cmd+K "Threads"
    /// section, ordered oldest-retained first.
    pub(crate) fn retained_thread_summaries(&self, cx: &gpui::App) -> Vec<AgentChatThreadSummary> {
        self.retained_threads
            .iter()
            .map(|thread| {
                let t = thread.read(cx);
                let title = t
                    .messages
                    .iter()
                    .find(|m| matches!(m.role, AgentChatThreadMessageRole::User))
                    .map(|m| Self::thread_summary_title(m.body.as_ref()))
                    .unwrap_or_else(|| "New Thread".to_string());
                let seen = self
                    .thread_last_seen
                    .get(t.ui_thread_id())
                    .copied()
                    .unwrap_or(0);
                AgentChatThreadSummary {
                    ui_thread_id: t.ui_thread_id().to_string(),
                    title,
                    unread: t.messages.len().saturating_sub(seen),
                    is_streaming: matches!(
                        t.status,
                        super::thread::AgentChatThreadStatus::Streaming
                    ),
                }
            })
            .collect()
    }

    /// First line of the first user message, truncated for switcher rows.
    fn thread_summary_title(body: &str) -> String {
        const MAX_CHARS: usize = 48;
        let line = body.lines().next().unwrap_or("").trim();
        if line.is_empty() {
            return "New Thread".to_string();
        }
        if line.chars().count() <= MAX_CHARS {
            return line.to_string();
        }
        let truncated: String = line.chars().take(MAX_CHARS).collect();
        format!("{}…", truncated.trim_end())
    }

    /// Activate `thread` as the session thread, retaining the previous live
    /// thread so it keeps streaming on its own connection in the background.
    pub(crate) fn activate_session_thread(
        &mut self,
        thread: Entity<AgentChatThread>,
        cx: &mut Context<Self>,
    ) {
        if let AgentChatSession::Live(current) = &self.session {
            if current.entity_id() == thread.entity_id() {
                return;
            }
            let current = current.clone();
            let (current_id, current_len) = {
                let t = current.read(cx);
                (t.ui_thread_id().to_string(), t.messages.len())
            };
            self.thread_last_seen.insert(current_id, current_len);
            if !self
                .retained_threads
                .iter()
                .any(|t| t.entity_id() == current.entity_id())
            {
                self.retained_threads.push(current);
            }
        }
        self.retained_threads
            .retain(|t| t.entity_id() != thread.entity_id());
        {
            let t = thread.read(cx);
            self.thread_last_seen
                .insert(t.ui_thread_id().to_string(), t.messages.len());
        }
        self.thread_observers
            .entry(thread.entity_id())
            .or_insert_with(|| Self::observe_session_thread(&thread, cx));
        self.session = AgentChatSession::Live(thread.clone());
        if let Some(transcript) = &self.transcript {
            transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
        }
        // One observer pass resyncs transcript/toolbar/composer from the
        // newly active thread.
        thread.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    /// Start a fresh thread on a new Pi connection. The current thread keeps
    /// streaming in the background and appears in the Cmd+K Threads section.
    pub(crate) fn start_new_thread(&mut self, cx: &mut Context<Self>) {
        let AgentChatSession::Live(current) = &self.session else {
            return;
        };
        let requirements = current.read(cx).current_setup_requirements();
        match super::hosted::spawn_hosted_thread(None, requirements, cx) {
            Ok(thread) => {
                thread.update(cx, |thread, cx| thread.refresh_models(cx));
                self.activate_session_thread(thread, cx);
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_new_thread_started",
                    retained_count = self.retained_threads.len(),
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_new_thread_failed",
                    error = %error,
                );
            }
        }
    }

    fn respawn_live_thread_for_cwd(
        &mut self,
        selected_cwd: PathBuf,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(thread_entity) = self.thread() else {
            return false;
        };
        let (current_cwd, status, current_profile_id) = {
            let thread = thread_entity.read(cx);
            (
                thread.cwd().clone(),
                thread.status(),
                thread.profile_id().to_string(),
            )
        };
        match decide_agent_chat_cwd_resolution(&current_cwd, &selected_cwd, status) {
            AgentChatCwdResolutionDecision::Unchanged => return true,
            AgentChatCwdResolutionDecision::BlockInFlight => {
                thread_entity.update(cx, |thread, cx| {
                    thread.set_notice_callout(
                        "Working directory not changed",
                        "Wait for the current Agent Chat turn to finish, then pick the directory again.",
                        cx,
                    );
                });
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_cwd_respawn_blocked",
                    old_cwd = %current_cwd.display(),
                    new_cwd = %selected_cwd.display(),
                    status = ?status,
                );
                return false;
            }
            AgentChatCwdResolutionDecision::RespawnNow => {}
        }

        let profile_ctx = crate::ai::agent_chat::profiles::AgentChatProfileContext::from_setup();
        let ai_preferences = crate::config::load_user_preferences().ai;
        let pi_launch =
            match crate::ai::agent_chat::launch::resolve_selected_pi_launch_with_cwd_override(
                &ai_preferences,
                &profile_ctx,
                Some(selected_cwd.clone()),
            ) {
                Ok(launch) => launch,
                Err(error) => {
                    thread_entity.update(cx, |thread, cx| {
                        thread.set_notice_callout(
                            "Working directory not changed",
                            format!("Failed to resolve Pi Agent Chat session: {error}"),
                            cx,
                        );
                    });
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_cwd_respawn_failed",
                        old_cwd = %current_cwd.display(),
                        new_cwd = %selected_cwd.display(),
                        error = %error,
                    );
                    return false;
                }
            };
        let manager = crate::ai::agent_chat::launch::warm_session_manager();
        let (lease, origin) = match manager.acquire_ready_or_spawn_cold(pi_launch.warm_spec()) {
            Ok(result) => result,
            Err(error) => {
                thread_entity.update(cx, |thread, cx| {
                    thread.set_notice_callout(
                        "Working directory not changed",
                        format!("Failed to start Pi Agent Chat session: {error}"),
                        cx,
                    );
                });
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_cwd_respawn_failed",
                    old_cwd = %current_cwd.display(),
                    new_cwd = %selected_cwd.display(),
                    error = %error,
                );
                return false;
            }
        };

        let new_ui_thread_id = lease.ui_thread_id.clone();
        let new_cwd = lease.cwd.clone();
        // The respawn resolves the *currently selected* profile (same ambient
        // resolution as every launch path). If the user changed their selected
        // profile since this thread launched, the swap also changes the
        // thread's profile — surface that instead of switching silently.
        let profile_changed = pi_launch.profile.id != current_profile_id;
        let new_profile_name = pi_launch.profile.name.clone();
        thread_entity.update(cx, |thread, cx| {
            thread.replace_pi_session(
                lease.connection.clone(),
                lease.ui_thread_id.clone(),
                lease.cwd.clone(),
                pi_launch.profile.id.clone(),
                pi_launch.profile.name.clone().into(),
                pi_launch.profile.icon_name.clone(),
                pi_launch.available_models.clone(),
                pi_launch.selected_model_id.clone(),
                cx,
            );
            let mut message = format!(
                "Working directory changed to `{}`. The Pi session was restarted for future turns; visible chat history was preserved.",
                new_cwd.display()
            );
            if profile_changed {
                message.push_str(&format!(
                    " This thread now uses the currently selected profile: {new_profile_name}."
                ));
            }
            thread.push_system_message(message, cx);
        });
        // Record under the profile the thread now runs as (pi_launch.profile),
        // not the pre-respawn profile — on profile drift the picker reads
        // recents for the live thread's profile, so recording under the old id
        // would silently misattribute the entry. The profile's own default cwd
        // is excluded as noise (it's already the Current row on fresh threads).
        let default_cwd = pi_launch
            .profile
            .cwd
            .clone()
            .unwrap_or_else(crate::setup::get_kit_path);
        crate::ai::agent_chat::ui::record_agent_chat_cwd_recent(
            &pi_launch.profile.id,
            new_cwd.clone(),
            Some(default_cwd.as_path()),
        );
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_cwd_respawn",
            old_cwd = %current_cwd.display(),
            new_cwd = %new_cwd.display(),
            ui_thread_id = %new_ui_thread_id,
            old_profile_id = %current_profile_id,
            new_profile_id = %pi_launch.profile.id,
            profile_changed = profile_changed,
            warm_origin = ?origin,
        );
        true
    }

    /// Switch the session to a retained background thread by `ui_thread_id`.
    /// Returns false when no retained thread matches.
    pub(crate) fn switch_to_thread(&mut self, ui_thread_id: &str, cx: &mut Context<Self>) -> bool {
        let Some(thread) = self
            .retained_threads
            .iter()
            .find(|t| t.read(cx).ui_thread_id() == ui_thread_id)
            .cloned()
        else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_switch_thread_missing",
                ui_thread_id,
            );
            return false;
        };
        self.activate_session_thread(thread, cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_switched_thread",
            ui_thread_id,
        );
        true
    }

    /// Build a machine-readable Agent Chat state snapshot for agentic testing.
    ///
    /// Returns cursor, picker, accepted item, thread status, layout metrics,
    /// and context readiness — everything an agent needs to verify Agent Chat
    /// interactions without screenshots.
    pub(crate) fn collect_agent_chat_state_snapshot(
        &self,
        cx: &App,
    ) -> crate::protocol::AgentChatStateSnapshot {
        let setup_snapshot = self.build_setup_protocol_snapshot(cx);

        if self.is_setup_mode() || setup_snapshot.is_some() {
            return self.build_agent_chat_setup_state_snapshot(setup_snapshot);
        }

        let thread = self.live_thread().read(cx);
        self.build_agent_chat_live_state_snapshot(thread, setup_snapshot, cx)
    }

    pub(crate) fn transcript_viewport_bounds_px(&self, cx: &App) -> Option<(f32, f32, f32, f32)> {
        self.transcript
            .as_ref()
            .map(|transcript| transcript.read(cx).viewport_bounds_px())
            .filter(|(_, _, width, height)| *width > 0.0 && *height > 0.0)
    }

    fn agent_chat_thread_status_label(status: AgentChatThreadStatus) -> &'static str {
        match status {
            AgentChatThreadStatus::Idle => "idle",
            AgentChatThreadStatus::Streaming => "streaming",
            AgentChatThreadStatus::WaitingForPermission => "waitingForPermission",
            AgentChatThreadStatus::Error => "error",
        }
    }

    fn build_agent_chat_setup_state_snapshot(
        &self,
        setup_snapshot: Option<crate::protocol::AgentChatSetupSnapshot>,
    ) -> crate::protocol::AgentChatStateSnapshot {
        let snapshot = crate::protocol::AgentChatStateSnapshot {
            status: "setup".to_string(),
            ui_variant: self.ui_variant.state_id().to_string(),
            setup: setup_snapshot,
            ..Default::default()
        };

        if let Some(ref setup) = snapshot.setup {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_setup_snapshot_built",
                reason_code = %setup.reason_code,
                primary_action = ?setup.primary_action,
                compatible_count = setup.compatible_agent_ids.len(),
                agent_picker_open = setup.agent_picker_open,
            );
        }

        snapshot
    }

    fn build_agent_chat_live_state_snapshot(
        &self,
        thread: &AgentChatThread,
        setup_snapshot: Option<crate::protocol::AgentChatSetupSnapshot>,
        cx: &App,
    ) -> crate::protocol::AgentChatStateSnapshot {
        let status_str = Self::agent_chat_thread_status_label(thread.status);

        let input_text = thread.input.text().to_string();
        let cursor_index = thread.input.cursor();
        let selection = thread.input.selection();
        let has_selection = !selection.is_empty();
        let selection_range = if has_selection {
            let (start, end) = selection.range();
            Some([start, end])
        } else {
            None
        };

        let context_ready =
            thread.context_bootstrap_state() != AgentChatContextBootstrapState::Preparing;

        let pending_parts = thread.pending_context_parts();

        let dictation_phase = crate::dictation::current_dictation_phase()
            .map(|phase| phase.as_automation_str().to_string());
        let input_layout =
            Self::build_agent_chat_input_layout_metrics(thread, &input_text, cursor_index);
        let transcript_scroll = self
            .transcript
            .as_ref()
            .map(|transcript| transcript.read(cx).scroll_metrics());
        let redact_focused_text_input =
            self.ui_variant == AgentChatUiVariant::FocusedTextMini && self.focused_text.is_some();

        crate::protocol::AgentChatStateSnapshot {
            schema_version: crate::protocol::AGENT_CHAT_STATE_SCHEMA_VERSION,
            resolved_target: None, // Populated by the caller (prompt handler) based on target resolution.
            ui_variant: self.ui_variant.state_id().to_string(),
            status: status_str.to_string(),
            input_text: if redact_focused_text_input {
                String::new()
            } else {
                input_text
            },
            cursor_index,
            has_selection,
            selection_range,
            message_count: thread.messages.len(),
            retained_thread_count: self.retained_threads.len(),
            fork_point_count: thread.fork_points().len(),
            awaiting_first_assistant_text: thread.awaiting_first_assistant_text(),
            picker: self.build_agent_chat_picker_state_snapshot(),
            spine: self.build_agent_chat_spine_state_snapshot(),
            last_accepted_item: self.last_accepted_item.clone(),
            context_chip_count: pending_parts.len(),
            context_summary: Self::build_agent_chat_context_summary(pending_parts),
            dictation_phase,
            context_ready,
            has_pending_permission: thread.pending_permission.is_some(),
            input_layout: Some(input_layout),
            transcript_scroll,
            focused_text: self.focused_text_state_snapshot(thread),
            setup: Self::build_agent_chat_live_setup_snapshot(thread, setup_snapshot),
            warnings: Vec::new(),
        }
    }

    fn build_agent_chat_picker_state_snapshot(
        &self,
    ) -> Option<crate::protocol::AgentChatPickerState> {
        self.composer_picker_session.as_ref().map(|session| {
            let selected_label = session
                .items
                .get(session.selected_index)
                .map(|item| item.label.to_string());
            let trigger = session.trigger.label();
            crate::protocol::AgentChatPickerState {
                open: true,
                trigger: trigger.to_string(),
                item_count: session.items.len(),
                selected_index: session.selected_index,
                selected_label,
            }
        })
    }

    fn build_agent_chat_spine_state_snapshot(
        &self,
    ) -> Option<crate::protocol::AgentChatSpineSnapshot> {
        let _projection = self.composer_spine.input.projection.as_ref()?;
        let owns_list = self.agent_chat_spine_owns_list();
        let started = std::time::Instant::now();
        let rows = if owns_list {
            self.agent_chat_spine_rows()
        } else {
            Vec::new()
        };
        let refresh_elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let selectable_rows = rows
            .iter()
            .filter(|row| row.is_selectable)
            .collect::<Vec<_>>();
        let selected_index = if selectable_rows.is_empty() {
            0
        } else {
            self.composer_spine
                .selected_index
                .min(selectable_rows.len().saturating_sub(1))
        };
        let selected_row_fingerprint = selectable_rows
            .get(selected_index)
            .map(|row| Self::agent_chat_spine_single_row_fingerprint(row));

        Some(crate::protocol::AgentChatSpineSnapshot {
            owns_list,
            active_segment_kind: self
                .agent_chat_spine_active_segment_kind_id()
                .unwrap_or("none")
                .to_string(),
            subsearch_source: self
                .agent_chat_spine_subsearch_source_id()
                .map(|source| source.to_string()),
            row_count: rows.len(),
            selectable_row_count: selectable_rows.len(),
            selected_index,
            row_fingerprint: Self::agent_chat_spine_row_fingerprint(&rows),
            selected_row_fingerprint,
            refresh_elapsed_ms,
        })
    }

    fn agent_chat_spine_active_segment_kind_id(&self) -> Option<&'static str> {
        let projection = self.composer_spine.input.projection.as_ref()?;
        Some(match &projection.active_segment_kind {
            crate::spine::SpineSegmentKind::FreeText => "freeText",
            crate::spine::SpineSegmentKind::ContextMention { .. } => "contextMention",
            crate::spine::SpineSegmentKind::SlashCommand { .. } => "slashCommand",
            crate::spine::SpineSegmentKind::Profile { .. } => "profile",
            crate::spine::SpineSegmentKind::Style { .. } => "style",
            crate::spine::SpineSegmentKind::Capture { .. } => "capture",
            crate::spine::SpineSegmentKind::ListFilter { .. } => "listFilter",
            crate::spine::SpineSegmentKind::ProjectCwd { .. } => "projectCwd",
            crate::spine::SpineSegmentKind::ModeExit { .. } => "modeExit",
        })
    }

    fn agent_chat_spine_subsearch_source_id(&self) -> Option<&'static str> {
        let projection = self.composer_spine.input.projection.as_ref()?;
        let crate::spine::SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } = &projection.active_segment_kind
        else {
            return None;
        };
        let (source, _) = crate::spine::catalog_subsearch::parse_context_subsearch(
            context_type,
            sub_query.as_deref(),
        )?;
        Some(match source {
            crate::spine::catalog_subsearch::ContextSubsearchSource::File => "file",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Project => "project",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard => "clipboard",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Notes => "notes",
            crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory => {
                "browserHistory"
            }
            crate::spine::catalog_subsearch::ContextSubsearchSource::Dictation => "dictation",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Scripts => "scripts",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Scriptlets => "scriptlets",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Skills => "skills",
            crate::spine::catalog_subsearch::ContextSubsearchSource::History => "history",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Calendar => "calendar",
            crate::spine::catalog_subsearch::ContextSubsearchSource::Notifications => {
                "notifications"
            }
        })
    }

    fn agent_chat_spine_row_fingerprint(rows: &[SpineListRow]) -> Option<String> {
        if rows.is_empty() {
            return None;
        }
        let parts = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                format!(
                    "{index}:{}",
                    Self::agent_chat_spine_single_row_fingerprint(row)
                )
            })
            .collect::<Vec<_>>();
        Some(Self::agent_chat_spine_hash_parts(&parts))
    }

    fn agent_chat_spine_single_row_fingerprint(row: &SpineListRow) -> String {
        let kind = match &row.kind {
            SpineListRowKind::ContextBuiltin { .. } => "contextBuiltin",
            SpineListRowKind::ContextSubSearch { .. } => "contextSubSearch",
            SpineListRowKind::ContextResult { .. } => "contextResult",
            SpineListRowKind::SlashCommand { .. } => "slashCommand",
            SpineListRowKind::Profile { .. } => "profile",
            SpineListRowKind::Style { .. } => "style",
            SpineListRowKind::CaptureTarget { .. } => "captureTarget",
            SpineListRowKind::Hint => "hint",
            SpineListRowKind::Empty => "empty",
        };
        let action = match &row.action {
            SpineListAction::InsertSegmentText { .. } => "insertSegmentText",
            SpineListAction::ResolveSegment {
                resolution_source, ..
            } => resolution_source.as_ref(),
            SpineListAction::OpenModeExit { .. } => "openModeExit",
            SpineListAction::OpenFileSearchPortal { .. } => "openFileSearchPortal",
            SpineListAction::Noop => "noop",
        };
        Self::agent_chat_spine_hash_parts(&[
            row.id.to_string(),
            kind.to_string(),
            row.is_selectable.to_string(),
            action.to_string(),
        ])
    }

    fn agent_chat_spine_hash_parts(parts: &[String]) -> String {
        let mut hash = 0xcbf29ce484222325_u64;
        for part in parts {
            for byte in part.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash ^= 0xff;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("fnv1a64:{hash:016x}")
    }

    fn build_agent_chat_input_layout_metrics(
        thread: &AgentChatThread,
        input_text: &str,
        cursor_index: usize,
    ) -> crate::protocol::AgentChatInputLayoutMetrics {
        let char_count = input_text.chars().count();
        let (visible_start, visible_end) = thread.input.visible_window_range(60);
        crate::protocol::AgentChatInputLayoutMetrics {
            char_count,
            visible_start,
            visible_end,
            cursor_in_window: cursor_index.saturating_sub(visible_start),
        }
    }

    fn build_agent_chat_context_summary(
        pending_parts: &[crate::ai::message_parts::AiContextPart],
    ) -> Option<String> {
        if pending_parts.is_empty() {
            None
        } else {
            Some(
                pending_parts
                    .iter()
                    .map(|part| part.label())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        }
    }

    fn build_agent_chat_live_setup_snapshot(
        thread: &AgentChatThread,
        setup_snapshot: Option<crate::protocol::AgentChatSetupSnapshot>,
    ) -> Option<crate::protocol::AgentChatSetupSnapshot> {
        if thread.setup_state().is_some() {
            setup_snapshot
        } else {
            None
        }
    }

    /// Build a protocol-layer setup snapshot from the current session state.
    fn build_setup_protocol_snapshot(
        &self,
        cx: &App,
    ) -> Option<crate::protocol::AgentChatSetupSnapshot> {
        let (agent_picker_open, agent_picker_selected_id) = if let Some(card) = &self.setup_card {
            let card = card.read(cx);
            if let Some(picker) = &card.agent_picker {
                let selected_id = picker
                    .items
                    .get(picker.selected_index)
                    .map(|entry| entry.id.to_string());
                (true, selected_id)
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };

        match &self.session {
            AgentChatSession::Setup(setup) => {
                Some(setup.to_protocol_snapshot(agent_picker_open, agent_picker_selected_id))
            }
            AgentChatSession::Live(thread) => {
                let t = thread.read(cx);
                t.setup_state()
                    .map(|s| s.to_protocol_snapshot(agent_picker_open, agent_picker_selected_id))
            }
        }
    }

    /// Observe a thread entity and sync the shared transcript/toolbar/composer
    /// whenever it notifies — but only while it is the active session thread.
    /// Retained background threads keep streaming on their own connections;
    /// their notifications only repaint unread indicators, never the shared UI.
    fn observe_session_thread(
        thread: &Entity<AgentChatThread>,
        cx: &mut Context<Self>,
    ) -> gpui::Subscription {
        cx.observe(thread, |this: &mut Self, thread, cx| {
            let is_session_thread = matches!(
                &this.session,
                AgentChatSession::Live(active) if active.entity_id() == thread.entity_id()
            );
            if !is_session_thread {
                // Background thread streamed; repaint so any visible unread
                // badge stays current, but leave the shared UI alone.
                cx.notify();
                return;
            }

            // Extract data from thread before mutable operations.
            let (
                activity_row_visible,
                messages,
                status,
                profile_display,
                model_display,
                new_ready,
                focused_text_phase,
                focused_text_input_locked,
                ui_thread_id,
                fork_points,
            ) = {
                let thread_ref = thread.read(cx);
                let activity = thread_ref.awaiting_first_assistant_text();
                let msgs = thread_ref.messages.clone();
                let st = thread_ref.status;
                let pd = thread_ref.profile_display().to_string();
                let md = thread_ref.selected_model_display().to_string();
                let phase = this.focused_text_mini_phase_for_thread(thread_ref);
                let locked = matches!(
                    phase,
                    Some(FocusedTextMiniPhase::Loading | FocusedTextMiniPhase::Streaming)
                );
                let ready = thread_ref
                    .messages
                    .iter()
                    .rev()
                    .filter(|m| matches!(m.role, AgentChatThreadMessageRole::Assistant))
                    .find_map(|m| parse_script_ready_receipt(m.body.as_ref()))
                    .filter(|r| r.validated)
                    .map(|r| r.path);
                let tid = thread_ref.ui_thread_id().to_string();
                let forks = thread_ref.fork_points().to_vec();
                (activity, msgs, st, pd, md, ready, phase, locked, tid, forks)
            };

            // The active thread's messages are on screen — mark them seen.
            this.thread_last_seen.insert(ui_thread_id, messages.len());

            let focused_text_mini_active = focused_text_phase.is_some();
            if focused_text_mini_active
                && this.focused_text_mini_input_locked
                && !focused_text_input_locked
            {
                this.pending_focused_text_mini_focus_restore = true;
                this.scope_focused = false;
                this.cursor_visible = true;
                tracing::info!(
                    target: "script_kit::focused_text",
                    event = "focused_text_mini_input_unlocked_focus_restore_queued",
                    phase = ?focused_text_phase,
                );
                cx.notify();
            }
            this.focused_text_mini_input_locked =
                focused_text_mini_active && focused_text_input_locked;

            if new_ready != this.ready_script_path {
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "agent_chat_generated_script_ready_state_changed",
                    ready = new_ready.is_some(),
                    path = ?new_ready,
                );
                this.ready_script_path = new_ready;
            }

            this.sync_balanced_focused_text_variation(&messages, status, cx);

            // Update toolbar status and model.
            if let Some(toolbar) = &this.toolbar {
                toolbar.update(cx, |toolbar, cx| {
                    toolbar.set_status(status, cx);
                    toolbar.set_profile_name(profile_display, cx);
                    toolbar.set_model_name(model_display, cx);
                });
            }

            // Update transcript.
            if let Some(transcript) = &this.transcript {
                transcript.update(cx, |transcript, cx| {
                    transcript.set_messages(messages, cx);
                    transcript.set_show_activity_row(activity_row_visible, cx);
                    transcript.set_thread_status(status, cx);
                    transcript.set_fork_points(fork_points, cx);
                });
            }

            // Update composer projections on any input/cursor change.
            this.refresh_agent_chat_spine_from_composer(cx);
            if !this.agent_chat_spine_owns_list() {
                this.refresh_composer_picker_session(cx);
            }

            if let Some(item_count) = this.focused_text_mini_sizing_count(&*cx) {
                crate::window_resize::resize_to_view_sync(
                    crate::window_resize::ViewType::FocusedTextMini,
                    item_count,
                );
            }
        })
    }

    pub(crate) fn new(thread: Entity<AgentChatThread>, cx: &mut Context<Self>) -> Self {
        // Preflight the Agent Chat session so the agent's advertised model list lands
        // in `thread.available_models` before the user opens the Change Model
        // picker. Fire-and-forget; `apply_event` handles the resulting
        // `ModelsAvailable` and `SetupRequired` events.
        thread.update(cx, |thread, cx| thread.refresh_models(cx));

        // Auto-scroll when thread state changes (new messages, streaming updates).
        let mut thread_observers = std::collections::HashMap::new();
        thread_observers.insert(
            thread.entity_id(),
            Self::observe_session_thread(&thread, cx),
        );
        // Cursor blink loop (530ms interval, same as ChatPrompt).
        let blink_task = cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(530))
                .await;
            if !crate::is_main_window_visible() {
                continue;
            }
            let result = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.cursor_visible = !view.cursor_visible;
                    cx.notify();
                })
            });
            if result.is_err() {
                break;
            }
        });

        // Defer slash command discovery (filesystem I/O) to after the first
        // render frame so the view switch is not blocked by skill enumeration.
        let slash_task = cx.spawn(async move |this, cx| {
            // Yield to let the initial render happen first.
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
            let commands = Self::discover_slash_commands();
            let _ = cx.update(|cx| {
                this.update(cx, |view, cx| {
                    view.cached_slash_commands = commands;
                    view.refresh_agent_chat_spine_from_composer(cx);
                    if !view.agent_chat_spine_owns_list() {
                        view.refresh_composer_picker_session(cx);
                    }
                    cx.notify();
                })
            });
        });

        Self {
            session: AgentChatSession::Live(thread),
            retained_threads: Vec::new(),
            thread_last_seen: std::collections::HashMap::new(),
            thread_observers,
            host_activation_subscription: None,
            focus_handle: cx.focus_handle(),
            permission_index: 0,
            permission_options_open: false,

            cursor_visible: true,
            _blink_task: blink_task,
            history_menu: None,
            history_closed_at: None,
            attach_menu_open: false,
            message_queue_expanded: false,
            search_state: None,
            cached_slash_commands: Vec::new(),
            _slash_discovery_task: slash_task,
            composer_picker_session: None,
            expanded_composer: false,
            composer_spine: Default::default(),
            dismissed_mention_trigger: None,
            composer_parent_window: None,
            inline_owned_context_tokens: HashSet::new(),
            typed_mention_aliases: std::collections::HashMap::new(),
            pasted_text_tokens: Vec::new(),
            pasted_image_tokens: Vec::new(),
            setup_card: None,
            toolbar: None,
            transcript: None,
            ui_variant: AgentChatUiVariant::Standard,
            focused_text: None,
            focused_text_variations: Vec::new(),
            focused_text_variation_tasks: Vec::new(),
            focused_text_variation_cancel_flags: Vec::new(),
            focused_text_variation_generation: 0,
            focused_text_variation_history: Vec::new(),
            focused_text_variation_history_index: None,
            focused_text_selected_variation: None,
            focused_text_editing_variation: None,
            focused_text_instruction_history: Vec::new(),
            focused_text_instruction_history_index: None,
            focused_text_instruction_history_draft: None,
            scope_input: String::new(),
            scope_visible: false,
            scope_focused: false,
            setup_agent_picker: None,
            opened_via_transient_trigger: None,

            last_accepted_item: None,
            test_probe: AgentChatTestProbe::default(),
            pending_retry_request: None,
            pending_history_resume: None,
            on_toggle_actions: None,
            on_close_requested: None,
            on_close_window_requested: None,
            on_open_history_command: None,
            on_paste_response_requested: None,
            on_focused_text_expand_requested: None,
            on_focused_text_collapse_requested: None,
            on_open_portal: None,
            on_profile_selected: None,
            pending_portal_session: None,
            footer_host: AgentChatFooterHost::Inline,
            ready_script_path: None,
            pending_slash_prime: None,
            context_capture_pending: false,
            focused_text_mini_input_locked: false,
            pending_focused_text_mini_focus_restore: false,
            allowed_portal_kinds: Self::all_portal_kinds(),
            _footer_action_task: None,
        }
    }

    /// Create an `AgentChatView` in **setup mode** — no live thread, just an
    /// inline setup card describing the blocker and available recovery actions.
    pub(crate) fn new_setup(
        state: super::setup_state::AgentChatInlineSetupState,
        cx: &mut Context<Self>,
    ) -> Self {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_setup_surface_rendered",
            title = %state.title,
        );
        let noop_blink = cx.spawn(async move |_this, _cx| {});
        let noop_slash = cx.spawn(async move |_this, _cx| {});
        Self {
            session: AgentChatSession::Setup(Box::new(state)),
            retained_threads: Vec::new(),
            thread_last_seen: std::collections::HashMap::new(),
            thread_observers: std::collections::HashMap::new(),
            host_activation_subscription: None,
            focus_handle: cx.focus_handle(),
            permission_index: 0,
            permission_options_open: false,

            cursor_visible: false,
            _blink_task: noop_blink,
            history_menu: None,
            history_closed_at: None,
            attach_menu_open: false,
            message_queue_expanded: false,
            search_state: None,
            cached_slash_commands: Vec::new(),
            _slash_discovery_task: noop_slash,
            composer_picker_session: None,
            expanded_composer: false,
            composer_spine: Default::default(),
            dismissed_mention_trigger: None,
            composer_parent_window: None,
            inline_owned_context_tokens: HashSet::new(),
            typed_mention_aliases: std::collections::HashMap::new(),
            pasted_text_tokens: Vec::new(),
            pasted_image_tokens: Vec::new(),
            setup_card: None,
            toolbar: None,
            transcript: None,
            ui_variant: AgentChatUiVariant::Standard,
            focused_text: None,
            focused_text_variations: Vec::new(),
            focused_text_variation_tasks: Vec::new(),
            focused_text_variation_cancel_flags: Vec::new(),
            focused_text_variation_generation: 0,
            focused_text_variation_history: Vec::new(),
            focused_text_variation_history_index: None,
            focused_text_selected_variation: None,
            focused_text_editing_variation: None,
            focused_text_instruction_history: Vec::new(),
            focused_text_instruction_history_index: None,
            focused_text_instruction_history_draft: None,
            scope_input: String::new(),
            scope_visible: false,
            scope_focused: false,
            setup_agent_picker: None,
            opened_via_transient_trigger: None,
            last_accepted_item: None,
            test_probe: AgentChatTestProbe::default(),
            pending_retry_request: None,
            pending_history_resume: None,
            on_toggle_actions: None,
            on_close_requested: None,
            on_close_window_requested: None,
            on_open_history_command: None,
            on_paste_response_requested: None,
            on_focused_text_expand_requested: None,
            on_focused_text_collapse_requested: None,
            on_open_portal: None,
            on_profile_selected: None,
            pending_portal_session: None,
            footer_host: AgentChatFooterHost::Inline,
            ready_script_path: None,
            pending_slash_prime: None,
            context_capture_pending: false,
            focused_text_mini_input_locked: false,
            pending_focused_text_mini_focus_restore: false,
            allowed_portal_kinds: Self::all_portal_kinds(),
            _footer_action_task: None,
        }
    }

    /// Scan plugin skill directories for slash command candidates, combine with
    /// built-in Claude Code commands. Returns typed `SlashCommandEntry` entries
    /// with full source identity.
    ///
    /// Uses `discover_plugin_skills()` so skill enumeration is routed through
    /// plugin ownership instead of hand-scanning `plugins/*/skills/`.
    /// Known Claude Code slash commands (used when the agent doesn't send
    /// an AvailableCommandsUpdate notification).
    const DEFAULT_SLASH_COMMANDS: &'static [&'static str] = &[
        "compact", "clear", "bug", "help", "init", "login", "logout", "status", "cost", "doctor",
        "review", "memory",
    ];

    fn discover_slash_commands() -> Vec<SlashCommandEntry> {
        let mut commands: Vec<SlashCommandEntry> = Self::DEFAULT_SLASH_COMMANDS
            .iter()
            .map(|s| SlashCommandEntry::default_command(s))
            .collect();

        let mut seen: std::collections::HashSet<String> =
            commands.iter().map(|e| e.qualified_key()).collect();

        // Seed collision tracker with default slash names so plugin/Claude
        // collisions against built-ins are detected.
        let default_names: std::collections::HashSet<String> =
            commands.iter().map(|e| e.name.clone()).collect();
        let mut owners_by_slash: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for entry in &commands {
            owners_by_slash
                .entry(entry.name.clone())
                .or_default()
                .push(entry.source.owner_label());
        }

        // Track plugin slash names for Claude-vs-plugin collision detection.
        let mut plugin_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Ok(index) = crate::plugins::discover_plugins() {
            if let Ok(skills) = crate::plugins::discover_plugin_skills(&index) {
                for skill in &skills {
                    let entry = SlashCommandEntry::plugin_skill(skill);
                    let owner = entry.source.owner_label();

                    plugin_names.insert(entry.name.clone());
                    owners_by_slash
                        .entry(entry.name.clone())
                        .or_default()
                        .push(owner);

                    if default_names.contains(&entry.name) {
                        tracing::warn!(
                            plugin_id = %skill.plugin_id,
                            skill_id = %skill.skill_id,
                            slash_name = %entry.name,
                            "agent_chat_slash_plugin_collides_with_default"
                        );
                    }

                    if seen.insert(entry.qualified_key()) {
                        tracing::info!(
                            plugin_id = %skill.plugin_id,
                            skill_id = %skill.skill_id,
                            "agent_chat_slash_skill_cataloged"
                        );
                        commands.push(entry);
                    }
                }
            }
        }

        // Also scan .claude/skills for user-level Claude Code skills
        let kit_path = crate::setup::get_kit_path();
        let claude_skills_dir = kit_path.join(".claude").join("skills");
        if let Ok(entries) = std::fs::read_dir(&claude_skills_dir) {
            for entry in entries.flatten() {
                let skill_md = entry.path().join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }
                let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };

                let desc = std::fs::read_to_string(&skill_md)
                    .ok()
                    .and_then(|content| parse_skill_description(&content))
                    .unwrap_or_default();

                let slash_entry =
                    SlashCommandEntry::claude_code_skill(name.clone(), desc, skill_md);

                owners_by_slash
                    .entry(name.clone())
                    .or_default()
                    .push("Claude Code".to_string());

                if plugin_names.contains(&name) {
                    tracing::warn!(
                        skill_id = %name,
                        "agent_chat_slash_claude_collides_with_plugin"
                    );
                }
                if default_names.contains(&name) {
                    tracing::warn!(
                        skill_id = %name,
                        "agent_chat_slash_claude_collides_with_default"
                    );
                }

                if seen.insert(slash_entry.qualified_key()) {
                    commands.push(slash_entry);
                }
            }
        }

        // Final cross-source collision pass: warn when multiple distinct
        // owners share the same bare slash name.
        for (slash_name, owners) in &owners_by_slash {
            if owners.len() > 1 {
                tracing::warn!(
                    slash_name = %slash_name,
                    owners = ?owners,
                    "agent_chat_slash_skill_name_collision"
                );
            }
        }

        tracing::info!(
            count = commands.len(),
            "agent_chat_slash_entries_discovered"
        );
        commands
    }

    /// Resolve cached slash commands against the agent-reported available
    /// commands. Plugin and Claude skills are always included regardless
    /// of provider advertisement; only default commands are gated.
    fn resolved_slash_commands(&self, available_commands: &[String]) -> Vec<SlashCommandEntry> {
        if available_commands.is_empty() {
            return self.cached_slash_commands.clone();
        }

        let available_set: std::collections::HashSet<&str> =
            available_commands.iter().map(|s| s.as_str()).collect();

        let mut result = Vec::new();

        for entry in &self.cached_slash_commands {
            match &entry.source {
                // Default commands are only included if the provider advertises them.
                SlashCommandSource::Default if available_set.contains(entry.name.as_str()) => {
                    result.push(entry.clone());
                }
                // Plugin and Claude skills are always included.
                SlashCommandSource::PluginSkill(_) | SlashCommandSource::ClaudeCodeSkill { .. } => {
                    result.push(entry.clone());
                }
                _ => {}
            }
        }

        // Include agent-reported commands that aren't in our cache
        for cmd in available_commands {
            let already_present = result.iter().any(|entry| {
                matches!(entry.source, SlashCommandSource::Default) && entry.name == *cmd
            });
            if !already_present {
                result.push(SlashCommandEntry::default_command(cmd));
            }
        }

        result
    }

    fn handle_picker_accept_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let accepted_via_key = if crate::ui_foundation::is_key_tab(key) {
            "tab"
        } else if crate::ui_foundation::is_key_enter(key) {
            "enter"
        } else {
            return false;
        };

        let Some(session) = self.composer_picker_session.as_ref() else {
            return false;
        };

        let pre_accept_item = session.items.get(session.selected_index).map(|item| {
            let trigger_str = session.trigger.label();
            (
                trigger_str.to_string(),
                item.label.to_string(),
                Self::telemetry_item_id(item),
            )
        });
        let cursor_before = self.live_thread().read(cx).input.cursor();

        self.accept_composer_picker_selection_impl(false, cx);

        let cursor_after = self.live_thread().read(cx).input.cursor();
        let permission_active = self.live_thread().read(cx).pending_permission.is_some();
        self.emit_key_route_telemetry(
            key,
            AgentChatKeyRouteTelemetryArgs {
                route: crate::protocol::AgentChatKeyRoute::Picker,
                cursor_before,
                cursor_after,
                caused_submit: false,
                consumed: true,
                permission_active,
            },
        );
        if let Some((trigger, label, id)) = pre_accept_item {
            self.emit_picker_accepted_telemetry(
                &trigger,
                &label,
                &id,
                accepted_via_key,
                cursor_after,
                false,
            );
        }
        if let Some(ref layout) = self.collect_agent_chat_state_snapshot(cx).input_layout {
            self.emit_input_layout_telemetry(layout);
        }

        true
    }

    /// Consume Tab / Shift+Tab. When a permission card is active,
    /// cycle the highlighted option; otherwise just swallow the key so
    /// the global interceptors do not re-open a fresh Agent Chat chat.
    pub(crate) fn handle_tab_key(
        &mut self,
        has_shift: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_setup_mode() {
            cx.notify();
            return true;
        }

        let option_count = self
            .live_thread()
            .read(cx)
            .pending_permission
            .as_ref()
            .map(|r| r.options.len())
            .unwrap_or(0);

        if option_count > 0 {
            self.permission_index =
                Self::step_permission_index(self.permission_index, option_count, has_shift);
            self.permission_options_open = option_count > 1;
            cx.notify();
            return true;
        }

        // Plain Tab accepts the focused picker item (same as Enter but without submit).
        if !has_shift && self.handle_picker_accept_key("tab", cx) {
            return true;
        }

        // Plain Tab accepts the highlighted spine projection row (e.g. the
        // `>` working-directory list). The global Tab interceptor routes Tab
        // here before the composer's key-down handler ever sees it, so
        // without this check Tab silently no-ops while Enter accepts —
        // proven at runtime via the cwd picker (Tab left `>desk` unresolved
        // while Enter resolved and respawned).
        if !has_shift
            && self.agent_chat_spine_owns_list()
            && self.accept_agent_chat_spine_projection_row(window, cx)
        {
            return true;
        }

        if self.handle_focused_text_variation_tab(has_shift, cx) {
            return true;
        }

        if self.handle_focused_text_scope_tab(has_shift, cx) {
            return true;
        }

        cx.notify();
        true
    }

    fn stage_pending_portal_session(
        &mut self,
        contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
        cx: &mut Context<Self>,
    ) {
        let thread = self.live_thread().read(cx);
        let composer_text = thread.input.text().to_string();
        let composer_cursor = thread.input.cursor();
        let replace_label = contract.replacement.preview_label();

        let Some(staged_state) = crate::ai::agent_chat::ui::portal_contract::next_portal_state(
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState::Idle,
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionEvent::Stage,
        ) else {
            tracing::error!(
                target: "script_kit::agent_chat",
                event = "agent_chat_portal_stage_state_missing",
                "idle portal session failed to stage"
            );
            return;
        };

        self.pending_portal_session = Some(AgentChatPendingPortalSession {
            contract: contract.clone(),
            composer_text,
            composer_cursor,
            state: staged_state,
        });
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::PortalStaged, cx);
        self.history_menu = None;
        self.attach_menu_open = false;

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_contract_staged",
            kind = ?contract.portal_kind,
            query = %contract.query,
            replace_label = %replace_label,
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    fn open_picker_portal(
        &mut self,
        portal_kind: crate::ai::context_selector::types::ContextPortalKind,
        replace_range: std::ops::Range<usize>,
        query: String,
        cx: &mut Context<Self>,
    ) {
        let current_text = self.live_thread().read(cx).input.text().to_string();
        let contract = crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract {
            portal_kind,
            query,
            replacement:
                crate::ai::agent_chat::ui::portal_contract::exact_replacement_target_for_range(
                    &current_text,
                    replace_range.clone(),
                    replace_range.start,
                ),
        };
        let _ = self.open_portal_contract(contract, cx);
    }

    fn focused_inline_token_span(
        &self,
        cx: &App,
    ) -> Option<crate::ai::context_mentions::InlineTokenSpan> {
        let thread = self.live_thread().read(cx);
        crate::ai::context_mentions::inline_token_at_cursor(
            thread.input.text(),
            thread.input.cursor(),
        )
    }

    fn focused_inline_mention(
        &self,
        cx: &App,
    ) -> Option<crate::ai::context_mentions::InlineContextMention> {
        let thread = self.live_thread().read(cx);
        let cursor = thread.input.cursor();
        crate::ai::context_mentions::parse_inline_context_mentions_with_aliases(
            thread.input.text(),
            &self.typed_mention_aliases,
        )
        .into_iter()
        .find(|mention| cursor > mention.range.start && cursor <= mention.range.end)
    }

    fn focused_inline_portal_intent(
        &self,
        cx: &App,
    ) -> Option<crate::ai::agent_chat::ui::portal_contract::AgentChatPortalIntent> {
        use crate::ai::agent_chat::ui::portal_contract::{
            intent_from_inline_token, intent_from_part, AgentChatPortalReplacementTarget,
        };

        let span = self.focused_inline_token_span(cx)?;
        let replacement = AgentChatPortalReplacementTarget::ExactToken {
            char_range: span.range.clone(),
            original_text: span.token.clone(),
            fallback_cursor: span.range.start,
        };
        if let Some(mention) = self.focused_inline_mention(cx) {
            return Some(intent_from_part(&mention.part, replacement));
        }

        intent_from_inline_token(&span.token, replacement)
    }

    fn focused_inline_mention_preview(&self, cx: &App) -> Option<AgentChatFocusedMentionPreview> {
        let span = self.focused_inline_token_span(cx)?;
        let intent = self.focused_inline_portal_intent(cx)?;
        Some(AgentChatFocusedMentionPreview {
            token: span.token,
            detail: crate::ai::agent_chat::ui::portal_contract::format_intent_preview(&intent),
        })
    }

    fn open_focused_mention_portal(&mut self, cx: &mut Context<Self>) -> bool {
        use crate::ai::agent_chat::ui::portal_contract::AgentChatPortalIntent;

        let Some(intent) = self.focused_inline_portal_intent(cx) else {
            return false;
        };
        let AgentChatPortalIntent::Portal(contract) = intent else {
            return false;
        };

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_focused_mention_portal_open",
            kind = ?contract.portal_kind,
            query = %contract.query,
            replace_label = %contract.replacement.preview_label(),
        );

        self.open_portal_contract(contract, cx)
    }

    pub(crate) fn attach_portal_part(
        &mut self,
        part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        use crate::ai::context_mentions::part_to_inline_token;

        let inline_token =
            part_to_inline_token(&part).unwrap_or_else(|| format!("@{}", part.label()));
        let should_claim_inline_ownership = self.should_claim_inline_mention_ownership(&part, cx);
        let current_text = self.live_thread().read(cx).input.text().to_string();
        let replacement = format!("{inline_token} ");

        let pending_portal_session = self.pending_portal_session.take();
        let (next_text, next_cursor, exact_match) =
            if let Some(session) = pending_portal_session.as_ref() {
                debug_assert_eq!(
                    session.state,
                    crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState::Active
                );
                crate::ai::agent_chat::ui::portal_contract::apply_portal_replacement(
                    &current_text,
                    &session.contract.replacement,
                    &replacement,
                )
            } else {
                let separator = if current_text.is_empty() || current_text.ends_with(' ') {
                    ""
                } else {
                    " "
                };
                let next_text = format!("{current_text}{separator}{inline_token} ");
                let next_cursor = next_text.chars().count();
                (next_text, next_cursor, false)
            };

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_reentry_applied",
            exact_match,
            new_token = %inline_token,
            portal_kind = ?pending_portal_session
                .as_ref()
                .map(|session| session.contract.portal_kind),
        );

        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(next_text);
            thread.input.set_cursor(next_cursor);
            thread.add_context_part(part.clone(), cx);
            cx.notify();
        });

        self.register_typed_alias(inline_token.clone(), part);
        if should_claim_inline_ownership {
            self.register_inline_owned_token(inline_token);
        }
        self.sync_inline_mentions(cx);
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn cancel_pending_portal_session(
        &mut self,
        portal_kind: crate::ai::context_selector::types::ContextPortalKind,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(session) = self.pending_portal_session.take() else {
            return false;
        };
        if session.contract.portal_kind != portal_kind {
            self.pending_portal_session = Some(session);
            return false;
        }

        let Some(state) = crate::ai::agent_chat::ui::portal_contract::next_portal_state(
            session.state,
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionEvent::Cancel,
        ) else {
            self.pending_portal_session = Some(session);
            return false;
        };
        let restore_text = session.composer_text.clone();
        let restore_cursor = session.composer_cursor;
        let cleared_state =
            crate::ai::agent_chat::ui::portal_contract::clear_terminal_portal_state(state);
        debug_assert_eq!(
            cleared_state,
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState::Idle
        );

        self.live_thread().update(cx, |thread, cx| {
            let cursor = restore_cursor.min(restore_text.chars().count());
            thread.input.set_text(restore_text.clone());
            thread.input.set_cursor(cursor);
            cx.notify();
        });

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_session_cancelled",
            kind = ?portal_kind,
            restored_cursor = restore_cursor,
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
        true
    }

    fn open_portal_contract(
        &mut self,
        contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
        cx: &mut Context<Self>,
    ) -> bool {
        matches!(
            self.open_portal_contract_result(contract, cx),
            PortalOpenResult::Opened
        )
    }

    fn open_portal_contract_result(
        &mut self,
        contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
        cx: &mut Context<Self>,
    ) -> PortalOpenResult {
        use crate::ai::agent_chat::ui::portal_contract::{
            decide_portal_open, next_portal_state, AgentChatPortalOpenRefusal,
            AgentChatPortalSessionEvent, AgentChatPortalSessionState,
        };

        let portal_kind = contract.portal_kind;
        let query = contract.query.clone();
        let is_allowed = self.is_portal_kind_allowed(portal_kind);
        let has_host_callback = self.on_open_portal.is_some();

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_open_decision",
            kind = ?portal_kind,
            allowed = is_allowed,
            has_host_callback,
        );

        match decide_portal_open(is_allowed, has_host_callback) {
            Ok(()) => {}
            Err(AgentChatPortalOpenRefusal::UnsupportedByHost) => {
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_portal_blocked_by_host_capability",
                    kind = ?portal_kind,
                );
                return PortalOpenResult::Refused(PortalRefusal::UnsupportedByHost);
            }
            Err(AgentChatPortalOpenRefusal::MissingHostCallback) => {
                tracing::warn!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_portal_open_blocked_missing_host_callback",
                    kind = ?portal_kind,
                );
                return PortalOpenResult::Refused(PortalRefusal::NoHost);
            }
        }

        let Some(callback) = self.on_open_portal.clone() else {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_portal_open_blocked_missing_host_callback",
                kind = ?portal_kind,
            );
            return PortalOpenResult::Refused(PortalRefusal::NoHost);
        };
        self.stage_pending_portal_session(contract, cx);
        if let Some(session) = self.pending_portal_session.as_mut() {
            session.state = next_portal_state(session.state, AgentChatPortalSessionEvent::Activate)
                .unwrap_or(AgentChatPortalSessionState::Active);
        }
        if portal_kind == crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_portal_query_staged",
                query = %query,
            );
        }
        cx.defer(move |cx| {
            callback(portal_kind, cx);
        });
        cx.notify();
        PortalOpenResult::Opened
    }

    fn approve_permission(&mut self, option_id: Option<String>, cx: &mut Context<Self>) {
        self.permission_index = 0;
        self.permission_options_open = false;
        self.live_thread().update(cx, |thread, cx| {
            thread.approve_pending_permission(option_id, cx);
        });
    }

    fn permission_request_tool_call_id(request: &AgentChatApprovalRequest) -> Option<&str> {
        let tool_call_id = request.preview.as_ref()?.tool_call_id.trim();
        if tool_call_id.is_empty() {
            None
        } else {
            Some(tool_call_id)
        }
    }

    fn permission_request_matches_message(
        msg: &AgentChatThreadMessage,
        request: &AgentChatApprovalRequest,
    ) -> bool {
        msg.tool_call_id
            .as_deref()
            .zip(Self::permission_request_tool_call_id(request))
            .is_some_and(|(msg_id, request_id)| msg_id == request_id)
    }

    fn selected_permission_option<'a>(
        &self,
        request: &'a AgentChatApprovalRequest,
    ) -> Option<(usize, &'a AgentChatApprovalOption)> {
        let index = self.normalized_permission_index(request.options.len());
        request.options.get(index).map(|option| (index, option))
    }

    fn first_allow_once_option(
        request: &AgentChatApprovalRequest,
    ) -> Option<(usize, &AgentChatApprovalOption)> {
        request
            .options
            .iter()
            .enumerate()
            .find(|(_, option)| !option.is_reject() && !option.is_persistent_allow())
    }

    fn first_allow_option(
        request: &AgentChatApprovalRequest,
    ) -> Option<(usize, &AgentChatApprovalOption)> {
        request
            .options
            .iter()
            .enumerate()
            .find(|(_, option)| !option.is_reject())
    }

    fn first_reject_option(
        request: &AgentChatApprovalRequest,
    ) -> Option<(usize, &AgentChatApprovalOption)> {
        request
            .options
            .iter()
            .enumerate()
            .find(|(_, option)| option.is_reject())
    }

    fn preferred_allow_option<'a>(
        &self,
        request: &'a AgentChatApprovalRequest,
    ) -> Option<(usize, &'a AgentChatApprovalOption)> {
        match self.selected_permission_option(request) {
            Some((index, option)) if !option.is_reject() => Some((index, option)),
            _ => {
                Self::first_allow_once_option(request).or_else(|| Self::first_allow_option(request))
            }
        }
    }

    fn approve_preferred_allow_option(
        &mut self,
        request: &AgentChatApprovalRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((index, option)) = self.preferred_allow_option(request) {
            self.permission_index = index;
            self.approve_permission(Some(option.option_id.clone()), cx);
            true
        } else {
            false
        }
    }

    fn approve_reject_option(
        &mut self,
        request: &AgentChatApprovalRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((index, option)) = Self::first_reject_option(request) {
            self.permission_index = index;
            self.approve_permission(Some(option.option_id.clone()), cx);
            true
        } else {
            self.approve_permission(None, cx);
            true
        }
    }

    fn toggle_permission_options(
        &mut self,
        request: &AgentChatApprovalRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        if request.options.len() <= 1 {
            return false;
        }

        if !self.permission_options_open {
            if let Some((index, _)) = self.preferred_allow_option(request) {
                self.permission_index = index;
            }
        }

        self.permission_options_open = !self.permission_options_open;
        cx.notify();
        true
    }

    fn normalized_permission_index(&self, option_count: usize) -> usize {
        if option_count == 0 {
            0
        } else {
            self.permission_index.min(option_count - 1)
        }
    }

    fn step_permission_index(current: usize, option_count: usize, reverse: bool) -> usize {
        if option_count == 0 {
            return 0;
        }

        if reverse {
            if current == 0 {
                option_count - 1
            } else {
                current - 1
            }
        } else {
            (current + 1) % option_count
        }
    }

    /// Handle key events when an inline permission card is active.
    /// Returns `true` if the key was consumed.
    fn handle_permission_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        request: &AgentChatApprovalRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        let option_count = request.options.len();
        self.permission_index = self.normalized_permission_index(option_count);

        if modifiers.platform
            && !modifiers.alt
            && !modifiers.control
            && key.eq_ignore_ascii_case("y")
        {
            return self.approve_preferred_allow_option(request, cx);
        }

        if modifiers.platform
            && modifiers.alt
            && !modifiers.control
            && key.eq_ignore_ascii_case("a")
        {
            self.toggle_permission_options(request, cx);
            return true;
        }

        if modifiers.platform
            && modifiers.alt
            && !modifiers.control
            && key.eq_ignore_ascii_case("z")
        {
            return self.approve_reject_option(request, cx);
        }

        if crate::ui_foundation::is_key_up(key) {
            self.permission_index =
                Self::step_permission_index(self.permission_index, option_count, true);
            self.permission_options_open = option_count > 1;
            cx.notify();
            return true;
        }

        if crate::ui_foundation::is_key_down(key) {
            self.permission_index =
                Self::step_permission_index(self.permission_index, option_count, false);
            self.permission_options_open = option_count > 1;
            cx.notify();
            return true;
        }

        // J/K navigation (vim-style, unmodified only)
        match key {
            "j" | "J" => {
                self.permission_index =
                    Self::step_permission_index(self.permission_index, option_count, false);
                self.permission_options_open = option_count > 1;
                cx.notify();
                return true;
            }
            "k" | "K" => {
                self.permission_index =
                    Self::step_permission_index(self.permission_index, option_count, true);
                self.permission_options_open = option_count > 1;
                cx.notify();
                return true;
            }
            _ => {}
        }

        if crate::ui_foundation::is_key_escape(key) && self.permission_options_open {
            self.permission_options_open = false;
            cx.notify();
            return true;
        }

        if crate::ui_foundation::is_key_escape(key) {
            self.approve_permission(None, cx);
            return true;
        }

        if crate::ui_foundation::is_key_enter(key) {
            if let Some(option) = request
                .options
                .get(self.normalized_permission_index(option_count))
            {
                self.approve_permission(Some(option.option_id.clone()), cx);
            } else {
                let _ = self.approve_preferred_allow_option(request, cx);
            }
            return true;
        }

        // 1-9 instant pick
        if let Ok(digit) = key.parse::<usize>() {
            if digit >= 1 {
                let idx = digit - 1;
                if let Some(option) = request.options.get(idx) {
                    self.permission_index = idx;
                    self.approve_permission(Some(option.option_id.clone()), cx);
                    return true;
                }
            }
        }

        false
    }

    pub(crate) fn set_input(&mut self, value: String, cx: &mut Context<Self>) {
        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_set_input_ignored_setup_mode",
                value_len = value.chars().count(),
            );
            return;
        }

        self.live_thread()
            .update(cx, |thread, cx| thread.set_input(value, cx));
        self.refresh_agent_chat_spine_from_composer(cx);
        if !self.agent_chat_spine_owns_list() {
            self.refresh_composer_picker_session(cx);
        }
    }

    pub(crate) fn set_input_in_window(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cache_composer_parent_window(window, cx);
        self.set_input(value, cx);
    }

    pub(crate) fn apply_test_fixture(
        &mut self,
        phase: &str,
        user_text: Option<String>,
        assistant_text: Option<String>,
        message_count: Option<usize>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(thread) = self.thread() else {
            return Err("Agent Chat view is not active".to_string());
        };

        thread.update(cx, |thread, cx| {
            thread.apply_test_fixture(phase, user_text, assistant_text, message_count, cx)
        })
    }

    pub(crate) fn scroll_test_transcript_to(
        &mut self,
        item_ix: usize,
        offset_px: f32,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let transcript = self.ensure_transcript(cx);
        transcript.read(cx).scroll_to(gpui::ListOffset {
            item_ix,
            offset_in_item: px(offset_px),
        });
        cx.notify();
        Ok(())
    }

    fn focused_text_previous_turns(
        thread: &AgentChatThread,
    ) -> Vec<crate::ai::focused_text::FocusedTextTurnSummary> {
        let mut turns = Vec::new();
        let mut pending_instruction: Option<String> = None;

        for message in &thread.messages {
            match message.role {
                AgentChatThreadMessageRole::User => {
                    if let Some(instruction) = pending_instruction.take() {
                        turns.push(crate::ai::focused_text::FocusedTextTurnSummary {
                            instruction,
                            semantics: crate::ai::focused_text::FocusedTextEditSemantics::Chat,
                            assistant_output: None,
                        });
                    }
                    pending_instruction = Some(message.body.to_string());
                }
                AgentChatThreadMessageRole::Assistant => {
                    if let Some(instruction) = pending_instruction.take() {
                        turns.push(crate::ai::focused_text::FocusedTextTurnSummary {
                            instruction,
                            semantics: crate::ai::focused_text::FocusedTextEditSemantics::Chat,
                            assistant_output: Some(message.body.to_string()),
                        });
                    }
                }
                AgentChatThreadMessageRole::Thought
                | AgentChatThreadMessageRole::Tool
                | AgentChatThreadMessageRole::System
                | AgentChatThreadMessageRole::Error => {}
            }
        }

        if let Some(instruction) = pending_instruction {
            turns.push(crate::ai::focused_text::FocusedTextTurnSummary {
                instruction,
                semantics: crate::ai::focused_text::FocusedTextEditSemantics::Chat,
                assistant_output: None,
            });
        }

        turns
    }

    /// Instant-rewrite entry: the staged focused text IS the context, so mark
    /// bootstrap ready and fire the three-variation rewrite turn immediately
    /// instead of waiting for the user to press Enter.
    pub(crate) fn submit_instant_rewrite(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        self.live_thread()
            .update(cx, |thread, cx| thread.mark_context_bootstrap_ready(cx));
        self.submit_focused_text_turn(
            crate::ai::focused_text::FocusedTextEditSemantics::Replace,
            cx,
            None,
        )
    }

    pub(crate) fn submit_focused_text_turn(
        &mut self,
        semantics: crate::ai::focused_text::FocusedTextEditSemantics,
        cx: &mut Context<Self>,
        source_text_override: Option<String>,
    ) -> Result<(), String> {
        let Some(state) = self.focused_text.as_ref() else {
            return Err("no_focused_text".to_string());
        };
        let mut snapshot = state.snapshot.clone();
        if let Some(text) = source_text_override.as_ref() {
            snapshot.text = text.clone();
            snapshot.metrics = crate::platform::accessibility::TextMetrics::from_text(text);
        }

        let Some(thread_entity) = self.thread() else {
            return Err("Agent Chat view is not active".to_string());
        };

        let instruction = {
            let thread = thread_entity.read(cx);
            if matches!(
                thread.status,
                AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
            ) {
                return Ok(());
            }
            let input = thread.input.text().trim().to_string();
            if !input.is_empty() {
                input
            } else if source_text_override.is_some() {
                thread
                    .messages
                    .iter()
                    .rev()
                    .find(|message| matches!(message.role, AgentChatThreadMessageRole::User))
                    .map(|message| message.body.trim().to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            }
        };
        if instruction.is_empty() {
            return Ok(());
        }

        self.push_focused_text_instruction_history(&instruction);
        self.reset_focused_text_instruction_history_navigation();

        let scope = self.scope_input.trim().to_string();
        let scope = if scope.is_empty() { None } else { Some(scope) };

        let previous_turns = {
            let thread = thread_entity.read(cx);
            Self::focused_text_previous_turns(thread)
        };

        let build_prompt_for = |angle: crate::ai::focused_text::FocusedTextPromptAngle| {
            crate::ai::focused_text::build_focused_text_prompt_with_angle(
                crate::ai::focused_text::FocusedTextPromptRequest {
                    snapshot: &snapshot,
                    instruction: &instruction,
                    scope: scope.as_deref(),
                    semantics,
                    previous_turns: &previous_turns,
                },
                angle,
            )
        };

        let angles = Self::focused_text_variation_angles();
        let (balanced_prompt, audit) =
            build_prompt_for(angles[FOCUSED_TEXT_BALANCED_VARIATION_INDEX]);

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_prompt_built",
            session_id = %audit.session_id,
            app_bundle_id = %audit.app_bundle_id.as_deref().unwrap_or(""),
            semantics = %audit.semantics,
            turn_count = audit.turn_count,
            capture_char_count = audit.capture_char_count,
            prompt_capture_char_count = audit.prompt_capture_char_count,
            capture_truncated = audit.capture_truncated,
            completion_status = %audit.completion_status,
            variation_angle = angles[FOCUSED_TEXT_BALANCED_VARIATION_INDEX].id(),
        );

        self.reset_focused_text_variations_for_submit();

        let balanced_blocks = vec![ContentBlock::Text(TextContent::new(balanced_prompt))];

        let submit_result = thread_entity.update(cx, |thread, cx| {
            thread.submit_blocks(balanced_blocks, instruction.clone(), cx)
        });
        if let Err(error) = submit_result {
            self.clear_focused_text_variations();
            return Err(error);
        }

        let base_thread_id = thread_entity.read(cx).ui_thread_id().to_string();
        for (index, angle) in angles.iter().copied().enumerate() {
            if index == FOCUSED_TEXT_BALANCED_VARIATION_INDEX {
                continue;
            }

            let (prompt, audit) = build_prompt_for(angle);
            tracing::info!(
                target: "script_kit::focused_text",
                event = "focused_text_variation_prompt_built",
                session_id = %audit.session_id,
                app_bundle_id = %audit.app_bundle_id.as_deref().unwrap_or(""),
                semantics = %audit.semantics,
                turn_count = audit.turn_count,
                capture_char_count = audit.capture_char_count,
                prompt_capture_char_count = audit.prompt_capture_char_count,
                capture_truncated = audit.capture_truncated,
                completion_status = %audit.completion_status,
                variation_angle = angle.id(),
                variation_index = index,
            );

            let blocks = vec![ContentBlock::Text(TextContent::new(prompt))];
            let aux_thread_id =
                format!("{}::focused-text-variation-{}", base_thread_id, angle.id());

            match thread_entity
                .read(cx)
                .start_auxiliary_turn(aux_thread_id, blocks)
            {
                Ok(handle) => {
                    if let Some(cancel) = handle.cancel {
                        self.focused_text_variation_cancel_flags.push(cancel);
                    }
                    self.spawn_focused_text_variation_task(index, handle.rx, cx);
                }
                Err(error) => self.mark_focused_text_variation_failed(index, error, cx),
            }
        }

        cx.notify();
        Ok(())
    }

    pub(crate) fn stage_inline_context_parts_from_host(
        &mut self,
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.is_setup_mode() {
            return Err("Agent Chat is in setup mode".to_string());
        }

        self.refresh_composer_picker_state_after_parent_change(cx);
        self.typed_mention_aliases.clear();
        self.inline_owned_context_tokens.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();

        let mut staged_text = String::new();
        let mut staged_aliases = Vec::with_capacity(parts.len());

        for part in parts {
            let inline_token = crate::ai::context_mentions::part_to_inline_token(&part)
                .unwrap_or_else(|| {
                    crate::ai::context_mentions::format_typed_label_mention_token(
                        "context",
                        part.label(),
                    )
                });
            if !staged_text.is_empty() && !staged_text.ends_with(' ') {
                staged_text.push(' ');
            }
            staged_text.push_str(&inline_token);
            staged_text.push(' ');
            staged_aliases.push((inline_token, part));
        }

        let staged_cursor = staged_text.chars().count();
        let staged_parts = staged_aliases
            .iter()
            .map(|(_, part)| part.clone())
            .collect::<Vec<_>>();

        self.live_thread().update(cx, move |thread, cx| {
            thread.replace_pending_context_parts(staged_parts, source, cx);
            thread.input.set_text(staged_text.clone());
            thread.input.set_cursor(staged_cursor);
            cx.notify();
        });

        for (inline_token, part) in staged_aliases {
            self.register_inline_owned_context_part(inline_token, part);
        }

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_host_inline_context_staged",
            source,
            token_count = self.inline_owned_context_tokens.len(),
        );
        cx.notify();
        Ok(())
    }

    pub(crate) fn stage_focused_text_from_host(
        &mut self,
        snapshot: crate::platform::accessibility::FocusedTextSnapshot,
        instruction: Option<String>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.is_setup_mode() {
            return Err("Agent Chat is in setup mode".to_string());
        }

        let mut snapshot = snapshot;
        let (text, capture_truncated) =
            crate::platform::accessibility::focused_text::truncate_focused_text_capture(
                snapshot.text,
            );
        snapshot.text = text;
        snapshot.metrics = crate::platform::accessibility::TextMetrics::from_text(&snapshot.text);
        let char_count = snapshot.metrics.chars;
        let word_count = snapshot.metrics.words;
        let app_name = snapshot.app.name.clone();
        let app_bundle_id = snapshot.app.bundle_id.clone();
        let capabilities = snapshot.capabilities;
        let session_id = snapshot.session_id.clone();
        let source_uri = format!("focused-text://{}", snapshot.session_id);
        let part = crate::ai::message_parts::AiContextPart::TextBlock {
            label: format!("Focused Text · {app_name} · {char_count} chars"),
            source: source_uri,
            text: snapshot.text.clone(),
            mime_type: Some("text/plain".to_string()),
        };

        let input = instruction
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let cursor = input.chars().count();

        self.typed_mention_aliases.clear();
        self.inline_owned_context_tokens.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();
        self.pending_portal_session = None;
        self.scope_input.clear();
        self.scope_visible = false;
        self.scope_focused = false;
        self.focused_text_mini_input_locked = false;
        self.pending_focused_text_mini_focus_restore = false;
        self.clear_focused_text_variations();
        self.focused_text = Some(FocusedTextAgentChatState {
            snapshot,
            session_id,
            app_name: app_name.clone(),
            app_bundle_id,
            char_count,
            word_count,
            context_status: FocusedTextContextStatus::Captured,
            capture_truncated,
            can_replace: capabilities.can_replace,
            can_append: capabilities.can_append,
            can_copy: capabilities.can_copy,
            originated_from_quick_prompt: false,
            last_apply_receipt: None,
            last_action_receipt: None,
        });

        self.live_thread().update(cx, move |thread, cx| {
            thread.replace_pending_context_parts(vec![part], source, cx);
            thread.input.set_text(input);
            thread.input.set_cursor(cursor);
            cx.notify();
        });

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_context_staged",
            source,
            app_name = %app_name,
            chars = char_count,
            words = word_count,
            context_status = "captured",
            capture_truncated,
        );
        cx.notify();
        Ok(())
    }

    pub(crate) fn stage_focused_text_capture_failure_from_host(
        &mut self,
        reason_code: &'static str,
        instruction: Option<String>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.is_setup_mode() {
            return Err("Agent Chat is in setup mode".to_string());
        }

        let snapshot =
            crate::platform::accessibility::focused_text::focused_text_snapshot_for_capture_failure(
            );
        let session_id = snapshot.session_id.clone();
        let app_name = snapshot.app.name.clone();
        let app_bundle_id = snapshot.app.bundle_id.clone();
        let input = instruction
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let cursor = input.chars().count();

        self.typed_mention_aliases.clear();
        self.inline_owned_context_tokens.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();
        self.pending_portal_session = None;
        self.scope_input.clear();
        self.scope_visible = false;
        self.scope_focused = false;
        self.focused_text_mini_input_locked = false;
        self.pending_focused_text_mini_focus_restore = false;
        self.clear_focused_text_variations();
        self.reset_focused_text_instruction_history_navigation();
        self.focused_text = Some(FocusedTextAgentChatState {
            snapshot,
            session_id,
            app_name: app_name.clone(),
            app_bundle_id,
            char_count: 0,
            word_count: 0,
            context_status: FocusedTextContextStatus::CaptureFailed { reason_code },
            capture_truncated: false,
            can_replace: false,
            can_append: false,
            can_copy: true,
            originated_from_quick_prompt: false,
            last_apply_receipt: None,
            last_action_receipt: None,
        });

        self.live_thread().update(cx, move |thread, cx| {
            thread.replace_pending_context_parts(Vec::new(), source, cx);
            thread.input.set_text(input);
            thread.input.set_cursor(cursor);
            cx.notify();
        });

        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_context_staged",
            source,
            app_name = %app_name,
            context_status = "captureFailed",
            reason_code,
        );
        cx.notify();
        Ok(())
    }

    pub(crate) fn clear_hosted_context_parts_from_host(
        &mut self,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.typed_mention_aliases.clear();
        self.inline_owned_context_tokens.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();
        self.pending_portal_session = None;
        self.live_thread().update(cx, |thread, cx| {
            thread.replace_pending_context_parts(Vec::new(), source, cx)
        });
        self.sync_inline_mentions(cx);
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    /// Stage a plugin skill exactly like accepting it from the Agent Chat slash picker.
    ///
    /// Main-menu skill launch is an external handoff, so it replaces stale
    /// composer context instead of appending to a previous draft, but it still
    /// leaves the slash text in the composer and does not submit.
    pub(crate) fn stage_selected_plugin_skill_from_main_menu(
        &mut self,
        skill: &crate::plugins::PluginSkill,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_setup_mode() {
            return false;
        }

        self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        self.history_menu = None;
        self.attach_menu_open = false;
        self.last_accepted_item = None;
        self.pending_history_resume = None;
        self.pending_portal_session = None;
        self.inline_owned_context_tokens.clear();
        self.typed_mention_aliases.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();

        let owner = if skill.plugin_title.is_empty() {
            skill.plugin_id.as_str()
        } else {
            skill.plugin_title.as_str()
        };
        let command_text = build_skill_slash_command_text(&skill.skill_id);
        let cursor_after = command_text.chars().count();
        let part = build_skill_context_part(&skill.title, owner, &skill.skill_id, &skill.path);
        let thread_id = self.live_thread().read(cx).ui_thread_id().to_string();
        let skill_file_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            skill.path.hash(&mut hasher);
            std::fs::metadata(&skill.path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .hash(&mut hasher);
            hasher.finish().to_string()
        };
        let identity = super::thread::SkillContextIdentity {
            thread_id,
            skill_id: skill.skill_id.clone(),
            skill_file_hash,
            staged_by: super::thread::SkillContextStagedBy::MainMenu,
        };

        self.last_accepted_item = Some(crate::protocol::AgentChatAcceptedItem {
            label: skill.title.clone(),
            id: format!("slash-cmd:plugin:{}:{}", skill.plugin_id, skill.skill_id),
            trigger: "/".to_string(),
            cursor_after,
        });

        self.live_thread().update(cx, |thread, cx| {
            thread.add_or_replace_skill_context(identity, part, cx);
            thread.input.set_text(command_text.clone());
            thread.input.set_cursor(cursor_after);
            thread.mark_context_bootstrap_ready(cx);
            cx.notify();
        });

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_main_menu_skill_staged_as_slash_selection",
            plugin_id = %skill.plugin_id,
            skill_id = %skill.skill_id,
            owner,
            cursor_after,
            "Main-menu skill staged without auto-submit"
        );
        true
    }

    /// Reuse the current live thread for a fresh external entry intent.
    ///
    /// Clears composer-local transient state and thread-scoped pending
    /// context so launcher-driven submits do not inherit stale chips or
    /// queued bootstrap work from the previous draft.
    pub(crate) fn submit_reused_entry_intent(&mut self, intent: String, cx: &mut Context<Self>) {
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::SubmitStarted, cx);
        self.history_menu = None;
        self.attach_menu_open = false;
        self.last_accepted_item = None;
        self.pending_history_resume = None;
        self.pending_portal_session = None;
        self.inline_owned_context_tokens.clear();
        self.typed_mention_aliases.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();

        self.live_thread().update(cx, |thread, cx| {
            thread.clear_pending_context_for_new_entry_intent(cx);
            thread.set_input(intent, cx);
            if let Err(error) = thread.submit_input(cx) {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "tab_ai_embedded_agent_chat_reuse_submit_failed",
                    error = %error,
                );
            }
        });
    }

    /// Reuse the current live thread for a fresh external entry intent that
    /// also replaces host-owned pending context in one atomic handoff.
    ///
    /// This is the detached/host reuse path when a surface needs to stage
    /// new inline context tokens and submit fresh user intent together. The
    /// two operations cannot be safely sequenced through the separate host
    /// staging and intent-only reuse helpers because they clear different
    /// parts of composer/thread state.
    pub(crate) fn submit_reused_entry_intent_with_host_context(
        &mut self,
        intent: String,
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.is_setup_mode() {
            return Err("Agent Chat is in setup mode".to_string());
        }

        self.refresh_composer_picker_state_after_parent_change(cx);
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::SubmitStarted, cx);
        self.history_menu = None;
        self.attach_menu_open = false;
        self.last_accepted_item = None;
        self.pending_history_resume = None;
        self.pending_portal_session = None;
        self.typed_mention_aliases.clear();
        self.inline_owned_context_tokens.clear();
        self.pasted_text_tokens.clear();
        self.pasted_image_tokens.clear();

        let trimmed_intent = intent.trim().to_string();
        let intent_len = trimmed_intent.len();
        let mut staged_text = String::new();
        let mut staged_aliases = Vec::with_capacity(parts.len());

        for part in parts {
            let inline_token = crate::ai::context_mentions::part_to_inline_token(&part)
                .unwrap_or_else(|| {
                    crate::ai::context_mentions::format_typed_label_mention_token(
                        "context",
                        part.label(),
                    )
                });
            if !staged_text.is_empty() && !staged_text.ends_with(' ') {
                staged_text.push(' ');
            }
            staged_text.push_str(&inline_token);
            staged_text.push(' ');
            staged_aliases.push((inline_token, part));
        }

        if !trimmed_intent.is_empty() {
            if !staged_text.is_empty() && !staged_text.ends_with(' ') {
                staged_text.push(' ');
            }
            staged_text.push_str(&trimmed_intent);
        }

        let staged_cursor = staged_text.chars().count();
        let staged_parts = staged_aliases
            .iter()
            .map(|(_, part)| part.clone())
            .collect::<Vec<_>>();

        for (inline_token, part) in &staged_aliases {
            self.register_inline_owned_context_part(inline_token.clone(), part.clone());
        }

        self.live_thread().update(cx, move |thread, cx| {
            thread.replace_pending_context_parts(staged_parts, source, cx);
            thread.input.set_text(staged_text.clone());
            thread.input.set_cursor(staged_cursor);
            if let Err(error) = thread.submit_input(cx) {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_reused_entry_intent_with_host_context_submit_failed",
                    error = %error,
                );
                return Err(error.to_string());
            }
            cx.notify();
            Ok::<(), String>(())
        })?;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_reused_entry_intent_with_host_context_submitted",
            source,
            token_count = self.inline_owned_context_tokens.len(),
            intent_len,
        );
        cx.notify();
        Ok(())
    }

    fn open_picker_trigger(&mut self, trigger: &str, cx: &mut Context<Self>) {
        if self.is_setup_mode() {
            self.composer_picker_session = None;
            self.dismissed_mention_trigger = None;
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_picker_trigger_ignored_setup_mode",
                trigger,
            );
            cx.notify();
            return;
        }

        self.attach_menu_open = false;
        self.history_menu = None;
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        self.set_input(trigger.to_string(), cx);
        self.refresh_agent_chat_spine_from_composer(cx);
        if !self.agent_chat_spine_owns_list() {
            self.refresh_composer_picker_session(cx);
        }
    }

    pub(crate) fn open_slash_picker(&mut self, cx: &mut Context<Self>) {
        self.open_picker_trigger("/", cx);
    }

    pub(crate) fn open_profile_trigger_picker(&mut self, cx: &mut Context<Self>) {
        self.open_picker_trigger(PROFILE_TRIGGER_STR, cx);
    }

    pub(crate) fn refresh_agent_chat_spine_from_composer(&mut self, cx: &mut Context<Self>) {
        if self.is_setup_mode() {
            self.composer_spine.clear();
            tracing::info!(target: "script_kit::agent_chat_spine", event = "refresh_skipped_setup_mode");
            return;
        }
        let (text, cursor, thread_cwd, profile_id) = {
            let thread = self.live_thread().read(cx);
            (
                thread.input.text().to_string(),
                thread.input.cursor(),
                thread.cwd().clone(),
                thread.profile_id().to_string(),
            )
        };
        // Snapshot for the `@project:` subsearch: section builders run
        // without cx and cannot read the thread entity.
        self.composer_spine.project_scope_cwd = Some(thread_cwd).filter(|cwd| {
            !cwd.as_os_str().is_empty() && cwd.as_path() != std::path::Path::new(".")
        });
        self.composer_spine.project_scope_cwd_recents =
            crate::ai::agent_chat::ui::agent_chat_cwd_recents_for_profile(&profile_id);
        self.composer_spine.refresh(&text, cursor);
        let owns = self.agent_chat_spine_owns_list();
        let kind = self
            .composer_spine
            .input
            .projection
            .as_ref()
            .map(|p| format!("{:?}", p.active_segment_kind))
            .unwrap_or_else(|| "none".to_string());
        tracing::info!(
            target: "script_kit::agent_chat_spine",
            event = "refresh_agent_chat_spine_from_composer",
            text = %text,
            cursor,
            owns_list = owns,
            active_kind = %kind,
        );
        if owns {
            self.composer_picker_session = None;
            self.dismissed_mention_trigger = None;
        }
        cx.notify();
    }

    pub(crate) fn agent_chat_spine_owns_list(&self) -> bool {
        self.composer_spine.owns_list() && self.agent_chat_spine_has_context_projection()
    }

    fn agent_chat_spine_has_context_projection(&self) -> bool {
        let Some(kind) = self
            .composer_spine
            .input
            .projection
            .as_ref()
            .map(|projection| &projection.active_segment_kind)
        else {
            return false;
        };
        Self::agent_chat_spine_segment_kind_has_context_projection(kind)
    }

    fn agent_chat_spine_segment_kind_has_context_projection(
        kind: &crate::spine::SpineSegmentKind,
    ) -> bool {
        matches!(
            kind,
            crate::spine::SpineSegmentKind::ContextMention { .. }
                | crate::spine::SpineSegmentKind::SlashCommand { .. }
                | crate::spine::SpineSegmentKind::Profile { .. }
                | crate::spine::SpineSegmentKind::Style { .. }
                | crate::spine::SpineSegmentKind::Capture { .. }
                | crate::spine::SpineSegmentKind::ListFilter { .. }
                | crate::spine::SpineSegmentKind::ProjectCwd { .. }
        )
    }

    fn agent_chat_spine_rows(&self) -> Vec<SpineListRow> {
        self.agent_chat_spine_sections()
            .into_iter()
            .flat_map(|section| section.rows)
            .collect()
    }

    fn agent_chat_spine_sections(&self) -> Vec<SpineListSection> {
        if !self.agent_chat_spine_has_context_projection() {
            return Vec::new();
        }
        let Some(projection) = self.composer_spine.input.projection.as_ref() else {
            return Vec::new();
        };

        if let crate::spine::SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } = &projection.active_segment_kind
        {
            if let Some((source, rich_query)) =
                crate::spine::catalog_subsearch::parse_context_subsearch(
                    context_type,
                    sub_query.as_deref(),
                )
            {
                let segment_index = projection.active_segment_index;
                let Some(segment_byte_range) = self
                    .composer_spine
                    .input
                    .parse
                    .segments
                    .get(segment_index)
                    .map(|segment| segment.byte_range.clone())
                else {
                    return Vec::new();
                };

                return match source {
                    crate::spine::catalog_subsearch::ContextSubsearchSource::File => {
                        let files = crate::file_search::search_files(rich_query, None, 10);
                        vec![self.agent_chat_rich_file_subsearch_section(
                            rich_query,
                            segment_index,
                            segment_byte_range,
                            &files,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Project => {
                        // Scoped to the thread cwd snapshot; `search_files`
                        // already falls back to a filesystem walk when
                        // Spotlight can't serve the scope (dot-directories).
                        let scope = self
                            .composer_spine
                            .project_scope_cwd
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .or_else(|| {
                                dirs::home_dir().map(|home| home.to_string_lossy().to_string())
                            });
                        let files =
                            crate::file_search::search_files(rich_query, scope.as_deref(), 10);
                        vec![self.agent_chat_rich_project_subsearch_section(
                            rich_query,
                            segment_index,
                            segment_byte_range,
                            &files,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Clipboard => {
                        let options =
                            crate::clipboard_history::RootClipboardHistorySectionOptions {
                                enabled: true,
                                max_results: 10,
                                min_query_chars: 0,
                                ..Default::default()
                            };
                        let hits =
                            crate::clipboard_history::search_root_clipboard_history_meta_direct(
                                rich_query, options,
                            );
                        vec![self.agent_chat_rich_clipboard_subsearch_section(
                            rich_query,
                            segment_index,
                            segment_byte_range,
                            &hits,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Notes
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::BrowserHistory
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Dictation
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::History
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Calendar
                    | crate::spine::catalog_subsearch::ContextSubsearchSource::Notifications => {
                        // Composer parity with the main window: these sources
                        // resolve through the shared spine attach resolver.
                        match crate::spine::attach::composer_subsearch_section(
                            source,
                            rich_query,
                            segment_index,
                            segment_byte_range,
                        ) {
                            Some(section) => {
                                vec![Self::agent_chat_rich_shared_subsearch_section(
                                    section, rich_query,
                                )]
                            }
                            None => Vec::new(),
                        }
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Scripts => {
                        vec![Self::agent_chat_spine_pending_subsearch_section(
                            "scripts",
                            "@scripts:",
                            "file-code",
                            rich_query,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Scriptlets => {
                        vec![Self::agent_chat_spine_pending_subsearch_section(
                            "scriptlets",
                            "@scriptlets:",
                            "file-code",
                            rich_query,
                        )]
                    }
                    crate::spine::catalog_subsearch::ContextSubsearchSource::Skills => {
                        vec![Self::agent_chat_spine_pending_subsearch_section(
                            "skills", "@skills:", "sparkles", rich_query,
                        )]
                    }
                };
            }
        }

        crate::spine::list::build_spine_list_sections_full_with_resolved_tokens_and_context(
            &self.composer_spine.input.parse,
            projection,
            None,
            &|token| self.typed_mention_aliases.contains_key(token),
            crate::spine::list::SpineListBuildContext {
                current_cwd: self.composer_spine.project_scope_cwd.as_deref(),
                cwd_recents: &self.composer_spine.project_scope_cwd_recents,
            },
        )
    }

    fn agent_chat_rich_file_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        files: &[crate::file_search::FileResult],
    ) -> SpineListSection {
        self.agent_chat_rich_file_backed_subsearch_section(
            query,
            segment_index,
            segment_byte_range,
            files,
            "Files",
            "@file:",
        )
    }

    fn agent_chat_rich_project_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        files: &[crate::file_search::FileResult],
    ) -> SpineListSection {
        self.agent_chat_rich_file_backed_subsearch_section(
            query,
            segment_index,
            segment_byte_range,
            files,
            "Project Files",
            "@project:",
        )
    }

    fn agent_chat_rich_file_backed_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        files: &[crate::file_search::FileResult],
        noun: &str,
        prefix: &str,
    ) -> SpineListSection {
        let trimmed = query.trim();
        let title = if trimmed.is_empty() {
            noun.to_string()
        } else {
            format!("{noun} matching \"{trimmed}\"")
        };
        let empty_subtitle = format!("Type after {prefix} to search");
        let rows = if files.is_empty() {
            vec![Self::agent_chat_spine_hint_row(
                "No files",
                if trimmed.is_empty() {
                    &empty_subtitle
                } else {
                    "No matching files"
                },
                Some("file"),
            )]
        } else {
            files
                .iter()
                .take(10)
                .enumerate()
                .map(|(index, file)| {
                    let short_path = crate::file_search::shorten_path(&file.path);
                    let replacement = format!(
                        "@file:{}",
                        crate::spine::catalog_subsearch::escape_ref_component(&short_path),
                    );
                    SpineListRow {
                        id: SharedString::from(format!(
                            "agent_chat-spine:file:{index}:{}",
                            file.path
                        )),
                        kind: SpineListRowKind::ContextResult {
                            context_type: SharedString::from("file"),
                            result_id: SharedString::from(file.path.clone()),
                        },
                        title: SharedString::from(file.name.clone()),
                        subtitle: Some(SharedString::from(short_path)),
                        meta: None,
                        icon: Some(SharedString::from("file")),
                        badges: Vec::new(),
                        score: 0,
                        is_selectable: true,
                        action_label: None,
                        action: SpineListAction::ResolveSegment {
                            segment_index,
                            segment_byte_range: segment_byte_range.clone(),
                            replacement: SharedString::from(replacement),
                            resolution_id: SharedString::from(file.path.clone()),
                            resolution_label: SharedString::from(file.name.clone()),
                            resolution_source: SharedString::from("file"),
                            trailing_space: true,
                        },
                    }
                })
                .collect()
        };

        SpineListSection {
            id: SharedString::from("agent_chat-spine-section-subsearch:file"),
            title: SharedString::from(title),
            subtitle: Some(SharedString::from("@file:")),
            icon: Some(SharedString::from("file")),
            rows,
        }
    }

    fn agent_chat_rich_clipboard_subsearch_section(
        &self,
        query: &str,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        hits: &[crate::clipboard_history::ClipboardEntryMeta],
    ) -> SpineListSection {
        let trimmed = query.trim();
        let title = if trimmed.is_empty() {
            "Recent Clipboard".to_string()
        } else {
            format!("Clipboard matching \"{trimmed}\"")
        };
        let rows = if hits.is_empty() {
            vec![Self::agent_chat_spine_hint_row(
                "No clipboard entries",
                if trimmed.is_empty() {
                    "Clipboard is empty"
                } else {
                    "No matching clipboard entries"
                },
                Some("clipboard"),
            )]
        } else {
            hits.iter()
                .take(10)
                .enumerate()
                .map(|(index, entry)| {
                    let preview =
                        crate::spine::text_preview::single_line_truncate(&entry.text_preview, 72);
                    let replacement = format!(
                        "@clipboard:{}",
                        crate::spine::catalog_subsearch::escape_ref_component(&entry.id),
                    );
                    SpineListRow {
                        id: SharedString::from(format!(
                            "agent_chat-spine:clipboard:{index}:{}",
                            entry.id
                        )),
                        kind: SpineListRowKind::ContextResult {
                            context_type: SharedString::from("clipboard"),
                            result_id: SharedString::from(entry.id.clone()),
                        },
                        title: SharedString::from(preview.clone()),
                        subtitle: Some(SharedString::from("Clipboard History")),
                        meta: None,
                        icon: Some(SharedString::from("clipboard")),
                        badges: Vec::new(),
                        score: 0,
                        is_selectable: true,
                        action_label: None,
                        action: SpineListAction::ResolveSegment {
                            segment_index,
                            segment_byte_range: segment_byte_range.clone(),
                            replacement: SharedString::from(replacement),
                            resolution_id: SharedString::from(entry.id.clone()),
                            resolution_label: SharedString::from(format!("Clipboard: {preview}")),
                            resolution_source: SharedString::from("clipboard"),
                            trailing_space: true,
                        },
                    }
                })
                .collect()
        };

        SpineListSection {
            id: SharedString::from("agent_chat-spine-section-subsearch:clipboard"),
            title: SharedString::from(title),
            subtitle: Some(SharedString::from("@clipboard:")),
            icon: Some(SharedString::from("clipboard")),
            rows,
        }
    }

    /// Convert a shared-resolver subsearch section into the composer's
    /// dropdown section, with an explicit empty row when nothing matches.
    fn agent_chat_rich_shared_subsearch_section(
        section: crate::spine::attach::ComposerSubsearchSection,
        query: &str,
    ) -> SpineListSection {
        let trimmed = query.trim();
        let rows = if section.rows.is_empty() {
            vec![Self::agent_chat_spine_hint_row(
                "No results",
                if trimmed.is_empty() {
                    "Nothing to attach from this source yet"
                } else {
                    "No matching entries"
                },
                Some(section.icon),
            )]
        } else {
            section.rows.into_iter().map(|row| row.row).collect()
        };
        SpineListSection {
            id: SharedString::from(format!(
                "agent_chat-spine-section-subsearch:{}",
                section.source_id
            )),
            title: SharedString::from(section.title),
            subtitle: Some(SharedString::from(format!("@{}:", section.source_id))),
            icon: Some(SharedString::from(section.icon.to_string())),
            rows,
        }
    }

    /// Re-derive the alias content for a shared-resolver subsearch token at
    /// accept time. The projection still reflects the pre-replacement input,
    /// so re-running the same deterministic query finds the accepted row.
    fn agent_chat_rich_subsearch_alias(
        &self,
        token: &str,
    ) -> Option<crate::ai::message_parts::AiContextPart> {
        let projection = self.composer_spine.input.projection.as_ref()?;
        let crate::spine::SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } = &projection.active_segment_kind
        else {
            return None;
        };
        let (source, rich_query) = crate::spine::catalog_subsearch::parse_context_subsearch(
            context_type,
            sub_query.as_deref(),
        )?;
        let section =
            crate::spine::attach::composer_subsearch_section(source, rich_query, 0, 0..0)?;
        section.rows.into_iter().find_map(|row| match row.alias {
            Some((alias_token, part)) if alias_token == token => Some(part),
            _ => None,
        })
    }

    fn agent_chat_spine_pending_subsearch_section(
        kind: &str,
        prefix: &str,
        icon: &str,
        query: &str,
    ) -> SpineListSection {
        let trimmed = query.trim();
        let title = if trimmed.is_empty() {
            format!("{prefix} search")
        } else {
            format!("{prefix}{trimmed}")
        };
        SpineListSection {
            id: SharedString::from(format!("agent_chat-spine-section-subsearch:{kind}")),
            title: SharedString::from(title),
            subtitle: Some(SharedString::from(prefix.to_string())),
            icon: Some(SharedString::from(icon.to_string())),
            rows: vec![Self::agent_chat_spine_hint_row(
                "Coming soon",
                "Rich search for this source is not wired yet",
                Some(icon),
            )],
        }
    }

    fn agent_chat_spine_hint_row(title: &str, subtitle: &str, icon: Option<&str>) -> SpineListRow {
        SpineListRow {
            id: SharedString::from(format!("agent_chat-spine:hint:{title}:{subtitle}")),
            kind: SpineListRowKind::Hint,
            title: SharedString::from(title.to_string()),
            subtitle: Some(SharedString::from(subtitle.to_string())),
            meta: None,
            icon: icon.map(|icon| SharedString::from(icon.to_string())),
            badges: Vec::new(),
            score: 0,
            is_selectable: false,
            action_label: None,
            action: SpineListAction::Noop,
        }
    }

    fn agent_chat_spine_selectable_rows(&self) -> Vec<SpineListRow> {
        self.agent_chat_spine_rows()
            .into_iter()
            .filter(|row| row.is_selectable)
            .collect()
    }

    pub(crate) fn move_agent_chat_spine_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.agent_chat_spine_selectable_rows().len();
        if len == 0 {
            self.composer_spine.selected_index = 0;
            self.composer_spine.visible_start = 0;
            cx.notify();
            return;
        }
        let current = self.composer_spine.selected_index.min(len - 1);
        self.composer_spine.selected_index = if delta < 0 {
            if current == 0 {
                len - 1
            } else {
                current - 1
            }
        } else {
            (current + 1) % len
        };
        let visible = crate::components::inline_dropdown::inline_dropdown_visible_range_from_start(
            self.composer_spine.visible_start,
            self.composer_spine.selected_index,
            len,
            8,
        );
        self.composer_spine.visible_start = visible.start;
        cx.notify();
    }

    fn selected_agent_chat_spine_row(&self) -> Option<SpineListRow> {
        self.agent_chat_spine_selectable_rows()
            .get(self.composer_spine.selected_index)
            .cloned()
    }

    pub(crate) fn accept_agent_chat_spine_projection_row(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(row) = self.selected_agent_chat_spine_row() else {
            return false;
        };
        self.apply_agent_chat_spine_action(row.action, window, cx)
    }

    fn apply_agent_chat_spine_action(
        &mut self,
        action: SpineListAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match action {
            SpineListAction::OpenFileSearchPortal {
                segment_byte_range,
                query,
                ..
            } => {
                // The composer spine's Files row routes through the same
                // portal contract as the context selector's top-level Files
                // row: full built-in File Search with preview, accept
                // replaces the `@file` segment with a compact token.
                let text = self.live_thread().read(cx).input.text().to_string();
                if segment_byte_range.end > text.len()
                    || !text.is_char_boundary(segment_byte_range.start)
                    || !text.is_char_boundary(segment_byte_range.end)
                {
                    return false;
                }
                let char_start = text[..segment_byte_range.start].chars().count();
                let char_end = text[..segment_byte_range.end].chars().count();
                self.open_picker_portal(
                    crate::ai::context_selector::types::ContextPortalKind::FileSearch,
                    char_start..char_end,
                    query.to_string(),
                    cx,
                );
                true
            }
            SpineListAction::InsertSegmentText {
                segment_index,
                segment_byte_range,
                text,
                trailing_space,
            } => self.replace_agent_chat_spine_segment(
                segment_index,
                segment_byte_range,
                text.as_ref(),
                trailing_space,
                cx,
            ),
            SpineListAction::ResolveSegment {
                segment_index,
                segment_byte_range,
                replacement,
                resolution_id,
                resolution_label,
                resolution_source,
                trailing_space,
            } => {
                // CWD resolution mirrors the main-menu behavior: strip the
                // segment from the composer entirely and respawn/re-acquire
                // the Pi session under the chosen directory. Pi binds cwd at
                // launch time, so updating only `thread.cwd()` would make the
                // footer chip lie while the agent kept using its original cwd.
                // Other sources insert the resolved replacement token like
                // normal.
                if resolution_source.as_ref() == "cwd" {
                    let path = PathBuf::from(resolution_id.as_ref());
                    let changed = self.respawn_live_thread_for_cwd(path, cx);
                    if !changed {
                        return false;
                    }
                    let ok = self.replace_agent_chat_spine_segment(
                        segment_index,
                        segment_byte_range,
                        "",
                        false,
                        cx,
                    );
                    // Bump the view so the script-app observer re-snapshots
                    // the footer with the new thread.cwd(); otherwise the
                    // stored snapshot keeps the prior cwd_display.
                    cx.notify();
                    return ok;
                }
                if resolution_source.as_ref() == "file" {
                    let full_path = resolution_id.as_ref().to_string();
                    // Compact token parity with portal attachments: show only
                    // `basename.ext`; the alias registry preserves the full
                    // path for context staging and the spine prompt plan.
                    let basename = std::path::Path::new(&full_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(&full_path);
                    let token = format!(
                        "@file:{}",
                        crate::spine::catalog_subsearch::escape_ref_component(basename),
                    );
                    let part = crate::ai::message_parts::AiContextPart::FilePath {
                        path: full_path,
                        label: resolution_label.as_ref().to_string(),
                    };
                    self.typed_mention_aliases.insert(token.clone(), part);
                    let ok = self.replace_agent_chat_spine_segment(
                        segment_index,
                        segment_byte_range,
                        &token,
                        trailing_space,
                        cx,
                    );
                    if ok {
                        self.sync_inline_mentions(cx);
                    }
                    return ok;
                }
                if resolution_source.as_ref() == "clipboard" {
                    let part = crate::ai::message_parts::AiContextPart::ResourceUri {
                        uri: format!("kit://clipboard-history?id={}", resolution_id.as_ref()),
                        label: resolution_label.as_ref().to_string(),
                    };
                    let ok = self.replace_agent_chat_spine_segment(
                        segment_index,
                        segment_byte_range,
                        replacement.as_ref(),
                        trailing_space,
                        cx,
                    );
                    if ok {
                        self.live_thread().update(cx, |thread, cx| {
                            thread.add_context_part(part.clone(), cx);
                            cx.notify();
                        });
                        self.sync_inline_mentions(cx);
                    }
                    return ok;
                }
                // Shared-resolver sources (notes, browser history, dictation,
                // chat history, calendar, notifications): register the alias
                // so sync_inline_mentions stages the real content, exactly
                // like the main window's spine_mention_aliases path.
                if let Some(part) = self.agent_chat_rich_subsearch_alias(replacement.as_ref()) {
                    self.typed_mention_aliases
                        .insert(replacement.as_ref().to_string(), part);
                }
                let ok = self.replace_agent_chat_spine_segment(
                    segment_index,
                    segment_byte_range,
                    replacement.as_ref(),
                    trailing_space,
                    cx,
                );
                if ok {
                    self.sync_inline_mentions(cx);
                }
                ok
            }
            SpineListAction::OpenModeExit { .. } | SpineListAction::Noop => false,
        }
    }

    fn replace_agent_chat_spine_segment(
        &mut self,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        replacement: &str,
        trailing_space: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let current = self.live_thread().read(cx).input.text().to_string();
        if segment_byte_range.start > segment_byte_range.end
            || segment_byte_range.end > current.len()
        {
            return false;
        }
        let Some(segment) = self.composer_spine.input.parse.segments.get(segment_index) else {
            return false;
        };
        if segment.byte_range != segment_byte_range {
            return false;
        }

        let prefix = &current[..segment_byte_range.start];
        let suffix = &current[segment_byte_range.end..];
        let add_space = trailing_space
            && !replacement.ends_with(char::is_whitespace)
            && !suffix.starts_with(char::is_whitespace);
        let space = if add_space { " " } else { "" };
        let next_text = format!("{prefix}{replacement}{space}{suffix}");
        let next_cursor_byte = prefix.len() + replacement.len() + space.len();
        let next_cursor = crate::spine::input_projection::char_cursor_for_byte_offset(
            &next_text,
            next_cursor_byte,
        );

        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(next_text);
            thread.input.set_cursor(next_cursor);
            cx.notify();
        });
        self.refresh_agent_chat_spine_from_composer(cx);
        true
    }

    pub(crate) fn dismiss_agent_chat_spine_projection(&mut self, cx: &mut Context<Self>) {
        self.composer_spine.clear();
        self.composer_picker_session = None;
        self.dismissed_mention_trigger = None;
        cx.notify();
    }

    /// If the cursor sits immediately after a resolved sigil segment (with
    /// optional trailing whitespace), remove the entire segment atomically so
    /// Backspace doesn't peel a resolved `@clipboard ` one char at a time.
    /// Returns `true` when an atomic removal happened.
    pub(crate) fn try_atomic_token_backspace(&mut self, cx: &mut Context<Self>) -> bool {
        if self.is_setup_mode() {
            return false;
        }
        let (text, cursor_chars) = {
            let thread = self.live_thread().read(cx);
            (thread.input.text().to_string(), thread.input.cursor())
        };
        let cursor_byte =
            crate::spine::input_projection::byte_offset_for_char_cursor(&text, cursor_chars);
        let parse = crate::spine::parse_spine(&text);
        // Find a non-FreeText segment whose end + trailing whitespace lands
        // exactly at the cursor.
        let candidate = parse.segments.iter().find(|seg| {
            if matches!(seg.kind, crate::spine::SpineSegmentKind::FreeText) {
                return false;
            }
            let after = &text[seg.byte_range.end..];
            let ws_end = seg.byte_range.end
                + after
                    .char_indices()
                    .take_while(|(_, c)| c.is_whitespace())
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
            ws_end == cursor_byte && cursor_byte > seg.byte_range.start
        });
        let Some(seg) = candidate else {
            return false;
        };
        // Only treat as atomic when the segment body has non-trivial content
        // beyond the sigil — avoid eating a lone `@` or `/` the user is
        // mid-typing.
        let body_len = seg.byte_range.end - seg.byte_range.start;
        if body_len <= 1 {
            return false;
        }
        let prefix = &text[..seg.byte_range.start];
        let suffix = &text[cursor_byte..];
        let next_text = format!("{prefix}{suffix}");
        let next_cursor =
            crate::spine::input_projection::char_cursor_for_byte_offset(&next_text, prefix.len());
        if let Some(thread_entity) = self.thread() {
            thread_entity.update(cx, |thread, cx| {
                thread.input.set_text(next_text);
                thread.input.set_cursor(next_cursor);
                cx.notify();
            });
        }
        self.refresh_agent_chat_spine_from_composer(cx);
        true
    }

    pub(crate) fn try_submit_agent_chat_spine_prompt_plan(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        // Protocol dispatch (simulateKey Cmd+Enter) reaches here without the
        // render/handle_key_down setup-mode early-returns; there is no live
        // thread to submit to while setup is showing.
        if self.is_setup_mode() {
            return false;
        }
        let (text, cursor, thread_cwd, profile_id) = {
            let thread = self.live_thread().read(cx);
            (
                thread.input.text().to_string(),
                thread.input.cursor(),
                thread.cwd().clone(),
                thread.profile_id().to_string(),
            )
        };
        // Snapshot for the `@project:` subsearch: section builders run
        // without cx and cannot read the thread entity.
        self.composer_spine.project_scope_cwd = Some(thread_cwd).filter(|cwd| {
            !cwd.as_os_str().is_empty() && cwd.as_path() != std::path::Path::new(".")
        });
        self.composer_spine.project_scope_cwd_recents =
            crate::ai::agent_chat::ui::agent_chat_cwd_recents_for_profile(&profile_id);
        self.composer_spine.refresh(&text, cursor);
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan_with_aliases(
            &self.composer_spine.input.parse,
            &self.typed_mention_aliases,
        );
        if !plan.should_submit_to_chat() {
            return false;
        }
        let prompt = plan.normalized_prompt.trim().to_string();
        let parts = plan.context_parts.clone();
        if prompt.is_empty() && parts.is_empty() {
            return false;
        }
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::SubmitStarted, cx);
        if let Err(error) = self.submit_reused_entry_intent_with_host_context(
            prompt,
            parts,
            "agent_chat_spine_prompt_plan",
            cx,
        ) {
            tracing::warn!(
                target: "script_kit::spine",
                event = "agent_chat_spine_prompt_plan_submit_failed",
                error = %error,
            );
            return false;
        }
        self.composer_spine.clear();
        cx.notify();
        true
    }

    pub(crate) fn current_prompt_handoff_payload(
        &mut self,
        adapter_id: crate::ai::agent_prompt_handoff::AgentPromptHandoffAdapterId,
        cx: &mut Context<Self>,
    ) -> Result<
        crate::ai::agent_prompt_handoff::AgentPromptHandoffPayload,
        crate::ai::agent_prompt_handoff::AgentPromptHandoffError,
    > {
        if self.is_setup_mode() {
            return Err(crate::ai::agent_prompt_handoff::AgentPromptHandoffError::SetupMode);
        }

        self.sync_inline_mentions(cx);
        let (raw_input, cursor, cwd, model_id, attached_parts) = {
            let thread = self.live_thread().read(cx);
            (
                thread.input.text().to_string(),
                thread.input.cursor(),
                thread.cwd().clone(),
                thread.selected_model_id().map(str::to_string),
                thread.pending_context_parts().to_vec(),
            )
        };

        if raw_input.trim().is_empty() {
            return Err(crate::ai::agent_prompt_handoff::AgentPromptHandoffError::EmptyPrompt);
        }

        self.composer_spine.refresh(&raw_input, cursor);
        let plan = crate::spine::prompt_plan::build_spine_prompt_plan_with_aliases(
            &self.composer_spine.input.parse,
            &self.typed_mention_aliases,
        );
        crate::ai::agent_prompt_handoff::compile_handoff_payload_from_spine_plan(
            adapter_id,
            raw_input,
            cwd,
            model_id,
            attached_parts,
            plan,
        )
    }

    fn handle_agent_chat_spine_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.agent_chat_spine_owns_list() {
            return false;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        if modifiers.platform && crate::ui_foundation::is_key_enter(key) {
            let _ = self.try_submit_agent_chat_spine_prompt_plan(cx);
            return true;
        }
        if crate::ui_foundation::is_key_up(key) {
            self.move_agent_chat_spine_selection(-1, cx);
            return true;
        }
        if crate::ui_foundation::is_key_down(key) {
            self.move_agent_chat_spine_selection(1, cx);
            return true;
        }
        if crate::ui_foundation::is_key_enter(key)
            || (crate::ui_foundation::is_key_tab(key) && !modifiers.shift)
        {
            return self.accept_agent_chat_spine_projection_row(window, cx);
        }
        if crate::ui_foundation::is_key_escape(key) {
            self.dismiss_agent_chat_spine_projection(cx);
            return true;
        }
        false
    }

    fn render_agent_chat_spine_projection_area(
        &mut self,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
        _cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if !self.agent_chat_spine_has_context_projection() {
            return div().flex_1().min_h(px(0.0)).into_any_element();
        }
        if self.composer_spine.input.projection.is_none() {
            return div().flex_1().min_h(px(0.0)).into_any_element();
        };
        let sections = self.agent_chat_spine_sections();
        let selected_index = self.composer_spine.selected_index;
        let list_colors = ListItemColors::from_theme(theme);
        let main_menu_theme = crate::designs::MainMenuThemeVariant::default();
        let row_height = crate::list_item::effective_list_item_height_for_theme(main_menu_theme);
        let mut selectable_index = 0usize;
        let mut children = Vec::new();
        let mut is_first_section = true;

        for section in sections {
            children.push(
                crate::list_item::render_section_header_with_subtitle(
                    section.title.as_ref(),
                    section.icon.as_ref().map(|icon| icon.as_ref()),
                    section.subtitle.as_ref().map(|subtitle| subtitle.as_ref()),
                    list_colors,
                    is_first_section,
                )
                .into_any_element(),
            );
            is_first_section = false;
            for row in section.rows {
                let row_selectable_index = selectable_index;
                let selected = row.is_selectable && row_selectable_index == selected_index;
                if row.is_selectable {
                    selectable_index += 1;
                }
                let row_id = row.id.to_string();
                let title = row.title.to_string();
                let subtitle = row.subtitle.as_ref().map(|s| s.to_string());
                let source_hint = row.meta.as_ref().map(|s| s.to_string());
                let shortcut = row.action_label.as_ref().map(|s| s.to_string());
                let icon_kind = row
                    .icon
                    .as_ref()
                    .and_then(|icon| IconKind::from_icon_hint(icon.as_ref()));
                let (type_label, type_icon) = row.kind.type_accessory_info();
                let action = row.action.clone();
                let click_view = weak_view.clone();
                let list_row = ListItem::new(title, list_colors)
                    .selected(selected)
                    .main_menu_theme(main_menu_theme)
                    .semantic_id(format!("agent_chat-spine-row-{row_id}"))
                    .description_opt(subtitle)
                    .source_hint_opt(source_hint)
                    .shortcut_opt(shortcut)
                    .icon_kind_opt(icon_kind)
                    .type_accessory_opt(Some(TypeAccessory {
                        label: type_label,
                        icon_name: type_icon,
                    }));

                children.push(
                    div()
                        .id(SharedString::from(format!("agent_chat-spine-row-{row_id}")))
                        .w_full()
                        .h(px(row_height))
                        .when(row.is_selectable, |d| {
                            d.cursor_pointer().on_click(move |_event, window, cx| {
                                if let Some(entity) = click_view.upgrade() {
                                    entity.update(cx, |chat, cx| {
                                        chat.composer_spine.selected_index = row_selectable_index;
                                        chat.apply_agent_chat_spine_action(
                                            action.clone(),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                            })
                        })
                        .child(list_row)
                        .into_any_element(),
                );
            }
        }

        div()
            .id("agent_chat-spine-projection")
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scrollbar()
            .py(px(4.0))
            .children(children)
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_agent_chat_middle_area(
        &mut self,
        is_empty: bool,
        show_sidecar: bool,
        ui_variant: AgentChatUiVariant,
        status_label: &'static str,
        message_count: usize,
        context_chip_count: usize,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if self.agent_chat_spine_owns_list() {
            return self.render_agent_chat_spine_projection_area(weak_view, theme, cx);
        }
        if is_empty {
            return div()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .h_full()
                .overflow_hidden()
                .child(crate::components::render_agent_chat_empty_guidance(
                    theme, cx,
                ))
                .into_any_element();
        }
        if show_sidecar {
            return div()
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .h_full()
                .overflow_hidden()
                .flex()
                .flex_row()
                .child(self.ensure_transcript(cx).into_any_element())
                .child(Self::render_variant_sidecar(
                    ui_variant,
                    status_label,
                    message_count,
                    context_chip_count,
                    theme,
                ))
                .into_any_element();
        }
        div()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(self.ensure_transcript(cx).into_any_element())
            .into_any_element()
    }

    pub(crate) fn open_profile_picker(&mut self, cx: &mut Context<Self>) {
        self.open_profile_trigger_picker(cx);
    }

    pub(crate) fn open_slash_picker_in_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cache_composer_parent_window(window, cx);
        self.open_slash_picker(cx);
    }

    pub(crate) fn open_profile_picker_in_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_profile_trigger_picker_in_window(window, cx);
    }

    pub(crate) fn open_profile_trigger_picker_in_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cache_composer_parent_window(window, cx);
        self.open_profile_trigger_picker(cx);
    }

    // ── Rendering helpers ─────────────────────────────────────────

    fn prompt_colors() -> PromptColors {
        PromptColors::from_theme(&theme::get_cached_theme())
    }

    fn render_variant_badge(
        ui_variant: AgentChatUiVariant,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        div()
            .w_full()
            .px(px(12.0))
            .pt(px(6.0))
            .pb(px(2.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme.colors.accent.selected))
                    .child(ui_variant.menu_name()),
            )
            .child(div().text_xs().opacity(0.45).child(ui_variant.state_id()))
            .into_any_element()
    }

    fn render_variant_sidecar(
        ui_variant: AgentChatUiVariant,
        status_label: &'static str,
        message_count: usize,
        context_chip_count: usize,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        div()
            .w(px(168.0))
            .flex_shrink_0()
            .h_full()
            .border_l_1()
            .border_color(rgba((theme.colors.ui.border << 8) | 0x38))
            .px(px(10.0))
            .py(px(8.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .text_xs()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme.colors.text.primary))
                    .child("State"),
            )
            .child(
                div()
                    .opacity(0.58)
                    .child("variant ")
                    .child(ui_variant.state_id()),
            )
            .child(div().opacity(0.58).child("status ").child(status_label))
            .child(
                div()
                    .opacity(0.58)
                    .child("messages ")
                    .child(message_count.to_string()),
            )
            .child(
                div()
                    .opacity(0.58)
                    .child("context ")
                    .child(context_chip_count.to_string()),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_composer_input_text(
        input_text: &str,
        input_cursor: usize,
        input_selection: TextSelection,
        cursor_visible: bool,
        placeholder_label: &'static str,
        multiline: bool,
        mention_highlights: &[TextHighlightRange],
        pasted_text_pills: &[TextInlinePillRange],
        placeholder_text: Rgba,
        theme: &crate::theme::Theme,
        max_visible_height: Option<f32>,
    ) -> gpui::AnyElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .justify_center()
            .min_h(px(Self::AGENT_CHAT_INPUT_LINE_HEIGHT))
            .when_some(max_visible_height, |d, height| {
                d.max_h(px(height)).overflow_hidden()
            })
            // Empirical: px(17) here renders identically to px(16) in
            // the main menu input. The 1px offset is a GPUI layout quirk.
            .text_size(px(Self::AGENT_CHAT_INPUT_FONT_SIZE))
            .line_height(px(Self::AGENT_CHAT_INPUT_LINE_HEIGHT))
            .text_color(if input_text.is_empty() {
                placeholder_text
            } else {
                rgb(theme.colors.text.primary)
            })
            .child(if input_text.is_empty() {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .w(px(crate::panel::CURSOR_WIDTH))
                            .h(px(crate::panel::CURSOR_HEIGHT_LG))
                            .when(cursor_visible, |d| d.bg(rgb(theme.colors.text.primary))),
                    )
                    .child(
                        div()
                            .ml(px(-2.0))
                            .text_color(placeholder_text)
                            .child(placeholder_label),
                    )
                    .into_any_element()
            } else {
                render_text_input_cursor_selection(TextInputRenderConfig {
                    cursor: input_cursor,
                    selection: Some(input_selection),
                    multiline,
                    cursor_visible,
                    cursor_color: theme.colors.accent.selected,
                    text_color: theme.colors.text.primary,
                    selection_color: theme.colors.accent.selected,
                    selection_text_color: theme.colors.text.primary,
                    cursor_height: crate::panel::CURSOR_HEIGHT_LG,
                    cursor_width: crate::panel::CURSOR_WIDTH,
                    container_height: Some(Self::AGENT_CHAT_INPUT_LINE_HEIGHT),
                    highlight_ranges: mention_highlights,
                    pill_ranges: pasted_text_pills,
                    ..TextInputRenderConfig::default_for_prompt(input_text)
                })
                .into_any_element()
            })
            .into_any_element()
    }

    fn render_input_profile_icon(
        id: &'static str,
        profile_icon_name: Option<&str>,
        active_pending: bool,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let icon_path = crate::components::footer_chrome::footer_icon_path_or_profile(
            profile_icon_name
                .unwrap_or(crate::components::footer_chrome::FOOTER_PROFILE_ICON_TOKEN),
        );
        let icon = gpui::svg()
            .path(icon_path)
            .size(px(13.0))
            .text_color(if active_pending {
                rgb(theme.colors.accent.selected)
            } else {
                rgb(theme.colors.text.muted)
            });

        let container = div()
            .id(id)
            .flex_none()
            .size(px(24.0))
            .rounded(px(7.0))
            .bg(rgba((theme.colors.text.primary << 8) | 0x08))
            .border_1()
            .border_color(rgba((theme.colors.text.primary << 8) | 0x14))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(move |_event, window, cx| {
                if let Some(entity) = weak_view.upgrade() {
                    entity.update(cx, |chat, cx| {
                        chat.open_profile_trigger_picker_in_window(window, cx);
                    });
                }
            });

        if active_pending {
            container
                .child(icon)
                .with_animation(
                    "agent_chat-input-profile-icon-pulse",
                    Animation::new(Duration::from_millis(2000)).repeat(),
                    |style, delta| {
                        let sine = (delta * std::f32::consts::PI * 2.0).sin();
                        let a = 0.8 + (0.2 * sine);
                        style.opacity(a)
                    },
                )
                .into_any_element()
        } else {
            container.child(icon).into_any_element()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_composer_input_shell(
        input_text: &str,
        input_cursor: usize,
        input_selection: TextSelection,
        cursor_visible: bool,
        is_empty: bool,
        mention_highlights: &[TextHighlightRange],
        pasted_text_pills: &[TextInlinePillRange],
        placeholder_text: Rgba,
        profile_icon_name: Option<&str>,
        profile_active_pending: bool,
        status: AgentChatThreadStatus,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
        expanded_composer: bool,
    ) -> gpui::AnyElement {
        let menu_def = crate::designs::current_main_menu_theme().def();
        let max_visible_height =
            expanded_composer.then_some(Self::AGENT_CHAT_INPUT_LINE_HEIGHT * 6.0);
        let input_body = Self::render_composer_input_text(
            input_text,
            input_cursor,
            input_selection,
            cursor_visible,
            if is_empty {
                "Ask anything\u{2026}"
            } else {
                "Follow up\u{2026}"
            },
            true,
            mention_highlights,
            pasted_text_pills,
            placeholder_text,
            theme,
            max_visible_height,
        );
        let _ = (profile_icon_name, profile_active_pending);
        let can_send = !input_text.trim().is_empty();

        crate::components::main_view_chrome::render_main_view_input_shell_with_height(
            theme,
            menu_def,
            crate::components::main_view_chrome::MainViewInputChrome {
                body: input_body,
                trailing: vec![Self::render_send_button_for_state(
                    can_send, status, weak_view, theme,
                )],
            },
            expanded_composer.then_some(Self::composer_height_for_expanded(true)),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_composer_bar(
        input_text: &str,
        input_cursor: usize,
        input_selection: TextSelection,
        cursor_visible: bool,
        is_empty: bool,
        mention_highlights: &[TextHighlightRange],
        pasted_text_pills: &[TextInlinePillRange],
        placeholder_text: Rgba,
        profile_icon_name: Option<&str>,
        profile_active_pending: bool,
        status: AgentChatThreadStatus,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let menu_def = crate::designs::current_main_menu_theme().def();
        let input = Self::render_composer_input_shell(
            input_text,
            input_cursor,
            input_selection,
            cursor_visible,
            is_empty,
            mention_highlights,
            pasted_text_pills,
            placeholder_text,
            profile_icon_name,
            profile_active_pending,
            status,
            weak_view,
            theme,
            false,
        );
        crate::components::main_view_chrome::render_main_view_header(
            crate::components::main_view_chrome::MainViewHeaderChrome {
                context: Some(
                    crate::components::main_view_chrome::render_main_view_context_zone_inert(
                        theme, menu_def, None, None,
                    ),
                ),
                input,
                padding_x: menu_def.shell.header_padding_x,
                padding_y: menu_def.shell.header_padding_y,
                gap: menu_def.shell.header_gap,
            },
        )
    }

    pub(crate) fn focused_text_mini_sizing_count(&self, cx: &App) -> Option<usize> {
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini || self.focused_text.is_none() {
            return None;
        }

        let thread = self.live_thread().read(cx);
        let scope_extra = if self.scope_visible { 1 } else { 0 };
        let has_variations = !self.focused_text_variations.is_empty();
        const FOCUSED_TEXT_MINI_SIZE_INPUT_ONLY: usize = 0;
        const FOCUSED_TEXT_MINI_SIZE_RESULT: usize = 2;
        const FOCUSED_TEXT_MINI_SIZE_VARIATIONS: usize = 5;
        let result_size = if has_variations {
            FOCUSED_TEXT_MINI_SIZE_VARIATIONS
        } else {
            FOCUSED_TEXT_MINI_SIZE_RESULT
        };
        match self.focused_text_mini_phase_for_thread(thread)? {
            FocusedTextMiniPhase::InputOnly => {
                Some(FOCUSED_TEXT_MINI_SIZE_INPUT_ONLY + scope_extra)
            }
            FocusedTextMiniPhase::Loading if has_variations => Some(result_size + scope_extra),
            FocusedTextMiniPhase::Loading => Some(FOCUSED_TEXT_MINI_SIZE_INPUT_ONLY + scope_extra),
            FocusedTextMiniPhase::Streaming => Some(result_size + scope_extra),
            FocusedTextMiniPhase::Result | FocusedTextMiniPhase::Error => {
                Some(result_size + scope_extra)
            }
        }
    }

    fn resize_focused_text_mini_for_scope_change(&self, cx: &App) {
        if let Some(item_count) = self.focused_text_mini_sizing_count(cx) {
            crate::window_resize::resize_to_view_sync(
                crate::window_resize::ViewType::FocusedTextMini,
                item_count,
            );
        }
    }

    pub(crate) fn resize_focused_text_mini_for_scope_change_public(&self, cx: &App) {
        self.resize_focused_text_mini_for_scope_change(cx);
    }

    fn normalize_focused_text_scope_input(value: &str) -> String {
        value
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', " ")
    }

    pub(crate) fn normalize_focused_text_scope_input_public(value: &str) -> String {
        Self::normalize_focused_text_scope_input(value)
    }

    pub(crate) fn handle_protocol_escape(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.is_focused_text_mini() || self.focused_text_originated_from_quick_prompt() {
            let (phase, input_has_text) = {
                let thread = self.live_thread().read(cx);
                (
                    self.focused_text_mini_phase_for_thread(thread),
                    !thread.input.text().is_empty() || !self.scope_input.is_empty(),
                )
            };

            let has_editor = self.focused_text_editing_variation.is_some();
            if has_editor {
                self.exit_focused_text_variation_editor(cx);
                return;
            }

            match phase {
                Some(FocusedTextMiniPhase::InputOnly) if input_has_text => {
                    self.scope_input.clear();
                    self.scope_visible = false;
                    self.scope_focused = false;
                    self.live_thread().update(cx, |thread, cx| {
                        thread.input.clear();
                        cx.notify();
                    });
                    self.resize_focused_text_mini_for_scope_change(&*cx);
                }
                Some(FocusedTextMiniPhase::InputOnly) => {
                    self.trigger_close_window_requested(window, cx);
                }
                Some(FocusedTextMiniPhase::Loading) => {
                    let _ = self.cancel_streaming_from_escape(cx);
                    self.scope_input.clear();
                    self.scope_visible = false;
                    self.scope_focused = false;
                    self.live_thread().update(cx, |thread, cx| {
                        thread.input.clear();
                        cx.notify();
                    });
                    self.resize_focused_text_mini_for_scope_change(&*cx);
                }
                Some(FocusedTextMiniPhase::Streaming) => {
                    let _ = self.cancel_streaming_from_escape(cx);
                }
                Some(FocusedTextMiniPhase::Result | FocusedTextMiniPhase::Error) => {
                    self.trigger_close_window_requested(window, cx);
                }
                None => {
                    let _ = self.cancel_streaming_from_escape(cx);
                    self.trigger_close_window_requested(window, cx);
                }
            }
        } else if !self.cancel_streaming_from_escape(cx) {
            self.trigger_close_requested(window, cx);
        }
    }

    fn normalize_focused_text_variation_editor_input(value: &str) -> String {
        value.replace("\r\n", "\n").replace('\r', "\n")
    }

    fn edit_focused_text_variation_text(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut String),
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(variation) = self.focused_text_variations.get_mut(index) else {
            self.focused_text_editing_variation = None;
            cx.notify();
            return false;
        };
        edit(&mut variation.text);
        variation.status = FocusedTextVariationStatus::Complete;
        variation.error = None;
        self.focused_text_selected_variation = Some(index);
        self.cursor_visible = true;
        cx.notify();
        true
    }

    pub(crate) fn enter_focused_text_variation_editor(&mut self, cx: &mut Context<Self>) -> bool {
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini
            || self.focused_text.is_none()
            || self.scope_focused
            || self.composer_picker_session.is_some()
        {
            return false;
        }
        let Some(index) = self.focused_text_selected_variation else {
            return false;
        };
        if index >= self.focused_text_variations.len() {
            self.focused_text_selected_variation = None;
            self.focused_text_editing_variation = None;
            cx.notify();
            return false;
        }
        self.focused_text_editing_variation = Some(index);
        self.scope_focused = false;
        self.cursor_visible = true;
        tracing::info!(
            target: "script_kit::focused_text",
            event = "focused_text_variation_editor_opened",
            index,
            angle = self.focused_text_variations[index].angle.id(),
            text_len = self.focused_text_variations[index].text.chars().count(),
        );
        cx.notify();
        true
    }

    pub(crate) fn exit_focused_text_variation_editor(&mut self, cx: &mut Context<Self>) -> bool {
        if self.focused_text_editing_variation.take().is_some() {
            self.cursor_visible = true;
            cx.notify();
            true
        } else {
            false
        }
    }

    fn handle_focused_text_variation_editor_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.focused_text_editing_variation else {
            return false;
        };
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini
            || self.focused_text.is_none()
            || index >= self.focused_text_variations.len()
        {
            self.focused_text_editing_variation = None;
            cx.notify();
            return false;
        }

        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;

        if crate::ui_foundation::is_key_escape(key) {
            self.exit_focused_text_variation_editor(cx);
            return true;
        }

        if crate::ui_foundation::is_key_enter(key) && modifiers.platform && !modifiers.shift {
            self.focused_text_selected_variation = Some(index);
            let receipt = self.apply_focused_text_output(
                crate::ai::focused_text::FocusedTextApplyAction::Replace,
                cx,
            );
            if receipt.success {
                self.focused_text_editing_variation = None;
                self.cursor_visible = true;
                self.trigger_close_window_requested(window, cx);
            }
            cx.notify();
            return true;
        }

        if crate::ui_foundation::is_key_enter(key)
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
        {
            return self.edit_focused_text_variation_text(index, |text| text.push('\n'), cx);
        }

        if modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && key.eq_ignore_ascii_case("v")
        {
            if let Some(clipboard) = cx.read_from_clipboard() {
                if let Some(text) = clipboard.text() {
                    let normalized = Self::normalize_focused_text_variation_editor_input(&text);
                    if !normalized.is_empty() {
                        let _ = self.edit_focused_text_variation_text(
                            index,
                            |current| current.push_str(&normalized),
                            cx,
                        );
                    }
                }
            }
            return true;
        }

        if crate::ui_foundation::is_key_backspace(key) {
            return self.edit_focused_text_variation_text(
                index,
                |text| {
                    text.pop();
                },
                cx,
            );
        }

        if crate::ui_foundation::is_key_delete(key)
            || crate::ui_foundation::is_key_tab(key)
            || crate::ui_foundation::is_key_left(key)
            || crate::ui_foundation::is_key_right(key)
            || crate::ui_foundation::is_key_up(key)
            || crate::ui_foundation::is_key_down(key)
            || key.eq_ignore_ascii_case("home")
            || key.eq_ignore_ascii_case("end")
            || key.eq_ignore_ascii_case("pageup")
            || key.eq_ignore_ascii_case("pagedown")
        {
            return true;
        }

        if modifiers.platform || modifiers.control || modifiers.alt {
            return false;
        }

        if let Some(ch) = event.keystroke.key_char.as_deref() {
            let normalized = Self::normalize_focused_text_variation_editor_input(ch);
            if !normalized.is_empty() {
                return self.edit_focused_text_variation_text(
                    index,
                    |text| text.push_str(&normalized),
                    cx,
                );
            }
        }

        false
    }

    fn handle_focused_text_scope_tab(&mut self, has_shift: bool, cx: &mut Context<Self>) -> bool {
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini || self.focused_text.is_none() {
            return false;
        }
        let input_locked = {
            let thread = self.live_thread().read(cx);
            self.focused_text_input_locked_for_thread(thread)
        };
        if input_locked {
            return false;
        }
        if has_shift {
            if self.scope_focused {
                self.scope_focused = false;
                self.cursor_visible = true;
                cx.notify();
                return true;
            }
            return false;
        }
        let was_visible = self.scope_visible;
        self.scope_visible = true;
        self.scope_focused = true;
        self.cursor_visible = true;
        if !was_visible {
            self.resize_focused_text_mini_for_scope_change(&*cx);
        }
        cx.notify();
        true
    }

    fn handle_focused_text_scope_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.ui_variant != AgentChatUiVariant::FocusedTextMini
            || self.focused_text.is_none()
            || !self.scope_focused
        {
            return false;
        }
        let input_locked = {
            let thread = self.live_thread().read(cx);
            self.focused_text_input_locked_for_thread(thread)
        };
        if input_locked {
            return false;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        if crate::ui_foundation::is_key_escape(key) {
            return false;
        }
        if crate::ui_foundation::is_key_enter(key) && !modifiers.platform && !modifiers.shift {
            if let Err(error) = self.submit_focused_text_from_enter(cx) {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "focused_text_submit_failed",
                    error = %error,
                );
            }
            return true;
        }
        if modifiers.platform && key.eq_ignore_ascii_case("v") {
            if let Some(clipboard) = cx.read_from_clipboard() {
                if let Some(text) = clipboard.text() {
                    let normalized = Self::normalize_focused_text_scope_input(&text);
                    if !normalized.is_empty() {
                        self.scope_input.push_str(&normalized);
                        cx.notify();
                    }
                }
            }
            return true;
        }
        if crate::ui_foundation::is_key_backspace(key) {
            self.scope_input.pop();
            cx.notify();
            return true;
        }
        if crate::ui_foundation::is_key_delete(key) {
            return true;
        }
        if crate::ui_foundation::is_key_left(key)
            || crate::ui_foundation::is_key_right(key)
            || crate::ui_foundation::is_key_up(key)
            || crate::ui_foundation::is_key_down(key)
            || key.eq_ignore_ascii_case("home")
            || key.eq_ignore_ascii_case("end")
            || key.eq_ignore_ascii_case("pageup")
            || key.eq_ignore_ascii_case("pagedown")
        {
            return true;
        }
        if modifiers.platform || modifiers.control {
            return false;
        }
        if let Some(ch) = event.keystroke.key_char.as_deref() {
            if !ch.is_empty() {
                self.scope_input
                    .push_str(&Self::normalize_focused_text_scope_input(ch));
                cx.notify();
                return true;
            }
        }
        false
    }

    fn focused_text_context_status_label(state: &FocusedTextAgentChatState) -> String {
        match state.context_status {
            FocusedTextContextStatus::Captured => {
                format!("{}w", Self::focused_text_compact_count(state.word_count))
            }
            FocusedTextContextStatus::CaptureFailed { .. } => "redacted".to_string(),
        }
    }

    fn render_focused_text_context_status_badge(
        state: &FocusedTextAgentChatState,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let captured = matches!(state.context_status, FocusedTextContextStatus::Captured);
        div()
            .id("focused-text-context-status")
            .flex_none()
            .h(px(22.0))
            .px(px(6.0))
            .rounded(px(6.0))
            .bg(rgba((theme.colors.text.primary << 8) | 0x08))
            .border_1()
            .border_color(rgba((theme.colors.text.primary << 8) | 0x14))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .line_height(px(14.0))
            .text_color(if captured {
                rgb(theme.colors.text.muted)
            } else {
                rgb(theme.colors.ui.error)
            })
            .child(Self::focused_text_context_status_label(state))
            .into_any_element()
    }

    fn render_focused_text_capture_error(
        state: &FocusedTextAgentChatState,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let message = state
            .context_status
            .user_message()
            .unwrap_or("Unable to grab text. Select text and try again.");
        let show_open_settings = state.context_status.offers_open_settings();

        div()
            .id("focused-text-capture-error")
            .w_full()
            .flex_none()
            .px(px(crate::panel::HEADER_PADDING_X))
            .py(px(10.0))
            .border_b_1()
            .border_color(rgba((theme.colors.text.primary << 8) | 0x14))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(theme.colors.ui.error))
                    .child(message),
            )
            .when(show_open_settings, |row| {
                row.child(
                    div()
                        .id("focused-text-open-accessibility-settings")
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .py(px(4.0))
                        .rounded(px(6.0))
                        .bg(rgba((theme.colors.text.primary << 8) | 0x10))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme.colors.text.primary))
                        .cursor_pointer()
                        .child("Open Settings")
                        .on_mouse_down(gpui::MouseButton::Left, |_event, _window, _cx| {
                            let _ = crate::platform::accessibility::permissions::open_accessibility_settings();
                        }),
                )
            })
            .into_any_element()
    }

    fn render_focused_text_capture_truncation_warning(
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        div()
            .id("focused-text-capture-truncation-warning")
            .w_full()
            .flex_none()
            .px(px(crate::panel::HEADER_PADDING_X))
            .py(px(6.0))
            .border_b_1()
            .border_color(rgba((theme.colors.text.primary << 8) | 0x14))
            .text_xs()
            .text_color(rgb(theme.colors.text.muted))
            .child(format!(
                "Captured text exceeded {} characters and was truncated.",
                crate::platform::accessibility::focused_text::MAX_FOCUSED_TEXT_CAPTURE_CHARS
            ))
            .into_any_element()
    }

    fn render_focused_text_app_icon_badge(
        state: &FocusedTextAgentChatState,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let content = if let Some(icon) = state.app_bundle_id.as_deref().and_then(|bundle_id| {
            let bundle_id = bundle_id.trim();
            if bundle_id.is_empty() {
                None
            } else {
                crate::app_launcher::cached_app_icon_for_bundle(bundle_id)
            }
        }) {
            crate::icons::render_image(icon, 16.0, 1.0)
        } else {
            use gpui_component::IconNamed;
            gpui::svg()
                .path(gpui_component::IconName::AppWindow.path())
                .size(px(14.0))
                .text_color(rgb(theme.colors.text.muted))
                .into_any_element()
        };

        div()
            .id("focused-text-context-badge")
            .flex_none()
            .size(px(24.0))
            .rounded(px(6.0))
            .bg(rgba((theme.colors.text.primary << 8) | 0x08))
            .border_1()
            .border_color(rgba((theme.colors.text.primary << 8) | 0x14))
            .flex()
            .items_center()
            .justify_center()
            .child(content)
            .into_any_element()
    }

    fn focused_text_variation_area_height(count: usize, fallback_height: f32) -> f32 {
        if count == 0 {
            return fallback_height;
        }
        let cards_height = (count as f32 * Self::FOCUSED_TEXT_VARIATION_CARD_MIN_HEIGHT)
            + (count.saturating_sub(1) as f32 * Self::FOCUSED_TEXT_VARIATION_CARD_GAP)
            + (Self::FOCUSED_TEXT_VARIATION_AREA_PADDING_Y * 2.0);
        cards_height
            .max(fallback_height)
            .min(Self::FOCUSED_TEXT_VARIATION_AREA_MAX_HEIGHT)
    }

    fn render_focused_text_variation_card(
        variation: FocusedTextVariationSnapshot,
        editing: bool,
        cursor_visible: bool,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let selected = variation.selected;
        let streaming = matches!(variation.status, FocusedTextVariationStatus::Streaming);
        let error = matches!(variation.status, FocusedTextVariationStatus::Error);
        let status_label = if editing {
            "Editing"
        } else {
            match variation.status {
                FocusedTextVariationStatus::Idle => "Idle",
                FocusedTextVariationStatus::Streaming => "Streaming",
                FocusedTextVariationStatus::Complete => "Ready",
                FocusedTextVariationStatus::Error => "Error",
            }
        };
        let body = if error {
            variation
                .error
                .clone()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("Error: {value}"))
                .unwrap_or_else(|| "This variation failed.".to_string())
        } else if variation.text.trim().is_empty() {
            match variation.status {
                FocusedTextVariationStatus::Idle => "Waiting to start\u{2026}".to_string(),
                FocusedTextVariationStatus::Streaming => "Thinking\u{2026}".to_string(),
                FocusedTextVariationStatus::Complete => "No text returned.".to_string(),
                FocusedTextVariationStatus::Error => "This variation failed.".to_string(),
            }
        } else {
            variation.text.clone()
        };
        let dot_color = match variation.status {
            FocusedTextVariationStatus::Streaming => rgb(theme.colors.accent.selected),
            FocusedTextVariationStatus::Complete => {
                rgba((theme.colors.accent.selected << 8) | 0xB8)
            }
            FocusedTextVariationStatus::Error => rgb(theme.colors.ui.error),
            FocusedTextVariationStatus::Idle => rgba((theme.colors.text.primary << 8) | 0x32),
        };
        let dot = div().size(px(7.0)).rounded(px(999.0)).bg(dot_color);
        let dot = if streaming {
            dot.with_animation(
                "focused-text-variation-dot-pulse",
                Animation::new(Duration::from_millis(1200)).repeat(),
                |style, delta| {
                    let sine = (delta * std::f32::consts::PI * 2.0).sin();
                    style.opacity(0.65 + (0.35 * ((sine + 1.0) / 2.0)))
                },
            )
            .into_any_element()
        } else {
            dot.into_any_element()
        };
        let variation_index = variation.index;
        let editor_cursor = variation.text.chars().count();
        let editor_selection = TextSelection::caret(editor_cursor);
        let select_view = weak_view.clone();
        div()
            .id(SharedString::from(format!(
                "focused-text-variation-card-{}",
                variation.index
            )))
            .w_full()
            .min_h(px(Self::FOCUSED_TEXT_VARIATION_CARD_MIN_HEIGHT))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(if editing {
                rgba((theme.colors.accent.selected << 8) | 0xD0)
            } else if selected {
                rgba((theme.colors.accent.selected << 8) | 0xA8)
            } else {
                rgba((theme.colors.ui.border << 8) | 0x36)
            })
            .bg(if editing {
                rgba((theme.colors.accent.selected << 8) | 0x10)
            } else if selected {
                rgba((theme.colors.accent.selected << 8) | 0x14)
            } else {
                rgba((theme.colors.text.primary << 8) | 0x05)
            })
            .cursor_pointer()
            .hover(|d| d.bg(rgba((theme.colors.text.primary << 8) | 0x08)))
            .on_click(move |_event, window, cx| {
                if let Some(entity) = select_view.upgrade() {
                    entity.update(cx, |chat, cx| {
                        window.focus(&chat.focus_handle, cx);
                        let _ = chat.select_focused_text_variation(variation_index, cx);
                    });
                }
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .child(dot)
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if selected {
                                        rgb(theme.colors.accent.selected)
                                    } else {
                                        rgb(theme.colors.text.primary)
                                    })
                                    .child(variation.label),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(if error {
                                rgb(theme.colors.ui.error)
                            } else {
                                rgb(theme.colors.text.muted)
                            })
                            .child(status_label),
                    ),
            )
            .child(if editing {
                div()
                    .w_full()
                    .pt(px(6.0))
                    .child(
                        div()
                            .w_full()
                            .min_h(px(54.0))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(rgba((theme.colors.accent.selected << 8) | 0x38))
                            .bg(rgba((theme.colors.text.primary << 8) | 0x04))
                            .px(px(8.0))
                            .py(px(6.0))
                            .child(Self::render_composer_input_text(
                                &variation.text,
                                editor_cursor,
                                editor_selection,
                                cursor_visible,
                                "Edit variation\u{2026}",
                                true,
                                &[],
                                &[],
                                rgba((theme.colors.text.primary << 8) | 0x62),
                                theme,
                                None,
                            )),
                    )
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .pt(px(6.0))
                    .text_sm()
                    .line_height(px(18.0))
                    .text_color(if error {
                        rgb(theme.colors.ui.error)
                    } else {
                        rgb(theme.colors.text.primary)
                    })
                    .opacity(if variation.text.trim().is_empty() && !error {
                        0.62
                    } else {
                        0.92
                    })
                    .child(body)
                    .into_any_element()
            })
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn render_focused_text_mini(
        &self,
        active_pending: bool,
        show_transcript: bool,
        profile_icon_name: Option<&str>,
        weak_view: WeakEntity<AgentChatView>,
        transcript: Option<gpui::AnyElement>,
        variations: Vec<FocusedTextVariationSnapshot>,
        input_text: &str,
        input_cursor: usize,
        input_selection: TextSelection,
        cursor_visible: bool,
        input_locked: bool,
        placeholder_text: Rgba,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let chrome = AppChromeColors::from_theme(theme);
        let input_height = crate::window_resize::focused_text_mini_input_height();
        let mini_result_height = crate::window_resize::focused_text_mini_result_height();
        let fallback_preview_height = crate::window_resize::focused_text_mini_preview_height();
        let has_variation_cards = !variations.is_empty();
        let editing_variation = self.focused_text_editing_variation;
        let show_result_area = has_variation_cards || show_transcript || transcript.is_some();
        let preview_height = if has_variation_cards {
            Self::focused_text_variation_area_height(variations.len(), fallback_preview_height)
        } else {
            fallback_preview_height
        };
        let scope_height = if self.scope_visible {
            input_height
        } else {
            0.0
        };
        let content_height = if has_variation_cards {
            input_height + scope_height + preview_height
        } else {
            mini_result_height + scope_height
        };
        let instruction_focus_view = weak_view.clone();

        let input_row = div()
            .id("focused-text-mini-input-row")
            .w_full()
            .h(px(input_height))
            .max_h(px(input_height))
            .flex_none()
            .overflow_hidden()
            .px(px(crate::panel::HEADER_PADDING_X))
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(show_result_area || self.scope_visible, |d| {
                d.border_b_1().border_color(rgba(chrome.divider_rgba))
            })
            .on_click(move |_event, window, cx| {
                if let Some(entity) = instruction_focus_view.upgrade() {
                    entity.update(cx, |chat, cx| {
                        window.focus(&chat.focus_handle, cx);
                        chat.scope_focused = false;
                        chat.cursor_visible = true;
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .id("focused-text-input")
                    .min_w_0()
                    .flex_1()
                    .when(input_locked, |d| d.opacity(0.55))
                    .when(self.scope_focused && !input_locked, |d| d.opacity(0.72))
                    .child(Self::render_composer_input_text(
                        input_text,
                        input_cursor,
                        input_selection,
                        if input_locked
                            || self.scope_focused
                            || self.focused_text_editing_variation.is_some()
                        {
                            false
                        } else {
                            cursor_visible
                        },
                        Self::FOCUSED_TEXT_MINI_PLACEHOLDER,
                        false,
                        &[],
                        &[],
                        placeholder_text,
                        theme,
                        Some(Self::FOCUSED_TEXT_MINI_INPUT_MAX_VISIBLE_HEIGHT),
                    )),
            )
            .when_some(self.focused_text.as_ref(), |d, state| {
                d.child(Self::render_focused_text_app_icon_badge(state, theme))
                    .child(Self::render_focused_text_context_status_badge(state, theme))
            })
            .child(Self::render_input_profile_icon(
                "focused-text-profile-icon",
                profile_icon_name,
                active_pending,
                weak_view.clone(),
                theme,
            ));

        let scope_row = if self.scope_visible {
            let scope_cursor = self.scope_input.chars().count();
            let scope_selection = TextSelection::caret(scope_cursor);
            let scope_focus_view = weak_view.clone();
            Some(
                div()
                    .id("focused-text-mini-scope-row")
                    .w_full()
                    .h(px(input_height))
                    .max_h(px(input_height))
                    .flex_none()
                    .overflow_hidden()
                    .px(px(crate::panel::HEADER_PADDING_X))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .when(show_result_area, |d| {
                        d.border_b_1().border_color(rgba(chrome.divider_rgba))
                    })
                    .on_click(move |_event, window, cx| {
                        if let Some(entity) = scope_focus_view.upgrade() {
                            entity.update(cx, |chat, cx| {
                                window.focus(&chat.focus_handle, cx);
                                chat.scope_visible = true;
                                chat.scope_focused = true;
                                chat.cursor_visible = true;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .flex_none()
                            .w(px(44.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme.colors.text.muted))
                            .child("Scope"),
                    )
                    .child(
                        div()
                            .id("focused-text-scope-input")
                            .min_w_0()
                            .flex_1()
                            .when(input_locked, |d| d.opacity(0.55))
                            .child(Self::render_composer_input_text(
                                &self.scope_input,
                                scope_cursor,
                                scope_selection,
                                if input_locked {
                                    false
                                } else {
                                    cursor_visible && self.scope_focused
                                },
                                "Scope\u{2026}",
                                false,
                                &[],
                                &[],
                                placeholder_text,
                                theme,
                                Some(Self::FOCUSED_TEXT_MINI_INPUT_MAX_VISIBLE_HEIGHT),
                            )),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };

        let mut content = div()
            .id("focused-text-mini-content")
            .w_full()
            .h(px(content_height))
            .max_h(px(content_height))
            .flex_none()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(input_row);

        if let Some(scope_row) = scope_row {
            content = content.child(scope_row);
        }

        if let Some(state) = self.focused_text.as_ref() {
            match state.context_status {
                FocusedTextContextStatus::CaptureFailed { .. } => {
                    content = content.child(Self::render_focused_text_capture_error(state, theme));
                }
                FocusedTextContextStatus::Captured if state.capture_truncated => {
                    content =
                        content.child(Self::render_focused_text_capture_truncation_warning(theme));
                }
                FocusedTextContextStatus::Captured => {}
            }
        }

        if has_variation_cards {
            content = content.child(
                div()
                    .id("focused-text-variations-preview")
                    .w_full()
                    .h(px(preview_height))
                    .max_h(px(Self::FOCUSED_TEXT_VARIATION_AREA_MAX_HEIGHT))
                    .flex_none()
                    .border_b_1()
                    .border_color(rgba(chrome.divider_rgba))
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .id("focused-text-variation-cards")
                            .w_full()
                            .px(px(8.0))
                            .py(px(Self::FOCUSED_TEXT_VARIATION_AREA_PADDING_Y))
                            .flex()
                            .flex_col()
                            .gap(px(Self::FOCUSED_TEXT_VARIATION_CARD_GAP))
                            .children(variations.into_iter().map(|variation| {
                                let editing = editing_variation == Some(variation.index);
                                Self::render_focused_text_variation_card(
                                    variation,
                                    editing,
                                    cursor_visible && editing,
                                    weak_view.clone(),
                                    theme,
                                )
                            })),
                    )
                    .with_animation(
                        "focused-text-mini-variations-enter",
                        Animation::new(Duration::from_millis(160)),
                        |style, delta| style.opacity(delta),
                    ),
            );
        } else if let Some(transcript) = transcript {
            content = content.child(
                div()
                    .id("focused-text-preview")
                    .w_full()
                    .h(px(preview_height))
                    .max_h(px(preview_height))
                    .flex_none()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(rgba(chrome.divider_rgba))
                    .child(div().size_full().overflow_hidden().child(transcript))
                    .with_animation(
                        "focused-text-mini-preview-enter",
                        Animation::new(Duration::from_millis(160)),
                        |style, delta| style.opacity(delta),
                    ),
            );
        }

        let root = div()
            .id("focused-text-mini-root")
            .size_full()
            .when_some(
                crate::ui_foundation::get_vibrancy_background(theme),
                |d, bg| d.bg(bg),
            )
            .border_1()
            .border_color(rgba(chrome.border_rgba))
            .rounded(px(10.0))
            .overflow_hidden()
            .child(content);

        root.into_any_element()
    }

    /// Render context chips below the composer input, but only for parts
    /// that are NOT already represented by an inline `@mention` token.
    ///
    /// Accent left-bar design: a 2px gold bar on the left edge with
    /// a ghost-opacity chip containing the label and a × dismiss button.
    #[allow(dead_code)]
    fn render_pending_context_chips(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        use crate::ai::context_mentions::visible_context_chip_indices;

        let (parts, input_text) = {
            let thread = self.live_thread().read(cx);
            (
                thread.pending_context_parts().to_vec(),
                thread.input.text().to_string(),
            )
        };

        if parts.is_empty() {
            return div()
                .id("agent_chat-pending-context-chips-empty")
                .into_any_element();
        }

        let chip_indices = visible_context_chip_indices(&input_text, &parts);
        let chip_parts: Vec<(usize, &AiContextPart)> = chip_indices
            .into_iter()
            .filter_map(|ix| parts.get(ix).map(|part| (ix, part)))
            .collect();

        if chip_parts.is_empty() {
            return div()
                .id("agent_chat-pending-context-chips-empty")
                .into_any_element();
        }

        let theme = theme::get_cached_theme();
        let accent = theme.colors.accent.selected;
        let border = theme.colors.ui.border;
        let dimmed = theme.colors.text.dimmed;
        let muted_text = theme.colors.text.muted;
        let primary_text = theme.colors.text.primary;

        let mut container = div()
            .id("agent_chat-pending-context-chips")
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(6.0))
            .px(px(12.0))
            .pb(px(6.0));

        for (chip_idx, &(remove_idx, part)) in chip_parts.iter().enumerate() {
            let label = SharedString::from(part.label().to_string());
            let remove_id = ElementId::Name(SharedString::from(format!(
                "agent_chat-ctx-remove-{chip_idx}"
            )));

            let chip = div()
                .id(ElementId::Name(SharedString::from(format!(
                    "agent_chat-ctx-chip-{chip_idx}"
                ))))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.0))
                // Gold left accent bar
                .child(
                    div()
                        .w(px(2.0))
                        .h(px(14.0))
                        .rounded(px(1.0))
                        .bg(rgb(accent)),
                )
                // Label + dismiss in ghost container
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(4.0))
                        .py(px(2.0))
                        .rounded(px(3.0))
                        .bg(rgba((border << 8) | 0x0A))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(dimmed))
                                .overflow_hidden()
                                .text_ellipsis()
                                .max_w(px(280.0))
                                .child(label),
                        )
                        .child(
                            div()
                                .id(remove_id)
                                .cursor_pointer()
                                .text_xs()
                                .text_color(rgba((muted_text << 8) | 0x60))
                                .px(px(4.0))
                                .py(px(1.0))
                                .rounded(px(999.0))
                                .hover(|el| {
                                    el.text_color(rgb(primary_text))
                                        .bg(rgba((border << 8) | 0x18))
                                        .rounded(px(999.0))
                                })
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.live_thread().update(cx, |thread, cx| {
                                        thread.remove_context_part(remove_idx, cx);
                                    });
                                }))
                                .child("\u{00d7}"),
                        ),
                );

            container = container.child(chip);
        }

        container.into_any_element()
    }

    fn retry_last_user_turn(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self
            .live_thread()
            .update(cx, |thread, cx| thread.retry_last_user_turn(cx))
        {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_retry_failed",
                error = %error,
            );
        }
    }

    fn render_active_callout(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (callout, selected_model_id) = {
            let thread = self.live_thread().read(cx);
            (
                thread.active_callout().cloned(),
                thread.selected_model_id().map(str::to_string),
            )
        };
        let Some(callout) = callout else {
            return div().id("agent_chat-callout-empty").into_any_element();
        };

        let theme = theme::get_cached_theme();
        let colors = PromptColors::from_theme(&theme);
        let button_colors = crate::components::ButtonColors::from_theme(&theme);
        let accent = match callout.severity {
            AgentChatCalloutSeverity::Error => theme.colors.ui.error,
        };
        let detail = callout
            .detail
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "The provider could not complete this turn.".into());
        let raw_detail = callout.raw_detail.clone();
        let show_auth_recovery = callout.auth_recovery.is_some();
        let auth_provider = show_auth_recovery
            .then(|| {
                crate::ai::agent_chat::pi::auth_recovery::resolve_auth_recovery_provider(
                    selected_model_id.as_deref(),
                    raw_detail.as_ref().map(|detail| detail.as_ref()),
                )
            })
            .flatten();
        let pi_binary = crate::ai::agent_chat::pi::binary::default_pi_binary();
        let weak_view = cx.entity().downgrade();

        let mut actions = div().flex().flex_wrap().items_center().gap(px(6.0));
        if let (Some(provider), Some(binary)) = (auth_provider.clone(), pi_binary.clone()) {
            actions = actions.child(
                crate::components::Button::new("Sign in again", button_colors)
                    .id("agent_chat-callout-sign-in")
                    .variant(crate::components::ButtonVariant::Primary)
                    .on_click(Box::new(move |_event, _window, _cx| {
                        if let Err(error) = crate::ai::agent_chat::pi::auth_recovery::launch_pi_auth_recovery(
                            binary.clone(),
                            provider.clone(),
                            crate::ai::agent_chat::pi::auth_recovery::PiAuthRecoveryAction::SignInAgain,
                        ) {
                            tracing::warn!(
                                target: "script_kit::agent_chat",
                                event = "agent_chat_auth_recovery_launch_failed",
                                action = "sign_in_again",
                                error = %error,
                            );
                        }
                    })),
            );
        }
        if let (Some(provider), Some(binary)) = (auth_provider, pi_binary) {
            actions = actions.child(
                crate::components::Button::new("Switch account", button_colors)
                    .id("agent_chat-callout-switch-account")
                    .variant(crate::components::ButtonVariant::Ghost)
                    .on_click(Box::new(move |_event, _window, _cx| {
                        if let Err(error) = crate::ai::agent_chat::pi::auth_recovery::launch_pi_auth_recovery(
                            binary.clone(),
                            provider.clone(),
                            crate::ai::agent_chat::pi::auth_recovery::PiAuthRecoveryAction::SwitchAccount,
                        ) {
                            tracing::warn!(
                                target: "script_kit::agent_chat",
                                event = "agent_chat_auth_recovery_launch_failed",
                                action = "switch_account",
                                error = %error,
                            );
                        }
                    })),
            );
        }
        if callout.can_retry {
            let retry_view = weak_view.clone();
            actions = actions.child(
                crate::components::Button::new("Retry", button_colors)
                    .id("agent_chat-callout-retry")
                    .variant(crate::components::ButtonVariant::Ghost)
                    .on_click(Box::new(move |_event, _window, cx| {
                        if let Some(view) = retry_view.upgrade() {
                            view.update(cx, |this, cx| this.retry_last_user_turn(cx));
                        }
                    })),
            );
        }
        if raw_detail.is_some() {
            let copy_view = weak_view.clone();
            actions = actions.child(
                crate::components::Button::new("Copy Error", button_colors)
                    .id("agent_chat-callout-copy-error")
                    .variant(crate::components::ButtonVariant::Ghost)
                    .on_click(Box::new(move |_event, _window, cx| {
                        if let Some(view) = copy_view.upgrade() {
                            if let Some(text) = view
                                .read(cx)
                                .live_thread()
                                .read(cx)
                                .active_callout()
                                .and_then(|active| active.raw_detail.as_ref())
                                .map(ToString::to_string)
                            {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                            }
                        }
                    })),
            );
        }

        div()
            .id("agent_chat-callout-stack")
            .w_full()
            .px(px(12.0))
            .pb(px(6.0))
            .child(
                div()
                    .id("agent_chat-active-callout")
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .bg(rgba((theme.colors.ui.border << 8) | 0x10))
                    .border_1()
                    .border_color(rgba((theme.colors.ui.border << 8) | 0x28))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().text_xs().text_color(rgb(accent)).child("⚠"))
                            .child(
                                div()
                                    .flex_grow()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(colors.text_primary))
                                    .child(callout.title.clone()),
                            )
                            .child(
                                div()
                                    .id("agent_chat-callout-dismiss")
                                    .cursor_pointer()
                                    .text_xs()
                                    .px(px(5.0))
                                    .py(px(1.0))
                                    .rounded(px(999.0))
                                    .text_color(rgba((theme.colors.text.muted << 8) | 0x70))
                                    .hover(|el| el.bg(rgba((theme.colors.ui.border << 8) | 0x18)))
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.live_thread().update(cx, |thread, cx| {
                                            thread.dismiss_active_callout(cx)
                                        });
                                    }))
                                    .child("×"),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(rgb(colors.text_secondary))
                            .child(detail),
                    )
                    .child(actions),
            )
            .into_any_element()
    }

    fn render_reserved_transient_lane(
        id: &'static str,
        height_px: f32,
        content: Option<gpui::AnyElement>,
    ) -> gpui::AnyElement {
        let height_px = agent_chat_transient_lane_height(height_px, content.is_some());
        div()
            .id(id)
            .w_full()
            .h(px(height_px))
            .overflow_hidden()
            .when_some(content, |d, content| d.child(content))
            .into_any_element()
    }

    fn message_queue_lane_active(&self, cx: &App) -> bool {
        !self.live_thread().read(cx).queued_messages().is_empty()
    }

    fn context_bootstrap_note_lane_active(&self, cx: &App) -> bool {
        self.live_thread()
            .read(cx)
            .context_bootstrap_note()
            .is_some_and(|note| !note.trim().is_empty())
    }

    fn render_message_queue_strip(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (queued, paused) = {
            let thread = self.live_thread().read(cx);
            (
                thread.queued_messages().iter().cloned().collect::<Vec<_>>(),
                thread.queue_paused(),
            )
        };

        if queued.is_empty() {
            return div()
                .id("agent_chat-message-queue-empty")
                .into_any_element();
        }

        let theme = theme::get_cached_theme();
        let border = theme.colors.ui.border;
        let accent = theme.colors.accent.selected;
        let expanded = self.message_queue_expanded;
        let count = queued.len();
        let mut container = div()
            .id("agent_chat-message-queue-strip")
            .w_full()
            .px(px(12.0))
            .pb(px(6.0));

        let header = div()
            .id("agent_chat-message-queue-header")
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(6.0))
            .bg(rgba((border << 8) | 0x10))
            .border_1()
            .border_color(rgba((border << 8) | 0x28))
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.message_queue_expanded = !this.message_queue_expanded;
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().text_xs().text_color(rgb(accent)).child("↑"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.colors.text.muted))
                            .child(if paused {
                                format!("{count} queued · paused")
                            } else {
                                format!("{count} queued")
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .opacity(0.45)
                    .child(if expanded { "Hide" } else { "Show" }),
            );

        container = container.child(header);

        if expanded {
            let mut list = div()
                .mt(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(rgba((border << 8) | 0x22))
                .bg(rgba((theme.colors.text.primary << 8) | 0x04));

            for (index, message) in queued.into_iter().enumerate() {
                let text = message.text.trim().replace('\n', " ");
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(8.0))
                        .py(px(5.0))
                        .child(
                            div()
                                .flex_grow()
                                .min_w(px(0.0))
                                .text_xs()
                                .text_color(rgb(theme.colors.text.dimmed))
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(text),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(SharedString::from(format!(
                                    "agent_chat-queue-remove-{index}"
                                ))))
                                .cursor_pointer()
                                .text_xs()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(px(999.0))
                                .text_color(rgba((theme.colors.text.muted << 8) | 0x70))
                                .hover(|d| d.bg(rgba((border << 8) | 0x18)))
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.live_thread().update(cx, |thread, cx| {
                                        thread.remove_queued_message(index, cx);
                                    });
                                }))
                                .child("×"),
                        ),
                );
            }

            list = list.child(
                div()
                    .id("agent_chat-message-queue-clear")
                    .px(px(8.0))
                    .py(px(5.0))
                    .text_xs()
                    .text_color(rgb(theme.colors.text.muted))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba((border << 8) | 0x12)))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.live_thread()
                            .update(cx, |thread, cx| thread.clear_queued_messages(cx));
                    }))
                    .child("Clear"),
            );
            container = container.child(list);
        }

        container.into_any_element()
    }

    fn render_context_bootstrap_note(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (state, note) = {
            let thread = self.live_thread().read(cx);
            (
                thread.context_bootstrap_state(),
                thread.context_bootstrap_note().map(|v| v.to_string()),
            )
        };

        let Some(note) = note.filter(|v| !v.trim().is_empty()) else {
            return div()
                .id("agent_chat-context-bootstrap-note-empty")
                .into_any_element();
        };

        let theme = theme::get_cached_theme();
        let accent = theme.colors.accent.selected;
        let border = theme.colors.ui.border;

        let (fg_color, bg, outline) = match state {
            AgentChatContextBootstrapState::Preparing => {
                (accent, (accent << 8) | 0x10, (accent << 8) | 0x24)
            }
            AgentChatContextBootstrapState::Ready => (
                theme.colors.text.muted,
                (border << 8) | 0x10,
                (border << 8) | 0x24,
            ),
            AgentChatContextBootstrapState::Failed => (
                theme.colors.text.primary,
                (border << 8) | 0x14,
                (border << 8) | 0x28,
            ),
        };

        div()
            .id("agent_chat-context-bootstrap-note")
            .px(px(12.0))
            .pb(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(6.0))
                    .bg(rgba(bg))
                    .border_1()
                    .border_color(rgba(outline))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(fg_color))
                            .child(SharedString::from(note)),
                    ),
            )
            .into_any_element()
    }

    fn render_permission_section(title: &'static str, text: String) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();

        div()
            .pt(px(8.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .opacity(0.48)
                    .child(title),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .max_h(px(120.0))
                    .overflow_y_hidden()
                    .border_l_2()
                    .border_color(rgba((theme.colors.ui.border << 8) | 0x18))
                    .bg(rgba((theme.colors.text.primary << 8) | 0x04))
                    .pl(px(10.0))
                    .pr(px(8.0))
                    .py(px(6.0))
                    .text_xs()
                    .opacity(0.76)
                    .child(text),
            )
            .into_any_element()
    }

    fn render_permission_header(preview: &AgentChatApprovalPreview) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();

        let (badge_bg, badge_border) = match preview.kind {
            AgentChatApprovalPreviewKind::Read => (
                rgba((theme.colors.text.primary << 8) | 0x10),
                rgba((theme.colors.ui.border << 8) | 0x30),
            ),
            AgentChatApprovalPreviewKind::Write => (
                rgba((theme.colors.accent.selected << 8) | 0x16),
                rgba((theme.colors.accent.selected << 8) | 0x38),
            ),
            AgentChatApprovalPreviewKind::Execute => (rgba(0xF59E0B18), rgba(0xF59E0B50)),
            AgentChatApprovalPreviewKind::Generic => (
                rgba((theme.colors.text.primary << 8) | 0x08),
                rgba((theme.colors.ui.border << 8) | 0x24),
            ),
        };

        div()
            .pt(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .px(px(7.0))
                            .py(px(3.0))
                            .rounded(px(999.0))
                            .bg(badge_bg)
                            .border_1()
                            .border_color(badge_border)
                            .text_xs()
                            .opacity(0.68)
                            .child(preview.kind.badge_label()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .opacity(0.92)
                            .child(preview.tool_title.clone()),
                    ),
            )
            .when_some(preview.subject.clone(), |d, subject| {
                d.child(div().pt(px(4.0)).text_sm().opacity(0.68).child(subject))
            })
            .into_any_element()
    }

    fn render_permission_option_row(
        option: &AgentChatApprovalOption,
        index: usize,
        is_selected: bool,
        view: WeakEntity<AgentChatView>,
    ) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();
        let option_id = option.option_id.clone();

        let (accent, bg, hover_bg, caption) = if option.is_reject() {
            (
                rgba(0xEF4444AA),
                if is_selected {
                    rgba(0xEF444418)
                } else {
                    rgba(0xEF444406)
                },
                rgba(0xEF444410),
                "Deny this request",
            )
        } else if option.is_persistent_allow() {
            (
                rgb(theme.colors.accent.selected),
                if is_selected {
                    rgba((theme.colors.accent.selected << 8) | 0x18)
                } else {
                    rgba((theme.colors.accent.selected << 8) | 0x06)
                },
                rgba((theme.colors.accent.selected << 8) | 0x10),
                "Remember this choice",
            )
        } else {
            (
                rgb(theme.colors.accent.selected),
                if is_selected {
                    rgba((theme.colors.accent.selected << 8) | 0x12)
                } else {
                    rgba((theme.colors.text.primary << 8) | 0x04)
                },
                rgba((theme.colors.text.primary << 8) | 0x08),
                "Approve once",
            )
        };

        div()
            .id(SharedString::from(format!("perm-opt-{index}")))
            .mt(px(4.0))
            .pl(px(10.0))
            .pr(px(6.0))
            .py(px(6.0))
            .border_l_2()
            .border_color(if is_selected {
                accent
            } else {
                rgba(0x00000000)
            })
            .cursor_pointer()
            .bg(bg)
            .hover(move |d| d.bg(hover_bg))
            .on_click(move |_event, _window, cx| {
                if let Some(entity) = view.upgrade() {
                    entity.update(cx, |this, cx| {
                        this.permission_index = index;
                        this.approve_permission(Some(option_id.clone()), cx);
                    });
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(option.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .opacity(0.34)
                            .child(format!("{}", index + 1)),
                    ),
            )
            .child(div().pt(px(2.0)).text_xs().opacity(0.42).child(caption))
            .into_any_element()
    }

    fn render_permission_inline_card(
        request: &AgentChatApprovalRequest,
        selected_index: usize,
        options_open: bool,
        view: WeakEntity<AgentChatView>,
    ) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();
        let preview = request.preview.clone();
        let selected_index = selected_index.min(request.options.len().saturating_sub(1));
        let show_options_button = request.options.len() > 2
            || request
                .options
                .iter()
                .any(|option| option.is_persistent_allow());
        let selected_option_label = request
            .options
            .get(selected_index)
            .map(|option| option.name.clone())
            .unwrap_or_else(|| "Options".to_string());
        let shortcut_hint = if show_options_button {
            "\u{2318}Y Allow \u{00b7} \u{2318}\u{2325}A Options \u{00b7} \u{2318}\u{2325}Z Deny"
        } else {
            "\u{2318}Y Allow \u{00b7} \u{2318}\u{2325}Z Deny \u{00b7} Esc Cancel"
        };

        let accent = preview
            .as_ref()
            .map(|preview| match preview.kind {
                AgentChatApprovalPreviewKind::Read => rgba((theme.colors.ui.border << 8) | 0x44),
                AgentChatApprovalPreviewKind::Write => rgb(theme.colors.accent.selected),
                AgentChatApprovalPreviewKind::Execute => rgba(0xF59E0BCC),
                AgentChatApprovalPreviewKind::Generic => rgba((theme.colors.ui.border << 8) | 0x32),
            })
            .unwrap_or_else(|| rgb(theme.colors.accent.selected));

        let allow_request = request.clone();
        let allow_view = view.clone();
        let deny_request = request.clone();
        let deny_view = view.clone();
        let options_request = request.clone();
        let options_view = view.clone();

        div()
            .id("agent_chat-inline-permission-card")
            .w_full()
            .mt(px(6.0))
            .ml(px(12.0))
            .pl(px(10.0))
            .pr(px(8.0))
            .py(px(8.0))
            .border_l_2()
            .border_color(accent)
            .bg(rgba((theme.colors.text.primary << 8) | 0x04))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .opacity(0.48)
                    .child(request.title.clone()),
            )
            .when_some(preview.clone(), |d, preview| {
                d.child(Self::render_permission_header(&preview))
                    .when_some(preview.summary, |d, summary| {
                        d.child(div().pt(px(6.0)).text_sm().opacity(0.72).child(summary))
                    })
                    .when_some(preview.input_preview, |d, input| {
                        d.child(Self::render_permission_section("Input", input))
                    })
                    .when_some(preview.output_preview, |d, output| {
                        d.child(Self::render_permission_section("Output", output))
                    })
            })
            .when(preview.is_none(), |d| {
                d.child(
                    div()
                        .pt(px(6.0))
                        .text_sm()
                        .opacity(0.72)
                        .child(request.body.clone()),
                )
            })
            .child(
                div()
                    .pt(px(8.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .id("agent_chat-inline-permission-allow")
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .cursor_pointer()
                                    .border_l_2()
                                    .border_color(rgb(theme.colors.accent.selected))
                                    .bg(rgba((theme.colors.accent.selected << 8) | 0x12))
                                    .hover(|d| {
                                        d.bg(rgba((theme.colors.accent.selected << 8) | 0x1C))
                                    })
                                    .on_click(move |_event, _window, cx| {
                                        if let Some(entity) = allow_view.upgrade() {
                                            entity.update(cx, |this, cx| {
                                                let _ = this.approve_preferred_allow_option(
                                                    &allow_request,
                                                    cx,
                                                );
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child("Allow"),
                                            )
                                            .child(
                                                div().text_xs().opacity(0.42).child("\u{2318}Y"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("agent_chat-inline-permission-deny")
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .cursor_pointer()
                                    .border_l_2()
                                    .border_color(rgba(0xEF4444AA))
                                    .bg(rgba(0xEF444408))
                                    .hover(|d| d.bg(rgba(0xEF444414)))
                                    .on_click(move |_event, _window, cx| {
                                        if let Some(entity) = deny_view.upgrade() {
                                            entity.update(cx, |this, cx| {
                                                let _ =
                                                    this.approve_reject_option(&deny_request, cx);
                                            });
                                        }
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child("Deny"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .opacity(0.42)
                                                    .child("\u{2318}\u{2325}Z"),
                                            ),
                                    ),
                            ),
                    )
                    .when(show_options_button, |d| {
                        d.child(
                            div()
                                .id("agent_chat-inline-permission-options")
                                .px(px(10.0))
                                .py(px(6.0))
                                .cursor_pointer()
                                .border_l_2()
                                .border_color(if options_open {
                                    rgb(theme.colors.accent.selected)
                                } else {
                                    rgba(0x00000000)
                                })
                                .bg(rgba((theme.colors.text.primary << 8) | 0x06))
                                .hover(|this| {
                                    this.bg(rgba((theme.colors.text.primary << 8) | 0x0C))
                                })
                                .on_click(move |_event, _window, cx| {
                                    if let Some(entity) = options_view.upgrade() {
                                        entity.update(cx, |this, cx| {
                                            let _ = this
                                                .toggle_permission_options(&options_request, cx);
                                        });
                                    }
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(selected_option_label.clone()),
                                        )
                                        .child(div().text_xs().opacity(0.42).child(
                                            if options_open {
                                                "\u{2318}\u{2325}A \u{25BE}"
                                            } else {
                                                "\u{2318}\u{2325}A \u{25B8}"
                                            },
                                        )),
                                ),
                        )
                    }),
            )
            .when(options_open && request.options.len() > 1, |d| {
                d.child(
                    div()
                        .pt(px(6.0))
                        .children(request.options.iter().enumerate().map(|(i, option)| {
                            Self::render_permission_option_row(
                                option,
                                i,
                                i == selected_index,
                                view.clone(),
                            )
                        })),
                )
            })
            .child(
                div()
                    .pt(px(8.0))
                    .text_xs()
                    .opacity(0.42)
                    .child(shortcut_hint),
            )
            .into_any_element()
    }

    fn render_plan_strip(entries: &[String]) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();

        div()
            .w_full()
            .px(px(12.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .bg(rgba((theme.colors.accent.selected << 8) | 0x0C))
            .border_1()
            .border_color(rgba((theme.colors.accent.selected << 8) | 0x28))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .opacity(0.7)
                    .pb(px(4.0))
                    .child("Plan"),
            )
            .children(entries.iter().enumerate().map(|(i, entry)| {
                div()
                    .text_xs()
                    .opacity(0.65)
                    .py(px(1.0))
                    .child(format!("{}. {}", i + 1, entry))
            }))
            .into_any_element()
    }

    // ── Toolbar ───────────────────────────────────────────────────

    fn render_attach_menu(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = theme::get_cached_theme();

        div()
            .w_full()
            .px(px(8.0))
            .pb(px(4.0))
            .child(
                div()
                    .w_full()
                    .rounded(px(8.0))
                    .bg(rgb(theme.colors.background.search_box))
                    .border_1()
                    .border_color(rgba((theme.colors.ui.border << 8) | 0x40))
                    .py(px(4.0))
                    .child(
                        div()
                            .id("attach-paste")
                            .w_full()
                            .px(px(10.0))
                            .py(px(4.0))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgba((theme.colors.text.primary << 8) | 0x0C)))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                if let Some(clipboard) = cx.read_from_clipboard() {
                                    if let Some(text) = clipboard.text() {
                                        if !text.is_empty() {
                                            this.live_thread().update(cx, |thread, cx| {
                                                thread.input.insert_str(&text);
                                                cx.notify();
                                            });
                                            this.refresh_agent_chat_spine_from_composer(cx);
                                            if !this.agent_chat_spine_owns_list() {
                                                this.refresh_composer_picker_session(cx);
                                            }
                                            this.cursor_visible = true;
                                        }
                                    }
                                }
                                this.attach_menu_open = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(div().text_sm().child("Paste Clipboard"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .opacity(0.45)
                                            .child("Insert clipboard text at cursor"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("attach-screenshot")
                            .w_full()
                            .px(px(10.0))
                            .py(px(4.0))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgba((theme.colors.text.primary << 8) | 0x0C)))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                // Insert a hint about the screenshot path
                                this.live_thread().update(cx, |thread, cx| {
                                    thread.input.insert_str("What's on my screen? ");
                                    cx.notify();
                                });
                                this.refresh_agent_chat_spine_from_composer(cx);
                                if !this.agent_chat_spine_owns_list() {
                                    this.refresh_composer_picker_session(cx);
                                }
                                this.attach_menu_open = false;
                                this.cursor_visible = true;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(div().text_sm().child("Ask About Screen"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .opacity(0.45)
                                            .child("Screenshot is in context"),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_toolbar(
        &self,
        weak_view: WeakEntity<AgentChatView>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        Self::render_toolbar_from_snapshot(self.footer_snapshot(cx), weak_view)
    }

    fn render_send_button_for_state(
        can_send: bool,
        status: AgentChatThreadStatus,
        weak_view: WeakEntity<AgentChatView>,
        theme: &crate::theme::Theme,
    ) -> gpui::AnyElement {
        let is_streaming = matches!(status, AgentChatThreadStatus::Streaming);
        let is_waiting = matches!(status, AgentChatThreadStatus::WaitingForPermission);
        let accent = theme.colors.accent.selected;
        let text_primary = theme.colors.text.primary;

        let (icon_char, bg, opacity, tooltip, id) = match (is_streaming || is_waiting, can_send) {
            (true, true) => (
                "\u{21E7}",
                rgba((accent << 8) | 0x24),
                0.92_f32,
                "Queue message — sends when the current turn finishes",
                "agent_chat-queue-btn",
            ),
            // Quiet activity dot — Esc (and the footer Stop) is the stop
            // affordance; clicking the dot still cancels for mouse users.
            (true, false) => (
                "\u{25CF}",
                rgba(0x00000000),
                0.40_f32,
                "Streaming \u{2014} press Esc to stop",
                "agent_chat-streaming-dot",
            ),
            (false, true) => (
                "\u{2191}",
                rgba((accent << 8) | 0x30),
                0.90_f32,
                "Send message",
                "agent_chat-send-btn",
            ),
            (false, false) => (
                "\u{2191}",
                rgba((text_primary << 8) | 0x06),
                0.30_f32,
                "Type a message first",
                "agent_chat-send-btn",
            ),
        };

        let tooltip_text = tooltip.to_string();
        let mut btn = div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.0))
            .rounded(px(6.0))
            .bg(bg)
            .text_sm()
            .opacity(opacity)
            .tooltip(move |window, cx| Tooltip::new(tooltip_text.clone()).build(window, cx));

        if is_streaming && !can_send {
            btn = btn.cursor_pointer().on_click(move |_event, _window, cx| {
                if let Some(view) = weak_view.upgrade() {
                    view.update(cx, |this, cx| {
                        this.live_thread()
                            .update(cx, |thread, cx| thread.cancel_streaming(cx));
                    });
                }
            });
        } else if can_send {
            btn = btn.cursor_pointer().on_click(move |_event, _window, cx| {
                if let Some(view) = weak_view.upgrade() {
                    view.update(cx, |this, cx| this.submit_with_expanded_tokens(cx));
                }
            });
        }

        btn.child(icon_char).into_any_element()
    }

    // ── Composer picker ────────────────────────────────────────────

    /// Maximum visible rows in the composer picker.
    pub(super) const COMPOSER_PICKER_MAX_VISIBLE: usize = 8;

    /// Detect an active slash/profile query from the input text and cursor position.
    ///
    /// Returns the character range of `@query` and the query string, or `None`
    /// if the cursor is not inside a popup-owned trigger.
    /// Find an active trigger (`/` or profile) before the cursor.
    ///
    /// Returns `(trigger, char_range, query_text)` when the cursor is
    /// immediately after an in-progress `@query` or `/query`.
    fn find_active_trigger(
        text: &str,
        cursor: usize,
    ) -> Option<(
        AgentChatComposerPickerTrigger,
        std::ops::Range<usize>,
        String,
    )> {
        let query =
            crate::ai::context_selector::context_selector_query_before_cursor(text, cursor)?;
        let trigger = AgentChatComposerPickerTrigger::from_context_selector(query.trigger)?;
        Some((trigger, query.char_range, query.query))
    }

    fn focused_inline_token_prefers_preview(
        text: &str,
        cursor: usize,
        typed_aliases: &std::collections::HashMap<String, crate::ai::message_parts::AiContextPart>,
    ) -> bool {
        let Some(token_span) = crate::ai::context_mentions::inline_token_at_cursor(text, cursor)
        else {
            return false;
        };

        let has_resolved_mention =
            crate::ai::context_mentions::parse_inline_context_mentions_with_aliases(
                text,
                typed_aliases,
            )
            .into_iter()
            .any(|mention| cursor > mention.range.start && cursor <= mention.range.end);

        has_resolved_mention
            || crate::ai::agent_chat::ui::portal_contract::portal_target_from_inline_token(
                &token_span.token,
            )
            .is_some()
    }

    fn is_reopen_focused_mention_shortcut(key: &str, modifiers: &gpui::Modifiers) -> bool {
        let is_cmd_period =
            modifiers.platform && !modifiers.shift && (key == "." || key == "period");
        let is_cmd_shift_o = modifiers.platform && modifiers.shift && key.eq_ignore_ascii_case("o");
        is_cmd_period || is_cmd_shift_o
    }

    /// Re-derive the composer session from current input state.
    ///
    /// Called after every input mutation and cursor movement.
    pub(super) fn refresh_composer_picker_session(&mut self, cx: &mut Context<Self>) {
        if self.agent_chat_spine_owns_list() {
            self.composer_picker_session = None;
            self.dismissed_mention_trigger = None;
            cx.notify();
            return;
        }

        if self.is_setup_mode() {
            let had_picker = self.composer_picker_session.take().is_some()
                || self.dismissed_mention_trigger.take().is_some();
            if had_picker {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_composer_picker_cleared_setup_mode",
                );
                cx.notify();
            }
            return;
        }

        let (text, cursor, available_commands) = {
            let thread = self.live_thread().read(cx);
            (
                thread.input.text().to_string(),
                thread.input.cursor(),
                thread.available_commands().to_vec(),
            )
        };

        let previous_index = self
            .composer_picker_session
            .as_ref()
            .map(|s| s.selected_index)
            .unwrap_or(0);
        let previous_visible_start = self
            .composer_picker_session
            .as_ref()
            .map(|s| s.visible_start)
            .unwrap_or(0);

        let focused_inline_preview =
            Self::focused_inline_token_prefers_preview(&text, cursor, &self.typed_mention_aliases);
        let mut active_dismissed_trigger = None;
        let next_session = if focused_inline_preview {
            None
        } else {
            match Self::find_active_trigger(&text, cursor) {
                Some((trigger, trigger_range, query)) => {
                    let active_trigger = AgentChatDismissedComposerPickerTrigger {
                        trigger,
                        trigger_range: trigger_range.clone(),
                        query: query.clone(),
                        cursor,
                    };
                    if self.dismissed_mention_trigger.as_ref() == Some(&active_trigger) {
                        active_dismissed_trigger = Some(active_trigger);
                        None
                    } else {
                        let mut items = match trigger {
                            AgentChatComposerPickerTrigger::Slash => {
                                if self.cached_slash_commands.is_empty() {
                                    // Async discovery hasn't completed yet — show
                                    // intentional loading row instead of blank list.
                                    vec![slash_command_loading_row()]
                                } else {
                                    let entries = if available_commands.is_empty() {
                                        self.cached_slash_commands.clone()
                                    } else {
                                        self.resolved_slash_commands(&available_commands)
                                    };
                                    if entries.is_empty() {
                                        // Discovery completed but catalog is empty
                                        // (no defaults, no plugins, no Claude skills).
                                        vec![slash_command_empty_row()]
                                    } else {
                                        let payloads: Vec<(SlashCommandPayload, String)> = entries
                                            .iter()
                                            .map(|e| (e.to_payload(), e.description.clone()))
                                            .collect();
                                        let mut items = slash_command_rows_with_payloads(
                                            &query,
                                            payloads.iter().map(|(p, d)| (p, d.as_str())),
                                        );
                                        if items.is_empty() {
                                            // Non-empty catalog filtered to zero by
                                            // query — distinct from empty catalog.
                                            items.push(slash_command_no_match_row());
                                        }
                                        items
                                    }
                                }
                            }
                            AgentChatComposerPickerTrigger::Profile => {
                                self.build_profile_picker_items(&query)
                            }
                        };

                        // Filter out portal items the host does not support.
                        items.retain(|item| {
                            if let ContextSelectorRowKind::Portal(kind) = item.kind {
                                self.is_portal_kind_allowed(kind)
                            } else {
                                true
                            }
                        });

                        let mut selected_index =
                        crate::components::inline_dropdown::inline_dropdown_clamp_selected_index(
                            previous_index,
                            items.len(),
                        );

                        // If a slash prime is pending, pre-select the matching row.
                        if let Some(ref prime_name) = self.pending_slash_prime {
                            if trigger == AgentChatComposerPickerTrigger::Slash {
                                if let Some(ix) = items.iter().position(|item| {
                                    matches!(
                                        &item.kind,
                                        ContextSelectorRowKind::SlashCommand(payload)
                                        if payload.slash_name() == prime_name
                                    )
                                }) {
                                    selected_index = ix;
                                    // Consume the prime so it doesn't override future selections.
                                    self.pending_slash_prime = None;
                                }
                            }
                        }

                        let visible = Self::composer_picker_visible_range_from_start(
                            previous_visible_start,
                            selected_index,
                            items.len(),
                        );
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_composer_picker_refreshed",
                            layout = "inline_dropdown",
                            ?trigger,
                            query = %query,
                            item_count = items.len(),
                            selected_index,
                            live_command_count = available_commands.len(),
                            anchor_char = trigger_range.start,
                            visible_start = visible.start,
                            visible_end = visible.end,
                        );
                        Some(AgentChatComposerPickerSession {
                            trigger,
                            trigger_range,
                            query,
                            selected_index,
                            visible_start: visible.start,
                            items,
                        })
                    }
                }
                None => None,
            }
        };

        let transition = reduce_agent_chat_composer_picker(
            self.composer_picker_state(),
            AgentChatComposerPickerEvent::Refresh(AgentChatComposerPickerRefreshInput {
                active_trigger: active_dismissed_trigger,
                next_session,
                focused_inline_preview,
            }),
        );
        self.apply_composer_picker_transition(transition, cx);
    }

    /// Log the visible window range for observability.
    fn log_composer_picker_visible_range(&self, reason: &'static str) {
        let Some(session) = self.composer_picker_session.as_ref() else {
            return;
        };
        let visible = Self::composer_picker_visible_range(session);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_composer_picker_visible_range",
            reason,
            selected_index = session.selected_index,
            item_count = session.items.len(),
            visible_start = visible.start,
            visible_end = visible.end,
        );
    }

    /// Apply a hint chip token by inserting it at the cursor (or replacing
    /// the active trigger) and running it through the normal picker acceptance
    /// path. Preserves surrounding composer text.
    pub(super) fn apply_picker_hint_token(&mut self, token: &str, cx: &mut Context<Self>) {
        let (text, cursor) = {
            let thread = self.live_thread().read(cx);
            (thread.input.text().to_string(), thread.input.cursor())
        };

        let (next_text, next_cursor) =
            Self::replace_active_trigger_or_insert_at_cursor(&text, cursor, token);

        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(next_text);
            thread.input.set_cursor(next_cursor);
            cx.notify();
        });
        self.refresh_agent_chat_spine_from_composer(cx);
        if !self.agent_chat_spine_owns_list() {
            self.refresh_composer_picker_session(cx);
        }
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_picker_hint_applied",
            token,
            has_session = self.composer_picker_session.is_some(),
            cursor_after = next_cursor,
        );
        if self.composer_picker_session.is_some() {
            self.accept_composer_picker_selection_impl(false, cx);
        } else {
            self.sync_inline_mentions(cx);
            self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        }
    }

    pub(super) fn insert_picker_hint_prefix(&mut self, prefix: &str, cx: &mut Context<Self>) {
        let (text, cursor) = {
            let thread = self.live_thread().read(cx);
            (thread.input.text().to_string(), thread.input.cursor())
        };

        let (next_text, next_cursor) =
            Self::replace_active_trigger_or_insert_at_cursor(&text, cursor, prefix);

        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(next_text);
            thread.input.set_cursor(next_cursor);
            cx.notify();
        });
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_picker_hint_prefix_inserted",
            prefix,
            cursor_after = next_cursor,
        );
        self.refresh_agent_chat_spine_from_composer(cx);
        if !self.agent_chat_spine_owns_list() {
            self.refresh_composer_picker_session(cx);
        }
        self.sync_inline_mentions(cx);
        self.refresh_composer_picker_state_after_parent_change(cx);
    }

    /// Accept the currently selected picker row.
    ///
    /// Both Enter and Tab autocomplete the focused picker row. Literal slash
    /// commands are inserted into the composer; slash-picked context items
    /// attach a pending context part and remove the typed `/query` token.
    pub(crate) fn accept_composer_picker_selection(&mut self, cx: &mut Context<Self>) {
        self.accept_composer_picker_selection_impl(false, cx);
    }

    /// Fallback entry for main-window key interceptors that need to keep Enter
    /// routed to the Agent Chat picker when the composer view does not receive it.
    pub(crate) fn handle_enter_key(&mut self, cx: &mut Context<Self>) -> bool {
        self.handle_picker_accept_key("enter", cx)
    }

    pub(crate) fn select_mention_index(&mut self, index: usize) {
        if let Some(session) = self.composer_picker_session.as_mut() {
            if !session.items.is_empty() {
                session.selected_index = index.min(session.items.len().saturating_sub(1));
                let visible = Self::composer_picker_visible_range_from_start(
                    session.visible_start,
                    session.selected_index,
                    session.items.len(),
                );
                session.visible_start = visible.start;
            }
        }
    }

    /// Insert `replacement` at the cursor, replacing the active trigger range
    /// if one is found. Preserves surrounding text and returns the updated
    /// text plus the new cursor position.
    fn replace_active_trigger_or_insert_at_cursor(
        text: &str,
        cursor: usize,
        replacement: &str,
    ) -> (String, usize) {
        let content = replacement.trim();
        let wants_trailing_space = replacement.chars().last().is_some_and(char::is_whitespace);

        match Self::find_active_trigger(text, cursor) {
            Some((_trigger, trigger_range, _query)) => {
                let mut inserted = content.to_string();
                if wants_trailing_space {
                    inserted.push(' ');
                }
                let cursor_after = trigger_range.start + inserted.chars().count();
                let next_text = Self::replace_text_in_char_range(text, trigger_range, &inserted);
                (next_text, cursor_after)
            }
            None => {
                let prev = cursor.checked_sub(1).and_then(|ix| text.chars().nth(ix));
                let next = text.chars().nth(cursor);
                let mut formatted = String::new();
                if prev.is_some_and(|ch| !ch.is_whitespace()) {
                    formatted.push(' ');
                }
                formatted.push_str(content);
                if wants_trailing_space || next.is_some_and(|ch| !ch.is_whitespace()) {
                    formatted.push(' ');
                }
                let cursor_after = cursor + formatted.trim_end().chars().count();
                let next_text = Self::replace_text_in_char_range(text, cursor..cursor, &formatted);
                (next_text, cursor_after)
            }
        }
    }

    /// Replace a char-range in the given text with `replacement`.
    fn replace_text_in_char_range(
        text: &str,
        char_range: std::ops::Range<usize>,
        replacement: &str,
    ) -> String {
        let start_byte = Self::char_to_byte_offset(text, char_range.start);
        let end_byte = Self::char_to_byte_offset(text, char_range.end);
        let mut out =
            String::with_capacity(text.len() - (end_byte - start_byte) + replacement.len());
        out.push_str(&text[..start_byte]);
        out.push_str(replacement);
        out.push_str(&text[end_byte..]);
        out
    }

    fn text_in_char_range(text: &str, char_range: std::ops::Range<usize>) -> String {
        let start_byte = Self::char_to_byte_offset(text, char_range.start);
        let end_byte = Self::char_to_byte_offset(text, char_range.end);
        text[start_byte..end_byte].to_string()
    }

    /// Return the caret position immediately after replacing `char_range`
    /// with `replacement`.
    fn caret_after_replacement(char_range: &std::ops::Range<usize>, replacement: &str) -> usize {
        char_range.start + replacement.chars().count()
    }

    /// Accept the currently selected picker row, optionally submitting literal
    /// slash commands after insertion.
    ///
    /// `submit` only applies to literal slash commands such as `/compact`.
    /// Context attachments picked from slash mode never auto-submit.
    fn accept_composer_picker_selection_impl(&mut self, submit: bool, cx: &mut Context<Self>) {
        use crate::ai::context_mentions::part_to_inline_token;

        let transition = reduce_agent_chat_composer_picker(
            self.composer_picker_state(),
            AgentChatComposerPickerEvent::Accept,
        );
        let session = match self.apply_composer_picker_transition(transition, cx) {
            Some(s) => s,
            None => return,
        };
        let item = match session.items.get(session.selected_index).cloned() {
            Some(i) => i,
            None => return,
        };

        // Inert items (loading spinner, empty state) are non-actionable.
        if matches!(item.kind, ContextSelectorRowKind::Inert) {
            tracing::debug!(item_id = %item.id, "agent_chat_picker_inert_item_ignored");
            let transition = reduce_agent_chat_composer_picker(
                self.composer_picker_state(),
                AgentChatComposerPickerEvent::AcceptIgnoredKeepOpen(session),
            );
            self.apply_composer_picker_transition(transition, cx);
            return;
        }

        let trigger_str = session.trigger.label();

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_picker_item_accepted",
            trigger = ?session.trigger,
            submit,
            item_id = %item.id,
            item_label = %item.label,
        );

        // Record accepted item for telemetry / getAgentChatState queries.
        // cursor_after is set to 0 here and updated after insertion below.
        self.last_accepted_item = Some(crate::protocol::AgentChatAcceptedItem {
            label: item.label.to_string(),
            id: item.id.to_string(),
            trigger: trigger_str.to_string(),
            cursor_after: 0, // Updated after insertion.
        });

        // ── Slash command acceptance: default inserts text, skills stage content ──
        if session.trigger == AgentChatComposerPickerTrigger::Slash {
            if let ContextSelectorRowKind::SlashCommand(ref payload) = item.kind {
                match payload {
                    SlashCommandPayload::Default { name } => {
                        // Default commands insert literal `/command ` text.
                        let current_text = self.live_thread().read(cx).input.text().to_string();
                        let command_text = format!("/{name} ");
                        let next_text = Self::replace_text_in_char_range(
                            &current_text,
                            session.trigger_range.clone(),
                            &command_text,
                        );
                        let next_cursor =
                            Self::caret_after_replacement(&session.trigger_range, &command_text);
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_picker_literal_slash_inserted",
                            slash_name = %name,
                            submit,
                        );
                        if let Some(ref mut accepted) = self.last_accepted_item {
                            accepted.cursor_after = next_cursor;
                        }
                        self.live_thread().update(cx, |thread, cx| {
                            thread.input.set_text(next_text);
                            thread.input.set_cursor(next_cursor);
                            if submit {
                                let _ = thread.submit_input(cx);
                            } else {
                                cx.notify();
                            }
                        });
                    }
                    SlashCommandPayload::PluginSkill(skill) => {
                        // Plugin skills insert `/slash-name ` as visible text
                        // and attach the skill body as a context part so the
                        // composer stays compact while the agent still receives
                        // the staged skill prompt on submit.
                        let owner = if skill.plugin_title.is_empty() {
                            skill.plugin_id.clone()
                        } else {
                            skill.plugin_title.clone()
                        };
                        let current_text = self.live_thread().read(cx).input.text().to_string();
                        let command_text = build_skill_slash_command_text(&skill.skill_id);
                        let next_text = Self::replace_text_in_char_range(
                            &current_text,
                            session.trigger_range.clone(),
                            &command_text,
                        );
                        let next_cursor =
                            Self::caret_after_replacement(&session.trigger_range, &command_text);
                        let part = build_skill_context_part(
                            &skill.title,
                            &owner,
                            &skill.skill_id,
                            &skill.path,
                        );
                        tracing::info!(
                            plugin_id = %skill.plugin_id,
                            skill_id = %skill.skill_id,
                            "agent_chat_slash_skill_selected"
                        );
                        if let Some(ref mut accepted) = self.last_accepted_item {
                            accepted.cursor_after = next_cursor;
                        }
                        self.live_thread().update(cx, |thread, cx| {
                            thread.input.set_text(next_text);
                            thread.input.set_cursor(next_cursor);
                            thread.add_context_part(part, cx);
                            if submit {
                                let _ = thread.submit_input(cx);
                            } else {
                                cx.notify();
                            }
                        });
                    }
                    SlashCommandPayload::ClaudeCodeSkill {
                        skill_id,
                        skill_path,
                    } => {
                        // Claude Code skills insert `/slash-name ` and attach
                        // the skill body as a context part, mirroring plugin
                        // skill behavior so the composer stays compact.
                        let current_text = self.live_thread().read(cx).input.text().to_string();
                        let command_text = build_skill_slash_command_text(skill_id);
                        let next_text = Self::replace_text_in_char_range(
                            &current_text,
                            session.trigger_range.clone(),
                            &command_text,
                        );
                        let next_cursor =
                            Self::caret_after_replacement(&session.trigger_range, &command_text);
                        let part =
                            build_skill_context_part(skill_id, "Claude Code", skill_id, skill_path);
                        tracing::info!(
                            skill_id = %skill_id,
                            path = %skill_path.display(),
                            "agent_chat_slash_claude_skill_selected"
                        );
                        if let Some(ref mut accepted) = self.last_accepted_item {
                            accepted.cursor_after = next_cursor;
                        }
                        self.live_thread().update(cx, |thread, cx| {
                            thread.input.set_text(next_text);
                            thread.input.set_cursor(next_cursor);
                            thread.add_context_part(part, cx);
                            if submit {
                                let _ = thread.submit_input(cx);
                            } else {
                                cx.notify();
                            }
                        });
                    }
                }
                self.refresh_composer_picker_state_after_parent_change(cx);
                cx.notify();
                return;
            }
        }

        if session.trigger == AgentChatComposerPickerTrigger::Profile {
            if let ContextSelectorRowKind::AgentChatProfile { profile_id, .. } = item.kind {
                let current_text = self.live_thread().read(cx).input.text().to_string();
                let next_text = Self::replace_text_in_char_range(
                    &current_text,
                    session.trigger_range.clone(),
                    "",
                );
                let next_cursor = session.trigger_range.start;
                if let Some(ref mut accepted) = self.last_accepted_item {
                    accepted.cursor_after = next_cursor;
                }
                self.live_thread().update(cx, |thread, cx| {
                    thread.input.set_text(next_text);
                    thread.input.set_cursor(next_cursor);
                    cx.notify();
                });
                self.select_profile_from_popup(&profile_id, cx);
                self.refresh_composer_picker_state_after_parent_change(cx);
                cx.notify();
                return;
            }
        }

        // ── Build context part; decide if inline-mention sync applies ──
        let (part, inline_text, allow_inline_sync) = match &item.kind {
            ContextSelectorRowKind::PortalPrefix(payload) => {
                let current_text = self.live_thread().read(cx).input.text().to_string();
                let prefix_text = format!("@{}:", payload.prefix);
                let next_text = Self::replace_text_in_char_range(
                    &current_text,
                    session.trigger_range.clone(),
                    &prefix_text,
                );
                let next_cursor =
                    Self::caret_after_replacement(&session.trigger_range, &prefix_text);
                if let Some(ref mut accepted) = self.last_accepted_item {
                    accepted.cursor_after = next_cursor;
                }
                self.live_thread().update(cx, |thread, cx| {
                    thread.input.set_text(next_text);
                    thread.input.set_cursor(next_cursor);
                    cx.notify();
                });
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_inline_portal_prefix_inserted",
                    portal_kind = ?payload.portal_kind,
                    prefix = %payload.prefix,
                    cursor_after = next_cursor,
                );
                self.refresh_agent_chat_spine_from_composer(cx);
                if !self.agent_chat_spine_owns_list() {
                    self.refresh_composer_picker_session(cx);
                }
                self.refresh_composer_picker_state_after_parent_change(cx);
                return;
            }
            ContextSelectorRowKind::BuiltIn(kind) => {
                if *kind == crate::ai::context_contract::ContextAttachmentKind::Dictation {
                    let portal_kind =
                        crate::ai::context_selector::types::ContextPortalKind::DictationHistory;
                    self.open_picker_portal(
                        portal_kind,
                        session.trigger_range.clone(),
                        crate::ai::agent_chat::ui::portal_contract::picker_portal_query(
                            portal_kind,
                            &session.query,
                        ),
                        cx,
                    );
                    return;
                }

                (
                    kind.part(),
                    kind.spec().mention.unwrap_or("@here").to_string(),
                    false,
                )
            }

            ContextSelectorRowKind::File(path) | ContextSelectorRowKind::Folder(path) => {
                let path_text = path.to_string_lossy().to_string();
                let file_part = AiContextPart::FilePath {
                    path: path_text.clone(),
                    label: item.label.to_string(),
                };
                let inline_text = crate::ai::context_mentions::part_to_inline_token(&file_part)
                    .unwrap_or_else(|| format!("@file:{path_text}"));
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_inline_file_token_inserted",
                    path = %path_text,
                    inline_text = %inline_text,
                );
                (file_part, inline_text, false)
            }
            ContextSelectorRowKind::SlashCommand(_)
            | ContextSelectorRowKind::AgentChatProfile { .. }
            | ContextSelectorRowKind::Inert => return,
            ContextSelectorRowKind::PortalResult(payload) => {
                let part = match &payload.attachment {
                    crate::ai::context_selector::types::InlinePortalAttachment::ResourceUri {
                        uri,
                        label,
                    } => AiContextPart::ResourceUri {
                        uri: uri.clone(),
                        label: label.clone(),
                    },
                    crate::ai::context_selector::types::InlinePortalAttachment::FilePath {
                        path,
                        label,
                    } => AiContextPart::FilePath {
                        path: path.clone(),
                        label: label.clone(),
                    },
                    crate::ai::context_selector::types::InlinePortalAttachment::SkillFile {
                        path,
                        label,
                        skill_name,
                        owner_label,
                        slash_name,
                    } => AiContextPart::SkillFile {
                        path: path.clone(),
                        label: label.clone(),
                        skill_name: skill_name.clone(),
                        owner_label: owner_label.clone(),
                        slash_name: slash_name.clone(),
                    },
                    crate::ai::context_selector::types::InlinePortalAttachment::TextBlock {
                        label,
                        source,
                        text,
                        mime_type,
                    } => AiContextPart::TextBlock {
                        label: label.clone(),
                        source: source.clone(),
                        text: text.clone(),
                        mime_type: mime_type.clone(),
                    },
                    crate::ai::context_selector::types::InlinePortalAttachment::FocusedTarget {
                        source,
                        kind,
                        semantic_id,
                        label,
                        metadata,
                    } => AiContextPart::FocusedTarget {
                        target: crate::ai::TabAiTargetContext {
                            source: source.clone(),
                            kind: kind.clone(),
                            semantic_id: semantic_id.clone(),
                            label: label.clone(),
                            metadata: metadata.clone(),
                        },
                        label: label.clone(),
                    },
                };
                let fallback_prefix = match payload.portal_kind {
                    crate::ai::context_selector::types::ContextPortalKind::FileSearch => "file",
                    crate::ai::context_selector::types::ContextPortalKind::BrowserHistory => {
                        "browser-history"
                    }
                    crate::ai::context_selector::types::ContextPortalKind::BrowserTabs => "tabs",
                    crate::ai::context_selector::types::ContextPortalKind::ClipboardHistory => {
                        "clipboard"
                    }
                    crate::ai::context_selector::types::ContextPortalKind::DictationHistory => {
                        "dictation"
                    }
                    crate::ai::context_selector::types::ContextPortalKind::ScriptSearch => "script",
                    crate::ai::context_selector::types::ContextPortalKind::ScriptletSearch => {
                        "scriptlet"
                    }
                    crate::ai::context_selector::types::ContextPortalKind::SkillSearch => "skill",
                    crate::ai::context_selector::types::ContextPortalKind::NotesBrowse => "note",
                    crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory => {
                        "history"
                    }
                    crate::ai::context_selector::types::ContextPortalKind::Terminal => "terminal",
                };
                let inline_text = part_to_inline_token(&part).unwrap_or_else(|| {
                    crate::ai::context_mentions::format_typed_label_mention_token(
                        fallback_prefix,
                        item.label.as_ref(),
                    )
                });
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_inline_portal_result_inserted",
                    portal_kind = ?payload.portal_kind,
                    inline_text = %inline_text,
                );
                (part, inline_text, false)
            }
            ContextSelectorRowKind::Portal(portal_kind) => {
                self.open_picker_portal(
                    *portal_kind,
                    session.trigger_range.clone(),
                    crate::ai::agent_chat::ui::portal_contract::picker_portal_query(
                        *portal_kind,
                        &session.query,
                    ),
                    cx,
                );
                return;
            }
        };

        let current_text = self.live_thread().read(cx).input.text().to_string();

        // Decide ownership *before* mutating the thread — the check reads
        // the current pending_context_parts to see if the part was already
        // attached from a non-inline source (slash, chip, setup).
        let should_claim_inline_ownership = if allow_inline_sync {
            self.should_claim_inline_mention_ownership(&part, cx)
        } else {
            false
        };

        // For @-mention triggers: replace trigger+query with the inline
        // mention text and run inline sync.
        // Slash mode is command-only, so built-in context items should not
        // normally reach this path from `/`.
        let replacement = if allow_inline_sync {
            format!("{inline_text} ")
        } else {
            String::new()
        };
        let next_cursor = Self::caret_after_replacement(&session.trigger_range, &replacement);

        if let Some(ref mut accepted) = self.last_accepted_item {
            accepted.cursor_after = next_cursor;
        }

        let next_text = Self::replace_text_in_char_range(
            &current_text,
            session.trigger_range.clone(),
            &replacement,
        );

        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(next_text);
            thread.input.set_cursor(next_cursor);
            thread.add_context_part(part.clone(), cx);
            cx.notify();
        });

        // Register typed alias for non-builtin parts so the parser can
        // resolve typed @type:name tokens back to the full AiContextPart.
        if matches!(
            item.kind,
            ContextSelectorRowKind::File(_)
                | ContextSelectorRowKind::Folder(_)
                | ContextSelectorRowKind::PortalResult(_)
        ) {
            if let Some(token) = part_to_inline_token(&part) {
                self.typed_mention_aliases.insert(token, part.clone());
            } else {
                self.typed_mention_aliases
                    .insert(inline_text.clone(), part.clone());
            }
        }

        if allow_inline_sync {
            if let Some(token) = part_to_inline_token(&part) {
                if should_claim_inline_ownership {
                    self.inline_owned_context_tokens.insert(token.clone());
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_inline_mention_ownership_claimed",
                        token = %token,
                        item_id = %item.id,
                        item_label = %item.label,
                    );
                } else {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_inline_mention_ownership_preserved_existing_attachment",
                        token = %token,
                        item_id = %item.id,
                        item_label = %item.label,
                    );
                }
            }
            self.sync_inline_mentions(cx);
        } else {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_picker_context_attached_from_slash",
                item_id = %item.id,
                item_label = %item.label,
                source = %part.source(),
            );
            cx.notify();
        }
        self.refresh_composer_picker_state_after_parent_change(cx);
    }

    /// Check whether accepting a picker item should claim inline ownership
    /// of the resulting token.  Delegates to the shared helper in
    /// `context_mentions::should_claim_inline_mention_ownership`.
    fn should_claim_inline_mention_ownership(
        &self,
        part: &crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) -> bool {
        crate::ai::context_mentions::should_claim_inline_mention_ownership(
            part,
            self.live_thread().read(cx).pending_context_parts(),
            &self.inline_owned_context_tokens,
        )
    }

    /// Return highlight ranges for inline `@mentions` that are **actually
    /// attached** as pending context parts. Unattached lookalike tokens are
    /// not highlighted.
    fn attached_inline_mention_highlight_ranges(
        text: &str,
        attached_parts: &[AiContextPart],
        accent_color: u32,
        aliases: &std::collections::HashMap<String, AiContextPart>,
    ) -> Vec<TextHighlightRange> {
        use crate::ai::context_mentions::{
            parse_inline_context_mentions_with_aliases, part_to_inline_token,
        };

        let attached_tokens: HashSet<String> = attached_parts
            .iter()
            .filter_map(part_to_inline_token)
            .collect();

        parse_inline_context_mentions_with_aliases(text, aliases)
            .into_iter()
            .filter(|mention| attached_tokens.contains(&mention.canonical_token))
            .map(|mention| TextHighlightRange {
                start: mention.range.start,
                end: mention.range.end,
                color: accent_color,
            })
            .collect()
    }

    /// Return a highlight range for a leading `/slash-name` token in the
    /// composer. Only the first token is recognized because slash commands
    /// are positional; mid-text `/...` sequences stay in the default color.
    fn leading_slash_highlight_range(text: &str, accent_color: u32) -> Option<TextHighlightRange> {
        let mut chars = text.chars();
        if chars.next()? != '/' {
            return None;
        }
        let mut end = 1usize;
        for ch in chars {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                end += 1;
            } else {
                break;
            }
        }
        if end <= 1 {
            return None;
        }
        Some(TextHighlightRange {
            start: 0,
            end,
            color: accent_color,
        })
    }

    /// Synchronise `pending_context_parts` from the live inline `@mention`
    /// tokens. Removes stale parts whose token was deleted from the input
    /// and adds new parts for freshly typed tokens.
    fn sync_inline_mentions(&mut self, cx: &mut Context<Self>) {
        let text = self.live_thread().read(cx).input.text().to_string();
        let attached_parts = self.live_thread().read(cx).pending_context_parts().to_vec();

        let plan = crate::ai::context_mentions::build_inline_mention_sync_plan_with_aliases(
            &text,
            &attached_parts,
            &self.inline_owned_context_tokens,
            &self.typed_mention_aliases,
        );

        self.live_thread().update(cx, |thread, cx| {
            for ix in plan.stale_indices.iter().rev().copied() {
                thread.remove_context_part(ix, cx);
            }
            for part in &plan.added_parts {
                thread.add_context_part(part.clone(), cx);
            }
        });

        self.inline_owned_context_tokens
            .retain(|token| plan.desired_tokens.contains(token));
        self.inline_owned_context_tokens
            .extend(plan.added_tokens.iter().cloned());

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_inline_mentions_synced",
            desired_count = plan.desired_parts.len(),
            added_count = plan.added_parts.len(),
            removed_count = plan.stale_indices.len(),
            token_count = self.inline_owned_context_tokens.len(),
        );
    }

    /// Fixed picker dropdown width.
    const AGENT_CHAT_COMPOSER_PICKER_WIDTH: f32 = 320.0;

    /// Minimum usable picker width when the window is narrow.
    const AGENT_CHAT_COMPOSER_PICKER_MIN_WIDTH: f32 = 200.0;

    /// Horizontal padding used by the Agent Chat composer input row.
    const AGENT_CHAT_INPUT_PADDING_X: f32 = 12.0;

    /// Keep the picker inset from the right edge so it never clips.
    const AGENT_CHAT_COMPOSER_PICKER_EDGE_GUTTER: f32 = 12.0;

    /// Top padding used by the Agent Chat composer input row.
    const AGENT_CHAT_INPUT_PADDING_Y: f32 = 10.0;

    /// Effective visual line height of the Agent Chat composer text.
    const AGENT_CHAT_INPUT_LINE_HEIGHT: f32 = 22.0;

    /// Gap between the active mention line and the picker.
    const AGENT_CHAT_COMPOSER_PICKER_OFFSET_Y: f32 = 4.0;

    /// Composer text size used for the inline Agent Chat input.
    const AGENT_CHAT_INPUT_FONT_SIZE: f32 = 17.0;

    /// The composer inherits GPUI's default window font; measurements must
    /// resolve the same family to stay anchored to real glyph positions.
    const AGENT_CHAT_INPUT_FONT_FAMILY: &'static str = ".SystemUIFont";

    /// One-word focused-text quick prompt placeholder. The input chrome itself
    /// is rendered through the standard Agent Chat composer text renderer.
    const FOCUSED_TEXT_MINI_PLACEHOLDER: &'static str = "Ask";
    const FOCUSED_TEXT_MINI_INPUT_MAX_VISIBLE_HEIGHT: f32 = 44.0;
    const FOCUSED_TEXT_VARIATION_CARD_MIN_HEIGHT: f32 = 96.0;
    const FOCUSED_TEXT_VARIATION_CARD_GAP: f32 = 8.0;
    const FOCUSED_TEXT_VARIATION_AREA_PADDING_Y: f32 = 8.0;
    const FOCUSED_TEXT_VARIATION_AREA_MAX_HEIGHT: f32 = 500.0;

    fn composer_picker_width_for_window(window_width: f32) -> f32 {
        let max_width = (window_width - (Self::AGENT_CHAT_COMPOSER_PICKER_EDGE_GUTTER * 2.0))
            .min(Self::AGENT_CHAT_COMPOSER_PICKER_WIDTH);
        max_width.max(Self::AGENT_CHAT_COMPOSER_PICKER_MIN_WIDTH)
    }

    fn clamp_composer_picker_left(anchor_left: f32, picker_width: f32, window_width: f32) -> f32 {
        let min_left = Self::AGENT_CHAT_INPUT_PADDING_X;
        let max_left = (window_width - picker_width - Self::AGENT_CHAT_COMPOSER_PICKER_EDGE_GUTTER)
            .max(min_left);
        anchor_left.clamp(min_left, max_left)
    }

    /// Measured width of `prefix` at the composer's real font and size.
    /// Per-glyph advances from the text system replace the old flat
    /// 8.5px-per-char estimate that drifted on wide or narrow glyph runs.
    fn measure_agent_chat_input_prefix_width(prefix: &str, cx: &App) -> f32 {
        if prefix.is_empty() {
            return 0.0;
        }

        let text_system = cx.text_system();
        let font_id = text_system.resolve_font(&gpui::font(Self::AGENT_CHAT_INPUT_FONT_FAMILY));
        let font_size = gpui::px(Self::AGENT_CHAT_INPUT_FONT_SIZE);
        prefix
            .chars()
            .map(|ch| f32::from(text_system.layout_width(font_id, font_size, ch)))
            .sum()
    }

    /// Returns the maximum text wrapping width for the Agent Chat composer.
    fn composer_wrap_width_for_window(window_width: f32) -> f32 {
        (window_width - (Self::AGENT_CHAT_INPUT_PADDING_X * 2.0)).max(1.0)
    }

    /// Returns the Agent Chat composer cursor position `(x, y)` after rendering `text`,
    /// accounting for explicit newlines and real word-wrap boundaries from the
    /// text system's line wrapper (the previous char-count modulo ignored
    /// word breaks, so anchors drifted on wrapped lines).
    fn measure_agent_chat_input_cursor_position(
        text: &str,
        window_width: f32,
        cx: &App,
    ) -> (f32, f32) {
        if text.is_empty() {
            return (0.0, 0.0);
        }
        let wrap_width = Self::composer_wrap_width_for_window(window_width);
        let mut wrapper = cx.text_system().line_wrapper(
            gpui::font(Self::AGENT_CHAT_INPUT_FONT_FAMILY),
            gpui::px(Self::AGENT_CHAT_INPUT_FONT_SIZE),
        );
        let logical_lines: Vec<&str> = text.split('\n').collect();
        let mut visual_row = 0usize;
        let mut cursor_x = 0.0f32;
        for (ix, logical_line) in logical_lines.iter().enumerate() {
            let boundaries: Vec<usize> = wrapper
                .wrap_line(
                    &[gpui::LineFragment::text(logical_line)],
                    gpui::px(wrap_width),
                )
                .map(|boundary| boundary.ix)
                .collect();
            if ix + 1 == logical_lines.len() {
                visual_row += boundaries.len();
                let tail_start = boundaries.last().copied().unwrap_or(0);
                cursor_x =
                    Self::measure_agent_chat_input_prefix_width(&logical_line[tail_start..], cx);
            } else {
                visual_row += boundaries.len() + 1;
            }
        }
        (
            cursor_x,
            visual_row as f32 * Self::AGENT_CHAT_INPUT_LINE_HEIGHT,
        )
    }

    /// Returns `(left, top, width)` for the composer picker, anchored to the
    /// trigger character position in the Agent Chat composer, including wrapping.
    fn composer_picker_anchor_for_session(
        &self,
        session: &AgentChatComposerPickerSession,
        input_text: &str,
        window_width: f32,
        cx: &App,
    ) -> (f32, f32, f32) {
        let picker_width = Self::composer_picker_width_for_window(window_width);
        let trigger_start_byte = Self::char_to_byte_offset(input_text, session.trigger_range.start);
        let prefix = &input_text[..trigger_start_byte];
        let trigger_text = match session.trigger {
            AgentChatComposerPickerTrigger::Slash => "/",
            AgentChatComposerPickerTrigger::Profile => PROFILE_TRIGGER_STR,
        };
        let trigger_width = Self::measure_agent_chat_input_prefix_width(trigger_text, cx);
        let (after_trigger_x, after_trigger_y) = Self::measure_agent_chat_input_cursor_position(
            &format!("{prefix}{trigger_text}"),
            window_width,
            cx,
        );
        let unclamped_left =
            Self::AGENT_CHAT_INPUT_PADDING_X + (after_trigger_x - trigger_width).max(0.0);
        let left = Self::clamp_composer_picker_left(unclamped_left, picker_width, window_width);
        let top = Self::AGENT_CHAT_INPUT_PADDING_Y
            + after_trigger_y
            + Self::AGENT_CHAT_INPUT_LINE_HEIGHT
            + Self::AGENT_CHAT_COMPOSER_PICKER_OFFSET_Y;
        (left, top, picker_width)
    }

    /// Compute the visible range of items for a selected index.
    pub(super) fn composer_picker_visible_range_for(
        selected_index: usize,
        item_count: usize,
    ) -> std::ops::Range<usize> {
        crate::components::inline_dropdown::inline_dropdown_visible_range(
            selected_index,
            item_count,
            Self::COMPOSER_PICKER_MAX_VISIBLE,
        )
    }

    /// Compute the visible range of items for the selected index.
    fn composer_picker_visible_range_from_start(
        visible_start: usize,
        selected_index: usize,
        item_count: usize,
    ) -> std::ops::Range<usize> {
        crate::components::inline_dropdown::inline_dropdown_visible_range_from_start(
            visible_start,
            selected_index,
            item_count,
            Self::COMPOSER_PICKER_MAX_VISIBLE,
        )
    }

    /// Compute the visible range of items for the selected index.
    fn composer_picker_visible_range(
        session: &AgentChatComposerPickerSession,
    ) -> std::ops::Range<usize> {
        Self::composer_picker_visible_range_from_start(
            session.visible_start,
            session.selected_index,
            session.items.len(),
        )
    }

    fn ensure_setup_card(
        &mut self,
        state: &super::setup_state::AgentChatInlineSetupState,
        cx: &mut Context<Self>,
    ) -> Entity<AgentChatSetupCard> {
        if let Some(card) = &self.setup_card {
            return card.clone();
        }

        let card = cx.new(|cx| AgentChatSetupCard::new(state.clone(), None, cx));

        cx.subscribe(&card, |this, _card, event, cx| match event {
            AgentChatSetupCardEvent::ConfirmAgent(entry) => {
                this.confirm_setup_agent_selection(entry.clone(), cx);
            }
            AgentChatSetupCardEvent::CancelPicker => {
                this.composer_picker_session = None;
                cx.notify();
            }
            AgentChatSetupCardEvent::ActivateAction(action) => {
                this.handle_setup_action(*action, cx);
            }
        })
        .detach();

        self.setup_card = Some(card.clone());
        card
    }

    fn ensure_toolbar(&mut self, cx: &mut Context<Self>) -> Entity<AgentChatToolbar> {
        if let Some(toolbar) = &self.toolbar {
            return toolbar.clone();
        }

        let thread_ref = self.live_thread().read(cx);
        let status = thread_ref.status;
        let profile_name = thread_ref.profile_display().to_string();
        let model_name = thread_ref.selected_model_display().to_string();

        let toolbar = cx.new(|cx| AgentChatToolbar::new(status, profile_name, model_name, cx));

        cx.subscribe(&toolbar, |this, _toolbar, event, cx| match event {
            AgentChatToolbarEvent::ToggleProfileSelector(parent) => {
                this.composer_parent_window = Some(*parent);
                this.open_profile_trigger_picker(cx);
            }
            AgentChatToolbarEvent::ToggleModelSelector(parent) => {
                this.composer_parent_window = Some(*parent);
                this.sync_agent_chat_popup_windows_from_cached_parent(cx);
                this.trigger_toggle_actions_from_parent(*parent, cx);
                cx.notify();
            }
            AgentChatToolbarEvent::ExportThread => {
                this.export_thread_to_downloads(cx);
            }
            AgentChatToolbarEvent::ClearThread => {
                this.live_thread().update(cx, |thread, cx| {
                    thread.clear_messages(cx);
                });
                if let Some(transcript) = &this.transcript {
                    transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
                }
                cx.notify();
            }
            AgentChatToolbarEvent::OpenHistory => {
                if let Some(parent) = this.composer_parent_window {
                    this.open_history_popup_from_host(
                        parent.handle,
                        parent.bounds,
                        parent.display_id,
                        cx,
                    );
                } else {
                    this.toggle_history_popup_from_cached_parent(cx);
                }
            }
            AgentChatToolbarEvent::CloseChat => {
                crate::ai::agent_chat::ui::chat_window::close_chat_window(cx);
            }
        })
        .detach();

        self.toolbar = Some(toolbar.clone());
        toolbar
    }

    fn ensure_transcript(&mut self, cx: &mut Context<Self>) -> Entity<AgentChatTranscript> {
        let (messages, status, fork_points) = {
            let thread_ref = self.live_thread().read(cx);
            (
                thread_ref.messages.clone(),
                thread_ref.status,
                thread_ref.fork_points().to_vec(),
            )
        };

        let weak_view = cx.entity().downgrade();
        let fork_handler = std::sync::Arc::new(
            move |message_id: u64, window: &mut Window, cx: &mut App| {
                let Some(view) = weak_view.upgrade() else {
                    return;
                };
                view.update(cx, |this, cx| {
                let Some(thread) = this.thread() else {
                    return;
                };
                let mut fork_requested = false;
                thread.update(cx, |thread, cx| {
                    let Some(point) = AgentChatThread::fork_point_for_message_id(
                        &thread.messages,
                        thread.fork_points(),
                        message_id,
                    ) else {
                        thread.push_system_message(
                            "Edit branch unavailable for this message. Try again after the fork list refreshes.",
                            cx,
                        );
                        return;
                    };
                    let entry_id = point.entry_id.clone();
                    if thread.fork_to_message(&entry_id, cx) {
                        fork_requested = true;
                    } else {
                        thread.push_system_message(
                            "Edit branch unavailable right now. Wait for the current turn to finish and try again.",
                            cx,
                        );
                    }
                });
                if fork_requested && !this.focus_handle.is_focused(window) {
                    window.focus(&this.focus_handle, cx);
                    this.cursor_visible = true;
                }
            });
            },
        );

        if let Some(transcript) = &self.transcript {
            transcript.update(cx, |transcript, cx| {
                transcript.set_on_fork_edit_message(fork_handler.clone());
                transcript.set_messages(messages, cx);
                transcript.set_ui_variant(self.ui_variant, cx);
                transcript.set_thread_status(status, cx);
                transcript.set_fork_points(fork_points, cx);
            });
            return transcript.clone();
        }

        let ui_variant = self.ui_variant;
        let transcript =
            cx.new(|cx| AgentChatTranscript::new(messages, cx).with_ui_variant(ui_variant));
        transcript.update(cx, |transcript, cx| {
            transcript.set_on_fork_edit_message(fork_handler);
            transcript.set_thread_status(status, cx);
            transcript.set_fork_points(fork_points, cx);
        });

        cx.subscribe(
            &transcript,
            |this, _transcript, event, cx| match event {
                AgentChatTranscriptEvent::ToggleMessage(_id) => {
                    // Handle message toggle if needed by parent
                }
                AgentChatTranscriptEvent::ForkEditMessage(message_id) => {
                    let Some(thread) = this.thread() else {
                        return;
                    };
                    thread.update(cx, |thread, cx| {
                        let Some(point) = AgentChatThread::fork_point_for_message_id(
                            &thread.messages,
                            thread.fork_points(),
                            *message_id,
                        ) else {
                            thread.push_system_message(
                                "Edit branch unavailable for this message. Try again after the fork list refreshes.",
                                cx,
                            );
                            return;
                        };
                        let entry_id = point.entry_id.clone();
                        if !thread.fork_to_message(&entry_id, cx) {
                            thread.push_system_message(
                                "Edit branch unavailable right now. Wait for the current turn to finish and try again.",
                                cx,
                            );
                        }
                    });
                }
            },
        )
        .detach();

        self.transcript = Some(transcript.clone());
        transcript
    }

    fn confirm_setup_agent_selection(
        &mut self,
        agent: super::catalog::AgentChatAgentCatalogEntry,
        cx: &mut Context<Self>,
    ) {
        let Some(current_setup) = self.read_active_setup_state(cx) else {
            return;
        };

        // Re-resolve against the catalog to rebuild card title/body/actions.
        let resolution = crate::ai::agent_chat::ui::resolve_agent_chat_launch_with_requirements(
            &current_setup.catalog_entries,
            Some(agent.id.as_ref()),
            current_setup.launch_requirements,
        );

        let next_setup = crate::ai::agent_chat::ui::AgentChatInlineSetupState::from_resolution(
            &resolution,
            current_setup.launch_requirements,
        );

        let should_auto_retry = resolution.is_ready();

        if let AgentChatSession::Live(thread) = &self.session {
            thread.update(cx, |thread, cx| {
                thread.replace_selected_agent(Some(agent.clone()), cx);
            });
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_setup_agent_confirmed_for_runtime_recovery",
                agent_id = %agent.id,
                auto_retry = should_auto_retry,
            );
        }

        self.replace_active_setup_state(next_setup, cx);

        if should_auto_retry {
            self.queue_setup_retry_request(cx);
        }
    }

    // ── Key handling ──────────────────────────────────────────────

    /// Whether an active setup card is showing (initial or runtime recovery).
    fn has_active_setup(&self, cx: &mut Context<Self>) -> bool {
        match &self.session {
            AgentChatSession::Setup(_) => true,
            AgentChatSession::Live(thread) => thread.read(cx).setup_state().is_some(),
        }
    }

    /// Take the pending retry request, if any. Used by the Agent Chat open path
    /// to consume an explicit relaunch payload ahead of fallback preference.
    pub(crate) fn take_retry_request(&mut self) -> Option<AgentChatRetryRequest> {
        self.pending_retry_request.take()
    }

    pub(crate) fn has_retry_request(&self) -> bool {
        self.pending_retry_request.is_some()
    }

    /// Stage a history resume request so the next Agent Chat open path loads
    /// the saved conversation instead of starting fresh.
    pub(crate) fn stage_history_resume(&mut self, session_id: String, cx: &mut Context<Self>) {
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_history_resume_staged",
            session_id = %session_id,
        );
        self.pending_history_resume = Some(AgentChatHistoryResumeRequest { session_id });
        cx.notify();
    }

    /// Take the pending history resume request, if any. Used by the Agent Chat
    /// open path to load a saved conversation by session_id.
    pub(crate) fn take_history_resume(&mut self) -> Option<AgentChatHistoryResumeRequest> {
        self.pending_history_resume.take()
    }

    /// Resume a conversation from history by session_id.
    ///
    /// Loads the saved conversation messages into the live thread.
    /// Returns `true` if the conversation was loaded, `false` if the
    /// saved file was not found (falls back to setting input text).
    pub(crate) fn resume_from_history(&mut self, session_id: &str, cx: &mut Context<Self>) -> bool {
        if let Some(conv) = super::history::load_conversation(session_id) {
            self.live_thread().update(cx, |thread, cx| {
                thread.load_saved_messages(&conv.messages, cx);
            });
            if let Some(transcript) = &self.transcript {
                transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
            }
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_item_resumed",
                session_id = %session_id,
                message_count = conv.messages.len(),
            );
            cx.notify();
            true
        } else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_resume_fallback",
                session_id = %session_id,
            );
            false
        }
    }

    /// Resume a saved conversation and deliver a Brain Inbox follow-up prompt.
    ///
    /// Auto-submits the follow-up when the saved conversation loaded, or when
    /// the live thread is still empty (matching the non-chat inbox handoff).
    /// Parks it as a composer draft when resume failed and the thread already
    /// holds a different conversation, so an unrelated chat never receives a
    /// surprise turn. An empty follow-up only loads the conversation.
    pub(crate) fn resume_from_history_with_followup(
        &mut self,
        session_id: &str,
        followup_prompt: &str,
        cx: &mut Context<Self>,
    ) {
        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_followup_ignored_setup_mode",
                session_id = %session_id,
            );
            return;
        }
        let resumed = self.resume_from_history(session_id, cx);
        let followup = followup_prompt.trim();
        if followup.is_empty() {
            return;
        }
        let thread_is_empty = self.live_thread().read(cx).messages.is_empty();
        if resumed || thread_is_empty {
            self.submit_reused_entry_intent(followup.to_string(), cx);
        } else {
            self.set_input(followup.to_string(), cx);
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_followup_parked_resume_missed",
                session_id = %session_id,
            );
        }
    }

    /// Derive current launch requirements from whichever session mode is active.
    fn current_retry_launch_requirements(
        &self,
        cx: &mut Context<Self>,
    ) -> super::preflight::AgentChatLaunchRequirements {
        match &self.session {
            AgentChatSession::Setup(setup) => setup.launch_requirements,
            AgentChatSession::Live(thread) => thread.read(cx).current_setup_requirements(),
        }
    }

    /// Stage a retry request for an action-surface agent switch.
    ///
    /// Preserves the active session's capability requirements so the next
    /// Agent Chat open path can consume them instead of re-deriving from scratch.
    fn current_retry_draft_state(&self, cx: &App) -> Option<AgentChatRetryDraftState> {
        match &self.session {
            AgentChatSession::Live(thread) => {
                let thread = thread.read(cx);
                Some(AgentChatRetryDraftState {
                    input_text: thread.input.text().to_string(),
                    input_cursor: thread.input.cursor(),
                    pending_context_parts: thread.pending_context_parts().to_vec(),
                    pasted_text_tokens: self.pasted_text_tokens.clone(),
                    pasted_image_tokens: self.pasted_image_tokens.clone(),
                    typed_mention_aliases: self.typed_mention_aliases.clone(),
                    inline_owned_context_tokens: self.inline_owned_context_tokens.clone(),
                })
            }
            AgentChatSession::Setup(_) => None,
        }
    }

    pub(crate) fn capture_draft_snapshot(&self, cx: &App) -> AgentChatViewDraftSnapshot {
        AgentChatViewDraftSnapshot {
            thread: self.thread().map(|thread| thread.read(cx).draft_snapshot()),
            pending_portal_session: self.pending_portal_session.clone(),
            pasted_text_tokens: self.pasted_text_tokens.clone(),
            pasted_image_tokens: self.pasted_image_tokens.clone(),
            typed_mention_aliases: self.typed_mention_aliases.clone(),
            inline_owned_context_tokens: self.inline_owned_context_tokens.clone(),
        }
    }

    pub(crate) fn restore_draft_snapshot(
        &mut self,
        snapshot: AgentChatViewDraftSnapshot,
        cx: &mut Context<Self>,
    ) {
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        self.history_menu = None;
        self.attach_menu_open = false;
        self.last_accepted_item = None;
        self.pending_history_resume = None;
        self.pending_portal_session = snapshot.pending_portal_session;
        if let Some(card) = &self.setup_card {
            card.update(cx, |view, cx| view.set_agent_picker(None, cx));
        }
        self.pasted_text_tokens = snapshot.pasted_text_tokens;
        self.pasted_image_tokens = snapshot.pasted_image_tokens;
        self.typed_mention_aliases = snapshot.typed_mention_aliases;
        self.inline_owned_context_tokens = snapshot.inline_owned_context_tokens;

        if let Some(thread_snapshot) = snapshot.thread {
            self.live_thread().update(cx, |thread, cx| {
                thread.restore_draft_snapshot(thread_snapshot, cx);
            });
        }

        self.sync_inline_mentions(cx);
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn restore_retry_draft_state(
        &mut self,
        draft_state: AgentChatRetryDraftState,
        cx: &mut Context<Self>,
    ) {
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::HostHide, cx);
        self.history_menu = None;
        self.attach_menu_open = false;
        self.last_accepted_item = None;
        self.pending_history_resume = None;
        self.pending_portal_session = None;
        self.setup_agent_picker = None;
        self.pasted_text_tokens = draft_state.pasted_text_tokens;
        self.pasted_image_tokens = draft_state.pasted_image_tokens;
        self.typed_mention_aliases = draft_state.typed_mention_aliases;
        self.inline_owned_context_tokens = draft_state.inline_owned_context_tokens;

        let input_text = draft_state.input_text;
        let input_len = input_text.len();
        let input_cursor = draft_state.input_cursor.min(input_text.chars().count());
        let pending_context_parts = draft_state.pending_context_parts;

        self.live_thread().update(cx, move |thread, cx| {
            thread.replace_pending_context_parts(
                pending_context_parts,
                "agent_chat_switch_agent_retry_restore",
                cx,
            );
            thread.input.set_text(input_text.clone());
            thread.input.set_cursor(input_cursor);
            cx.notify();
        });

        self.refresh_composer_picker_state_after_parent_change(cx);

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_switch_agent_retry_draft_restored",
            input_len,
            token_count = self.inline_owned_context_tokens.len(),
        );
        cx.notify();
    }

    /// Queue an explicit relaunch payload from the current setup state.
    /// Called on retry so the next Agent Chat open path reuses the selected agent
    /// and capability requirements instead of re-deriving them.
    fn queue_setup_retry_request(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = self.read_active_setup_state(cx) else {
            return;
        };
        let request = AgentChatRetryRequest::from_setup_state(&setup);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_setup_retry_payload_queued",
            preferred_agent_id = ?request.preferred_agent_id,
            needs_embedded_context = request.launch_requirements.needs_embedded_context,
            needs_image = request.launch_requirements.needs_image,
        );
        self.pending_retry_request = Some(request);
        cx.propagate();
    }

    /// Read the active setup state from either session mode.
    fn read_active_setup_state(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<super::setup_state::AgentChatInlineSetupState> {
        match &self.session {
            AgentChatSession::Setup(setup) => Some((**setup).clone()),
            AgentChatSession::Live(thread) => thread.read(cx).setup_state().cloned(),
        }
    }

    /// Replace the active setup state in whichever session mode is current.
    fn replace_active_setup_state(
        &mut self,
        next: super::setup_state::AgentChatInlineSetupState,
        cx: &mut Context<Self>,
    ) {
        match &mut self.session {
            AgentChatSession::Setup(setup) => {
                **setup = next;
                cx.notify();
            }
            AgentChatSession::Live(thread) => {
                thread.update(cx, |thread, cx| {
                    thread.replace_setup_state(next, cx);
                });
            }
        }
    }

    /// Open the agent selection picker overlay (works in both initial setup
    /// and runtime recovery).
    fn open_setup_agent_picker(&mut self, cx: &mut Context<Self>) {
        let Some(setup) = self.read_active_setup_state(cx) else {
            return;
        };
        if setup.catalog_entries.is_empty() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_setup_agent_picker_empty_catalog",
            );
            return;
        }
        let selected_index = setup
            .selected_agent
            .as_ref()
            .and_then(|selected| {
                setup
                    .catalog_entries
                    .iter()
                    .position(|entry| entry.id == selected.id)
            })
            .unwrap_or(0);

        if let Some(card) = &self.setup_card {
            card.update(cx, |view, cx| {
                view.set_agent_picker(
                    Some(AgentChatSetupAgentPickerState {
                        items: setup.catalog_entries.clone(),
                        selected_index,
                        visible_start: 0,
                    }),
                    cx,
                );
            });
        }

        let compatible_count = setup
            .catalog_entries
            .iter()
            .filter(|entry| entry.satisfies_requirements(setup.launch_requirements))
            .count();

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_setup_agent_picker_opened",
            item_count = 0, // Placeholder
            selected_index,
            compatible_count,
            needs_embedded_context = setup.launch_requirements.needs_embedded_context,
            needs_image = setup.launch_requirements.needs_image,
        );
        cx.notify();
    }

    /// Handle a setup action triggered by the user.
    fn handle_setup_action(
        &mut self,
        action: super::setup_state::AgentChatSetupAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            super::setup_state::AgentChatSetupAction::SelectAgent => {
                self.open_setup_agent_picker(cx);
            }
            super::setup_state::AgentChatSetupAction::Retry => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_setup_retry_requested",
                );
                self.queue_setup_retry_request(cx);
            }
            super::setup_state::AgentChatSetupAction::OpenCatalog => {
                match crate::ai::agent_chat::ui::open_agent_chat_agents_catalog_in_editor() {
                    Ok(path) => {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_setup_open_catalog_requested",
                            path = %path.display(),
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_setup_open_catalog_failed",
                            error = %error,
                        );
                    }
                }
            }
            super::setup_state::AgentChatSetupAction::Install
            | super::setup_state::AgentChatSetupAction::Authenticate => {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_setup_external_action_requested",
                    action = ?action,
                );
            }
        }
    }

    // ── Automation setup action dispatch ─���───────────────────

    /// Perform a setup action from the automation protocol.
    ///
    /// Returns `Ok(())` on success, or an error message if the action
    /// cannot be performed in the current state.
    pub(crate) fn perform_setup_automation_action(
        &mut self,
        action: crate::protocol::AgentChatSetupActionKind,
        agent_id: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        use crate::protocol::AgentChatSetupActionKind;

        match action {
            AgentChatSetupActionKind::OpenAgentPicker => {
                if !self.has_active_setup(cx) {
                    return Err("no active setup card".to_string());
                }
                self.open_setup_agent_picker(cx);
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_setup_action_completed",
                    action = "openAgentPicker",
                    success = true,
                );
                Ok(())
            }
            AgentChatSetupActionKind::CloseAgentPicker => {
                if let Some(card) = &self.setup_card {
                    card.update(cx, |view, cx| view.set_agent_picker(None, cx));
                }
                cx.notify();
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_setup_action_completed",
                    action = "closeAgentPicker",
                    success = true,
                );
                Ok(())
            }
            AgentChatSetupActionKind::SelectAgent => {
                let target_id =
                    agent_id.ok_or_else(|| "selectAgent requires an agentId field".to_string())?;
                if !self.has_active_setup(cx) {
                    return Err("no active setup card".to_string());
                }
                // Open the picker if not already open, select the target agent,
                // then confirm — replicating the user flow deterministically.
                let mut success = false;
                if let Some(card) = &self.setup_card {
                    success = card.update(cx, |view, cx| {
                        if view.select_agent_by_id(target_id, cx) {
                            if let Some(_agent) = view
                                .agent_picker
                                .as_ref()
                                .and_then(|p| p.items.get(p.selected_index).cloned())
                            {
                                // We need to trigger the confirmation.
                                // Instead of a callback, we can just call the method here.
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    });
                }

                if success {
                    // This is a bit hacky because we are bypassing the event emitter,
                    // but it's for the automation path.
                    let Some(setup) = self.read_active_setup_state(cx) else {
                        return Err("no setup".into());
                    };
                    let Some(agent) = setup
                        .catalog_entries
                        .iter()
                        .find(|e| e.id == target_id)
                        .cloned()
                    else {
                        return Err("no agent".into());
                    };
                    self.confirm_setup_agent_selection(agent, cx);
                } else {
                    return Err(format!(
                        "agent '{}' not found or setup card missing",
                        target_id
                    ));
                }
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_setup_action_completed",
                    action = "selectAgent",
                    success = true,
                    selected_agent_id = target_id,
                );
                Ok(())
            }
            AgentChatSetupActionKind::Retry
            | AgentChatSetupActionKind::Install
            | AgentChatSetupActionKind::Authenticate
            | AgentChatSetupActionKind::OpenCatalog => {
                if !self.has_active_setup(cx) {
                    return Err("no active setup card".to_string());
                }
                let internal = super::setup_state::AgentChatSetupAction::from_protocol_kind(action);
                self.handle_setup_action(internal, cx);
                let action_name = format!("{:?}", action);
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_setup_action_completed",
                    action = %action_name,
                    success = true,
                );
                Ok(())
            }
        }
    }

    // ── Test probe methods ────────────────────────────────────

    /// Reset the test probe, clearing all recorded events.
    pub(crate) fn reset_test_probe(&mut self) {
        self.test_probe.event_seq = 0;
        self.test_probe.key_routes.clear();
        self.test_probe.accepted_items.clear();
        self.test_probe.input_layout = None;
        self.test_probe.last_interaction_trace = None;
        tracing::info!(
            target: "script_kit::agent_chat_telemetry",
            event = "agent_chat_test_probe_reset",
        );
    }

    /// Record a key-route event into the test probe ring buffer.
    pub(crate) fn record_key_route(&mut self, event: crate::protocol::AgentChatKeyRouteTelemetry) {
        self.test_probe.event_seq += 1;
        if self.test_probe.key_routes.len() >= AGENT_CHAT_TEST_PROBE_MAX_EVENTS {
            self.test_probe.key_routes.pop_front();
        }
        self.test_probe.key_routes.push_back(event);
    }

    /// Record a picker-acceptance event into the test probe ring buffer.
    pub(crate) fn record_picker_accept(
        &mut self,
        event: crate::protocol::AgentChatPickerItemAcceptedTelemetry,
    ) {
        self.test_probe.event_seq += 1;
        if self.test_probe.accepted_items.len() >= AGENT_CHAT_TEST_PROBE_MAX_EVENTS {
            self.test_probe.accepted_items.pop_front();
        }
        self.test_probe.accepted_items.push_back(event);
    }

    /// Record an input-layout event into the test probe.
    pub(crate) fn record_input_layout(
        &mut self,
        event: crate::protocol::AgentChatInputLayoutTelemetry,
    ) {
        self.test_probe.event_seq += 1;
        self.test_probe.input_layout = Some(event);
    }

    /// Build a bounded snapshot of the test probe for agent queries.
    pub(crate) fn test_probe_snapshot(
        &self,
        tail: usize,
        cx: &gpui::App,
    ) -> crate::protocol::AgentChatTestProbeSnapshot {
        use crate::protocol::AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION;

        let key_routes: Vec<_> = self
            .test_probe
            .key_routes
            .iter()
            .rev()
            .take(tail)
            .rev()
            .cloned()
            .collect();
        let accepted_items: Vec<_> = self
            .test_probe
            .accepted_items
            .iter()
            .rev()
            .take(tail)
            .rev()
            .cloned()
            .collect();

        tracing::info!(
            target: "script_kit::agent_chat_telemetry",
            event = "agent_chat_test_probe_snapshot_requested",
            tail = tail,
            event_seq = self.test_probe.event_seq,
        );

        crate::protocol::AgentChatTestProbeSnapshot {
            schema_version: AGENT_CHAT_TEST_PROBE_SCHEMA_VERSION,
            event_seq: self.test_probe.event_seq,
            key_routes,
            accepted_items,
            input_layout: self.test_probe.input_layout.clone(),
            last_interaction_trace: self.test_probe.last_interaction_trace.clone(),
            state: self.collect_agent_chat_state_snapshot(cx),
            warnings: Vec::new(),
        }
    }
}

struct AgentChatKeyRouteTelemetryArgs {
    route: crate::protocol::AgentChatKeyRoute,
    permission_active: bool,
    cursor_before: usize,
    cursor_after: usize,
    caused_submit: bool,
    consumed: bool,
}

impl AgentChatView {
    // ── Telemetry emission ───────────────────────────────────

    /// Emit structured key-routing telemetry for agentic interactions.
    fn emit_key_route_telemetry(
        &mut self,
        key: &str,
        telemetry_args: AgentChatKeyRouteTelemetryArgs,
    ) {
        let picker_open = self.composer_picker_session.is_some();
        let telemetry = crate::protocol::AgentChatKeyRouteTelemetry {
            key: key.to_string(),
            route: telemetry_args.route.clone(),
            picker_open,
            permission_active: telemetry_args.permission_active,
            cursor_before: telemetry_args.cursor_before,
            cursor_after: telemetry_args.cursor_after,
            caused_submit: telemetry_args.caused_submit,
            consumed: telemetry_args.consumed,
        };
        // Build the interaction trace (no accept info yet — augmented by picker accept if it follows).
        let trace = crate::protocol::AgentChatLastInteractionTrace {
            key: key.to_string(),
            route: format!("{:?}", telemetry_args.route).to_lowercase(),
            picker_open_before: picker_open,
            accepted_via_key: None,
            accepted_label: None,
            cursor_before: telemetry_args.cursor_before,
            cursor_after: telemetry_args.cursor_after,
            caused_submit: telemetry_args.caused_submit,
        };
        self.test_probe.last_interaction_trace = Some(trace);

        // Record into test probe ring buffer.
        self.record_key_route(telemetry.clone());
        let telemetry_json = serde_json::to_string(&telemetry).unwrap_or_default();
        tracing::info!(
            target: "script_kit::agent_chat_telemetry",
            event = "agent_chat_key_routed",
            key = %key,
            route = ?telemetry_args.route,
            picker_open,
            permission_active = telemetry_args.permission_active,
            cursor_before = telemetry_args.cursor_before,
            cursor_after = telemetry_args.cursor_after,
            caused_submit = telemetry_args.caused_submit,
            consumed = telemetry_args.consumed,
            telemetry_json = %telemetry_json,
        );
    }

    /// Emit structured picker-accepted telemetry after a slash/profile item is accepted.
    fn emit_picker_accepted_telemetry(
        &mut self,
        trigger: &str,
        item_label: &str,
        item_id: &str,
        accepted_via_key: &str,
        cursor_after: usize,
        caused_submit: bool,
    ) {
        let telemetry = crate::protocol::AgentChatPickerItemAcceptedTelemetry {
            trigger: trigger.to_string(),
            item_label: item_label.to_string(),
            item_id: item_id.to_string(),
            accepted_via_key: accepted_via_key.to_string(),
            cursor_after,
            caused_submit,
        };
        // Augment the last interaction trace with acceptance info.
        if let Some(ref mut trace) = self.test_probe.last_interaction_trace {
            trace.accepted_via_key = Some(accepted_via_key.to_string());
            trace.accepted_label = Some(item_label.to_string());
            trace.cursor_after = cursor_after;
            trace.caused_submit = caused_submit;
        }

        // Record into test probe ring buffer.
        self.record_picker_accept(telemetry.clone());
        let telemetry_json = serde_json::to_string(&telemetry).unwrap_or_default();
        tracing::info!(
            target: "script_kit::agent_chat_telemetry",
            event = "agent_chat_picker_item_accepted",
            trigger = %trigger,
            item_label = %item_label,
            item_id = %item_id,
            accepted_via_key = %accepted_via_key,
            cursor_after,
            caused_submit,
            telemetry_json = %telemetry_json,
        );

        // Emit a single consolidated interaction trace log event.
        if let Some(ref trace) = self.test_probe.last_interaction_trace {
            tracing::info!(
                target: "script_kit::agent_chat_telemetry",
                event = "agent_chat_interaction_trace",
                trace.key = %trace.key,
                trace.route = %trace.route,
                trace.picker_open_before = trace.picker_open_before,
                trace.accepted_via_key = ?trace.accepted_via_key,
                trace.accepted_label = ?trace.accepted_label,
                trace.cursor_before = trace.cursor_before,
                trace.cursor_after = trace.cursor_after,
                trace.caused_submit = trace.caused_submit,
            );
        }
    }

    /// Emit structured input-layout telemetry after a mutation that may shift the visible window.
    fn emit_input_layout_telemetry(
        &mut self,
        layout: &crate::protocol::AgentChatInputLayoutMetrics,
    ) {
        let telemetry = crate::protocol::AgentChatInputLayoutTelemetry {
            char_count: layout.char_count,
            visible_start: layout.visible_start,
            visible_end: layout.visible_end,
            cursor_in_window: layout.cursor_in_window,
        };
        // Record into test probe.
        self.record_input_layout(telemetry.clone());
        let telemetry_json = serde_json::to_string(&telemetry).unwrap_or_default();
        tracing::info!(
            target: "script_kit::agent_chat_telemetry",
            event = "agent_chat_input_layout",
            char_count = layout.char_count,
            visible_start = layout.visible_start,
            visible_end = layout.visible_end,
            cursor_in_window = layout.cursor_in_window,
            telemetry_json = %telemetry_json,
        );
    }

    fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;

        // Setup mode (initial or runtime recovery): delegate to setup card.
        if let Some(card) = &self.setup_card {
            if card.update(cx, |view, cx| view.handle_key_down(event, cx)) {
                cx.stop_propagation();
                return;
            }
        }
        if self.is_setup_mode() {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_setup_mode_key_propagated_without_live_thread",
                key = %event.keystroke.key,
            );
            cx.propagate();
            return;
        }

        // Reset cursor blink on any key press.
        self.cursor_visible = true;

        // ── Detached actions popup routing ───────────────────────
        // The detached actions window (Cmd+K actions / Cmd+P history route)
        // can stay open while THIS window remains key — the popup keeps the
        // parent-focused contract, e.g. after clicking back into the host or
        // when popup activation fails. Route keys into the popup so arrows,
        // typing, and Enter drive the visible popup instead of leaking into
        // the composer, where Enter silently no-ops.
        if crate::actions::is_actions_window_open()
            && crate::actions::route_key_to_detached_actions_window(
                key,
                event.keystroke.key_char.as_deref(),
                modifiers,
                cx,
            )
        {
            cx.stop_propagation();
            return;
        }

        // ── Inline approval intercept ────────────────────────────
        let pending_permission = self.live_thread().read(cx).pending_permission.clone();
        if let Some(ref request) = pending_permission {
            if self.handle_permission_key_down(event, request, cx) {
                cx.stop_propagation();
                return;
            }
            // Block composer typing while approval is pending, but still allow
            // platform/control/alt shortcuts to propagate.
            if !event.keystroke.modifiers.platform
                && !event.keystroke.modifiers.control
                && !event.keystroke.modifiers.alt
            {
                cx.stop_propagation();
                return;
            }
        }

        if crate::ui_foundation::is_key_escape(key) && self.dismiss_escape_popup(cx) {
            cx.stop_propagation();
            return;
        }
        // ── Attach menu dismiss on Escape ───────────────────────
        if self.attach_menu_open && crate::ui_foundation::is_key_escape(key) {
            self.attach_menu_open = false;
            cx.notify();
            cx.stop_propagation();
            return;
        }
        // Close attach menu on any non-modifier key
        if self.attach_menu_open {
            self.attach_menu_open = false;
            cx.notify();
        }

        // ── Cmd+F → toggle search ────────────────────────────
        if modifiers.platform && key.eq_ignore_ascii_case("f") {
            if self.search_state.is_some() {
                self.search_state = None;
            } else {
                self.search_state = Some((String::new(), 0));
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }

        // ── Search intercept (when search bar is open) ──────
        let search_messages = if self.search_state.is_some() {
            Some(self.live_thread().read(cx).messages.clone())
        } else {
            None
        };
        if let Some((ref mut query, ref mut match_idx)) = self.search_state {
            if crate::ui_foundation::is_key_escape(key) {
                self.search_state = None;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_enter(key) {
                // Enter = next match, Shift+Enter = previous match.
                if !query.is_empty() {
                    let ql = query.to_lowercase();
                    if let Some(messages) = search_messages.as_ref() {
                        let match_indices: Vec<usize> = messages
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| m.body.to_lowercase().contains(&ql))
                            .map(|(i, _)| i)
                            .collect();
                        if !match_indices.is_empty() {
                            let total = match_indices.len();
                            if modifiers.shift {
                                // Previous match (wrap backward).
                                *match_idx = (*match_idx + total - 1) % total;
                            } else {
                                // Next match (wrap forward).
                                *match_idx = (*match_idx + 1) % total;
                            }
                            if let Some(transcript) = &self.transcript {
                                transcript
                                    .read(cx)
                                    .scroll_to_reveal_item(match_indices[*match_idx]);
                            }
                        }
                    }
                }
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_backspace(key) {
                query.pop();
                *match_idx = 0;
                cx.notify();
                cx.stop_propagation();
                return;
            }
            if let Some(ch) = event.keystroke.key_char.as_deref() {
                if !ch.is_empty() && !modifiers.platform && !modifiers.control {
                    query.push_str(ch);
                    *match_idx = 0;
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
            }
        }

        if self.history_menu.is_some() {
            match history_popup_key_intent(key, modifiers) {
                Some(AgentChatHistoryPopupKeyIntent::MoveUp) => {
                    self.navigate_history_popup_selection(-1, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::MoveDown) => {
                    self.navigate_history_popup_selection(1, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::MoveHome) => {
                    self.jump_history_popup_selection(false, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::MoveEnd) => {
                    self.jump_history_popup_selection(true, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::MovePageUp) => {
                    self.page_history_popup_selection(-1, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::MovePageDown) => {
                    self.page_history_popup_selection(1, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::ExecuteSelected) => {
                    self.execute_history_popup_selection(modifiers, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::Close) => {
                    self.dismiss_history_popup(cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::Backspace) => {
                    let next_query = self
                        .history_menu
                        .as_ref()
                        .map(|menu| {
                            let mut query = menu.query.clone();
                            query.pop();
                            query
                        })
                        .unwrap_or_default();
                    self.set_history_popup_query(next_query, cx);
                    cx.stop_propagation();
                    return;
                }
                Some(AgentChatHistoryPopupKeyIntent::TypeChar(ch)) => {
                    let next_query = self
                        .history_menu
                        .as_ref()
                        .map(|menu| format!("{}{}", menu.query, ch))
                        .unwrap_or_else(|| ch.to_string());
                    self.set_history_popup_query(next_query, cx);
                    cx.stop_propagation();
                    return;
                }
                None => {}
            }
        }

        // ── Cmd+K → open actions dialog ──────
        if modifiers.platform && crate::ui_foundation::is_key_k(key) {
            let detached_window_open =
                crate::ai::agent_chat::ui::chat_window::is_chat_window_open();
            let is_detached_host = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
            tracing::debug!(
                target: "script_kit::keyboard",
                event = "agent_chat_cmd_k_route",
                detached_window_open,
                is_detached_host,
                host = if is_detached_host { "detached" } else { "embedded" },
                route = if is_detached_host { "detached_local" } else { "embedded_host_callback" },
            );
            if is_detached_host {
                // Detached window: use the same deferred host callback as the
                // footer button so the AgentChatView update borrow unwinds before
                // the detached actions helper reads the view entity.
                tracing::info!(
                    target: "script_kit::keyboard",
                    event = "detached_actions_shortcut_pressed",
                );
                self.trigger_toggle_actions(window, cx);
                cx.stop_propagation();
            } else {
                // Embedded main-panel Agent Chat: call the host callback directly.
                // The composer owns focus, so bubbling back to the launcher
                // interceptor is not reliable across focus-handle changes.
                self.trigger_toggle_actions(window, cx);
                cx.stop_propagation();
            }
            return;
        }

        if modifiers.platform && key.eq_ignore_ascii_case("w") {
            let is_detached_host = crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
            tracing::info!(
                target: "script_kit::keyboard",
                event = "agent_chat_cmd_w_host_close_requested",
                host = if is_detached_host { "detached" } else { "embedded" },
            );
            // Detached host: on_close_window_requested is unset, so this falls
            // back to on_close_requested (close_chat_window) and closes the
            // detached window — same as the embedded host's window close.
            self.trigger_close_window_requested(window, cx);
            cx.stop_propagation();
            return;
        }

        // ── Cmd+N → start a new thread (both hosts) ──────────────
        if modifiers.platform && !modifiers.shift && !modifiers.alt && key.eq_ignore_ascii_case("n")
        {
            tracing::info!(
                target: "script_kit::keyboard",
                event = "agent_chat_cmd_n_new_thread",
                host = if crate::ai::agent_chat::ui::chat_window::is_chat_window(window) {
                    "detached"
                } else {
                    "embedded"
                },
            );
            self.start_new_thread(cx);
            cx.stop_propagation();
            return;
        }

        if modifiers.platform && modifiers.shift && key.eq_ignore_ascii_case("e") {
            self.toggle_expanded_composer(cx);
            cx.stop_propagation();
            return;
        }

        if modifiers.platform && modifiers.alt && key.eq_ignore_ascii_case("m") {
            self.live_thread()
                .update(cx, |thread, cx| thread.cycle_favorite_model(cx));
            cx.stop_propagation();
            return;
        }

        if modifiers.platform && modifiers.alt && key.eq_ignore_ascii_case("f") {
            self.live_thread().update(cx, |thread, cx| {
                if let Some(model_id) = thread.selected_model_id().map(str::to_string) {
                    thread.toggle_favorite_model(&model_id, cx);
                }
            });
            cx.stop_propagation();
            return;
        }

        // ── Cmd+. → cancel streaming (standard macOS cancel) ──────
        if modifiers.platform && key == "." {
            let is_streaming = matches!(
                self.live_thread().read(cx).status,
                AgentChatThreadStatus::Streaming
            );
            if is_streaming {
                self.live_thread()
                    .update(cx, |thread, cx| thread.cancel_streaming(cx));
                cx.stop_propagation();
                return;
            }
        }

        // ── Cmd+0 → reset Agent Chat zoom/font sizing ───────────
        if modifiers.platform && !modifiers.alt && !modifiers.shift && key == "0" {
            self.reset_agent_chat_zoom(cx);
            cx.stop_propagation();
            return;
        }

        // ── Cmd+Up/Down → jump between user turns ──────────────
        if modifiers.platform && crate::ui_foundation::is_key_up(key) {
            let messages = &self.live_thread().read(cx).messages;
            let current_top = self
                .transcript
                .as_ref()
                .map(|t| t.read(cx).logical_scroll_top().item_ix)
                .unwrap_or(0);
            // Find the user message before the current scroll position
            if let Some(target) = messages[..current_top.saturating_sub(1)]
                .iter()
                .rposition(|m| matches!(m.role, AgentChatThreadMessageRole::User))
            {
                if let Some(transcript) = &self.transcript {
                    transcript.read(cx).scroll_to_reveal_item(target);
                }
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        if modifiers.platform && crate::ui_foundation::is_key_down(key) {
            let messages = &self.live_thread().read(cx).messages;
            let current_top = self
                .transcript
                .as_ref()
                .map(|t| t.read(cx).logical_scroll_top().item_ix)
                .unwrap_or(0);
            // Find the user message after the current scroll position
            let search_start = (current_top + 1).min(messages.len());
            if let Some(offset) = messages[search_start..]
                .iter()
                .position(|m| matches!(m.role, AgentChatThreadMessageRole::User))
            {
                if let Some(transcript) = &self.transcript {
                    transcript
                        .read(cx)
                        .scroll_to_reveal_item(search_start + offset);
                }
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }

        if self.handle_focused_text_variation_editor_key_down(event, window, cx) {
            cx.stop_propagation();
            return;
        }

        // ── Focused-text variations: Up/Down selects stacked result cards ─
        // When the instruction input has text, Up/Down recalls instruction
        // history instead (handled in the next block).
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && !self.focused_text_variations.is_empty()
            && self.focused_text_editing_variation.is_none()
            && !self.scope_focused
            && self.composer_picker_session.is_none()
            && self.live_thread().read(cx).input.is_empty()
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
        {
            if crate::ui_foundation::is_key_up(key)
                && self.move_focused_text_variation_selection(-1, cx)
            {
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_down(key)
                && self.move_focused_text_variation_selection(1, cx)
            {
                cx.stop_propagation();
                return;
            }
        }

        // ── Focused-text instruction history: Up/Down recalls prior prompts ─
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && self.focused_text_editing_variation.is_none()
            && !self.scope_focused
            && self.composer_picker_session.is_none()
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
            && (self.focused_text_variations.is_empty()
                || !self.live_thread().read(cx).input.is_empty())
        {
            if crate::ui_foundation::is_key_up(key)
                && self.recall_focused_text_instruction_history(-1, cx)
            {
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_down(key)
                && self.recall_focused_text_instruction_history(1, cx)
            {
                cx.stop_propagation();
                return;
            }
        }

        // ── Up → recall latest user prompt when composer is empty ─
        if !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
            && crate::ui_foundation::is_key_up(key)
        {
            let recalled = self
                .live_thread()
                .update(cx, |thread, cx| thread.recall_last_user_message(cx));
            if recalled {
                tracing::info!(
                    target: "script_kit::keyboard",
                    event = "agent_chat_plain_up_recalled_last_user_prompt",
                );
                cx.stop_propagation();
                return;
            }
        }

        // ── Cmd+/ → toggle slash command picker ──────────────────
        if modifiers.platform && key == "/" {
            let transition = reduce_agent_chat_composer_picker(
                self.composer_picker_state(),
                AgentChatComposerPickerEvent::SlashToggle,
            );
            let should_refresh = transition.insert_slash_input;
            self.apply_composer_picker_transition(transition, cx);
            if should_refresh {
                self.refresh_agent_chat_spine_from_composer(cx);
                if !self.agent_chat_spine_owns_list() {
                    self.refresh_composer_picker_session(cx);
                }
            }
            cx.stop_propagation();
            return;
        }

        // ── Cmd+Shift+C → copy last response to clipboard ──────
        if modifiers.platform && modifiers.shift && key.eq_ignore_ascii_case("c") {
            let last = self
                .live_thread()
                .read(cx)
                .messages
                .iter()
                .rev()
                .find(|m| matches!(m.role, super::thread::AgentChatThreadMessageRole::Assistant))
                .map(|m| m.body.to_string());
            if let Some(text) = last {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            }
            cx.stop_propagation();
            return;
        }

        // ── Cmd+N → new thread (current keeps streaming in background) ──
        if modifiers.platform && key.eq_ignore_ascii_case("n") {
            self.start_new_thread(cx);
            cx.notify();
            cx.stop_propagation();
            return;
        }

        // ── Cmd+L → new conversation (clear messages, keep session) ──
        if modifiers.platform && key.eq_ignore_ascii_case("l") {
            self.live_thread().update(cx, |thread, cx| {
                thread.clear_messages(cx);
            });
            if let Some(transcript) = &self.transcript {
                transcript.update(cx, |t, cx| t.clear_collapsed_ids(cx));
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }

        // ── Cmd+. / Cmd+Shift+O → reopen focused mention in its portal ───
        if Self::is_reopen_focused_mention_shortcut(key, modifiers)
            && self.open_focused_mention_portal(cx)
        {
            cx.stop_propagation();
            return;
        }

        // ── Cmd+P → open dedicated history command surface ──────────
        if modifiers.platform && key.eq_ignore_ascii_case("p") {
            tracing::info!(event = "agent_chat_history_shortcut_routed_to_command");
            self.trigger_open_history_command(window, cx);
            cx.stop_propagation();
            return;
        }

        if self.focused_text.is_some()
            && self.ui_variant != AgentChatUiVariant::FocusedTextMini
            && modifiers.platform
            && !modifiers.control
            && !modifiers.alt
        {
            let focused_action = if !modifiers.shift && key.eq_ignore_ascii_case("r") {
                Some(FocusedTextMiniAction::Replace)
            } else if !modifiers.shift && key.eq_ignore_ascii_case("a") {
                Some(FocusedTextMiniAction::Append)
            } else if !modifiers.shift && key.eq_ignore_ascii_case("c") {
                Some(FocusedTextMiniAction::Copy)
            } else if modifiers.shift && key.eq_ignore_ascii_case("m") {
                Some(FocusedTextMiniAction::Expand)
            } else if modifiers.shift && key.eq_ignore_ascii_case("r") {
                Some(FocusedTextMiniAction::Retry)
            } else {
                None
            };

            if let Some(action) = focused_action {
                self.perform_focused_text_mini_action(action, cx);
                cx.stop_propagation();
                return;
            }
        }

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && self.focused_text_input_locked_for_thread(self.live_thread().read(cx))
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !Self::focused_text_locked_input_allows_key(key)
        {
            tracing::debug!(
                target: "script_kit::focused_text",
                event = "focused_text_locked_input_key_blocked",
                key = %key,
            );
            cx.stop_propagation();
            return;
        }

        if self.handle_agent_chat_spine_key_down(event, window, cx) {
            cx.stop_propagation();
            return;
        }

        // ── Unified picker intercept (slash/profile commands) ─────
        if self.composer_picker_session.is_some() {
            if crate::ui_foundation::is_key_up(key) {
                let transition = reduce_agent_chat_composer_picker(
                    self.composer_picker_state(),
                    AgentChatComposerPickerEvent::NavigatePrevious,
                );
                self.apply_composer_picker_transition(transition, cx);
                if let Some(session) = self.composer_picker_session.as_ref() {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_mention_selection_changed",
                        direction = "prev",
                        selected_index = session.selected_index,
                        item_count = session.items.len(),
                    );
                }
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_down(key) {
                let transition = reduce_agent_chat_composer_picker(
                    self.composer_picker_state(),
                    AgentChatComposerPickerEvent::NavigateNext,
                );
                self.apply_composer_picker_transition(transition, cx);
                if let Some(session) = self.composer_picker_session.as_ref() {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_mention_selection_changed",
                        direction = "next",
                        selected_index = session.selected_index,
                        item_count = session.items.len(),
                    );
                }
                cx.stop_propagation();
                return;
            }
            if (crate::ui_foundation::is_key_enter(key)
                || (crate::ui_foundation::is_key_tab(key) && !modifiers.shift))
                && self.handle_picker_accept_key(key, cx)
            {
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_escape(key) {
                let transition = reduce_agent_chat_composer_picker(
                    self.composer_picker_state(),
                    AgentChatComposerPickerEvent::Dismiss {
                        reason: AgentChatComposerPickerDismissReason::Escape,
                        cursor: self.live_thread().read(cx).input.cursor(),
                    },
                );
                self.apply_composer_picker_transition(transition, cx);
                cx.stop_propagation();
                return;
            }
            // Other keys fall through to normal input handling,
            // which will update the query text and refresh the session.
        }

        if crate::ui_foundation::is_key_tab(key)
            && self.handle_focused_text_variation_tab(modifiers.shift, cx)
        {
            cx.stop_propagation();
            return;
        }

        if crate::ui_foundation::is_key_tab(key)
            && self.handle_focused_text_scope_tab(modifiers.shift, cx)
        {
            cx.stop_propagation();
            return;
        }

        if self.handle_focused_text_scope_key_down(event, cx) {
            cx.stop_propagation();
            return;
        }

        // Shift+Enter inserts a newline.
        if crate::ui_foundation::is_key_enter(key) && modifiers.shift {
            self.live_thread().update(cx, |thread, cx| {
                thread.input.insert_char('\n');
                cx.notify();
            });
            self.refresh_agent_chat_spine_from_composer(cx);
            if !self.agent_chat_spine_owns_list() {
                self.refresh_composer_picker_session(cx);
            }
            cx.stop_propagation();
            return;
        }

        // Escape with no open dialogs unwinds focused-text mini state
        // progressively before falling back to the normal Agent Chat behavior.
        if crate::ui_foundation::is_key_escape(key) {
            if self.is_focused_text_mini() || self.focused_text_originated_from_quick_prompt() {
                let (phase, input_has_text) = {
                    let thread = self.live_thread().read(cx);
                    (
                        self.focused_text_mini_phase_for_thread(thread),
                        !thread.input.text().is_empty() || !self.scope_input.is_empty(),
                    )
                };

                let action = match phase {
                    Some(FocusedTextMiniPhase::InputOnly) if input_has_text => "clear_input",
                    Some(FocusedTextMiniPhase::InputOnly) => "close_empty_input",
                    Some(FocusedTextMiniPhase::Loading) => "cancel_loading",
                    Some(FocusedTextMiniPhase::Streaming) => "stop_streaming",
                    Some(FocusedTextMiniPhase::Result) => "close_result",
                    Some(FocusedTextMiniPhase::Error) => "close_error",
                    None => "close_non_mini_focused_text",
                };

                tracing::info!(
                    target: "script_kit::keyboard",
                    event = "focused_text_escape_progressive",
                    ui_variant = self.ui_variant.state_id(),
                    phase = phase.map(FocusedTextMiniPhase::state_id).unwrap_or("unknown"),
                    action = action,
                );

                match phase {
                    Some(FocusedTextMiniPhase::InputOnly) if input_has_text => {
                        self.scope_input.clear();
                        self.scope_visible = false;
                        self.scope_focused = false;
                        self.live_thread().update(cx, |thread, cx| {
                            thread.input.clear();
                            cx.notify();
                        });
                        self.resize_focused_text_mini_for_scope_change(&*cx);
                    }
                    Some(FocusedTextMiniPhase::InputOnly) => {
                        self.trigger_close_window_requested(window, cx);
                    }
                    Some(FocusedTextMiniPhase::Loading) => {
                        let _ = self.cancel_streaming_from_escape(cx);
                        self.scope_input.clear();
                        self.scope_visible = false;
                        self.scope_focused = false;
                        self.live_thread().update(cx, |thread, cx| {
                            thread.input.clear();
                            cx.notify();
                        });
                        self.resize_focused_text_mini_for_scope_change(&*cx);
                    }
                    Some(FocusedTextMiniPhase::Streaming) => {
                        let _ = self.cancel_streaming_from_escape(cx);
                    }
                    Some(FocusedTextMiniPhase::Result | FocusedTextMiniPhase::Error) => {
                        self.trigger_close_window_requested(window, cx);
                    }
                    None => {
                        let _ = self.cancel_streaming_from_escape(cx);
                        self.trigger_close_window_requested(window, cx);
                    }
                }

                cx.stop_propagation();
                return;
            }
            if self.cancel_streaming_from_escape(cx) {
                cx.stop_propagation();
                return;
            }
            tracing::info!(
                target: "script_kit::keyboard",
                event = "embedded_agent_chat_escape_host_close_requested",
            );
            self.trigger_close_requested(window, cx);
            cx.stop_propagation();
            return;
        }

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && key.eq_ignore_ascii_case("r")
            && modifiers.platform
            && !modifiers.shift
            && !self.focused_text_variations.is_empty()
            && self.focused_text_editing_variation.is_none()
            && !self.scope_focused
            && self.composer_picker_session.is_none()
        {
            self.regenerate_focused_text_variations(cx);
            cx.stop_propagation();
            return;
        }

        // ── ⌘E opens the manual editor on the selected variation ──
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && key.eq_ignore_ascii_case("e")
            && modifiers.platform
            && !modifiers.shift
            && !modifiers.control
            && !modifiers.alt
            && !self.focused_text_variations.is_empty()
            && self.focused_text_editing_variation.is_none()
            && !self.scope_focused
            && self.composer_picker_session.is_none()
            && self.enter_focused_text_variation_editor(cx)
        {
            cx.stop_propagation();
            return;
        }

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && !self.focused_text_variations.is_empty()
            && self.focused_text_editing_variation.is_none()
            && !self.scope_focused
            && self.composer_picker_session.is_none()
            && modifiers.platform
            && !modifiers.shift
            && !modifiers.control
            && !modifiers.alt
        {
            if crate::ui_foundation::is_key_left(key)
                && self.navigate_focused_text_variation_history(-1, cx)
            {
                cx.stop_propagation();
                return;
            }
            if crate::ui_foundation::is_key_right(key)
                && self.navigate_focused_text_variation_history(1, cx)
            {
                cx.stop_propagation();
                return;
            }
        }

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && crate::ui_foundation::is_key_enter(key)
            && modifiers.platform
            && !modifiers.shift
        {
            self.apply_focused_text_output(
                crate::ai::focused_text::FocusedTextApplyAction::Replace,
                cx,
            );
            cx.stop_propagation();
            return;
        }

        // ── Mini result phase: plain Enter with an empty input pastes the
        // selected variation back into the source app and dismisses the mini.
        // (Manual editing of a variation moved to ⌘E.)
        if self.ui_variant == AgentChatUiVariant::FocusedTextMini
            && self.focused_text.is_some()
            && crate::ui_foundation::is_key_enter(key)
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
            && !self.scope_focused
            && self.composer_picker_session.is_none()
            && self.focused_text_editing_variation.is_none()
        {
            let (input_empty, result_ready, has_output) = {
                let thread = self.live_thread().read(cx);
                (
                    thread.input.text().trim().is_empty(),
                    self.focused_text_mini_result_ready_for_thread(thread),
                    self.selected_focused_text_output(thread).is_some(),
                )
            };
            // A completed selected variation is pastable even while the other
            // angles are still streaming — Enter must not cancel the stream
            // out from under a user who already picked their rewrite.
            let selected_variation_complete = self
                .focused_text_selected_variation
                .and_then(|index| self.focused_text_variations.get(index))
                .map(|variation| {
                    variation.status == FocusedTextVariationStatus::Complete
                        && !variation.text.trim().is_empty()
                })
                .unwrap_or(false);
            if input_empty && (selected_variation_complete || (result_ready && has_output)) {
                let receipt = self.apply_focused_text_output(
                    crate::ai::focused_text::FocusedTextApplyAction::Replace,
                    cx,
                );
                if receipt.success {
                    self.trigger_close_window_requested(window, cx);
                }
                cx.stop_propagation();
                return;
            }
        }

        if self.focused_text.is_some()
            && crate::ui_foundation::is_key_enter(key)
            && !modifiers.platform
            && !modifiers.shift
        {
            if let Err(error) = self.submit_focused_text_from_enter(cx) {
                tracing::warn!(
                    target: "script_kit::focused_text",
                    event = "focused_text_submit_failed",
                    error = %error,
                );
            }
            cx.stop_propagation();
            return;
        }

        // Enter submits.
        if crate::ui_foundation::is_key_enter(key) && !modifiers.shift {
            let cursor_before = self.live_thread().read(cx).input.cursor();
            let permission_active = self.live_thread().read(cx).pending_permission.is_some();
            let should_paste_response = {
                let thread = self.live_thread().read(cx);
                thread.input.text().is_empty()
                    && matches!(
                        thread.status,
                        AgentChatThreadStatus::Idle | AgentChatThreadStatus::Error
                    )
                    && Self::has_pastable_assistant_response(thread)
            };
            if should_paste_response {
                self.trigger_paste_response_requested(window, cx);
                self.emit_key_route_telemetry(
                    key,
                    AgentChatKeyRouteTelemetryArgs {
                        route: crate::protocol::AgentChatKeyRoute::Composer,
                        cursor_before,
                        cursor_after: cursor_before,
                        caused_submit: false,
                        consumed: true,
                        permission_active,
                    },
                );
                cx.stop_propagation();
                return;
            }
            let transition = reduce_agent_chat_composer_picker(
                self.composer_picker_state(),
                AgentChatComposerPickerEvent::SubmitStarted,
            );
            self.apply_composer_picker_transition(transition, cx);
            self.submit_with_expanded_tokens(cx);
            self.emit_key_route_telemetry(
                key,
                AgentChatKeyRouteTelemetryArgs {
                    route: crate::protocol::AgentChatKeyRoute::Composer,
                    cursor_before,
                    cursor_after: 0,
                    caused_submit: true,
                    consumed: true,
                    permission_active,
                },
            );
            cx.stop_propagation();
            return;
        }

        if modifiers.platform
            && key.eq_ignore_ascii_case("v")
            && (self.paste_image_from_clipboard(cx) || self.paste_text_from_clipboard(cx))
        {
            self.refresh_agent_chat_spine_from_composer(cx);
            if !self.agent_chat_spine_owns_list() {
                self.refresh_composer_picker_session(cx);
            }
            cx.stop_propagation();
            return;
        }

        // ── Token-atomic inline mention deletion ──────────────
        // When backspace/delete lands inside, at the trailing edge, or at
        // the leading edge of an inline @mention token, remove the whole
        // token plus one trailing space (when present) instead of deleting
        // a single character.
        if crate::ui_foundation::is_key_backspace(key) || crate::ui_foundation::is_key_delete(key) {
            let current_text = self.live_thread().read(cx).input.text().to_string();
            let cursor = self.live_thread().read(cx).input.cursor();

            if let Some((next_text, next_cursor)) =
                crate::pasted_text::remove_pasted_text_token_at_cursor(
                    &current_text,
                    cursor,
                    crate::ui_foundation::is_key_delete(key),
                    &mut self.pasted_text_tokens,
                )
            {
                self.live_thread().update(cx, |thread, cx| {
                    thread.input.set_text(next_text);
                    thread.input.set_cursor(next_cursor);
                    cx.notify();
                });
                self.refresh_agent_chat_spine_from_composer(cx);
                if !self.agent_chat_spine_owns_list() {
                    self.refresh_composer_picker_session(cx);
                }
                self.sync_pasted_clipboard_tokens(cx);
                self.sync_inline_mentions(cx);
                cx.notify();
                self.check_for_transient_exit(window, cx);
                cx.stop_propagation();
                return;
            }

            if let Some((next_text, next_cursor)) =
                crate::pasted_image::remove_pasted_image_token_at_cursor(
                    &current_text,
                    cursor,
                    crate::ui_foundation::is_key_delete(key),
                    &mut self.pasted_image_tokens,
                )
            {
                self.live_thread().update(cx, |thread, cx| {
                    thread.input.set_text(next_text);
                    thread.input.set_cursor(next_cursor);
                    cx.notify();
                });
                self.refresh_agent_chat_spine_from_composer(cx);
                if !self.agent_chat_spine_owns_list() {
                    self.refresh_composer_picker_session(cx);
                }
                self.sync_pasted_clipboard_tokens(cx);
                self.sync_inline_mentions(cx);
                cx.notify();
                self.check_for_transient_exit(window, cx);
                cx.stop_propagation();
                return;
            }

            if let Some((next_text, next_cursor)) =
                crate::ai::context_mentions::remove_inline_mention_at_cursor_with_aliases(
                    &current_text,
                    cursor,
                    crate::ui_foundation::is_key_delete(key),
                    &self.typed_mention_aliases,
                )
            {
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "agent_chat_inline_mention_deleted_atomically",
                    key = %key,
                    cursor,
                    next_cursor,
                );

                self.live_thread().update(cx, |thread, cx| {
                    thread.input.set_text(next_text);
                    thread.input.set_cursor(next_cursor);
                    cx.notify();
                });
                self.refresh_agent_chat_spine_from_composer(cx);
                if !self.agent_chat_spine_owns_list() {
                    self.refresh_composer_picker_session(cx);
                }
                self.sync_inline_mentions(cx);
                cx.notify();
                self.check_for_transient_exit(window, cx);
                cx.stop_propagation();
                return;
            }
        }

        // Delegate all other keys to TextInputState::handle_key().
        // handle_key requires T: Render, so we extract input, mutate it here,
        // then write it back.
        let key_char = event.keystroke.key_char.as_deref();
        let mut input_snapshot = self.live_thread().read(cx).input.clone();
        let handled = input_snapshot.handle_key(
            key,
            key_char,
            modifiers.platform,
            modifiers.alt,
            modifiers.shift,
            cx,
        );

        if handled {
            if self.ui_variant == AgentChatUiVariant::FocusedTextMini
                && self.focused_text.is_some()
                && !crate::ui_foundation::is_key_up(key)
                && !crate::ui_foundation::is_key_down(key)
            {
                self.reset_focused_text_instruction_history_navigation();
            }
            self.live_thread().update(cx, |thread, cx| {
                thread.input = input_snapshot;
                cx.notify();
            });
            self.sync_pasted_clipboard_tokens(cx);
            self.refresh_agent_chat_spine_from_composer(cx);
            if !self.agent_chat_spine_owns_list() {
                self.refresh_composer_picker_session(cx);
            }
            self.sync_inline_mentions(cx);
            self.check_for_transient_exit(window, cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
}

impl Focusable for AgentChatView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AgentChatView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_host_activation_subscription(window, cx);
        self.sync_host_window_state(window, cx);

        // Setup mode: render the inline setup card instead of the chat.
        let setup_state = if let AgentChatSession::Setup(state) = &self.session {
            Some(state.clone())
        } else {
            None
        };
        if let Some(state) = setup_state {
            let setup_card = self.ensure_setup_card(&state, cx);
            return setup_card.into_any_element();
        }

        // Runtime setup recovery: if the live thread received a SetupRequired
        // event, show the setup card instead of the errored chat transcript.
        {
            let thread_ref = self.live_thread().read(cx);
            if let Some(setup) = thread_ref.setup_state().cloned() {
                let setup_card = self.ensure_setup_card(&setup, cx);
                return setup_card.into_any_element();
            }
        }

        let thread = self.live_thread().read(cx);
        let show_activity_row = thread.awaiting_first_assistant_text();
        let is_empty = thread.messages.is_empty() && !show_activity_row;
        let input_text = thread.input.text().to_string();
        let input_cursor = thread.input.cursor();
        let input_selection = thread.input.selection();
        let composer_active = Self::composer_is_active(
            window.is_window_active(),
            self.focus_handle.is_focused(window),
            crate::actions::is_actions_window_open(),
        );
        let cursor_visible = self.cursor_visible && composer_active;
        let pending_permission = thread.pending_permission.clone();
        let plan_entries = thread.active_plan_entries().to_vec();
        let attached_parts = thread.pending_context_parts().to_vec();
        let messages: Vec<AgentChatThreadMessage> = thread.messages.clone();
        let history_popup_open = self.history_menu.is_some();
        let _colors = Self::prompt_colors();
        let theme = theme::get_cached_theme();
        let menu_def = crate::designs::current_main_menu_theme().def();
        let chrome = AppChromeColors::from_theme(&theme);
        let placeholder_text = rgba(chrome.placeholder_text_rgba);
        let mention_accent = theme.colors.accent.selected;
        let mut mention_highlights = Self::attached_inline_mention_highlight_ranges(
            &input_text,
            &attached_parts,
            mention_accent,
            &self.typed_mention_aliases,
        );
        if let Some(slash_hl) = Self::leading_slash_highlight_range(&input_text, mention_accent) {
            mention_highlights.push(slash_hl);
        }
        let mut pasted_text_pills = self.pasted_text_pill_ranges(&input_text);
        pasted_text_pills.extend(self.pasted_image_pill_ranges(&input_text));
        pasted_text_pills.sort_by_key(|pill| pill.start);
        let pending_permission_has_message_target = pending_permission
            .as_ref()
            .and_then(Self::permission_request_tool_call_id)
            .is_some_and(|tool_call_id| {
                messages
                    .iter()
                    .any(|msg| msg.tool_call_id.as_deref() == Some(tool_call_id))
            });
        let view_entity: WeakEntity<AgentChatView> = cx.entity().downgrade();
        let ui_variant = self.ui_variant;
        let variant_config = ui_variant.config();
        let status = thread.status;
        let status_label = Self::agent_chat_thread_status_label(status);
        let context_chip_count = attached_parts.len();
        let message_count = messages.len();
        let profile_icon_name = thread.profile_icon_name().map(str::to_string);
        let profile_active_pending = matches!(
            thread.status,
            AgentChatThreadStatus::Streaming | AgentChatThreadStatus::WaitingForPermission
        ) || show_activity_row;

        if self.ui_variant == AgentChatUiVariant::FocusedTextMini {
            let focused_phase = self.focused_text_mini_phase_for_thread(thread);
            let active_pending = matches!(
                focused_phase,
                Some(FocusedTextMiniPhase::Loading | FocusedTextMiniPhase::Streaming)
            );
            let show_transcript = matches!(
                focused_phase,
                Some(FocusedTextMiniPhase::Streaming | FocusedTextMiniPhase::Result)
            );
            let input_locked = self.focused_text_input_locked_for_thread(thread);
            let display_input_text = if input_locked {
                Self::latest_user_prompt_for_display(thread).unwrap_or_default()
            } else {
                input_text.clone()
            };
            let display_input_cursor = if input_locked {
                display_input_text.chars().count()
            } else {
                input_cursor
            };
            let display_input_selection = if input_locked {
                TextSelection::caret(display_input_cursor)
            } else {
                input_selection
            };
            let _ = thread;

            let mut focused_text_cursor_visible = cursor_visible;
            if self.pending_focused_text_mini_focus_restore && !input_locked {
                self.pending_focused_text_mini_focus_restore = false;
                if !crate::actions::is_actions_window_open() {
                    window.focus(&self.focus_handle, cx);
                    self.cursor_visible = true;
                    focused_text_cursor_visible = true;
                    tracing::info!(
                        target: "script_kit::focused_text",
                        event = "focused_text_mini_input_focus_restored",
                        phase = ?focused_phase,
                    );
                }
            }

            let variations = self.focused_text_variation_snapshots();
            let transcript = if show_transcript && variations.is_empty() {
                Some(self.ensure_transcript(cx).into_any_element())
            } else {
                None
            };

            return div()
                .size_full()
                .relative()
                .track_focus(&self.focus_handle)
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                    this.cache_composer_parent_window(window, cx);
                    this.handle_key_down(event, window, cx);
                }))
                .on_any_mouse_down(cx.listener(|this, _event, _window, cx| {
                    this.dismiss_composer_picker(cx);
                }))
                .child(self.render_focused_text_mini(
                    active_pending,
                    show_transcript,
                    profile_icon_name.as_deref(),
                    view_entity.clone(),
                    transcript,
                    variations,
                    &display_input_text,
                    display_input_cursor,
                    display_input_selection,
                    focused_text_cursor_visible,
                    input_locked,
                    placeholder_text,
                    &theme,
                ))
                .into_any_element();
        }

        let root = crate::components::main_view_chrome::render_main_view_shell()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let modifiers = &event.keystroke.modifiers;
                this.cache_composer_parent_window(window, cx);

                // Cmd+W in detached window: close the window directly.
                // In the main panel, Cmd+W is handled by the interceptor.
                let is_detached_host =
                    crate::ai::agent_chat::ui::chat_window::is_chat_window(window);
                if modifiers.platform && key.eq_ignore_ascii_case("w") && is_detached_host {
                    tracing::info!(
                        target: "script_kit::keyboard",
                        event = "detached_agent_chat_cmd_w_close_requested",
                    );
                    let wb = window.window_bounds();
                    crate::window_state::save_window_from_gpui(
                        crate::window_state::WindowRole::AgentChat,
                        wb,
                    );
                    this.prepare_for_host_hide(cx);
                    crate::ai::agent_chat::ui::chat_window::clear_chat_window_handle();
                    window.remove_window();
                    cx.stop_propagation();
                    return;
                }

                this.handle_key_down(event, window, cx);
            }))
            .on_any_mouse_down(cx.listener(|this, _event, _window, cx| {
                this.dismiss_composer_picker(cx);
            }));

        if matches!(variant_config.composer, AgentChatComposerPlacement::Default) {
            let input = Self::render_composer_input_shell(
                &input_text,
                input_cursor,
                input_selection,
                cursor_visible,
                is_empty,
                &mention_highlights,
                &pasted_text_pills,
                placeholder_text,
                profile_icon_name.as_deref(),
                profile_active_pending,
                status,
                view_entity.clone(),
                &theme,
                self.expanded_composer,
            );
            let footer_snapshot = self.footer_snapshot(cx);
            let context_labels = crate::components::main_view_chrome::MainViewContextLabels::new(
                footer_snapshot
                    .cwd_display
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| {
                        crate::components::main_view_chrome::MAIN_VIEW_CWD_UNAVAILABLE_LABEL
                            .to_string()
                    }),
                footer_snapshot.agent_model_header_label(),
            )
            // In Agent Chat plain Tab is owned locally (picker accept /
            // swallowed) and never opens the cwd picker — hide the ⇥ keycap.
            // Shift+Tab does open the in-chat profile picker, so ⇧⇥ stays.
            .with_tab_action(crate::components::main_view_chrome::MainViewTabChipAction::Inactive);
            let header = crate::components::main_view_chrome::MainViewHeaderChrome {
                context: Some(
                    crate::components::main_view_chrome::render_main_view_context_zone_required(
                        &theme,
                        menu_def,
                        context_labels,
                        None,
                        |_event, _window, _cx| {},
                        |_event, _window, _cx| {},
                        |_event, _window, _cx| {},
                    ),
                ),
                input,
                padding_x: menu_def.shell.header_padding_x,
                padding_y: menu_def.shell.header_padding_y,
                gap: menu_def.shell.header_gap,
            };
            let divider = crate::components::main_view_chrome::MainViewDividerChrome {
                margin_x: menu_def.shell.divider_margin_x,
                height: menu_def.shell.divider_height,
                visible: false,
            };

            let mut pre_main = Vec::new();
            if variant_config.show_variant_badge {
                pre_main.push(Self::render_variant_badge(ui_variant, &theme));
            }
            if let Some(preview) = self.focused_inline_mention_preview(cx) {
                pre_main.push(
                    div()
                        .w_full()
                        .px(px(12.0))
                        .pb(px(4.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme.colors.text.muted))
                                .child(preview.token)
                                .child(" ")
                                .child(preview.detail),
                        )
                        .into_any_element(),
                );
            }
            pre_main.push(Self::render_reserved_transient_lane(
                "agent_chat-context-bootstrap-lane",
                AGENT_CHAT_TRANSIENT_BOOTSTRAP_LANE_HEIGHT_PX,
                if self.context_bootstrap_note_lane_active(cx) {
                    Some(self.render_context_bootstrap_note(cx))
                } else {
                    None
                },
            ));
            pre_main.push(Self::render_reserved_transient_lane(
                "agent_chat-message-queue-lane-top",
                AGENT_CHAT_TRANSIENT_QUEUE_LANE_HEIGHT_PX,
                if self.message_queue_lane_active(cx) {
                    Some(self.render_message_queue_strip(cx))
                } else {
                    None
                },
            ));
            pre_main.push(self.render_active_callout(cx));
            if let Some((query, current_idx)) = self.search_state.clone() {
                let match_count = if query.is_empty() {
                    0
                } else {
                    let q = query.to_lowercase();
                    messages
                        .iter()
                        .filter(|m| m.body.to_lowercase().contains(&q))
                        .count()
                };
                let display_idx = if match_count > 0 {
                    (current_idx % match_count) + 1
                } else {
                    0
                };
                pre_main.push(
                    div()
                        .w_full()
                        .px(px(12.0))
                        .py(px(4.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(div().text_xs().opacity(0.50).child("\u{1F50D}"))
                        .child(div().flex_grow().text_sm().child(if query.is_empty() {
                            "Search conversation\u{2026}".to_string()
                        } else {
                            query.clone()
                        }))
                        .when(!query.is_empty(), |d| {
                            d.child(div().text_xs().opacity(0.45).child(if match_count > 0 {
                                format!("{display_idx}/{match_count}")
                            } else {
                                "0 matches".to_string()
                            }))
                        })
                        .when(match_count > 1, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .opacity(0.30)
                                    .child("\u{21A9} next \u{00b7} \u{21E7}\u{21A9} prev"),
                            )
                        })
                        .child(div().text_xs().opacity(0.25).child("esc \u{00d7}"))
                        .into_any_element(),
                );
            }

            let middle_area = self.render_agent_chat_middle_area(
                is_empty,
                variant_config.show_sidecar,
                ui_variant,
                status_label,
                message_count,
                context_chip_count,
                view_entity.clone(),
                &theme,
                cx,
            );

            let mut post_main = Vec::new();
            post_main.push(Self::render_reserved_transient_lane(
                "agent_chat-plan-strip-lane",
                AGENT_CHAT_TRANSIENT_PLAN_LANE_HEIGHT_PX,
                if plan_entries.is_empty() {
                    None
                } else {
                    Some(
                        div()
                            .w_full()
                            .px(px(8.0))
                            .pb(px(4.0))
                            .child(Self::render_plan_strip(&plan_entries))
                            .into_any_element(),
                    )
                },
            ));
            post_main.push(Self::render_reserved_transient_lane(
                "agent_chat-permission-card-lane",
                AGENT_CHAT_TRANSIENT_PERMISSION_LANE_HEIGHT_PX,
                pending_permission
                    .clone()
                    .filter(|_| !pending_permission_has_message_target)
                    .map(|request| {
                        div()
                            .w_full()
                            .px(px(8.0))
                            .pb(px(4.0))
                            .child(Self::render_permission_inline_card(
                                &request,
                                self.permission_index,
                                self.permission_options_open,
                                view_entity.clone(),
                            ))
                            .into_any_element()
                    }),
            ));

            let main = div()
                .id("agent_chat-conversation")
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .h_full()
                .overflow_hidden()
                .flex()
                .flex_col()
                .children(pre_main)
                .child(middle_area)
                .children(post_main)
                .into_any_element();

            let mut overlays = Vec::new();
            if self.attach_menu_open {
                overlays.push(self.render_attach_menu(cx));
            }
            if history_popup_open {
                overlays.push(
                    div()
                        .id("agent_chat-history-popup-backdrop")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom(px(self.inline_footer_height()))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.dismiss_history_popup(cx);
                                cx.stop_propagation();
                            }),
                        )
                        .into_any_element(),
                );
            }

            let footer = if self.uses_external_footer_host() {
                None
            } else {
                let is_main_window = crate::get_main_window_handle()
                    .is_some_and(|handle| handle == window.window_handle());

                #[cfg(target_os = "macos")]
                {
                    if !is_main_window {
                        self.ensure_native_footer_action_listener(window, cx);
                        crate::footer_popup::sync_window_footer_popup(
                            window,
                            &self.agent_chat_detached_native_footer_config(cx),
                        );
                        Some(
                            crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(),
                        )
                    } else {
                        let active_surface =
                            crate::footer_popup::active_main_window_footer_surface();
                        let use_native_footer_spacer = active_surface == Some("agent_chat");
                        if use_native_footer_spacer {
                            Some(
                                crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(),
                            )
                        } else {
                            Some(self.ensure_toolbar(cx).into_any_element())
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    let active_surface = crate::footer_popup::active_main_window_footer_surface();
                    let use_native_footer_spacer =
                        is_main_window && active_surface == Some("agent_chat");
                    if use_native_footer_spacer {
                        Some(
                            crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(),
                        )
                    } else {
                        Some(self.ensure_toolbar(cx).into_any_element())
                    }
                }
            };

            return crate::components::main_view_chrome::render_main_view_chrome(
                root,
                &theme,
                menu_def,
                crate::components::main_view_chrome::MainViewChrome {
                    header,
                    divider,
                    main,
                    footer,
                    overlays,
                },
            );
        }

        root
            .when(variant_config.show_variant_badge, |d| {
                d.child(Self::render_variant_badge(ui_variant, &theme))
            })
            .child(Self::render_reserved_transient_lane(
                "agent_chat-message-queue-lane-top",
                AGENT_CHAT_TRANSIENT_QUEUE_LANE_HEIGHT_PX,
                if self.message_queue_lane_active(cx) {
                    Some(self.render_message_queue_strip(cx))
                } else {
                    None
                },
            ))
            .child(self.render_active_callout(cx))
            .when(
                matches!(variant_config.composer, AgentChatComposerPlacement::Default),
                |d| {
                    d.child(Self::render_composer_bar(
                        &input_text,
                        input_cursor,
                        input_selection,
                        cursor_visible,
                        is_empty,
                        &mention_highlights,
                        &pasted_text_pills,
                        placeholder_text,
                        profile_icon_name.as_deref(),
                        profile_active_pending,
                        status,
                        view_entity.clone(),
                        &theme,
                    ))
                },
            )
            .when_some(self.focused_inline_mention_preview(cx), |d, preview| {
                d.child(
                    div().w_full().px(px(12.0)).pb(px(4.0)).child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.colors.text.muted))
                            .child(preview.token)
                            .child(" ")
                            .child(preview.detail),
                    ),
                )
            })
            // Context chips removed — all attachments are now inline @type:name tokens.
            // .child(self.render_pending_context_chips(cx))
            .child(Self::render_reserved_transient_lane(
                "agent_chat-context-bootstrap-lane",
                AGENT_CHAT_TRANSIENT_BOOTSTRAP_LANE_HEIGHT_PX,
                if self.context_bootstrap_note_lane_active(cx) {
                    Some(self.render_context_bootstrap_note(cx))
                } else {
                    None
                },
            ))
            // ── Search bar (Cmd+F) ─────────────────────────
            .when_some(self.search_state.clone(), |d, (query, current_idx)| {
                let match_count = if query.is_empty() {
                    0
                } else {
                    let q = query.to_lowercase();
                    messages
                        .iter()
                        .filter(|m| m.body.to_lowercase().contains(&q))
                        .count()
                };
                let display_idx = if match_count > 0 {
                    (current_idx % match_count) + 1
                } else {
                    0
                };
                d.child(
                    div()
                        .w_full()
                        .px(px(12.0))
                        .py(px(4.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(div().text_xs().opacity(0.50).child("\u{1F50D}"))
                        .child(div().flex_grow().text_sm().child(if query.is_empty() {
                            "Search conversation\u{2026}".to_string()
                        } else {
                            query.clone()
                        }))
                        .when(!query.is_empty(), |d| {
                            d.child(div().text_xs().opacity(0.45).child(if match_count > 0 {
                                format!("{display_idx}/{match_count}")
                            } else {
                                "0 matches".to_string()
                            }))
                        })
                        .when(match_count > 1, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .opacity(0.30)
                                    .child("\u{21A9} next \u{00b7} \u{21E7}\u{21A9} prev"),
                            )
                        })
                        .child(div().text_xs().opacity(0.25).child("esc \u{00d7}")),
                )
            })
            // ── Message list / Agent Chat Spine projection ───────────
            .child(
                crate::components::main_view_chrome::render_main_view_main_slot(
                    menu_def,
                    self.render_agent_chat_middle_area(
                        is_empty,
                        variant_config.show_sidecar,
                        ui_variant,
                        status_label,
                        message_count,
                        context_chip_count,
                        view_entity.clone(),
                        &theme,
                        cx,
                    ),
                ),
            )
            // ── Plan strip ────────────────────────────────────
            .child(Self::render_reserved_transient_lane(
                "agent_chat-plan-strip-lane",
                AGENT_CHAT_TRANSIENT_PLAN_LANE_HEIGHT_PX,
                if plan_entries.is_empty() {
                    None
                } else {
                    Some(
                        div()
                            .w_full()
                            .px(px(8.0))
                            .pb(px(4.0))
                            .child(Self::render_plan_strip(&plan_entries))
                            .into_any_element(),
                    )
                },
            ))
            // ── Pending permission fallback (non-tool-linked) ──────
            .child(Self::render_reserved_transient_lane(
                "agent_chat-permission-card-lane",
                AGENT_CHAT_TRANSIENT_PERMISSION_LANE_HEIGHT_PX,
                pending_permission
                    .clone()
                    .filter(|_| !pending_permission_has_message_target)
                    .map(|request| {
                        div()
                            .w_full()
                            .px(px(8.0))
                            .pb(px(4.0))
                            .child(Self::render_permission_inline_card(
                                &request,
                                self.permission_index,
                                self.permission_options_open,
                                view_entity.clone(),
                            ))
                            .into_any_element()
                    }),
            ))
            .child(Self::render_reserved_transient_lane(
                "agent_chat-message-queue-lane-bottom",
                AGENT_CHAT_TRANSIENT_QUEUE_LANE_HEIGHT_PX,
                if self.message_queue_lane_active(cx) {
                    Some(self.render_message_queue_strip(cx))
                } else {
                    None
                },
            ))
            .child(self.render_active_callout(cx))
            .when(
                matches!(variant_config.composer, AgentChatComposerPlacement::BottomDock),
                |d| {
                    d.child(Self::render_composer_bar(
                        &input_text,
                        input_cursor,
                        input_selection,
                        cursor_visible,
                        is_empty,
                        &mention_highlights,
                        &pasted_text_pills,
                        placeholder_text,
                        profile_icon_name.as_deref(),
                        profile_active_pending,
                        status,
                        view_entity.clone(),
                        &theme,
                    ))
                },
            )
            // ── Attach menu popup ──────────────────────────
            .when(self.attach_menu_open, |d| {
                d.child(self.render_attach_menu(cx))
            })
            .when(history_popup_open, |d| {
                d.child(
                    div()
                        .id("agent_chat-history-popup-backdrop")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom(px(self.inline_footer_height()))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.dismiss_history_popup(cx);
                                cx.stop_propagation();
                            }),
                        ),
                )
            })
            .when(!self.uses_external_footer_host(), |d| {
                let is_main_window = crate::get_main_window_handle()
                    .is_some_and(|handle| handle == window.window_handle());

                #[cfg(target_os = "macos")]
                {
                    if !is_main_window {
                        self.ensure_native_footer_action_listener(window, cx);
                        crate::footer_popup::sync_window_footer_popup(
                            window,
                            &self.agent_chat_detached_native_footer_config(cx),
                        );
                        return d.child(crate::components::prompt_layout_shell::render_native_main_window_footer_spacer());
                    }
                }

                let active_surface = crate::footer_popup::active_main_window_footer_surface();
                let use_native_footer_spacer = is_main_window && active_surface == Some("agent_chat");

                if use_native_footer_spacer {
                    d.child(crate::components::prompt_layout_shell::render_native_main_window_footer_spacer())
                } else {
                    d.child(self.ensure_toolbar(cx).into_any_element())
                }
            })
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;
