//! Conversational flow sessions: metadata + turn-prompt building.
//!
//! A session is an agent conversation rendered with Script Kit's own chat
//! surface (`ChatPrompt`, the Threadline). No engine TUI is ever wrapped.
//! Two transports:
//!
//! - [`SessionTransport::CodexThread`] (flagship): codex-engine flows talk
//!   to a persistent `codex app-server` over JSON-RPC
//!   (`crate::flows::codex_client`). The first turn sends the flow's
//!   resolved mission (`resolve_flow_mission`); the protocol thread holds
//!   context, so later turns send the raw message.
//! - [`SessionTransport::MdflowTurns`] (second-class, non-codex engines):
//!   each user message launches one `md <flow> <prompt> --events` run (or
//!   `--_task <prompt>` only when an engine-free `md explain` proves that
//!   named contract). Streamed stdout fills the assistant bubble. mdflow runs
//!   are stateless, so context rides inside the turn prompt as a rolled-up
//!   transcript (`build_turn_task`).
//!
//! Contract (Conversation Desk):
//! - Enter on a flow = start (or resume) a conversation.
//! - Backgrounding NEVER kills a running turn; re-entering an Active row
//!   restores the SAME transcript entity.
//! - Stop cancels the in-flight turn only; the conversation survives.

use std::time::Instant;

use super::explain_cache::{run_md_explain_with_deadline, MdExplainOutput, MD_EXPLAIN_DEADLINE};

/// Coarse session state, following Orca's attention model. Working while a
/// turn's events run is in flight; NeedsYou when the agent has replied and
/// the composer waits on the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// A turn is in flight (events run active).
    Working,
    /// The conversation is idle — the agent answered and awaits the user.
    NeedsYou,
    /// The last turn ended with this exit code (None = signal/unknown) and
    /// the user has not sent a new message since a failure worth surfacing.
    Done(Option<i32>),
}

impl SessionState {
    pub fn label(self) -> &'static str {
        match self {
            SessionState::Working => "working",
            SessionState::NeedsYou => "needs you",
            SessionState::Done(Some(0)) => "done",
            SessionState::Done(_) => "failed",
        }
    }

    pub fn is_live(self) -> bool {
        !matches!(self, SessionState::Done(_))
    }
}

/// One committed conversation turn, kept engine-agnostic for prompt rollup.
///
/// `assistant` is RAW engine output only. UI decoration (the `*Stopped.*`
/// caption) is derived from `outcome` at display time — never stored here —
/// so rollup and persistence stay semantically clean (Oracle 2026-07-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTurn {
    pub user: String,
    pub assistant: String,
    pub outcome: PersistedTurnOutcome,
    /// Typed failure metadata for `Failed` turns (S09). `Ok`/`Stopped`
    /// turns never carry one. The safe summary is the ONLY failure text
    /// that may reach the transcript or persistence — raw provider/stderr
    /// payloads stop at the diagnostic vault.
    pub failure: Option<PersistedAiFailure>,
}

/// Stable persisted projection of one typed AI failure.
///
/// Slim by design: enough to render truthful recovery copy and route
/// recovery actions after an app restart, without persisting raw provider
/// payloads, stderr, or diagnostics (those stop at the redacting vault).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistedAiFailure {
    pub code: sk_protocol::ai_reliability::AiFailureCode,
    pub category: PersistedAiFailureCategory,
    pub safe_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_fingerprint: Option<String>,
}

/// Coarse failure family, persisted alongside the exact code so snapshots
/// stay meaningful even if a future reader no longer knows the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PersistedAiFailureCategory {
    Capability,
    Policy,
    Authentication,
    Configuration,
    Connectivity,
    Provider,
    Runtime,
    Protocol,
    Permission,
    Input,
    Unknown,
}

impl PersistedAiFailure {
    pub fn from_failure(
        failure: &sk_protocol::ai_reliability::AiFailure,
        safe_summary: &str,
    ) -> Self {
        use sk_protocol::ai_reliability::AiFailureKind;
        let category = match &failure.kind {
            AiFailureKind::Capability(_) => PersistedAiFailureCategory::Capability,
            AiFailureKind::Policy(_) => PersistedAiFailureCategory::Policy,
            AiFailureKind::Authentication(_) => PersistedAiFailureCategory::Authentication,
            AiFailureKind::Configuration(_) => PersistedAiFailureCategory::Configuration,
            AiFailureKind::Connectivity(_) => PersistedAiFailureCategory::Connectivity,
            AiFailureKind::Provider(_) => PersistedAiFailureCategory::Provider,
            AiFailureKind::Runtime(_) => PersistedAiFailureCategory::Runtime,
            AiFailureKind::Protocol(_) => PersistedAiFailureCategory::Protocol,
            AiFailureKind::Permission(_) => PersistedAiFailureCategory::Permission,
            AiFailureKind::Input(_) => PersistedAiFailureCategory::Input,
            AiFailureKind::Unknown => PersistedAiFailureCategory::Unknown,
        };
        Self {
            code: failure.code,
            category,
            safe_summary: bounded_safe_summary(safe_summary),
            diagnostic_fingerprint: failure
                .diagnostic
                .as_ref()
                .map(|diagnostic| diagnostic.fingerprint.0.clone()),
        }
    }

    /// Classify one legacy (v0–v2 snapshot) failure caption while loading.
    /// Legacy captions were already user-visible safe copy, so the text is
    /// kept as the summary; the code is recovered from the small closed set
    /// of strings those versions could write, defaulting to `Unknown`.
    pub fn from_legacy_error(error: &str) -> Self {
        use sk_protocol::ai_reliability::AiFailureCode;
        let trimmed = error.trim();
        let (code, category) = if trimmed.contains("mdflow CLI not found") {
            (
                AiFailureCode::MdflowMissing,
                PersistedAiFailureCategory::Configuration,
            )
        } else if trimmed.contains("protocol violation") {
            (
                AiFailureCode::ProtocolMalformedResponse,
                PersistedAiFailureCategory::Protocol,
            )
        } else if trimmed.contains("failed to spawn") {
            (
                AiFailureCode::SpawnFailed,
                PersistedAiFailureCategory::Runtime,
            )
        } else if trimmed.contains("Flow definition unreadable") {
            (
                AiFailureCode::InvalidConfiguration,
                PersistedAiFailureCategory::Configuration,
            )
        } else if trimmed.contains("exited") || trimmed.contains("wait failed") {
            (
                AiFailureCode::ChildExited,
                PersistedAiFailureCategory::Runtime,
            )
        } else {
            (AiFailureCode::Unknown, PersistedAiFailureCategory::Unknown)
        };
        Self {
            code,
            category,
            safe_summary: bounded_safe_summary(if trimmed.is_empty() {
                FLOW_TURN_FAILED_SUMMARY
            } else {
                trimmed
            }),
            diagnostic_fingerprint: None,
        }
    }

    pub fn unknown_default() -> Self {
        Self {
            code: sk_protocol::ai_reliability::AiFailureCode::Unknown,
            category: PersistedAiFailureCategory::Unknown,
            safe_summary: FLOW_TURN_FAILED_SUMMARY.to_string(),
            diagnostic_fingerprint: None,
        }
    }

    pub fn from_record(record: &crate::ai::reliability::AppFailureRecord) -> Self {
        Self::from_failure(&record.failure, record.primary_message())
    }

    /// Reconstruct a best-effort typed failure from the persisted slim
    /// projection (restore path). Detail payloads are gone by design; the
    /// code maps back to its canonical kind so recovery planning and copy
    /// stay code-accurate after an app restart.
    pub fn to_failure(&self) -> sk_protocol::ai_reliability::AiFailure {
        use sk_protocol::ai_reliability::{
            AiFailure, AiFailureCode, AiFailureKind, AuthenticationFailure, CapabilityFailure,
            ClientKind, ConfigurationFailure, ConnectivityFailure, InputFailure,
            ModelAvailabilityReason, ModelId, PermissionFailure, PolicyFailure, ProtocolComponent,
            ProtocolFailure, ProviderFailure, ReattachAvailability, RetrySafety, RuntimeFailure,
        };
        let kind = match self.code {
            AiFailureCode::ClientTooOld => {
                AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
                    client: ClientKind::Other,
                    model: None,
                })
            }
            AiFailureCode::ModelUnavailable => {
                AiFailureKind::Capability(CapabilityFailure::ModelUnavailable {
                    model: ModelId("unknown".to_string()),
                    reason: ModelAvailabilityReason::NotAdvertised,
                })
            }
            AiFailureCode::NoCompatibleModel => {
                AiFailureKind::Capability(CapabilityFailure::NoCompatibleModel)
            }
            AiFailureCode::ProfileUnavailable => {
                AiFailureKind::Capability(CapabilityFailure::ProfileUnavailable {
                    profile: sk_protocol::ai_reliability::ProfileId("unknown".to_string()),
                })
            }
            AiFailureCode::QuickAiSearchBudgetExceeded => {
                AiFailureKind::Policy(PolicyFailure::QuickAiSearchBudgetExceeded {
                    completed_searches: 0,
                    budget: 0,
                    partial_answer_available: false,
                    source_count: 0,
                })
            }
            AiFailureCode::QuickAiDeadlineExceeded => {
                AiFailureKind::Policy(PolicyFailure::QuickAiDeadlineExceeded {
                    deadline_ms: 0,
                    completed_searches: 0,
                    partial_answer_available: false,
                    source_count: 0,
                })
            }
            AiFailureCode::ToolDenied => {
                AiFailureKind::Policy(PolicyFailure::ToolDenied { tool: None })
            }
            AiFailureCode::AuthenticationMissing => {
                AiFailureKind::Authentication(AuthenticationFailure::Missing)
            }
            AiFailureCode::AuthenticationExpired => {
                AiFailureKind::Authentication(AuthenticationFailure::Expired)
            }
            AiFailureCode::UsageExhausted => {
                AiFailureKind::Authentication(AuthenticationFailure::UsageExhausted)
            }
            AiFailureCode::ProviderNotConfigured => {
                AiFailureKind::Configuration(ConfigurationFailure::ProviderNotConfigured)
            }
            AiFailureCode::NoModelsAvailable => {
                AiFailureKind::Configuration(ConfigurationFailure::NoModelsAvailable)
            }
            AiFailureCode::SidecarMissing => {
                AiFailureKind::Configuration(ConfigurationFailure::SidecarMissing)
            }
            AiFailureCode::MdflowMissing => {
                AiFailureKind::Configuration(ConfigurationFailure::MdflowMissing)
            }
            AiFailureCode::InvalidConfiguration => {
                AiFailureKind::Configuration(ConfigurationFailure::InvalidConfiguration)
            }
            AiFailureCode::Offline => AiFailureKind::Connectivity(ConnectivityFailure::Offline),
            AiFailureCode::Timeout => AiFailureKind::Connectivity(ConnectivityFailure::Timeout),
            AiFailureCode::RateLimited => {
                AiFailureKind::Connectivity(ConnectivityFailure::RateLimited {
                    retry_after_ms: None,
                })
            }
            AiFailureCode::ProviderTemporarilyUnavailable => {
                AiFailureKind::Provider(ProviderFailure::TemporarilyUnavailable)
            }
            AiFailureCode::ProviderServerRejected => {
                AiFailureKind::Provider(ProviderFailure::ServerRejected)
            }
            AiFailureCode::SpawnFailed => AiFailureKind::Runtime(RuntimeFailure::SpawnFailed),
            AiFailureCode::RuntimeClosed => AiFailureKind::Runtime(RuntimeFailure::RuntimeClosed),
            AiFailureCode::ChildExited => AiFailureKind::Runtime(RuntimeFailure::ChildExited {
                exit_code: None,
                signal: None,
            }),
            AiFailureCode::SessionLost => AiFailureKind::Runtime(RuntimeFailure::SessionLost {
                reattach: ReattachAvailability::Unavailable,
            }),
            AiFailureCode::ProtocolVersionMismatch => {
                AiFailureKind::Protocol(ProtocolFailure::VersionMismatch {
                    component: ProtocolComponent::Mdflow,
                    expected: "supported".to_string(),
                    actual: None,
                })
            }
            AiFailureCode::ProtocolSequenceViolation => {
                AiFailureKind::Protocol(ProtocolFailure::SequenceViolation {
                    component: ProtocolComponent::Mdflow,
                })
            }
            AiFailureCode::ProtocolOrderViolation => {
                AiFailureKind::Protocol(ProtocolFailure::OrderViolation {
                    component: ProtocolComponent::Mdflow,
                })
            }
            AiFailureCode::ProtocolMalformedResponse => {
                AiFailureKind::Protocol(ProtocolFailure::MalformedResponse {
                    component: ProtocolComponent::Mdflow,
                })
            }
            AiFailureCode::ProtocolMissingTerminal => {
                AiFailureKind::Protocol(ProtocolFailure::MissingTerminal {
                    component: ProtocolComponent::Mdflow,
                })
            }
            AiFailureCode::PermissionDenied => {
                AiFailureKind::Permission(PermissionFailure::PermissionDenied)
            }
            AiFailureCode::UserDeniedTool => {
                AiFailureKind::Permission(PermissionFailure::UserDeniedTool)
            }
            AiFailureCode::MessageTooLarge => AiFailureKind::Input(InputFailure::MessageTooLarge),
            AiFailureCode::ContextLimitExceeded => {
                AiFailureKind::Input(InputFailure::ContextLimitExceeded)
            }
            AiFailureCode::Unknown => AiFailureKind::Unknown,
        };
        AiFailure::new(kind, RetrySafety::SameSelectionReadOnly)
    }
}

/// Fallback safe summary when a failed turn has no usable caption.
pub const FLOW_TURN_FAILED_SUMMARY: &str = "Flow turn failed";

/// Char-boundary-safe cap on persisted failure summaries: a summary is
/// display copy, never a payload dump.
fn bounded_safe_summary(summary: &str) -> String {
    const MAX_CHARS: usize = 300;
    if summary.chars().count() <= MAX_CHARS {
        return summary.to_string();
    }
    let truncated: String = summary.chars().take(MAX_CHARS - 1).collect();
    format!("{truncated}…")
}

/// The display caption for a user-stopped turn. Owned by the domain layer so
/// snapshot migration and UI projection share one definition.
pub const FLOW_STOPPED_CAPTION: &str = "*Stopped.*";

/// How a session's turns reach an engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransport {
    /// Native `codex app-server` thread (codex-engine flows).
    CodexThread,
    /// One engine-free-resolved `md <flow> <turn-input> --events` registry run.
    MdflowTurns,
}

impl SessionTransport {
    pub fn for_engine(engine: &str) -> Self {
        if engine.eq_ignore_ascii_case("codex") {
            SessionTransport::CodexThread
        } else {
            SessionTransport::MdflowTurns
        }
    }
}

/// Requests posted from `ChatPrompt` callbacks (which have no app access)
/// and drained in the app render pass (window access for actions).
/// Since the WP7 host-mode refactor made the Flow session view the only
/// key/lifecycle owner, ChatPrompt callbacks can no longer originate Escape —
/// the `Background` variant lost its last sender and was removed (Oracle
/// phase-c audit follow-up). `Submit`/`ShowActions` remain only because the
/// hosted ChatPrompt constructor still requires the callbacks; they are dead
/// ingresses pending a callback-free transcript-only constructor.
#[derive(Debug, Clone)]
pub enum FlowChatRequest {
    Submit {
        session_id: u64,
        text: String,
    },
    ShowActions {
        session_id: u64,
    },
    /// S10: a recovery-card action chosen on a failed TURN inside the hosted
    /// transcript. The prompt cannot perform these itself — rethreading,
    /// repairing mdflow and retrying all belong to the session owner — so it
    /// posts here and the render pass, which has the window, applies them.
    /// Without this the card's non-retry buttons never even render, because
    /// their availability is derived from the callback's presence.
    Recovery {
        session_id: u64,
        message_id: String,
        action: sk_protocol::ai_reliability::AiRecoveryAction,
    },
}

/// Metadata for one conversation, independent of the GPUI entity.
#[derive(Debug, Clone)]
pub struct FlowSessionMeta {
    pub id: u64,
    pub flow_id: String,
    pub flow_name: String,
    pub friendly_name: String,
    pub origin: String,
    pub engine: String,
    /// Definition path (the flow's markdown file).
    pub flow_path: String,
    /// Definition mtime when the session's engine contract was resolved.
    /// Reattach compares against the file's current mtime: a drifted value
    /// marks the session `needs_rethread` so an edited flow never keeps a
    /// thread built from the old contract.
    pub flow_mtime_ms: u64,
    /// Cwd every turn runs in (pinned at session start).
    pub cwd: String,
    pub transport: SessionTransport,
    pub state: SessionState,
    pub started_at: std::time::Instant,
    /// Committed turns (user + final assistant text) for context rollup.
    pub turns: Vec<SessionTurn>,
    /// Active turn: transport bookkeeping + ChatPrompt streaming message id.
    pub active_turn: Option<ActiveTurn>,
    /// Codex thread transport: true once `thread/start` was answered — the
    /// footer shows "Connecting" instead of "Working" until then.
    pub thread_ready: bool,
    /// Codex thread transport: the engine died and the next submit lands on
    /// a FRESH protocol thread. The submit path must re-resolve the flow
    /// contract and carry the transcript rollup so the flow's identity and
    /// conversation survive — never continue as a generic new thread.
    /// This boolean is transport bookkeeping only; user-facing recovery
    /// actions are owned by [`FlowSessionMeta::reliability`] (S09).
    pub needs_rethread: bool,
    /// Reducer-driven reliability/recovery state for this conversation.
    pub reliability: FlowReliability,
}

/// Reducer-driven reliability state for ONE flow conversation (S09).
///
/// A thin app-boundary driver over the pure `sk_protocol::ai_reliability`
/// state machine: turns, cancellations, failures, and recovery selections
/// flow through `transition`, so typed recovery state — not ad-hoc booleans —
/// owns which user actions the Flow surface offers. GPUI-free by design so
/// it unit-tests under `flows::session`.
#[derive(Debug, Clone)]
pub struct FlowReliability {
    state: sk_protocol::ai_reliability::AiOperationState,
}

impl FlowReliability {
    pub fn new(flow_id: &str, flow_path: &str, engine: &str) -> Self {
        use sk_protocol::ai_reliability::{
            AiOperationState, AiSelectionState, AiSurfaceIdentity, EngineId, Fingerprint, FlowId,
            RetryPolicy, SelectionOrigin,
        };
        let identity = AiSurfaceIdentity::FlowConversation {
            flow_id: FlowId::from(flow_id),
            definition_fingerprint: Fingerprint(crate::ai::reliability::redacted_fingerprint(
                flow_path,
            )),
            engine_id: EngineId::from(engine),
            provider_id: None,
            model_id: None,
        };
        let selection = AiSelectionState {
            requested: None,
            effective: None,
            origin: SelectionOrigin::BuiltInDefault,
            acknowledged_change: None,
        };
        let state = AiOperationState::ready(
            identity,
            selection,
            Self::work_snapshot(flow_id, flow_path, 0, false),
            RetryPolicy {
                automatic_max: 0,
                manual_max: 2,
            },
        );
        Self { state }
    }

    fn work_snapshot(
        flow_id: &str,
        flow_path: &str,
        turns: usize,
        partial_output: bool,
    ) -> sk_protocol::ai_reliability::AiWorkSnapshot {
        use sk_protocol::ai_reliability::{
            AiWorkSnapshot, Fingerprint, PreservationReceipt, WorkKey,
        };
        let fingerprint =
            |value: &str| Fingerprint(crate::ai::reliability::redacted_fingerprint(value));
        AiWorkSnapshot {
            key: WorkKey::from(format!("flow:{flow_id}:{flow_path}")),
            transcript: if turns == 0 {
                PreservationReceipt::NotApplicable
            } else {
                PreservationReceipt::Preserved {
                    fingerprint: fingerprint(&format!("turns:{turns}")),
                }
            },
            draft: PreservationReceipt::NotApplicable,
            attachments: PreservationReceipt::NotApplicable,
            partial_output: if partial_output {
                PreservationReceipt::Preserved {
                    fingerprint: fingerprint("partial"),
                }
            } else {
                PreservationReceipt::NotApplicable
            },
        }
    }

    pub fn state(&self) -> &sk_protocol::ai_reliability::AiOperationState {
        &self.state
    }

    pub fn awaiting_recovery(&self) -> bool {
        matches!(
            self.state.phase,
            sk_protocol::ai_reliability::AiPhase::AwaitingRecovery { .. }
        )
    }

    fn apply(
        &mut self,
        event: sk_protocol::ai_reliability::AiOperationEvent,
    ) -> Option<Vec<sk_protocol::ai_reliability::AiCommand>> {
        match sk_protocol::ai_reliability::transition(self.state.clone(), event) {
            Ok(next) => {
                self.state = next.next;
                Some(next.commands)
            }
            Err(invalid) => {
                tracing::warn!(
                    target: "script_kit::flows",
                    event = "flow_reliability_invalid_transition",
                    phase = ?invalid.phase,
                    rejected = ?invalid.event,
                    reason = ?invalid.reason,
                    "Flow reliability transition rejected"
                );
                None
            }
        }
    }

    fn reset_for_next_turn(&mut self) {
        use sk_protocol::ai_reliability::{AiOperationEvent, AiPhaseTag};
        match self.state.phase.tag() {
            AiPhaseTag::Ready => {}
            AiPhaseTag::AwaitingRecovery => {
                self.apply(AiOperationEvent::DismissRequested);
                self.apply(AiOperationEvent::ResetForNextTurn);
            }
            AiPhaseTag::Recovered
            | AiPhaseTag::Succeeded
            | AiPhaseTag::Cancelled
            | AiPhaseTag::Dismissed => {
                self.apply(AiOperationEvent::ResetForNextTurn);
            }
            AiPhaseTag::Preflighting
            | AiPhaseTag::Running
            | AiPhaseTag::Cancelling
            | AiPhaseTag::Recovering => {}
        }
    }

    /// Drive one submitted turn to `Running`.
    pub fn begin_turn(&mut self, flow_id: &str, flow_path: &str, turn_ordinal: usize) {
        use sk_protocol::ai_reliability::{
            AiCommand, AiOperationEvent, CapabilityDecision, TurnRef, TurnRequestRef, TurnRisk,
        };
        self.reset_for_next_turn();
        let work = Self::work_snapshot(flow_id, flow_path, turn_ordinal, false);
        let selection = self.state.selection.clone();
        if self
            .apply(AiOperationEvent::SubmitRequested {
                request: TurnRequestRef::from(format!("flow:{flow_id}:{turn_ordinal}")),
                work,
                selection,
                risk: TurnRisk::MayMutate,
            })
            .is_none()
        {
            return;
        }
        let Some(commands) = self.apply(AiOperationEvent::CapabilityResolved(
            CapabilityDecision::Compatible,
        )) else {
            return;
        };
        let start = commands.iter().find_map(|command| match command {
            AiCommand::StartTurn(command) => Some(command.command_id),
            _ => None,
        });
        if let Some(command_id) = start {
            self.apply(AiOperationEvent::RuntimeStarted {
                command_id,
                turn: TurnRef::from(format!("flow:{flow_id}:{turn_ordinal}")),
            });
        }
    }

    pub fn complete_turn(&mut self) {
        use sk_protocol::ai_reliability::{AiOperationEvent, CompletionKind};
        self.apply(AiOperationEvent::Completed(CompletionKind::Complete));
    }

    /// A user stop is truthful cancellation — quiet stopped copy, never the
    /// shared error treatment.
    pub fn cancel_turn(&mut self, partial_output: bool) {
        use sk_protocol::ai_reliability::{AiOperationEvent, Fingerprint, PartialOutputState};
        self.apply(AiOperationEvent::CancelRequested);
        self.apply(AiOperationEvent::RuntimeCancelled {
            partial: if partial_output {
                PartialOutputState::Preserved {
                    fingerprint: Fingerprint(crate::ai::reliability::redacted_fingerprint(
                        "partial",
                    )),
                }
            } else {
                PartialOutputState::None
            },
        });
    }

    /// A turn-level failure. Falls back to the outside-turn projection when
    /// no turn is in flight (defensive: transport events can race a settle).
    pub fn fail_turn(&mut self, failure: sk_protocol::ai_reliability::AiFailure) {
        use sk_protocol::ai_reliability::{AiOperationEvent, AiPhaseTag};
        if matches!(
            self.state.phase.tag(),
            AiPhaseTag::Preflighting | AiPhaseTag::Running
        ) && self
            .apply(AiOperationEvent::Failed(failure.clone()))
            .is_some()
        {
            return;
        }
        self.fail_outside_turn(failure);
    }

    /// A session-level failure with no turn in flight (engine death while
    /// idle). The pure reducer has no event for this shape, so the
    /// actionable recovery phase is projected directly from the same pure
    /// plan builder the reducer uses.
    pub fn fail_outside_turn(&mut self, failure: sk_protocol::ai_reliability::AiFailure) {
        use sk_protocol::ai_reliability::{recovery_plan_for, AiPhase, ProgressSnapshot, TurnRisk};
        let plan = recovery_plan_for(
            &self.state.identity,
            &failure,
            self.state.retry,
            TurnRisk::MayMutate,
            &ProgressSnapshot::none(),
        );
        self.state.diagnostic = failure.diagnostic.clone();
        self.state.phase = AiPhase::AwaitingRecovery { failure, plan };
    }

    pub fn select_recovery(
        &mut self,
        action: sk_protocol::ai_reliability::AiRecoveryAction,
    ) -> Vec<sk_protocol::ai_reliability::AiCommand> {
        use sk_protocol::ai_reliability::AiOperationEvent;
        self.apply(AiOperationEvent::RecoverySelected(action))
            .unwrap_or_default()
    }

    /// Drive a manual Retry through backoff → preflight → running.
    /// Returns false when the reducer refused (e.g. retry budget exhausted).
    pub fn retry_turn(&mut self, flow_id: &str, turn_ordinal: usize) -> bool {
        use sk_protocol::ai_reliability::{
            AiCommand, AiOperationEvent, AiPhaseTag, AiRecoveryAction, CapabilityDecision, TurnRef,
        };
        let commands = self.select_recovery(AiRecoveryAction::Retry);
        let Some(backoff_id) = commands.iter().find_map(|command| match command {
            AiCommand::ScheduleBackoff { command_id, .. } => Some(*command_id),
            _ => None,
        }) else {
            return false;
        };
        self.apply(AiOperationEvent::BackoffElapsed {
            command_id: backoff_id,
        });
        let Some(commands) = self.apply(AiOperationEvent::CapabilityResolved(
            CapabilityDecision::Compatible,
        )) else {
            return false;
        };
        let Some(start) = commands.iter().find_map(|command| match command {
            AiCommand::StartTurn(command) => Some(command.command_id),
            _ => None,
        }) else {
            return false;
        };
        self.apply(AiOperationEvent::RuntimeStarted {
            command_id: start,
            turn: TurnRef::from(format!("flow:{flow_id}:retry:{turn_ordinal}")),
        });
        matches!(self.state.phase.tag(), AiPhaseTag::Running)
    }

    /// Select Rethread and acknowledge the effect once the host performed it.
    /// Returns false when the reducer refused the action.
    pub fn select_rethread(&mut self) -> bool {
        use sk_protocol::ai_reliability::{
            AiCommand, AiOperationEvent, AiRecoveryAction, RecoveryEffectResult,
        };
        let commands = self.select_recovery(AiRecoveryAction::RethreadFlow);
        let Some(command_id) = commands.iter().find_map(|command| match command {
            AiCommand::RethreadFlow(command) => Some(command.command_id),
            _ => None,
        }) else {
            return false;
        };
        self.apply(AiOperationEvent::RecoveryCommandSucceeded {
            command_id,
            result: RecoveryEffectResult::FlowRethreaded,
        });
        true
    }

    pub fn dismiss(&mut self) {
        use sk_protocol::ai_reliability::AiOperationEvent;
        self.apply(AiOperationEvent::DismissRequested);
    }
}

/// Bookkeeping for the in-flight turn.
#[derive(Debug, Clone)]
pub struct ActiveTurn {
    /// Registry run id for [`SessionTransport::MdflowTurns`]; `None` on the
    /// codex thread transport.
    pub run_id: Option<u64>,
    /// ChatPrompt streaming message this turn appends into.
    pub message_id: String,
    /// Assistant text forwarded so far (also the mdflow tail watermark).
    pub assistant_acc: String,
    /// Codex agentMessage item currently streaming (a turn can carry
    /// several items; boundaries render as paragraph breaks).
    pub current_item_id: Option<String>,
    /// Accumulated text of the CURRENT item only — `item/completed`
    /// reconciliation compares against this, never the whole turn.
    pub item_acc: String,
    pub user_text: String,
}

impl ActiveTurn {
    /// Enter an agentMessage item. A turn carries several items; when the
    /// stream moves to a NEW item this resets the per-item accumulator that
    /// `item/completed` reconciliation compares against, and returns true
    /// when a paragraph break must be appended first so consecutive items
    /// never butt-join ("…summarizing.The listed…"). Re-entering the
    /// current item is a no-op returning false.
    pub fn enter_item(&mut self, item_id: &str) -> bool {
        if self.current_item_id.as_deref() == Some(item_id) {
            return false;
        }
        self.current_item_id = Some(item_id.to_string());
        self.item_acc.clear();
        !self.assistant_acc.is_empty() && !self.assistant_acc.ends_with("\n\n")
    }
}

impl FlowSessionMeta {
    pub fn elapsed_label(&self) -> String {
        let secs = self.started_at.elapsed().as_secs();
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            format!("{}m", secs / 60)
        } else {
            format!("{}h", secs / 3600)
        }
    }
}

/// Cap on rolled-up history characters per turn prompt. Oldest turns fall
/// off first; the newest message always survives intact.
const HISTORY_CHAR_BUDGET: usize = 8_000;

/// Build the mdflow turn prompt: prior transcript (newest-biased, budgeted)
/// then the new message. First turn passes the message verbatim so simple
/// one-shot flows behave exactly like the CLI.
pub fn build_turn_task(turns: &[SessionTurn], message: &str) -> String {
    if turns.is_empty() {
        return message.to_string();
    }
    let mut history: Vec<String> = Vec::new();
    let mut used = 0usize;
    for turn in turns.iter().rev() {
        // Outcome-aware rollup (Oracle 2026-07-21): a stopped/failed partial
        // must never be presented to the engine as an ordinary completed
        // answer, and UI captions/transport errors never enter the prompt.
        let assistant_label = match turn.outcome {
            PersistedTurnOutcome::Ok => "Assistant",
            PersistedTurnOutcome::Stopped => "Assistant (partial; turn stopped)",
            PersistedTurnOutcome::Failed => "Assistant (partial; turn failed)",
        };
        let block = format!(
            "User: {}\n{}: {}",
            turn.user, assistant_label, turn.assistant
        );
        if used + block.len() > HISTORY_CHAR_BUDGET {
            break;
        }
        used += block.len();
        history.push(block);
    }
    history.reverse();
    format!(
        "Conversation so far (for context):\n\n{}\n\nReply to the user's new message:\n{}",
        history.join("\n\n"),
        message
    )
}

/// The one user-prompt argument for a non-Codex Threadline turn.
///
/// Positional `_1` is mdflow's ordinary/public flow contract. `NamedTask` is
/// selected only after positional explain fails and a second, engine-free
/// explain proves that the flow consumes `{{ _task }}`. This prevents an
/// unused `--_task` flag from starving required `_1` before engine launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdflowTurnArg {
    Positional(String),
    NamedTask(String),
}

/// Resolve the turn argument shape without launching the flow's engine.
/// Prefer the public positional contract; fall back to `--_task` only when
/// `md explain` accepts that named input. If neither shape resolves, fail
/// closed rather than risk launching an engine with a guessed prompt shape.
pub fn resolve_mdflow_turn_arg(
    binary: &str,
    flow_path: &str,
    cwd: &str,
    prompt: &str,
) -> Result<MdflowTurnArg, String> {
    resolve_mdflow_turn_arg_with_deadline(
        binary,
        flow_path,
        cwd,
        prompt,
        Instant::now() + MD_EXPLAIN_DEADLINE,
    )
}

fn resolve_mdflow_turn_arg_with_deadline(
    binary: &str,
    flow_path: &str,
    cwd: &str,
    prompt: &str,
    deadline: Instant,
) -> Result<MdflowTurnArg, String> {
    let resolves_all_template_vars = |result: &MdExplainOutput| {
        result.output.status.success()
            && result
                .explain
                .as_ref()
                .is_some_and(|explain| explain.missing_template_vars.is_empty())
    };

    let positional_args = [prompt];
    let positional =
        run_md_explain_with_deadline(binary, flow_path, cwd, &positional_args, deadline)
            .map_err(|err| format!("mdflow explain failed for positional input: {err}"))?;
    if resolves_all_template_vars(&positional) {
        return Ok(MdflowTurnArg::Positional(prompt.to_string()));
    }

    let named_args = ["--_task", prompt];
    let named = run_md_explain_with_deadline(binary, flow_path, cwd, &named_args, deadline)
        .map_err(|err| format!("mdflow explain failed for --_task input: {err}"))?;
    if resolves_all_template_vars(&named) {
        return Ok(MdflowTurnArg::NamedTask(prompt.to_string()));
    }

    let diagnostic = |result: &MdExplainOutput| {
        if let Some(explain) = &result.explain {
            if !explain.missing_template_vars.is_empty() {
                return format!(
                    "unresolved template variables: {}",
                    explain.missing_template_vars.join(", ")
                );
            }
        }
        String::from_utf8_lossy(&result.output.stderr)
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("mdflow explain rejected the input")
            .to_string()
    };
    Err(format!(
        "mdflow explain rejected both Threadline input shapes (positional: {}; --_task: {})",
        diagnostic(&positional),
        diagnostic(&named)
    ))
}

/// Resolve a flow's mission for the FIRST codex-thread turn: frontmatter
/// stripped, `{{ _task }}` substituted with the user's message (appended
/// when the template has no task slot). Later turns send the raw message —
/// the protocol thread holds context.
///
/// This is a deliberate v1 of mdflow's own resolution (`md explain --json`
/// is the robust path once its output is cached per flow); flows in
/// `@johnlindquist/flows` are frontmatter + prose + `{{ _task }}`.
/// Frontmatter contract a flow pins for its conversation thread. Passed
/// into `thread/start` so a codex session honors the flow's declared
/// `model:`/`sandbox:` instead of silently falling back to the user's
/// global codex defaults, and carries the mission as developer
/// instructions so it survives every turn (and engine re-threads).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlowThreadProfile {
    pub model: Option<String>,
    pub sandbox: Option<String>,
    pub developer_instructions: Option<String>,
}

/// Resolved first-turn contract for the codex thread transport.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowThreadContract {
    pub profile: FlowThreadProfile,
    pub first_prompt: String,
}

/// Resolve a flow definition into its thread contract. Bodies that template
/// `{{ _task }}` keep the legacy shape (mission+task as the first prompt);
/// plain bodies become persistent developer instructions and the first
/// prompt stays the user's own words.
pub fn resolve_flow_thread_contract(markdown: &str, task: &str) -> FlowThreadContract {
    let (model, sandbox) = parse_frontmatter_overrides(markdown);
    let body = strip_frontmatter(markdown).trim();
    let templated = body.contains("{{ _task }}") || body.contains("{{_task}}");
    if templated || body.is_empty() {
        FlowThreadContract {
            profile: FlowThreadProfile {
                model,
                sandbox,
                developer_instructions: None,
            },
            first_prompt: resolve_flow_mission(markdown, task),
        }
    } else {
        FlowThreadContract {
            profile: FlowThreadProfile {
                model,
                sandbox,
                developer_instructions: Some(body.to_string()),
            },
            first_prompt: task.to_string(),
        }
    }
}

/// Minimal frontmatter scan for the two keys codex `thread/start` accepts.
/// Values pass through verbatim (quotes stripped) — an invalid sandbox mode
/// should fail the thread start loudly, never be silently dropped.
fn parse_frontmatter_overrides(markdown: &str) -> (Option<String>, Option<String>) {
    let Some(rest) = markdown.strip_prefix("---") else {
        return (None, None);
    };
    let Some(end) = rest.find("\n---") else {
        return (None, None);
    };
    let block = &rest[..end];
    let mut model = None;
    let mut sandbox = None;
    for line in block.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "model" => model = Some(value.to_string()),
            "sandbox" => sandbox = Some(value.to_string()),
            _ => {}
        }
    }
    (model, sandbox)
}

pub fn resolve_flow_mission(markdown: &str, task: &str) -> String {
    let body = strip_frontmatter(markdown).trim();
    let with_task = if body.contains("{{ _task }}") || body.contains("{{_task}}") {
        body.replace("{{ _task }}", task).replace("{{_task}}", task)
    } else if body.is_empty() {
        task.to_string()
    } else {
        format!("{body}\n\n{task}")
    };
    with_task.trim().to_string()
}

fn strip_frontmatter(markdown: &str) -> &str {
    let Some(rest) = markdown.strip_prefix("---") else {
        return markdown;
    };
    match rest.find("\n---") {
        Some(end) => {
            let after = &rest[end + 4..];
            after.strip_prefix('\n').unwrap_or(after)
        }
        None => markdown,
    }
}

// ---------------------------------------------------------------------
// Conversation persistence (survives app restarts)
// ---------------------------------------------------------------------

/// One most-recent conversation snapshot per flow, rewritten after every
/// committed turn. `flow_sessions` is in-memory only, so a dev rebuild or
/// app restart used to strand the user's conversation: Enter on the flow's
/// launcher row landed in a blank composer (2026-07-10 report). A restored
/// session sets `needs_rethread`, so the next submit rolls this transcript
/// back into the engine prompt via `build_turn_task`.
///
/// Identity is `flow_id` + `flow_path`: protocol flow ids are only
/// `<source>:<slug>` (`project:review`), so two different projects can carry
/// the same id — keying by id alone restored the WRONG project's transcript
/// into the wrong agent (2026-07-11 audit P0, correctness + privacy).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowConversation {
    pub flow_id: String,
    /// Definition path this conversation belongs to (empty on legacy
    /// snapshots persisted before identity was path-qualified).
    #[serde(default)]
    pub flow_path: String,
    pub saved_at: String,
    /// Snapshot format version. 0 (absent) = legacy: either two-field turns
    /// or transitional records whose Stopped assistants carry the UI caption
    /// baked into the text. 2 = raw assistant text with the caption derived
    /// from `outcome` at display time, failures as raw caption strings.
    /// `SNAPSHOT_VERSION` (3) = failures persisted as typed
    /// [`PersistedAiFailure`] records; the legacy `error` field is never
    /// written and is classified into a typed record while loading.
    #[serde(default)]
    pub version: u32,
    pub turns: Vec<PersistedFlowTurn>,
}

/// Current snapshot format: raw assistant text + structured outcome +
/// typed persisted failures.
pub const SNAPSHOT_VERSION: u32 = 3;

/// Convert a persisted snapshot into the ONE canonical in-memory turn vector
/// (Oracle 2026-07-21, WP-A4): restore must render and store from this same
/// vector, never from the raw persisted fields.
///
/// Normalization invariants:
/// - `Ok`/`Stopped` ⇒ `failure = None` (stopped turns never carry a failure).
/// - `Failed` ⇒ `failure = Some(typed)`: the typed record when the snapshot
///   has one, otherwise the legacy v0–v2 `error` caption classified while
///   loading (blank/absent → the `Unknown` default).
/// - Pre-version-2 Stopped records may carry the UI caption baked into the
///   assistant text; strip exactly one canonical caption suffix so
///   `assistant` is raw engine output.
pub fn canonical_session_turns(snapshot: &PersistedFlowConversation) -> Vec<SessionTurn> {
    /// Caption stripping applies to snapshots written before version 2
    /// introduced outcome-derived display captions.
    const CAPTION_DERIVED_VERSION: u32 = 2;
    snapshot
        .turns
        .iter()
        .map(|turn| {
            let mut assistant = turn.assistant.clone();
            if snapshot.version < CAPTION_DERIVED_VERSION
                && turn.outcome == PersistedTurnOutcome::Stopped
            {
                if assistant == FLOW_STOPPED_CAPTION {
                    assistant.clear();
                } else if let Some(stripped) =
                    assistant.strip_suffix(&format!("\n\n{FLOW_STOPPED_CAPTION}"))
                {
                    assistant = stripped.to_string();
                }
            }
            let failure = match turn.outcome {
                PersistedTurnOutcome::Ok | PersistedTurnOutcome::Stopped => None,
                PersistedTurnOutcome::Failed => Some(match &turn.failure {
                    Some(failure) => failure.clone(),
                    None => turn
                        .error
                        .as_deref()
                        .map(str::trim)
                        .filter(|error| !error.is_empty())
                        .map(PersistedAiFailure::from_legacy_error)
                        .unwrap_or_else(PersistedAiFailure::unknown_default),
                }),
            };
            SessionTurn {
                user: turn.user.clone(),
                assistant,
                outcome: turn.outcome,
                failure,
            }
        })
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowTurn {
    pub user: String,
    pub assistant: String,
    #[serde(default)]
    pub outcome: PersistedTurnOutcome,
    /// Legacy (v0–v2) raw failure caption. Read-only: version-3 snapshots
    /// never write it; loading classifies it into `failure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Typed persisted failure (version 3+).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<PersistedAiFailure>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PersistedTurnOutcome {
    #[default]
    Ok,
    Stopped,
    Failed,
}

/// Newest persisted turns kept per flow — comfortably above what
/// `build_turn_task`'s `HISTORY_CHAR_BUDGET` can roll up, without letting
/// the snapshot grow unbounded.
const PERSISTED_TURN_CAP: usize = 12;

fn conversation_store_dir() -> std::path::PathBuf {
    crate::setup::get_kit_path()
        .join("flows")
        .join("conversations")
}

/// Filesystem-safe slug of one identity component. Output is pure ASCII, so
/// byte-slicing the result is always char-boundary-safe.
fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Conversation file name: flow id PLUS definition path, so two projects
/// with the same `project:review` id can never share (or steal) a
/// transcript. The path portion keeps its most distinctive tail when it
/// would push the file name over conservative filesystem limits.
fn conversation_file_name(flow_id: &str, flow_path: &str) -> String {
    let id = sanitize_component(flow_id);
    let mut path = sanitize_component(flow_path.trim_start_matches('/'));
    const PATH_PORTION_MAX: usize = 160;
    if path.len() > PATH_PORTION_MAX {
        path = path[path.len() - PATH_PORTION_MAX..].to_string();
    }
    format!("{id}--{path}.json")
}

/// Legacy (pre path-qualified identity) file name, keyed by flow id alone.
fn legacy_conversation_file_name(flow_id: &str) -> String {
    format!("{}.json", sanitize_component(flow_id))
}

pub fn persist_conversation_to(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
    turns: &[SessionTurn],
) -> std::io::Result<()> {
    let kept: Vec<PersistedFlowTurn> = turns
        .iter()
        .rev()
        .take(PERSISTED_TURN_CAP)
        .rev()
        .map(|turn| PersistedFlowTurn {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
            outcome: turn.outcome,
            // v3 never writes the legacy raw caption field.
            error: None,
            failure: turn.failure.clone(),
        })
        .collect();
    let snapshot = PersistedFlowConversation {
        flow_id: flow_id.to_string(),
        flow_path: flow_path.to_string(),
        saved_at: chrono::Utc::now().to_rfc3339(),
        version: SNAPSHOT_VERSION,
        turns: kept,
    };
    std::fs::create_dir_all(dir)?;
    let path = dir.join(conversation_file_name(flow_id, flow_path));
    // Unique-temp atomic write (see src/atomic_file.rs): a fixed temp path let
    // concurrent savers corrupt the persisted conversation.
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(std::io::Error::other)?;
    crate::atomic_file::write_atomic(&path, &bytes)
}

pub fn load_persisted_conversation_from(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
) -> Option<PersistedFlowConversation> {
    let path = dir.join(conversation_file_name(flow_id, flow_path));
    if let Ok(raw) = std::fs::read_to_string(&path) {
        let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
        return (!snapshot.turns.is_empty()).then_some(snapshot);
    }
    // Legacy adoption (one-shot): a pre-identity snapshot keyed by id alone
    // is claimed by the FIRST flow that opens it, re-persisted under the
    // path-qualified name, and the legacy file removed — so it can never
    // silently leak into another project again.
    let legacy = dir.join(legacy_conversation_file_name(flow_id));
    let raw = std::fs::read_to_string(&legacy).ok()?;
    let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
    if snapshot.turns.is_empty() {
        return None;
    }
    // Adopt through the ONE canonical conversion so the re-persisted file
    // is a fully migrated current-version snapshot (typed failures included).
    let turns: Vec<SessionTurn> = canonical_session_turns(&snapshot);
    if persist_conversation_to(dir, flow_id, flow_path, &turns).is_ok() {
        let _ = std::fs::remove_file(&legacy);
    }
    Some(PersistedFlowConversation {
        flow_path: flow_path.to_string(),
        ..snapshot
    })
}

/// Erase a conversation's persisted history (both the path-qualified file
/// and any legacy id-only file). "Terminate Flow" promises permanent
/// removal — before this existed, the next activation silently restored the
/// supposedly terminated conversation (2026-07-11 audit P0).
pub fn delete_persisted_conversation_from(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
) -> std::io::Result<()> {
    // NotFound is success (the promise is "no transcript remains"); every
    // other failure — e.g. permissions — must surface, because a silently
    // surviving file resurrects a "permanently ended" conversation.
    let mut result = Ok(());
    for path in [
        dir.join(conversation_file_name(flow_id, flow_path)),
        dir.join(legacy_conversation_file_name(flow_id)),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => result = Err(err),
        }
    }
    result
}

/// One FIFO worker owns every conversation-store mutation (Oracle 2026-07-21
/// WP-A1): per-turn detached threads let an older snapshot finish AFTER a
/// newer one (silent transcript regression) and let a pending persist
/// resurrect a terminated conversation. Commands are enqueued synchronously
/// from the UI thread, so on-disk order always matches user-visible order.
pub struct FlowConversationStore {
    tx: std::sync::mpsc::Sender<ConversationStoreCommand>,
}

enum ConversationStoreCommand {
    Persist {
        flow_id: String,
        flow_path: String,
        turns: Vec<SessionTurn>,
    },
    Delete {
        flow_id: String,
        flow_path: String,
    },
    Flush(std::sync::mpsc::Sender<()>),
}

impl FlowConversationStore {
    /// Store rooted at `dir`. Tests construct their own with a temp dir; the
    /// app uses [`conversation_store`].
    pub fn new(dir: std::path::PathBuf) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<ConversationStoreCommand>();
        let spawned = std::thread::Builder::new()
            .name("flow-conversation-store".into())
            .spawn(move || {
                while let Ok(command) = rx.recv() {
                    match command {
                        ConversationStoreCommand::Persist {
                            flow_id,
                            flow_path,
                            turns,
                        } => {
                            if turns.is_empty() {
                                continue;
                            }
                            if let Err(err) =
                                persist_conversation_to(&dir, &flow_id, &flow_path, &turns)
                            {
                                tracing::warn!(
                                    target: "script_kit::flows",
                                    event = "flow_conversation_persist_failed",
                                    flow_id = %flow_id,
                                    error = %err,
                                    "Failed to persist flow conversation"
                                );
                            }
                        }
                        ConversationStoreCommand::Delete { flow_id, flow_path } => {
                            if let Err(err) =
                                delete_persisted_conversation_from(&dir, &flow_id, &flow_path)
                            {
                                tracing::warn!(
                                    target: "script_kit::flows",
                                    event = "flow_conversation_delete_failed",
                                    flow_id = %flow_id,
                                    error = %err,
                                    "Failed to delete persisted flow conversation"
                                );
                            }
                        }
                        ConversationStoreCommand::Flush(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            });
        if let Err(err) = spawned {
            // Thread creation failure must be loud — a store with no worker
            // silently drops every persistence command.
            tracing::error!(
                target: "script_kit::flows",
                event = "flow_conversation_store_spawn_failed",
                error = %err,
                "Flow conversation store worker failed to start"
            );
        }
        Self { tx }
    }

    pub fn persist(&self, flow_id: &str, flow_path: &str, turns: Vec<SessionTurn>) {
        let _ = self.tx.send(ConversationStoreCommand::Persist {
            flow_id: flow_id.to_string(),
            flow_path: flow_path.to_string(),
            turns,
        });
    }

    pub fn delete(&self, flow_id: &str, flow_path: &str) {
        let _ = self.tx.send(ConversationStoreCommand::Delete {
            flow_id: flow_id.to_string(),
            flow_path: flow_path.to_string(),
        });
    }

    /// Barrier: returns once every previously enqueued command has reached
    /// disk. Used by tests and shutdown.
    pub fn flush(&self) {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        if self
            .tx
            .send(ConversationStoreCommand::Flush(done_tx))
            .is_ok()
        {
            let _ = done_rx.recv_timeout(std::time::Duration::from_secs(10));
        }
    }
}

/// The app-wide store rooted at the active workspace.
pub fn conversation_store() -> &'static FlowConversationStore {
    static STORE: std::sync::OnceLock<FlowConversationStore> = std::sync::OnceLock::new();
    STORE.get_or_init(|| FlowConversationStore::new(conversation_store_dir()))
}

/// Persist under the active workspace (`~/.scriptkit`, `SK_PATH` override).
pub fn persist_conversation(flow_id: &str, flow_path: &str, turns: &[SessionTurn]) {
    if turns.is_empty() {
        return;
    }
    if let Err(err) = persist_conversation_to(&conversation_store_dir(), flow_id, flow_path, turns)
    {
        tracing::warn!(
            target: "script_kit::flows",
            event = "flow_conversation_persist_failed",
            flow_id = %flow_id,
            error = %err,
            "Failed to persist flow conversation"
        );
    }
}

pub fn load_persisted_conversation(
    flow_id: &str,
    flow_path: &str,
) -> Option<PersistedFlowConversation> {
    load_persisted_conversation_from(&conversation_store_dir(), flow_id, flow_path)
}

pub fn delete_persisted_conversation(flow_id: &str, flow_path: &str) {
    if let Err(err) =
        delete_persisted_conversation_from(&conversation_store_dir(), flow_id, flow_path)
    {
        tracing::warn!(
            target: "script_kit::flows",
            event = "flow_conversation_delete_failed",
            flow_id = %flow_id,
            error = %err,
            "Failed to delete persisted flow conversation"
        );
    }
}

/// Definition-file mtime in ms (0 when unreadable) — the cheap staleness
/// signal for reattaching live sessions: an edited flow must not silently
/// keep a thread built from the old contract.
pub fn flow_definition_mtime_ms(path: &str) -> u64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_resolution_strips_frontmatter_and_substitutes_task() {
        let markdown =
            "---\ndescription: GitHub examples\n---\nSearch GitHub for examples.\n\n{{ _task }}\n";
        assert_eq!(
            resolve_flow_mission(markdown, "bun shell scripts"),
            "Search GitHub for examples.\n\nbun shell scripts"
        );
    }

    #[test]
    fn mission_without_task_slot_appends_message() {
        assert_eq!(
            resolve_flow_mission("Reply tersely.", "hello"),
            "Reply tersely.\n\nhello"
        );
        assert_eq!(resolve_flow_mission("", "hello"), "hello");
    }

    #[test]
    fn transport_picks_codex_thread_only_for_codex() {
        assert_eq!(
            SessionTransport::for_engine("codex"),
            SessionTransport::CodexThread
        );
        assert_eq!(
            SessionTransport::for_engine("claude"),
            SessionTransport::MdflowTurns
        );
        assert_eq!(
            SessionTransport::for_engine("fasteng"),
            SessionTransport::MdflowTurns
        );
    }

    #[test]
    fn first_turn_task_is_verbatim() {
        assert_eq!(
            build_turn_task(&[], "what did vercel email me?"),
            "what did vercel email me?"
        );
    }

    #[cfg(unix)]
    fn write_fake_md(path: &std::path::Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, body).expect("write fake md");
        let mut permissions = std::fs::metadata(path)
            .expect("fake md metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("make fake md executable");
    }

    #[cfg(unix)]
    mod of38 {
        use super::*;
        use std::time::Duration;

        #[test]
        fn turn_arg_resolution_uses_one_deadline_for_both_shapes() {
            let dir = tempfile::tempdir().expect("tempdir");
            let binary = dir.path().join("md");
            let attempts = dir.path().join("of38-attempts.txt");
            let named_overbudget = dir.path().join("of38-named-overbudget.txt");
            write_fake_md(
            &binary,
            "#!/bin/sh\nprintf '%s %s\\n' \"$$\" \"$3\" >> of38-attempts.txt\nif [ \"$3\" != \"--_task\" ]; then\n  sleep 0.02\n  printf 'positional rejected\\n' >&2\n  exit 1\nfi\nsleep 300 &\nchild=$!\nprintf '%s %s\\n' \"$$\" \"$child\" >> of38-attempts.txt\n(sleep 0.04; touch of38-named-overbudget.txt) &\nwait \"$child\"\n",
        );

            let started = Instant::now();
            let result = resolve_mdflow_turn_arg_with_deadline(
                binary.to_str().expect("utf8 binary"),
                "flow.md",
                dir.path().to_str().expect("utf8 cwd"),
                "hello",
                started + Duration::from_millis(50),
            );

            assert!(result.is_err(), "the named fallback must time out");
            assert!(started.elapsed() < Duration::from_secs(1));
            let attempt_log = std::fs::read_to_string(attempts).expect("both attempts logged");
            assert!(
                attempt_log.contains(" hello"),
                "positional shape was attempted"
            );
            assert!(
                attempt_log.contains(" --_task"),
                "named shape was attempted"
            );
            assert!(
                !named_overbudget.exists(),
                "named fallback received a fresh deadline instead of the remaining total budget"
            );
        }
    }

    #[cfg(unix)]
    mod of39 {
        use super::*;

        #[test]
        fn turn_arg_resolution_uses_parsed_missing_template_vars() {
            let dir = tempfile::tempdir().expect("tempdir");
            let binary = dir.path().join("md");
            let calls = dir.path().join("of39-calls.txt");
            write_fake_md(
                &binary,
                "#!/bin/sh\nprintf '%s %s\\n' \"$2\" \"$3\" >> of39-calls.txt\nif [ \"$2\" = \"ordinary.md\" ]; then\n  printf '%s\\n' '{\"protocolVersion\":1,\"flowId\":\"project:ordinary\",\"path\":\"ordinary.md\",\"engine\":\"pi\",\"command\":\"pi\",\"args\":[],\"cwd\":\".\",\"prompt\":\"ok\",\"promptTokensEstimate\":1,\"inputs\":[],\"warnings\":[],\"configFingerprint\":\"sha256:ordinary\",\"templateVars\":[\"_1\"],\"missingTemplateVars\":[]}'\n  exit 0\nfi\nif [ \"$3\" = \"--_task\" ]; then\n  missing='[]'\nelse\n  missing='[\"_task\"]'\nfi\nprintf '%s\\n' \"{\\\"protocolVersion\\\":1,\\\"flowId\\\":\\\"project:named\\\",\\\"path\\\":\\\"named.md\\\",\\\"engine\\\":\\\"pi\\\",\\\"command\\\":\\\"pi\\\",\\\"args\\\":[],\\\"cwd\\\":\\\".\\\",\\\"prompt\\\":\\\"ok\\\",\\\"promptTokensEstimate\\\":1,\\\"inputs\\\":[],\\\"warnings\\\":[],\\\"configFingerprint\\\":\\\"sha256:named\\\",\\\"templateVars\\\":[\\\"_task\\\"],\\\"missingTemplateVars\\\":$missing}\"\n",
            );

            let ordinary = resolve_mdflow_turn_arg(
                binary.to_str().expect("utf8 binary"),
                "ordinary.md",
                dir.path().to_str().expect("utf8 cwd"),
                "hello ordinary",
            )
            .expect("ordinary positional input resolves");
            assert_eq!(ordinary, MdflowTurnArg::Positional("hello ordinary".into()));

            let named = resolve_mdflow_turn_arg(
                binary.to_str().expect("utf8 binary"),
                "named.md",
                dir.path().to_str().expect("utf8 cwd"),
                "hello named",
            )
            .expect("named input resolves through fallback");
            assert_eq!(named, MdflowTurnArg::NamedTask("hello named".into()));

            let calls = std::fs::read_to_string(calls).expect("explain calls logged");
            assert_eq!(
                calls.lines().collect::<Vec<_>>(),
                [
                    "ordinary.md hello ordinary",
                    "named.md hello named",
                    "named.md --_task",
                ],
                "ordinary resolves once; named tries positional then --_task"
            );
        }
    }

    const GMAIL_PATH: &str = "/pkg/flows/flow-gog-gmail.codex.md";

    #[test]
    fn conversation_persistence_round_trips_and_caps_turns() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns: Vec<SessionTurn> = (0..20)
            .map(|i| SessionTurn {
                user: format!("question {i}"),
                assistant: format!("answer {i}"),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            })
            .collect();
        persist_conversation_to(dir.path(), "package:flow-gog-gmail", GMAIL_PATH, &turns)
            .expect("persist");

        let restored =
            load_persisted_conversation_from(dir.path(), "package:flow-gog-gmail", GMAIL_PATH)
                .expect("snapshot must load");
        assert_eq!(restored.flow_id, "package:flow-gog-gmail");
        assert_eq!(restored.flow_path, GMAIL_PATH);
        // Newest PERSISTED_TURN_CAP turns survive, oldest fall off.
        assert_eq!(restored.turns.len(), 12);
        assert_eq!(restored.turns.first().unwrap().user, "question 8");
        assert_eq!(restored.turns.last().unwrap().assistant, "answer 19");
    }

    #[test]
    fn persisted_flow_turn_roundtrips_outcome() {
        for turn in [
            PersistedFlowTurn {
                user: "stop".into(),
                assistant: "partial\n\n*Stopped.*".into(),
                outcome: PersistedTurnOutcome::Stopped,
                error: None,
                failure: None,
            },
            PersistedFlowTurn {
                user: "fail".into(),
                assistant: "partial".into(),
                outcome: PersistedTurnOutcome::Failed,
                error: Some("transport failed".into()),
                failure: None,
            },
            PersistedFlowTurn {
                user: "typed fail".into(),
                assistant: "partial".into(),
                outcome: PersistedTurnOutcome::Failed,
                error: None,
                failure: Some(PersistedAiFailure::from_legacy_error(
                    "protocol violation: x",
                )),
            },
        ] {
            let json = serde_json::to_string(&turn).expect("serialize persisted turn");
            let restored: PersistedFlowTurn =
                serde_json::from_str(&json).expect("deserialize persisted turn");
            assert_eq!(restored.outcome, turn.outcome);
            assert_eq!(restored.error, turn.error);
            assert_eq!(restored.failure, turn.failure);
            assert_eq!(restored.assistant, turn.assistant);
        }

        let legacy: PersistedFlowTurn =
            serde_json::from_str(r#"{"user":"old question","assistant":"old answer"}"#)
                .expect("legacy two-field turn must deserialize");
        assert_eq!(legacy.outcome, PersistedTurnOutcome::Ok);
        assert_eq!(legacy.error, None);
        assert_eq!(legacy.failure, None);
    }

    /// S09: a v3 snapshot persists ONLY the typed failure — the legacy raw
    /// caption field is never written again.
    #[test]
    fn v3_snapshots_never_write_the_legacy_error_field() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = vec![SessionTurn {
            user: "q".into(),
            assistant: "partial".into(),
            outcome: PersistedTurnOutcome::Failed,
            failure: Some(PersistedAiFailure::unknown_default()),
        }];
        persist_conversation_to(dir.path(), "project:t", "/w/flows/t.md", &turns).expect("persist");
        let raw = std::fs::read_to_string(
            dir.path()
                .join(conversation_file_name("project:t", "/w/flows/t.md")),
        )
        .expect("snapshot file");
        assert!(
            !raw.contains("\"error\""),
            "v3 must not write the legacy raw error field: {raw}"
        );
        assert!(raw.contains("\"failure\""), "typed failure must persist");
        let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).expect("parse");
        assert_eq!(snapshot.version, SNAPSHOT_VERSION);
    }

    /// S09 migration: v2 failed turns carry only the raw caption; loading
    /// classifies it into a typed record from the closed legacy string set.
    #[test]
    fn v2_legacy_errors_are_classified_while_loading() {
        use sk_protocol::ai_reliability::AiFailureCode;
        let cases = [
            (
                "mdflow CLI not found on PATH (npm i -g mdflow)",
                AiFailureCode::MdflowMissing,
                PersistedAiFailureCategory::Configuration,
            ),
            (
                "protocol violation: unknown event",
                AiFailureCode::ProtocolMalformedResponse,
                PersistedAiFailureCategory::Protocol,
            ),
            (
                "failed to spawn md: no such file",
                AiFailureCode::SpawnFailed,
                PersistedAiFailureCategory::Runtime,
            ),
            (
                "Flow definition unreadable: /w/flows/x.md (gone)",
                AiFailureCode::InvalidConfiguration,
                PersistedAiFailureCategory::Configuration,
            ),
            (
                "totally novel failure text",
                AiFailureCode::Unknown,
                PersistedAiFailureCategory::Unknown,
            ),
        ];
        for (legacy, code, category) in cases {
            let snapshot = snapshot_with(
                2,
                vec![PersistedFlowTurn {
                    user: "q".into(),
                    assistant: "partial".into(),
                    outcome: PersistedTurnOutcome::Failed,
                    error: Some(legacy.into()),
                    failure: None,
                }],
            );
            let turns = canonical_session_turns(&snapshot);
            let failure = turns[0].failure.as_ref().expect("failed turn is typed");
            assert_eq!(failure.code, code, "{legacy}");
            assert_eq!(failure.category, category, "{legacy}");
            assert_eq!(failure.safe_summary, legacy);
        }
    }

    fn snapshot_with(version: u32, turns: Vec<PersistedFlowTurn>) -> PersistedFlowConversation {
        PersistedFlowConversation {
            flow_id: "project:test".into(),
            flow_path: "/w/flows/test.md".into(),
            saved_at: "2026-07-21T00:00:00Z".into(),
            version,
            turns,
        }
    }

    /// WP-A4: canonical conversion migrates transitional caption-bearing
    /// Stopped records to raw text and normalizes outcome/error invariants.
    #[test]
    fn canonical_session_turns_migrates_and_normalizes() {
        let snapshot = snapshot_with(
            0,
            vec![
                // Transitional Phase-A record: caption baked into assistant.
                PersistedFlowTurn {
                    user: "stop".into(),
                    assistant: format!("partial\n\n{FLOW_STOPPED_CAPTION}"),
                    outcome: PersistedTurnOutcome::Stopped,
                    error: None,
                    failure: None,
                },
                // Caption-only stopped record (empty raw output).
                PersistedFlowTurn {
                    user: "stop2".into(),
                    assistant: FLOW_STOPPED_CAPTION.into(),
                    outcome: PersistedTurnOutcome::Stopped,
                    error: None,
                    failure: None,
                },
                // Failed with blank error → nonblank typed fallback.
                PersistedFlowTurn {
                    user: "fail".into(),
                    assistant: "partial".into(),
                    outcome: PersistedTurnOutcome::Failed,
                    error: Some("   ".into()),
                    failure: None,
                },
                // Stopped with an impossible error → dropped.
                PersistedFlowTurn {
                    user: "odd".into(),
                    assistant: "text".into(),
                    outcome: PersistedTurnOutcome::Stopped,
                    error: Some("junk".into()),
                    failure: None,
                },
            ],
        );
        let turns = canonical_session_turns(&snapshot);
        assert_eq!(turns[0].assistant, "partial");
        assert_eq!(turns[0].outcome, PersistedTurnOutcome::Stopped);
        assert_eq!(turns[1].assistant, "");
        assert_eq!(
            turns[2].failure.as_ref().map(|f| f.safe_summary.as_str()),
            Some(FLOW_TURN_FAILED_SUMMARY)
        );
        assert_eq!(turns[3].failure, None, "Stopped never carries a failure");
    }

    /// Current-version snapshots are NOT caption-stripped: raw text that
    /// legitimately ends with the caption phrase stays verbatim.
    #[test]
    fn canonical_session_turns_leaves_current_version_raw() {
        let raw = format!("The literal marker is\n\n{FLOW_STOPPED_CAPTION}");
        let snapshot = snapshot_with(
            SNAPSHOT_VERSION,
            vec![PersistedFlowTurn {
                user: "u".into(),
                assistant: raw.clone(),
                outcome: PersistedTurnOutcome::Stopped,
                error: None,
                failure: None,
            }],
        );
        assert_eq!(canonical_session_turns(&snapshot)[0].assistant, raw);
    }

    fn turn(user: &str, assistant: &str) -> SessionTurn {
        SessionTurn {
            user: user.into(),
            assistant: assistant.into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }
    }

    /// WP-A1: the FIFO store guarantees on-disk order matches enqueue order —
    /// a newer snapshot can never be replaced by a stale one.
    #[test]
    fn conversation_store_newer_snapshot_wins() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        store.persist("project:t", "/w/flows/t.md", vec![turn("q1", "a1")]);
        store.persist(
            "project:t",
            "/w/flows/t.md",
            vec![turn("q1", "a1"), turn("q2", "a2")],
        );
        store.flush();
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("snapshot present");
        assert_eq!(loaded.turns.len(), 2, "the newer 2-turn snapshot must win");
    }

    /// WP-A1: a delete enqueued after a persist is a tombstone — the pending
    /// persist cannot resurrect the terminated conversation.
    #[test]
    fn conversation_store_delete_tombstone_survives_pending_persist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        store.persist("project:t", "/w/flows/t.md", vec![turn("q1", "a1")]);
        store.delete("project:t", "/w/flows/t.md");
        store.flush();
        assert!(
            load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md").is_none(),
            "terminated conversation must stay deleted"
        );
    }

    /// Delete reports real I/O failures but treats NotFound as success.
    #[test]
    fn delete_treats_missing_files_as_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            delete_persisted_conversation_from(dir.path(), "project:none", "/w/none.md").is_ok()
        );
    }

    /// WP-A3: rollup is outcome-aware — stopped/failed partials are labeled
    /// as partial, and the UI caption never enters the engine prompt.
    #[test]
    fn build_turn_task_labels_partial_outcomes_and_excludes_caption() {
        let turns = vec![
            SessionTurn {
                user: "q1".into(),
                assistant: "full answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            },
            SessionTurn {
                user: "q2".into(),
                assistant: "cut short".into(),
                outcome: PersistedTurnOutcome::Stopped,
                failure: None,
            },
            SessionTurn {
                user: "q3".into(),
                assistant: "broke".into(),
                outcome: PersistedTurnOutcome::Failed,
                failure: Some(PersistedAiFailure::from_legacy_error("transport exploded")),
            },
        ];
        let task = build_turn_task(&turns, "next question");
        assert!(task.contains("Assistant: full answer"));
        assert!(task.contains("Assistant (partial; turn stopped): cut short"));
        assert!(task.contains("Assistant (partial; turn failed): broke"));
        assert!(
            !task.contains(FLOW_STOPPED_CAPTION),
            "UI caption must never enter the engine prompt"
        );
        assert!(
            !task.contains("transport exploded"),
            "transport error text must never enter the engine prompt"
        );
    }

    #[test]
    fn persisted_conversation_missing_or_empty_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .is_none()
        );
        persist_conversation_to(dir.path(), "project:scout", "/a/flows/scout.md", &[])
            .expect("persist empty");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .is_none(),
            "an empty snapshot must never restore a blank conversation"
        );
    }

    /// Two projects with the same `project:review` id must never share a
    /// transcript (2026-07-11 audit P0: cross-project restore was both a
    /// correctness and privacy failure).
    #[test]
    fn same_flow_id_in_different_projects_gets_separate_transcripts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turn = |text: &str| {
            vec![SessionTurn {
                user: text.to_string(),
                assistant: format!("re: {text}"),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            }]
        };
        persist_conversation_to(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
            &turn("alpha secrets"),
        )
        .expect("persist alpha");
        persist_conversation_to(
            dir.path(),
            "project:review",
            "/work/beta/flows/review.md",
            &turn("beta question"),
        )
        .expect("persist beta");

        let alpha = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .expect("alpha loads");
        let beta = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/beta/flows/review.md",
        )
        .expect("beta loads");
        assert_eq!(alpha.turns[0].user, "alpha secrets");
        assert_eq!(beta.turns[0].user, "beta question");
    }

    /// Legacy id-only snapshots are adopted once (re-keyed under the
    /// path-qualified name, legacy file removed) so they can never leak
    /// into a second project later.
    #[test]
    fn legacy_snapshot_is_adopted_once_and_removed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let legacy = dir
            .path()
            .join(legacy_conversation_file_name("project:review"));
        let snapshot = PersistedFlowConversation {
            flow_id: "project:review".into(),
            flow_path: String::new(),
            saved_at: "2026-07-10T00:00:00Z".into(),
            version: 0,
            turns: vec![PersistedFlowTurn {
                user: "old question".into(),
                assistant: "old answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                error: None,
                failure: None,
            }],
        };
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();

        let adopted = load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/work/alpha/flows/review.md",
        )
        .expect("legacy snapshot adopted");
        assert_eq!(adopted.turns[0].user, "old question");
        assert_eq!(adopted.flow_path, "/work/alpha/flows/review.md");
        assert!(!legacy.exists(), "legacy file must be consumed");
        assert!(
            load_persisted_conversation_from(
                dir.path(),
                "project:review",
                "/work/beta/flows/review.md",
            )
            .is_none(),
            "a second project must not inherit the adopted transcript"
        );
    }

    /// Terminate promises "permanently end this conversation" — deletion
    /// must actually remove the persisted history (2026-07-11 audit P0).
    #[test]
    fn delete_erases_persisted_history_including_legacy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = vec![SessionTurn {
            user: "q".into(),
            assistant: "a".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        persist_conversation_to(dir.path(), "project:review", "/w/flows/review.md", &turns)
            .expect("persist");
        let legacy = dir
            .path()
            .join(legacy_conversation_file_name("project:review"));
        std::fs::write(&legacy, b"{}").unwrap();

        let _ =
            delete_persisted_conversation_from(dir.path(), "project:review", "/w/flows/review.md");
        assert!(load_persisted_conversation_from(
            dir.path(),
            "project:review",
            "/w/flows/review.md"
        )
        .is_none());
        assert!(!legacy.exists());
    }

    #[test]
    fn later_turns_roll_up_history_then_message() {
        let turns = vec![SessionTurn {
            user: "find bun shell examples".into(),
            assistant: "Here are three repos …".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        let task = build_turn_task(&turns, "show me the second one");
        assert!(task.starts_with("Conversation so far"));
        assert!(task.contains("User: find bun shell examples"));
        assert!(task.contains("Assistant: Here are three repos …"));
        assert!(task.ends_with("show me the second one"));
    }

    #[test]
    fn history_budget_drops_oldest_turns_first() {
        let big = "x".repeat(6_000);
        let turns = vec![
            SessionTurn {
                user: "oldest".into(),
                assistant: big.clone(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            },
            SessionTurn {
                user: "newest".into(),
                assistant: big,
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            },
        ];
        let task = build_turn_task(&turns, "next");
        assert!(!task.contains("oldest"));
        assert!(task.contains("newest"));
        assert!(task.ends_with("next"));
    }

    #[test]
    fn templated_flow_contract_keeps_mission_in_first_prompt() {
        let markdown =
            "---\ndescription: GitHub examples\n---\nSearch GitHub for examples.\n\n{{ _task }}\n";
        let contract = resolve_flow_thread_contract(markdown, "bun shell scripts");
        assert_eq!(contract.profile.developer_instructions, None);
        assert_eq!(
            contract.first_prompt,
            "Search GitHub for examples.\n\nbun shell scripts"
        );
    }

    #[test]
    fn plain_flow_contract_pins_mission_as_developer_instructions() {
        let markdown = "---\ndescription: Terse\n---\nYou are gmail-agent. Reply tersely.\n";
        let contract = resolve_flow_thread_contract(markdown, "what did vercel email me?");
        assert_eq!(
            contract.profile.developer_instructions.as_deref(),
            Some("You are gmail-agent. Reply tersely.")
        );
        assert_eq!(contract.first_prompt, "what did vercel email me?");
    }

    #[test]
    fn empty_body_contract_sends_task_verbatim_with_no_instructions() {
        let contract = resolve_flow_thread_contract("---\nmodel: gpt-5\n---\n", "hello");
        assert_eq!(contract.profile.developer_instructions, None);
        assert_eq!(contract.first_prompt, "hello");
    }

    #[test]
    fn frontmatter_model_and_sandbox_pass_through_with_quotes_stripped() {
        let markdown =
            "---\nmodel: \"gpt-5.6-sol\"\nsandbox: 'read-only'\nother: x\n---\nMission.\n";
        let contract = resolve_flow_thread_contract(markdown, "go");
        assert_eq!(contract.profile.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(contract.profile.sandbox.as_deref(), Some("read-only"));
        let bare = resolve_flow_thread_contract("Mission.", "go");
        assert_eq!(bare.profile.model, None);
        assert_eq!(bare.profile.sandbox, None);
    }

    #[test]
    fn rethread_contract_carries_mission_and_transcript_rollup() {
        // Engine died mid-conversation: the submit path resolves the
        // contract again with build_turn_task(turns, message) as the task,
        // so the fresh thread gets BOTH the flow's identity and the prior
        // conversation — never a generic new thread.
        let turns = vec![SessionTurn {
            user: "find bun shell examples".into(),
            assistant: "Here are three repos …".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        }];
        let rollup = build_turn_task(&turns, "show me the second one");

        let plain = resolve_flow_thread_contract("You are repo-scout.", &rollup);
        assert_eq!(
            plain.profile.developer_instructions.as_deref(),
            Some("You are repo-scout.")
        );
        assert!(plain.first_prompt.contains("User: find bun shell examples"));
        assert!(plain.first_prompt.ends_with("show me the second one"));

        let templated = resolve_flow_thread_contract("Scout repos.\n\n{{ _task }}", &rollup);
        assert!(templated.first_prompt.starts_with("Scout repos."));
        assert!(templated
            .first_prompt
            .contains("Assistant: Here are three repos …"));
    }

    fn active_turn(assistant_acc: &str) -> ActiveTurn {
        ActiveTurn {
            run_id: None,
            message_id: "m".into(),
            assistant_acc: assistant_acc.into(),
            current_item_id: None,
            item_acc: String::new(),
            user_text: "u".into(),
        }
    }

    #[test]
    fn entering_a_new_item_after_text_needs_a_paragraph_break() {
        let mut turn = active_turn("First item ends with a period.");
        turn.current_item_id = Some("item-1".into());
        turn.item_acc = "First item ends with a period.".into();
        assert!(turn.enter_item("item-2"));
        assert_eq!(turn.item_acc, "");
        assert_eq!(turn.current_item_id.as_deref(), Some("item-2"));
    }

    #[test]
    fn same_item_and_first_item_never_break() {
        let mut turn = active_turn("");
        assert!(
            !turn.enter_item("item-1"),
            "first item: nothing to separate"
        );
        turn.item_acc = "streaming".into();
        assert!(!turn.enter_item("item-1"), "same item: no-op");
        assert_eq!(
            turn.item_acc, "streaming",
            "same item must keep its accumulator"
        );
    }

    #[test]
    fn existing_paragraph_break_is_not_doubled() {
        let mut turn = active_turn("First item.\n\n");
        turn.current_item_id = Some("item-1".into());
        assert!(!turn.enter_item("item-2"));
    }

    /// S09: the reducer-driven session reliability state makes a failed turn
    /// actionable (AwaitingRecovery with a Retry path), keeps a user Stop
    /// quiet (no recovery card), and makes an idle engine death actionable
    /// through the outside-turn projection.
    #[test]
    fn flow_reliability_failure_is_actionable_and_stop_stays_quiet() {
        use sk_protocol::ai_reliability::{
            AiFailure, AiFailureKind, AiPhaseTag, RetrySafety, RuntimeFailure,
        };
        let failure = || {
            AiFailure::new(
                AiFailureKind::Runtime(RuntimeFailure::RuntimeClosed),
                RetrySafety::SameSelectionReadOnly,
            )
        };

        // Failed turn → actionable recovery → manual Retry reaches Running.
        let mut failed = FlowReliability::new("project:test", "/w/flows/test.md", "codex");
        failed.begin_turn("project:test", "/w/flows/test.md", 0);
        assert_eq!(failed.state().phase.tag(), AiPhaseTag::Running);
        failed.fail_turn(failure());
        assert!(failed.awaiting_recovery(), "failure must become actionable");
        assert!(
            failed.retry_turn("project:test", 0),
            "manual retry must be accepted"
        );
        assert_eq!(failed.state().phase.tag(), AiPhaseTag::Running);

        // User stop → truthful cancellation, never the recovery treatment.
        let mut stopped = FlowReliability::new("project:test", "/w/flows/test.md", "codex");
        stopped.begin_turn("project:test", "/w/flows/test.md", 0);
        stopped.cancel_turn(true);
        assert_eq!(stopped.state().phase.tag(), AiPhaseTag::Cancelled);
        assert!(!stopped.awaiting_recovery(), "stop must stay quiet");

        // Engine death while idle → actionable without fabricating a turn.
        let mut idle = FlowReliability::new("project:test", "/w/flows/test.md", "codex");
        idle.fail_outside_turn(failure());
        assert!(idle.awaiting_recovery());

        // Rethread selection acknowledges through the reducer.
        assert!(idle.select_rethread(), "rethread must be selectable");
    }

    /// S09: persisted failures round-trip codes through `to_failure`, so
    /// restore-time recovery planning stays code-accurate.
    #[test]
    fn persisted_failure_round_trips_code_through_to_failure() {
        use sk_protocol::ai_reliability::AiFailureCode;
        for legacy in [
            "mdflow CLI not found on PATH (npm i -g mdflow)",
            "protocol violation: junk",
            "totally novel",
        ] {
            let persisted = PersistedAiFailure::from_legacy_error(legacy);
            assert_eq!(persisted.to_failure().code, persisted.code, "{legacy}");
        }
        assert_eq!(
            PersistedAiFailure::unknown_default().to_failure().code,
            AiFailureCode::Unknown
        );
    }

    #[test]
    fn state_labels_are_honest() {
        assert_eq!(SessionState::Working.label(), "working");
        assert_eq!(SessionState::NeedsYou.label(), "needs you");
        assert_eq!(SessionState::Done(Some(0)).label(), "done");
        assert_eq!(SessionState::Done(Some(1)).label(), "failed");
        assert!(SessionState::Working.is_live());
        assert!(SessionState::NeedsYou.is_live());
        assert!(!SessionState::Done(None).is_live());
    }
}
