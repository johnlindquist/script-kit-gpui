//! Redacted, target-scoped AI reliability automation state.

use serde::{Deserialize, Serialize};

pub const AI_RELIABILITY_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReliabilityRetrySnapshot {
    pub automatic_used: u8,
    pub manual_used: u8,
    pub automatic_max: u8,
    pub manual_max: u8,
    pub exhausted: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReliabilityIdentitySnapshot {
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub profile_id: Option<String>,
    pub flow_id: Option<String>,
    pub selection_origin: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReliabilityPreservationSnapshot {
    pub transcript_fingerprint: Option<String>,
    pub draft_fingerprint: Option<String>,
    pub partial_output_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReliabilityDiagnosticSnapshot {
    pub available: bool,
    pub redacted: bool,
    pub fingerprint: Option<String>,
    pub raw_primary_visible: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReliabilityTransitionSnapshot {
    pub from: Option<String>,
    pub event: Option<String>,
    pub to: Option<String>,
    pub command_ids: Vec<u64>,
    pub invalid_transition: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReliabilityStateSnapshot {
    pub schema_version: u32,
    pub surface: String,
    pub phase: String,
    pub failure_category: Option<String>,
    pub failure_code: Option<String>,
    pub primary_action_id: Option<String>,
    pub recovery_actions: Vec<String>,
    pub retry: AiReliabilityRetrySnapshot,
    pub identity: AiReliabilityIdentitySnapshot,
    pub preservation: AiReliabilityPreservationSnapshot,
    pub diagnostic: AiReliabilityDiagnosticSnapshot,
    pub last_transition: AiReliabilityTransitionSnapshot,
}

impl AiReliabilityStateSnapshot {
    pub fn ready(surface: impl Into<String>) -> Self {
        Self {
            schema_version: AI_RELIABILITY_STATE_SCHEMA_VERSION,
            surface: surface.into(),
            phase: "ready".to_string(),
            failure_category: None,
            failure_code: None,
            primary_action_id: None,
            recovery_actions: Vec::new(),
            retry: AiReliabilityRetrySnapshot {
                automatic_used: 0,
                manual_used: 0,
                automatic_max: 0,
                manual_max: 0,
                exhausted: false,
            },
            identity: AiReliabilityIdentitySnapshot::default(),
            preservation: AiReliabilityPreservationSnapshot::default(),
            diagnostic: AiReliabilityDiagnosticSnapshot {
                available: false,
                redacted: true,
                fingerprint: None,
                raw_primary_visible: false,
            },
            last_transition: AiReliabilityTransitionSnapshot::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AutomationWindowTarget, Message};

    #[test]
    fn reliability_query_and_fixture_commands_round_trip() {
        let query: Message = serde_json::from_str(
            r#"{"type":"getAiReliabilityState","requestId":"q1","target":{"type":"main"}}"#,
        )
        .expect("query parses");
        assert!(matches!(
            query,
            Message::GetAiReliabilityState {
                request_id,
                target: Some(AutomationWindowTarget::Main)
            } if request_id == "q1"
        ));

        let fixture: Message = serde_json::from_str(
            r#"{"type":"setAiReliabilityTestFixture","requestId":"f1","fixtureId":"image-1-client-too-old","target":{"type":"main"}}"#,
        )
        .expect("fixture command parses");
        assert!(matches!(
            fixture,
            Message::SetAiReliabilityTestFixture {
                request_id,
                fixture_id,
                target: Some(AutomationWindowTarget::Main)
            } if request_id == "f1" && fixture_id == "image-1-client-too-old"
        ));
    }

    #[test]
    fn ready_snapshot_serializes_the_redaction_contract() {
        let value = serde_json::to_value(AiReliabilityStateSnapshot::ready("agentChat"))
            .expect("serialize snapshot");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["surface"], "agentChat");
        assert_eq!(value["diagnostic"]["redacted"], true);
        assert_eq!(value["diagnostic"]["rawPrimaryVisible"], false);
    }
}
