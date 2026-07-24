//! Tests for the pure shared AI recovery projector.

use super::*;
use sk_protocol::ai_reliability::*;

fn identity() -> AiSurfaceIdentity {
    AiSurfaceIdentity::QuickAi {
        profile_id: ProfileId("quick".into()),
        provider_id: ProviderId("provider".into()),
        model_id: ModelId("model".into()),
    }
}

fn state_with(failure: AiFailure, options: Vec<RecoveryOption>) -> AiOperationState {
    let selection = AiSelectionState {
        requested: None,
        effective: None,
        origin: SelectionOrigin::BuiltInDefault,
        acknowledged_change: None,
    };
    let work = sk_protocol::ai_reliability::AiWorkSnapshot {
        key: WorkKey("work".into()),
        transcript: PreservationReceipt::Preserved {
            fingerprint: Fingerprint("transcript".into()),
        },
        draft: PreservationReceipt::NotApplicable,
        attachments: PreservationReceipt::NotApplicable,
        partial_output: PreservationReceipt::NotApplicable,
    };
    let mut state = AiOperationState::ready(
        identity(),
        selection,
        work,
        RetryPolicy {
            automatic_max: 1,
            manual_max: 1,
        },
    );
    state.diagnostic = failure.diagnostic.clone();
    state.phase = AiPhase::AwaitingRecovery {
        failure,
        plan: RecoveryPlan { options },
    };
    state
}

fn option(kind: RecoveryActionKind, role: RecoveryRole, enabled: bool) -> RecoveryOption {
    RecoveryOption {
        kind,
        role,
        enabled,
        disabled_reason: (!enabled).then_some(DisabledReason::WaitingForBackoff),
    }
}

fn representative_failures() -> Vec<AiFailure> {
    let never = RetrySafety::Never;
    vec![
        AiFailure::new(
            AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
                client: ClientKind::Codex,
                model: Some("model".into()),
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Capability(CapabilityFailure::ModelUnavailable {
                model: "model".into(),
                reason: ModelAvailabilityReason::UnsupportedByClient,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Capability(CapabilityFailure::NoCompatibleModel),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Capability(CapabilityFailure::ProfileUnavailable {
                profile: "profile".into(),
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Policy(PolicyFailure::QuickAiSearchBudgetExceeded {
                completed_searches: 1,
                budget: 1,
                partial_answer_available: true,
                source_count: 1,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Policy(PolicyFailure::ToolDenied { tool: None }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Authentication(AuthenticationFailure::Missing),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Authentication(AuthenticationFailure::Expired),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Authentication(AuthenticationFailure::UsageExhausted),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Configuration(ConfigurationFailure::ProviderNotConfigured),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Configuration(ConfigurationFailure::NoModelsAvailable),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Configuration(ConfigurationFailure::SidecarMissing),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Configuration(ConfigurationFailure::MdflowMissing),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Configuration(ConfigurationFailure::InvalidConfiguration),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Connectivity(ConnectivityFailure::Offline),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Connectivity(ConnectivityFailure::Timeout),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Connectivity(ConnectivityFailure::RateLimited {
                retry_after_ms: Some(1_000),
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Provider(ProviderFailure::TemporarilyUnavailable),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Provider(ProviderFailure::ServerRejected),
            never,
        ),
        AiFailure::new(AiFailureKind::Runtime(RuntimeFailure::SpawnFailed), never),
        AiFailure::new(AiFailureKind::Runtime(RuntimeFailure::RuntimeClosed), never),
        AiFailure::new(
            AiFailureKind::Runtime(RuntimeFailure::ChildExited {
                exit_code: Some(1),
                signal: None,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Runtime(RuntimeFailure::SessionLost {
                reattach: ReattachAvailability::Unavailable,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Protocol(ProtocolFailure::VersionMismatch {
                component: ProtocolComponent::Codex,
                expected: "supported".into(),
                actual: Some("old".into()),
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Protocol(ProtocolFailure::SequenceViolation {
                component: ProtocolComponent::Pi,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Protocol(ProtocolFailure::OrderViolation {
                component: ProtocolComponent::Mdflow,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Protocol(ProtocolFailure::MalformedResponse {
                component: ProtocolComponent::Provider,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Protocol(ProtocolFailure::MissingTerminal {
                component: ProtocolComponent::LocalLlm,
            }),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Permission(PermissionFailure::PermissionDenied),
            never,
        ),
        AiFailure::new(
            AiFailureKind::Permission(PermissionFailure::UserDeniedTool),
            never,
        ),
        AiFailure::new(AiFailureKind::Input(InputFailure::MessageTooLarge), never),
        AiFailure::new(
            AiFailureKind::Input(InputFailure::ContextLimitExceeded),
            never,
        ),
        AiFailure::new(AiFailureKind::Unknown, never),
    ]
}

#[test]
fn every_failure_kind_has_plain_copy_and_recovery_state() {
    for failure in representative_failures() {
        let state = state_with(
            failure,
            vec![option(
                RecoveryActionKind::Retry,
                RecoveryRole::Primary,
                true,
            )],
        );
        let spec = project_recovery(&identity(), &state, &Default::default()).unwrap();
        let copy = format!("{} {}", spec.title, spec.body);
        assert!(!copy.contains('{'));
        assert!(!copy.contains("Authorization"));
        assert!(!copy.contains("Cookie"));
        assert!(!copy.contains("Using default model"));
        assert!(spec.actions.iter().any(|action| action.enabled));
    }
}

#[test]
fn action_order_limits_visible_choices_and_puts_diagnostics_last() {
    let failure = AiFailure::new(AiFailureKind::Unknown, RetrySafety::Never);
    let mut state = state_with(
        failure,
        vec![
            option(RecoveryActionKind::Retry, RecoveryRole::Primary, true),
            option(RecoveryActionKind::CheckAgain, RecoveryRole::Primary, true),
            option(
                RecoveryActionKind::ChooseProvider,
                RecoveryRole::Secondary,
                true,
            ),
            option(
                RecoveryActionKind::ChooseProfile,
                RecoveryRole::Secondary,
                true,
            ),
            option(
                RecoveryActionKind::ConfigureProvider,
                RecoveryRole::Secondary,
                true,
            ),
            option(
                RecoveryActionKind::CopyDetails,
                RecoveryRole::Diagnostic,
                true,
            ),
        ],
    );
    state.diagnostic = Some(sk_protocol::ai_reliability::DiagnosticDescriptor {
        id: "detail".into(),
        fingerprint: "fingerprint".into(),
        availability: sk_protocol::ai_reliability::DiagnosticAvailability::Available,
        visibility: sk_protocol::ai_reliability::DiagnosticVisibility::SecondaryOnly,
        redaction: sk_protocol::ai_reliability::DiagnosticRedaction::AllowlistedFieldsV1,
    });
    let spec = project_recovery(&identity(), &state, &Default::default()).unwrap();
    assert_eq!(spec.actions.len(), 4);
    assert_eq!(spec.actions[0].role, RecoveryRole::Primary);
    assert_eq!(spec.actions[3].role, RecoveryRole::Diagnostic);
}

#[test]
fn exhausted_retry_disappears_and_waiting_progress_is_explicit() {
    let failure = AiFailure::new(AiFailureKind::Unknown, RetrySafety::Never);
    let mut state = state_with(
        failure,
        vec![option(
            RecoveryActionKind::Retry,
            RecoveryRole::Primary,
            true,
        )],
    );
    state.retry.manual_used = state.retry.policy.manual_max;
    let caps = SurfaceRecoveryCapabilities::all().waiting_for_external_state(true);
    let spec = project_recovery(&identity(), &state, &caps).unwrap();
    assert!(spec.actions.is_empty());
    assert!(spec.progress.is_some());
}

#[test]
fn first_secondary_becomes_primary_when_plan_has_no_primary() {
    let state = state_with(
        AiFailure::new(AiFailureKind::Unknown, RetrySafety::Never),
        vec![option(
            RecoveryActionKind::CheckAgain,
            RecoveryRole::Secondary,
            true,
        )],
    );
    let spec = project_recovery(&identity(), &state, &Default::default()).unwrap();
    assert_eq!(spec.actions[0].role, RecoveryRole::Primary);
}

#[test]
fn selection_success_copy_requires_acknowledged_user_change() {
    let mut state = state_with(
        AiFailure::new(AiFailureKind::Unknown, RetrySafety::Never),
        vec![],
    );
    let action = AiRecoveryAction::ChooseCompatibleModel { selection: None };
    state.phase = AiPhase::Recovered {
        action: action.clone(),
        summary: sk_protocol::ai_reliability::RecoverySuccess::SelectionApplied,
    };
    let unacknowledged = project_recovery(&identity(), &state, &Default::default()).unwrap();
    assert_eq!(
        unacknowledged.body.as_ref(),
        "Recovery completed successfully."
    );

    let applied = AiModelSelection {
        provider_id: Some("provider".into()),
        model_id: Some("model".into()),
        profile_id: None,
    };
    state.selection.effective = Some(applied.clone());
    state.selection.acknowledged_change = Some(SelectionChangeReceipt {
        previous: None,
        applied,
        origin: SelectionOrigin::RecoveryChoice,
    });
    let acknowledged = project_recovery(&identity(), &state, &Default::default()).unwrap();
    assert_eq!(
        acknowledged.body.as_ref(),
        "The selected AI configuration is ready."
    );
}

#[test]
fn dismissal_projection_does_not_mutate_work_or_diagnostics() {
    let failure = AiFailure::new(AiFailureKind::Unknown, RetrySafety::Never);
    let state = state_with(
        failure,
        vec![option(
            RecoveryActionKind::Retry,
            RecoveryRole::Primary,
            true,
        )],
    );
    let before_work = state.work.clone();
    let before_diagnostic = state.diagnostic.clone();
    let _ = project_recovery(&identity(), &state, &Default::default()).unwrap();
    assert_eq!(state.work, before_work);
    assert_eq!(state.diagnostic, before_diagnostic);
}
