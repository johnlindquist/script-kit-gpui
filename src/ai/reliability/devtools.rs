use crate::protocol::{
    AiReliabilityIdentitySnapshot, AiReliabilityPreservationSnapshot, AiReliabilityStateSnapshot,
    AutomationWindowKind,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static FIXTURES: OnceLock<Mutex<HashMap<String, AiReliabilityStateSnapshot>>> = OnceLock::new();

fn fixtures() -> &'static Mutex<HashMap<String, AiReliabilityStateSnapshot>> {
    FIXTURES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn set_ai_reliability_fixture(
    window_id: impl Into<String>,
    fixture_id: &str,
) -> Result<AiReliabilityStateSnapshot, String> {
    let snapshot = ai_reliability_fixture_snapshot(fixture_id)?;
    fixtures()
        .lock()
        .expect("AI reliability fixture mutex poisoned")
        .insert(window_id.into(), snapshot.clone());
    Ok(snapshot)
}

pub(crate) fn ai_reliability_snapshot_for_target(
    window_id: &str,
    kind: AutomationWindowKind,
) -> AiReliabilityStateSnapshot {
    fixtures()
        .lock()
        .expect("AI reliability fixture mutex poisoned")
        .get(window_id)
        .cloned()
        .unwrap_or_else(|| AiReliabilityStateSnapshot::ready(surface_for_kind(kind)))
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
