//! Typed warm/setup recovery projection for Agent Chat launch failures.
//!
//! Warm launch owns effects, but it shares the same provider-independent
//! state machine and recovery-card copy as a live Agent Chat thread.

use std::path::Path;

use sk_protocol::ai_reliability::{
    transition, AiModelSelection, AiOperationEvent, AiOperationState, AiSelectionState,
    AiSurfaceIdentity, AiWorkSnapshot, CapabilityDecision, Fingerprint, ModelId,
    PreservationReceipt, ProfileId, ProviderId, RetryPolicy, SelectionOrigin, TurnRequestRef,
    TurnRisk, WorkKey,
};

use crate::ai::reliability::{
    project_recovery, provider_failure, redacted_fingerprint, AiRecoveryCardSpec, AiRecoveryLayout,
    SurfaceRecoveryCapabilities,
};

fn provider_from_model(model_id: Option<&str>) -> Option<ProviderId> {
    model_id
        .and_then(|model| model.split_once('/').map(|(provider, _)| provider))
        .filter(|provider| !provider.is_empty())
        .map(ProviderId::from)
}

pub(crate) fn warm_recovery_state(
    profile_id: &str,
    model_id: Option<&str>,
    cwd: &Path,
    detail: &str,
    attempts: u32,
) -> AiOperationState {
    let provider_id = provider_from_model(model_id);
    let selection = AiModelSelection {
        provider_id: provider_id.clone(),
        model_id: model_id.map(ModelId::from),
        profile_id: Some(ProfileId::from(profile_id)),
    };
    let mut state = AiOperationState::ready(
        AiSurfaceIdentity::AgentChat {
            profile_id: ProfileId::from(profile_id),
            provider_id,
            model_id: model_id.map(ModelId::from),
            cwd_fingerprint: Fingerprint(redacted_fingerprint(&cwd.to_string_lossy())),
        },
        AiSelectionState {
            requested: Some(selection.clone()),
            effective: Some(selection),
            origin: SelectionOrigin::PersistedUserChoice,
            acknowledged_change: None,
        },
        AiWorkSnapshot {
            key: WorkKey::from("agent-chat-warm-launch"),
            transcript: PreservationReceipt::NotApplicable,
            draft: PreservationReceipt::NotApplicable,
            attachments: PreservationReceipt::NotApplicable,
            partial_output: PreservationReceipt::NotApplicable,
        },
        RetryPolicy {
            automatic_max: 0,
            manual_max: 2,
        },
    );
    state.retry.manual_used = attempts.min(u8::MAX as u32) as u8;
    state = transition(
        state,
        AiOperationEvent::SubmitRequested {
            request: TurnRequestRef::from("agent-chat-warm-launch"),
            work: AiWorkSnapshot {
                key: WorkKey::from("agent-chat-warm-launch"),
                transcript: PreservationReceipt::NotApplicable,
                draft: PreservationReceipt::NotApplicable,
                attachments: PreservationReceipt::NotApplicable,
                partial_output: PreservationReceipt::NotApplicable,
            },
            selection: AiSelectionState {
                requested: None,
                effective: None,
                origin: SelectionOrigin::BuiltInDefault,
                acknowledged_change: None,
            },
            risk: TurnRisk::ReadOnly,
        },
    )
    .expect("ready warm recovery submit is valid")
    .next;
    state = transition(
        state,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    )
    .expect("warm recovery capability transition is valid")
    .next;
    let failure = provider_failure(sk_protocol::ai_reliability::ProtocolComponent::Pi, detail);
    transition(state, AiOperationEvent::Failed(failure.failure))
        .expect("preflight warm failure is valid")
        .next
}

pub(crate) fn warm_recovery_spec(state: &AiOperationState) -> Option<AiRecoveryCardSpec> {
    project_recovery(
        &state.identity,
        state,
        &SurfaceRecoveryCapabilities::all()
            .layout(AiRecoveryLayout::BlockingPanel)
            .dismissible(true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sk_protocol::ai_reliability::{AiPhase, RecoveryActionKind};

    #[test]
    fn warm_failure_uses_shared_typed_recovery_and_retry_budget() {
        let state = warm_recovery_state(
            "general",
            Some("anthropic/claude"),
            Path::new("/tmp/project"),
            "authentication required",
            0,
        );
        assert!(matches!(state.phase, AiPhase::AwaitingRecovery { .. }));
        let spec = warm_recovery_spec(&state).expect("recovery spec");
        assert_eq!(spec.layout, AiRecoveryLayout::BlockingPanel);
        assert!(spec.actions.iter().any(|action| {
            action.enabled && action.action.kind() == RecoveryActionKind::SignIn
        }));
    }

    #[test]
    fn exhausted_warm_retry_is_not_presented_as_available() {
        let state = warm_recovery_state(
            "general",
            None,
            Path::new("/tmp/project"),
            "connection timed out",
            2,
        );
        let spec = warm_recovery_spec(&state).expect("recovery spec");
        assert!(!spec
            .actions
            .iter()
            .any(|action| { action.enabled && action.action.kind() == RecoveryActionKind::Retry }));
    }
}
