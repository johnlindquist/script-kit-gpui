use crate::protocol::{
    AiReliabilityDiagnosticSnapshot, AiReliabilityIdentitySnapshot,
    AiReliabilityPreservationSnapshot, AiReliabilityRetrySnapshot, AiReliabilityStateSnapshot,
    AiReliabilityTransitionSnapshot, AutomationWindowKind,
};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use sk_protocol::ai_reliability::{
    AiFailure, AiFailureKind, AiOperationState, AiPhase, AiSelectionState, AiSurfaceIdentity,
    DiagnosticAvailability, PreservationReceipt, RecoveryRole,
};
use std::collections::HashMap;
use std::sync::OnceLock;

static FIXTURES: OnceLock<Mutex<HashMap<String, AiReliabilityStateSnapshot>>> = OnceLock::new();

fn fixtures() -> &'static Mutex<HashMap<String, AiReliabilityStateSnapshot>> {
    FIXTURES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn set_ai_reliability_fixture(
    window_id: impl Into<String>,
    fixture_id: &str,
) -> Result<AiReliabilityStateSnapshot, String> {
    let snapshot = ai_reliability_fixture_snapshot(fixture_id)?;
    fixtures().lock().insert(window_id.into(), snapshot.clone());
    Ok(snapshot)
}

pub(crate) fn ai_reliability_snapshot_for_target(
    window_id: &str,
    kind: AutomationWindowKind,
) -> AiReliabilityStateSnapshot {
    fixtures()
        .lock()
        .get(window_id)
        .cloned()
        .unwrap_or_else(|| AiReliabilityStateSnapshot::ready(surface_for_kind(kind)))
}

pub(crate) fn ai_reliability_fixture_for_target(
    window_id: &str,
) -> Option<AiReliabilityStateSnapshot> {
    fixtures().lock().get(window_id).cloned()
}

/// Project the live provider-independent state machine into the redacted
/// automation contract. This is intentionally a pure read: fixtures may
/// describe legacy defects, while live Agent Chat proof must reflect the
/// thread-owned authority rather than a detached global default.
pub(crate) fn ai_operation_state_snapshot(
    surface: &str,
    state: &AiOperationState,
    card: Option<&super::AiRecoveryCardSpec>,
) -> AiReliabilityStateSnapshot {
    let failure = match &state.phase {
        AiPhase::AwaitingRecovery { failure, .. } => Some(failure),
        _ => None,
    };
    let recovery_actions = card
        .map(|card| {
            card.actions
                .iter()
                .filter(|action| action.enabled)
                .map(|action| action.semantic_id.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let primary_action_id = card.and_then(|card| {
        card.actions
            .iter()
            .find(|action| action.enabled && action.role == RecoveryRole::Primary)
            .map(|action| action.semantic_id.to_string())
    });

    AiReliabilityStateSnapshot {
        schema_version: crate::protocol::AI_RELIABILITY_STATE_SCHEMA_VERSION,
        surface: surface.to_string(),
        phase: phase_name(&state.phase).to_string(),
        failure_category: failure.map(|failure| failure_category(&failure.kind).to_string()),
        failure_code: failure.map(|failure| format!("{:?}", failure.code)),
        primary_action_id,
        recovery_actions,
        retry: AiReliabilityRetrySnapshot {
            automatic_used: state.retry.automatic_used,
            manual_used: state.retry.manual_used,
            automatic_max: state.retry.policy.automatic_max,
            manual_max: state.retry.policy.manual_max,
            exhausted: !state.retry.automatic_available() && !state.retry.manual_available(),
        },
        identity: identity_snapshot(&state.identity, &state.selection),
        preservation: AiReliabilityPreservationSnapshot {
            transcript_fingerprint: preservation_fingerprint(&state.work.transcript),
            draft_fingerprint: preservation_fingerprint(&state.work.draft),
            partial_output_fingerprint: preservation_fingerprint(&state.work.partial_output),
        },
        diagnostic: diagnostic_snapshot(failure, state),
        last_transition: AiReliabilityTransitionSnapshot::default(),
    }
}

pub(crate) fn phase_name(phase: &AiPhase) -> &'static str {
    match phase {
        AiPhase::Ready => "ready",
        AiPhase::Preflighting { .. } => "preflighting",
        AiPhase::Running { .. } => "running",
        AiPhase::Cancelling { .. } => "cancelling",
        AiPhase::AwaitingRecovery { .. } => "awaitingRecovery",
        AiPhase::Recovering { .. } => "recovering",
        AiPhase::Recovered { .. } => "recovered",
        AiPhase::Succeeded { .. } => "succeeded",
        AiPhase::Cancelled { .. } => "cancelled",
        AiPhase::Dismissed { .. } => "dismissed",
    }
}

fn failure_category(kind: &AiFailureKind) -> &'static str {
    match kind {
        AiFailureKind::Capability(_) => "capability",
        AiFailureKind::Policy(_) => "policy",
        AiFailureKind::Authentication(_) => "authentication",
        AiFailureKind::Configuration(_) => "configuration",
        AiFailureKind::Connectivity(_) => "connectivity",
        AiFailureKind::Provider(_) => "provider",
        AiFailureKind::Runtime(_) => "runtime",
        AiFailureKind::Protocol(_) => "protocol",
        AiFailureKind::Permission(_) => "permission",
        AiFailureKind::Input(_) => "input",
        AiFailureKind::Unknown => "unknown",
    }
}

fn identity_snapshot(
    identity: &AiSurfaceIdentity,
    selection: &AiSelectionState,
) -> AiReliabilityIdentitySnapshot {
    let effective = selection
        .effective
        .as_ref()
        .or(selection.requested.as_ref());
    let mut snapshot = AiReliabilityIdentitySnapshot {
        provider_id: effective
            .and_then(|selection| selection.provider_id.as_ref())
            .map(|id| id.0.clone()),
        model_id: effective
            .and_then(|selection| selection.model_id.as_ref())
            .map(|id| id.0.clone()),
        profile_id: effective
            .and_then(|selection| selection.profile_id.as_ref())
            .map(|id| id.0.clone()),
        flow_id: None,
        selection_origin: Some(format!("{:?}", selection.origin)),
    };
    match identity {
        AiSurfaceIdentity::QuickAi {
            profile_id,
            provider_id,
            model_id,
        }
        | AiSurfaceIdentity::FocusedText {
            profile_id,
            provider_id,
            model_id,
        } => {
            snapshot
                .profile_id
                .get_or_insert_with(|| profile_id.0.clone());
            snapshot
                .provider_id
                .get_or_insert_with(|| provider_id.0.clone());
            snapshot.model_id.get_or_insert_with(|| model_id.0.clone());
        }
        AiSurfaceIdentity::AgentChat {
            profile_id,
            provider_id,
            model_id,
            ..
        } => {
            snapshot
                .profile_id
                .get_or_insert_with(|| profile_id.0.clone());
            if let Some(provider_id) = provider_id {
                snapshot
                    .provider_id
                    .get_or_insert_with(|| provider_id.0.clone());
            }
            if let Some(model_id) = model_id {
                snapshot.model_id.get_or_insert_with(|| model_id.0.clone());
            }
        }
        AiSurfaceIdentity::FlowConversation {
            flow_id,
            provider_id,
            model_id,
            ..
        } => {
            snapshot.flow_id = Some(flow_id.0.clone());
            if let Some(provider_id) = provider_id {
                snapshot
                    .provider_id
                    .get_or_insert_with(|| provider_id.0.clone());
            }
            if let Some(model_id) = model_id {
                snapshot.model_id.get_or_insert_with(|| model_id.0.clone());
            }
        }
        AiSurfaceIdentity::FlowRun { flow_id, .. } => {
            snapshot.flow_id = Some(flow_id.0.clone());
        }
        AiSurfaceIdentity::LegacyChatPrompt {
            provider_id,
            model_id,
            ..
        }
        | AiSurfaceIdentity::Other {
            provider_id,
            model_id,
            ..
        } => {
            if let Some(provider_id) = provider_id {
                snapshot
                    .provider_id
                    .get_or_insert_with(|| provider_id.0.clone());
            }
            if let Some(model_id) = model_id {
                snapshot.model_id.get_or_insert_with(|| model_id.0.clone());
            }
        }
    }
    snapshot
}

fn preservation_fingerprint(receipt: &PreservationReceipt) -> Option<String> {
    match receipt {
        PreservationReceipt::Preserved { fingerprint }
        | PreservationReceipt::Restorable { fingerprint } => Some(fingerprint.0.clone()),
        PreservationReceipt::NotApplicable | PreservationReceipt::Missing { .. } => None,
    }
}

fn diagnostic_snapshot(
    failure: Option<&AiFailure>,
    state: &AiOperationState,
) -> AiReliabilityDiagnosticSnapshot {
    let descriptor = state
        .diagnostic
        .as_ref()
        .or_else(|| failure.and_then(|failure| failure.diagnostic.as_ref()));
    AiReliabilityDiagnosticSnapshot {
        available: descriptor.is_some_and(|descriptor| {
            !matches!(descriptor.availability, DiagnosticAvailability::Unavailable)
        }),
        redacted: true,
        fingerprint: descriptor.map(|descriptor| descriptor.fingerprint.0.clone()),
        raw_primary_visible: false,
    }
}

pub(crate) fn ai_reliability_fixture_snapshot(
    fixture_id: &str,
) -> Result<AiReliabilityStateSnapshot, String> {
    let (surface, category, code, transcript, draft) = match fixture_id {
        "image-1-client-too-old" => (
            "chatPrompt",
            "capability",
            "ClientTooOld",
            "image-1-transcript",
            "image-1-draft",
        ),
        "image-2-search-budget" => (
            "quickAi",
            "policy",
            "QuickAiSearchBudgetExceeded",
            "image-2-transcript",
            "image-2-draft",
        ),
        "protocol-failure" => (
            "flowConversation",
            "protocol",
            "ProtocolVersionMismatch",
            "flow-protocol-transcript",
            "flow-protocol-draft",
        ),
        _ => return Err(format!("unknown AI reliability fixture: {fixture_id}")),
    };
    let mut snapshot = AiReliabilityStateSnapshot::ready(surface);
    snapshot.phase = "awaitingRecovery".to_string();
    snapshot.failure_category = Some(category.to_string());
    snapshot.failure_code = Some(code.to_string());
    snapshot.preservation = AiReliabilityPreservationSnapshot {
        transcript_fingerprint: Some(redacted_fingerprint(transcript)),
        draft_fingerprint: Some(redacted_fingerprint(draft)),
        partial_output_fingerprint: None,
    };
    snapshot.diagnostic.available = true;
    snapshot.diagnostic.fingerprint = Some(redacted_fingerprint(fixture_id));
    // Red fixtures deliberately describe the old defect without retaining its
    // payload: primary raw/internal detail was visible and no action existed.
    snapshot.diagnostic.raw_primary_visible = true;
    snapshot.last_transition.from = Some("running".to_string());
    snapshot.last_transition.event = Some("legacyRawFailure".to_string());
    snapshot.last_transition.to = Some("awaitingRecovery".to_string());
    snapshot.identity = match fixture_id {
        "image-1-client-too-old" => AiReliabilityIdentitySnapshot {
            provider_id: Some("codex".to_string()),
            model_id: Some("gpt-5.6-sol".to_string()),
            ..Default::default()
        },
        "image-2-search-budget" => AiReliabilityIdentitySnapshot {
            provider_id: Some("codex".to_string()),
            model_id: Some("gpt-5.3-codex-spark".to_string()),
            profile_id: Some("quick-ai".to_string()),
            ..Default::default()
        },
        "protocol-failure" => AiReliabilityIdentitySnapshot {
            flow_id: Some("fixture-flow".to_string()),
            ..Default::default()
        },
        _ => AiReliabilityIdentitySnapshot::default(),
    };
    Ok(snapshot)
}

fn surface_for_kind(kind: AutomationWindowKind) -> &'static str {
    match kind {
        AutomationWindowKind::AgentChatDetached => "agentChat",
        AutomationWindowKind::Main => "agentChat",
        AutomationWindowKind::ActionsDialog
        | AutomationWindowKind::Notes
        | AutomationWindowKind::Dictation
        | AutomationWindowKind::PromptPopup
        | AutomationWindowKind::Hud => "agentChat",
    }
}

pub(crate) fn redacted_fingerprint(value: &str) -> String {
    let bytes = Sha256::digest(value.as_bytes());
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_fixtures_are_payload_free_and_exactly_classified() {
        for (id, surface, category, code) in [
            (
                "image-1-client-too-old",
                "chatPrompt",
                "capability",
                "ClientTooOld",
            ),
            (
                "image-2-search-budget",
                "quickAi",
                "policy",
                "QuickAiSearchBudgetExceeded",
            ),
            (
                "protocol-failure",
                "flowConversation",
                "protocol",
                "ProtocolVersionMismatch",
            ),
        ] {
            let snapshot = ai_reliability_fixture_snapshot(id).expect("known fixture");
            assert_eq!(snapshot.surface, surface);
            assert_eq!(snapshot.failure_category.as_deref(), Some(category));
            assert_eq!(snapshot.failure_code.as_deref(), Some(code));
            assert!(snapshot.diagnostic.raw_primary_visible);
            assert!(snapshot.diagnostic.redacted);
            assert!(snapshot.recovery_actions.is_empty());
            let json = serde_json::to_string(&snapshot).expect("snapshot serialization");
            assert!(!json.contains("invalid_request_error"));
            assert!(!json.contains("quick_ai_more_than_two_search_queries"));
            assert!(!json.contains("/Users/"));
        }
    }

    #[test]
    fn unknown_fixture_fails_closed() {
        assert!(ai_reliability_fixture_snapshot("not-a-fixture").is_err());
    }
}
