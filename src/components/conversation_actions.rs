//! Shared per-turn conversation action affordances.
//!
//! Owns the response **copy** control that both Flow's `ChatPrompt` and Agent
//! Chat's transcript render. Flow had this control; Agent Chat did not. Rather
//! than author a second one, the existing Flow implementation was extracted
//! here verbatim (metrics now live in
//! [`crate::components::conversation_style::ConversationActionStyle`]) and both
//! surfaces render through it.
//!
//! ## Why eligibility is a pure function
//!
//! Whether a row gets a copy button depends on the message role, whether the
//! body is empty, and whether the turn is still streaming. Encoding that in
//! renderer `if` chains is how the two surfaces drifted in the first place, and
//! it cannot be unit-tested. [`turn_copy_eligibility`] is a total function over
//! those inputs, so every case — including the ones that must NOT show a button
//! — is enumerable in a test without constructing a window.

use gpui::{div, prelude::*, px, rgb, svg, Animation, AnimationExt as _, SharedString};

use crate::components::conversation_style::ConversationStyleDef;
use crate::designs::icon_variations::IconName;

/// Closed command vocabulary shared by conversation footers, Actions, key
/// routing, and semantic automation. Hosts bind only commands they can execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ConversationCommandId {
    Send,
    Stop,
    Retry,
    Background,
    Back,
    Close,
    NewConversation,
    DeleteConversation,
    TerminateRuntime,
    ContinueAsNewConversation,
    CopyLastResponse,
    CopyTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationCommandRole {
    Primary,
    Secondary,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmPolicy {
    None,
    Required,
}

/// Closed user-safe reasons for supported commands that cannot run right now.
/// Callers cannot smuggle provider, adapter, path, or authored text into UI copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationCommandDisabledReason {
    TypeMessageFirst,
    ContextStillPreparing,
    ResponseInProgress,
    NoResponseRunning,
    WaitingForPermission,
}

impl ConversationCommandDisabledReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::TypeMessageFirst => "Type a message first.",
            Self::ContextStillPreparing => "Wait for context to finish loading.",
            Self::ResponseInProgress => "Stop the current response first.",
            Self::NoResponseRunning => "No response is running.",
            Self::WaitingForPermission => "Resolve the permission request first.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationCommandAvailability {
    Enabled,
    Disabled {
        reason: ConversationCommandDisabledReason,
    },
}

impl ConversationCommandAvailability {
    pub(crate) const fn disabled(reason: ConversationCommandDisabledReason) -> Self {
        Self::Disabled { reason }
    }

    pub(crate) const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub(crate) const fn disabled_reason(self) -> Option<&'static str> {
        match self {
            Self::Enabled => None,
            Self::Disabled { reason } => Some(reason.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversationCommandDescriptor {
    pub(crate) id: ConversationCommandId,
    pub(crate) label: &'static str,
    pub(crate) shortcut: Option<&'static str>,
    pub(crate) role: ConversationCommandRole,
    pub(crate) availability: ConversationCommandAvailability,
    pub(crate) confirmation: ConfirmPolicy,
    pub(crate) semantic_action_id: &'static str,
}

fn command_metadata(
    id: ConversationCommandId,
) -> (
    &'static str,
    Option<&'static str>,
    ConversationCommandRole,
    ConfirmPolicy,
    &'static str,
) {
    use ConversationCommandId::*;
    match id {
        Send => (
            "Send",
            Some("↵"),
            ConversationCommandRole::Primary,
            ConfirmPolicy::None,
            "conversation.send",
        ),
        Stop => (
            "Stop",
            Some("⌘."),
            ConversationCommandRole::Primary,
            ConfirmPolicy::None,
            "conversation.stop",
        ),
        Retry => (
            "Retry",
            Some("⇧⌘R"),
            ConversationCommandRole::Primary,
            ConfirmPolicy::None,
            "conversation.retry",
        ),
        Background => (
            "Background",
            Some("Esc"),
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.background",
        ),
        Back => (
            "Back",
            Some("Esc"),
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.back",
        ),
        Close => (
            "Close",
            Some("⌘W"),
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.close",
        ),
        NewConversation => (
            "New Conversation",
            Some("⌘L"),
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.new",
        ),
        DeleteConversation => (
            "Delete Conversation…",
            None,
            ConversationCommandRole::Destructive,
            ConfirmPolicy::Required,
            "conversation.delete",
        ),
        TerminateRuntime => (
            "Terminate Runtime…",
            Some("⇧⌘⎋"),
            ConversationCommandRole::Destructive,
            ConfirmPolicy::Required,
            "conversation.terminateRuntime",
        ),
        ContinueAsNewConversation => (
            "Continue as New Conversation",
            None,
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.continueAsNew",
        ),
        CopyLastResponse => (
            "Copy Last Response",
            Some("⇧⌘C"),
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.copyLast",
        ),
        CopyTurn => (
            "Copy Turn",
            None,
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.copyTurn",
        ),
    }
}

/// A descriptor and its host-owned executable binding are inseparable. A host
/// cannot advertise a command without supplying the exhaustive handler token
/// that its owning surface dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundConversationCommand<Handler> {
    pub(crate) descriptor: ConversationCommandDescriptor,
    pub(crate) handler: Handler,
}

impl<Handler> BoundConversationCommand<Handler> {
    pub(crate) fn enabled(id: ConversationCommandId, handler: Handler) -> Self {
        Self::with_availability(id, ConversationCommandAvailability::Enabled, handler)
    }

    pub(crate) fn disabled(
        id: ConversationCommandId,
        reason: ConversationCommandDisabledReason,
        handler: Handler,
    ) -> Self {
        Self::with_availability(
            id,
            ConversationCommandAvailability::disabled(reason),
            handler,
        )
    }

    fn with_availability(
        id: ConversationCommandId,
        availability: ConversationCommandAvailability,
        handler: Handler,
    ) -> Self {
        let (label, shortcut, role, confirmation, semantic_action_id) = command_metadata(id);
        Self {
            descriptor: ConversationCommandDescriptor {
                id,
                label,
                shortcut,
                role,
                availability,
                confirmation,
                semantic_action_id,
            },
            handler,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatConversationCommand {
    Send,
    Stop,
    Retry,
    NewConversation,
    CopyLastResponse,
    Close,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentChatConversationCommandFacts {
    pub(crate) response_in_progress: bool,
    pub(crate) waiting_for_permission: bool,
    pub(crate) context_preparing: bool,
    pub(crate) composer_has_text: bool,
    pub(crate) retry_available: bool,
    pub(crate) has_response: bool,
}

pub(crate) fn agent_chat_conversation_commands(
    facts: AgentChatConversationCommandFacts,
) -> Vec<BoundConversationCommand<AgentChatConversationCommand>> {
    let mut commands = Vec::new();
    if facts.response_in_progress {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::Stop,
            AgentChatConversationCommand::Stop,
        ));
    } else {
        let send_availability = if facts.waiting_for_permission {
            Some(ConversationCommandDisabledReason::WaitingForPermission)
        } else if facts.context_preparing {
            Some(ConversationCommandDisabledReason::ContextStillPreparing)
        } else if !facts.composer_has_text {
            Some(ConversationCommandDisabledReason::TypeMessageFirst)
        } else {
            None
        };
        commands.push(match send_availability {
            Some(reason) => BoundConversationCommand::disabled(
                ConversationCommandId::Send,
                reason,
                AgentChatConversationCommand::Send,
            ),
            None => BoundConversationCommand::enabled(
                ConversationCommandId::Send,
                AgentChatConversationCommand::Send,
            ),
        });
    }
    if facts.retry_available {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::Retry,
            AgentChatConversationCommand::Retry,
        ));
    }
    commands.push(if facts.response_in_progress {
        BoundConversationCommand::disabled(
            ConversationCommandId::NewConversation,
            ConversationCommandDisabledReason::ResponseInProgress,
            AgentChatConversationCommand::NewConversation,
        )
    } else {
        BoundConversationCommand::enabled(
            ConversationCommandId::NewConversation,
            AgentChatConversationCommand::NewConversation,
        )
    });
    if facts.has_response {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::CopyLastResponse,
            AgentChatConversationCommand::CopyLastResponse,
        ));
    }
    commands.push(BoundConversationCommand::enabled(
        ConversationCommandId::Close,
        AgentChatConversationCommand::Close,
    ));
    commands
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowConversationCommand {
    Send,
    Stop,
    Background,
    NewConversation,
    CopyLastResponse,
    TerminateRuntime,
}

pub(crate) fn flow_conversation_commands(
    response_in_progress: bool,
) -> Vec<BoundConversationCommand<FlowConversationCommand>> {
    vec![
        if response_in_progress {
            BoundConversationCommand::enabled(
                ConversationCommandId::Stop,
                FlowConversationCommand::Stop,
            )
        } else {
            BoundConversationCommand::enabled(
                ConversationCommandId::Send,
                FlowConversationCommand::Send,
            )
        },
        BoundConversationCommand::enabled(
            ConversationCommandId::Background,
            FlowConversationCommand::Background,
        ),
        if response_in_progress {
            BoundConversationCommand::disabled(
                ConversationCommandId::NewConversation,
                ConversationCommandDisabledReason::ResponseInProgress,
                FlowConversationCommand::NewConversation,
            )
        } else {
            BoundConversationCommand::enabled(
                ConversationCommandId::NewConversation,
                FlowConversationCommand::NewConversation,
            )
        },
        BoundConversationCommand::enabled(
            ConversationCommandId::CopyLastResponse,
            FlowConversationCommand::CopyLastResponse,
        ),
        BoundConversationCommand::disabled(
            ConversationCommandId::Stop,
            ConversationCommandDisabledReason::NoResponseRunning,
            FlowConversationCommand::Stop,
        ),
        BoundConversationCommand::enabled(
            ConversationCommandId::TerminateRuntime,
            FlowConversationCommand::TerminateRuntime,
        ),
    ]
    .into_iter()
    .filter(|command| {
        command.descriptor.id != ConversationCommandId::Stop
            || command.descriptor.availability.is_enabled()
            || !response_in_progress
    })
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatPromptConversationCommand {
    Send,
    Stop,
    Close,
    CopyLastResponse,
}

pub(crate) fn chat_prompt_conversation_commands(
    response_in_progress: bool,
    composer_has_text: bool,
    has_response: bool,
) -> Vec<BoundConversationCommand<ChatPromptConversationCommand>> {
    let mut commands = vec![
        if response_in_progress {
            BoundConversationCommand::enabled(
                ConversationCommandId::Stop,
                ChatPromptConversationCommand::Stop,
            )
        } else if composer_has_text {
            BoundConversationCommand::enabled(
                ConversationCommandId::Send,
                ChatPromptConversationCommand::Send,
            )
        } else {
            BoundConversationCommand::disabled(
                ConversationCommandId::Send,
                ConversationCommandDisabledReason::TypeMessageFirst,
                ChatPromptConversationCommand::Send,
            )
        },
        BoundConversationCommand::enabled(
            ConversationCommandId::Close,
            ChatPromptConversationCommand::Close,
        ),
    ];
    if has_response {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::CopyLastResponse,
            ChatPromptConversationCommand::CopyLastResponse,
        ));
    }
    commands
}

pub(crate) fn match_conversation_command_shortcut<Handler: Copy>(
    commands: &[BoundConversationCommand<Handler>],
    key: &str,
    platform: bool,
    shift: bool,
) -> Option<(Handler, ConversationCommandAvailability)> {
    commands
        .iter()
        .find(|command| match command.descriptor.shortcut {
            Some("↵") => crate::ui_foundation::is_key_enter(key) && !platform && !shift,
            Some("Esc") => crate::ui_foundation::is_key_escape(key) && !platform && !shift,
            Some("⌘.") => platform && !shift && key == ".",
            Some("⌘W") => platform && !shift && key.eq_ignore_ascii_case("w"),
            Some("⌘L") => platform && !shift && key.eq_ignore_ascii_case("l"),
            Some("⇧⌘C") => platform && shift && key.eq_ignore_ascii_case("c"),
            Some("⇧⌘R") => platform && shift && key.eq_ignore_ascii_case("r"),
            Some("⇧⌘⎋") => platform && shift && crate::ui_foundation::is_key_escape(key),
            _ => false,
        })
        .map(|command| (command.handler, command.descriptor.availability))
}

pub(crate) fn resolve_conversation_command_shortcut<Handler: Copy>(
    commands: &[BoundConversationCommand<Handler>],
    key: &str,
    platform: bool,
    shift: bool,
) -> Option<Handler> {
    match match_conversation_command_shortcut(commands, key, platform, shift) {
        Some((handler, ConversationCommandAvailability::Enabled)) => Some(handler),
        Some((_, ConversationCommandAvailability::Disabled { .. })) | None => None,
    }
}

pub(crate) fn validate_conversation_command_bindings<Handler>(
    commands: &[BoundConversationCommand<Handler>],
) -> Result<(), &'static str> {
    let mut ids = std::collections::HashSet::new();
    let mut semantic_ids = std::collections::HashSet::new();
    for command in commands {
        if !ids.insert(command.descriptor.id) {
            return Err("duplicate conversation command id");
        }
        if !semantic_ids.insert(command.descriptor.semantic_action_id) {
            return Err("duplicate conversation semantic action id");
        }
        if command.descriptor.role == ConversationCommandRole::Destructive
            && command.descriptor.confirmation != ConfirmPolicy::Required
        {
            return Err("destructive conversation commands require confirmation");
        }
        if command
            .descriptor
            .availability
            .disabled_reason()
            .is_some_and(str::is_empty)
        {
            return Err("disabled conversation commands require a safe reason");
        }
    }
    Ok(())
}

/// What a conversation row should do about a per-turn copy control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnCopyEligibility {
    /// No control at all — there is nothing meaningful to copy yet.
    Absent,
    /// Control shown and clickable.
    Enabled,
    /// Control shown and clickable, with the streaming activity dot.
    EnabledStreaming,
}

impl TurnCopyEligibility {
    pub(crate) fn is_present(self) -> bool {
        !matches!(self, Self::Absent)
    }

    pub(crate) fn shows_activity_dot(self) -> bool {
        matches!(self, Self::EnabledStreaming)
    }
}

/// Role of the conversation row being considered, reduced to the only
/// distinction the copy affordance cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnCopyRole {
    /// An ordinary assistant answer row.
    Assistant,
    /// Thought, tool, system, and error rows. These carry diagnostics or
    /// internal state, not an answer the user asked for, so they never get a
    /// response-copy control.
    NonAnswer,
}

/// Decide whether a conversation row shows a per-turn copy control.
///
/// Total over its inputs so the negative cases are testable. In particular an
/// empty pending assistant row must be `Absent`: a visible copy button that
/// silently copies nothing is worse than no button, and it is the state a row
/// sits in for the entire gap between submit and first token.
pub(crate) fn turn_copy_eligibility(
    role: TurnCopyRole,
    body_is_empty: bool,
    is_streaming: bool,
) -> TurnCopyEligibility {
    match role {
        TurnCopyRole::NonAnswer => TurnCopyEligibility::Absent,
        TurnCopyRole::Assistant => {
            if body_is_empty {
                // Nothing to put on the pasteboard yet, streaming or not.
                TurnCopyEligibility::Absent
            } else if is_streaming {
                // A partial answer IS copyable — the user may want the part
                // already on screen — and the dot signals more is coming.
                TurnCopyEligibility::EnabledStreaming
            } else {
                TurnCopyEligibility::Enabled
            }
        }
    }
}

/// Everything a surface must supply to render the shared copy control.
pub(crate) struct ConversationCopyButtonSpec {
    /// GPUI element id. Must be stable across frames for the same row.
    pub id: SharedString,
    /// Semantic id projected for `getElements`/probes.
    pub fidelity_id: SharedString,
    /// Semantic id for the streaming dot, projected separately so a probe can
    /// distinguish "copy present" from "copy present and still streaming".
    pub activity_fidelity_id: SharedString,
    pub eligibility: TurnCopyEligibility,
    /// Animation key discriminator, so two rows' dots animate independently.
    pub animation_index: usize,
}

/// Render the shared per-turn copy control.
///
/// Returns `None` when the row is not eligible, so callers cannot accidentally
/// paint a disabled-looking button for a row that has nothing to copy.
pub(crate) fn render_conversation_copy_button(
    spec: ConversationCopyButtonSpec,
    style: &ConversationStyleDef,
    theme: &crate::theme::Theme,
    on_copy: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Option<gpui::Stateful<gpui::Div>> {
    if !spec.eligibility.is_present() {
        return None;
    }

    let actions = style.actions;
    let theme_colors = &theme.colors;
    let hover_bg = crate::theme::hover_overlay_bg(theme, actions.button_hover_bg_alpha as u8);
    let hover_opacity = actions.button_hover_opacity;
    let accent = theme_colors.accent.selected;
    let icon_color = theme_colors.text.secondary;
    let activity_fidelity_id = spec.activity_fidelity_id.clone();
    let animation_index = spec.animation_index;

    let control = div()
        .id(spec.id)
        .debug_selector(move || spec.fidelity_id.to_string())
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(px(actions.button_size))
        .h(px(actions.button_size))
        .rounded(px(actions.button_radius))
        .cursor_pointer()
        .opacity(actions.button_opacity)
        .hover(move |s| s.opacity(hover_opacity).bg(hover_bg))
        .child(
            svg()
                .path(IconName::Copy.asset_path())
                .size(px(actions.icon_size))
                .text_color(rgb(icon_color)),
        )
        .when(spec.eligibility.shows_activity_dot(), move |slot| {
            slot.child(
                div()
                    .debug_selector(move || activity_fidelity_id.to_string())
                    .absolute()
                    .right(px(actions.activity_dot_inset))
                    .bottom(px(actions.activity_dot_inset))
                    .size(px(actions.activity_dot_size))
                    .rounded(px(999.0))
                    .bg(rgb(accent))
                    .with_animation(
                        ("conversation-turn-streaming-dot-pulse", animation_index),
                        Animation::new(std::time::Duration::from_millis(actions.activity_pulse_ms))
                            .repeat(),
                        |style, delta| {
                            let sine = (delta * std::f32::consts::PI * 2.0).sin();
                            style.opacity(0.65 + (0.35 * ((sine + 1.0) / 2.0)))
                        },
                    ),
            )
        })
        .on_click(move |event, window, cx| on_copy(event, window, cx));

    Some(control)
}

#[cfg(test)]
mod conversation_actions_tests {
    use super::*;

    #[test]
    fn command_bindings_require_unique_ids_safe_reasons_and_confirmation() {
        let commands = vec![
            BoundConversationCommand::enabled(ConversationCommandId::Send, "send"),
            BoundConversationCommand::disabled(
                ConversationCommandId::NewConversation,
                ConversationCommandDisabledReason::ResponseInProgress,
                "new",
            ),
            BoundConversationCommand::enabled(ConversationCommandId::DeleteConversation, "delete"),
        ];
        validate_conversation_command_bindings(&commands).unwrap();
        let delete = commands
            .iter()
            .find(|command| command.descriptor.id == ConversationCommandId::DeleteConversation)
            .unwrap();
        assert_eq!(delete.descriptor.confirmation, ConfirmPolicy::Required);
        assert_eq!(delete.descriptor.role, ConversationCommandRole::Destructive);
        assert_eq!(
            commands[1].descriptor.availability.disabled_reason(),
            Some("Stop the current response first.")
        );

        let duplicate = vec![
            BoundConversationCommand::enabled(ConversationCommandId::Send, "one"),
            BoundConversationCommand::enabled(ConversationCommandId::Send, "two"),
        ];
        assert_eq!(
            validate_conversation_command_bindings(&duplicate),
            Err("duplicate conversation command id")
        );
    }

    #[test]
    fn unsupported_commands_are_absent_from_host_bindings() {
        let chat_prompt = vec![
            BoundConversationCommand::enabled(ConversationCommandId::Send, "send"),
            BoundConversationCommand::enabled(ConversationCommandId::Close, "close"),
        ];
        assert!(chat_prompt.iter().all(|command| !matches!(
            command.descriptor.id,
            ConversationCommandId::Background | ConversationCommandId::NewConversation
        )));
        assert!(chat_prompt
            .iter()
            .all(|command| command.descriptor.availability.is_enabled()));
    }

    #[test]
    fn host_adapters_expose_only_bound_commands_with_typed_availability() {
        let agent = agent_chat_conversation_commands(AgentChatConversationCommandFacts {
            response_in_progress: true,
            waiting_for_permission: false,
            context_preparing: false,
            composer_has_text: true,
            retry_available: false,
            has_response: true,
        });
        validate_conversation_command_bindings(&agent).unwrap();
        assert!(agent.iter().any(|command| {
            command.handler == AgentChatConversationCommand::Stop
                && command.descriptor.availability.is_enabled()
        }));
        assert!(agent.iter().any(|command| {
            command.handler == AgentChatConversationCommand::NewConversation
                && command.descriptor.availability.disabled_reason()
                    == Some("Stop the current response first.")
        }));

        let flow = flow_conversation_commands(false);
        validate_conversation_command_bindings(&flow).unwrap();
        assert!(flow.iter().any(|command| {
            command.handler == FlowConversationCommand::Stop
                && command.descriptor.availability.disabled_reason()
                    == Some("No response is running.")
        }));

        let prompt = chat_prompt_conversation_commands(false, false, false);
        validate_conversation_command_bindings(&prompt).unwrap();
        assert!(prompt.iter().all(|command| !matches!(
            command.descriptor.id,
            ConversationCommandId::Background | ConversationCommandId::NewConversation
        )));
    }

    #[test]
    fn completed_assistant_answer_has_turn_copy() {
        assert_eq!(
            turn_copy_eligibility(TurnCopyRole::Assistant, false, false),
            TurnCopyEligibility::Enabled
        );
    }

    #[test]
    fn streaming_partial_answer_has_turn_copy_with_activity_dot() {
        let eligibility = turn_copy_eligibility(TurnCopyRole::Assistant, false, true);
        assert_eq!(eligibility, TurnCopyEligibility::EnabledStreaming);
        assert!(eligibility.is_present());
        assert!(eligibility.shows_activity_dot());
    }

    /// The row sits in this state for the whole gap between submit and first
    /// token. A button here would copy an empty string while looking like it
    /// worked.
    #[test]
    fn empty_pending_assistant_row_has_no_turn_copy() {
        assert_eq!(
            turn_copy_eligibility(TurnCopyRole::Assistant, true, true),
            TurnCopyEligibility::Absent
        );
        assert_eq!(
            turn_copy_eligibility(TurnCopyRole::Assistant, true, false),
            TurnCopyEligibility::Absent
        );
    }

    #[test]
    fn thought_tool_system_and_error_rows_have_no_turn_copy() {
        for streaming in [true, false] {
            for empty in [true, false] {
                assert_eq!(
                    turn_copy_eligibility(TurnCopyRole::NonAnswer, empty, streaming),
                    TurnCopyEligibility::Absent,
                    "non-answer rows never expose response copy \
                     (empty={empty}, streaming={streaming})"
                );
            }
        }
    }

    /// Exhaustive: every (role, empty, streaming) combination is decided, and
    /// exactly the two non-empty assistant cases are present.
    #[test]
    fn eligibility_is_total_and_only_non_empty_assistant_rows_are_present() {
        let mut present = 0;
        for role in [TurnCopyRole::Assistant, TurnCopyRole::NonAnswer] {
            for empty in [true, false] {
                for streaming in [true, false] {
                    if turn_copy_eligibility(role, empty, streaming).is_present() {
                        present += 1;
                        assert_eq!(role, TurnCopyRole::Assistant);
                        assert!(!empty);
                    }
                }
            }
        }
        assert_eq!(present, 2, "only streaming + settled non-empty assistant");
    }

    #[test]
    fn action_metrics_come_from_the_shared_style_owner() {
        let style = crate::components::conversation_style::production_conversation_style();
        // Lifted verbatim from Flow's original control so the port cannot
        // silently resize the hit target.
        assert_eq!(style.actions.button_size, 24.0);
        assert_eq!(style.actions.button_radius, 4.0);
        assert_eq!(style.actions.button_opacity, 0.7);
        assert_eq!(style.actions.icon_size, 16.0);
        assert_eq!(style.actions.activity_dot_size, 7.0);
    }
}
