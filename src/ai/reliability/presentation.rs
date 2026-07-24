//! Pure recovery-state projection shared by every AI surface.
//!
//! This module owns copy and action semantics only. It performs no GPUI,
//! provider, process, persistence, clock, or navigation effects.

use std::collections::HashSet;

use gpui::SharedString;
use sk_protocol::ai_reliability::{
    AiFailure, AiFailureKind, AiOperationState, AiPhase, AiRecoveryAction, AiSelectionState,
    AiSurfaceIdentity, AuthenticationFailure, CapabilityFailure, ClientKind, ConfigurationFailure,
    ConnectivityFailure, DisabledReason, InputFailure, PermissionFailure, PolicyFailure,
    PreservationReceipt, ProtocolComponent, ProtocolFailure, ReattachAvailability,
    RecoveryActionKind, RecoveryRole, RuntimeFailure, SelectionOrigin,
};

pub const AI_RECOVERY_CARD_ID: &str = "ai-recovery-card";
pub const AI_RECOVERY_TITLE_ID: &str = "ai-recovery-title";
pub const AI_RECOVERY_BODY_ID: &str = "ai-recovery-body";
pub const AI_RECOVERY_PROGRESS_ID: &str = "ai-recovery-progress";
pub const AI_RECOVERY_DISMISS_ID: &str = "ai-recovery-dismiss";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRecoveryLayout {
    ComposerInline,
    TranscriptCard,
    BlockingPanel,
    DeskRow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRecoveryTone {
    Warning,
    Error,
    Progress,
    Success,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRecoveryProgress {
    pub label: SharedString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRecoveryActionSpec {
    pub semantic_id: &'static str,
    pub label: SharedString,
    pub action: AiRecoveryAction,
    pub role: RecoveryRole,
    pub enabled: bool,
    pub disabled_reason: Option<DisabledReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRecoveryCardSpec {
    pub semantic_id: &'static str,
    pub layout: AiRecoveryLayout,
    pub tone: AiRecoveryTone,
    pub title: SharedString,
    pub body: SharedString,
    pub preservation_note: Option<SharedString>,
    pub progress: Option<AiRecoveryProgress>,
    pub actions: Vec<AiRecoveryActionSpec>,
    pub dismissible: bool,
}

#[derive(Debug, Clone)]
pub struct SurfaceRecoveryCapabilities {
    supported_actions: HashSet<RecoveryActionKind>,
    pub layout_override: Option<AiRecoveryLayout>,
    pub allow_dismiss: bool,
    pub waiting_for_external_state: bool,
}

impl SurfaceRecoveryCapabilities {
    pub fn all() -> Self {
        Self {
            supported_actions: ALL_RECOVERY_ACTION_KINDS.into_iter().collect(),
            layout_override: None,
            allow_dismiss: true,
            waiting_for_external_state: false,
        }
    }

    pub fn only(actions: impl IntoIterator<Item = RecoveryActionKind>) -> Self {
        Self {
            supported_actions: actions.into_iter().collect(),
            ..Self::all()
        }
    }

    pub fn supports(&self, kind: RecoveryActionKind) -> bool {
        self.supported_actions.contains(&kind)
    }

    pub fn layout(mut self, layout: AiRecoveryLayout) -> Self {
        self.layout_override = Some(layout);
        self
    }

    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.allow_dismiss = dismissible;
        self
    }

    pub fn waiting_for_external_state(mut self, waiting: bool) -> Self {
        self.waiting_for_external_state = waiting;
        self
    }
}

impl Default for SurfaceRecoveryCapabilities {
    fn default() -> Self {
        Self::all()
    }
}

const ALL_RECOVERY_ACTION_KINDS: [RecoveryActionKind; 17] = [
    RecoveryActionKind::Retry,
    RecoveryActionKind::UseCurrentResults,
    RecoveryActionKind::ContinueInAgentChat,
    RecoveryActionKind::ChooseCompatibleModel,
    RecoveryActionKind::ChooseProvider,
    RecoveryActionKind::ChooseProfile,
    RecoveryActionKind::UpdateClient,
    RecoveryActionKind::CheckAgain,
    RecoveryActionKind::SignIn,
    RecoveryActionKind::SwitchAccount,
    RecoveryActionKind::ConfigureProvider,
    RecoveryActionKind::RepairComponent,
    RecoveryActionKind::Reattach,
    RecoveryActionKind::RethreadFlow,
    RecoveryActionKind::RestartFlowRun,
    RecoveryActionKind::TrimContext,
    RecoveryActionKind::CopyDetails,
];

pub fn project_recovery(
    identity: &AiSurfaceIdentity,
    state: &AiOperationState,
    surface_capabilities: &SurfaceRecoveryCapabilities,
) -> Option<AiRecoveryCardSpec> {
    let layout = surface_capabilities
        .layout_override
        .unwrap_or_else(|| layout_for_identity(identity));
    match &state.phase {
        AiPhase::AwaitingRecovery { failure, plan } => {
            let copy = failure_copy(failure);
            let mut actions = plan
                .options
                .iter()
                .filter_map(|option| {
                    if option.kind == RecoveryActionKind::Retry && !state.retry.manual_available() {
                        return None;
                    }
                    if option.kind == RecoveryActionKind::CopyDetails
                        && (state.diagnostic.is_none()
                            || !surface_capabilities.supports(option.kind))
                    {
                        return None;
                    }
                    let mut enabled = option.enabled && surface_capabilities.supports(option.kind);
                    let mut disabled_reason = option.disabled_reason.clone();
                    if !surface_capabilities.supports(option.kind) {
                        enabled = false;
                        disabled_reason = Some(DisabledReason::UnsupportedBySurface);
                    }
                    let action = recovery_action(option.kind, failure)?;
                    Some(AiRecoveryActionSpec {
                        semantic_id: semantic_id_for_action(option.kind),
                        label: label_for_action(option.kind).into(),
                        action,
                        role: option.role,
                        enabled,
                        disabled_reason,
                    })
                })
                .collect::<Vec<_>>();
            normalize_action_order(&mut actions);
            let progress = if actions
                .iter()
                .any(|action| action.enabled && action.role != RecoveryRole::Diagnostic)
            {
                None
            } else {
                Some(AiRecoveryProgress {
                    label: if surface_capabilities.waiting_for_external_state {
                        "Waiting for the AI service to become available.".into()
                    } else {
                        "Recovery is waiting for an available action.".into()
                    },
                })
            };
            Some(AiRecoveryCardSpec {
                semantic_id: AI_RECOVERY_CARD_ID,
                layout,
                tone: AiRecoveryTone::Error,
                title: copy.title.into(),
                body: copy.body.into(),
                preservation_note: preservation_note(identity, &state.work),
                progress,
                actions,
                dismissible: surface_capabilities.allow_dismiss,
            })
        }
        AiPhase::Recovering { action, .. } => Some(AiRecoveryCardSpec {
            semantic_id: AI_RECOVERY_CARD_ID,
            layout,
            tone: AiRecoveryTone::Progress,
            title: "Recovery in progress".into(),
            body: progress_copy(action).into(),
            preservation_note: preservation_note(identity, &state.work),
            progress: Some(AiRecoveryProgress {
                label: "Working…".into(),
            }),
            actions: Vec::new(),
            dismissible: false,
        }),
        AiPhase::Recovered { action, .. } => Some(AiRecoveryCardSpec {
            semantic_id: AI_RECOVERY_CARD_ID,
            layout,
            tone: AiRecoveryTone::Success,
            title: "Recovery complete".into(),
            body: recovered_copy(action, &state.selection).into(),
            preservation_note: preservation_note(identity, &state.work),
            progress: Some(AiRecoveryProgress {
                label: "Ready".into(),
            }),
            actions: Vec::new(),
            dismissible: false,
        }),
        AiPhase::Ready
        | AiPhase::Preflighting { .. }
        | AiPhase::Running { .. }
        | AiPhase::Cancelling { .. }
        | AiPhase::Succeeded { .. }
        | AiPhase::Cancelled { .. }
        | AiPhase::Dismissed { .. } => None,
    }
}

fn layout_for_identity(identity: &AiSurfaceIdentity) -> AiRecoveryLayout {
    match identity {
        AiSurfaceIdentity::QuickAi { .. }
        | AiSurfaceIdentity::AgentChat { .. }
        | AiSurfaceIdentity::FocusedText { .. } => AiRecoveryLayout::ComposerInline,
        AiSurfaceIdentity::FlowConversation { .. } | AiSurfaceIdentity::LegacyChatPrompt { .. } => {
            AiRecoveryLayout::TranscriptCard
        }
        AiSurfaceIdentity::FlowRun { .. } => AiRecoveryLayout::DeskRow,
        AiSurfaceIdentity::Other { .. } => AiRecoveryLayout::BlockingPanel,
    }
}

struct FailureCopy {
    title: &'static str,
    body: &'static str,
}

fn failure_copy(failure: &AiFailure) -> FailureCopy {
    match &failure.kind {
        AiFailureKind::Capability(capability) => match capability {
            CapabilityFailure::ClientTooOld { client, .. } => FailureCopy {
                title: match client {
                    ClientKind::Codex => "Codex needs an update for this model",
                    ClientKind::Pi => "Pi needs an update for this model",
                    ClientKind::Mdflow => "mdflow needs an update for this model",
                    ClientKind::LocalLlm | ClientKind::Other => "AI client update needed",
                },
                body: "Your turn is saved. Choose a compatible model or update the client before retrying.",
            },
            CapabilityFailure::ModelUnavailable { .. } => FailureCopy {
                title: "Model unavailable",
                body: "Choose a compatible model, then try this request again.",
            },
            CapabilityFailure::NoCompatibleModel => FailureCopy {
                title: "No compatible model found",
                body: "Choose another provider or update the AI client to continue.",
            },
            CapabilityFailure::ProfileUnavailable { .. } => FailureCopy {
                title: "Profile unavailable",
                body: "Choose an available AI profile before continuing.",
            },
        },
        AiFailureKind::Policy(policy) => match policy {
            PolicyFailure::QuickAiSearchBudgetExceeded {
                partial_answer_available,
                ..
            } => FailureCopy {
                title: "Quick AI reached its search limit",
                body: if *partial_answer_available {
                    "Use the current results or continue the question in Agent Chat."
                } else {
                    "Continue the question in Agent Chat for deeper research."
                },
            },
            PolicyFailure::QuickAiDeadlineExceeded {
                partial_answer_available,
                ..
            } => FailureCopy {
                title: "Quick AI took too long to finish",
                body: if *partial_answer_available {
                    "Use the current results or continue the question in Agent Chat."
                } else {
                    "Continue the question in Agent Chat for deeper research."
                },
            },
            PolicyFailure::ToolDenied { .. } => FailureCopy {
                title: "AI tool unavailable",
                body: "This request needs a tool that is not available for the current profile.",
            },
        },
        AiFailureKind::Authentication(authentication) => match authentication {
            AuthenticationFailure::Missing => FailureCopy {
                title: "Sign in required",
                body: "Sign in to the selected AI provider, then check again.",
            },
            AuthenticationFailure::Expired => FailureCopy {
                title: "Sign-in expired",
                body: "Sign in again to restore this AI connection.",
            },
            AuthenticationFailure::UsageExhausted => FailureCopy {
                title: "Usage limit reached",
                body: "Switch accounts or choose another configured provider to continue.",
            },
        },
        AiFailureKind::Configuration(configuration) => match configuration {
            ConfigurationFailure::ProviderNotConfigured => FailureCopy {
                title: "Provider setup needed",
                body: "Configure an AI provider before continuing.",
            },
            ConfigurationFailure::NoModelsAvailable => FailureCopy {
                title: "No models available",
                body: "Check the provider configuration or choose another provider.",
            },
            ConfigurationFailure::SidecarMissing => FailureCopy {
                title: "AI component missing",
                body: "Repair the bundled AI component, then check again.",
            },
            ConfigurationFailure::MdflowMissing => FailureCopy {
                title: "Flow engine missing",
                body: "Repair the Flow engine before running this Flow.",
            },
            ConfigurationFailure::InvalidConfiguration => FailureCopy {
                title: "AI setup needs attention",
                body: "Review the current AI configuration before trying again.",
            },
        },
        AiFailureKind::Connectivity(connectivity) => match connectivity {
            ConnectivityFailure::Offline => FailureCopy {
                title: "You appear to be offline",
                body: "Reconnect to the internet, then try again.",
            },
            ConnectivityFailure::Timeout => FailureCopy {
                title: "AI request timed out",
                body: "Your work is saved. Try again when the connection is stable.",
            },
            ConnectivityFailure::RateLimited { .. } => FailureCopy {
                title: "AI service is busy",
                body: "Wait for the retry window, then try again.",
            },
        },
        AiFailureKind::Provider(provider) => match provider {
            sk_protocol::ai_reliability::ProviderFailure::TemporarilyUnavailable => FailureCopy {
                title: "AI service unavailable",
                body: "The provider is temporarily unavailable. Try again shortly.",
            },
            sk_protocol::ai_reliability::ProviderFailure::ServerRejected => FailureCopy {
                title: "AI request rejected",
                body: "Review the request or provider setup before trying again.",
            },
        },
        AiFailureKind::Runtime(runtime) => match runtime {
            RuntimeFailure::SpawnFailed => FailureCopy {
                title: "AI connection could not start",
                body: "Repair the AI component or check its configuration.",
            },
            RuntimeFailure::RuntimeClosed | RuntimeFailure::ChildExited { .. } => FailureCopy {
                title: "AI connection stopped",
                body: "Reconnect or retry without losing the saved work.",
            },
            RuntimeFailure::SessionLost { reattach } => FailureCopy {
                title: "AI session interrupted",
                body: match reattach {
                    ReattachAvailability::Available { .. } => {
                        "Reattach the existing session without resending the turn."
                    }
                    ReattachAvailability::Unavailable => {
                        "Start a new thread when you are ready to continue."
                    }
                },
            },
        },
        AiFailureKind::Protocol(protocol) => match protocol {
            ProtocolFailure::VersionMismatch { .. } => FailureCopy {
                title: "AI component update needed",
                body: "Update or repair the incompatible AI component.",
            },
            ProtocolFailure::SequenceViolation { .. }
            | ProtocolFailure::OrderViolation { .. }
            | ProtocolFailure::MalformedResponse { .. }
            | ProtocolFailure::MissingTerminal { .. } => FailureCopy {
                title: "AI response could not be completed",
                body: "Reconnect or repair the AI component before trying again.",
            },
        },
        AiFailureKind::Permission(permission) => match permission {
            PermissionFailure::PermissionDenied => FailureCopy {
                title: "Permission required",
                body: "Grant the required permission, then check again.",
            },
            PermissionFailure::UserDeniedTool => FailureCopy {
                title: "AI tool was not approved",
                body: "Approve the tool or continue without that action.",
            },
        },
        AiFailureKind::Input(input) => match input {
            InputFailure::MessageTooLarge => FailureCopy {
                title: "Message is too large",
                body: "Shorten the message or remove an attachment before retrying.",
            },
            InputFailure::ContextLimitExceeded => FailureCopy {
                title: "Conversation is too large",
                body: "Trim older context before retrying this request.",
            },
        },
        AiFailureKind::Unknown => FailureCopy {
            title: "AI request did not finish",
            body: "Your work is saved. Try again or view the safe diagnostic details.",
        },
    }
}

fn preservation_note(
    identity: &AiSurfaceIdentity,
    work: &sk_protocol::ai_reliability::AiWorkSnapshot,
) -> Option<SharedString> {
    let preserved = [
        &work.transcript,
        &work.draft,
        &work.attachments,
        &work.partial_output,
    ]
    .iter()
    .any(|receipt| {
        matches!(
            receipt,
            PreservationReceipt::Preserved { .. } | PreservationReceipt::Restorable { .. }
        )
    });
    if !preserved {
        return None;
    }
    Some(
        match identity {
            AiSurfaceIdentity::QuickAi { .. } | AiSurfaceIdentity::FocusedText { .. } => {
                "Your question and current results are saved."
            }
            AiSurfaceIdentity::AgentChat { .. }
            | AiSurfaceIdentity::FlowConversation { .. }
            | AiSurfaceIdentity::LegacyChatPrompt { .. } => {
                "Your conversation and draft are saved."
            }
            AiSurfaceIdentity::FlowRun { .. } => "The Flow definition and prior output are saved.",
            AiSurfaceIdentity::Other { .. } => "Your current work is saved.",
        }
        .into(),
    )
}

fn normalize_action_order(actions: &mut Vec<AiRecoveryActionSpec>) {
    let mut primary_seen = false;
    let mut secondary_count = 0usize;
    actions.retain(|action| match action.role {
        RecoveryRole::Primary if !primary_seen => {
            primary_seen = true;
            true
        }
        RecoveryRole::Primary => false,
        RecoveryRole::Secondary if secondary_count < 2 => {
            secondary_count += 1;
            true
        }
        RecoveryRole::Secondary => false,
        RecoveryRole::Diagnostic => true,
    });
    if !actions
        .iter()
        .any(|action| action.role == RecoveryRole::Primary)
    {
        if let Some(action) = actions
            .iter_mut()
            .find(|action| action.role == RecoveryRole::Secondary)
        {
            action.role = RecoveryRole::Primary;
        }
    }
    actions.sort_by_key(|action| match action.role {
        RecoveryRole::Primary => 0,
        RecoveryRole::Secondary => 1,
        RecoveryRole::Diagnostic => 2,
    });
}

fn recovery_action(kind: RecoveryActionKind, failure: &AiFailure) -> Option<AiRecoveryAction> {
    Some(match kind {
        RecoveryActionKind::Retry => AiRecoveryAction::Retry,
        RecoveryActionKind::UseCurrentResults => AiRecoveryAction::UseCurrentResults,
        RecoveryActionKind::ContinueInAgentChat => AiRecoveryAction::ContinueInAgentChat,
        RecoveryActionKind::ChooseCompatibleModel => {
            AiRecoveryAction::ChooseCompatibleModel { selection: None }
        }
        RecoveryActionKind::ChooseProvider => AiRecoveryAction::ChooseProvider { selection: None },
        RecoveryActionKind::ChooseProfile => AiRecoveryAction::ChooseProfile { selection: None },
        RecoveryActionKind::UpdateClient => AiRecoveryAction::UpdateClient {
            client: match &failure.kind {
                AiFailureKind::Capability(CapabilityFailure::ClientTooOld { client, .. }) => {
                    *client
                }
                _ => ClientKind::Other,
            },
        },
        RecoveryActionKind::CheckAgain => AiRecoveryAction::CheckAgain,
        RecoveryActionKind::SignIn => AiRecoveryAction::SignIn,
        RecoveryActionKind::SwitchAccount => AiRecoveryAction::SwitchAccount,
        RecoveryActionKind::ConfigureProvider => AiRecoveryAction::ConfigureProvider,
        RecoveryActionKind::RepairComponent => AiRecoveryAction::RepairComponent {
            component: component_for_failure(failure),
        },
        RecoveryActionKind::Reattach => AiRecoveryAction::Reattach {
            session: match &failure.kind {
                AiFailureKind::Runtime(RuntimeFailure::SessionLost {
                    reattach: ReattachAvailability::Available { session },
                }) => session.clone(),
                _ => return None,
            },
        },
        RecoveryActionKind::RethreadFlow => AiRecoveryAction::RethreadFlow,
        RecoveryActionKind::RestartFlowRun => AiRecoveryAction::RestartFlowRun,
        RecoveryActionKind::TrimContext => AiRecoveryAction::TrimContext,
        RecoveryActionKind::CopyDetails => AiRecoveryAction::CopyDetails,
    })
}

fn component_for_failure(failure: &AiFailure) -> ProtocolComponent {
    match &failure.kind {
        AiFailureKind::Protocol(ProtocolFailure::VersionMismatch { component, .. })
        | AiFailureKind::Protocol(ProtocolFailure::SequenceViolation { component })
        | AiFailureKind::Protocol(ProtocolFailure::OrderViolation { component })
        | AiFailureKind::Protocol(ProtocolFailure::MalformedResponse { component })
        | AiFailureKind::Protocol(ProtocolFailure::MissingTerminal { component }) => *component,
        AiFailureKind::Configuration(ConfigurationFailure::MdflowMissing) => {
            ProtocolComponent::Mdflow
        }
        AiFailureKind::Configuration(ConfigurationFailure::SidecarMissing) => ProtocolComponent::Pi,
        _ => ProtocolComponent::Provider,
    }
}

fn semantic_id_for_action(kind: RecoveryActionKind) -> &'static str {
    match kind {
        RecoveryActionKind::Retry => "ai-recovery-retry",
        RecoveryActionKind::UseCurrentResults => "ai-recovery-use-current-results",
        RecoveryActionKind::ContinueInAgentChat => "ai-recovery-continue-agent-chat",
        RecoveryActionKind::ChooseCompatibleModel => "ai-recovery-choose-model",
        RecoveryActionKind::ChooseProvider => "ai-recovery-choose-provider",
        RecoveryActionKind::ChooseProfile => "ai-recovery-choose-profile",
        RecoveryActionKind::UpdateClient => "ai-recovery-update-client",
        RecoveryActionKind::CheckAgain => "ai-recovery-check-again",
        RecoveryActionKind::SignIn => "ai-recovery-sign-in",
        RecoveryActionKind::SwitchAccount => "ai-recovery-switch-account",
        RecoveryActionKind::ConfigureProvider => "ai-recovery-configure-provider",
        RecoveryActionKind::RepairComponent => "ai-recovery-repair-component",
        RecoveryActionKind::Reattach => "ai-recovery-reattach",
        RecoveryActionKind::RethreadFlow => "ai-recovery-rethread-flow",
        RecoveryActionKind::RestartFlowRun => "ai-recovery-restart-flow-run",
        RecoveryActionKind::TrimContext => "ai-recovery-trim-context",
        RecoveryActionKind::CopyDetails => "ai-recovery-copy-details",
    }
}

fn label_for_action(kind: RecoveryActionKind) -> &'static str {
    match kind {
        RecoveryActionKind::Retry => "Try again",
        RecoveryActionKind::UseCurrentResults => "Use current results",
        RecoveryActionKind::ContinueInAgentChat => "Continue in Agent Chat",
        RecoveryActionKind::ChooseCompatibleModel => "Choose compatible model",
        RecoveryActionKind::ChooseProvider => "Choose provider",
        RecoveryActionKind::ChooseProfile => "Choose profile",
        RecoveryActionKind::UpdateClient => "Update client",
        RecoveryActionKind::CheckAgain => "Check again",
        RecoveryActionKind::SignIn => "Sign in",
        RecoveryActionKind::SwitchAccount => "Switch account",
        RecoveryActionKind::ConfigureProvider => "Configure provider",
        RecoveryActionKind::RepairComponent => "Repair component",
        RecoveryActionKind::Reattach => "Reattach session",
        RecoveryActionKind::RethreadFlow => "Start a new thread",
        RecoveryActionKind::RestartFlowRun => "Restart Flow",
        RecoveryActionKind::TrimContext => "Trim context",
        RecoveryActionKind::CopyDetails => "Copy details",
    }
}

fn progress_copy(action: &AiRecoveryAction) -> &'static str {
    match action {
        AiRecoveryAction::Retry => "Retrying with the same confirmed selection.",
        AiRecoveryAction::UseCurrentResults => "Preparing the current results.",
        AiRecoveryAction::ContinueInAgentChat => "Opening Agent Chat with the saved question.",
        AiRecoveryAction::ChooseCompatibleModel { .. } => "Applying the selected compatible model.",
        AiRecoveryAction::ChooseProvider { .. } => "Applying the selected provider.",
        AiRecoveryAction::ChooseProfile { .. } => "Applying the selected profile.",
        AiRecoveryAction::UpdateClient { .. } => "Opening the supported client update path.",
        AiRecoveryAction::CheckAgain => "Checking the AI connection again.",
        AiRecoveryAction::SignIn => "Opening provider sign-in.",
        AiRecoveryAction::SwitchAccount => "Opening account selection.",
        AiRecoveryAction::ConfigureProvider => "Opening provider configuration.",
        AiRecoveryAction::RepairComponent { .. } => "Opening the component repair path.",
        AiRecoveryAction::Reattach { .. } => "Reattaching the saved session.",
        AiRecoveryAction::RethreadFlow => "Starting a new Flow conversation thread.",
        AiRecoveryAction::RestartFlowRun => "Starting one new Flow run.",
        AiRecoveryAction::TrimContext => "Preparing a smaller conversation context.",
        AiRecoveryAction::CopyDetails => "Preparing safe diagnostic details.",
    }
}

fn recovered_copy(action: &AiRecoveryAction, selection: &AiSelectionState) -> &'static str {
    match action {
        AiRecoveryAction::ChooseCompatibleModel { .. }
        | AiRecoveryAction::ChooseProvider { .. }
        | AiRecoveryAction::ChooseProfile { .. }
            if selection_change_was_acknowledged(selection) =>
        {
            "The selected AI configuration is ready."
        }
        AiRecoveryAction::Reattach { .. } => "The existing session is connected again.",
        AiRecoveryAction::RethreadFlow => "A new Flow conversation thread is ready.",
        AiRecoveryAction::RestartFlowRun => "The new Flow run has started.",
        AiRecoveryAction::ContinueInAgentChat => "Agent Chat opened with the saved question.",
        AiRecoveryAction::SignIn | AiRecoveryAction::SwitchAccount => {
            "Provider authentication is ready."
        }
        AiRecoveryAction::UpdateClient { .. } | AiRecoveryAction::RepairComponent { .. } => {
            "The AI component is ready."
        }
        AiRecoveryAction::Retry
        | AiRecoveryAction::UseCurrentResults
        | AiRecoveryAction::ChooseCompatibleModel { .. }
        | AiRecoveryAction::ChooseProvider { .. }
        | AiRecoveryAction::ChooseProfile { .. }
        | AiRecoveryAction::CheckAgain
        | AiRecoveryAction::ConfigureProvider
        | AiRecoveryAction::TrimContext
        | AiRecoveryAction::CopyDetails => "Recovery completed successfully.",
    }
}

fn selection_change_was_acknowledged(selection: &AiSelectionState) -> bool {
    selection
        .acknowledged_change
        .as_ref()
        .is_some_and(|receipt| {
            selection.effective.as_ref() == Some(&receipt.applied)
                && matches!(
                    receipt.origin,
                    SelectionOrigin::ExplicitThisTurn | SelectionOrigin::RecoveryChoice
                )
        })
}
