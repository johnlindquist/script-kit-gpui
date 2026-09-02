impl ScriptListApp {
    pub(crate) fn owned_dictation_destination_window(
        &self,
        request: &crate::dictation::DictationDeliveryRequest,
        cx: &Context<Self>,
    ) -> Result<gpui::AnyWindowHandle, String> {
        use crate::dictation::{FrozenAgentChatPolicy, FrozenDictationDestination};
        // Notes owns a separate exact-window update in inject_text_into_frozen_notes;
        // the Main window is only its coordinator, never its editor authority.
        if matches!(&request.selection.destination, FrozenDictationDestination::NotesEditor { .. }) {
            let generation = crate::windows::automation_window_by_id("main").and_then(|info| info.generation)
                .ok_or_else(|| "The Main delivery coordinator was closed".to_string())?;
            return crate::windows::get_runtime_window_handle_for_generation("main", generation)
                .ok_or_else(|| "The Main delivery coordinator was closed".into());
        }
        let owner = match &request.selection.destination {
            FrozenDictationDestination::MainWindowFilter { owner, .. }
            | FrozenDictationDestination::MainWindowPrompt { owner, .. } => owner,
            FrozenDictationDestination::AgentChat { policy } => match policy {
                FrozenAgentChatPolicy::ExistingThread { main_owner, .. }
                | FrozenAgentChatPolicy::FreshStandard { main_owner, .. } => main_owner,
            },
            FrozenDictationDestination::DayPage { main_owner: Some(owner), .. } => owner,
            _ => return Err("The frozen destination has no owned Main window".into()),
        };
        if owner.root_entity_id != cx.entity_id().as_u64()
            || owner.surface_generation != self.owned_revision_facts().surface_generation
            || owner.visibility_generation != script_kit_gpui::main_window_visibility_generation()
        {
            return Err("The frozen Main destination was replaced".into());
        }
        let generation = owner.window_generation.ok_or_else(|| "The frozen Main window has no lifetime".to_string())?;
        crate::windows::get_runtime_window_handle_for_generation("main", generation)
            .ok_or_else(|| "The frozen Main window was closed or reopened".into())
    }

    pub(crate) fn deliver_owned_dictation_request(
        &mut self,
        request: crate::dictation::DictationDeliveryRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<crate::dictation::DictationDeliveryOutcome, String> {
        use crate::dictation::{DictationDeliveryFailureReason, DictationDeliveryOutcome, DictationTarget};
        if !crate::runtime_policy::is_owned_evaluation() {
            return Err("Owned Dictation delivery requires the owned runtime".into());
        }
        let refused = |detail: &str| DictationDeliveryOutcome::Refused {
            failure: crate::ai::reliability::destination_failure(true, detail),
            reason: DictationDeliveryFailureReason::DestinationStale,
        };
        let forbidden = match request.selection.target {
            DictationTarget::ExternalApp => Some(crate::runtime_policy::ExternalEffect::NativeInput),
            DictationTarget::QuickAiQuestion => Some(crate::runtime_policy::ExternalEffect::Provider),
            _ => None,
        };
        if let Some(effect) = forbidden {
            return match crate::runtime_policy::check(effect) {
                Err(error) => Ok(refused(&error.to_string())),
                Ok(()) => Err("Owned Dictation external-effect boundary was not enforced".into()),
            };
        }
        let destination = match self.owned_dictation_destination_window(&request, cx) {
            Ok(destination) => destination,
            Err(detail) => return Ok(refused(&detail)),
        };
        if destination != window.window_handle() {
            return Ok(refused("The supplied window does not own the frozen destination"));
        }
        if !crate::dictation::claim_dictation_delivery(request.delivery_id) {
            return Ok(refused("The Dictation delivery was already attempted"));
        }
        if let Err(detail) = crate::dictation::validate_owned_dictation_delivery_request(&request) {
            return Ok(refused(&detail));
        }
        let (result, pending) = self.mutate_internal_dictation_request(&request, Some(window), cx);
        match (result, pending) {
            (Ok(Some(range)), None) => Ok(dictation_outcome_from_insertion_range(&request, &range)
                .unwrap_or_else(|detail| DictationDeliveryOutcome::Failed {
                    failure: crate::ai::reliability::destination_failure(false, &detail),
                    reason: DictationDeliveryFailureReason::MutationOutcomeUnknown,
                    retry_safety: sk_protocol::ai_reliability::RetrySafety::Never,
                })),
            (Err(detail), _) => Ok(dictation_mutation_error_outcome(&detail)),
            (Ok(_), _) => {
                let detail = "The Dictation mutation outcome was not observed";
                Ok(DictationDeliveryOutcome::Failed {
                    failure: crate::ai::reliability::destination_failure(false, detail),
                    reason: DictationDeliveryFailureReason::MutationOutcomeUnknown,
                    retry_safety: sk_protocol::ai_reliability::RetrySafety::Never,
                })
            }
        }
    }

}
