//! The single mutator for [`ScriptListApp::agent_chat_surface_state`].
//!
//! Oracle-Session `agent_chat-chat-state-machine-audit` PR1. Every write to
//! the placement machine goes through [`transition_agent_chat_surface`], which
//! runs the pure reducer, emits a structured transition event, and in
//! debug builds asserts the placement agrees with
//! [`ScriptListApp::current_view`]. Raw writes to `agent_chat_surface_state`
//! are forbidden; an audit test pins that contract.

use super::*;
use crate::ai::agent_chat::ui::surface_state::{
    reduce_agent_chat_surface, AgentChatSurfaceEvent, AgentChatSurfaceState,
};

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct AgentChatSurfaceLifecycleReceipt {
    pub(crate) schema_version: u32,
    pub(crate) event: &'static str,
    pub(crate) source: &'static str,
    pub(crate) previous_state: String,
    pub(crate) next_state: String,
    pub(crate) previous_view: String,
    pub(crate) next_view: String,
    pub(crate) target_automation_id: String,
    pub(crate) target_kind: String,
    pub(crate) surface_kind: String,
    pub(crate) app_view_variant: String,
    pub(crate) return_view: Option<String>,
    pub(crate) return_focus_target: Option<String>,
    pub(crate) focused_input: String,
    pub(crate) main_rekeyed: bool,
    pub(crate) embedded_ai_window_visible: bool,
    pub(crate) actions_popup_cleared: bool,
    pub(crate) warnings: Vec<String>,
}

struct AgentChatSurfaceTransitionSnapshot {
    previous_state: AgentChatSurfaceState,
    previous_view: AppView,
    main_rekeyed: bool,
    embedded_ai_window_visible: bool,
    return_view: Option<String>,
    return_focus_target: Option<String>,
    warnings: Vec<String>,
}

impl ScriptListApp {
    /// Switch the launcher into the embedded Agent Chat chat surface and update
    /// every state mirror that must move in lock-step with that view.
    pub(crate) fn enter_embedded_agent_chat_surface(
        &mut self,
        entity: gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut gpui::Context<Self>,
    ) -> AgentChatSurfaceLifecycleReceipt {
        let previous_state = self.agent_chat_surface_state;
        let previous_view = self.current_view.clone();
        self.embedded_agent_chat_focus_handle = Some(entity.read(cx).focus_handle(cx));
        self.current_view = AppView::AgentChatView { entity };
        let main_rekeyed = self.rekey_main_automation_surface_from_current_view();
        self.transition_agent_chat_surface(AgentChatSurfaceEvent::EmbeddedOpened);
        self.focused_input = FocusedInput::None;
        self.clear_actions_popup_state();
        self.focus_coordinator
            .request(crate::focus_coordinator::FocusRequest::agent_chat());
        self.sync_coordinator_to_legacy();
        self.debug_assert_agent_chat_surface_consistent();

        let receipt = self.agent_chat_surface_lifecycle_receipt(
            "embedded_entry",
            "enter_embedded_agent_chat_surface",
            AgentChatSurfaceTransitionSnapshot {
                previous_state,
                previous_view,
                main_rekeyed,
                embedded_ai_window_visible: true,
                return_view: None,
                return_focus_target: None,
                warnings: Vec::new(),
            },
        );
        self.log_agent_chat_surface_lifecycle_receipt(&receipt);
        receipt
    }

    pub(crate) fn exit_embedded_agent_chat_surface(
        &mut self,
        return_view: AppView,
        return_focus_target: FocusTarget,
        source: &'static str,
        _cx: &mut gpui::Context<Self>,
    ) -> AgentChatSurfaceLifecycleReceipt {
        let previous_state = self.agent_chat_surface_state;
        let previous_view = self.current_view.clone();
        let return_view_debug = format!("{return_view:?}");
        let return_focus_debug = format!("{return_focus_target:?}");
        self.restore_current_view_with_focus(return_view.clone(), return_focus_target);
        let main_rekeyed = self.rekey_main_automation_surface_from_current_view();
        self.embedded_agent_chat_focus_handle = None;
        self.clear_actions_popup_state();
        self.transition_agent_chat_surface(AgentChatSurfaceEvent::EmbeddedClosed);
        self.sync_coordinator_to_legacy();
        self.debug_assert_agent_chat_surface_consistent();

        let receipt = self.agent_chat_surface_lifecycle_receipt(
            "embedded_exit",
            source,
            AgentChatSurfaceTransitionSnapshot {
                previous_state,
                previous_view,
                main_rekeyed,
                embedded_ai_window_visible: false,
                return_view: Some(return_view_debug),
                return_focus_target: Some(return_focus_debug),
                warnings: Vec::new(),
            },
        );
        self.log_agent_chat_surface_lifecycle_receipt(&receipt);
        receipt
    }

    /// Apply an [`AgentChatSurfaceEvent`]. No-op when the reduced next state
    /// equals the current state. Emits one `agent_chat_surface_transition`
    /// tracing event per real transition so operators can correlate
    /// placement drift with launcher-entry bugs.
    pub(crate) fn transition_agent_chat_surface(&mut self, event: AgentChatSurfaceEvent) {
        let previous = self.agent_chat_surface_state;
        let next = reduce_agent_chat_surface(previous, event);
        if next == previous {
            tracing::trace!(
                target: "script_kit::agent_chat",
                event = "agent_chat_surface_transition_noop",
                from = ?previous,
                trigger = ?event,
            );
            return;
        }
        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_surface_transition",
            from = ?previous,
            to = ?next,
            trigger = ?event,
        );
        self.agent_chat_surface_state = next;
    }

    /// Debug-only consistency check between the placement enum and
    /// [`AppView`]. Fires when the two disagree — the embedded state
    /// must co-occur with `AppView::AgentChatView`, a portal must co-occur
    /// with the matching portal host view, and `Hidden` must not be
    /// observed while the Agent Chat chat view is on-screen.
    ///
    /// This is `debug_assert` so release builds pay no cost. The goal
    /// is to fail loudly in test / dev runs if a future refactor sets
    /// `current_view` without calling [`transition_agent_chat_surface`].
    #[cfg(debug_assertions)]
    pub(crate) fn debug_assert_agent_chat_surface_consistent(&self) {
        match self.agent_chat_surface_state {
            AgentChatSurfaceState::Embedded => {
                debug_assert!(
                    matches!(self.current_view, AppView::AgentChatView { .. }),
                    "AgentChatSurfaceState::Embedded must agree with AppView::AgentChatView; \
                     current_view = {:?}",
                    self.current_view
                );
            }
            AgentChatSurfaceState::AttachmentPortal { .. } => {
                // Portal host view is one of several builtin surfaces;
                // we can only assert the *negative* half — the chat
                // view must NOT be the current view while a portal
                // owns the panel.
                debug_assert!(
                    !matches!(self.current_view, AppView::AgentChatView { .. }),
                    "AgentChatSurfaceState::AttachmentPortal must not observe AppView::AgentChatView"
                );
            }
            AgentChatSurfaceState::Hidden => {
                debug_assert!(
                    !matches!(self.current_view, AppView::AgentChatView { .. }),
                    "AgentChatSurfaceState::Hidden must not observe AppView::AgentChatView"
                );
            }
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    pub(crate) fn debug_assert_agent_chat_surface_consistent(&self) {}

    fn agent_chat_surface_lifecycle_receipt(
        &self,
        event: &'static str,
        source: &'static str,
        snapshot: AgentChatSurfaceTransitionSnapshot,
    ) -> AgentChatSurfaceLifecycleReceipt {
        AgentChatSurfaceLifecycleReceipt {
            schema_version: 1,
            event,
            source,
            previous_state: format!("{:?}", snapshot.previous_state),
            next_state: format!("{:?}", self.agent_chat_surface_state),
            previous_view: format!("{:?}", snapshot.previous_view),
            next_view: format!("{:?}", self.current_view),
            target_automation_id: "main".to_string(),
            target_kind: "main".to_string(),
            surface_kind: crate::semantic_surface_for_main_view(&self.current_view)
                .unwrap_or_else(|| "unknown".to_string()),
            app_view_variant: format!("{:?}", self.current_view),
            return_view: snapshot.return_view,
            return_focus_target: snapshot.return_focus_target,
            focused_input: format!("{:?}", self.focused_input),
            main_rekeyed: snapshot.main_rekeyed,
            embedded_ai_window_visible: snapshot.embedded_ai_window_visible,
            actions_popup_cleared: !self.show_actions_popup && self.actions_dialog.is_none(),
            warnings: snapshot.warnings,
        }
    }

    fn log_agent_chat_surface_lifecycle_receipt(&self, receipt: &AgentChatSurfaceLifecycleReceipt) {
        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_surface_lifecycle_receipt",
            receipt_json = %serde_json::to_string(receipt).unwrap_or_default(),
        );
    }
}
