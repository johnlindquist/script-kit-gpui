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
            AiFailureCode::ContextUnavailable => {
                AiFailureKind::Input(InputFailure::ContextUnavailable)
            }
            AiFailureCode::DestinationUnavailable => {
                AiFailureKind::Input(InputFailure::DestinationUnavailable)
            }
            AiFailureCode::DestinationStale => AiFailureKind::Input(InputFailure::DestinationStale),
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

/// Why a flow conversation is being re-threaded.
///
/// Both entry points put the session on a FRESH protocol thread. They differ
/// on exactly one thing — whether the existing transcript survives — and that
/// difference is the whole user-visible contract, so it is a type rather than
/// a bare `bool` at two call sites.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowConversationResetCause {
    /// A failure recovery ("Start a new thread"). The engine died; the
    /// CONVERSATION did not. The transcript survives and is rolled up into the
    /// new thread's first prompt.
    Recovery,
    /// The user asked for a new conversation. The transcript is discarded and
    /// the next turn starts from an empty history.
    UserRequested,
}

impl FlowConversationResetCause {
    /// Whether the existing transcript survives into the new thread.
    ///
    /// This one predicate drives every downstream difference: clearing the
    /// rendered turns, clearing `meta.turns` (which is also what makes the
    /// submit path treat the next turn as a first turn rather than a rollup),
    /// and replacing the persisted snapshot.
    pub fn preserves_transcript(self) -> bool {
        matches!(self, Self::Recovery)
    }
}

/// Whether a fresh-conversation request may proceed right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowConversationResetGuard {
    Allowed,
    /// A turn is in flight. Resetting would orphan a running engine turn that
    /// keeps spending, and the user would have no way back to it.
    BlockedByActiveTurn,
}

/// Decide whether a conversation reset may run.
///
/// Pure so both the ⌘K path and the ⌘L path can be held to the same rule
/// without an app. Recovery is included deliberately: a recovery selection is
/// only reachable from a settled failure, so if one ever arrives with a turn
/// in flight that is a bug, and silently rethreading underneath it would hide
/// the bug behind a lost turn.
pub fn resolve_flow_conversation_reset_guard(
    _cause: FlowConversationResetCause,
    has_active_turn: bool,
) -> FlowConversationResetGuard {
    if has_active_turn {
        FlowConversationResetGuard::BlockedByActiveTurn
    } else {
        FlowConversationResetGuard::Allowed
    }
}

/// Pick the text a "Copy Last Response" invocation should put on the clipboard.
///
/// Newest-first, skipping blanks. The in-flight turn is appended to the
/// transcript with an empty `assistant` the moment it is submitted, so
/// `turns.last()` would hand back `""` mid-stream — and a clipboard write of
/// `""` succeeds, so the user gets a silent success and pastes nothing. Every
/// caller must therefore distinguish "no answer yet" from "an answer", which is
/// what returning `None` forces.
///
/// Trimming is deliberate: a turn holding only whitespace is not an answer.
pub fn resolve_last_copyable_response<'a>(
    assistant_texts: impl DoubleEndedIterator<Item = &'a str>,
) -> Option<&'a str> {
    assistant_texts
        .rev()
        .find(|assistant| !assistant.trim().is_empty())
}

/// Which transcript the Flow session is presenting. The active transcript is
/// the only writable one; archives are immutable until explicitly continued
/// into a new active thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowTranscriptSelection {
    Active,
    Archived(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowModelSource {
    Definition,
    Runtime,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowOriginKind {
    Project,
    Package,
    Global,
    BuiltIn,
    Unknown,
}

impl FlowOriginKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Package => "Package",
            Self::Global => "Global",
            Self::BuiltIn => "Built-in",
            Self::Unknown => "Unknown",
        }
    }
}

/// One archived conversation retained alongside the active thread.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowArchivedThread {
    pub id: String,
    pub parent_thread_id: Option<String>,
    pub created_at: String,
    pub archived_at: String,
    /// Number of leading turns inherited when this thread was continued from
    /// an archive. Preserve it when a continued active thread is archived
    /// again so lineage never silently resets to zero.
    pub inherited_turn_count: usize,
    pub turns: Vec<SessionTurn>,
}

/// Metadata for one conversation, independent of the GPUI entity.
#[derive(Debug, Clone)]
pub struct FlowSessionMeta {
    pub id: u64,
    pub flow_id: String,
    pub flow_name: String,
    pub friendly_name: String,
    pub origin: String,
    pub origin_kind: FlowOriginKind,
    pub engine: String,
    pub model: Option<String>,
    pub model_source: FlowModelSource,
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
    /// Last SEMANTIC user-relevant activity: creation, explicit resume/open,
    /// turn submit, or a turn reaching a terminal state. Deliberately NOT
    /// updated per streamed token — token-driven updates would reorder the
    /// Active Flows list continuously and make keyboard selection unstable.
    ///
    /// `SystemTime`, not `Instant`, so a future persisted-session identity
    /// (spec G2) can serialize it and agree with Agent Chat's `updated_at`.
    pub last_activity: std::time::SystemTime,
    /// Stable identity of the writable active conversation.
    pub active_thread_id: String,
    pub active_thread_created_at: String,
    pub active_parent_thread_id: Option<String>,
    /// Committed turns (user + final assistant text) for the writable active
    /// conversation and context rollup.
    pub turns: Vec<SessionTurn>,
    /// Immutable archived conversations. They are never merged back into the
    /// active vector; Continue as New clones the selected archive instead.
    pub archived_threads: Vec<FlowArchivedThread>,
    /// The transcript currently rendered by the session host.
    pub transcript_selection: FlowTranscriptSelection,
    /// Number of leading active turns inherited by Continue as New. Those rows
    /// remain read-only even though later turns may be appended.
    pub inherited_turn_count: usize,
    /// Session-owned live draft. The main filter is only its visible
    /// projection while the active conversation is open.
    pub active_draft: String,
    pub draft_generation: u64,
    pub runtime_generation: u64,
    /// Monotonic persisted model revision. Selection-only UI changes do not
    /// increment it; every committed thread mutation does.
    pub persistence_revision: u64,
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
    /// Terminate Runtime was requested while work was active. The host waits
    /// for the authoritative terminal turn event before forgetting transport.
    pub pending_runtime_termination: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletedFlowThreadKind {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedFlowThread {
    pub id: String,
    pub kind: DeletedFlowThreadKind,
    pub turn_count: usize,
}

impl FlowSessionMeta {
    pub fn new_thread_id() -> String {
        format!("flow-thread-{}", uuid::Uuid::new_v4().simple())
    }

    #[cfg(test)]
    pub(crate) fn test_fixture() -> Self {
        let flow_id = "project:test-flow".to_string();
        let flow_path = "/tmp/project/flows/test-flow.md".to_string();
        let engine = "pi".to_string();
        Self {
            id: 1,
            flow_id: flow_id.clone(),
            flow_name: "test-flow".to_string(),
            friendly_name: "Test Flow".to_string(),
            origin: "project".to_string(),
            origin_kind: FlowOriginKind::Project,
            engine: engine.clone(),
            model: None,
            model_source: FlowModelSource::Unavailable,
            flow_path: flow_path.clone(),
            flow_mtime_ms: 0,
            cwd: "/tmp/project".to_string(),
            transport: SessionTransport::MdflowTurns,
            state: SessionState::NeedsYou,
            started_at: std::time::Instant::now(),
            last_activity: std::time::SystemTime::now(),
            active_thread_id: "active-a".to_string(),
            active_thread_created_at: "2026-08-04T00:00:00Z".to_string(),
            active_parent_thread_id: None,
            turns: Vec::new(),
            archived_threads: Vec::new(),
            transcript_selection: FlowTranscriptSelection::Active,
            inherited_turn_count: 0,
            active_draft: String::new(),
            draft_generation: 0,
            runtime_generation: 0,
            persistence_revision: 1,
            active_turn: None,
            thread_ready: true,
            needs_rethread: false,
            pending_runtime_termination: false,
            reliability: FlowReliability::new(&flow_id, &flow_path, &engine),
        }
    }

    pub fn selected_turns(&self) -> &[SessionTurn] {
        match &self.transcript_selection {
            FlowTranscriptSelection::Active => &self.turns,
            FlowTranscriptSelection::Archived(id) => self
                .archived_threads
                .iter()
                .find(|thread| &thread.id == id)
                .map(|thread| thread.turns.as_slice())
                .unwrap_or(&[]),
        }
    }

    pub fn selected_is_archived(&self) -> bool {
        matches!(
            self.transcript_selection,
            FlowTranscriptSelection::Archived(_)
        )
    }

    pub fn archive_active_and_start_empty(&mut self) {
        let archived_at = chrono::Utc::now().to_rfc3339();
        self.archived_threads.push(FlowArchivedThread {
            id: self.active_thread_id.clone(),
            parent_thread_id: self.active_parent_thread_id.clone(),
            created_at: self.active_thread_created_at.clone(),
            archived_at,
            inherited_turn_count: self.inherited_turn_count,
            turns: std::mem::take(&mut self.turns),
        });
        self.active_thread_id = Self::new_thread_id();
        self.active_thread_created_at = chrono::Utc::now().to_rfc3339();
        self.active_parent_thread_id = None;
        self.transcript_selection = FlowTranscriptSelection::Active;
        self.inherited_turn_count = 0;
    }

    pub fn continue_archive_as_new(&mut self, archived_id: &str) -> bool {
        let Some(source) = self
            .archived_threads
            .iter()
            .find(|thread| thread.id == archived_id)
            .cloned()
        else {
            return false;
        };
        if !self.turns.is_empty() {
            self.archive_active_and_start_empty();
        }
        self.active_thread_id = Self::new_thread_id();
        self.active_thread_created_at = chrono::Utc::now().to_rfc3339();
        self.active_parent_thread_id = Some(source.id);
        self.turns = source.turns;
        self.inherited_turn_count = self.turns.len();
        self.transcript_selection = FlowTranscriptSelection::Active;
        true
    }

    pub fn select_archive(&mut self, archived_id: &str) -> bool {
        if self
            .archived_threads
            .iter()
            .any(|thread| thread.id == archived_id)
        {
            self.transcript_selection = FlowTranscriptSelection::Archived(archived_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn select_active(&mut self) {
        self.transcript_selection = FlowTranscriptSelection::Active;
    }

    pub fn delete_selected_thread(&mut self) -> DeletedFlowThread {
        let deleted = match self.transcript_selection.clone() {
            FlowTranscriptSelection::Active => {
                let deleted = DeletedFlowThread {
                    id: self.active_thread_id.clone(),
                    kind: DeletedFlowThreadKind::Active,
                    turn_count: self.turns.len(),
                };
                self.turns.clear();
                self.active_thread_id = Self::new_thread_id();
                self.active_thread_created_at = chrono::Utc::now().to_rfc3339();
                self.active_parent_thread_id = None;
                self.inherited_turn_count = 0;
                deleted
            }
            FlowTranscriptSelection::Archived(id) => {
                let turn_count = self
                    .archived_threads
                    .iter()
                    .find(|thread| thread.id == id)
                    .map_or(0, |thread| thread.turns.len());
                self.archived_threads.retain(|thread| thread.id != id);
                DeletedFlowThread {
                    id,
                    kind: DeletedFlowThreadKind::Archived,
                    turn_count,
                }
            }
        };
        self.transcript_selection = FlowTranscriptSelection::Active;
        deleted
    }

    pub fn persisted_snapshot_at_revision(&self, revision: u64) -> PersistedFlowConversation {
        let mut threads: Vec<PersistedFlowThread> = self
            .archived_threads
            .iter()
            .map(|thread| PersistedFlowThread {
                id: thread.id.clone(),
                state: PersistedFlowThreadState::Archived,
                parent_thread_id: thread.parent_thread_id.clone(),
                created_at: thread.created_at.clone(),
                archived_at: Some(thread.archived_at.clone()),
                inherited_turn_count: thread.inherited_turn_count,
                turns: thread.turns.iter().map(PersistedFlowTurn::from).collect(),
            })
            .collect();
        threads.push(PersistedFlowThread {
            id: self.active_thread_id.clone(),
            state: PersistedFlowThreadState::Active,
            parent_thread_id: self.active_parent_thread_id.clone(),
            created_at: self.active_thread_created_at.clone(),
            archived_at: None,
            inherited_turn_count: self.inherited_turn_count,
            turns: self.turns.iter().map(PersistedFlowTurn::from).collect(),
        });
        PersistedFlowConversation {
            flow_id: self.flow_id.clone(),
            flow_path: self.flow_path.clone(),
            saved_at: chrono::Utc::now().to_rfc3339(),
            version: SNAPSHOT_VERSION,
            revision: revision.max(1),
            active_thread_id: self.active_thread_id.clone(),
            threads,
            turns: Vec::new(),
        }
    }

    pub fn persisted_snapshot(&self) -> PersistedFlowConversation {
        self.persisted_snapshot_at_revision(self.persistence_revision.max(1))
    }

    pub fn next_persisted_snapshot(&mut self) -> PersistedFlowConversation {
        self.persistence_revision = self.persistence_revision.saturating_add(1).max(1);
        self.persisted_snapshot_at_revision(self.persistence_revision)
    }

    /// Record semantic activity at an explicit time. Tests call this with a
    /// controlled clock so ordering is provable without sleeping.
    pub fn touch_at(&mut self, at: std::time::SystemTime) {
        self.last_activity = at;
    }

    /// Record semantic activity now. Production call sites: session creation,
    /// explicit open/resume, turn submit, terminal turn transition.
    pub fn touch_now(&mut self) {
        self.touch_at(std::time::SystemTime::now());
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSessionIdentitySnapshot {
    pub session_id: u64,
    pub flow_id: String,
    pub friendly_name: String,
    pub engine: String,
    pub model: Option<String>,
    pub model_source: FlowModelSource,
    pub origin_kind: FlowOriginKind,
    pub origin_label: &'static str,
    pub cwd_display: String,
    pub cwd_fingerprint: String,
    pub selection: &'static str,
    pub read_only: bool,
    pub active_thread_fingerprint: String,
    pub selected_thread_fingerprint: String,
    pub parent_thread_fingerprint: Option<String>,
    pub parent_retained: Option<bool>,
    pub inherited_turn_count: usize,
    pub active_turn_count: usize,
    pub selected_turn_count: usize,
    pub archive_count: usize,
    pub thread_count: usize,
    pub total_turn_count: usize,
    pub needs_rethread: bool,
    pub thread_ready: bool,
    pub runtime_generation: u64,
    pub draft_chars: usize,
    pub draft_fingerprint: Option<String>,
    pub draft_generation: u64,
    pub persistence_revision: u64,
}

pub fn safe_cwd_display(cwd: &str) -> String {
    let path = std::path::Path::new(cwd);
    if std::env::var_os("HOME")
        .as_deref()
        .is_some_and(|home| path == std::path::Path::new(home))
    {
        return "~".to_string();
    }
    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    if components.is_empty() {
        "Working directory".to_string()
    } else {
        components[components.len().saturating_sub(2)..].join("/")
    }
}

impl FlowSessionIdentitySnapshot {
    pub fn from_meta(meta: &FlowSessionMeta) -> Self {
        let fingerprint = |value: &str| crate::ai::reliability::redacted_fingerprint(value);
        let selected_archive = match &meta.transcript_selection {
            FlowTranscriptSelection::Active => None,
            FlowTranscriptSelection::Archived(id) => {
                meta.archived_threads.iter().find(|thread| &thread.id == id)
            }
        };
        let selected_thread_id = selected_archive
            .map(|thread| thread.id.as_str())
            .unwrap_or(meta.active_thread_id.as_str());
        let parent_thread_id = selected_archive
            .and_then(|thread| thread.parent_thread_id.as_deref())
            .or(meta.active_parent_thread_id.as_deref());
        let parent_retained = parent_thread_id.map(|parent| {
            parent == meta.active_thread_id
                || meta
                    .archived_threads
                    .iter()
                    .any(|thread| thread.id == parent)
        });
        let selected_turn_count = meta.selected_turns().len();
        let total_turn_count = meta.turns.len()
            + meta
                .archived_threads
                .iter()
                .map(|thread| thread.turns.len())
                .sum::<usize>();
        Self {
            session_id: meta.id,
            flow_id: meta.flow_id.clone(),
            friendly_name: meta.friendly_name.clone(),
            engine: meta.engine.clone(),
            model: meta.model.clone(),
            model_source: meta.model_source,
            origin_kind: meta.origin_kind,
            origin_label: meta.origin_kind.label(),
            cwd_display: safe_cwd_display(&meta.cwd),
            cwd_fingerprint: fingerprint(&meta.cwd),
            selection: if meta.selected_is_archived() {
                "archive"
            } else {
                "active"
            },
            read_only: meta.selected_is_archived(),
            active_thread_fingerprint: fingerprint(&meta.active_thread_id),
            selected_thread_fingerprint: fingerprint(selected_thread_id),
            parent_thread_fingerprint: parent_thread_id.map(fingerprint),
            parent_retained,
            inherited_turn_count: selected_archive
                .map(|thread| thread.inherited_turn_count)
                .unwrap_or(meta.inherited_turn_count),
            active_turn_count: meta.turns.len(),
            selected_turn_count,
            archive_count: meta.archived_threads.len(),
            thread_count: meta.archived_threads.len() + 1,
            total_turn_count,
            needs_rethread: meta.needs_rethread,
            thread_ready: meta.thread_ready,
            runtime_generation: meta.runtime_generation,
            draft_chars: meta.active_draft.chars().count(),
            draft_fingerprint: (!meta.active_draft.is_empty())
                .then(|| fingerprint(&meta.active_draft)),
            draft_generation: meta.draft_generation,
            persistence_revision: meta.persistence_revision,
        }
    }

    pub fn retention_text(&self) -> String {
        format!(
            "No Script Kit turn cap · {} turns retained across {} threads",
            self.total_turn_count, self.thread_count
        )
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
    resolve_mdflow_turn_arg_with_runner(
        binary,
        flow_path,
        cwd,
        prompt,
        deadline,
        run_md_explain_with_deadline,
    )
}

fn resolve_mdflow_turn_arg_with_runner<F>(
    binary: &str,
    flow_path: &str,
    cwd: &str,
    prompt: &str,
    deadline: Instant,
    mut run_explain: F,
) -> Result<MdflowTurnArg, String>
where
    F: FnMut(&str, &str, &str, &[&str], Instant) -> std::io::Result<MdExplainOutput>,
{
    let resolves_all_template_vars = |result: &MdExplainOutput| {
        result.output.status.success()
            && result
                .explain
                .as_ref()
                .is_some_and(|explain| explain.missing_template_vars.is_empty())
    };

    let positional_args = [prompt];
    let positional = run_explain(binary, flow_path, cwd, &positional_args, deadline)
        .map_err(|err| format!("mdflow explain failed for positional input: {err}"))?;
    if resolves_all_template_vars(&positional) {
        return Ok(MdflowTurnArg::Positional(prompt.to_string()));
    }

    let named_args = ["--_task", prompt];
    let named = run_explain(binary, flow_path, cwd, &named_args, deadline)
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

include!("session_persistence.rs");
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
include!("session_tests.rs");
