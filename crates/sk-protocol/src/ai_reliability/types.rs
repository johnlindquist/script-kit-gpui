use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

string_id!(ProfileId);
string_id!(ProviderId);
string_id!(ModelId);
string_id!(FlowId);
string_id!(EngineId);
string_id!(RunId);
string_id!(PromptId);
string_id!(IntegrationId);
string_id!(ToolId);
string_id!(Fingerprint);
string_id!(WorkKey);
string_id!(TurnRequestRef);
string_id!(TurnRef);
string_id!(SessionRef);
string_id!(DiagnosticId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommandId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiSurfaceIdentity {
    QuickAi {
        profile_id: ProfileId,
        provider_id: ProviderId,
        model_id: ModelId,
    },
    AgentChat {
        profile_id: ProfileId,
        provider_id: Option<ProviderId>,
        model_id: Option<ModelId>,
        cwd_fingerprint: Fingerprint,
    },
    FlowConversation {
        flow_id: FlowId,
        definition_fingerprint: Fingerprint,
        engine_id: EngineId,
        provider_id: Option<ProviderId>,
        model_id: Option<ModelId>,
    },
    FlowRun {
        flow_id: FlowId,
        engine_id: EngineId,
        run_id: Option<RunId>,
    },
    LegacyChatPrompt {
        prompt_id: PromptId,
        provider_id: Option<ProviderId>,
        model_id: Option<ModelId>,
    },
    FocusedText {
        profile_id: ProfileId,
        provider_id: ProviderId,
        model_id: ModelId,
    },
    Other {
        integration_id: IntegrationId,
        provider_id: Option<ProviderId>,
        model_id: Option<ModelId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiModelSelection {
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ModelId>,
    pub profile_id: Option<ProfileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionOrigin {
    BuiltInDefault,
    PersistedUserChoice,
    ExplicitThisTurn,
    RecoveryChoice,
    RuntimeReported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionChangeReceipt {
    pub previous: Option<AiModelSelection>,
    pub applied: AiModelSelection,
    pub origin: SelectionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSelectionState {
    pub requested: Option<AiModelSelection>,
    pub effective: Option<AiModelSelection>,
    pub origin: SelectionOrigin,
    pub acknowledged_change: Option<SelectionChangeReceipt>,
}

impl AiSelectionState {
    pub fn can_start_turn(&self) -> bool {
        match (&self.requested, &self.effective) {
            (None, None) => true,
            (Some(requested), Some(effective)) if requested == effective => true,
            (Some(requested), Some(effective)) => {
                self.acknowledged_change.as_ref().is_some_and(|receipt| {
                    &receipt.applied == effective
                        && &receipt.applied == requested
                        && matches!(
                            receipt.origin,
                            SelectionOrigin::ExplicitThisTurn | SelectionOrigin::RecoveryChoice
                        )
                })
            }
            (None, Some(_)) | (Some(_), None) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreservationFailure {
    NeverCaptured,
    RuntimeDiscarded,
    CorruptSnapshot,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreservationReceipt {
    NotApplicable,
    Preserved { fingerprint: Fingerprint },
    Restorable { fingerprint: Fingerprint },
    Missing { reason: PreservationFailure },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiWorkSnapshot {
    pub key: WorkKey,
    pub transcript: PreservationReceipt,
    pub draft: PreservationReceipt,
    pub attachments: PreservationReceipt,
    pub partial_output: PreservationReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnRisk {
    ReadOnly,
    MayMutate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    pub partial_output_available: bool,
    pub mutating_effect_started: bool,
    pub externally_visible_effect_started: bool,
}

impl ProgressSnapshot {
    pub fn none() -> Self {
        Self {
            partial_output_available: false,
            mutating_effect_started: false,
            externally_visible_effect_started: false,
        }
    }

    pub fn permits_automatic_replay(&self, risk: TurnRisk) -> bool {
        matches!(risk, TurnRisk::ReadOnly)
            && !self.partial_output_available
            && !self.mutating_effect_started
            && !self.externally_visible_effect_started
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrySafety {
    SameSelectionReadOnly,
    ReconnectOnly,
    ExplicitUserConfirmation,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub automatic_max: u8,
    pub manual_max: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryLedger {
    pub automatic_used: u8,
    pub manual_used: u8,
    pub policy: RetryPolicy,
}

impl RetryLedger {
    pub fn automatic_available(self) -> bool {
        self.automatic_used < self.policy.automatic_max
    }

    pub fn manual_available(self) -> bool {
        self.manual_used < self.policy.manual_max
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryAttempt {
    Automatic(u8),
    Manual(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackoffClass {
    Immediate,
    Network,
    RateLimit { retry_after_ms: Option<u64> },
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticDescriptor {
    pub id: DiagnosticId,
    pub fingerprint: Fingerprint,
    pub availability: DiagnosticAvailability,
    pub visibility: DiagnosticVisibility,
    pub redaction: DiagnosticRedaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticAvailability {
    Available,
    FingerprintOnly,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticVisibility {
    SecondaryOnly,
    DeveloperOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticRedaction {
    AllowlistedFieldsV1,
    HashOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiFailure {
    pub code: AiFailureCode,
    pub kind: AiFailureKind,
    pub retry_safety: RetrySafety,
    pub diagnostic: Option<DiagnosticDescriptor>,
}

impl AiFailure {
    pub fn new(kind: AiFailureKind, retry_safety: RetrySafety) -> Self {
        let code = AiFailureCode::from_kind(&kind);
        Self {
            code,
            kind,
            retry_safety,
            diagnostic: None,
        }
    }

    pub fn with_diagnostic(mut self, diagnostic: DiagnosticDescriptor) -> Self {
        self.diagnostic = Some(diagnostic);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiFailureCode {
    ClientTooOld,
    ModelUnavailable,
    NoCompatibleModel,
    ProfileUnavailable,
    QuickAiSearchBudgetExceeded,
    QuickAiDeadlineExceeded,
    ToolDenied,
    AuthenticationMissing,
    AuthenticationExpired,
    UsageExhausted,
    ProviderNotConfigured,
    NoModelsAvailable,
    SidecarMissing,
    MdflowMissing,
    InvalidConfiguration,
    Offline,
    Timeout,
    RateLimited,
    ProviderTemporarilyUnavailable,
    ProviderServerRejected,
    SpawnFailed,
    RuntimeClosed,
    ChildExited,
    SessionLost,
    ProtocolVersionMismatch,
    ProtocolSequenceViolation,
    ProtocolOrderViolation,
    ProtocolMalformedResponse,
    ProtocolMissingTerminal,
    PermissionDenied,
    UserDeniedTool,
    MessageTooLarge,
    ContextLimitExceeded,
    ContextUnavailable,
    Unknown,
}

impl AiFailureCode {
    pub fn from_kind(kind: &AiFailureKind) -> Self {
        match kind {
            AiFailureKind::Capability(failure) => match failure {
                CapabilityFailure::ClientTooOld { .. } => Self::ClientTooOld,
                CapabilityFailure::ModelUnavailable { .. } => Self::ModelUnavailable,
                CapabilityFailure::NoCompatibleModel => Self::NoCompatibleModel,
                CapabilityFailure::ProfileUnavailable { .. } => Self::ProfileUnavailable,
            },
            AiFailureKind::Policy(failure) => match failure {
                PolicyFailure::QuickAiSearchBudgetExceeded { .. } => {
                    Self::QuickAiSearchBudgetExceeded
                }
                PolicyFailure::QuickAiDeadlineExceeded { .. } => Self::QuickAiDeadlineExceeded,
                PolicyFailure::ToolDenied { .. } => Self::ToolDenied,
            },
            AiFailureKind::Authentication(failure) => match failure {
                AuthenticationFailure::Missing => Self::AuthenticationMissing,
                AuthenticationFailure::Expired => Self::AuthenticationExpired,
                AuthenticationFailure::UsageExhausted => Self::UsageExhausted,
            },
            AiFailureKind::Configuration(failure) => match failure {
                ConfigurationFailure::ProviderNotConfigured => Self::ProviderNotConfigured,
                ConfigurationFailure::NoModelsAvailable => Self::NoModelsAvailable,
                ConfigurationFailure::SidecarMissing => Self::SidecarMissing,
                ConfigurationFailure::MdflowMissing => Self::MdflowMissing,
                ConfigurationFailure::InvalidConfiguration => Self::InvalidConfiguration,
            },
            AiFailureKind::Connectivity(failure) => match failure {
                ConnectivityFailure::Offline => Self::Offline,
                ConnectivityFailure::Timeout => Self::Timeout,
                ConnectivityFailure::RateLimited { .. } => Self::RateLimited,
            },
            AiFailureKind::Provider(failure) => match failure {
                ProviderFailure::TemporarilyUnavailable => Self::ProviderTemporarilyUnavailable,
                ProviderFailure::ServerRejected => Self::ProviderServerRejected,
            },
            AiFailureKind::Runtime(failure) => match failure {
                RuntimeFailure::SpawnFailed => Self::SpawnFailed,
                RuntimeFailure::RuntimeClosed => Self::RuntimeClosed,
                RuntimeFailure::ChildExited { .. } => Self::ChildExited,
                RuntimeFailure::SessionLost { .. } => Self::SessionLost,
            },
            AiFailureKind::Protocol(failure) => match failure {
                ProtocolFailure::VersionMismatch { .. } => Self::ProtocolVersionMismatch,
                ProtocolFailure::SequenceViolation { .. } => Self::ProtocolSequenceViolation,
                ProtocolFailure::OrderViolation { .. } => Self::ProtocolOrderViolation,
                ProtocolFailure::MalformedResponse { .. } => Self::ProtocolMalformedResponse,
                ProtocolFailure::MissingTerminal { .. } => Self::ProtocolMissingTerminal,
            },
            AiFailureKind::Permission(failure) => match failure {
                PermissionFailure::PermissionDenied => Self::PermissionDenied,
                PermissionFailure::UserDeniedTool => Self::UserDeniedTool,
            },
            AiFailureKind::Input(failure) => match failure {
                InputFailure::MessageTooLarge => Self::MessageTooLarge,
                InputFailure::ContextLimitExceeded => Self::ContextLimitExceeded,
                InputFailure::ContextUnavailable => Self::ContextUnavailable,
            },
            AiFailureKind::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiFailureKind {
    Capability(CapabilityFailure),
    Policy(PolicyFailure),
    Authentication(AuthenticationFailure),
    Configuration(ConfigurationFailure),
    Connectivity(ConnectivityFailure),
    Provider(ProviderFailure),
    Runtime(RuntimeFailure),
    Protocol(ProtocolFailure),
    Permission(PermissionFailure),
    Input(InputFailure),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityFailure {
    ClientTooOld {
        client: ClientKind,
        model: Option<ModelId>,
    },
    ModelUnavailable {
        model: ModelId,
        reason: ModelAvailabilityReason,
    },
    NoCompatibleModel,
    ProfileUnavailable {
        profile: ProfileId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientKind {
    Codex,
    Pi,
    Mdflow,
    LocalLlm,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelAvailabilityReason {
    Removed,
    UnsupportedByClient,
    UnsupportedByProvider,
    NotAdvertised,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyFailure {
    QuickAiSearchBudgetExceeded {
        completed_searches: u8,
        budget: u8,
        partial_answer_available: bool,
        source_count: u16,
    },
    QuickAiDeadlineExceeded {
        deadline_ms: u32,
        completed_searches: u8,
        partial_answer_available: bool,
        source_count: u16,
    },
    ToolDenied {
        tool: Option<ToolId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthenticationFailure {
    Missing,
    Expired,
    UsageExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationFailure {
    ProviderNotConfigured,
    NoModelsAvailable,
    SidecarMissing,
    MdflowMissing,
    InvalidConfiguration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectivityFailure {
    Offline,
    Timeout,
    RateLimited { retry_after_ms: Option<u64> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderFailure {
    TemporarilyUnavailable,
    ServerRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeFailure {
    SpawnFailed,
    RuntimeClosed,
    ChildExited {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    SessionLost {
        reattach: ReattachAvailability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReattachAvailability {
    Available { session: SessionRef },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolFailure {
    VersionMismatch {
        component: ProtocolComponent,
        expected: String,
        actual: Option<String>,
    },
    SequenceViolation {
        component: ProtocolComponent,
    },
    OrderViolation {
        component: ProtocolComponent,
    },
    MalformedResponse {
        component: ProtocolComponent,
    },
    MissingTerminal {
        component: ProtocolComponent,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolComponent {
    Codex,
    Pi,
    Mdflow,
    Provider,
    LocalLlm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionFailure {
    PermissionDenied,
    UserDeniedTool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputFailure {
    MessageTooLarge,
    ContextLimitExceeded,
    ContextUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryRole {
    Primary,
    Secondary,
    Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryActionKind {
    Retry,
    UseCurrentResults,
    ContinueInAgentChat,
    ChooseCompatibleModel,
    ChooseProvider,
    ChooseProfile,
    UpdateClient,
    CheckAgain,
    SignIn,
    SwitchAccount,
    ConfigureProvider,
    RepairComponent,
    Reattach,
    RethreadFlow,
    RestartFlowRun,
    TrimContext,
    CopyDetails,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisabledReason {
    RetryBudgetExhausted,
    UnsafeToReplay,
    NoCompatibleSelection,
    MissingSession,
    UnsupportedBySurface,
    WaitingForBackoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryOption {
    pub kind: RecoveryActionKind,
    pub role: RecoveryRole,
    pub enabled: bool,
    pub disabled_reason: Option<DisabledReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPlan {
    pub options: Vec<RecoveryOption>,
}

impl RecoveryPlan {
    pub fn option(&self, kind: RecoveryActionKind) -> Option<&RecoveryOption> {
        self.options.iter().find(|option| option.kind == kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiRecoveryAction {
    Retry,
    UseCurrentResults,
    ContinueInAgentChat,
    ChooseCompatibleModel { selection: Option<AiModelSelection> },
    ChooseProvider { selection: Option<AiModelSelection> },
    ChooseProfile { selection: Option<AiModelSelection> },
    UpdateClient { client: ClientKind },
    CheckAgain,
    SignIn,
    SwitchAccount,
    ConfigureProvider,
    RepairComponent { component: ProtocolComponent },
    Reattach { session: SessionRef },
    RethreadFlow,
    RestartFlowRun,
    TrimContext,
    CopyDetails,
}

impl AiRecoveryAction {
    pub fn kind(&self) -> RecoveryActionKind {
        match self {
            Self::Retry => RecoveryActionKind::Retry,
            Self::UseCurrentResults => RecoveryActionKind::UseCurrentResults,
            Self::ContinueInAgentChat => RecoveryActionKind::ContinueInAgentChat,
            Self::ChooseCompatibleModel { .. } => RecoveryActionKind::ChooseCompatibleModel,
            Self::ChooseProvider { .. } => RecoveryActionKind::ChooseProvider,
            Self::ChooseProfile { .. } => RecoveryActionKind::ChooseProfile,
            Self::UpdateClient { .. } => RecoveryActionKind::UpdateClient,
            Self::CheckAgain => RecoveryActionKind::CheckAgain,
            Self::SignIn => RecoveryActionKind::SignIn,
            Self::SwitchAccount => RecoveryActionKind::SwitchAccount,
            Self::ConfigureProvider => RecoveryActionKind::ConfigureProvider,
            Self::RepairComponent { .. } => RecoveryActionKind::RepairComponent,
            Self::Reattach { .. } => RecoveryActionKind::Reattach,
            Self::RethreadFlow => RecoveryActionKind::RethreadFlow,
            Self::RestartFlowRun => RecoveryActionKind::RestartFlowRun,
            Self::TrimContext => RecoveryActionKind::TrimContext,
            Self::CopyDetails => RecoveryActionKind::CopyDetails,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingTurn {
    pub request: TurnRequestRef,
    pub risk: TurnRisk,
    pub start_command_id: Option<CommandId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiOperationState {
    pub identity: AiSurfaceIdentity,
    pub phase: AiPhase,
    pub selection: AiSelectionState,
    pub work: AiWorkSnapshot,
    pub retry: RetryLedger,
    pub diagnostic: Option<DiagnosticDescriptor>,
    pub pending: Option<PendingTurn>,
    pub next_command_seq: u64,
}

impl AiOperationState {
    pub fn ready(
        identity: AiSurfaceIdentity,
        selection: AiSelectionState,
        work: AiWorkSnapshot,
        retry: RetryPolicy,
    ) -> Self {
        Self {
            identity,
            phase: AiPhase::Ready,
            selection,
            work,
            retry: RetryLedger {
                automatic_used: 0,
                manual_used: 0,
                policy: retry,
            },
            diagnostic: None,
            pending: None,
            next_command_seq: 1,
        }
    }

    pub(crate) fn take_command_id(&mut self) -> CommandId {
        let command_id = CommandId(self.next_command_seq);
        self.next_command_seq = self.next_command_seq.saturating_add(1);
        command_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPhase {
    Ready,
    Preflighting {
        request: TurnRequestRef,
    },
    Running {
        turn: TurnRef,
        risk: TurnRisk,
        progress: ProgressSnapshot,
    },
    Cancelling {
        turn: TurnRef,
        partial: PartialOutputState,
        /// Serialized cancellation meaning. Older persisted states did not
        /// carry this field, so they retain the legacy UserCancelled default.
        #[serde(default = "default_user_cancelled")]
        kind: CancellationKind,
    },
    AwaitingRecovery {
        failure: AiFailure,
        plan: RecoveryPlan,
    },
    Recovering {
        action: AiRecoveryAction,
        command_id: CommandId,
        origin: RecoveryOrigin,
    },
    Recovered {
        action: AiRecoveryAction,
        summary: RecoverySuccess,
    },
    Succeeded {
        completeness: CompletionKind,
        recovered_from: Option<AiFailureCode>,
    },
    Cancelled {
        kind: CancellationKind,
        partial: PartialOutputState,
    },
    Dismissed {
        failure: AiFailureCode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiPhaseTag {
    Ready,
    Preflighting,
    Running,
    Cancelling,
    AwaitingRecovery,
    Recovering,
    Recovered,
    Succeeded,
    Cancelled,
    Dismissed,
}

impl AiPhase {
    pub fn tag(&self) -> AiPhaseTag {
        match self {
            Self::Ready => AiPhaseTag::Ready,
            Self::Preflighting { .. } => AiPhaseTag::Preflighting,
            Self::Running { .. } => AiPhaseTag::Running,
            Self::Cancelling { .. } => AiPhaseTag::Cancelling,
            Self::AwaitingRecovery { .. } => AiPhaseTag::AwaitingRecovery,
            Self::Recovering { .. } => AiPhaseTag::Recovering,
            Self::Recovered { .. } => AiPhaseTag::Recovered,
            Self::Succeeded { .. } => AiPhaseTag::Succeeded,
            Self::Cancelled { .. } => AiPhaseTag::Cancelled,
            Self::Dismissed { .. } => AiPhaseTag::Dismissed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityDecision {
    Compatible,
    Unknown,
    Blocked(AiFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiOperationEvent {
    SubmitRequested {
        request: TurnRequestRef,
        work: AiWorkSnapshot,
        selection: AiSelectionState,
        risk: TurnRisk,
    },
    CapabilityResolved(CapabilityDecision),
    RuntimeStarted {
        command_id: CommandId,
        turn: TurnRef,
    },
    Progressed {
        progress: ProgressSnapshot,
        work: AiWorkSnapshot,
    },
    Completed(CompletionKind),
    Failed(AiFailure),
    CancelRequested,
    RuntimeCancelled {
        partial: PartialOutputState,
    },
    /// An explicit user Stop. Dismissal and legacy cancellation retain their
    /// existing events and cannot be mistaken for this intent.
    StopRequested,
    RuntimeStopped {
        partial: PartialOutputState,
    },
    RecoverySelected(AiRecoveryAction),
    RecoveryCommandSucceeded {
        command_id: CommandId,
        result: RecoveryEffectResult,
    },
    RecoveryCommandFailed {
        command_id: CommandId,
        failure: AiFailure,
    },
    BackoffElapsed {
        command_id: CommandId,
    },
    RestartObserved(RestartSnapshot),
    SessionReattached(ReattachReceipt),
    SessionReattachFailed(AiFailure),
    SessionReplaced {
        identity: AiSurfaceIdentity,
        selection: AiSelectionState,
        work: AiWorkSnapshot,
    },
    DismissRequested,
    ResetForNextTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiEventTag {
    SubmitRequested,
    CapabilityResolved,
    RuntimeStarted,
    Progressed,
    Completed,
    Failed,
    CancelRequested,
    RuntimeCancelled,
    StopRequested,
    RuntimeStopped,
    RecoverySelected,
    RecoveryCommandSucceeded,
    RecoveryCommandFailed,
    BackoffElapsed,
    RestartObserved,
    SessionReattached,
    SessionReattachFailed,
    SessionReplaced,
    DismissRequested,
    ResetForNextTurn,
}

impl AiOperationEvent {
    pub fn tag(&self) -> AiEventTag {
        match self {
            Self::SubmitRequested { .. } => AiEventTag::SubmitRequested,
            Self::CapabilityResolved(_) => AiEventTag::CapabilityResolved,
            Self::RuntimeStarted { .. } => AiEventTag::RuntimeStarted,
            Self::Progressed { .. } => AiEventTag::Progressed,
            Self::Completed(_) => AiEventTag::Completed,
            Self::Failed(_) => AiEventTag::Failed,
            Self::CancelRequested => AiEventTag::CancelRequested,
            Self::RuntimeCancelled { .. } => AiEventTag::RuntimeCancelled,
            Self::StopRequested => AiEventTag::StopRequested,
            Self::RuntimeStopped { .. } => AiEventTag::RuntimeStopped,
            Self::RecoverySelected(_) => AiEventTag::RecoverySelected,
            Self::RecoveryCommandSucceeded { .. } => AiEventTag::RecoveryCommandSucceeded,
            Self::RecoveryCommandFailed { .. } => AiEventTag::RecoveryCommandFailed,
            Self::BackoffElapsed { .. } => AiEventTag::BackoffElapsed,
            Self::RestartObserved(_) => AiEventTag::RestartObserved,
            Self::SessionReattached(_) => AiEventTag::SessionReattached,
            Self::SessionReattachFailed(_) => AiEventTag::SessionReattachFailed,
            Self::SessionReplaced { .. } => AiEventTag::SessionReplaced,
            Self::DismissRequested => AiEventTag::DismissRequested,
            Self::ResetForNextTurn => AiEventTag::ResetForNextTurn,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiTransition {
    pub next: AiOperationState,
    pub commands: Vec<AiCommand>,
    pub outcome: Option<AiOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidTransition {
    pub phase: AiPhaseTag,
    pub event: AiEventTag,
    pub reason: InvalidTransitionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvalidTransitionReason {
    EventNotAllowed,
    CommandIdMismatch,
    RecoveryActionUnavailable,
    RecoveryActionDisabled,
    RetryBudgetExhausted,
    UnsafeReplay,
    UnacknowledgedSelection,
    MissingPendingTurn,
    MissingDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiCommand {
    PersistWork(AiWorkSnapshot),
    CheckCapabilities(CapabilityRequest),
    StartTurn(StartTurnCommand),
    CancelTurn {
        turn: TurnRef,
    },
    ScheduleBackoff {
        command_id: CommandId,
        attempt: RetryAttempt,
        class: BackoffClass,
    },
    ApplySelection(ExplicitSelectionChange),
    LaunchAuthentication(AuthRecoveryCommand),
    OpenConfiguration(ConfigurationTarget),
    OpenClientUpdate(ClientUpdateTarget),
    RecheckClientCapability(ClientCapabilityKey),
    ReattachSession(SessionRef),
    RethreadFlow(RethreadFlowCommand),
    RestartFlowRun(RestartFlowRunCommand),
    ContinueInAgentChat(AgentChatEscalation),
    InstallOrRepairComponent(ComponentRecoveryCommand),
    CopyRedactedDiagnostics(DiagnosticId),
    ClearPendingWork(WorkKey),
    ScheduleRecoveredDismiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub command_id: CommandId,
    pub identity: AiSurfaceIdentity,
    pub selection: AiSelectionState,
    pub request: TurnRequestRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartTurnCommand {
    pub command_id: CommandId,
    pub request: TurnRequestRef,
    pub selection: AiSelectionState,
    pub work_key: WorkKey,
    pub risk: TurnRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplicitSelectionChange {
    pub command_id: CommandId,
    pub requested: AiModelSelection,
    pub origin: SelectionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthRecoveryCommand {
    pub command_id: CommandId,
    pub mode: AuthRecoveryMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthRecoveryMode {
    SignIn,
    SwitchAccount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationTarget {
    pub command_id: CommandId,
    pub kind: ConfigurationTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationTargetKind {
    Provider,
    Model,
    Profile,
    Mdflow,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientUpdateTarget {
    pub command_id: CommandId,
    pub client: ClientKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCapabilityKey {
    pub command_id: CommandId,
    pub identity: AiSurfaceIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RethreadFlowCommand {
    pub command_id: CommandId,
    pub flow_id: FlowId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestartFlowRunCommand {
    pub command_id: CommandId,
    pub flow_id: FlowId,
    pub previous_run_id: Option<RunId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentChatEscalation {
    pub command_id: CommandId,
    pub work: AiWorkSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRecoveryCommand {
    pub command_id: CommandId,
    pub component: ProtocolComponent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryEffectResult {
    SelectionApplied(SelectionChangeReceipt),
    AuthenticationReady,
    ConfigurationChanged,
    ClientUpdated,
    CapabilityRechecked,
    ComponentReady,
    FlowRethreaded,
    FlowRunRestarted {
        run_id: RunId,
    },
    AgentChatOpened,
    ContextTrimmed,
    /// An external recovery surface was opened, but the app has not yet
    /// observed that the underlying capability is healthy.
    ExternalActionLaunched,
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryOrigin {
    Failure {
        failure: AiFailure,
        plan: RecoveryPlan,
    },
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestartSnapshot {
    pub session: Option<SessionRef>,
    pub partial: PartialOutputState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReattachReceipt {
    pub session: SessionRef,
    pub turn: TurnRef,
    pub risk: TurnRisk,
    pub progress: ProgressSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompletionKind {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationKind {
    UserCancelled,
    UserStopped,
    AppShutdown,
}

fn default_user_cancelled() -> CancellationKind {
    CancellationKind::UserCancelled
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartialOutputState {
    None,
    Preserved { fingerprint: Fingerprint },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoverySuccess {
    ReadyToRetry,
    SelectionApplied,
    AuthenticationReady,
    ConfigurationChanged,
    ClientUpdated,
    ComponentReady,
    SessionReattached,
    FlowRethreaded,
    FlowRunRestarted,
    AgentChatOpened,
    ContextTrimmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiOutcome {
    Succeeded {
        completeness: CompletionKind,
    },
    Cancelled {
        kind: CancellationKind,
        partial: PartialOutputState,
    },
    RecoverySucceeded {
        action: RecoveryActionKind,
    },
    RecoveryFailed {
        failure: AiFailureCode,
    },
    Dismissed {
        failure: AiFailureCode,
    },
}
