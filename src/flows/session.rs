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
        .map(|assistant| assistant.trim())
        .find(|assistant| !assistant.is_empty())
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Monotonic model revision. Version-4 writers start at one; the store uses
    /// it with per-thread tombstones to reject stale asynchronous snapshots.
    #[serde(default)]
    pub revision: u64,
    /// Version-4 active thread identity. Empty on legacy v0-v3 snapshots.
    #[serde(default)]
    pub active_thread_id: String,
    /// Version-4 thread manifest. Legacy snapshots migrate their `turns` into
    /// one active thread without dropping any rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threads: Vec<PersistedFlowThread>,
    /// Legacy v0-v3 turn vector. Read-only compatibility; v4 never writes it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<PersistedFlowTurn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PersistedFlowThreadState {
    Active,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistedFlowThread {
    pub id: String,
    pub state: PersistedFlowThreadState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    #[serde(default)]
    pub inherited_turn_count: usize,
    pub turns: Vec<PersistedFlowTurn>,
}

/// Current snapshot format: a v4 active-thread manifest plus immutable
/// archives. Turns retain raw assistant text, structured outcome, and typed
/// persisted failures.
pub const SNAPSHOT_VERSION: u32 = 4;

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
    let persisted_turns = if snapshot.version >= 4 {
        snapshot
            .threads
            .iter()
            .find(|thread| {
                thread.id == snapshot.active_thread_id
                    && thread.state == PersistedFlowThreadState::Active
            })
            .map(|thread| thread.turns.as_slice())
            .unwrap_or(&[])
    } else {
        snapshot.turns.as_slice()
    };
    canonical_persisted_turns(snapshot.version, persisted_turns)
}

pub fn canonical_persisted_turns(
    version: u32,
    persisted_turns: &[PersistedFlowTurn],
) -> Vec<SessionTurn> {
    const CAPTION_DERIVED_VERSION: u32 = 2;
    persisted_turns
        .iter()
        .map(|turn| {
            let mut assistant = turn.assistant.clone();
            if version < CAPTION_DERIVED_VERSION && turn.outcome == PersistedTurnOutcome::Stopped {
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl From<&SessionTurn> for PersistedFlowTurn {
    fn from(turn: &SessionTurn) -> Self {
        Self {
            user: turn.user.clone(),
            assistant: turn.assistant.clone(),
            outcome: turn.outcome,
            error: None,
            failure: turn.failure.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PersistedTurnOutcome {
    #[default]
    Ok,
    Stopped,
    Failed,
}

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

fn migrated_thread_id(flow_id: &str, flow_path: &str) -> String {
    format!(
        "flow-thread-migrated-{}",
        crate::ai::reliability::redacted_fingerprint(&format!("{flow_id}:{flow_path}"))
    )
}

fn snapshot_from_turns(
    flow_id: &str,
    flow_path: &str,
    turns: &[SessionTurn],
) -> PersistedFlowConversation {
    let thread_id = migrated_thread_id(flow_id, flow_path);
    let now = chrono::Utc::now().to_rfc3339();
    PersistedFlowConversation {
        flow_id: flow_id.to_string(),
        flow_path: flow_path.to_string(),
        saved_at: now.clone(),
        version: SNAPSHOT_VERSION,
        revision: 1,
        active_thread_id: thread_id.clone(),
        threads: vec![PersistedFlowThread {
            id: thread_id,
            state: PersistedFlowThreadState::Active,
            parent_thread_id: None,
            created_at: now,
            archived_at: None,
            inherited_turn_count: 0,
            turns: turns.iter().map(PersistedFlowTurn::from).collect(),
        }],
        turns: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistedConversationLoadError {
    FutureVersion(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalizedConversation {
    pub snapshot: PersistedFlowConversation,
    pub changed: bool,
}

fn canonical_timestamp(value: &str, fallback: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| {
            timestamp
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
        })
        .unwrap_or_else(|_| fallback.to_string())
}

fn recovered_thread_id(flow_id: &str, flow_path: &str, index: usize, raw_id: &str) -> String {
    format!(
        "flow-thread-recovered-{}",
        crate::ai::reliability::redacted_fingerprint(&format!(
            "{flow_id}:{flow_path}:{index}:{raw_id}"
        ))
    )
}

fn remove_retained_parent_cycles(threads: &mut [PersistedFlowThread]) {
    let parent_by_id: std::collections::HashMap<String, Option<String>> = threads
        .iter()
        .map(|thread| (thread.id.clone(), thread.parent_thread_id.clone()))
        .collect();
    for thread in threads.iter_mut() {
        let start = thread.id.clone();
        let mut cursor = thread.parent_thread_id.clone();
        let mut visited = std::collections::HashSet::new();
        while let Some(parent) = cursor {
            if parent == start || !visited.insert(parent.clone()) {
                thread.parent_thread_id = None;
                break;
            }
            cursor = parent_by_id.get(&parent).cloned().flatten();
        }
    }
}

/// Normalize any persisted conversation into the single canonical v4 shape.
/// The caller captures `now` once so repairs never depend on repeated clock
/// reads. Future versions fail closed and are never rewritten.
pub fn canonicalize_persisted_conversation(
    raw: PersistedFlowConversation,
    expected_flow_id: &str,
    expected_flow_path: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<CanonicalizedConversation, PersistedConversationLoadError> {
    if raw.version > SNAPSHOT_VERSION {
        return Err(PersistedConversationLoadError::FutureVersion(raw.version));
    }

    let original = raw.clone();
    let now = now.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
    let saved_at = canonical_timestamp(&raw.saved_at, &now);

    if raw.version < SNAPSHOT_VERSION {
        let turns = canonical_persisted_turns(raw.version, &raw.turns);
        let thread_id = migrated_thread_id(expected_flow_id, expected_flow_path);
        let snapshot = PersistedFlowConversation {
            flow_id: expected_flow_id.to_string(),
            flow_path: expected_flow_path.to_string(),
            saved_at: saved_at.clone(),
            version: SNAPSHOT_VERSION,
            revision: 1,
            active_thread_id: thread_id.clone(),
            threads: vec![PersistedFlowThread {
                id: thread_id,
                state: PersistedFlowThreadState::Active,
                parent_thread_id: None,
                created_at: saved_at,
                archived_at: None,
                inherited_turn_count: 0,
                turns: turns.iter().map(PersistedFlowTurn::from).collect(),
            }],
            turns: Vec::new(),
        };
        return Ok(CanonicalizedConversation {
            changed: snapshot != original,
            snapshot,
        });
    }

    let raw_threads = raw.threads;
    let active_index = if !raw.active_thread_id.is_empty() {
        let matches: Vec<usize> = raw_threads
            .iter()
            .enumerate()
            .filter_map(|(index, thread)| (thread.id == raw.active_thread_id).then_some(index))
            .collect();
        (matches.len() == 1).then_some(matches[0])
    } else {
        None
    }
    .or_else(|| {
        let active: Vec<usize> = raw_threads
            .iter()
            .enumerate()
            .filter_map(|(index, thread)| {
                (thread.state == PersistedFlowThreadState::Active).then_some(index)
            })
            .collect();
        match active.as_slice() {
            [] => None,
            [only] => Some(*only),
            many => many.last().copied(),
        }
    })
    .or_else(|| raw_threads.len().checked_sub(1));

    let mut id_counts = std::collections::HashMap::<String, usize>::new();
    for thread in &raw_threads {
        *id_counts.entry(thread.id.clone()).or_default() += 1;
    }
    let mut used_ids = std::collections::HashSet::new();
    let canonical_ids: Vec<String> = raw_threads
        .iter()
        .enumerate()
        .map(|(index, thread)| {
            if !thread.id.is_empty() && used_ids.insert(thread.id.clone()) {
                thread.id.clone()
            } else {
                let mut recovered =
                    recovered_thread_id(expected_flow_id, expected_flow_path, index, &thread.id);
                while !used_ids.insert(recovered.clone()) {
                    recovered.push('x');
                }
                recovered
            }
        })
        .collect();

    let mut threads = Vec::with_capacity(raw_threads.len().max(1));
    for (index, thread) in raw_threads.into_iter().enumerate() {
        let is_active = Some(index) == active_index;
        let created_at = canonical_timestamp(&thread.created_at, &saved_at);
        let id = canonical_ids[index].clone();
        let parent_thread_id = thread.parent_thread_id.and_then(|parent| {
            if parent == thread.id || parent == id {
                return None;
            }
            match id_counts.get(&parent).copied() {
                Some(1) => original
                    .threads
                    .iter()
                    .position(|candidate| candidate.id == parent)
                    .map(|parent_index| canonical_ids[parent_index].clone()),
                Some(_) => None,
                None if parent.is_empty() => None,
                None => Some(parent),
            }
        });
        let archived_at = if is_active {
            None
        } else {
            let candidate = thread
                .archived_at
                .as_deref()
                .map(|value| canonical_timestamp(value, &saved_at))
                .unwrap_or_else(|| saved_at.clone());
            let created = chrono::DateTime::parse_from_rfc3339(&created_at).ok();
            let archived = chrono::DateTime::parse_from_rfc3339(&candidate).ok();
            Some(
                if created
                    .zip(archived)
                    .is_some_and(|(created, archived)| archived >= created)
                {
                    candidate
                } else {
                    created_at.clone()
                },
            )
        };
        let turns = canonical_persisted_turns(SNAPSHOT_VERSION, &thread.turns);
        threads.push(PersistedFlowThread {
            id,
            state: if is_active {
                PersistedFlowThreadState::Active
            } else {
                PersistedFlowThreadState::Archived
            },
            parent_thread_id,
            created_at,
            archived_at,
            inherited_turn_count: thread.inherited_turn_count.min(turns.len()),
            turns: turns.iter().map(PersistedFlowTurn::from).collect(),
        });
    }

    if threads.is_empty() {
        let thread_id = migrated_thread_id(expected_flow_id, expected_flow_path);
        let turns = canonical_persisted_turns(SNAPSHOT_VERSION, &raw.turns);
        threads.push(PersistedFlowThread {
            id: thread_id,
            state: PersistedFlowThreadState::Active,
            parent_thread_id: None,
            created_at: saved_at.clone(),
            archived_at: None,
            inherited_turn_count: 0,
            turns: turns.iter().map(PersistedFlowTurn::from).collect(),
        });
    } else if let Some(active_position) = threads
        .iter()
        .position(|thread| thread.state == PersistedFlowThreadState::Active)
    {
        let active = threads.remove(active_position);
        threads.push(active);
    }

    remove_retained_parent_cycles(&mut threads);
    let active_thread_id = threads
        .last()
        .expect("canonical v4 always has an active thread")
        .id
        .clone();
    let snapshot = PersistedFlowConversation {
        flow_id: expected_flow_id.to_string(),
        flow_path: expected_flow_path.to_string(),
        saved_at,
        version: SNAPSHOT_VERSION,
        revision: raw.revision.max(1),
        active_thread_id,
        threads,
        turns: Vec::new(),
    };
    Ok(CanonicalizedConversation {
        changed: snapshot != original,
        snapshot,
    })
}

pub fn persist_conversation_snapshot_to(
    dir: &std::path::Path,
    snapshot: &PersistedFlowConversation,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(conversation_file_name(
        &snapshot.flow_id,
        &snapshot.flow_path,
    ));
    let bytes = serde_json::to_vec_pretty(snapshot).map_err(std::io::Error::other)?;
    crate::atomic_file::write_atomic(&path, &bytes)
}

pub fn persist_conversation_to(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
    turns: &[SessionTurn],
) -> std::io::Result<()> {
    persist_conversation_snapshot_to(dir, &snapshot_from_turns(flow_id, flow_path, turns))
}

pub fn load_persisted_conversation_from(
    dir: &std::path::Path,
    flow_id: &str,
    flow_path: &str,
) -> Option<PersistedFlowConversation> {
    let path = dir.join(conversation_file_name(flow_id, flow_path));
    if let Ok(raw) = std::fs::read_to_string(&path) {
        let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
        let canonical =
            canonicalize_persisted_conversation(snapshot, flow_id, flow_path, chrono::Utc::now())
                .ok()?;
        if canonical.changed {
            let _ = persist_conversation_snapshot_to(dir, &canonical.snapshot);
        }
        return Some(canonical.snapshot);
    }
    // Legacy adoption (one-shot): a pre-identity snapshot keyed by id alone
    // is claimed by the FIRST flow that opens it, re-persisted under the
    // path-qualified name, and the legacy file removed — so it can never
    // silently leak into another project again.
    let legacy = dir.join(legacy_conversation_file_name(flow_id));
    let raw = std::fs::read_to_string(&legacy).ok()?;
    let snapshot: PersistedFlowConversation = serde_json::from_str(&raw).ok()?;
    if !snapshot.flow_path.is_empty() && snapshot.flow_path != flow_path {
        return None;
    }
    if snapshot.version < SNAPSHOT_VERSION && snapshot.turns.is_empty() {
        let _ = std::fs::remove_file(&legacy);
        return None;
    }
    let snapshot =
        canonicalize_persisted_conversation(snapshot, flow_id, flow_path, chrono::Utc::now())
            .ok()?
            .snapshot;
    if persist_conversation_snapshot_to(dir, &snapshot).is_ok() {
        let _ = std::fs::remove_file(&legacy);
    }
    Some(snapshot)
}

/// One FIFO worker owns every conversation-store mutation (Oracle 2026-07-21
/// WP-A1): per-turn detached threads let an older snapshot finish AFTER a
/// newer one (silent transcript regression) and let a pending persist
/// resurrect a terminated conversation. Commands are enqueued synchronously
/// from the UI thread, so on-disk order always matches user-visible order.
pub struct FlowConversationStore {
    tx: std::sync::mpsc::Sender<ConversationStoreCommand>,
    helper_revision: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationStoreReceipt {
    Written,
    IgnoredStaleRevision,
    IgnoredTombstonedThread,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationStoreError {
    ChannelClosed,
    Timeout,
    WriteFailed,
}

type ConversationStoreAck =
    std::sync::mpsc::Sender<Result<ConversationStoreReceipt, ConversationStoreError>>;

enum ConversationStoreCommand {
    Persist {
        snapshot: PersistedFlowConversation,
        ack: Option<ConversationStoreAck>,
    },
    PersistSelectedDeletion {
        snapshot: PersistedFlowConversation,
        deleted_thread_id: String,
        ack: Option<ConversationStoreAck>,
    },
    Flush(std::sync::mpsc::Sender<Result<(), ConversationStoreError>>),
}

#[derive(Default)]
struct ConversationStoreKeyState {
    highest_revision: u64,
    tombstoned_thread_ids: std::collections::HashSet<String>,
}

fn conversation_store_key(snapshot: &PersistedFlowConversation) -> (String, String) {
    (snapshot.flow_id.clone(), snapshot.flow_path.clone())
}

fn initial_conversation_store_state(
    dir: &std::path::Path,
    snapshot: &PersistedFlowConversation,
) -> ConversationStoreKeyState {
    let highest_revision =
        load_persisted_conversation_from(dir, &snapshot.flow_id, &snapshot.flow_path)
            .map_or(0, |persisted| persisted.revision);
    ConversationStoreKeyState {
        highest_revision,
        tombstoned_thread_ids: std::collections::HashSet::new(),
    }
}

/// Debug-only runtime seam for the stale-write negative control. When the app
/// is launched with SCRIPT_KIT_TEST_STATUS=1 and the named marker exists, the
/// FIFO worker publishes `<marker>.held` and pauses the next ordinary Persist
/// until the marker is removed. Selected deletion remains queued behind it,
/// proving that release cannot resurrect the tombstoned thread.
fn wait_for_flow_persist_test_release() {
    if std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() != Some("1") {
        return;
    }
    let Some(marker) = std::env::var_os("SCRIPT_KIT_TEST_HOLD_FLOW_PERSIST_MARKER") else {
        return;
    };
    let marker = std::path::PathBuf::from(marker);
    if !marker.exists() {
        return;
    }
    let held = std::path::PathBuf::from(format!("{}.held", marker.to_string_lossy()));
    let _ = std::fs::write(&held, b"held");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while marker.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let _ = std::fs::remove_file(held);
}

fn persist_snapshot_in_worker(
    dir: &std::path::Path,
    states: &mut std::collections::HashMap<(String, String), ConversationStoreKeyState>,
    snapshot: PersistedFlowConversation,
    deleted_thread_id: Option<String>,
) -> Result<ConversationStoreReceipt, ConversationStoreError> {
    let key = conversation_store_key(&snapshot);
    let state = states
        .entry(key)
        .or_insert_with(|| initial_conversation_store_state(dir, &snapshot));
    if snapshot.revision <= state.highest_revision {
        return Ok(ConversationStoreReceipt::IgnoredStaleRevision);
    }
    if snapshot
        .threads
        .iter()
        .any(|thread| state.tombstoned_thread_ids.contains(&thread.id))
    {
        return Ok(ConversationStoreReceipt::IgnoredTombstonedThread);
    }
    if deleted_thread_id
        .as_ref()
        .is_some_and(|deleted| snapshot.threads.iter().any(|thread| &thread.id == deleted))
    {
        return Ok(ConversationStoreReceipt::IgnoredTombstonedThread);
    }
    if persist_conversation_snapshot_to(dir, &snapshot).is_err() {
        return Err(ConversationStoreError::WriteFailed);
    }
    state.highest_revision = snapshot.revision;
    if let Some(deleted_thread_id) = deleted_thread_id {
        state.tombstoned_thread_ids.insert(deleted_thread_id);
    }
    Ok(ConversationStoreReceipt::Written)
}

impl FlowConversationStore {
    /// Store rooted at `dir`. Tests construct their own with a temp dir; the
    /// app uses [`conversation_store`].
    pub fn new(dir: std::path::PathBuf) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<ConversationStoreCommand>();
        let spawned = std::thread::Builder::new()
            .name("flow-conversation-store".into())
            .spawn(move || {
                let mut states = std::collections::HashMap::new();
                while let Ok(command) = rx.recv() {
                    match command {
                        ConversationStoreCommand::Persist { snapshot, ack } => {
                            wait_for_flow_persist_test_release();
                            let flow_id = snapshot.flow_id.clone();
                            let result =
                                persist_snapshot_in_worker(&dir, &mut states, snapshot, None);
                            if result.is_err() {
                                tracing::warn!(
                                    target: "script_kit::flows",
                                    event = "flow_conversation_persist_failed",
                                    flow_id = %flow_id,
                                    "Failed to persist flow conversation"
                                );
                            }
                            if let Some(ack) = ack {
                                let _ = ack.send(result);
                            }
                        }
                        ConversationStoreCommand::PersistSelectedDeletion {
                            snapshot,
                            deleted_thread_id,
                            ack,
                        } => {
                            let flow_id = snapshot.flow_id.clone();
                            let result = persist_snapshot_in_worker(
                                &dir,
                                &mut states,
                                snapshot,
                                Some(deleted_thread_id),
                            );
                            if result.is_err() {
                                tracing::warn!(
                                    target: "script_kit::flows",
                                    event = "flow_conversation_selected_delete_failed",
                                    flow_id = %flow_id,
                                    "Failed to persist selected Flow deletion"
                                );
                            }
                            if let Some(ack) = ack {
                                let _ = ack.send(result);
                            }
                        }
                        ConversationStoreCommand::Flush(done) => {
                            let _ = done.send(Ok(()));
                        }
                    }
                }
            });
        if let Err(err) = spawned {
            tracing::error!(
                target: "script_kit::flows",
                event = "flow_conversation_store_spawn_failed",
                error = %err,
                "Flow conversation store worker failed to start"
            );
        }
        Self {
            tx,
            helper_revision: std::sync::atomic::AtomicU64::new(1),
        }
    }

    #[cfg(test)]
    pub fn persist(&self, flow_id: &str, flow_path: &str, turns: Vec<SessionTurn>) {
        let mut snapshot = snapshot_from_turns(flow_id, flow_path, &turns);
        snapshot.revision = self
            .helper_revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.persist_snapshot(snapshot);
    }

    pub fn persist_snapshot(&self, snapshot: PersistedFlowConversation) {
        let _ = self.tx.send(ConversationStoreCommand::Persist {
            snapshot,
            ack: None,
        });
    }

    pub fn persist_snapshot_and_wait(
        &self,
        snapshot: PersistedFlowConversation,
    ) -> Result<ConversationStoreReceipt, ConversationStoreError> {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        self.tx
            .send(ConversationStoreCommand::Persist {
                snapshot,
                ack: Some(ack_tx),
            })
            .map_err(|_| ConversationStoreError::ChannelClosed)?;
        ack_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ConversationStoreError::Timeout)?
    }

    pub fn persist_selected_deletion(
        &self,
        snapshot: PersistedFlowConversation,
        deleted_thread_id: String,
    ) {
        let _ = self
            .tx
            .send(ConversationStoreCommand::PersistSelectedDeletion {
                snapshot,
                deleted_thread_id,
                ack: None,
            });
    }

    pub fn persist_selected_deletion_and_wait(
        &self,
        snapshot: PersistedFlowConversation,
        deleted_thread_id: String,
    ) -> Result<ConversationStoreReceipt, ConversationStoreError> {
        let (ack_tx, ack_rx) = std::sync::mpsc::channel();
        self.tx
            .send(ConversationStoreCommand::PersistSelectedDeletion {
                snapshot,
                deleted_thread_id,
                ack: Some(ack_tx),
            })
            .map_err(|_| ConversationStoreError::ChannelClosed)?;
        ack_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ConversationStoreError::Timeout)?
    }

    /// Barrier: returns once every previously enqueued command has reached
    /// disk. Used by tests and shutdown.
    pub fn flush(&self) -> Result<(), ConversationStoreError> {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        self.tx
            .send(ConversationStoreCommand::Flush(done_tx))
            .map_err(|_| ConversationStoreError::ChannelClosed)?;
        done_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| ConversationStoreError::Timeout)?
    }
}

/// The app-wide store rooted at the active workspace.
pub fn conversation_store() -> &'static FlowConversationStore {
    static STORE: std::sync::OnceLock<FlowConversationStore> = std::sync::OnceLock::new();
    STORE.get_or_init(|| FlowConversationStore::new(conversation_store_dir()))
}

/// Persist under the active workspace (`~/.scriptkit`, `SK_PATH` override).
pub fn persist_conversation(flow_id: &str, flow_path: &str, turns: &[SessionTurn]) {
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

    /// The cause is the whole difference between the two rethread entry
    /// points, so it is asserted exhaustively rather than at one call site.
    #[test]
    fn only_a_user_requested_reset_discards_the_transcript() {
        assert!(
            FlowConversationResetCause::Recovery.preserves_transcript(),
            "the engine died, the conversation did not — recovery rolls the \
             transcript into the new thread"
        );
        assert!(
            !FlowConversationResetCause::UserRequested.preserves_transcript(),
            "'New Conversation' that kept the old turns would not be a new \
             conversation"
        );
    }

    /// The defect this helper exists to prevent: the in-flight turn is already
    /// in `turns` with an empty `assistant`, so the obvious `turns.last()`
    /// copies `""` — and writing `""` to the clipboard SUCCEEDS. The user sees
    /// a copy that worked and pastes nothing.
    #[test]
    fn copying_mid_stream_reaches_past_the_empty_in_flight_turn() {
        let turns = ["first answer", "second answer", ""];
        assert_eq!(
            resolve_last_copyable_response(turns.iter().copied()),
            Some("second answer"),
            "the newest turn with an actual answer wins, not the newest turn"
        );
    }

    #[test]
    fn a_whitespace_only_turn_is_not_an_answer() {
        let turns = ["real answer", "   \n\t "];
        assert_eq!(
            resolve_last_copyable_response(turns.iter().copied()),
            Some("real answer")
        );
    }

    /// `None` and `Some("")` must not be confusable — only `None` may reach the
    /// "nothing to copy" toast, and only a non-empty string may reach the
    /// clipboard.
    #[test]
    fn a_conversation_with_no_answer_yet_copies_nothing() {
        assert_eq!(resolve_last_copyable_response([].iter().copied()), None);
        assert_eq!(
            resolve_last_copyable_response(["", "  "].iter().copied()),
            None
        );
    }

    #[test]
    fn the_copied_answer_is_trimmed() {
        assert_eq!(
            resolve_last_copyable_response(["\n  answer body  \n"].iter().copied()),
            Some("answer body")
        );
    }

    /// A reset while a turn is in flight would orphan a running engine turn:
    /// it keeps spending, and the user has no route back to it.
    #[test]
    fn a_reset_is_refused_while_a_turn_is_in_flight() {
        for cause in [
            FlowConversationResetCause::Recovery,
            FlowConversationResetCause::UserRequested,
        ] {
            assert_eq!(
                resolve_flow_conversation_reset_guard(cause, true),
                FlowConversationResetGuard::BlockedByActiveTurn,
                "{cause:?} must not reset over a live turn"
            );
            assert_eq!(
                resolve_flow_conversation_reset_guard(cause, false),
                FlowConversationResetGuard::Allowed,
                "{cause:?} is allowed on an idle session"
            );
        }
    }

    /// Empty active-thread metadata is a real persisted conversation state.
    /// New Conversation must not turn emptiness into deletion or revive the
    /// replaced transcript.
    #[test]
    fn an_empty_active_thread_persists_without_restoring_old_turns() {
        let dir = std::env::temp_dir().join(format!(
            "sk-flow-empty-snapshot-{}",
            std::process::id() as u64
        ));
        let _ = std::fs::create_dir_all(&dir);
        let flow_id = "project:reset-probe";
        let flow_path = "/tmp/reset-probe.md";

        persist_conversation_to(
            &dir,
            flow_id,
            flow_path,
            &[SessionTurn {
                user: "first question".into(),
                assistant: "first answer".into(),
                outcome: PersistedTurnOutcome::Ok,
                failure: None,
            }],
        )
        .expect("seed snapshot");
        assert!(
            load_persisted_conversation_from(&dir, flow_id, flow_path).is_some(),
            "a real transcript must load, or this test proves nothing"
        );

        persist_conversation_to(&dir, flow_id, flow_path, &[]).expect("replacement snapshot");
        let replacement = load_persisted_conversation_from(&dir, flow_id, flow_path)
            .expect("empty active metadata remains loadable");
        assert!(canonical_session_turns(&replacement).is_empty());
        assert_eq!(replacement.threads.len(), 1);
        assert_eq!(
            replacement.threads[0].state,
            PersistedFlowThreadState::Active
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

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
        use std::os::unix::process::ExitStatusExt;
        use std::time::Duration;

        fn rejected_explain() -> MdExplainOutput {
            MdExplainOutput {
                output: std::process::Output {
                    status: std::process::ExitStatus::from_raw(1 << 8),
                    stdout: Vec::new(),
                    stderr: b"positional rejected".to_vec(),
                },
                explain: None,
            }
        }

        #[test]
        fn turn_arg_resolution_uses_one_deadline_for_both_shapes() {
            for iteration in 0..20 {
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut calls: Vec<(Vec<String>, Instant)> = Vec::new();
                let result = resolve_mdflow_turn_arg_with_runner(
                    "md",
                    "flow.md",
                    "/tmp",
                    "hello",
                    deadline,
                    |_binary, _flow_path, _cwd, args, received_deadline| {
                        calls.push((
                            args.iter().map(|arg| (*arg).to_string()).collect(),
                            received_deadline,
                        ));
                        if calls.len() == 1 {
                            Ok(rejected_explain())
                        } else {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                "shared deadline exhausted",
                            ))
                        }
                    },
                );

                assert!(result.is_err(), "iteration {iteration} must fail closed");
                assert_eq!(calls.len(), 2, "both shapes are attempted in order");
                assert_eq!(calls[0].0, ["hello"]);
                assert_eq!(calls[1].0, ["--_task", "hello"]);
                assert_eq!(calls[0].1, deadline);
                assert_eq!(
                    calls[1].1, deadline,
                    "iteration {iteration} must not grant a fresh deadline"
                );
            }
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
    fn conversation_persistence_round_trips_every_turn_without_a_cap() {
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
        let restored_turns = canonical_session_turns(&restored);
        assert_eq!(restored_turns.len(), 20);
        assert_eq!(restored_turns.first().unwrap().user, "question 0");
        assert_eq!(restored_turns.last().unwrap().assistant, "answer 19");
    }

    fn thousand_turn_fixture() -> Vec<SessionTurn> {
        (0..1_000)
            .map(|index| {
                let outcome = match index % 3 {
                    0 => PersistedTurnOutcome::Ok,
                    1 => PersistedTurnOutcome::Stopped,
                    _ => PersistedTurnOutcome::Failed,
                };
                SessionTurn {
                    user: format!("question-{index:04}"),
                    assistant: format!("answer-{index:04}"),
                    outcome,
                    failure: (outcome == PersistedTurnOutcome::Failed)
                        .then(PersistedAiFailure::unknown_default),
                }
            })
            .collect()
    }

    #[test]
    fn one_thousand_turns_round_trip_without_a_cap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let turns = thousand_turn_fixture();
        persist_conversation_to(
            dir.path(),
            "project:thousand",
            "/p/flows/thousand.md",
            &turns,
        )
        .expect("persist one thousand turns");
        let restored = load_persisted_conversation_from(
            dir.path(),
            "project:thousand",
            "/p/flows/thousand.md",
        )
        .expect("restore one thousand turns");
        let restored_turns = canonical_session_turns(&restored);
        assert_eq!(restored_turns.len(), 1_000);
        for index in [0, 500, 999] {
            assert_eq!(restored_turns[index].user, turns[index].user);
            assert_eq!(restored_turns[index].assistant, turns[index].assistant);
            assert_eq!(restored_turns[index].outcome, turns[index].outcome);
            assert_eq!(restored_turns[index].failure, turns[index].failure);
        }
    }

    #[test]
    #[ignore = "architecture benchmark; run twice in fresh test processes"]
    fn benchmark_v4_manifest_1000_turns() {
        let turns = thousand_turn_fixture();
        let snapshot = snapshot_from_turns(
            "project:thousand-benchmark",
            "/p/flows/thousand-benchmark.md",
            &turns,
        );

        for _ in 0..3 {
            let bytes = serde_json::to_vec(&snapshot).expect("warm serialization");
            let _: PersistedFlowConversation =
                serde_json::from_slice(&bytes).expect("warm deserialization");
        }

        let mut serialize_ms = Vec::new();
        let mut deserialize_ms = Vec::new();
        let mut encoded_len = 0;
        for _ in 0..21 {
            let started = Instant::now();
            let bytes = serde_json::to_vec(&snapshot).expect("serialize benchmark snapshot");
            serialize_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
            encoded_len = bytes.len();

            let started = Instant::now();
            let restored: PersistedFlowConversation =
                serde_json::from_slice(&bytes).expect("deserialize benchmark snapshot");
            deserialize_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
            assert_eq!(canonical_session_turns(&restored).len(), 1_000);
        }
        serialize_ms.sort_by(f64::total_cmp);
        deserialize_ms.sort_by(f64::total_cmp);
        let serialize_median_ms = serialize_ms[serialize_ms.len() / 2];
        let deserialize_median_ms = deserialize_ms[deserialize_ms.len() / 2];
        let combined_median_ms = serialize_median_ms + deserialize_median_ms;
        println!(
            "{{\"event\":\"flowManifestBenchmark\",\"turns\":1000,\"samples\":21,\"serializeMedianMs\":{serialize_median_ms:.3},\"deserializeMedianMs\":{deserialize_median_ms:.3},\"combinedMedianMs\":{combined_median_ms:.3},\"encodedBytes\":{encoded_len}}}"
        );
    }

    #[test]
    fn new_conversation_archives_populated_and_empty_active_metadata() {
        for populated in [false, true] {
            let mut meta = FlowSessionMeta::test_fixture();
            let original_id = meta.active_thread_id.clone();
            if populated {
                meta.turns.push(turn("question", "answer"));
            }
            meta.archive_active_and_start_empty();

            assert_ne!(meta.active_thread_id, original_id);
            assert!(meta.turns.is_empty());
            assert_eq!(meta.archived_threads.len(), 1);
            assert_eq!(meta.archived_threads[0].id, original_id);
            assert_eq!(meta.archived_threads[0].turns.len(), usize::from(populated));
            assert_eq!(meta.transcript_selection, FlowTranscriptSelection::Active);
        }
    }

    #[test]
    fn continue_as_new_retains_archive_and_records_lineage() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.turns = vec![turn("source-1", "answer-1"), turn("source-2", "answer-2")];
        meta.archive_active_and_start_empty();
        let source_id = meta.archived_threads[0].id.clone();
        let source_turns = meta.archived_threads[0].turns.clone();
        assert!(meta.select_archive(&source_id));

        assert!(meta.continue_archive_as_new(&source_id));
        assert_eq!(meta.transcript_selection, FlowTranscriptSelection::Active);
        assert_eq!(
            meta.active_parent_thread_id.as_deref(),
            Some(source_id.as_str())
        );
        assert_eq!(meta.turns, source_turns);
        assert_eq!(meta.inherited_turn_count, source_turns.len());
        assert!(meta
            .archived_threads
            .iter()
            .any(|thread| thread.id == source_id));
    }

    #[test]
    fn selected_delete_removes_only_active_or_selected_archive() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.turns = vec![turn("first", "one")];
        meta.archive_active_and_start_empty();
        meta.turns = vec![turn("second", "two")];
        meta.archive_active_and_start_empty();
        meta.turns = vec![turn("current", "three")];
        let first_archive_id = meta.archived_threads[0].id.clone();
        let second_archive_id = meta.archived_threads[1].id.clone();
        let active_id = meta.active_thread_id.clone();
        let runtime_generation = meta.runtime_generation;

        assert!(meta.select_archive(&first_archive_id));
        let deleted_archive = meta.delete_selected_thread();
        assert_eq!(deleted_archive.id, first_archive_id);
        assert_eq!(deleted_archive.kind, DeletedFlowThreadKind::Archived);
        assert!(meta
            .archived_threads
            .iter()
            .any(|thread| thread.id == second_archive_id));
        assert_eq!(meta.active_thread_id, active_id);
        assert_eq!(meta.turns, vec![turn("current", "three")]);
        assert_eq!(meta.runtime_generation, runtime_generation);

        let deleted_active = meta.delete_selected_thread();
        assert_eq!(deleted_active.id, active_id);
        assert_eq!(deleted_active.kind, DeletedFlowThreadKind::Active);
        assert!(meta.turns.is_empty());
        assert_ne!(meta.active_thread_id, active_id);
        assert!(meta
            .archived_threads
            .iter()
            .any(|thread| thread.id == second_archive_id));
    }

    #[test]
    fn archive_navigation_preserves_the_hidden_active_draft() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.active_draft = "private draft canary".to_string();
        meta.turns.push(turn("question", "answer"));
        meta.archive_active_and_start_empty();
        let archive_id = meta.archived_threads[0].id.clone();
        assert!(meta.select_archive(&archive_id));
        assert_eq!(meta.active_draft, "private draft canary");
        meta.select_active();
        assert_eq!(meta.active_draft, "private draft canary");
    }

    #[test]
    fn flow_identity_snapshot_is_typed_redacted_and_lineage_aware() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.engine = "codex".to_string();
        meta.model = Some("gpt-test".to_string());
        meta.model_source = FlowModelSource::Runtime;
        meta.cwd = "/private/path-canary/project-alpha".to_string();
        meta.active_draft = "PRIVATE_DRAFT_CANARY".to_string();
        meta.draft_generation = 4;
        meta.runtime_generation = 7;
        meta.needs_rethread = true;
        meta.turns = vec![turn("active", "answer")];
        meta.archived_threads.push(FlowArchivedThread {
            id: "archive-child".to_string(),
            parent_thread_id: Some("missing-parent".to_string()),
            created_at: "2026-08-04T00:00:00Z".to_string(),
            archived_at: "2026-08-04T01:00:00Z".to_string(),
            inherited_turn_count: 1,
            turns: vec![turn("archived", "answer")],
        });
        meta.transcript_selection = FlowTranscriptSelection::Archived("archive-child".to_string());

        let identity = FlowSessionIdentitySnapshot::from_meta(&meta);
        assert_eq!(identity.engine, "codex");
        assert_eq!(identity.model.as_deref(), Some("gpt-test"));
        assert_eq!(identity.model_source, FlowModelSource::Runtime);
        assert_eq!(identity.cwd_display, "path-canary/project-alpha");
        assert!(!identity.cwd_fingerprint.contains("/private/"));
        assert_eq!(identity.selection, "archive");
        assert!(identity.read_only);
        assert_eq!(identity.parent_retained, Some(false));
        assert_eq!(identity.inherited_turn_count, 1);
        assert_eq!(identity.active_turn_count, 1);
        assert_eq!(identity.selected_turn_count, 1);
        assert_eq!(identity.total_turn_count, 2);
        assert_eq!(identity.draft_chars, "PRIVATE_DRAFT_CANARY".chars().count());
        assert!(!identity
            .draft_fingerprint
            .as_deref()
            .unwrap_or_default()
            .contains("PRIVATE_DRAFT_CANARY"));
        let debug = format!("{identity:?}");
        assert!(!debug.contains("/private/path-canary"));
        assert!(!debug.contains("PRIVATE_DRAFT_CANARY"));
    }

    #[test]
    fn flow_identity_origin_and_cwd_labels_are_closed_typed_sets() {
        let home = std::env::var("HOME").expect("HOME");
        assert_eq!(safe_cwd_display(&home), "~");
        assert_eq!(safe_cwd_display(""), "Working directory");
        assert_eq!(safe_cwd_display("/a/b/c"), "b/c");

        for (kind, label) in [
            (FlowOriginKind::Project, "Project"),
            (FlowOriginKind::Package, "Package"),
            (FlowOriginKind::Global, "Global"),
            (FlowOriginKind::BuiltIn, "Built-in"),
            (FlowOriginKind::Unknown, "Unknown"),
        ] {
            let mut meta = FlowSessionMeta::test_fixture();
            meta.origin_kind = kind;
            let identity = FlowSessionIdentitySnapshot::from_meta(&meta);
            assert_eq!(identity.origin_label, label);
        }
    }

    #[test]
    fn retention_copy_states_the_app_policy_without_promising_storage() {
        let mut meta = FlowSessionMeta::test_fixture();
        meta.turns = thousand_turn_fixture();
        let identity = FlowSessionIdentitySnapshot::from_meta(&meta);
        assert_eq!(
            identity.retention_text(),
            "No Script Kit turn cap · 1000 turns retained across 1 threads"
        );
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

    /// S09: a v4 snapshot persists ONLY the typed failure — the legacy raw
    /// caption field is never written again.
    #[test]
    fn v4_snapshots_never_write_the_legacy_error_field() {
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
        let (active_thread_id, threads, legacy_turns) = if version >= SNAPSHOT_VERSION {
            let id = "flow-thread-test".to_string();
            (
                id.clone(),
                vec![PersistedFlowThread {
                    id,
                    state: PersistedFlowThreadState::Active,
                    parent_thread_id: None,
                    created_at: "2026-07-21T00:00:00Z".into(),
                    archived_at: None,
                    inherited_turn_count: 0,
                    turns,
                }],
                Vec::new(),
            )
        } else {
            (String::new(), Vec::new(), turns)
        };
        PersistedFlowConversation {
            flow_id: "project:test".into(),
            flow_path: "/w/flows/test.md".into(),
            saved_at: "2026-07-21T00:00:00Z".into(),
            version,
            revision: u64::from(version >= SNAPSHOT_VERSION),
            active_thread_id,
            threads,
            turns: legacy_turns,
        }
    }

    fn fixed_canonical_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
            .expect("fixed timestamp")
            .with_timezone(&chrono::Utc)
    }

    fn legacy_turns_for_version(version: u32) -> Vec<PersistedFlowTurn> {
        vec![
            PersistedFlowTurn {
                user: "question one".into(),
                assistant: if version < 2 {
                    format!("partial\n\n{FLOW_STOPPED_CAPTION}")
                } else {
                    "partial".into()
                },
                outcome: PersistedTurnOutcome::Stopped,
                error: None,
                failure: None,
            },
            PersistedFlowTurn {
                user: "question two".into(),
                assistant: "answer two".into(),
                outcome: PersistedTurnOutcome::Failed,
                error: (version < 3).then(|| "protocol violation: legacy".into()),
                failure: (version >= 3).then(PersistedAiFailure::unknown_default),
            },
        ]
    }

    fn assert_legacy_migration(version: u32) {
        let raw = snapshot_with(version, legacy_turns_for_version(version));
        let canonical = canonicalize_persisted_conversation(
            raw,
            "project:test",
            "/w/flows/test.md",
            fixed_canonical_now(),
        )
        .expect("legacy version migrates")
        .snapshot;
        assert_eq!(canonical.version, SNAPSHOT_VERSION);
        assert_eq!(canonical.revision, 1);
        assert!(canonical.turns.is_empty(), "v4 never writes legacy turns");
        assert_eq!(canonical.threads.len(), 1);
        assert_eq!(canonical.active_thread_id, canonical.threads[0].id);
        assert_eq!(canonical.threads[0].state, PersistedFlowThreadState::Active);
        let turns = canonical_session_turns(&canonical);
        assert_eq!(turns.len(), 2, "migration must not lose turns");
        assert_eq!(turns[0].user, "question one");
        assert_eq!(turns[0].assistant, "partial");
        assert_eq!(turns[0].outcome, PersistedTurnOutcome::Stopped);
        assert_eq!(turns[1].user, "question two");
        assert_eq!(turns[1].assistant, "answer two");
        assert_eq!(turns[1].outcome, PersistedTurnOutcome::Failed);
        assert!(turns[1].failure.is_some());
    }

    #[test]
    fn v0_migrates_to_one_v4_active_thread_without_turn_loss() {
        assert_legacy_migration(0);
    }

    #[test]
    fn v1_migrates_to_one_v4_active_thread_without_turn_loss() {
        assert_legacy_migration(1);
    }

    #[test]
    fn v2_migrates_legacy_failures_without_turn_loss() {
        assert_legacy_migration(2);
    }

    #[test]
    fn v3_migrates_typed_failures_without_turn_loss() {
        assert_legacy_migration(3);
    }

    #[test]
    fn malformed_v4_manifest_is_canonicalized_without_turn_loss() {
        let make_turn = |label: &str| PersistedFlowTurn {
            user: format!("user-{label}"),
            assistant: format!("assistant-{label}"),
            outcome: PersistedTurnOutcome::Ok,
            error: None,
            failure: None,
        };
        let mut raw = PersistedFlowConversation {
            flow_id: "stale:id".into(),
            flow_path: "/stale/path.md".into(),
            saved_at: "not-a-time".into(),
            version: SNAPSHOT_VERSION,
            revision: 0,
            active_thread_id: "duplicate".into(),
            threads: vec![
                PersistedFlowThread {
                    id: "duplicate".into(),
                    state: PersistedFlowThreadState::Archived,
                    parent_thread_id: Some("duplicate".into()),
                    created_at: "bad".into(),
                    archived_at: None,
                    inherited_turn_count: 99,
                    turns: vec![make_turn("first")],
                },
                PersistedFlowThread {
                    id: "duplicate".into(),
                    state: PersistedFlowThreadState::Active,
                    parent_thread_id: Some("missing-parent".into()),
                    created_at: "2026-08-04T10:00:00Z".into(),
                    archived_at: Some("2026-08-04T09:00:00Z".into()),
                    inherited_turn_count: 1,
                    turns: vec![make_turn("second")],
                },
                PersistedFlowThread {
                    id: String::new(),
                    state: PersistedFlowThreadState::Active,
                    parent_thread_id: None,
                    created_at: "bad".into(),
                    archived_at: None,
                    inherited_turn_count: 0,
                    turns: vec![make_turn("third")],
                },
            ],
            turns: vec![make_turn("ignored-top-level")],
        };
        let canonical = canonicalize_persisted_conversation(
            raw.clone(),
            "project:test",
            "/w/flows/test.md",
            fixed_canonical_now(),
        )
        .expect("malformed v4 is repairable")
        .snapshot;
        assert_eq!(canonical.flow_id, "project:test");
        assert_eq!(canonical.flow_path, "/w/flows/test.md");
        assert_eq!(canonical.revision, 1);
        assert_eq!(canonical.threads.len(), 3);
        assert_eq!(
            canonical
                .threads
                .iter()
                .map(|thread| thread.turns.len())
                .sum::<usize>(),
            3,
            "non-empty v4 threads are authoritative; top-level turns are ignored"
        );
        assert!(canonical.turns.is_empty());
        assert_eq!(
            canonical
                .threads
                .iter()
                .filter(|thread| thread.state == PersistedFlowThreadState::Active)
                .count(),
            1
        );
        assert_eq!(
            canonical.active_thread_id,
            canonical.threads.last().unwrap().id
        );
        let unique: std::collections::HashSet<_> =
            canonical.threads.iter().map(|thread| &thread.id).collect();
        assert_eq!(unique.len(), canonical.threads.len());
        assert!(canonical.threads.iter().all(|thread| !thread.id.is_empty()));
        assert!(canonical
            .threads
            .iter()
            .all(|thread| thread.inherited_turn_count <= thread.turns.len()));
        assert!(canonical.threads.last().unwrap().archived_at.is_none());

        raw.version = SNAPSHOT_VERSION + 1;
        assert_eq!(
            canonicalize_persisted_conversation(
                raw,
                "project:test",
                "/w/flows/test.md",
                fixed_canonical_now(),
            ),
            Err(PersistedConversationLoadError::FutureVersion(
                SNAPSHOT_VERSION + 1
            ))
        );
    }

    #[test]
    fn empty_active_metadata_is_present_while_missing_store_is_absent() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:empty", "/w/flows/empty.md")
                .is_none()
        );
        persist_conversation_to(dir.path(), "project:empty", "/w/flows/empty.md", &[])
            .expect("persist empty active metadata");
        let loaded =
            load_persisted_conversation_from(dir.path(), "project:empty", "/w/flows/empty.md")
                .expect("empty active metadata must remain present");
        assert_eq!(loaded.threads.len(), 1);
        assert!(canonical_session_turns(&loaded).is_empty());
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

    fn store_snapshot(revision: u64, turns: Vec<SessionTurn>) -> PersistedFlowConversation {
        let mut snapshot = snapshot_from_turns("project:t", "/w/flows/t.md", &turns);
        snapshot.revision = revision;
        snapshot
    }

    #[test]
    fn conversation_store_newer_revision_wins() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        assert_eq!(
            store.persist_snapshot_and_wait(store_snapshot(1, vec![turn("q1", "a1")])),
            Ok(ConversationStoreReceipt::Written)
        );
        assert_eq!(
            store.persist_snapshot_and_wait(store_snapshot(
                2,
                vec![turn("q1", "a1"), turn("q2", "a2")],
            )),
            Ok(ConversationStoreReceipt::Written)
        );
        assert_eq!(
            store.persist_snapshot_and_wait(store_snapshot(1, vec![turn("old", "old")])),
            Ok(ConversationStoreReceipt::IgnoredStaleRevision)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("snapshot present");
        assert_eq!(loaded.revision, 2);
        assert_eq!(canonical_session_turns(&loaded).len(), 2);
    }

    fn snapshot_with_archive(revision: u64) -> PersistedFlowConversation {
        let mut snapshot = store_snapshot(revision, vec![turn("active", "answer")]);
        snapshot.threads.insert(
            0,
            PersistedFlowThread {
                id: "archive-b".into(),
                state: PersistedFlowThreadState::Archived,
                parent_thread_id: None,
                created_at: "2026-08-04T10:00:00Z".into(),
                archived_at: Some("2026-08-04T11:00:00Z".into()),
                inherited_turn_count: 0,
                turns: vec![PersistedFlowTurn::from(&turn("archived", "answer"))],
            },
        );
        snapshot
    }

    #[test]
    fn selected_thread_tombstone_rejects_late_stale_persist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let held = snapshot_with_archive(1);
        assert_eq!(
            store.persist_snapshot_and_wait(held.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut deletion = held.clone();
        deletion.revision = 2;
        deletion.threads.retain(|thread| thread.id != "archive-b");
        assert_eq!(
            store.persist_selected_deletion_and_wait(deletion, "archive-b".into()),
            Ok(ConversationStoreReceipt::Written)
        );
        assert_eq!(
            store.persist_snapshot_and_wait(held),
            Ok(ConversationStoreReceipt::IgnoredStaleRevision)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("manifest remains present");
        assert_eq!(loaded.revision, 2);
        assert!(loaded.threads.iter().all(|thread| thread.id != "archive-b"));
    }

    #[test]
    fn selected_thread_tombstone_rejects_forged_higher_revision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let original = snapshot_with_archive(1);
        assert_eq!(
            store.persist_snapshot_and_wait(original.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut deletion = original.clone();
        deletion.revision = 2;
        deletion.threads.retain(|thread| thread.id != "archive-b");
        assert_eq!(
            store.persist_selected_deletion_and_wait(deletion, "archive-b".into()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut forged = original;
        forged.revision = 3;
        assert_eq!(
            store.persist_snapshot_and_wait(forged),
            Ok(ConversationStoreReceipt::IgnoredTombstonedThread)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("manifest remains present");
        assert_eq!(loaded.revision, 2);
        assert!(loaded.threads.iter().all(|thread| thread.id != "archive-b"));
    }

    #[test]
    fn deleting_active_persists_one_empty_replacement_thread() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let original = store_snapshot(1, vec![turn("active", "answer")]);
        let deleted_id = original.active_thread_id.clone();
        assert_eq!(
            store.persist_snapshot_and_wait(original.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut replacement = snapshot_from_turns("project:t", "/w/flows/t.md", &[]);
        replacement.revision = 2;
        replacement.active_thread_id = "active-replacement".into();
        replacement.threads[0].id = replacement.active_thread_id.clone();
        assert_eq!(
            store.persist_selected_deletion_and_wait(replacement, deleted_id.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("empty replacement remains present");
        assert_eq!(loaded.threads.len(), 1);
        assert_ne!(loaded.active_thread_id, deleted_id);
        assert!(canonical_session_turns(&loaded).is_empty());
    }

    #[test]
    fn deleting_archive_preserves_active_and_other_archives() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FlowConversationStore::new(dir.path().to_path_buf());
        let mut original = snapshot_with_archive(1);
        original.threads.insert(
            1,
            PersistedFlowThread {
                id: "archive-c".into(),
                state: PersistedFlowThreadState::Archived,
                parent_thread_id: None,
                created_at: "2026-08-04T10:00:00Z".into(),
                archived_at: Some("2026-08-04T11:00:00Z".into()),
                inherited_turn_count: 0,
                turns: vec![PersistedFlowTurn::from(&turn("other", "answer"))],
            },
        );
        let active_id = original.active_thread_id.clone();
        assert_eq!(
            store.persist_snapshot_and_wait(original.clone()),
            Ok(ConversationStoreReceipt::Written)
        );
        let mut deletion = original;
        deletion.revision = 2;
        deletion.threads.retain(|thread| thread.id != "archive-b");
        assert_eq!(
            store.persist_selected_deletion_and_wait(deletion, "archive-b".into()),
            Ok(ConversationStoreReceipt::Written)
        );
        store.flush().expect("flush");
        let loaded = load_persisted_conversation_from(dir.path(), "project:t", "/w/flows/t.md")
            .expect("manifest remains present");
        assert_eq!(loaded.active_thread_id, active_id);
        assert!(loaded.threads.iter().any(|thread| thread.id == "archive-c"));
        assert!(loaded.threads.iter().all(|thread| thread.id != "archive-b"));
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
    fn persisted_conversation_distinguishes_missing_from_empty_active() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .is_none()
        );
        persist_conversation_to(dir.path(), "project:scout", "/a/flows/scout.md", &[])
            .expect("persist empty");
        let empty =
            load_persisted_conversation_from(dir.path(), "project:scout", "/a/flows/scout.md")
                .expect("empty active metadata is persisted");
        assert!(canonical_session_turns(&empty).is_empty());
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
        assert_eq!(canonical_session_turns(&alpha)[0].user, "alpha secrets");
        assert_eq!(canonical_session_turns(&beta)[0].user, "beta question");
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
            revision: 0,
            active_thread_id: String::new(),
            threads: Vec::new(),
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
        assert_eq!(canonical_session_turns(&adopted)[0].user, "old question");
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
