use super::capabilities::AiModelCandidate;
use sk_protocol::ai_reliability::{
    AiModelSelection, AiSelectionState, ExplicitSelectionChange, SelectionChangeReceipt,
    SelectionOrigin,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionDecision {
    Ready(Box<AiSelectionState>),
    ChooseCompatibleModel {
        requested: AiModelSelection,
        candidates: Vec<AiModelCandidate>,
    },
    ApplySelection(ExplicitSelectionChange),
}

pub fn decide_selection_change(
    current: &AiSelectionState,
    requested: AiModelSelection,
    candidates: &[AiModelCandidate],
    command_id: sk_protocol::ai_reliability::CommandId,
) -> SelectionDecision {
    if current.effective.as_ref() == Some(&requested) {
        return SelectionDecision::Ready(Box::new(current.clone()));
    }
    if candidates.iter().any(|candidate| {
        requested.provider_id.as_ref() == Some(&candidate.provider_id)
            && requested.model_id.as_ref() == Some(&candidate.model_id)
    }) {
        SelectionDecision::ApplySelection(ExplicitSelectionChange {
            command_id,
            requested,
            origin: SelectionOrigin::RecoveryChoice,
        })
    } else {
        SelectionDecision::ChooseCompatibleModel {
            requested,
            candidates: candidates.to_vec(),
        }
    }
}

pub fn acknowledge_selection(
    previous: &AiSelectionState,
    applied: AiModelSelection,
) -> AiSelectionState {
    AiSelectionState {
        requested: Some(applied.clone()),
        effective: Some(applied.clone()),
        origin: SelectionOrigin::RecoveryChoice,
        acknowledged_change: Some(SelectionChangeReceipt {
            previous: previous.effective.clone(),
            applied,
            origin: SelectionOrigin::RecoveryChoice,
        }),
    }
}
