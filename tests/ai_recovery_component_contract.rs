use script_kit_gpui::ai::reliability::{
    project_recovery, AiRecoveryActionSpec, AiRecoveryCardSpec, AiRecoveryLayout,
    AiRecoveryProgress, AiRecoveryTone, SurfaceRecoveryCapabilities, AI_RECOVERY_CARD_ID,
};
use script_kit_gpui::components::{
    decide_recovery_key, recovery_semantic_tree, AiRecoveryFocusTarget, AiRecoveryKey,
    AiRecoveryKeyDecision,
};
use sk_protocol::ai_reliability::{
    AiFailure, AiFailureKind, AiOperationState, AiPhase, AiRecoveryAction, AiSelectionState,
    AiSurfaceIdentity, DisabledReason, Fingerprint, FlowId, ModelId, PreservationReceipt,
    ProviderId, RecoveryActionKind, RecoveryOption, RecoveryPlan, RecoveryRole, RetryPolicy,
    RetrySafety, SelectionOrigin, WorkKey,
};

fn card_spec() -> AiRecoveryCardSpec {
    AiRecoveryCardSpec {
        semantic_id: AI_RECOVERY_CARD_ID,
        layout: AiRecoveryLayout::TranscriptCard,
        tone: AiRecoveryTone::Error,
        title: "AI request did not finish".into(),
        body: "Your work is saved.".into(),
        preservation_note: Some("Your conversation and draft are saved.".into()),
        progress: Some(AiRecoveryProgress {
            label: "Waiting".into(),
        }),
        actions: vec![
            AiRecoveryActionSpec {
                semantic_id: "ai-recovery-retry",
                label: "Try again".into(),
                action: AiRecoveryAction::Retry,
                role: RecoveryRole::Primary,
                enabled: true,
                disabled_reason: None,
            },
            AiRecoveryActionSpec {
                semantic_id: "ai-recovery-copy-details",
                label: "Copy details".into(),
                action: AiRecoveryAction::CopyDetails,
                role: RecoveryRole::Diagnostic,
                enabled: false,
                disabled_reason: Some(DisabledReason::UnsupportedBySurface),
            },
        ],
        dismissible: true,
    }
}

#[test]
fn shared_semantic_tree_and_keyboard_contract_are_public_and_deterministic() {
    let spec = card_spec();
    let semantic_tree = recovery_semantic_tree(&spec);
    assert!(semantic_tree
        .iter()
        .any(|node| node.semantic_id == "ai-recovery-card"));
    assert!(semantic_tree.iter().any(|node| {
        node.semantic_id == "ai-recovery-copy-details"
            && node.disabled_reason == Some(DisabledReason::UnsupportedBySurface)
    }));

    let focused = decide_recovery_key(&spec, None, AiRecoveryKey::Tab { shift: false });
    assert_eq!(
        focused,
        AiRecoveryKeyDecision::Focus(AiRecoveryFocusTarget::Action(0))
    );
    let activated = decide_recovery_key(
        &spec,
        Some(AiRecoveryFocusTarget::Action(0)),
        AiRecoveryKey::Enter,
    );
    assert_eq!(
        activated,
        AiRecoveryKeyDecision::Activate(AiRecoveryAction::Retry)
    );
    assert_eq!(
        decide_recovery_key(&spec, None, AiRecoveryKey::Escape),
        AiRecoveryKeyDecision::Dismiss
    );
}

#[test]
fn flow_conversation_projects_to_transcript_card_without_raw_copy() {
    let identity = AiSurfaceIdentity::FlowConversation {
        flow_id: FlowId("review".into()),
        definition_fingerprint: Fingerprint("definition".into()),
        engine_id: "codex".into(),
        provider_id: Some(ProviderId("provider".into())),
        model_id: Some(ModelId("model".into())),
    };
    let selection = AiSelectionState {
        requested: None,
        effective: None,
        origin: SelectionOrigin::BuiltInDefault,
        acknowledged_change: None,
    };
    let work = sk_protocol::ai_reliability::AiWorkSnapshot {
        key: WorkKey("flow-work".into()),
        transcript: PreservationReceipt::Preserved {
            fingerprint: Fingerprint("transcript".into()),
        },
        draft: PreservationReceipt::Restorable {
            fingerprint: Fingerprint("draft".into()),
        },
        attachments: PreservationReceipt::NotApplicable,
        partial_output: PreservationReceipt::NotApplicable,
    };
    let failure = AiFailure::new(AiFailureKind::Unknown, RetrySafety::Never);
    let mut state = AiOperationState::ready(
        identity.clone(),
        selection,
        work,
        RetryPolicy {
            automatic_max: 0,
            manual_max: 1,
        },
    );
    state.phase = AiPhase::AwaitingRecovery {
        failure,
        plan: RecoveryPlan {
            options: vec![RecoveryOption {
                kind: RecoveryActionKind::RethreadFlow,
                role: RecoveryRole::Primary,
                enabled: true,
                disabled_reason: None,
            }],
        },
    };

    let spec = project_recovery(&identity, &state, &SurfaceRecoveryCapabilities::all())
        .expect("awaiting recovery should project");
    assert_eq!(spec.layout, AiRecoveryLayout::TranscriptCard);
    assert_eq!(spec.actions[0].action, AiRecoveryAction::RethreadFlow);
    let copy = format!("{} {}", spec.title, spec.body);
    assert!(!copy.contains('{'));
    assert!(!copy.contains("token"));
}
