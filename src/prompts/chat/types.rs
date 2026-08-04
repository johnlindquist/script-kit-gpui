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

/// Host callback for shared recovery-card actions the prompt cannot perform
/// itself: (message_id, action). Availability of the corresponding card
/// buttons is derived from this callback's presence (S10).
pub type ChatRecoveryCallback =
    Arc<dyn Fn(String, sk_protocol::ai_reliability::AiRecoveryAction) + Send + Sync>;

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
    resolve_chat_input_key_action_with_facts(
        key,
        cmd_pressed,
        shift_pressed,
        key == ".",
        true,
        true,
    )
}

pub(crate) fn resolve_chat_input_key_action_with_facts(
    key: &str,
    cmd_pressed: bool,
    shift_pressed: bool,
    response_in_progress: bool,
    composer_has_text: bool,
    has_response: bool,
) -> ChatInputKeyAction {
    use crate::components::conversation_actions::{
        chat_prompt_conversation_commands, match_conversation_command_shortcut,
        ChatPromptConversationCommand,
    };

    if is_key_escape(key) {
        return ChatInputKeyAction::Escape;
    }

    if key.eq_ignore_ascii_case("end") {
        return ChatInputKeyAction::JumpToLatest;
    }

    let commands =
        chat_prompt_conversation_commands(response_in_progress, composer_has_text, has_response);
    if let Some((handler, _availability)) =
        match_conversation_command_shortcut(&commands, key, cmd_pressed, shift_pressed)
    {
        return match handler {
            ChatPromptConversationCommand::Send => ChatInputKeyAction::Submit,
            ChatPromptConversationCommand::Stop => ChatInputKeyAction::StopStreaming,
            ChatPromptConversationCommand::Close => ChatInputKeyAction::Ignore,
            ChatPromptConversationCommand::CopyLastResponse => ChatInputKeyAction::CopyLastResponse,
        };
    }

    if cmd_pressed {
        if is_key_down(key) {
            return ChatInputKeyAction::JumpToLatest;
        }
        if is_key_k(key) {
            return ChatInputKeyAction::ToggleActions;
        }
        if is_key_enter(key) {
            return ChatInputKeyAction::ContinueInChat;
        }
        if is_key_backspace(key) {
            return ChatInputKeyAction::ClearConversation;
        }
        if key.eq_ignore_ascii_case("v") {
            return ChatInputKeyAction::Paste;
        }
        return ChatInputKeyAction::Ignore;
    }

    if is_key_enter(key) && shift_pressed {
        return ChatInputKeyAction::InsertNewline;
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
    pub failure: Option<sk_protocol::ai_reliability::AiFailure>,
    pub message_id: Option<String>,
    pub user_image: Option<Arc<RenderImage>>,
    /// Stable identity for this turn's rendered answer region.
    ///
    /// Distinct from [`Self::message_id`] ON PURPOSE. `message_id` is
    /// reassigned from the user's id to the assistant's id the moment an
    /// assistant message arrives (see `build_conversation_turns`), which is
    /// correct for addressing the message but WRONG as a render identity: the
    /// key would change underneath a turn mid-stream, dropping any per-turn
    /// view state keyed by it and forcing a full re-parse on the first token.
    ///
    /// This key is chosen once from the turn's ORIGINATING message and never
    /// moves.
    pub render_key: ConversationTurnRenderKey,
}

/// Stable per-turn render identity. See [`ConversationTurn::render_key`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConversationTurnRenderKey(pub String);

impl ConversationTurnRenderKey {
    /// Choose a turn's render identity, in priority order:
    ///
    /// 1. the originating USER message id — stable across the whole turn,
    ///    including when the assistant message appears later;
    /// 2. the assistant message id, for standalone assistant turns that never
    ///    had a user message;
    /// 3. a positional fallback, for messages carrying no id at all.
    ///
    /// Never derived from a value that changes during streaming.
    pub fn resolve(
        user_message_id: Option<&str>,
        assistant_message_id: Option<&str>,
        turn_index: usize,
    ) -> Self {
        match (user_message_id, assistant_message_id) {
            (Some(id), _) if !id.is_empty() => Self(format!("u:{id}")),
            (_, Some(id)) if !id.is_empty() => Self(format!("a:{id}")),
            _ => Self(format!("i:{turn_index}")),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(super) fn conversation_turn_pending_indicator_visible(turn: &ConversationTurn) -> bool {
    turn.error.is_none()
        && turn.failure.is_none()
        && turn.streaming
        && turn
            .assistant_response
            .as_deref()
            .is_none_or(|response| response.trim().is_empty())
}

pub(super) fn conversation_turn_streaming_copy_available(turn: &ConversationTurn) -> bool {
    turn.error.is_none()
        && turn.failure.is_none()
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
            // Resolved from the USER message and never revisited. `message_id`
            // below is reassigned to the assistant's id once its message
            // arrives; the render key must not follow it or per-turn view
            // state would be dropped on the first streamed token.
            let render_key = ConversationTurnRenderKey::resolve(
                msg.id.as_deref(),
                messages.get(i + 1).and_then(|m| m.id.as_deref()),
                turns.len(),
            );
            let mut turn = ConversationTurn {
                user_prompt,
                assistant_response: None,
                model: None,
                streaming: false,
                error: None,
                failure: None,
                message_id: msg.id.clone(),
                user_image,
                render_key,
            };

            // Look for the next assistant response
            if i + 1 < messages.len() {
                let next_msg = &messages[i + 1];
                if !next_msg.is_user() {
                    turn.assistant_response = Some(next_msg.get_content().to_string());
                    turn.model = next_msg.model.clone();
                    turn.streaming = next_msg.streaming;
                    turn.error = next_msg.error.clone();
                    turn.failure = next_msg.failure.clone();
                    turn.message_id = next_msg.id.clone().or(turn.message_id);
                    i += 1;
                }
            }

            turns.push(turn);
        } else {
            // Standalone assistant message (no user prompt before it)
            // This happens for system-initiated messages
            // Standalone assistant turn: no user message exists, so the
            // assistant's own id IS the stable origin.
            let render_key =
                ConversationTurnRenderKey::resolve(None, msg.id.as_deref(), turns.len());
            let turn = ConversationTurn {
                user_prompt: String::new(),
                assistant_response: Some(msg.get_content().to_string()),
                model: msg.model.clone(),
                streaming: msg.streaming,
                error: msg.error.clone(),
                failure: msg.failure.clone(),
                message_id: msg.id.clone(),
                user_image: None,
                render_key,
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

// The string-sniffing `ChatErrorType` taxonomy (and its false
// "Model unavailable. Using default model." copy) was removed in S10/S11 of
// the ai-rock-solid-ux plan. Failure classification is owned by the shared
// `crate::ai::reliability` boundary; presentation is owned by the shared
// recovery projector and card.

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
    fn test_conversation_and_host_key_mappings_remain_distinct() {
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
            resolve_chat_input_key_action("c", true, true),
            ChatInputKeyAction::CopyLastResponse
        );
        assert_eq!(
            resolve_chat_input_key_action("c", true, false),
            ChatInputKeyAction::Ignore,
            "plain Cmd+C must remain available to copy the current selection"
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

#[cfg(test)]
mod conversation_turn_render_key_tests {
    use super::*;

    #[test]
    fn user_message_id_wins_so_the_key_survives_the_assistant_arriving() {
        let before = ConversationTurnRenderKey::resolve(Some("user-1"), None, 0);
        let after = ConversationTurnRenderKey::resolve(Some("user-1"), Some("asst-9"), 0);
        assert_eq!(
            before, after,
            "the render key must not move when the assistant message appears"
        );
        assert_eq!(before.as_str(), "u:user-1");
    }

    #[test]
    fn standalone_assistant_turn_keys_off_its_own_id() {
        let key = ConversationTurnRenderKey::resolve(None, Some("asst-9"), 3);
        assert_eq!(key.as_str(), "a:asst-9");
    }

    #[test]
    fn missing_ids_fall_back_to_a_stable_position() {
        assert_eq!(
            ConversationTurnRenderKey::resolve(None, None, 7).as_str(),
            "i:7"
        );
        // Empty strings are not ids; they must not produce a colliding "u:" key
        // that every id-less turn would share.
        assert_eq!(
            ConversationTurnRenderKey::resolve(Some(""), Some(""), 2).as_str(),
            "i:2"
        );
    }

    #[test]
    fn user_and_assistant_namespaces_cannot_collide() {
        // Without the prefixes, a user id "7" and an assistant id "7" would be
        // the same key and share one text view.
        assert_ne!(
            ConversationTurnRenderKey::resolve(Some("7"), None, 0),
            ConversationTurnRenderKey::resolve(None, Some("7"), 0)
        );
    }

    /// The end-to-end version of the first test, through the real builder:
    /// build the same conversation before and after the assistant reply lands
    /// and require the turn's render key to be unchanged.
    #[test]
    fn builder_keeps_the_turn_key_stable_when_the_assistant_reply_arrives() {
        let image_cache = HashMap::new();

        let mut messages = vec![ChatPromptMessage::user("explain this").with_id("m-user")];
        let before = build_conversation_turns(&messages, &image_cache);
        assert_eq!(before.len(), 1);
        let key_before = before[0].render_key.clone();

        messages.push(ChatPromptMessage::assistant("here you go").with_id("m-asst"));
        let after = build_conversation_turns(&messages, &image_cache);
        assert_eq!(after.len(), 1, "the reply joins the existing turn");
        assert_eq!(
            after[0].render_key, key_before,
            "render key must be stable across the assistant reply, even though \
             message_id is reassigned to the assistant id"
        );
        // Guard that message_id really does move, so this test is proving
        // something rather than asserting two constants are equal.
        assert_ne!(
            after[0].message_id, before[0].message_id,
            "message_id is expected to move; render_key is the stable one"
        );
    }
}

/// How a persistent assistant `TextViewState` should be updated for new source.
///
/// See `ChatPrompt::resolve_assistant_text_update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssistantTextUpdate<'a> {
    /// No state exists yet for this answer region.
    Create,
    /// Source is byte-identical to what the state already parsed.
    Unchanged,
    /// Source extends the previous source; only this tail is new.
    Append(&'a str),
    /// Source diverged from history and the document must be rebuilt.
    Replace,
}
