//! ChatPrompt - Raycast-style chat interface
//!
//! Features:
//! - Input at TOP (not bottom)
//! - Messages bundled as conversation turns (user prompt + AI response in same container)
//! - Full-width containers (not bubbles)
//! - Footer with model selector and "Continue in Chat"
//! - Actions menu (⌘+K) with model picker

use crate::components::TextInputState;
use crate::designs::icon_variations::IconName;
use gpui::{
    div, img, list, prelude::*, px, rgb, rgba, svg, AnyElement, App, Context, ExternalPaths,
    FocusHandle, Focusable, Hsla, KeyDownEvent, ListAlignment, ListSizingBehavior, ListState,
    Render, RenderImage, ScrollWheelEvent, Window,
};
use gpui_component::scroll::ScrollableElement;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::ai::providers::{ProviderMessage, ProviderRegistry};
use crate::ai::{self, Chat, ChatSource, Message, MessageRole, ModelInfo};
use crate::logging;
use crate::prompts::commands::transform_with_command;
use crate::prompts::context::expand_context;
use crate::prompts::markdown::render_markdown;
use crate::protocol::{ChatMessagePosition, ChatMessageRole, ChatPromptMessage};
use crate::theme;
use crate::ui_foundation::get_vibrancy_background;

mod actions;
mod prompt;
mod render_core;
mod render_input;
mod render_setup;
mod render_turns;
mod state;
mod streaming;
#[cfg(test)]
mod tests;
mod types;

#[cfg(test)]
pub(crate) use tests::chat_tests;

/// Maximum image size (10 MB) shared across all image intake paths:
/// clipboard paste, file drop, screen capture, and image picker.
pub(crate) const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

pub(crate) const CHAT_LAYOUT_PADDING_X: f32 = 12.0;
pub(crate) const CHAT_LAYOUT_SECTION_PADDING_Y: f32 = 8.0;
pub(crate) const CHAT_LAYOUT_MESSAGES_PADDING_Y: f32 = CHAT_LAYOUT_SECTION_PADDING_Y;
pub(crate) const CHAT_LAYOUT_CARD_PADDING_X: f32 = 12.0;
pub(crate) const CHAT_LAYOUT_CARD_PADDING_Y: f32 = 10.0;
pub(crate) const CHAT_LAYOUT_BORDER_ALPHA: u32 = 0x40;

use self::types::{
    build_conversation_turns, default_conversation_starters, next_reveal_boundary,
    resolve_chat_input_key_action_with_commands, resolve_chat_scroll_follow_after_scroll,
    resolve_setup_card_key, scroll_offset_is_at_bottom, should_ignore_stream_reveal_update,
    should_show_script_generation_actions, AssistantTextUpdate, ChatInputKeyAction,
    ChatScrollDirection, RunScriptCallback, ScriptGenerationAction, ScriptSavedCallback,
    SetupCardAction, CHAT_SCROLL_BOTTOM_TOLERANCE_PX,
};

#[cfg(test)]
use self::types::{next_chat_scroll_follow_state, resolve_chat_input_key_action};

pub use prompt::ChatPrompt;
pub use types::{
    default_models, ChatClaudeCodeCallback, ChatConfigureCallback, ChatContinueCallback, ChatModel,
    ChatPromptDismissBinding, ChatPromptDismissCallback, ChatPromptDismissRequest,
    ChatPromptDismissRoute, ChatPromptHostMode, ChatPromptPreparedRequest,
    ChatPromptRecoveryBinding, ChatPromptRetryRequest, ChatPromptStopRequest, ChatRetryCallback,
    ChatShowActionsCallback, ChatStopCallback, ChatSubmitCallback, ChatTranscriptAlignment,
    ConversationStarter, ConversationTurn, ConversationTurnRenderKey,
};
// Consumers of the public recovery binding also need its callback type.
#[allow(unused_imports)]
pub use types::ChatRecoveryCallback;

// Internal render-plan resolution (pure logic, unit-tested).
pub(crate) use types::resolve_chat_render_plan;
