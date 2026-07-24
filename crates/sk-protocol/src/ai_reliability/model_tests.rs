use std::collections::HashMap;

use super::*;

fn selection(model: &str) -> AiSelectionState {
    let selected = AiModelSelection {
        provider_id: Some(ProviderId::from("openai")),
        model_id: Some(ModelId::from(model)),
        profile_id: Some(ProfileId::from("quick")),
    };
    AiSelectionState {
        requested: Some(selected.clone()),
        effective: Some(selected),
        origin: SelectionOrigin::PersistedUserChoice,
        acknowledged_change: None,
    }
}

fn work() -> AiWorkSnapshot {
    AiWorkSnapshot {
        key: WorkKey::from("work-1"),
        transcript: PreservationReceipt::Preserved {
            fingerprint: Fingerprint::from("transcript"),
        },
        draft: PreservationReceipt::Restorable {
            fingerprint: Fingerprint::from("draft"),
        },
        attachments: PreservationReceipt::Preserved {
            fingerprint: Fingerprint::from("attachments"),
        },
        partial_output: PreservationReceipt::Restorable {
            fingerprint: Fingerprint::from("partial"),
        },
    }
}

fn identity() -> AiSurfaceIdentity {
    AiSurfaceIdentity::QuickAi {
        profile_id: ProfileId::from("quick"),
        provider_id: ProviderId::from("openai"),
        model_id: ModelId::from("spark"),
    }
}

fn ready() -> AiOperationState {
    AiOperationState::ready(
        identity(),
        selection("spark"),
        work(),
        RetryPolicy {
            automatic_max: 1,
            manual_max: 1,
        },
    )
}

fn pending() -> PendingTurn {
    PendingTurn {
        request: TurnRequestRef::from("request-1"),
        risk: TurnRisk::ReadOnly,
        start_command_id: Some(CommandId(1)),
    }
}

fn timeout_failure() -> AiFailure {
    AiFailure::new(
        AiFailureKind::Connectivity(ConnectivityFailure::Timeout),
        RetrySafety::SameSelectionReadOnly,
    )
}

fn unknown_failure() -> AiFailure {
    AiFailure::new(
        AiFailureKind::Unknown,
        RetrySafety::ExplicitUserConfirmation,
    )
}

fn transition_ok(state: AiOperationState, event: AiOperationEvent) -> AiTransition {
    transition(state, event).expect("transition should be valid")
}

fn submit(state: AiOperationState) -> AiTransition {
    transition_ok(
        state,
        AiOperationEvent::SubmitRequested {
            request: TurnRequestRef::from("request-1"),
            work: work(),
            selection: selection("spark"),
            risk: TurnRisk::ReadOnly,
        },
    )
}

fn running_state() -> AiOperationState {
    let preflight = submit(ready()).next;
    let starting = transition_ok(
        preflight,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    );
    let command_id = starting
        .commands
        .iter()
        .find_map(|command| match command {
            AiCommand::StartTurn(command) => Some(command.command_id),
            AiCommand::PersistWork(_)
            | AiCommand::CheckCapabilities(_)
            | AiCommand::CancelTurn { .. }
            | AiCommand::ScheduleBackoff { .. }
            | AiCommand::ApplySelection(_)
            | AiCommand::LaunchAuthentication(_)
            | AiCommand::OpenConfiguration(_)
            | AiCommand::OpenClientUpdate(_)
            | AiCommand::RecheckClientCapability(_)
            | AiCommand::ReattachSession(_)
            | AiCommand::RethreadFlow(_)
            | AiCommand::RestartFlowRun(_)
            | AiCommand::ContinueInAgentChat(_)
            | AiCommand::InstallOrRepairComponent(_)
            | AiCommand::CopyRedactedDiagnostics(_)
            | AiCommand::ClearPendingWork(_)
            | AiCommand::ScheduleRecoveredDismiss => None,
        })
        .expect("start command");
    transition_ok(
        starting.next,
        AiOperationEvent::RuntimeStarted {
            command_id,
            turn: TurnRef::from("turn-1"),
        },
    )
    .next
}

#[test]
fn happy_path_is_preflight_then_runtime_then_success() {
    let preflight = submit(ready());
    assert_eq!(preflight.next.phase.tag(), AiPhaseTag::Preflighting);
    assert!(matches!(
        preflight.commands.as_slice(),
        [AiCommand::PersistWork(_), AiCommand::CheckCapabilities(_)]
    ));

    let starting = transition_ok(
        preflight.next,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    );
    assert!(matches!(
        starting.commands.as_slice(),
        [AiCommand::PersistWork(_), AiCommand::StartTurn(_)]
    ));
    let AiCommand::StartTurn(start) = &starting.commands[1] else {
        panic!("second command should start the turn");
    };
    let running = transition_ok(
        starting.next,
        AiOperationEvent::RuntimeStarted {
            command_id: start.command_id,
            turn: TurnRef::from("turn-1"),
        },
    );
    assert_eq!(running.next.phase.tag(), AiPhaseTag::Running);

    let succeeded = transition_ok(
        running.next,
        AiOperationEvent::Completed(CompletionKind::Complete),
    );
    assert_eq!(succeeded.next.phase.tag(), AiPhaseTag::Succeeded);
    assert_eq!(
        succeeded.outcome,
        Some(AiOutcome::Succeeded {
            completeness: CompletionKind::Complete
        })
    );
}

#[test]
fn blocked_capability_produces_actionable_recovery_without_starting() {
    let blocked = AiFailure::new(
        AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
            client: ClientKind::Codex,
            model: Some(ModelId::from("gpt-5.6-sol")),
        }),
        RetrySafety::Never,
    );
    let next = transition_ok(
        submit(ready()).next,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Blocked(blocked)),
    );
    assert!(next.commands.is_empty());
    let AiPhase::AwaitingRecovery { plan, .. } = next.next.phase else {
        panic!("blocked capability should await recovery");
    };
    assert!(
        plan.option(RecoveryActionKind::ChooseCompatibleModel)
            .is_some_and(|option| option.enabled)
    );
    assert!(
        plan.option(RecoveryActionKind::UpdateClient)
            .is_some_and(|option| option.enabled)
    );
}

#[test]
fn automatic_retry_is_bounded_and_preserves_selection() {
    let first_failure = transition_ok(running_state(), AiOperationEvent::Failed(timeout_failure()));
    assert_eq!(first_failure.next.retry.automatic_used, 1);
    let AiPhase::Recovering { command_id, .. } = first_failure.next.phase.clone() else {
        panic!("safe read-only failure should schedule one automatic retry");
    };
    assert!(
        first_failure
            .commands
            .iter()
            .any(|command| matches!(command, AiCommand::ScheduleBackoff { .. }))
    );

    let preflight = transition_ok(
        first_failure.next,
        AiOperationEvent::BackoffElapsed { command_id },
    );
    let starting = transition_ok(
        preflight.next,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    );
    let AiCommand::StartTurn(start) = &starting.commands[1] else {
        panic!("start command expected");
    };
    let running = transition_ok(
        starting.next,
        AiOperationEvent::RuntimeStarted {
            command_id: start.command_id,
            turn: TurnRef::from("turn-2"),
        },
    );
    let second_failure = transition_ok(running.next, AiOperationEvent::Failed(timeout_failure()));
    assert_eq!(second_failure.next.retry.automatic_used, 1);
    assert_eq!(
        second_failure.next.phase.tag(),
        AiPhaseTag::AwaitingRecovery
    );
    assert!(
        !second_failure
            .commands
            .iter()
            .any(|command| matches!(command, AiCommand::ScheduleBackoff { .. }))
    );
}

#[test]
fn mutating_progress_is_never_automatically_replayed() {
    let progressed = transition_ok(
        running_state(),
        AiOperationEvent::Progressed(ProgressSnapshot {
            partial_output_available: true,
            mutating_effect_started: true,
            externally_visible_effect_started: true,
        }),
    );
    let failed = transition_ok(progressed.next, AiOperationEvent::Failed(timeout_failure()));
    assert_eq!(failed.next.phase.tag(), AiPhaseTag::AwaitingRecovery);
    assert_eq!(failed.next.retry.automatic_used, 0);
    assert!(
        !failed
            .commands
            .iter()
            .any(|command| matches!(command, AiCommand::ScheduleBackoff { .. }))
    );
}

#[test]
fn cancellation_is_an_outcome_not_a_failure() {
    let cancelling = transition_ok(running_state(), AiOperationEvent::CancelRequested);
    assert_eq!(cancelling.next.phase.tag(), AiPhaseTag::Cancelling);
    assert!(matches!(
        cancelling.commands.as_slice(),
        [AiCommand::CancelTurn { .. }]
    ));
    let cancelled = transition_ok(
        cancelling.next,
        AiOperationEvent::RuntimeCancelled {
            partial: PartialOutputState::Preserved {
                fingerprint: Fingerprint::from("partial"),
            },
        },
    );
    assert_eq!(cancelled.next.phase.tag(), AiPhaseTag::Cancelled);
    assert!(matches!(
        cancelled.outcome,
        Some(AiOutcome::Cancelled { .. })
    ));
    assert!(!cancelled.commands.iter().any(|command| matches!(
        command,
        AiCommand::ScheduleBackoff { .. } | AiCommand::StartTurn(_)
    )));
}

#[test]
fn selection_change_cannot_start_until_effect_acknowledges_it() {
    let mut state = submit(ready()).next;
    state.selection.requested = Some(AiModelSelection {
        provider_id: Some(ProviderId::from("openai")),
        model_id: Some(ModelId::from("compatible")),
        profile_id: Some(ProfileId::from("quick")),
    });
    let error = transition(
        state.clone(),
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    )
    .expect_err("unacknowledged selection must not start");
    assert_eq!(
        error.reason,
        InvalidTransitionReason::UnacknowledgedSelection
    );

    let failure = AiFailure::new(
        AiFailureKind::Capability(CapabilityFailure::ModelUnavailable {
            model: ModelId::from("spark"),
            reason: ModelAvailabilityReason::Removed,
        }),
        RetrySafety::Never,
    );
    let plan = recovery_plan_for(
        &state.identity,
        &failure,
        state.retry,
        TurnRisk::ReadOnly,
        &ProgressSnapshot::none(),
    );
    state.phase = AiPhase::AwaitingRecovery { failure, plan };
    let applied = AiModelSelection {
        provider_id: Some(ProviderId::from("openai")),
        model_id: Some(ModelId::from("compatible")),
        profile_id: Some(ProfileId::from("quick")),
    };
    let selecting = transition_ok(
        state,
        AiOperationEvent::RecoverySelected(AiRecoveryAction::ChooseCompatibleModel {
            selection: Some(applied.clone()),
        }),
    );
    let AiCommand::ApplySelection(change) = &selecting.commands[0] else {
        panic!("selection recovery must emit an explicit selection command");
    };
    let acknowledged = transition_ok(
        selecting.next,
        AiOperationEvent::RecoveryCommandSucceeded {
            command_id: change.command_id,
            result: RecoveryEffectResult::SelectionApplied(SelectionChangeReceipt {
                previous: Some(AiModelSelection {
                    provider_id: Some(ProviderId::from("openai")),
                    model_id: Some(ModelId::from("spark")),
                    profile_id: Some(ProfileId::from("quick")),
                }),
                applied,
                origin: SelectionOrigin::RecoveryChoice,
            }),
        },
    );
    assert_eq!(acknowledged.next.phase.tag(), AiPhaseTag::Recovered);
    let preflight = transition_ok(acknowledged.next, AiOperationEvent::ResetForNextTurn);
    let starting = transition_ok(
        preflight.next,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    );
    assert!(matches!(
        starting.commands.as_slice(),
        [AiCommand::PersistWork(_), AiCommand::StartTurn(_)]
    ));
}

#[test]
fn work_fingerprints_survive_failure_retry_dismiss_and_reattach() {
    let original = work();
    let mut state = running_state();
    state.work = original.clone();
    let failed = transition_ok(state, AiOperationEvent::Failed(unknown_failure()));
    assert_eq!(failed.next.work, original);

    let retrying = transition_ok(
        failed.next.clone(),
        AiOperationEvent::RecoverySelected(AiRecoveryAction::Retry),
    );
    assert_eq!(retrying.next.work, original);

    let dismissed = transition_ok(failed.next, AiOperationEvent::DismissRequested);
    assert_eq!(dismissed.next.work, original);

    let restarted = transition_ok(
        dismissed.next,
        AiOperationEvent::RestartObserved(RestartSnapshot {
            session: Some(SessionRef::from("session-1")),
            partial: PartialOutputState::None,
        }),
    );
    assert_eq!(restarted.next.work, original);
    let reattached = transition_ok(
        restarted.next,
        AiOperationEvent::SessionReattached(ReattachReceipt {
            session: SessionRef::from("session-1"),
            turn: TurnRef::from("turn-1"),
            risk: TurnRisk::ReadOnly,
            progress: ProgressSnapshot::none(),
        }),
    );
    assert_eq!(reattached.next.work, original);
}

#[test]
fn restart_always_reattaches_or_becomes_actionable() {
    let with_session = transition_ok(
        running_state(),
        AiOperationEvent::RestartObserved(RestartSnapshot {
            session: Some(SessionRef::from("session-1")),
            partial: PartialOutputState::None,
        }),
    );
    assert!(matches!(
        with_session.commands.as_slice(),
        [AiCommand::PersistWork(_), AiCommand::ReattachSession(_)]
    ));

    let without_session = transition_ok(
        running_state(),
        AiOperationEvent::RestartObserved(RestartSnapshot {
            session: None,
            partial: PartialOutputState::None,
        }),
    );
    let AiPhase::AwaitingRecovery { plan, .. } = without_session.next.phase else {
        panic!("lost session should be actionable");
    };
    assert!(plan.options.iter().any(|option| option.enabled));
}

#[test]
fn every_failure_variant_has_an_exhaustive_recovery_decision() {
    let diagnostic = DiagnosticDescriptor {
        id: DiagnosticId::from("diag"),
        fingerprint: Fingerprint::from("diag-fingerprint"),
        availability: DiagnosticAvailability::Available,
        visibility: DiagnosticVisibility::SecondaryOnly,
        redaction: DiagnosticRedaction::AllowlistedFieldsV1,
    };
    for kind in representative_failure_kinds() {
        let failure = AiFailure::new(kind, RetrySafety::ExplicitUserConfirmation)
            .with_diagnostic(diagnostic.clone());
        assert_eq!(failure.code, AiFailureCode::from_kind(&failure.kind));
        let plan = recovery_plan_for(
            &identity(),
            &failure,
            ready().retry,
            TurnRisk::ReadOnly,
            &ProgressSnapshot::none(),
        );
        assert!(
            plan.options
                .iter()
                .any(|option| option.enabled && option.role == RecoveryRole::Primary),
            "failure {:?} lacks an enabled primary action",
            failure.code
        );
        assert!(
            plan.option(RecoveryActionKind::CopyDetails)
                .is_some_and(|option| option.enabled)
        );
    }
}

#[test]
fn cartesian_state_event_table_is_deterministic_and_explicit() {
    let states = representative_states();
    let events = representative_events();
    let mut valid: HashMap<AiEventTag, usize> = HashMap::new();
    let mut rejected: HashMap<AiEventTag, usize> = HashMap::new();
    for state in states {
        for event in &events {
            let first = transition(state.clone(), event.clone());
            let second = transition(state.clone(), event.clone());
            assert_eq!(first, second, "decision must be deterministic");
            match first {
                Ok(result) => {
                    *valid.entry(event.tag()).or_default() += 1;
                    assert_command_invariants(&state, event, &result);
                }
                Err(error) => {
                    *rejected.entry(event.tag()).or_default() += 1;
                    assert_eq!(error.phase, state.phase.tag());
                    assert_eq!(error.event, event.tag());
                }
            }
        }
    }
    for event in events {
        assert!(
            valid.get(&event.tag()).copied().unwrap_or(0) > 0,
            "event {:?} needs a valid representative",
            event.tag()
        );
        match event.tag() {
            AiEventTag::RestartObserved => assert_eq!(
                rejected.get(&event.tag()).copied().unwrap_or(0),
                0,
                "restart observation is deliberately valid from every phase"
            ),
            AiEventTag::SubmitRequested
            | AiEventTag::CapabilityResolved
            | AiEventTag::RuntimeStarted
            | AiEventTag::Progressed
            | AiEventTag::Completed
            | AiEventTag::Failed
            | AiEventTag::CancelRequested
            | AiEventTag::RuntimeCancelled
            | AiEventTag::RecoverySelected
            | AiEventTag::RecoveryCommandSucceeded
            | AiEventTag::RecoveryCommandFailed
            | AiEventTag::BackoffElapsed
            | AiEventTag::SessionReattached
            | AiEventTag::SessionReattachFailed
            | AiEventTag::DismissRequested
            | AiEventTag::ResetForNextTurn => assert!(
                rejected.get(&event.tag()).copied().unwrap_or(0) > 0,
                "event {:?} needs a rejected representative",
                event.tag()
            ),
        }
    }
}

#[test]
fn bounded_generated_sequences_preserve_global_invariants() {
    let events = representative_events();
    for first in &events {
        for second in &events {
            for third in &events {
                let mut state = ready();
                for event in [first, second, third] {
                    let before = state.clone();
                    match transition(state.clone(), event.clone()) {
                        Ok(result) => {
                            assert_command_invariants(&before, event, &result);
                            assert!(
                                result.next.retry.automatic_used
                                    <= result.next.retry.policy.automatic_max
                            );
                            assert!(
                                result.next.retry.manual_used
                                    <= result.next.retry.policy.manual_max
                            );
                            state = result.next;
                        }
                        Err(error) => {
                            assert_eq!(error.phase, before.phase.tag());
                            assert_eq!(error.event, event.tag());
                            state = before;
                        }
                    }
                }
            }
        }
    }
}

fn assert_command_invariants(
    before: &AiOperationState,
    event: &AiOperationEvent,
    result: &AiTransition,
) {
    if let Some(start_index) = result
        .commands
        .iter()
        .position(|command| matches!(command, AiCommand::StartTurn(_)))
    {
        let persist_index = result
            .commands
            .iter()
            .position(|command| matches!(command, AiCommand::PersistWork(_)))
            .expect("PersistWork must accompany StartTurn");
        assert!(persist_index < start_index);
        assert!(result.next.selection.can_start_turn());
    }
    if result
        .commands
        .iter()
        .any(|command| matches!(command, AiCommand::ApplySelection(_)))
    {
        assert!(matches!(
            event,
            AiOperationEvent::RecoverySelected(
                AiRecoveryAction::ChooseCompatibleModel { .. }
                    | AiRecoveryAction::ChooseProvider { .. }
                    | AiRecoveryAction::ChooseProfile { .. }
            )
        ));
    }
    if let AiPhase::Running { progress, .. } = &before.phase {
        if progress.mutating_effect_started || progress.externally_visible_effect_started {
            if matches!(event, AiOperationEvent::Failed(_)) {
                assert!(
                    !result
                        .commands
                        .iter()
                        .any(|command| matches!(command, AiCommand::ScheduleBackoff { .. }))
                );
            }
        }
    }
    if matches!(event, AiOperationEvent::RuntimeCancelled { .. }) {
        assert!(!result.commands.iter().any(|command| matches!(
            command,
            AiCommand::ScheduleBackoff { .. } | AiCommand::StartTurn(_)
        )));
    }
}

fn representative_states() -> Vec<AiOperationState> {
    let mut states = Vec::new();

    states.push(ready());

    let mut preflight = ready();
    preflight.phase = AiPhase::Preflighting {
        request: TurnRequestRef::from("request-1"),
    };
    preflight.pending = Some(pending());
    states.push(preflight);

    states.push(running_state());

    let mut cancelling = running_state();
    cancelling.phase = AiPhase::Cancelling {
        turn: TurnRef::from("turn-1"),
        partial: PartialOutputState::None,
    };
    states.push(cancelling);

    let failure = unknown_failure();
    let plan = recovery_plan_for(
        &identity(),
        &failure,
        ready().retry,
        TurnRisk::ReadOnly,
        &ProgressSnapshot::none(),
    );
    let mut awaiting = ready();
    awaiting.phase = AiPhase::AwaitingRecovery { failure, plan };
    awaiting.pending = Some(pending());
    states.push(awaiting);

    let mut recovering = ready();
    recovering.phase = AiPhase::Recovering {
        action: AiRecoveryAction::Retry,
        command_id: CommandId(1),
    };
    recovering.pending = Some(pending());
    states.push(recovering);

    let mut reattaching = ready();
    reattaching.phase = AiPhase::Recovering {
        action: AiRecoveryAction::Reattach {
            session: SessionRef::from("session-1"),
        },
        command_id: CommandId(1),
    };
    reattaching.pending = Some(pending());
    states.push(reattaching);

    let mut recovered = ready();
    recovered.phase = AiPhase::Recovered {
        action: AiRecoveryAction::CheckAgain,
        summary: RecoverySuccess::ReadyToRetry,
    };
    recovered.pending = Some(pending());
    states.push(recovered);

    let mut succeeded = ready();
    succeeded.phase = AiPhase::Succeeded {
        completeness: CompletionKind::Complete,
        recovered_from: None,
    };
    states.push(succeeded);

    let mut cancelled = ready();
    cancelled.phase = AiPhase::Cancelled {
        kind: CancellationKind::UserCancelled,
        partial: PartialOutputState::None,
    };
    states.push(cancelled);

    let mut dismissed = ready();
    dismissed.phase = AiPhase::Dismissed {
        failure: AiFailureCode::Unknown,
    };
    states.push(dismissed);

    states
}

fn representative_events() -> Vec<AiOperationEvent> {
    vec![
        AiOperationEvent::SubmitRequested {
            request: TurnRequestRef::from("request-1"),
            work: work(),
            selection: selection("spark"),
            risk: TurnRisk::ReadOnly,
        },
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
        AiOperationEvent::RuntimeStarted {
            command_id: CommandId(1),
            turn: TurnRef::from("turn-1"),
        },
        AiOperationEvent::Progressed(ProgressSnapshot::none()),
        AiOperationEvent::Completed(CompletionKind::Complete),
        AiOperationEvent::Failed(unknown_failure()),
        AiOperationEvent::CancelRequested,
        AiOperationEvent::RuntimeCancelled {
            partial: PartialOutputState::None,
        },
        AiOperationEvent::RecoverySelected(AiRecoveryAction::Retry),
        AiOperationEvent::RecoveryCommandSucceeded {
            command_id: CommandId(1),
            result: RecoveryEffectResult::NoChange,
        },
        AiOperationEvent::RecoveryCommandFailed {
            command_id: CommandId(1),
            failure: unknown_failure(),
        },
        AiOperationEvent::BackoffElapsed {
            command_id: CommandId(1),
        },
        AiOperationEvent::RestartObserved(RestartSnapshot {
            session: Some(SessionRef::from("session-1")),
            partial: PartialOutputState::None,
        }),
        AiOperationEvent::SessionReattached(ReattachReceipt {
            session: SessionRef::from("session-1"),
            turn: TurnRef::from("turn-1"),
            risk: TurnRisk::ReadOnly,
            progress: ProgressSnapshot::none(),
        }),
        AiOperationEvent::SessionReattachFailed(unknown_failure()),
        AiOperationEvent::DismissRequested,
        AiOperationEvent::ResetForNextTurn,
    ]
}

fn representative_failure_kinds() -> Vec<AiFailureKind> {
    vec![
        AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
            client: ClientKind::Codex,
            model: Some(ModelId::from("model")),
        }),
        AiFailureKind::Capability(CapabilityFailure::ModelUnavailable {
            model: ModelId::from("model"),
            reason: ModelAvailabilityReason::Removed,
        }),
        AiFailureKind::Capability(CapabilityFailure::NoCompatibleModel),
        AiFailureKind::Capability(CapabilityFailure::ProfileUnavailable {
            profile: ProfileId::from("profile"),
        }),
        AiFailureKind::Policy(PolicyFailure::QuickAiSearchBudgetExceeded {
            completed_searches: 1,
            budget: 1,
            partial_answer_available: true,
            source_count: 2,
        }),
        AiFailureKind::Policy(PolicyFailure::ToolDenied {
            tool: Some(ToolId::from("write")),
        }),
        AiFailureKind::Authentication(AuthenticationFailure::Missing),
        AiFailureKind::Authentication(AuthenticationFailure::Expired),
        AiFailureKind::Authentication(AuthenticationFailure::UsageExhausted),
        AiFailureKind::Configuration(ConfigurationFailure::ProviderNotConfigured),
        AiFailureKind::Configuration(ConfigurationFailure::NoModelsAvailable),
        AiFailureKind::Configuration(ConfigurationFailure::SidecarMissing),
        AiFailureKind::Configuration(ConfigurationFailure::MdflowMissing),
        AiFailureKind::Configuration(ConfigurationFailure::InvalidConfiguration),
        AiFailureKind::Connectivity(ConnectivityFailure::Offline),
        AiFailureKind::Connectivity(ConnectivityFailure::Timeout),
        AiFailureKind::Connectivity(ConnectivityFailure::RateLimited {
            retry_after_ms: Some(1000),
        }),
        AiFailureKind::Provider(ProviderFailure::TemporarilyUnavailable),
        AiFailureKind::Provider(ProviderFailure::ServerRejected),
        AiFailureKind::Runtime(RuntimeFailure::SpawnFailed),
        AiFailureKind::Runtime(RuntimeFailure::RuntimeClosed),
        AiFailureKind::Runtime(RuntimeFailure::ChildExited {
            exit_code: Some(1),
            signal: None,
        }),
        AiFailureKind::Runtime(RuntimeFailure::SessionLost {
            reattach: ReattachAvailability::Available {
                session: SessionRef::from("session"),
            },
        }),
        AiFailureKind::Protocol(ProtocolFailure::VersionMismatch {
            component: ProtocolComponent::Codex,
            expected: "2".to_string(),
            actual: Some("1".to_string()),
        }),
        AiFailureKind::Protocol(ProtocolFailure::SequenceViolation {
            component: ProtocolComponent::Mdflow,
        }),
        AiFailureKind::Protocol(ProtocolFailure::OrderViolation {
            component: ProtocolComponent::Mdflow,
        }),
        AiFailureKind::Protocol(ProtocolFailure::MalformedResponse {
            component: ProtocolComponent::Pi,
        }),
        AiFailureKind::Protocol(ProtocolFailure::MissingTerminal {
            component: ProtocolComponent::Provider,
        }),
        AiFailureKind::Permission(PermissionFailure::PermissionDenied),
        AiFailureKind::Permission(PermissionFailure::UserDeniedTool),
        AiFailureKind::Input(InputFailure::MessageTooLarge),
        AiFailureKind::Input(InputFailure::ContextLimitExceeded),
        AiFailureKind::Unknown,
    ]
}
