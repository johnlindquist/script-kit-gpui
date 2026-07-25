//! App-level adapters for the provider-independent AI reliability domain.
//!
//! Raw provider, process, and protocol failures stop at this boundary. Callers
//! receive an exhaustive domain failure plus presentation keys and a reference
//! to redacted secondary diagnostics; primary UI never receives the raw input.

mod capabilities;
mod capability_cache;
mod classify;
mod devtools;
mod diagnostics;
mod presentation;
#[cfg(test)]
mod presentation_tests;
mod runtime_boundary;
mod selection;

#[cfg(test)]
mod tests;

pub use capabilities::{
    preflight, AiModelCandidate, CapabilityDecision, CapabilityEvidence, CapabilityEvidenceKind,
    CapabilityReceipt, ExecutableIdentity,
};
pub use capability_cache::{
    CapabilityCache, CapabilityCacheKey, CompatibilityRecord, CompatibilityRecordKind,
    PersistedCapabilityRecords,
};
pub use classify::{
    classify_process_failure, classify_protocol_failure, classify_provider_failure,
    classify_runtime_closed, classify_setup_required, classify_spawn_failure,
    primary_message_for_failure, quick_ai_deadline_failure, quick_ai_search_budget_failure,
    quick_ai_turn_failure, AppFailureRecord, FailureContext, FailurePresentationInput,
    ProcessFailureFacts, ProtocolFailureFacts,
};
pub(crate) use devtools::{
    ai_operation_state_snapshot, ai_reliability_fixture_for_target,
    ai_reliability_fixture_snapshot, ai_reliability_snapshot_for_target, phase_name,
    redacted_fingerprint, set_ai_reliability_fixture,
};
pub use diagnostics::{redact_diagnostic, DiagnosticVault, RedactedDiagnostic};
pub use presentation::{
    flow_session_recovery_capabilities, project_recovery, standalone_failure_recovery_spec,
    AiRecoveryActionSpec, AiRecoveryCardSpec, AiRecoveryLayout, AiRecoveryProgress, AiRecoveryTone,
    SurfaceRecoveryCapabilities, AI_RECOVERY_BODY_ID, AI_RECOVERY_CARD_ID, AI_RECOVERY_DISMISS_ID,
    AI_RECOVERY_PROGRESS_ID, AI_RECOVERY_TITLE_ID,
};
pub(crate) use runtime_boundary::{
    process_failure, protocol_failure, provider_failure, quick_ai_failure, runtime_closed_failure,
    setup_required_failure, spawn_failure, AiAdapterError, AiAdapterResult, AiTurnRuntimeOutcome,
};
pub use selection::{acknowledge_selection, decide_selection_change, SelectionDecision};
