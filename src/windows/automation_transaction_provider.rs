//! [`TransactionStateProvider`] implementation for detached Agent Chat windows.
//!
//! Bridges the generic transaction executor (used by `batch`) with the
//! live state of a detached [`AgentChatView`] entity, enabling `setInput`,
//! `waitFor`, `selectByValue`, and `selectBySemanticId` against a
//! non-main automation target.

use crate::protocol::transaction_executor::TransactionStateProvider;
use crate::protocol::UiStateSnapshot;
use anyhow::{anyhow, Result};
use gpui::{App, Entity};

/// Transaction provider backed by a live detached Agent Chat entity.
///
/// Created per-batch-request and dropped when the batch completes.
/// Currently used by contract tests; the async batch handler inlines
/// operations directly against the entity to avoid blocking the UI thread.
#[allow(dead_code)]
pub(crate) struct DetachedAgentChatTransactionProvider<'a> {
    pub cx: &'a mut App,
    pub entity: Entity<crate::ai::agent_chat::ui::view::AgentChatView>,
}

impl<'a> TransactionStateProvider for DetachedAgentChatTransactionProvider<'a> {
    fn snapshot(&self) -> UiStateSnapshot {
        let view = self.entity.read(self.cx);
        let state = view.collect_agent_chat_state_snapshot(self.cx);

        // Build semantic IDs from the surface collector snapshot.
        let surface =
            crate::windows::automation_surface_collector::collect_agent_chat_detached_elements(
                &self.entity,
                200,
                self.cx,
            );

        UiStateSnapshot {
            window_visible: true,
            window_focused: true,
            prompt_type: Some("agentChatChat".to_string()),
            input_value: Some(state.input_text.clone()),
            selected_value: state
                .picker
                .as_ref()
                .and_then(|picker| picker.selected_label.clone()),
            choice_count: state.picker.as_ref().map_or(0, |picker| picker.item_count),
            visible_semantic_ids: surface
                .elements
                .iter()
                .map(|el| el.semantic_id.clone())
                .collect(),
            focused_semantic_id: surface.focused_semantic_id,
            agent_chat_status: Some(state.status.clone()),
            agent_chat_context_ready: state.context_ready,
            agent_chat_picker_open: state.picker.as_ref().is_some_and(|picker| picker.open),
            agent_chat_cursor_index: Some(state.cursor_index),
        }
    }

    fn set_input(&mut self, text: &str) -> Result<()> {
        let text = text.to_string();
        self.entity.update(self.cx, |view, cx| {
            let thread = view
                .thread()
                .ok_or_else(|| anyhow!("detached Agent Chat window is in setup mode"))?;
            thread.update(cx, |thread, cx| {
                thread.set_input(&text, cx);
            });
            tracing::info!(
                target: "script_kit::transaction",
                event = "transaction_detached_agent_chat_set_input",
                text_len = text.len(),
                "detached Agent Chat set_input"
            );
            Ok::<(), anyhow::Error>(())
        })
    }

    fn select_by_value(&mut self, value: &str, submit: bool) -> Result<Option<String>> {
        let value = value.to_string();
        self.entity.update(self.cx, |view, cx| {
            let Some(ref session) = view.composer_picker_session else {
                return Ok(None);
            };
            let Some(index) = session
                .items
                .iter()
                .position(|item| item.label.as_ref() == value || item.id.as_ref() == value)
            else {
                return Ok(None);
            };
            view.select_mention_index(index);
            if submit {
                view.accept_composer_picker_selection(cx);
            }
            tracing::info!(
                target: "script_kit::transaction",
                event = "transaction_detached_agent_chat_select_by_value",
                value = %value,
                submit,
                "detached Agent Chat select_by_value"
            );
            Ok::<Option<String>, anyhow::Error>(Some(value))
        })
    }

    fn select_by_semantic_id(&mut self, semantic_id: &str, submit: bool) -> Result<Option<String>> {
        self.select_by_value(semantic_id, submit)
    }

    fn agent_chat_test_probe(&self, tail: usize) -> crate::protocol::AgentChatTestProbeSnapshot {
        self.entity.read(self.cx).test_probe_snapshot(tail, self.cx)
    }
}

// ---------------------------------------------------------------------------
// ActionsDialog transaction provider
// ---------------------------------------------------------------------------

/// Transaction provider backed by a live ActionsDialog entity.
///
/// Enables `setInput`, `selectByValue`, and `selectBySemanticId` against
/// the actions dialog popup without requiring foreground keyboard focus.
/// Created per-batch-request and dropped when the batch completes.
#[allow(dead_code)]
pub(crate) struct ActionsDialogTransactionProvider<'a> {
    pub cx: &'a mut App,
    pub entity: Entity<crate::actions::ActionsDialog>,
}

impl<'a> TransactionStateProvider for ActionsDialogTransactionProvider<'a> {
    fn snapshot(&self) -> UiStateSnapshot {
        let surface = crate::windows::automation_surface_collector::collect_actions_dialog_elements(
            &self.entity,
            200,
            self.cx,
        );

        let dialog = self.entity.read(self.cx);

        UiStateSnapshot {
            window_visible: true,
            window_focused: true,
            prompt_type: Some("actionsDialog".to_string()),
            input_value: Some(dialog.search_text.clone()),
            selected_value: dialog.get_selected_action_id(),
            choice_count: dialog.filtered_actions.len(),
            visible_semantic_ids: surface
                .elements
                .iter()
                .map(|el| el.semantic_id.clone())
                .collect(),
            focused_semantic_id: surface.focused_semantic_id,
            ..Default::default()
        }
    }

    fn set_input(&mut self, text: &str) -> Result<()> {
        let text = text.to_string();
        anyhow::ensure!(
            crate::actions::set_actions_dialog_search_text(&self.entity, text.clone(), self.cx,),
            "ActionsDialog input owner is unavailable"
        );
        tracing::info!(
            target: "script_kit::transaction",
            event = "transaction_actions_dialog_set_input",
            text_len = text.len(),
            "ActionsDialog set_input"
        );
        Ok(())
    }

    fn select_by_value(&mut self, value: &str, _submit: bool) -> Result<Option<String>> {
        let value = value.to_string();
        let result = self
            .entity
            .update(self.cx, |dialog, cx| dialog.select_action_by_id(&value, cx));
        if result.is_some() {
            tracing::info!(
                target: "script_kit::transaction",
                event = "transaction_actions_dialog_select_by_value",
                value = %value,
                "ActionsDialog select_by_value"
            );
        }
        Ok(result)
    }

    fn select_by_semantic_id(
        &mut self,
        semantic_id: &str,
        _submit: bool,
    ) -> Result<Option<String>> {
        let semantic_id = semantic_id.to_string();
        let result = self.entity.update(self.cx, |dialog, cx| {
            dialog.select_action_by_semantic_id(&semantic_id, cx)
        });
        if result.is_some() {
            tracing::info!(
                target: "script_kit::transaction",
                event = "transaction_actions_dialog_select_by_semantic_id",
                semantic_id = %semantic_id,
                "ActionsDialog select_by_semantic_id"
            );
        }
        Ok(result)
    }
}
