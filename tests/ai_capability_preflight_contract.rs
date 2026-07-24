use script_kit_gpui::ai::reliability::{
    acknowledge_selection, decide_selection_change, preflight, AiModelCandidate, CapabilityCache,
    CapabilityCacheKey, CapabilityDecision, CapabilityEvidence, CapabilityEvidenceKind,
    CompatibilityRecordKind, ExecutableIdentity, SelectionDecision,
};
use sk_protocol::ai_reliability::{
    AiFailureCode, AiModelSelection, AiSelectionState, AiSurfaceIdentity, CommandId, EngineId,
    Fingerprint, FlowId, ModelId, ProfileId, PromptId, ProviderId, SelectionOrigin,
};

fn selection(provider: &str, model: &str) -> AiModelSelection {
    AiModelSelection {
        provider_id: Some(ProviderId::from(provider)),
        model_id: Some(ModelId::from(model)),
        profile_id: Some(ProfileId::from("profile")),
    }
}

fn selected(provider: &str, model: &str) -> AiSelectionState {
    let selected = selection(provider, model);
    AiSelectionState {
        requested: Some(selected.clone()),
        effective: Some(selected),
        origin: SelectionOrigin::PersistedUserChoice,
        acknowledged_change: None,
    }
}

fn executable(fingerprint: &str) -> ExecutableIdentity {
    ExecutableIdentity {
        canonical_id: "/Applications/Codex.app/Contents/MacOS/codex".to_string(),
        fingerprint: fingerprint.to_string(),
        version: Some("0.1.0".to_string()),
    }
}

fn identity() -> AiSurfaceIdentity {
    AiSurfaceIdentity::AgentChat {
        profile_id: ProfileId::from("profile"),
        provider_id: Some(ProviderId::from("openai")),
        model_id: Some(ModelId::from("gpt-5.6-sol")),
        cwd_fingerprint: Fingerprint::from("cwd"),
    }
}

fn evidence(record: Option<CompatibilityRecordKind>) -> CapabilityEvidence {
    CapabilityEvidence {
        executable: executable("binary-a"),
        advertised_models: vec![AiModelCandidate {
            provider_id: ProviderId::from("openai"),
            model_id: ModelId::from("gpt-5.6-sol"),
            advertised: true,
        }],
        exact_record: record,
        roster_protocol_ready: None,
        spawned_processes: 0,
    }
}

#[test]
fn known_old_client_blocks_before_start_turn() {
    let decision = preflight(
        &identity(),
        &selected("openai", "gpt-5.6-sol"),
        &evidence(Some(CompatibilityRecordKind::ClientTooOld {
            client: sk_protocol::ai_reliability::ClientKind::Codex,
        })),
    );
    assert!(matches!(
        decision,
        CapabilityDecision::Blocked(failure) if failure.code == AiFailureCode::ClientTooOld
    ));
}

#[test]
fn unknown_evidence_is_truthful_and_does_not_probe_on_submit() {
    let mut facts = evidence(None);
    facts.advertised_models.clear();
    let decision = preflight(&identity(), &selected("openai", "gpt-5.6-sol"), &facts);
    assert!(matches!(
        decision,
        CapabilityDecision::Unknown(receipt)
            if receipt.evidence == CapabilityEvidenceKind::Unknown
                && receipt.spawned_processes == 0
    ));
}

#[test]
fn missing_model_requires_explicit_apply_and_ack_before_ready() {
    let current = selected("openai", "retired-model");
    let requested = selection("openai", "gpt-5.6-sol");
    let candidates = evidence(None).advertised_models;

    let apply = decide_selection_change(&current, requested.clone(), &candidates, CommandId(9));
    assert!(matches!(apply, SelectionDecision::ApplySelection(_)));

    let acknowledged = acknowledge_selection(&current, requested.clone());
    assert!(acknowledged.can_start_turn());
    assert!(matches!(
        decide_selection_change(&acknowledged, requested, &candidates, CommandId(10)),
        SelectionDecision::Ready(_)
    ));
}

#[test]
fn unadvertised_model_requires_compatible_choice() {
    let current = selected("openai", "retired-model");
    let requested = selection("openai", "also-missing");
    assert!(matches!(
        decide_selection_change(
            &current,
            requested,
            &evidence(None).advertised_models,
            CommandId(11)
        ),
        SelectionDecision::ChooseCompatibleModel { .. }
    ));
}

#[test]
fn cache_key_includes_fingerprint_and_submit_snapshot_is_bounded() {
    let cache = CapabilityCache::default();
    let provider = ProviderId::from("openai");
    let model = ModelId::from("gpt-5.6-sol");
    cache.record_negative(
        CapabilityCacheKey::new(&executable("binary-a"), provider.clone(), model.clone()),
        sk_protocol::ai_reliability::ClientKind::Codex,
    );

    assert!(matches!(
        cache
            .snapshot(executable("binary-a"), provider.clone(), model.clone())
            .exact_record,
        Some(CompatibilityRecordKind::ClientTooOld { .. })
    ));
    let changed_binary = cache.snapshot(executable("binary-b"), provider, model);
    assert!(changed_binary.exact_record.is_none());
    assert_eq!(changed_binary.spawned_processes, 0);

    for index in 0..300 {
        cache.record_success(CapabilityCacheKey::new(
            &executable("binary-a"),
            ProviderId::from("openai"),
            ModelId::from(format!("model-{index}")),
        ));
    }
    assert!(cache.len() <= 256);
}

#[test]
fn legacy_prompt_and_flow_use_the_same_preflight_contract() {
    let state = selected("openai", "gpt-5.6-sol");
    let facts = evidence(Some(CompatibilityRecordKind::LastSuccessful));
    let surfaces = [
        AiSurfaceIdentity::LegacyChatPrompt {
            prompt_id: PromptId::from("prompt"),
            provider_id: Some(ProviderId::from("openai")),
            model_id: Some(ModelId::from("gpt-5.6-sol")),
        },
        AiSurfaceIdentity::FlowConversation {
            flow_id: FlowId::from("agent-chat"),
            definition_fingerprint: Fingerprint::from("flow"),
            engine_id: EngineId::from("codex"),
            provider_id: Some(ProviderId::from("openai")),
            model_id: Some(ModelId::from("gpt-5.6-sol")),
        },
    ];

    for surface in surfaces {
        assert!(matches!(
            preflight(&surface, &state, &facts),
            CapabilityDecision::Compatible(_)
        ));
    }
}
