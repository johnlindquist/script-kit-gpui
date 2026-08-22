//! Honest observation classes and explicitly ratified latency budgets.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObservationPoint {
    NativeEventReceived,
    StateUpdated,
    FrameCallback,
    CompositorPresented,
    UserReadableOutput,
}

impl ObservationPoint {
    pub const fn proves_paint(self) -> bool {
        matches!(self, Self::CompositorPresented)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BudgetApproval {
    PendingOwnerRatification,
    Ratified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyBudget {
    pub scenario: String,
    pub observation: ObservationPoint,
    pub approval: BudgetApproval,
    pub minimum_samples: usize,
    pub p50_max_ms: u64,
    pub p95_max_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyReceipt {
    pub scenario: String,
    pub observation: ObservationPoint,
    pub binary_sha256: String,
    pub source_sha: String,
    pub sample_count: usize,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub max_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyVerdict {
    Passed,
    PendingRatification,
    ScenarioMismatch,
    ObservationMismatch,
    MissingArtifactIdentity,
    InsufficientSamples,
    Regressed,
}

impl LatencyBudget {
    pub fn evaluate(&self, receipt: &LatencyReceipt) -> LatencyVerdict {
        if self.approval != BudgetApproval::Ratified {
            return LatencyVerdict::PendingRatification;
        }
        if self.scenario != receipt.scenario {
            return LatencyVerdict::ScenarioMismatch;
        }
        if self.observation != receipt.observation {
            return LatencyVerdict::ObservationMismatch;
        }
        if receipt.binary_sha256.trim().is_empty() || receipt.source_sha.trim().is_empty() {
            return LatencyVerdict::MissingArtifactIdentity;
        }
        if receipt.sample_count < self.minimum_samples {
            return LatencyVerdict::InsufficientSamples;
        }
        if receipt.p50_ms > self.p50_max_ms || receipt.p95_ms > self.p95_max_ms {
            return LatencyVerdict::Regressed;
        }
        LatencyVerdict::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> LatencyBudget {
        LatencyBudget {
            scenario: "root-key-to-paint".to_owned(),
            observation: ObservationPoint::CompositorPresented,
            approval: BudgetApproval::Ratified,
            minimum_samples: 10,
            p50_max_ms: 25,
            p95_max_ms: 60,
        }
    }

    fn receipt() -> LatencyReceipt {
        LatencyReceipt {
            scenario: "root-key-to-paint".to_owned(),
            observation: ObservationPoint::CompositorPresented,
            binary_sha256: "binary-sha".to_owned(),
            source_sha: "source-sha".to_owned(),
            sample_count: 10,
            p50_ms: 20,
            p95_ms: 45,
            max_ms: 55,
        }
    }

    #[test]
    fn state_echo_and_frame_callbacks_cannot_claim_visible_paint() {
        assert!(!ObservationPoint::StateUpdated.proves_paint());
        assert!(!ObservationPoint::FrameCallback.proves_paint());
        assert!(ObservationPoint::CompositorPresented.proves_paint());
        let mut actual = receipt();
        actual.observation = ObservationPoint::StateUpdated;
        assert_eq!(
            budget().evaluate(&actual),
            LatencyVerdict::ObservationMismatch
        );
    }

    #[test]
    fn unratified_budget_never_becomes_a_shipping_gate() {
        let mut candidate = budget();
        candidate.approval = BudgetApproval::PendingOwnerRatification;
        assert_eq!(
            candidate.evaluate(&receipt()),
            LatencyVerdict::PendingRatification
        );
    }

    #[test]
    fn missing_identity_insufficient_samples_and_regressions_fail_closed() {
        let expected = budget();
        let mut actual = receipt();
        actual.binary_sha256.clear();
        assert_eq!(
            expected.evaluate(&actual),
            LatencyVerdict::MissingArtifactIdentity
        );

        let mut actual = receipt();
        actual.sample_count = 9;
        assert_eq!(
            expected.evaluate(&actual),
            LatencyVerdict::InsufficientSamples
        );

        let mut actual = receipt();
        actual.p95_ms = 61;
        assert_eq!(expected.evaluate(&actual), LatencyVerdict::Regressed);
    }

    #[test]
    fn ratified_identity_matched_observation_passes() {
        assert_eq!(budget().evaluate(&receipt()), LatencyVerdict::Passed);
    }
}
