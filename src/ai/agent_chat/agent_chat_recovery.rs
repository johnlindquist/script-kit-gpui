//! Typed warm/setup recovery projection for Agent Chat launch failures.
//!
//! Warm launch owns effects, but it shares the same provider-independent
//! state machine and recovery-card copy as a live Agent Chat thread.

use std::path::Path;

use sk_protocol::ai_reliability::{
    recovery_plan_for, transition, AiModelSelection, AiOperationEvent, AiOperationState, AiPhase,
    AiSelectionState, AiSurfaceIdentity, AiWorkSnapshot, CapabilityDecision, Fingerprint,
    InvalidTransition, ModelId, PreservationReceipt, ProfileId, ProgressSnapshot, ProviderId,
    RetryPolicy, SelectionOrigin, TurnRequestRef, TurnRisk, WorkKey,
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
    failure: Option<&crate::ai::reliability::AppFailureRecord>,
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
    // The warm slot already classified this failure. Re-classifying its own
    // safe copy would erase actionable auth/config/runtime recovery details.
    let failure = failure.cloned().unwrap_or_else(|| {
        provider_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            "warm prepare failed before reporting available models",
        )
    });
    let recover_invalid_transition = |mut fallback: AiOperationState, error: InvalidTransition| {
        tracing::error!(
            ?error,
            "Agent Chat warm recovery encountered an invalid state transition"
        );
        let typed_failure = failure.failure.clone();
        let plan = recovery_plan_for(
            &fallback.identity,
            &typed_failure,
            fallback.retry,
            TurnRisk::ReadOnly,
            &ProgressSnapshot::none(),
        );
        fallback.diagnostic = typed_failure.diagnostic.clone();
        fallback.phase = AiPhase::AwaitingRecovery {
            failure: typed_failure,
            plan,
        };
        fallback
    };

    state = match transition(
        state.clone(),
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
    ) {
        Ok(outcome) => outcome.next,
        Err(error) => return recover_invalid_transition(state, error),
    };
    state = match transition(
        state.clone(),
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    ) {
        Ok(outcome) => outcome.next,
        Err(error) => return recover_invalid_transition(state, error),
    };
    match transition(
        state.clone(),
        AiOperationEvent::Failed(failure.failure.clone()),
    ) {
        Ok(outcome) => outcome.next,
        Err(error) => recover_invalid_transition(state, error),
    }
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

    fn pi_failure(raw: &str) -> crate::ai::reliability::AppFailureRecord {
        provider_failure(sk_protocol::ai_reliability::ProtocolComponent::Pi, raw)
    }

    #[test]
    fn warm_failure_uses_shared_typed_recovery_and_retry_budget() {
        let failure = pi_failure("authentication required");
        let state = warm_recovery_state(
            "general",
            Some("anthropic/claude"),
            Path::new("/tmp/project"),
            Some(&failure),
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
        let failure = pi_failure("connection timed out");
        let state = warm_recovery_state(
            "general",
            None,
            Path::new("/tmp/project"),
            Some(&failure),
            2,
        );
        let spec = warm_recovery_spec(&state).expect("recovery spec");
        assert!(!spec
            .actions
            .iter()
            .any(|action| { action.enabled && action.action.kind() == RecoveryActionKind::Retry }));
    }

    /// S11 regression lock: a warm slot that already classified its failure
    /// must keep that classification. The launch path used to hand the
    /// record's own safe copy back to the free-text classifier, which matched
    /// no auth wording and produced `Unknown` — so the one failure a Sign In
    /// button fixes rendered a card without one.
    #[test]
    fn warm_recovery_keeps_the_typed_failure_instead_of_reclassifying_its_own_copy() {
        let typed = crate::ai::reliability::setup_required_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            "login required",
            &["browser".to_string()],
        );
        assert_eq!(
            typed.failure.code,
            sk_protocol::ai_reliability::AiFailureCode::AuthenticationMissing
        );

        let state =
            warm_recovery_state("general", None, Path::new("/tmp/project"), Some(&typed), 0);
        let spec = warm_recovery_spec(&state).expect("recovery spec");
        assert!(
            spec.actions
                .iter()
                .any(|action| action.enabled && action.action.kind() == RecoveryActionKind::SignIn),
            "a setup-required warm failure must offer Sign In"
        );

        // The old round-trip: classify the record's own user-facing copy.
        let round_tripped = pi_failure(typed.primary_message());
        assert_eq!(
            round_tripped.failure.code,
            sk_protocol::ai_reliability::AiFailureCode::Unknown,
            "safe copy is not classifiable evidence — this is why the typed \
             record must be carried, not re-derived"
        );
    }
}
