use super::capability_cache::CompatibilityRecordKind;
use sk_protocol::ai_reliability::{
    AiFailure, AiFailureKind, AiModelSelection, AiSelectionState, AiSurfaceIdentity,
    CapabilityFailure, ClientKind, ModelAvailabilityReason, ModelId, ProviderId, RetrySafety,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableIdentity {
    pub canonical_id: String,
    pub fingerprint: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiModelCandidate {
    pub provider_id: ProviderId,
    pub model_id: ModelId,
    pub advertised: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityEvidenceKind {
    ExactNegativeRecord,
    LastSuccessfulFingerprint,
    AdvertisedModelList,
    MdflowRoster,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReceipt {
    pub evidence: CapabilityEvidenceKind,
    pub executable_fingerprint: String,
    pub provider_id: Option<ProviderId>,
    pub model_id: Option<ModelId>,
    pub spawned_processes: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvidence {
    pub executable: ExecutableIdentity,
    pub advertised_models: Vec<AiModelCandidate>,
    pub exact_record: Option<CompatibilityRecordKind>,
    pub roster_protocol_ready: Option<bool>,
    /// Must remain zero for a submission-time snapshot. Refresh adapters probe
    /// only at startup, explicit profile selection, or fingerprint change.
    pub spawned_processes: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDecision {
    Compatible(CapabilityReceipt),
    Unknown(CapabilityReceipt),
    Blocked(AiFailure),
    SelectionUnavailable {
        requested: AiModelSelection,
        candidates: Vec<AiModelCandidate>,
    },
}

pub fn preflight(
    identity: &AiSurfaceIdentity,
    selection: &AiSelectionState,
    evidence: &CapabilityEvidence,
) -> CapabilityDecision {
    let requested = selection
        .requested
        .clone()
        .or_else(|| selection.effective.clone())
        .unwrap_or_else(|| selection_from_identity(identity));
    let provider_id = requested.provider_id.clone();
    let model_id = requested.model_id.clone();
    let receipt = |kind| CapabilityReceipt {
        evidence: kind,
        executable_fingerprint: evidence.executable.fingerprint.clone(),
        provider_id: provider_id.clone(),
        model_id: model_id.clone(),
        spawned_processes: evidence.spawned_processes,
    };

    if selection.requested != selection.effective
        || selection.effective.is_none()
        || model_id.is_none()
    {
        return CapabilityDecision::SelectionUnavailable {
            requested,
            candidates: evidence.advertised_models.clone(),
        };
    }

    match &evidence.exact_record {
        Some(CompatibilityRecordKind::ClientTooOld { client }) => {
            CapabilityDecision::Blocked(AiFailure::new(
                AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
                    client: *client,
                    model: model_id,
                }),
                RetrySafety::ExplicitUserConfirmation,
            ))
        }
        Some(CompatibilityRecordKind::LastSuccessful) => CapabilityDecision::Compatible(receipt(
            CapabilityEvidenceKind::LastSuccessfulFingerprint,
        )),
        None => {
            if evidence.roster_protocol_ready == Some(false) {
                return CapabilityDecision::Blocked(AiFailure::new(
                    AiFailureKind::Capability(CapabilityFailure::ClientTooOld {
                        client: client_for_identity(identity),
                        model: model_id,
                    }),
                    RetrySafety::ExplicitUserConfirmation,
                ));
            }
            if let (Some(provider), Some(model)) = (provider_id.as_ref(), model_id.as_ref()) {
                if !evidence.advertised_models.is_empty() {
                    let advertised = evidence.advertised_models.iter().any(|candidate| {
                        &candidate.provider_id == provider && &candidate.model_id == model
                    });
                    if advertised {
                        CapabilityDecision::Compatible(receipt(
                            CapabilityEvidenceKind::AdvertisedModelList,
                        ))
                    } else {
                        CapabilityDecision::SelectionUnavailable {
                            requested,
                            candidates: evidence.advertised_models.clone(),
                        }
                    }
                } else if evidence.roster_protocol_ready == Some(true) {
                    CapabilityDecision::Compatible(receipt(CapabilityEvidenceKind::MdflowRoster))
                } else {
                    CapabilityDecision::Unknown(receipt(CapabilityEvidenceKind::Unknown))
                }
            } else {
                CapabilityDecision::SelectionUnavailable {
                    requested,
                    candidates: evidence.advertised_models.clone(),
                }
            }
        }
    }
}

fn selection_from_identity(identity: &AiSurfaceIdentity) -> AiModelSelection {
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
        } => AiModelSelection {
            provider_id: Some(provider_id.clone()),
            model_id: Some(model_id.clone()),
            profile_id: Some(profile_id.clone()),
        },
        AiSurfaceIdentity::AgentChat {
            profile_id,
            provider_id,
            model_id,
            ..
        } => AiModelSelection {
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
            profile_id: Some(profile_id.clone()),
        },
        AiSurfaceIdentity::FlowConversation {
            provider_id,
            model_id,
            ..
        }
        | AiSurfaceIdentity::LegacyChatPrompt {
            provider_id,
            model_id,
            ..
        }
        | AiSurfaceIdentity::Other {
            provider_id,
            model_id,
            ..
        } => AiModelSelection {
            provider_id: provider_id.clone(),
            model_id: model_id.clone(),
            profile_id: None,
        },
        AiSurfaceIdentity::FlowRun { .. } => AiModelSelection {
            provider_id: None,
            model_id: None,
            profile_id: None,
        },
    }
}

fn client_for_identity(identity: &AiSurfaceIdentity) -> ClientKind {
    match identity {
        AiSurfaceIdentity::FlowConversation { .. } | AiSurfaceIdentity::FlowRun { .. } => {
            ClientKind::Mdflow
        }
        AiSurfaceIdentity::QuickAi { .. }
        | AiSurfaceIdentity::AgentChat { .. }
        | AiSurfaceIdentity::LegacyChatPrompt { .. }
        | AiSurfaceIdentity::FocusedText { .. } => ClientKind::Codex,
        AiSurfaceIdentity::Other { .. } => ClientKind::Other,
    }
}

pub fn model_unavailable_failure(model: ModelId) -> AiFailure {
    AiFailure::new(
        AiFailureKind::Capability(CapabilityFailure::ModelUnavailable {
            model,
            reason: ModelAvailabilityReason::NotAdvertised,
        }),
        RetrySafety::ExplicitUserConfirmation,
    )
}
