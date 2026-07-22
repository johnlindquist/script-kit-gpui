use super::*;
use crate::ui_foundation::{
    is_key_backspace, is_key_down, is_key_enter, is_key_escape, is_key_k, is_key_space, is_key_tab,
    is_key_up,
};
use std::borrow::Cow;

/// Where a transcript-only host anchors the message list.
///
/// A composer-at-top host (flow sessions) reads top-down: a short
/// conversation must sit right under the composer, so it anchors `Top`.
/// The stock standalone chat anchors `Bottom` (newest at the fold).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatTranscriptAlignment {
    Bottom,
    Top,
}

/// Exhaustive host mode for a [`ChatPrompt`].
///
/// Replaces the independent booleans `mini_mode`, `escape_over_stop`,
/// `external_header`, `external_input`, and `external_footer`, which
/// allowed incoherent combinations (e.g. an "external footer" host that
/// still installed its own escape ladder). A ChatPrompt is either fully
/// self-hosted or purely a transcript body — never a partial mixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatPromptHostMode {
    /// The prompt owns its full chrome and lifecycle: header, input area,
    /// footer, and the Enter/Escape submission handlers. `mini` selects the
    /// borderless mini-window chrome (matches the mini main window).
    Standalone { mini: bool },
    /// An external host (a flow session) owns the header, composer, footer,
    /// and ALL lifecycle/key handling. This surface renders transcript,
    /// empty state, setup body, or loading body ONLY — no header, no input,
    /// no footer, no Enter/Escape handlers, and no escape callback. The host
    /// is the single lifecycle/key owner.
    TranscriptOnly { alignment: ChatTranscriptAlignment },
}

impl ChatPromptHostMode {
    /// Whether an external host owns chrome + keys (transcript body only).
    pub fn is_transcript_only(self) -> bool {
        matches!(self, ChatPromptHostMode::TranscriptOnly { .. })
    }

    /// Whether the standalone prompt renders the borderless mini chrome.
    /// A transcript-only host owns its own chrome, so mini never applies.
    pub fn mini(self) -> bool {
        matches!(self, ChatPromptHostMode::Standalone { mini: true })
    }
}

/// Which body a ChatPrompt paints for the current provider/setup state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatBodyKind {
    /// API-key configuration card (no providers configured).
    Setup,
    /// "Connecting to AI…" placeholder while providers load.
    Loading,
    /// The conversation transcript (or its empty state).
    Transcript,
}

/// The chrome + key-handler composition for one render pass, resolved from
/// the host mode and provider/setup state BEFORE any early return. This is
/// the single source of truth the renderer follows in every body branch, so
/// a transcript-only host can never leak local chrome through the setup or
/// loading paths (the bug this replaces).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatRenderPlan {
    pub body: ChatBodyKind,
    pub render_header: bool,
    pub render_input: bool,
    pub render_footer: bool,
    pub install_key_handlers: bool,
    /// Whether this host owns first-render focus. A transcript-only host's
    /// composer lives in the external host, so it must NEVER grab focus (the
    /// suppressed internal input would silently steal it). `false` for
    /// TranscriptOnly, `true` for Standalone.
    pub owns_focus: bool,
    /// Whether this host owns the local input lifecycle: cursor-blink startup,
    /// pending-submit processing, and initial-response processing. A
    /// transcript-only host owns none of these — the external host drives
    /// submission and there is no visible local composer to blink. `false` for
    /// TranscriptOnly, `true` for Standalone. (C-R2: these ran unconditionally
    /// before the plan resolved, so a hidden transcript-only host could
    /// auto-submit or start a blink task.)
    pub owns_input_lifecycle: bool,
}

/// Resolve the render plan from the host mode and current provider state.
///
/// Transcript-only hosts suppress ALL local chrome and key handlers in every
/// body state — the host is the only lifecycle/key owner. Standalone hosts
/// keep their key handlers in every state; the setup and loading bodies show
/// the header only (no input, no footer), and the transcript body shows the
/// header (unless mini), the input, and the footer.
pub fn resolve_chat_render_plan(
    mode: ChatPromptHostMode,
    needs_setup: bool,
    loading_providers: bool,
) -> ChatRenderPlan {
    let body = if needs_setup {
        ChatBodyKind::Setup
    } else if loading_providers {
        ChatBodyKind::Loading
    } else {
        ChatBodyKind::Transcript
    };

    if mode.is_transcript_only() {
        return ChatRenderPlan {
            body,
            render_header: false,
            render_input: false,
            render_footer: false,
            install_key_handlers: false,
            owns_focus: false,
            owns_input_lifecycle: false,
        };
    }

    let (render_header, render_input, render_footer) = match body {
        // Setup card / loading placeholder: header + body only, no
        // composer and no footer (matches the standalone chat's chrome).
        ChatBodyKind::Setup | ChatBodyKind::Loading => (true, false, false),
        // Transcript: header unless mini, plus composer and footer.
        ChatBodyKind::Transcript => (!mode.mini(), true, true),
    };

    ChatRenderPlan {
        body,
        render_header,
        render_input,
        render_footer,
        install_key_handlers: true,
        owns_focus: true,
        owns_input_lifecycle: true,
    }
}

/// Available AI models for the chat
#[derive(Clone, Debug, PartialEq)]
pub struct ChatModel {
    pub id: String,
    pub name: String,
    pub provider: String,
}

impl ChatModel {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            provider: provider.into(),
        }
    }
}

/// Default models available in the chat
/// NOTE: First model in list is the default
pub fn default_models() -> Vec<ChatModel> {
    vec![
        // Default: Claude Haiku 4.5 (fast, good quality)
        ChatModel::new("claude-haiku-4-5-20250514", "Claude Haiku 4.5", "Anthropic"),
        ChatModel::new("claude-3-5-haiku-20241022", "Claude 3.5 Haiku", "Anthropic"),
        ChatModel::new(
            "claude-3-5-sonnet-20241022",
            "Claude 3.5 Sonnet",
            "Anthropic",
        ),
        ChatModel::new("gpt-4o-mini", "GPT-4o mini", "OpenAI"),
        ChatModel::new("gpt-4o", "GPT-4o", "OpenAI"),
    ]
}

/// Callback type for when user submits a message: (prompt_id, message_text)
pub type ChatSubmitCallback = Arc<dyn Fn(String, String) + Send + Sync>;

/// Callback type for when user presses Escape: (prompt_id)
pub type ChatEscapeCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Callback type for "Continue in Chat": (prompt_id)
pub type ChatContinueCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Callback type for retry: (prompt_id, message_id)
pub type ChatRetryCallback = Arc<dyn Fn(String, String) + Send + Sync>;

/// Callback type for "Configure API" action: () -> triggers API key setup
pub type ChatConfigureCallback = Arc<dyn Fn() + Send + Sync>;

/// Callback type for "Connect to Claude Code" action: () -> enables Claude Code in config
pub type ChatClaudeCodeCallback = Arc<dyn Fn() + Send + Sync>;

/// Callback type for showing actions menu: (prompt_id) -> triggers ActionsDialog
pub type ChatShowActionsCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Callback type for running a saved generated script path in the parent app.
pub type RunScriptCallback =
    Arc<dyn Fn(std::path::PathBuf, &mut gpui::Context<super::prompt::ChatPrompt>) + Send + Sync>;

/// Callback type for when a generated script has been saved to disk: (script_path)
pub type ScriptSavedCallback =
    Arc<dyn Fn(std::path::PathBuf, &mut gpui::Context<super::prompt::ChatPrompt>) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetupCardAction {
    None,
    ActivateConfigure,
    ActivateClaudeCode,
    Escape,
}

pub(crate) fn resolve_setup_card_key(
    key: &str,
    shift: bool,
    current_index: usize,
) -> (usize, SetupCardAction, bool) {
    let current_index = current_index % 2;

    if is_key_tab(key) {
        let next_index = if shift {
            if current_index == 0 {
                1
            } else {
                current_index - 1
            }
        } else {
            (current_index + 1) % 2
        };
        return (next_index, SetupCardAction::None, true);
    }

    if is_key_up(key) {
        let next_index = if current_index == 0 {
            1
        } else {
            current_index - 1
        };
        return (next_index, SetupCardAction::None, true);
    }

    if is_key_down(key) {
        let next_index = (current_index + 1) % 2;
        return (next_index, SetupCardAction::None, true);
    }

    if is_key_enter(key) || is_key_space(key) {
        let action = if current_index == 0 {
            SetupCardAction::ActivateConfigure
        } else {
            SetupCardAction::ActivateClaudeCode
        };
        return (current_index, action, false);
    }

    if is_key_escape(key) {
        return (current_index, SetupCardAction::Escape, false);
    }

    (current_index, SetupCardAction::None, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScriptGenerationAction {
    Save,
    Run,
    SaveAndRun,
}

impl ScriptGenerationAction {
    pub(crate) fn should_run_after_save(self) -> bool {
        matches!(self, Self::Run | Self::SaveAndRun)
    }
}

pub(crate) fn should_show_script_generation_actions(
    script_generation_mode: bool,
    is_streaming: bool,
    has_draft: bool,
) -> bool {
    script_generation_mode && !is_streaming && has_draft
}

/// Normalize assistant content for markdown rendering in script-generation mode.
///
/// Script generation prompts ask models to return raw TypeScript without markdown
/// fences. Wrap that raw code in a fenced block so the chat renderer can apply
/// code-block styling while preserving non-script chat behavior.
pub(crate) fn assistant_response_markdown_source<'a>(
    script_generation_mode: bool,
    response: &'a str,
) -> Cow<'a, str> {
    if !script_generation_mode {
        return Cow::Borrowed(response);
    }

    let trimmed = response.trim();
    if trimmed.is_empty() || response.contains("```") {
        return Cow::Borrowed(response);
    }

    let code_body = response.trim_end_matches('\n');
    Cow::Owned(format!("```typescript\n{}\n```", code_body))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChatInputKeyAction {
    Escape,
    StopStreaming,
    ToggleActions,
    ContinueInChat,
    Submit,
    InsertNewline,
    CopyLastResponse,
    ClearConversation,
    Paste,
    JumpToLatest,
    DelegateToInput,
    Ignore,
}

pub(crate) fn resolve_chat_input_key_action(
    key: &str,
    cmd_pressed: bool,
    shift_pressed: bool,
) -> ChatInputKeyAction {
    if is_key_escape(key) {
        return ChatInputKeyAction::Escape;
    }

    if key.eq_ignore_ascii_case("end") {
        return ChatInputKeyAction::JumpToLatest;
    }

    if cmd_pressed {
        if key == "." {
            return ChatInputKeyAction::StopStreaming;
        }
        if is_key_down(key) {
            return ChatInputKeyAction::JumpToLatest;
        }
        if is_key_k(key) {
            return ChatInputKeyAction::ToggleActions;
        }
        if is_key_enter(key) {
            return ChatInputKeyAction::ContinueInChat;
        }
        if key.eq_ignore_ascii_case("c") {
            return ChatInputKeyAction::CopyLastResponse;
        }
        if is_key_backspace(key) {
            return ChatInputKeyAction::ClearConversation;
        }
        if key.eq_ignore_ascii_case("v") {
            return ChatInputKeyAction::Paste;
        }
        return ChatInputKeyAction::Ignore;
    }

    if is_key_enter(key) {
        if shift_pressed {
            return ChatInputKeyAction::InsertNewline;
        }
        return ChatInputKeyAction::Submit;
    }

    ChatInputKeyAction::DelegateToInput
}

pub(super) fn should_ignore_stream_reveal_update(
    active_stream_message_id: Option<&str>,
    streaming_message_id: &str,
) -> bool {
    active_stream_message_id != Some(streaming_message_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChatScrollDirection {
    Up,
    Down,
    None,
}

/// How close (in px) to the maximum scroll offset still counts as "at the
/// bottom" of the conversation. GPUI's bottom-aligned list only reports the
/// exact bottom (`logical_scroll_top == None`) when a scroll lands precisely
/// on `scroll_max`; trackpad momentum regularly stops a fraction of a pixel
/// short, which kept the "Jump to latest" pill visible at the visual bottom.
/// One text line of slack matches what the user perceives as "at the bottom".
pub(crate) const CHAT_SCROLL_BOTTOM_TOLERANCE_PX: f32 = 24.0;

/// Tolerant bottom test on raw scroll offsets. `current_offset_px` may exceed
/// `max_offset_px` near the real bottom (the list's max ignores the element's
/// vertical padding), so this is a signed comparison, not an equality.
pub(crate) fn scroll_offset_is_at_bottom(
    current_offset_px: f32,
    max_offset_px: f32,
    tolerance_px: f32,
) -> bool {
    max_offset_px - current_offset_px <= tolerance_px
}

pub(crate) fn next_chat_scroll_follow_state(
    user_has_scrolled_up: bool,
    direction: ChatScrollDirection,
    is_at_bottom: bool,
) -> bool {
    match direction {
        // Upward intent means "stop following streaming output".
        ChatScrollDirection::Up => true,
        // Resume follow mode only once the user reaches the true bottom.
        ChatScrollDirection::Down if user_has_scrolled_up && is_at_bottom => false,
        ChatScrollDirection::Down | ChatScrollDirection::None => user_has_scrolled_up,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChatScrollFollowDecision {
    pub next_manual_mode: bool,
}

pub(crate) fn resolve_chat_scroll_follow_after_scroll(
    previous_manual_mode: bool,
    direction: ChatScrollDirection,
    at_bottom_before: bool,
    at_bottom_after: bool,
) -> ChatScrollFollowDecision {
    let next_manual_mode = if at_bottom_after {
        false
    } else {
        next_chat_scroll_follow_state(previous_manual_mode, direction, at_bottom_before)
    };

    ChatScrollFollowDecision { next_manual_mode }
}

/// A conversation turn: user prompt + optional AI response
#[derive(Clone, Debug)]
pub struct ConversationTurn {
    pub user_prompt: String,
    pub assistant_response: Option<String>,
    pub model: Option<String>,
    pub streaming: bool,
    pub error: Option<String>,
    pub message_id: Option<String>,
    pub user_image: Option<Arc<RenderImage>>,
}

pub(super) fn conversation_turn_pending_indicator_visible(turn: &ConversationTurn) -> bool {
    turn.error.is_none()
        && turn.streaming
        && turn
            .assistant_response
            .as_deref()
            .is_none_or(|response| response.trim().is_empty())
}

pub(super) fn conversation_turn_streaming_copy_available(turn: &ConversationTurn) -> bool {
    turn.error.is_none()
        && turn.streaming
        && turn
            .assistant_response
            .as_deref()
            .is_some_and(|response| !response.trim().is_empty())
}

/// Conversation starter suggestion
#[derive(Clone, Debug)]
pub struct ConversationStarter {
    pub id: String,
    pub label: String,
    pub prompt: String,
}

impl ConversationStarter {
    pub fn new(id: impl Into<String>, label: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            prompt: prompt.into(),
        }
    }
}

/// Default conversation starters
pub(super) fn default_conversation_starters() -> Vec<ConversationStarter> {
    vec![
        ConversationStarter::new("explain", "Explain this code", "Explain this code: "),
        ConversationStarter::new("debug", "Debug an error", "Help me debug this error: "),
        ConversationStarter::new("tests", "Write tests", "Write tests for: "),
        ConversationStarter::new("improve", "Improve code", "Improve this code: "),
    ]
}

pub(super) fn build_conversation_turns(
    messages: &[ChatPromptMessage],
    image_render_cache: &HashMap<String, Arc<RenderImage>>,
) -> Vec<ConversationTurn> {
    let mut turns = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        let msg = &messages[i];

        if msg.is_user() {
            // Start a new turn with this user message
            let user_prompt = msg.get_content().to_string();
            let user_image = msg
                .id
                .as_ref()
                .and_then(|id| image_render_cache.get(id).cloned());
            let mut turn = ConversationTurn {
                user_prompt,
                assistant_response: None,
                model: None,
                streaming: false,
                error: None,
                message_id: msg.id.clone(),
                user_image,
            };

            // Look for the next assistant response
            if i + 1 < messages.len() {
                let next_msg = &messages[i + 1];
                if !next_msg.is_user() {
                    turn.assistant_response = Some(next_msg.get_content().to_string());
                    turn.model = next_msg.model.clone();
                    turn.streaming = next_msg.streaming;
                    turn.error = next_msg.error.clone();
                    turn.message_id = next_msg.id.clone().or(turn.message_id);
                    i += 1;
                }
            }

            turns.push(turn);
        } else {
            // Standalone assistant message (no user prompt before it)
            // This happens for system-initiated messages
            let turn = ConversationTurn {
                user_prompt: String::new(),
                assistant_response: Some(msg.get_content().to_string()),
                model: msg.model.clone(),
                streaming: msg.streaming,
                error: msg.error.clone(),
                message_id: msg.id.clone(),
                user_image: None,
            };
            turns.push(turn);
        }

        i += 1;
    }

    turns
}

/// Find the next reveal boundary after `offset` in `text`.
///
/// Reveals through the next newline so markdown structural elements (list markers,
/// headings) are always delivered as complete lines. Within a long line that has no
/// newline yet, falls back to word boundaries for smooth character-level pacing.
///
/// Returns `None` when only a partial token remains (no whitespace yet), signalling
/// the reveal loop to wait for more data. All returned offsets land on UTF-8
/// character boundaries.
pub(super) fn next_reveal_boundary(text: &str, offset: usize) -> Option<usize> {
    let remaining = &text[offset..];
    if remaining.is_empty() {
        return None;
    }

    // Strategy: reveal through the next newline (keeps markdown lines intact).
    // If no newline is found, fall back to next word boundary within the line.
    if let Some(nl_pos) = remaining.find('\n') {
        // Include the newline itself
        return Some(offset + nl_pos + 1);
    }

    // No newline — reveal next word within the current (incomplete) line.
    let mut found_non_ws = false;
    let mut word_end: Option<usize> = None;

    for (i, c) in remaining.char_indices() {
        if c.is_whitespace() {
            if found_non_ws && word_end.is_none() {
                word_end = Some(i);
            }
            if word_end.is_some() {
                continue;
            }
        } else {
            if word_end.is_some() {
                return Some(offset + i);
            }
            found_non_ws = true;
        }
    }

    if word_end.is_some() {
        Some(offset + remaining.len())
    } else if found_non_ws {
        // Partial word, no trailing whitespace — wait for more data
        None
    } else {
        Some(offset + remaining.len())
    }
}

/// Error types for chat operations
#[derive(Clone, Debug, PartialEq)]
pub enum ChatErrorType {
    NoApiKey,
    NetworkError,
    StreamInterrupted,
    RateLimited,
    InvalidModel,
    TokenLimit,
    ClaudeCodeNested,
    ClaudeCodeNotFound,
    ProviderError,
    ServerError,
    Unknown,
}

impl ChatErrorType {
    pub fn from_error_string(s: &str) -> Self {
        let s_lower = s.to_lowercase();
        if s_lower.contains("api key")
            || s_lower.contains("unauthorized")
            || s_lower.contains("401")
        {
            ChatErrorType::NoApiKey
        } else if s_lower.contains("cannot be launched inside another claude code session")
            || s_lower.contains("nested sessions")
        {
            ChatErrorType::ClaudeCodeNested
        } else if s_lower.contains("claude")
            && (s_lower.contains("not found")
                || s_lower.contains("no such file")
                || s_lower.contains("command not found"))
        {
            ChatErrorType::ClaudeCodeNotFound
        } else if s_lower.contains("network")
            || s_lower.contains("connection")
            || s_lower.contains("timeout")
        {
            ChatErrorType::NetworkError
        } else if s_lower.contains("interrupt") || s_lower.contains("abort") {
            ChatErrorType::StreamInterrupted
        } else if s_lower.contains("rate limit") || s_lower.contains("429") {
            ChatErrorType::RateLimited
        } else if s_lower.contains("model")
            && (s_lower.contains("invalid")
                || s_lower.contains("not found")
                || s_lower.contains("unavailable")
                || s_lower.contains("does not exist")
                || s_lower.contains("not supported"))
        {
            ChatErrorType::InvalidModel
        } else if s_lower.contains("token")
            || s_lower.contains("too long")
            || s_lower.contains("length")
        {
            ChatErrorType::TokenLimit
        } else if s_lower.contains("500")
            || s_lower.contains("502")
            || s_lower.contains("503")
            || s_lower.contains("server error")
            || s_lower.contains("internal server error")
        {
            ChatErrorType::ServerError
        } else if s_lower.contains("cli exited with status") || s_lower.contains("returned error") {
            ChatErrorType::ProviderError
        } else {
            ChatErrorType::Unknown
        }
    }

    pub fn display_message(&self) -> &'static str {
        match self {
            ChatErrorType::NoApiKey => {
                "\u{26a0} API key not configured. Set up your API key to continue."
            }
            ChatErrorType::NetworkError => {
                "\u{26a0} Network error. Check your connection and try again."
            }
            ChatErrorType::StreamInterrupted => {
                "\u{26a0} Response interrupted. Click retry to continue."
            }
            ChatErrorType::RateLimited => {
                "\u{26a0} Rate limited. Please wait a moment and try again."
            }
            ChatErrorType::InvalidModel => "\u{26a0} Model unavailable. Using default model.",
            ChatErrorType::TokenLimit => "\u{26a0} Message too long. Try a shorter prompt.",
            ChatErrorType::ClaudeCodeNested => {
                "\u{26a0} Cannot run Claude Code inside an existing Claude Code session. \
                 Close the outer session first."
            }
            ChatErrorType::ClaudeCodeNotFound => {
                "\u{26a0} Claude Code CLI not found. \
                 Install it from https://docs.anthropic.com/en/docs/claude-code"
            }
            ChatErrorType::ProviderError => "\u{26a0} AI provider error. Check the details below.",
            ChatErrorType::ServerError => {
                "\u{26a0} Server error. The AI provider may be experiencing issues."
            }
            ChatErrorType::Unknown => "\u{26a0} Something went wrong. Please try again.",
        }
    }

    pub fn can_retry(&self) -> bool {
        matches!(
            self,
            ChatErrorType::NetworkError
                | ChatErrorType::StreamInterrupted
                | ChatErrorType::RateLimited
                | ChatErrorType::ProviderError
                | ChatErrorType::ServerError
                | ChatErrorType::Unknown
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_down_maps_to_jump_to_latest() {
        assert_eq!(
            resolve_chat_input_key_action("down", true, false),
            ChatInputKeyAction::JumpToLatest
        );
        assert_eq!(
            resolve_chat_input_key_action("arrowdown", true, false),
            ChatInputKeyAction::JumpToLatest
        );
    }

    #[test]
    fn test_end_maps_to_jump_to_latest() {
        assert_eq!(
            resolve_chat_input_key_action("end", false, false),
            ChatInputKeyAction::JumpToLatest
        );
        assert_eq!(
            resolve_chat_input_key_action("End", false, false),
            ChatInputKeyAction::JumpToLatest
        );
    }

    #[test]
    fn test_existing_key_mappings_unchanged() {
        assert_eq!(
            resolve_chat_input_key_action("escape", false, false),
            ChatInputKeyAction::Escape
        );
        assert_eq!(
            resolve_chat_input_key_action(".", true, false),
            ChatInputKeyAction::StopStreaming
        );
        assert_eq!(
            resolve_chat_input_key_action("k", true, false),
            ChatInputKeyAction::ToggleActions
        );
        assert_eq!(
            resolve_chat_input_key_action("enter", true, false),
            ChatInputKeyAction::ContinueInChat
        );
        assert_eq!(
            resolve_chat_input_key_action("c", true, false),
            ChatInputKeyAction::CopyLastResponse
        );
        assert_eq!(
            resolve_chat_input_key_action("backspace", true, false),
            ChatInputKeyAction::ClearConversation
        );
        assert_eq!(
            resolve_chat_input_key_action("v", true, false),
            ChatInputKeyAction::Paste
        );
        assert_eq!(
            resolve_chat_input_key_action("enter", false, false),
            ChatInputKeyAction::Submit
        );
        assert_eq!(
            resolve_chat_input_key_action("enter", false, true),
            ChatInputKeyAction::InsertNewline
        );
    }

    #[test]
    fn test_scroll_follow_state_up_enters_manual() {
        assert!(
            next_chat_scroll_follow_state(false, ChatScrollDirection::Up, true),
            "upward scroll should disable auto-follow"
        );
        assert!(
            next_chat_scroll_follow_state(false, ChatScrollDirection::Up, false),
            "upward scroll should disable auto-follow regardless of bottom position"
        );
    }

    #[test]
    fn test_scroll_follow_state_down_not_at_bottom_stays_manual() {
        assert!(
            next_chat_scroll_follow_state(true, ChatScrollDirection::Down, false),
            "downward scroll away from bottom should remain in manual mode"
        );
    }

    #[test]
    fn test_scroll_follow_state_down_at_bottom_resumes_auto() {
        assert!(
            !next_chat_scroll_follow_state(true, ChatScrollDirection::Down, true),
            "downward scroll at bottom should resume auto-follow"
        );
    }

    #[test]
    fn test_after_scroll_bottom_resumes_auto_follow() {
        assert_eq!(
            resolve_chat_scroll_follow_after_scroll(true, ChatScrollDirection::Down, false, true,),
            ChatScrollFollowDecision {
                next_manual_mode: false,
            }
        );
    }

    #[test]
    fn test_after_scroll_up_not_at_bottom_enters_manual_follow() {
        assert_eq!(
            resolve_chat_scroll_follow_after_scroll(false, ChatScrollDirection::Up, true, false,),
            ChatScrollFollowDecision {
                next_manual_mode: true,
            }
        );
    }
}
