use super::types::*;

pub fn transition(
    state: AiOperationState,
    event: AiOperationEvent,
) -> Result<AiTransition, InvalidTransition> {
    let event_tag = event.tag();
    match event {
        AiOperationEvent::SubmitRequested {
            request,
            work,
            selection,
            risk,
        } => submit_requested(state, event_tag, request, work, selection, risk),
        AiOperationEvent::CapabilityResolved(decision) => {
            capability_resolved(state, event_tag, decision)
        }
        AiOperationEvent::RuntimeStarted { command_id, turn } => {
            runtime_started(state, event_tag, command_id, turn)
        }
        AiOperationEvent::Progressed { progress, work } => {
            progressed(state, event_tag, progress, work)
        }
        AiOperationEvent::Completed(completeness) => completed(state, event_tag, completeness),
        AiOperationEvent::Failed(failure) => failed(state, event_tag, failure),
        AiOperationEvent::CancelRequested => cancel_requested(state, event_tag),
        AiOperationEvent::RuntimeCancelled { partial } => {
            runtime_cancelled(state, event_tag, partial)
        }
        AiOperationEvent::StopRequested => stop_requested(state, event_tag),
        AiOperationEvent::RuntimeStopped { partial } => runtime_stopped(state, event_tag, partial),
        AiOperationEvent::RecoverySelected(action) => recovery_selected(state, event_tag, action),
        AiOperationEvent::RecoveryCommandSucceeded { command_id, result } => {
            recovery_command_succeeded(state, event_tag, command_id, result)
        }
        AiOperationEvent::RecoveryCommandFailed {
            command_id,
            failure,
        } => recovery_command_failed(state, event_tag, command_id, failure),
        AiOperationEvent::BackoffElapsed { command_id } => {
            backoff_elapsed(state, event_tag, command_id)
        }
        AiOperationEvent::RestartObserved(snapshot) => restart_observed(state, event_tag, snapshot),
        AiOperationEvent::SessionReattached(receipt) => {
            session_reattached(state, event_tag, receipt)
        }
        AiOperationEvent::SessionReattachFailed(failure) => {
            session_reattach_failed(state, event_tag, failure)
        }
        AiOperationEvent::SessionReplaced {
            identity,
            selection,
            work,
        } => session_replaced(state, identity, selection, work),
        AiOperationEvent::DismissRequested => dismiss_requested(state, event_tag),
        AiOperationEvent::ResetForNextTurn => reset_for_next_turn(state, event_tag),
    }
}

fn session_replaced(
    mut state: AiOperationState,
    identity: AiSurfaceIdentity,
    selection: AiSelectionState,
    work: AiWorkSnapshot,
) -> Result<AiTransition, InvalidTransition> {
    state.identity = identity;
    state.selection = selection;
    state.work = work;
    state.phase = AiPhase::Ready;
    state.pending = None;
    state.diagnostic = None;
    state.retry.automatic_used = 0;
    state.retry.manual_used = 0;
    Ok(no_commands(state))
}

/// Plan the options a person may choose from after a failure.
///
/// `risk` and `progress` describe how far the failed turn got. They are
/// deliberately NOT consulted here: every option in this plan is chosen by a
/// person, and `automatic_retry_allowed` is where unattended replay is judged
/// against exactly those two values. They stay in the signature because a
/// caller holding them is the caller that has enough context to plan at all,
/// and because a future progress-sensitive option (for example "resume from
/// partial output") belongs in this function rather than at each call site.
pub fn recovery_plan_for(
    identity: &AiSurfaceIdentity,
    failure: &AiFailure,
    retry: RetryLedger,
    _risk: TurnRisk,
    _progress: &ProgressSnapshot,
) -> RecoveryPlan {
    let mut options = match &failure.kind {
        AiFailureKind::Capability(capability) => match capability {
            CapabilityFailure::ClientTooOld { .. } => vec![
                enabled(
                    RecoveryActionKind::ChooseCompatibleModel,
                    RecoveryRole::Primary,
                ),
                enabled(RecoveryActionKind::UpdateClient, RecoveryRole::Secondary),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
            CapabilityFailure::ModelUnavailable { .. } => vec![
                enabled(
                    RecoveryActionKind::ChooseCompatibleModel,
                    RecoveryRole::Primary,
                ),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
            CapabilityFailure::NoCompatibleModel => vec![
                enabled(RecoveryActionKind::UpdateClient, RecoveryRole::Primary),
                enabled(RecoveryActionKind::ChooseProvider, RecoveryRole::Secondary),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
            CapabilityFailure::ProfileUnavailable { .. } => vec![
                enabled(RecoveryActionKind::ChooseProfile, RecoveryRole::Primary),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
        },
        AiFailureKind::Policy(policy) => match policy {
            PolicyFailure::QuickAiSearchBudgetExceeded {
                partial_answer_available,
                source_count,
                ..
            }
            | PolicyFailure::QuickAiDeadlineExceeded {
                partial_answer_available,
                source_count,
                ..
            } => {
                let mut policy_options = vec![enabled(
                    RecoveryActionKind::ContinueInAgentChat,
                    RecoveryRole::Primary,
                )];
                if *partial_answer_available || *source_count > 0 {
                    policy_options.push(enabled(
                        RecoveryActionKind::UseCurrentResults,
                        RecoveryRole::Secondary,
                    ));
                }
                policy_options
            }
            PolicyFailure::ToolDenied { .. } => vec![
                enabled(
                    RecoveryActionKind::ContinueInAgentChat,
                    RecoveryRole::Primary,
                ),
                enabled(RecoveryActionKind::ChooseProfile, RecoveryRole::Secondary),
            ],
        },
        AiFailureKind::Authentication(authentication) => match authentication {
            AuthenticationFailure::Missing | AuthenticationFailure::Expired => vec![
                enabled(RecoveryActionKind::SignIn, RecoveryRole::Primary),
                enabled(RecoveryActionKind::SwitchAccount, RecoveryRole::Secondary),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
            AuthenticationFailure::UsageExhausted => vec![
                enabled(RecoveryActionKind::SwitchAccount, RecoveryRole::Primary),
                enabled(RecoveryActionKind::SignIn, RecoveryRole::Secondary),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
        },
        AiFailureKind::Configuration(configuration) => match configuration {
            ConfigurationFailure::ProviderNotConfigured
            | ConfigurationFailure::InvalidConfiguration => vec![
                enabled(RecoveryActionKind::ConfigureProvider, RecoveryRole::Primary),
                enabled(RecoveryActionKind::ChooseProvider, RecoveryRole::Secondary),
            ],
            ConfigurationFailure::NoModelsAvailable => vec![
                enabled(RecoveryActionKind::ChooseProvider, RecoveryRole::Primary),
                enabled(
                    RecoveryActionKind::ConfigureProvider,
                    RecoveryRole::Secondary,
                ),
            ],
            ConfigurationFailure::SidecarMissing | ConfigurationFailure::MdflowMissing => vec![
                enabled(RecoveryActionKind::RepairComponent, RecoveryRole::Primary),
                manual_retry_option(failure, retry, RecoveryRole::Secondary),
            ],
        },
        AiFailureKind::Connectivity(connectivity) => match connectivity {
            ConnectivityFailure::Offline | ConnectivityFailure::Timeout => vec![
                manual_retry_option(failure, retry, RecoveryRole::Primary),
                enabled(RecoveryActionKind::CheckAgain, RecoveryRole::Secondary),
            ],
            ConnectivityFailure::RateLimited { .. } => {
                vec![manual_retry_option(failure, retry, RecoveryRole::Primary)]
            }
        },
        AiFailureKind::Provider(provider) => match provider {
            ProviderFailure::TemporarilyUnavailable | ProviderFailure::ServerRejected => vec![
                manual_retry_option(failure, retry, RecoveryRole::Primary),
                enabled(RecoveryActionKind::ChooseProvider, RecoveryRole::Secondary),
            ],
        },
        AiFailureKind::Runtime(runtime) => match runtime {
            RuntimeFailure::SpawnFailed
            | RuntimeFailure::RuntimeClosed
            | RuntimeFailure::ChildExited { .. } => {
                match identity {
                    AiSurfaceIdentity::FlowRun { .. } => vec![
                        enabled(RecoveryActionKind::RestartFlowRun, RecoveryRole::Primary),
                        enabled(RecoveryActionKind::RepairComponent, RecoveryRole::Secondary),
                    ],
                    // A flow conversation whose engine died has a repair the
                    // other surfaces do not: start a fresh thread against the
                    // same flow. Without it, an engine death while idle left
                    // Retry as the only move, and retrying a turn on a dead
                    // engine is not what the user needs.
                    AiSurfaceIdentity::FlowConversation { .. } => vec![
                        manual_retry_option(failure, retry, RecoveryRole::Primary),
                        enabled(RecoveryActionKind::RethreadFlow, RecoveryRole::Secondary),
                        enabled(RecoveryActionKind::RepairComponent, RecoveryRole::Secondary),
                    ],
                    _ => vec![
                        manual_retry_option(failure, retry, RecoveryRole::Primary),
                        enabled(RecoveryActionKind::RepairComponent, RecoveryRole::Secondary),
                    ],
                }
            }
            RuntimeFailure::SessionLost { reattach } => match reattach {
                ReattachAvailability::Available { .. } => vec![
                    enabled(RecoveryActionKind::Reattach, RecoveryRole::Primary),
                    flow_rethread_or_retry(identity, failure, retry),
                ],
                ReattachAvailability::Unavailable => {
                    vec![flow_rethread_or_retry(identity, failure, retry)]
                }
            },
        },
        AiFailureKind::Protocol(protocol) => match protocol {
            ProtocolFailure::VersionMismatch { .. }
            | ProtocolFailure::SequenceViolation { .. }
            | ProtocolFailure::OrderViolation { .. }
            | ProtocolFailure::MalformedResponse { .. }
            | ProtocolFailure::MissingTerminal { .. } => vec![
                enabled(RecoveryActionKind::RepairComponent, RecoveryRole::Primary),
                if matches!(identity, AiSurfaceIdentity::FlowConversation { .. }) {
                    enabled(RecoveryActionKind::RethreadFlow, RecoveryRole::Secondary)
                } else {
                    manual_retry_option(failure, retry, RecoveryRole::Secondary)
                },
            ],
        },
        AiFailureKind::Permission(permission) => match permission {
            PermissionFailure::PermissionDenied | PermissionFailure::UserDeniedTool => vec![
                enabled(RecoveryActionKind::ChooseProfile, RecoveryRole::Primary),
                enabled(
                    RecoveryActionKind::ContinueInAgentChat,
                    RecoveryRole::Secondary,
                ),
            ],
        },
        AiFailureKind::Input(input) => match input {
            InputFailure::MessageTooLarge | InputFailure::ContextLimitExceeded => vec![
                enabled(RecoveryActionKind::TrimContext, RecoveryRole::Primary),
                enabled(
                    RecoveryActionKind::ChooseCompatibleModel,
                    RecoveryRole::Secondary,
                ),
            ],
            InputFailure::ContextUnavailable => {
                vec![manual_retry_option(failure, retry, RecoveryRole::Primary)]
            }
            InputFailure::DestinationUnavailable | InputFailure::DestinationStale => vec![
                enabled(RecoveryActionKind::ChooseDestination, RecoveryRole::Primary),
                enabled(RecoveryActionKind::CopyTranscript, RecoveryRole::Secondary),
                enabled(
                    RecoveryActionKind::OpenDictationHistory,
                    RecoveryRole::Secondary,
                ),
            ],
        },
        AiFailureKind::Unknown => vec![manual_retry_option(failure, retry, RecoveryRole::Primary)],
    };

    if failure.diagnostic.is_some() {
        options.push(enabled(
            RecoveryActionKind::CopyDetails,
            RecoveryRole::Diagnostic,
        ));
    }
    RecoveryPlan { options }
}

fn enabled(kind: RecoveryActionKind, role: RecoveryRole) -> RecoveryOption {
    RecoveryOption {
        kind,
        role,
        enabled: true,
        disabled_reason: None,
    }
}

fn disabled(
    kind: RecoveryActionKind,
    role: RecoveryRole,
    reason: DisabledReason,
) -> RecoveryOption {
    RecoveryOption {
        kind,
        role,
        enabled: false,
        disabled_reason: Some(reason),
    }
}

/// Plan the MANUAL Retry option — the button a person presses.
///
/// Automatic replay is decided separately by `automatic_retry_allowed`, which
/// applies `ProgressSnapshot::permits_automatic_replay` itself. Applying that
/// same predicate here too made the ordering nonsensical:
/// `SameSelectionReadOnly` — the safest category, meaning "replaying with the
/// same selection is read-only" — came out LESS retryable than
/// `ExplicitUserConfirmation`, whose whole meaning is "ask the user first".
/// On any `TurnRisk::MayMutate` surface (every Flow turn) that disabled the
/// primary button on the recovery card and left the user with nothing to press.
///
/// A manual press IS the explicit confirmation. So manual Retry is offered
/// whenever the failure category permits replay at all, and refused only for
/// the categories that mean "replay cannot work" (`ReconnectOnly`) or "replay
/// must never happen" (`Never`).
fn manual_retry_option(
    failure: &AiFailure,
    retry: RetryLedger,
    role: RecoveryRole,
) -> RecoveryOption {
    if !retry.manual_available() {
        return disabled(
            RecoveryActionKind::Retry,
            role,
            DisabledReason::RetryBudgetExhausted,
        );
    }
    let safe = match failure.retry_safety {
        RetrySafety::SameSelectionReadOnly | RetrySafety::ExplicitUserConfirmation => true,
        RetrySafety::ReconnectOnly | RetrySafety::Never => false,
    };
    if safe {
        enabled(RecoveryActionKind::Retry, role)
    } else {
        disabled(
            RecoveryActionKind::Retry,
            role,
            DisabledReason::UnsafeToReplay,
        )
    }
}

fn flow_rethread_or_retry(
    identity: &AiSurfaceIdentity,
    failure: &AiFailure,
    retry: RetryLedger,
) -> RecoveryOption {
    match identity {
        AiSurfaceIdentity::FlowConversation { .. } => {
            enabled(RecoveryActionKind::RethreadFlow, RecoveryRole::Secondary)
        }
        AiSurfaceIdentity::FlowRun { .. } => {
            enabled(RecoveryActionKind::RestartFlowRun, RecoveryRole::Secondary)
        }
        AiSurfaceIdentity::QuickAi { .. }
        | AiSurfaceIdentity::AgentChat { .. }
        | AiSurfaceIdentity::LegacyChatPrompt { .. }
        | AiSurfaceIdentity::FocusedText { .. }
        | AiSurfaceIdentity::Other { .. } => {
            manual_retry_option(failure, retry, RecoveryRole::Secondary)
        }
    }
}

fn submit_requested(
    mut state: AiOperationState,
    event: AiEventTag,
    request: TurnRequestRef,
    work: AiWorkSnapshot,
    selection: AiSelectionState,
    risk: TurnRisk,
) -> Result<AiTransition, InvalidTransition> {
    if state.phase.tag() != AiPhaseTag::Ready {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    let command_id = state.take_command_id();
    state.phase = AiPhase::Preflighting {
        request: request.clone(),
    };
    state.selection = selection;
    state.work = work.clone();
    state.pending = Some(PendingTurn {
        request: request.clone(),
        risk,
        start_command_id: None,
    });
    state.diagnostic = None;
    Ok(AiTransition {
        commands: vec![
            AiCommand::PersistWork(work),
            AiCommand::CheckCapabilities(CapabilityRequest {
                command_id,
                identity: state.identity.clone(),
                selection: state.selection.clone(),
                request,
            }),
        ],
        next: state,
        outcome: None,
    })
}

fn capability_resolved(
    mut state: AiOperationState,
    event: AiEventTag,
    decision: CapabilityDecision,
) -> Result<AiTransition, InvalidTransition> {
    if state.phase.tag() != AiPhaseTag::Preflighting {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    match decision {
        CapabilityDecision::Blocked(failure) => {
            let risk = pending_risk(&state)?;
            let plan = recovery_plan_for(
                &state.identity,
                &failure,
                state.retry,
                risk,
                &ProgressSnapshot::none(),
            );
            state.diagnostic = failure.diagnostic.clone();
            state.phase = AiPhase::AwaitingRecovery { failure, plan };
            Ok(no_commands(state))
        }
        CapabilityDecision::Compatible | CapabilityDecision::Unknown => {
            if !state.selection.can_start_turn() {
                return invalid(
                    &state,
                    event,
                    InvalidTransitionReason::UnacknowledgedSelection,
                );
            }
            let pending = state.pending.clone().ok_or_else(|| {
                invalid_value(&state, event, InvalidTransitionReason::MissingPendingTurn)
            })?;
            let command_id = state.take_command_id();
            if let Some(pending_mut) = state.pending.as_mut() {
                pending_mut.start_command_id = Some(command_id);
            }
            Ok(AiTransition {
                commands: vec![
                    AiCommand::PersistWork(state.work.clone()),
                    AiCommand::StartTurn(StartTurnCommand {
                        command_id,
                        request: pending.request,
                        selection: state.selection.clone(),
                        work_key: state.work.key.clone(),
                        risk: pending.risk,
                    }),
                ],
                next: state,
                outcome: None,
            })
        }
    }
}

fn runtime_started(
    mut state: AiOperationState,
    event: AiEventTag,
    command_id: CommandId,
    turn: TurnRef,
) -> Result<AiTransition, InvalidTransition> {
    if state.phase.tag() != AiPhaseTag::Preflighting {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    let pending = state
        .pending
        .as_ref()
        .ok_or_else(|| invalid_value(&state, event, InvalidTransitionReason::MissingPendingTurn))?;
    if pending.start_command_id != Some(command_id) {
        return invalid(&state, event, InvalidTransitionReason::CommandIdMismatch);
    }
    state.phase = AiPhase::Running {
        turn,
        risk: pending.risk,
        progress: ProgressSnapshot::none(),
    };
    Ok(no_commands(state))
}

fn progressed(
    mut state: AiOperationState,
    event: AiEventTag,
    progress: ProgressSnapshot,
    work: AiWorkSnapshot,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Running { turn, risk, .. } = state.phase.clone() else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    state.phase = AiPhase::Running {
        turn,
        risk,
        progress,
    };
    state.work = work;
    Ok(no_commands(state))
}

fn completed(
    mut state: AiOperationState,
    event: AiEventTag,
    completeness: CompletionKind,
) -> Result<AiTransition, InvalidTransition> {
    if state.phase.tag() != AiPhaseTag::Running {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    state.phase = AiPhase::Succeeded {
        completeness,
        recovered_from: None,
    };
    state.pending = None;
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::Succeeded { completeness }),
    })
}

fn failed(
    mut state: AiOperationState,
    event: AiEventTag,
    failure: AiFailure,
) -> Result<AiTransition, InvalidTransition> {
    let (risk, progress) = match state.phase.clone() {
        AiPhase::Running { risk, progress, .. } => (risk, progress),
        AiPhase::Preflighting { .. } => (pending_risk(&state)?, ProgressSnapshot::none()),
        AiPhase::Ready
        | AiPhase::Cancelling { .. }
        | AiPhase::AwaitingRecovery { .. }
        | AiPhase::Recovering { .. }
        | AiPhase::Recovered { .. }
        | AiPhase::Succeeded { .. }
        | AiPhase::Cancelled { .. }
        | AiPhase::Dismissed { .. } => {
            return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
        }
    };
    state.diagnostic = failure.diagnostic.clone();
    let plan = recovery_plan_for(&state.identity, &failure, state.retry, risk, &progress);
    if automatic_retry_allowed(&state, &failure, risk, &progress) {
        state.retry.automatic_used = state.retry.automatic_used.saturating_add(1);
        let command_id = state.take_command_id();
        state.phase = AiPhase::Recovering {
            action: AiRecoveryAction::Retry,
            command_id,
            origin: RecoveryOrigin::Failure {
                failure: failure.clone(),
                plan: plan.clone(),
            },
        };
        return Ok(AiTransition {
            commands: vec![
                AiCommand::PersistWork(state.work.clone()),
                AiCommand::ScheduleBackoff {
                    command_id,
                    attempt: RetryAttempt::Automatic(state.retry.automatic_used),
                    class: backoff_class(&failure),
                },
            ],
            next: state,
            outcome: None,
        });
    }
    state.phase = AiPhase::AwaitingRecovery { failure, plan };
    Ok(no_commands(state))
}

fn automatic_retry_allowed(
    state: &AiOperationState,
    failure: &AiFailure,
    risk: TurnRisk,
    progress: &ProgressSnapshot,
) -> bool {
    let surface_safe = match state.identity {
        AiSurfaceIdentity::QuickAi { .. }
        | AiSurfaceIdentity::AgentChat { .. }
        | AiSurfaceIdentity::LegacyChatPrompt { .. }
        | AiSurfaceIdentity::FocusedText { .. }
        | AiSurfaceIdentity::Other { .. } => true,
        AiSurfaceIdentity::FlowConversation { .. } | AiSurfaceIdentity::FlowRun { .. } => false,
    };
    let retry_safe = match failure.retry_safety {
        RetrySafety::SameSelectionReadOnly => true,
        RetrySafety::ReconnectOnly | RetrySafety::ExplicitUserConfirmation | RetrySafety::Never => {
            false
        }
    };
    surface_safe
        && retry_safe
        && state.retry.automatic_available()
        && progress.permits_automatic_replay(risk)
        && state.selection.can_start_turn()
}

fn backoff_class(failure: &AiFailure) -> BackoffClass {
    match failure.code {
        AiFailureCode::Offline | AiFailureCode::Timeout => BackoffClass::Network,
        AiFailureCode::RateLimited => match failure.kind {
            AiFailureKind::Connectivity(ConnectivityFailure::RateLimited { retry_after_ms }) => {
                BackoffClass::RateLimit { retry_after_ms }
            }
            AiFailureKind::Capability(_)
            | AiFailureKind::Policy(_)
            | AiFailureKind::Authentication(_)
            | AiFailureKind::Configuration(_)
            | AiFailureKind::Connectivity(ConnectivityFailure::Offline)
            | AiFailureKind::Connectivity(ConnectivityFailure::Timeout)
            | AiFailureKind::Provider(_)
            | AiFailureKind::Runtime(_)
            | AiFailureKind::Protocol(_)
            | AiFailureKind::Permission(_)
            | AiFailureKind::Input(_)
            | AiFailureKind::Unknown => BackoffClass::Network,
        },
        AiFailureCode::ProviderTemporarilyUnavailable | AiFailureCode::ProviderServerRejected => {
            BackoffClass::Provider
        }
        AiFailureCode::ClientTooOld
        | AiFailureCode::ModelUnavailable
        | AiFailureCode::NoCompatibleModel
        | AiFailureCode::ProfileUnavailable
        | AiFailureCode::QuickAiSearchBudgetExceeded
        | AiFailureCode::QuickAiDeadlineExceeded
        | AiFailureCode::ToolDenied
        | AiFailureCode::AuthenticationMissing
        | AiFailureCode::AuthenticationExpired
        | AiFailureCode::UsageExhausted
        | AiFailureCode::ProviderNotConfigured
        | AiFailureCode::NoModelsAvailable
        | AiFailureCode::SidecarMissing
        | AiFailureCode::MdflowMissing
        | AiFailureCode::InvalidConfiguration
        | AiFailureCode::SpawnFailed
        | AiFailureCode::RuntimeClosed
        | AiFailureCode::ChildExited
        | AiFailureCode::SessionLost
        | AiFailureCode::ProtocolVersionMismatch
        | AiFailureCode::ProtocolSequenceViolation
        | AiFailureCode::ProtocolOrderViolation
        | AiFailureCode::ProtocolMalformedResponse
        | AiFailureCode::ProtocolMissingTerminal
        | AiFailureCode::PermissionDenied
        | AiFailureCode::UserDeniedTool
        | AiFailureCode::MessageTooLarge
        | AiFailureCode::ContextLimitExceeded
        | AiFailureCode::ContextUnavailable
        | AiFailureCode::DestinationUnavailable
        | AiFailureCode::DestinationStale
        | AiFailureCode::Unknown => BackoffClass::Immediate,
    }
}

fn cancel_requested(
    mut state: AiOperationState,
    event: AiEventTag,
) -> Result<AiTransition, InvalidTransition> {
    match state.phase.clone() {
        AiPhase::Running { turn, progress, .. } => {
            let partial = partial_output(&state.work, &progress);
            state.phase = AiPhase::Cancelling {
                turn: turn.clone(),
                partial,
                kind: CancellationKind::UserCancelled,
            };
            Ok(AiTransition {
                commands: vec![AiCommand::CancelTurn { turn }],
                next: state,
                outcome: None,
            })
        }
        AiPhase::Preflighting { .. } | AiPhase::Recovering { .. } => {
            let partial = partial_from_work(&state.work);
            state.phase = AiPhase::Cancelled {
                kind: CancellationKind::UserCancelled,
                partial: partial.clone(),
            };
            Ok(AiTransition {
                next: state,
                commands: Vec::new(),
                outcome: Some(AiOutcome::Cancelled {
                    kind: CancellationKind::UserCancelled,
                    partial,
                }),
            })
        }
        AiPhase::Ready
        | AiPhase::Cancelling { .. }
        | AiPhase::AwaitingRecovery { .. }
        | AiPhase::Recovered { .. }
        | AiPhase::Succeeded { .. }
        | AiPhase::Cancelled { .. }
        | AiPhase::Dismissed { .. } => {
            invalid(&state, event, InvalidTransitionReason::EventNotAllowed)
        }
    }
}

fn runtime_cancelled(
    state: AiOperationState,
    event: AiEventTag,
    partial: PartialOutputState,
) -> Result<AiTransition, InvalidTransition> {
    settle_runtime_cancellation(state, event, partial, CancellationKind::UserCancelled)
}

fn stop_requested(
    mut state: AiOperationState,
    event: AiEventTag,
) -> Result<AiTransition, InvalidTransition> {
    match state.phase.clone() {
        AiPhase::Running { turn, progress, .. } => {
            let partial = partial_output(&state.work, &progress);
            state.phase = AiPhase::Cancelling {
                turn: turn.clone(),
                partial,
                kind: CancellationKind::UserStopped,
            };
            Ok(AiTransition {
                commands: vec![AiCommand::CancelTurn { turn }],
                next: state,
                outcome: None,
            })
        }
        AiPhase::Preflighting { .. } | AiPhase::Recovering { .. } => {
            settle_immediate_cancellation(state, CancellationKind::UserStopped)
        }
        AiPhase::Ready
        | AiPhase::Cancelling { .. }
        | AiPhase::AwaitingRecovery { .. }
        | AiPhase::Recovered { .. }
        | AiPhase::Succeeded { .. }
        | AiPhase::Cancelled { .. }
        | AiPhase::Dismissed { .. } => {
            invalid(&state, event, InvalidTransitionReason::EventNotAllowed)
        }
    }
}

fn runtime_stopped(
    state: AiOperationState,
    event: AiEventTag,
    partial: PartialOutputState,
) -> Result<AiTransition, InvalidTransition> {
    settle_runtime_cancellation(state, event, partial, CancellationKind::UserStopped)
}

fn settle_immediate_cancellation(
    mut state: AiOperationState,
    kind: CancellationKind,
) -> Result<AiTransition, InvalidTransition> {
    let partial = partial_from_work(&state.work);
    state.phase = AiPhase::Cancelled {
        kind,
        partial: partial.clone(),
    };
    state.pending = None;
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::Cancelled { kind, partial }),
    })
}

fn settle_runtime_cancellation(
    mut state: AiOperationState,
    event: AiEventTag,
    partial: PartialOutputState,
    expected_kind: CancellationKind,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Cancelling { kind, .. } = state.phase.clone() else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    if kind != expected_kind {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    state.phase = AiPhase::Cancelled {
        kind,
        partial: partial.clone(),
    };
    state.pending = None;
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::Cancelled { kind, partial }),
    })
}

fn recovery_selected(
    mut state: AiOperationState,
    event: AiEventTag,
    action: AiRecoveryAction,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::AwaitingRecovery { failure, plan } = state.phase.clone() else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    let Some(option) = plan.option(action.kind()) else {
        return invalid(
            &state,
            event,
            InvalidTransitionReason::RecoveryActionUnavailable,
        );
    };
    if !option.enabled {
        let reason = match option.disabled_reason {
            Some(DisabledReason::RetryBudgetExhausted) => {
                InvalidTransitionReason::RetryBudgetExhausted
            }
            Some(DisabledReason::UnsafeToReplay) => InvalidTransitionReason::UnsafeReplay,
            Some(DisabledReason::NoCompatibleSelection)
            | Some(DisabledReason::MissingSession)
            | Some(DisabledReason::UnsupportedBySurface)
            | Some(DisabledReason::WaitingForBackoff)
            | None => InvalidTransitionReason::RecoveryActionDisabled,
        };
        return invalid(&state, event, reason);
    }

    match action.clone() {
        AiRecoveryAction::UseCurrentResults => {
            state.phase = AiPhase::Succeeded {
                completeness: CompletionKind::Partial,
                recovered_from: Some(failure.code),
            };
            state.pending = None;
            Ok(AiTransition {
                next: state,
                commands: Vec::new(),
                outcome: Some(AiOutcome::Succeeded {
                    completeness: CompletionKind::Partial,
                }),
            })
        }
        AiRecoveryAction::CopyDetails => {
            let Some(diagnostic) = state.diagnostic.clone() else {
                return invalid(&state, event, InvalidTransitionReason::MissingDiagnostic);
            };
            Ok(AiTransition {
                next: state,
                commands: vec![AiCommand::CopyRedactedDiagnostics(diagnostic.id)],
                outcome: None,
            })
        }
        AiRecoveryAction::Retry => {
            if !state.retry.manual_available() {
                return invalid(&state, event, InvalidTransitionReason::RetryBudgetExhausted);
            }
            state.retry.manual_used = state.retry.manual_used.saturating_add(1);
            let command_id = state.take_command_id();
            let class = backoff_class(&failure);
            state.phase = AiPhase::Recovering {
                action,
                command_id,
                origin: RecoveryOrigin::Failure { failure, plan },
            };
            Ok(AiTransition {
                commands: vec![
                    AiCommand::PersistWork(state.work.clone()),
                    AiCommand::ScheduleBackoff {
                        command_id,
                        attempt: RetryAttempt::Manual(state.retry.manual_used),
                        class,
                    },
                ],
                next: state,
                outcome: None,
            })
        }
        AiRecoveryAction::ChooseCompatibleModel { selection }
        | AiRecoveryAction::ChooseProvider { selection }
        | AiRecoveryAction::ChooseProfile { selection } => {
            let command_id = state.take_command_id();
            let command = match selection {
                Some(requested) => AiCommand::ApplySelection(ExplicitSelectionChange {
                    command_id,
                    requested,
                    origin: SelectionOrigin::RecoveryChoice,
                }),
                None => AiCommand::OpenConfiguration(ConfigurationTarget {
                    command_id,
                    kind: match action {
                        AiRecoveryAction::ChooseCompatibleModel { .. } => {
                            ConfigurationTargetKind::Model
                        }
                        AiRecoveryAction::ChooseProvider { .. } => {
                            ConfigurationTargetKind::Provider
                        }
                        AiRecoveryAction::ChooseProfile { .. } => ConfigurationTargetKind::Profile,
                        AiRecoveryAction::Retry
                        | AiRecoveryAction::UseCurrentResults
                        | AiRecoveryAction::ContinueInAgentChat
                        | AiRecoveryAction::UpdateClient { .. }
                        | AiRecoveryAction::CheckAgain
                        | AiRecoveryAction::SignIn
                        | AiRecoveryAction::SwitchAccount
                        | AiRecoveryAction::ConfigureProvider
                        | AiRecoveryAction::RepairComponent { .. }
                        | AiRecoveryAction::Reattach { .. }
                        | AiRecoveryAction::RethreadFlow
                        | AiRecoveryAction::RestartFlowRun
                        | AiRecoveryAction::TrimContext
                        | AiRecoveryAction::RetrySameDestination
                        | AiRecoveryAction::ChooseDestination
                        | AiRecoveryAction::CopyTranscript
                        | AiRecoveryAction::OpenDictationHistory
                        | AiRecoveryAction::CopyDetails => ConfigurationTargetKind::Model,
                    },
                }),
            };
            state.phase = AiPhase::Recovering {
                action,
                command_id,
                origin: RecoveryOrigin::Failure { failure, plan },
            };
            Ok(AiTransition {
                next: state,
                commands: vec![command],
                outcome: None,
            })
        }
        AiRecoveryAction::ContinueInAgentChat => {
            start_recovery_command(state, action, failure, plan, |command_id, state| {
                AiCommand::ContinueInAgentChat(AgentChatEscalation {
                    command_id,
                    work: state.work.clone(),
                })
            })
        }
        AiRecoveryAction::UpdateClient { client } => {
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::OpenClientUpdate(ClientUpdateTarget { command_id, client })
            })
        }
        AiRecoveryAction::CheckAgain => {
            start_recovery_command(state, action, failure, plan, |command_id, state| {
                AiCommand::RecheckClientCapability(ClientCapabilityKey {
                    command_id,
                    identity: state.identity.clone(),
                })
            })
        }
        AiRecoveryAction::SignIn => {
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::LaunchAuthentication(AuthRecoveryCommand {
                    command_id,
                    mode: AuthRecoveryMode::SignIn,
                })
            })
        }
        AiRecoveryAction::SwitchAccount => {
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::LaunchAuthentication(AuthRecoveryCommand {
                    command_id,
                    mode: AuthRecoveryMode::SwitchAccount,
                })
            })
        }
        AiRecoveryAction::ConfigureProvider => {
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::OpenConfiguration(ConfigurationTarget {
                    command_id,
                    kind: ConfigurationTargetKind::Provider,
                })
            })
        }
        AiRecoveryAction::RepairComponent { component } => {
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::InstallOrRepairComponent(ComponentRecoveryCommand {
                    command_id,
                    component,
                })
            })
        }
        AiRecoveryAction::Reattach { session } => {
            start_recovery_command(state, action, failure, plan, |_command_id, _state| {
                AiCommand::ReattachSession(session)
            })
        }
        AiRecoveryAction::RethreadFlow => {
            let Some(flow_id) = flow_id(&state.identity) else {
                return invalid(
                    &state,
                    event,
                    InvalidTransitionReason::RecoveryActionUnavailable,
                );
            };
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::RethreadFlow(RethreadFlowCommand {
                    command_id,
                    flow_id,
                })
            })
        }
        AiRecoveryAction::RestartFlowRun => {
            let Some((flow_id, previous_run_id)) = flow_run_identity(&state.identity) else {
                return invalid(
                    &state,
                    event,
                    InvalidTransitionReason::RecoveryActionUnavailable,
                );
            };
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::RestartFlowRun(RestartFlowRunCommand {
                    command_id,
                    flow_id,
                    previous_run_id,
                })
            })
        }
        AiRecoveryAction::TrimContext => {
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::OpenConfiguration(ConfigurationTarget {
                    command_id,
                    kind: ConfigurationTargetKind::Context,
                })
            })
        }
        AiRecoveryAction::RetrySameDestination
        | AiRecoveryAction::ChooseDestination
        | AiRecoveryAction::CopyTranscript
        | AiRecoveryAction::OpenDictationHistory => {
            let surface_action = action.kind();
            start_recovery_command(state, action, failure, plan, |command_id, _state| {
                AiCommand::RunSurfaceRecovery {
                    command_id,
                    action: surface_action,
                }
            })
        }
    }
}

fn start_recovery_command(
    mut state: AiOperationState,
    action: AiRecoveryAction,
    failure: AiFailure,
    plan: RecoveryPlan,
    command: impl FnOnce(CommandId, &AiOperationState) -> AiCommand,
) -> Result<AiTransition, InvalidTransition> {
    let command_id = state.take_command_id();
    let command = command(command_id, &state);
    state.phase = AiPhase::Recovering {
        action,
        command_id,
        origin: RecoveryOrigin::Failure { failure, plan },
    };
    Ok(AiTransition {
        next: state,
        commands: vec![command],
        outcome: None,
    })
}

fn recovery_command_succeeded(
    mut state: AiOperationState,
    event: AiEventTag,
    command_id: CommandId,
    result: RecoveryEffectResult,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Recovering {
        action,
        command_id: expected,
        origin,
    } = state.phase.clone()
    else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    if expected != command_id {
        return invalid(&state, event, InvalidTransitionReason::CommandIdMismatch);
    }
    if matches!(result, RecoveryEffectResult::ExternalActionLaunched) {
        let RecoveryOrigin::Failure { failure, plan } = origin else {
            return invalid(
                &state,
                event,
                InvalidTransitionReason::RecoveryActionUnavailable,
            );
        };
        state.phase = AiPhase::AwaitingRecovery { failure, plan };
        return Ok(no_commands(state));
    }
    let summary = match result {
        RecoveryEffectResult::SelectionApplied(receipt) => {
            state.selection.requested = Some(receipt.applied.clone());
            state.selection.effective = Some(receipt.applied.clone());
            state.selection.origin = receipt.origin;
            state.selection.acknowledged_change = Some(receipt);
            RecoverySuccess::SelectionApplied
        }
        RecoveryEffectResult::AuthenticationReady => RecoverySuccess::AuthenticationReady,
        RecoveryEffectResult::ConfigurationChanged => RecoverySuccess::ConfigurationChanged,
        RecoveryEffectResult::ClientUpdated => RecoverySuccess::ClientUpdated,
        RecoveryEffectResult::CapabilityRechecked => RecoverySuccess::ReadyToRetry,
        RecoveryEffectResult::ComponentReady => RecoverySuccess::ComponentReady,
        RecoveryEffectResult::FlowRethreaded => RecoverySuccess::FlowRethreaded,
        RecoveryEffectResult::FlowRunRestarted { run_id } => {
            state.identity = with_flow_run_id(state.identity, run_id);
            RecoverySuccess::FlowRunRestarted
        }
        RecoveryEffectResult::AgentChatOpened => RecoverySuccess::AgentChatOpened,
        RecoveryEffectResult::ContextTrimmed => RecoverySuccess::ContextTrimmed,
        RecoveryEffectResult::ExternalActionLaunched => unreachable!("handled above"),
        RecoveryEffectResult::NoChange => RecoverySuccess::ReadyToRetry,
    };
    state.phase = AiPhase::Recovered {
        action: action.clone(),
        summary,
    };
    Ok(AiTransition {
        next: state,
        commands: vec![AiCommand::ScheduleRecoveredDismiss],
        outcome: Some(AiOutcome::RecoverySucceeded {
            action: action.kind(),
        }),
    })
}

fn recovery_command_failed(
    mut state: AiOperationState,
    event: AiEventTag,
    command_id: CommandId,
    failure: AiFailure,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Recovering {
        command_id: expected,
        ..
    } = state.phase.clone()
    else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    if expected != command_id {
        return invalid(&state, event, InvalidTransitionReason::CommandIdMismatch);
    }
    let risk = pending_risk(&state).unwrap_or(TurnRisk::MayMutate);
    let plan = recovery_plan_for(
        &state.identity,
        &failure,
        state.retry,
        risk,
        &ProgressSnapshot::none(),
    );
    state.diagnostic = failure.diagnostic.clone();
    state.phase = AiPhase::AwaitingRecovery {
        failure: failure.clone(),
        plan,
    };
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::RecoveryFailed {
            failure: failure.code,
        }),
    })
}

fn backoff_elapsed(
    state: AiOperationState,
    event: AiEventTag,
    command_id: CommandId,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Recovering {
        action,
        command_id: expected,
        ..
    } = state.phase.clone()
    else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    if !matches!(action, AiRecoveryAction::Retry) {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    if expected != command_id {
        return invalid(&state, event, InvalidTransitionReason::CommandIdMismatch);
    }
    enter_preflight(state, event)
}

fn restart_observed(
    mut state: AiOperationState,
    _event: AiEventTag,
    snapshot: RestartSnapshot,
) -> Result<AiTransition, InvalidTransition> {
    match snapshot.session {
        Some(session) => {
            let command_id = state.take_command_id();
            state.phase = AiPhase::Recovering {
                action: AiRecoveryAction::Reattach {
                    session: session.clone(),
                },
                command_id,
                origin: RecoveryOrigin::Restart,
            };
            let commands = vec![
                AiCommand::PersistWork(state.work.clone()),
                AiCommand::ReattachSession(session),
            ];
            Ok(AiTransition {
                next: state,
                commands,
                outcome: None,
            })
        }
        None => {
            let failure = AiFailure::new(
                AiFailureKind::Runtime(RuntimeFailure::SessionLost {
                    reattach: ReattachAvailability::Unavailable,
                }),
                RetrySafety::ExplicitUserConfirmation,
            );
            let risk = pending_risk(&state).unwrap_or(TurnRisk::MayMutate);
            let plan = recovery_plan_for(
                &state.identity,
                &failure,
                state.retry,
                risk,
                &ProgressSnapshot::none(),
            );
            state.phase = AiPhase::AwaitingRecovery { failure, plan };
            Ok(no_commands(state))
        }
    }
}

fn session_reattached(
    mut state: AiOperationState,
    event: AiEventTag,
    receipt: ReattachReceipt,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Recovering { action, .. } = state.phase.clone() else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    if !matches!(action, AiRecoveryAction::Reattach { .. }) {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    state.phase = AiPhase::Running {
        turn: receipt.turn,
        risk: receipt.risk,
        progress: receipt.progress,
    };
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::RecoverySucceeded {
            action: RecoveryActionKind::Reattach,
        }),
    })
}

fn session_reattach_failed(
    mut state: AiOperationState,
    event: AiEventTag,
    failure: AiFailure,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::Recovering { action, .. } = state.phase.clone() else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    if !matches!(action, AiRecoveryAction::Reattach { .. }) {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    }
    let risk = pending_risk(&state).unwrap_or(TurnRisk::MayMutate);
    let plan = recovery_plan_for(
        &state.identity,
        &failure,
        state.retry,
        risk,
        &ProgressSnapshot::none(),
    );
    state.phase = AiPhase::AwaitingRecovery {
        failure: failure.clone(),
        plan,
    };
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::RecoveryFailed {
            failure: failure.code,
        }),
    })
}

fn dismiss_requested(
    mut state: AiOperationState,
    event: AiEventTag,
) -> Result<AiTransition, InvalidTransition> {
    let AiPhase::AwaitingRecovery { failure, .. } = state.phase.clone() else {
        return invalid(&state, event, InvalidTransitionReason::EventNotAllowed);
    };
    state.phase = AiPhase::Dismissed {
        failure: failure.code,
    };
    Ok(AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: Some(AiOutcome::Dismissed {
            failure: failure.code,
        }),
    })
}

fn reset_for_next_turn(
    mut state: AiOperationState,
    event: AiEventTag,
) -> Result<AiTransition, InvalidTransition> {
    match state.phase.tag() {
        AiPhaseTag::Recovered if state.pending.is_some() => enter_preflight(state, event),
        AiPhaseTag::Recovered
        | AiPhaseTag::Succeeded
        | AiPhaseTag::Cancelled
        | AiPhaseTag::Dismissed => {
            state.phase = AiPhase::Ready;
            state.pending = None;
            state.diagnostic = None;
            state.retry.automatic_used = 0;
            state.retry.manual_used = 0;
            Ok(no_commands(state))
        }
        AiPhaseTag::Ready
        | AiPhaseTag::Preflighting
        | AiPhaseTag::Running
        | AiPhaseTag::Cancelling
        | AiPhaseTag::AwaitingRecovery
        | AiPhaseTag::Recovering => {
            invalid(&state, event, InvalidTransitionReason::EventNotAllowed)
        }
    }
}

fn enter_preflight(
    mut state: AiOperationState,
    event: AiEventTag,
) -> Result<AiTransition, InvalidTransition> {
    let pending = state
        .pending
        .clone()
        .ok_or_else(|| invalid_value(&state, event, InvalidTransitionReason::MissingPendingTurn))?;
    let command_id = state.take_command_id();
    if let Some(pending_mut) = state.pending.as_mut() {
        pending_mut.start_command_id = None;
    }
    state.phase = AiPhase::Preflighting {
        request: pending.request.clone(),
    };
    Ok(AiTransition {
        commands: vec![
            AiCommand::PersistWork(state.work.clone()),
            AiCommand::CheckCapabilities(CapabilityRequest {
                command_id,
                identity: state.identity.clone(),
                selection: state.selection.clone(),
                request: pending.request,
            }),
        ],
        next: state,
        outcome: None,
    })
}

fn pending_risk(state: &AiOperationState) -> Result<TurnRisk, InvalidTransition> {
    state
        .pending
        .as_ref()
        .map(|pending| pending.risk)
        .ok_or_else(|| {
            invalid_value(
                state,
                AiEventTag::Failed,
                InvalidTransitionReason::MissingPendingTurn,
            )
        })
}

fn partial_output(work: &AiWorkSnapshot, progress: &ProgressSnapshot) -> PartialOutputState {
    if progress.partial_output_available {
        partial_from_work(work)
    } else {
        PartialOutputState::None
    }
}

fn partial_from_work(work: &AiWorkSnapshot) -> PartialOutputState {
    match &work.partial_output {
        PreservationReceipt::Preserved { fingerprint }
        | PreservationReceipt::Restorable { fingerprint } => PartialOutputState::Preserved {
            fingerprint: fingerprint.clone(),
        },
        PreservationReceipt::NotApplicable | PreservationReceipt::Missing { .. } => {
            PartialOutputState::None
        }
    }
}

fn flow_id(identity: &AiSurfaceIdentity) -> Option<FlowId> {
    match identity {
        AiSurfaceIdentity::FlowConversation { flow_id, .. }
        | AiSurfaceIdentity::FlowRun { flow_id, .. } => Some(flow_id.clone()),
        AiSurfaceIdentity::QuickAi { .. }
        | AiSurfaceIdentity::AgentChat { .. }
        | AiSurfaceIdentity::LegacyChatPrompt { .. }
        | AiSurfaceIdentity::FocusedText { .. }
        | AiSurfaceIdentity::Other { .. } => None,
    }
}

fn flow_run_identity(identity: &AiSurfaceIdentity) -> Option<(FlowId, Option<RunId>)> {
    match identity {
        AiSurfaceIdentity::FlowRun {
            flow_id, run_id, ..
        } => Some((flow_id.clone(), run_id.clone())),
        AiSurfaceIdentity::QuickAi { .. }
        | AiSurfaceIdentity::AgentChat { .. }
        | AiSurfaceIdentity::FlowConversation { .. }
        | AiSurfaceIdentity::LegacyChatPrompt { .. }
        | AiSurfaceIdentity::FocusedText { .. }
        | AiSurfaceIdentity::Other { .. } => None,
    }
}

fn with_flow_run_id(identity: AiSurfaceIdentity, run_id: RunId) -> AiSurfaceIdentity {
    match identity {
        AiSurfaceIdentity::FlowRun {
            flow_id, engine_id, ..
        } => AiSurfaceIdentity::FlowRun {
            flow_id,
            engine_id,
            run_id: Some(run_id),
        },
        AiSurfaceIdentity::QuickAi {
            profile_id,
            provider_id,
            model_id,
        } => AiSurfaceIdentity::QuickAi {
            profile_id,
            provider_id,
            model_id,
        },
        AiSurfaceIdentity::AgentChat {
            profile_id,
            provider_id,
            model_id,
            cwd_fingerprint,
        } => AiSurfaceIdentity::AgentChat {
            profile_id,
            provider_id,
            model_id,
            cwd_fingerprint,
        },
        AiSurfaceIdentity::FlowConversation {
            flow_id,
            definition_fingerprint,
            engine_id,
            provider_id,
            model_id,
        } => AiSurfaceIdentity::FlowConversation {
            flow_id,
            definition_fingerprint,
            engine_id,
            provider_id,
            model_id,
        },
        AiSurfaceIdentity::LegacyChatPrompt {
            prompt_id,
            provider_id,
            model_id,
        } => AiSurfaceIdentity::LegacyChatPrompt {
            prompt_id,
            provider_id,
            model_id,
        },
        AiSurfaceIdentity::FocusedText {
            profile_id,
            provider_id,
            model_id,
        } => AiSurfaceIdentity::FocusedText {
            profile_id,
            provider_id,
            model_id,
        },
        AiSurfaceIdentity::Other {
            integration_id,
            provider_id,
            model_id,
        } => AiSurfaceIdentity::Other {
            integration_id,
            provider_id,
            model_id,
        },
    }
}

fn no_commands(state: AiOperationState) -> AiTransition {
    AiTransition {
        next: state,
        commands: Vec::new(),
        outcome: None,
    }
}

fn invalid<T>(
    state: &AiOperationState,
    event: AiEventTag,
    reason: InvalidTransitionReason,
) -> Result<T, InvalidTransition> {
    Err(invalid_value(state, event, reason))
}

fn invalid_value(
    state: &AiOperationState,
    event: AiEventTag,
    reason: InvalidTransitionReason,
) -> InvalidTransition {
    InvalidTransition {
        phase: state.phase.tag(),
        event,
        reason,
    }
}
