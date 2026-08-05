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
    ConversationHistory,
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
    HiddenDraftMustBeResolved,
    RuntimeAlreadyDetached,
    ActiveWorkCannotSurviveDismissal,
}

impl ConversationCommandDisabledReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::TypeMessageFirst => "Type a message first.",
            Self::ContextStillPreparing => "Wait for context to finish loading.",
            Self::ResponseInProgress => "Stop the current response first.",
            Self::NoResponseRunning => "No response is running.",
            Self::WaitingForPermission => "Resolve the permission request first.",
            Self::HiddenDraftMustBeResolved => {
                "Return to Current and send or clear the draft first."
            }
            Self::RuntimeAlreadyDetached => "The runtime is already terminated.",
            Self::ActiveWorkCannotSurviveDismissal => {
                "Stop the current response first; this host cannot keep it running after you leave."
            }
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

/// The user gesture that asked a conversation host to leave its current
/// presentation. This is intentionally separate from Stop: no dismissal
/// trigger is ever allowed to cancel work as a side effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationDismissTrigger {
    BackButton,
    CloseButton,
    Escape,
    CommandW,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationOverlayKind {
    BlockingModal,
    Actions,
    AttachmentPortal,
    ComposerPicker,
}

/// Whether a host retains a strong, resumable owner for active work after its
/// visible conversation surface is dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveWorkDismissal {
    Survives,
    RequiresExplicitStop,
}

impl Default for ActiveWorkDismissal {
    fn default() -> Self {
        // Fail closed. A host must explicitly prove that active work survives.
        Self::RequiresExplicitStop
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ConversationOverlayFacts {
    pub(crate) blocking_modal_open: bool,
    pub(crate) actions_open: bool,
    pub(crate) attachment_portal_open: bool,
    pub(crate) composer_picker_open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConversationDismissFacts {
    pub(crate) overlays: ConversationOverlayFacts,
    pub(crate) response_in_progress: bool,
    pub(crate) active_work: ActiveWorkDismissal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationDismissDecision {
    DismissOverlay(ConversationOverlayKind),
    DismissConversation,
    Blocked(ConversationCommandDisabledReason),
}

/// Resolve one dismissal gesture. Exactly one transition is selected, with
/// the top-most overlay winning before the host's return route.
pub(crate) fn resolve_conversation_dismissal(
    facts: ConversationDismissFacts,
    _trigger: ConversationDismissTrigger,
) -> ConversationDismissDecision {
    if facts.overlays.blocking_modal_open {
        ConversationDismissDecision::DismissOverlay(ConversationOverlayKind::BlockingModal)
    } else if facts.overlays.actions_open {
        ConversationDismissDecision::DismissOverlay(ConversationOverlayKind::Actions)
    } else if facts.overlays.attachment_portal_open {
        ConversationDismissDecision::DismissOverlay(ConversationOverlayKind::AttachmentPortal)
    } else if facts.overlays.composer_picker_open {
        ConversationDismissDecision::DismissOverlay(ConversationOverlayKind::ComposerPicker)
    } else if facts.response_in_progress
        && facts.active_work == ActiveWorkDismissal::RequiresExplicitStop
    {
        ConversationDismissDecision::Blocked(
            ConversationCommandDisabledReason::ActiveWorkCannotSurviveDismissal,
        )
    } else {
        ConversationDismissDecision::DismissConversation
    }
}

/// Fail-closed execution receipt shared by Actions, keys, footer controls, and
/// semantic automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConversationCommandExecution {
    Executed,
    Disabled(ConversationCommandDisabledReason),
    Unsupported,
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
        ConversationHistory => (
            "Conversation History…",
            None,
            ConversationCommandRole::Secondary,
            ConfirmPolicy::None,
            "conversation.history",
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
            None,
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

pub(crate) fn conversation_command_descriptor(
    id: ConversationCommandId,
    availability: ConversationCommandAvailability,
) -> ConversationCommandDescriptor {
    let (label, shortcut, role, confirmation, semantic_action_id) = command_metadata(id);
    ConversationCommandDescriptor {
        id,
        label,
        shortcut,
        role,
        availability,
        confirmation,
        semantic_action_id,
    }
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
        Self {
            descriptor: conversation_command_descriptor(id, availability),
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
    pub(crate) dismiss_installed: bool,
    pub(crate) active_work: ActiveWorkDismissal,
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
    if facts.dismiss_installed {
        commands.push(
            if facts.response_in_progress
                && facts.active_work == ActiveWorkDismissal::RequiresExplicitStop
            {
                BoundConversationCommand::disabled(
                    ConversationCommandId::Close,
                    ConversationCommandDisabledReason::ActiveWorkCannotSurviveDismissal,
                    AgentChatConversationCommand::Close,
                )
            } else {
                BoundConversationCommand::enabled(
                    ConversationCommandId::Close,
                    AgentChatConversationCommand::Close,
                )
            },
        );
    }
    commands
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowConversationCommand {
    Send,
    Stop,
    Background,
    BackToCurrent,
    NewConversation,
    ConversationHistory,
    ContinueAsNewConversation,
    DeleteConversation,
    CopyLastResponse,
    TerminateRuntime,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FlowConversationCommandFacts {
    pub(crate) response_in_progress: bool,
    pub(crate) viewing_archive: bool,
    pub(crate) has_archives: bool,
    pub(crate) selected_has_response: bool,
    pub(crate) composer_has_text: bool,
    pub(crate) hidden_draft_exists: bool,
    pub(crate) runtime_attached: bool,
}

pub(crate) fn flow_conversation_commands(
    response_in_progress: bool,
) -> Vec<BoundConversationCommand<FlowConversationCommand>> {
    flow_conversation_commands_for_facts(FlowConversationCommandFacts {
        response_in_progress,
        selected_has_response: true,
        composer_has_text: true,
        runtime_attached: true,
        ..FlowConversationCommandFacts::default()
    })
}

pub(crate) fn flow_conversation_commands_for_facts(
    facts: FlowConversationCommandFacts,
) -> Vec<BoundConversationCommand<FlowConversationCommand>> {
    if facts.viewing_archive {
        let mut commands = vec![
            BoundConversationCommand::enabled(
                ConversationCommandId::Back,
                FlowConversationCommand::BackToCurrent,
            ),
            if facts.hidden_draft_exists {
                BoundConversationCommand::disabled(
                    ConversationCommandId::ContinueAsNewConversation,
                    ConversationCommandDisabledReason::HiddenDraftMustBeResolved,
                    FlowConversationCommand::ContinueAsNewConversation,
                )
            } else {
                BoundConversationCommand::enabled(
                    ConversationCommandId::ContinueAsNewConversation,
                    FlowConversationCommand::ContinueAsNewConversation,
                )
            },
            BoundConversationCommand::enabled(
                ConversationCommandId::DeleteConversation,
                FlowConversationCommand::DeleteConversation,
            ),
        ];
        if facts.selected_has_response {
            commands.insert(
                1,
                BoundConversationCommand::enabled(
                    ConversationCommandId::CopyLastResponse,
                    FlowConversationCommand::CopyLastResponse,
                ),
            );
        }
        return commands;
    }

    let mut commands = vec![
        if facts.response_in_progress {
            BoundConversationCommand::enabled(
                ConversationCommandId::Stop,
                FlowConversationCommand::Stop,
            )
        } else if facts.composer_has_text {
            BoundConversationCommand::enabled(
                ConversationCommandId::Send,
                FlowConversationCommand::Send,
            )
        } else {
            BoundConversationCommand::disabled(
                ConversationCommandId::Send,
                ConversationCommandDisabledReason::TypeMessageFirst,
                FlowConversationCommand::Send,
            )
        },
        BoundConversationCommand::enabled(
            ConversationCommandId::Background,
            FlowConversationCommand::Background,
        ),
        if facts.response_in_progress {
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
    ];
    if facts.has_archives {
        commands.push(if facts.response_in_progress {
            BoundConversationCommand::disabled(
                ConversationCommandId::ConversationHistory,
                ConversationCommandDisabledReason::ResponseInProgress,
                FlowConversationCommand::ConversationHistory,
            )
        } else {
            BoundConversationCommand::enabled(
                ConversationCommandId::ConversationHistory,
                FlowConversationCommand::ConversationHistory,
            )
        });
    }
    if facts.selected_has_response {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::CopyLastResponse,
            FlowConversationCommand::CopyLastResponse,
        ));
    }
    if !facts.response_in_progress {
        commands.push(BoundConversationCommand::disabled(
            ConversationCommandId::Stop,
            ConversationCommandDisabledReason::NoResponseRunning,
            FlowConversationCommand::Stop,
        ));
    }
    commands.push(if facts.runtime_attached || facts.response_in_progress {
        BoundConversationCommand::enabled(
            ConversationCommandId::TerminateRuntime,
            FlowConversationCommand::TerminateRuntime,
        )
    } else {
        BoundConversationCommand::disabled(
            ConversationCommandId::TerminateRuntime,
            ConversationCommandDisabledReason::RuntimeAlreadyDetached,
            FlowConversationCommand::TerminateRuntime,
        )
    });
    commands.push(if facts.response_in_progress {
        BoundConversationCommand::disabled(
            ConversationCommandId::DeleteConversation,
            ConversationCommandDisabledReason::ResponseInProgress,
            FlowConversationCommand::DeleteConversation,
        )
    } else {
        BoundConversationCommand::enabled(
            ConversationCommandId::DeleteConversation,
            FlowConversationCommand::DeleteConversation,
        )
    });
    commands
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatPromptConversationCommand {
    Send,
    Stop,
    Retry,
    Dismiss,
    CopyLastResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ChatPromptConversationCommandFacts {
    pub(crate) response_in_progress: bool,
    pub(crate) composer_has_text: bool,
    pub(crate) has_response: bool,
    pub(crate) submit_installed: bool,
    pub(crate) stop_installed: bool,
    pub(crate) retry_available: bool,
    pub(crate) dismiss_command: Option<ConversationCommandId>,
    pub(crate) active_work: ActiveWorkDismissal,
}

pub(crate) fn validate_chat_prompt_command_facts(
    facts: ChatPromptConversationCommandFacts,
) -> Result<(), &'static str> {
    if facts.dismiss_command.is_some_and(|id| {
        !matches!(
            id,
            ConversationCommandId::Back | ConversationCommandId::Close
        )
    }) {
        return Err("ChatPrompt dismiss command must be Back or Close");
    }
    Ok(())
}

pub(crate) fn chat_prompt_conversation_commands_for_facts(
    facts: ChatPromptConversationCommandFacts,
) -> Vec<BoundConversationCommand<ChatPromptConversationCommand>> {
    debug_assert!(validate_chat_prompt_command_facts(facts).is_ok());
    let mut commands = Vec::new();

    if facts.response_in_progress {
        if facts.stop_installed {
            commands.push(BoundConversationCommand::enabled(
                ConversationCommandId::Stop,
                ChatPromptConversationCommand::Stop,
            ));
        }
    } else if facts.submit_installed {
        commands.push(if facts.composer_has_text {
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
        });
    }

    if !facts.response_in_progress && facts.retry_available {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::Retry,
            ChatPromptConversationCommand::Retry,
        ));
    }

    if let Some(dismiss_command @ (ConversationCommandId::Back | ConversationCommandId::Close)) =
        facts.dismiss_command
    {
        commands.push(
            if facts.response_in_progress
                && facts.active_work == ActiveWorkDismissal::RequiresExplicitStop
            {
                BoundConversationCommand::disabled(
                    dismiss_command,
                    ConversationCommandDisabledReason::ActiveWorkCannotSurviveDismissal,
                    ChatPromptConversationCommand::Dismiss,
                )
            } else {
                BoundConversationCommand::enabled(
                    dismiss_command,
                    ChatPromptConversationCommand::Dismiss,
                )
            },
        );
    }

    if facts.has_response {
        commands.push(BoundConversationCommand::enabled(
            ConversationCommandId::CopyLastResponse,
            ChatPromptConversationCommand::CopyLastResponse,
        ));
    }
    commands
}

/// Transitional wrapper retained while all constructors migrate to explicit
/// capability facts. It preserves the old standalone assumptions only for
/// test callers; production ChatPrompt projects through the facts API.
pub(crate) fn chat_prompt_conversation_commands(
    response_in_progress: bool,
    composer_has_text: bool,
    has_response: bool,
) -> Vec<BoundConversationCommand<ChatPromptConversationCommand>> {
    chat_prompt_conversation_commands_for_facts(ChatPromptConversationCommandFacts {
        response_in_progress,
        composer_has_text,
        has_response,
        submit_installed: true,
        stop_installed: true,
        dismiss_command: Some(ConversationCommandId::Close),
        active_work: ActiveWorkDismissal::Survives,
        ..Default::default()
    })
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

/// Return the newest assistant response that contains non-whitespace bytes.
/// Whitespace is an eligibility test only; callers copy the original slice.
pub(crate) fn resolve_latest_copyable_assistant_response<'a>(
    responses: impl DoubleEndedIterator<Item = &'a str>,
) -> Option<&'a str> {
    responses.rev().find(|response| !response.trim().is_empty())
}

/// Privacy-safe proof that an exact copy happened. The fingerprint is salted
/// for this process and cannot be used as a stable content identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversationCopyReceipt {
    pub(crate) byte_len: usize,
    pub(crate) fingerprint: String,
}

pub(crate) fn write_exact_conversation_copy(
    content: &str,
    cx: &mut gpui::App,
) -> ConversationCopyReceipt {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(content.to_string()));
    let receipt = ConversationCopyReceipt {
        byte_len: content.len(),
        fingerprint: crate::ai::message_parts::run_scoped_fingerprint(content),
    };
    tracing::info!(
        target: "script_kit::conversation_copy",
        operation = "exact_assistant_copy",
        byte_len = receipt.byte_len,
        fingerprint = %receipt.fingerprint,
        "Conversation response copied"
    );
    receipt
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
            dismiss_installed: true,
            active_work: ActiveWorkDismissal::RequiresExplicitStop,
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
    fn flow_active_working_and_archive_command_sets_are_exact() {
        let active = flow_conversation_commands_for_facts(FlowConversationCommandFacts {
            composer_has_text: true,
            has_archives: true,
            selected_has_response: true,
            runtime_attached: true,
            ..Default::default()
        });
        validate_conversation_command_bindings(&active).unwrap();
        assert!(active.iter().any(|command| {
            command.handler == FlowConversationCommand::TerminateRuntime
                && command.descriptor.shortcut.is_none()
                && command.descriptor.confirmation == ConfirmPolicy::Required
        }));
        assert!(active.iter().any(|command| {
            command.handler == FlowConversationCommand::ConversationHistory
                && command.descriptor.availability.is_enabled()
        }));

        let working = flow_conversation_commands_for_facts(FlowConversationCommandFacts {
            response_in_progress: true,
            has_archives: true,
            runtime_attached: true,
            ..Default::default()
        });
        assert!(working.iter().any(|command| {
            command.handler == FlowConversationCommand::NewConversation
                && command.descriptor.availability.disabled_reason()
                    == Some("Stop the current response first.")
        }));
        assert!(working.iter().any(|command| {
            command.handler == FlowConversationCommand::DeleteConversation
                && !command.descriptor.availability.is_enabled()
        }));

        let archive = flow_conversation_commands_for_facts(FlowConversationCommandFacts {
            viewing_archive: true,
            selected_has_response: true,
            ..Default::default()
        });
        let archive_ids: Vec<_> = archive
            .iter()
            .map(|command| command.descriptor.id)
            .collect();
        assert_eq!(
            archive_ids,
            vec![
                ConversationCommandId::Back,
                ConversationCommandId::CopyLastResponse,
                ConversationCommandId::ContinueAsNewConversation,
                ConversationCommandId::DeleteConversation,
            ]
        );
        assert!(archive.iter().all(|command| !matches!(
            command.handler,
            FlowConversationCommand::Send
                | FlowConversationCommand::Stop
                | FlowConversationCommand::Background
                | FlowConversationCommand::NewConversation
                | FlowConversationCommand::ConversationHistory
                | FlowConversationCommand::TerminateRuntime
        )));

        let blocked_continue = flow_conversation_commands_for_facts(FlowConversationCommandFacts {
            viewing_archive: true,
            hidden_draft_exists: true,
            ..Default::default()
        });
        assert!(blocked_continue.iter().any(|command| {
            command.handler == FlowConversationCommand::ContinueAsNewConversation
                && command.descriptor.availability.disabled_reason()
                    == Some("Return to Current and send or clear the draft first.")
        }));
    }

    #[test]
    fn dismissal_resolver_closes_one_overlay_before_considering_active_work() {
        let triggers = [
            ConversationDismissTrigger::BackButton,
            ConversationDismissTrigger::CloseButton,
            ConversationDismissTrigger::Escape,
            ConversationDismissTrigger::CommandW,
        ];
        let precedence = [
            (
                ConversationOverlayFacts {
                    blocking_modal_open: true,
                    actions_open: true,
                    attachment_portal_open: true,
                    composer_picker_open: true,
                },
                ConversationOverlayKind::BlockingModal,
            ),
            (
                ConversationOverlayFacts {
                    actions_open: true,
                    attachment_portal_open: true,
                    composer_picker_open: true,
                    ..Default::default()
                },
                ConversationOverlayKind::Actions,
            ),
            (
                ConversationOverlayFacts {
                    attachment_portal_open: true,
                    composer_picker_open: true,
                    ..Default::default()
                },
                ConversationOverlayKind::AttachmentPortal,
            ),
            (
                ConversationOverlayFacts {
                    composer_picker_open: true,
                    ..Default::default()
                },
                ConversationOverlayKind::ComposerPicker,
            ),
        ];

        for trigger in triggers {
            for (overlays, expected) in precedence {
                assert_eq!(
                    resolve_conversation_dismissal(
                        ConversationDismissFacts {
                            overlays,
                            response_in_progress: true,
                            active_work: ActiveWorkDismissal::RequiresExplicitStop,
                        },
                        trigger,
                    ),
                    ConversationDismissDecision::DismissOverlay(expected),
                );
            }
        }
    }

    #[test]
    fn dismissal_blocks_only_active_work_that_cannot_survive() {
        let blocked = resolve_conversation_dismissal(
            ConversationDismissFacts {
                overlays: ConversationOverlayFacts::default(),
                response_in_progress: true,
                active_work: ActiveWorkDismissal::RequiresExplicitStop,
            },
            ConversationDismissTrigger::Escape,
        );
        assert_eq!(
            blocked,
            ConversationDismissDecision::Blocked(
                ConversationCommandDisabledReason::ActiveWorkCannotSurviveDismissal
            )
        );
        assert_eq!(
            ConversationCommandDisabledReason::ActiveWorkCannotSurviveDismissal.as_str(),
            "Stop the current response first; this host cannot keep it running after you leave."
        );

        for facts in [
            ConversationDismissFacts {
                overlays: ConversationOverlayFacts::default(),
                response_in_progress: true,
                active_work: ActiveWorkDismissal::Survives,
            },
            ConversationDismissFacts {
                overlays: ConversationOverlayFacts::default(),
                response_in_progress: false,
                active_work: ActiveWorkDismissal::RequiresExplicitStop,
            },
        ] {
            assert_eq!(
                resolve_conversation_dismissal(facts, ConversationDismissTrigger::CommandW),
                ConversationDismissDecision::DismissConversation
            );
        }
    }

    #[test]
    fn chat_prompt_commands_are_capability_derived_and_fail_closed() {
        let unsupported =
            chat_prompt_conversation_commands_for_facts(ChatPromptConversationCommandFacts {
                response_in_progress: true,
                composer_has_text: true,
                has_response: true,
                submit_installed: false,
                stop_installed: false,
                retry_available: false,
                dismiss_command: Some(ConversationCommandId::Back),
                active_work: ActiveWorkDismissal::RequiresExplicitStop,
            });
        assert!(unsupported.iter().all(|binding| !matches!(
            binding.handler,
            ChatPromptConversationCommand::Send
                | ChatPromptConversationCommand::Stop
                | ChatPromptConversationCommand::Retry
        )));
        let dismiss = unsupported
            .iter()
            .find(|binding| binding.handler == ChatPromptConversationCommand::Dismiss)
            .expect("installed dismiss route is projected");
        assert_eq!(dismiss.descriptor.id, ConversationCommandId::Back);
        assert_eq!(
            dismiss.descriptor.availability.disabled_reason(),
            Some(
                "Stop the current response first; this host cannot keep it running after you leave."
            )
        );
        assert!(unsupported
            .iter()
            .any(|binding| { binding.handler == ChatPromptConversationCommand::CopyLastResponse }));

        assert_eq!(
            validate_chat_prompt_command_facts(ChatPromptConversationCommandFacts {
                dismiss_command: Some(ConversationCommandId::Background),
                ..Default::default()
            }),
            Err("ChatPrompt dismiss command must be Back or Close")
        );
    }

    #[test]
    fn latest_assistant_copy_uses_trim_only_for_eligibility() {
        let responses = ["first", "\t \n", "  exact final\r\n"];
        assert_eq!(
            resolve_latest_copyable_assistant_response(responses.into_iter()),
            Some("  exact final\r\n")
        );
        assert_eq!(
            resolve_latest_copyable_assistant_response(["", " \n\t"].into_iter()),
            None
        );
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
