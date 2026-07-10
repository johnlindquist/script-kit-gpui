//! Portal lifecycle and host-capability policy for Agent Chat.

use gpui::{App, Context};

use super::super::composer_state::AgentChatComposerPickerDismissReason;
use super::super::types::AgentChatPendingPortalSession;
use super::AgentChatView;

/// Portal open callback — receives the portal kind so the host can open the
/// appropriate built-in view (file search, clipboard history, etc.).
/// Takes `&mut App` (not `&mut Window`) because the handler opens a new view
/// via entity update, and this callback is invoked from contexts where
/// `Window` is not available (e.g. `accept_composer_picker_selection_impl`).
pub(super) type AgentChatPortalHandler = std::sync::Arc<
    dyn Fn(crate::ai::context_selector::types::ContextPortalKind, &mut App) + 'static,
>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortalRefusal {
    NoHost,
    UnsupportedByHost,
    OpenFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortalOpenResult {
    Opened,
    Refused(PortalRefusal),
}

impl AgentChatView {
    /// All portal kinds — the default for launcher/detached Agent Chat surfaces.
    pub(super) fn all_portal_kinds() -> Vec<crate::ai::context_selector::types::ContextPortalKind> {
        use crate::ai::context_selector::types::ContextPortalKind;
        vec![
            ContextPortalKind::AgentChatHistory,
            ContextPortalKind::FileSearch,
            ContextPortalKind::BrowserHistory,
            ContextPortalKind::BrowserTabs,
            ContextPortalKind::ClipboardHistory,
            ContextPortalKind::DictationHistory,
            ContextPortalKind::ScriptSearch,
            ContextPortalKind::ScriptletSearch,
            ContextPortalKind::SkillSearch,
            ContextPortalKind::NotesBrowse,
            ContextPortalKind::Terminal,
        ]
    }

    pub(crate) fn set_on_open_portal(
        &mut self,
        callback: impl Fn(crate::ai::context_selector::types::ContextPortalKind, &mut App) + 'static,
    ) {
        self.on_open_portal = Some(std::sync::Arc::new(callback));
    }

    /// Restrict portal kinds this Agent Chat surface can open.
    ///
    /// Items for disallowed kinds are filtered from the composer picker and
    /// rejected at the portal-open dispatch. Call before wiring host callbacks.
    pub(crate) fn set_allowed_portal_kinds(
        &mut self,
        kinds: Vec<crate::ai::context_selector::types::ContextPortalKind>,
    ) {
        self.allowed_portal_kinds = kinds;
    }

    /// Whether the given portal kind is allowed by the host.
    pub(super) fn is_portal_kind_allowed(
        &self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
    ) -> bool {
        self.allowed_portal_kinds.contains(&kind)
    }

    pub(crate) fn prepare_for_attachment_portal_open(&mut self, cx: &mut Context<Self>) {
        self.attach_menu_open = false;
        self.permission_options_open = false;
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::PortalStaged, cx);
        self.history_menu = None;
        if let Some(card) = &self.setup_card {
            card.update(cx, |view, cx| view.set_agent_picker(None, cx));
        }

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_attachment_portal_prepare",
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn resume_after_attachment_portal_close(&mut self, cx: &mut Context<Self>) {
        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_attachment_portal_resume",
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(super) fn has_pending_history_portal_session(&self) -> bool {
        matches!(
            self.pending_portal_session.as_ref(),
            Some(session)
                if session.contract.portal_kind
                    == crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory
        )
    }

    /// Read the staged portal query for `kind`.
    pub(crate) fn portal_query_for(
        &self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
    ) -> Option<String> {
        self.pending_portal_session
            .as_ref()
            .filter(|session| session.contract.portal_kind == kind)
            .map(|session| {
                crate::ai::agent_chat::ui::portal_contract::picker_portal_query(
                    kind,
                    &session.contract.query,
                )
            })
    }

    /// Backward-compatible helper for the Agent Chat history host flow.
    pub(crate) fn take_pending_history_portal_query(&mut self) -> Option<String> {
        self.portal_query_for(
            crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory,
        )
    }

    fn stage_pending_portal_session(
        &mut self,
        contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
        cx: &mut Context<Self>,
    ) {
        let thread = self.live_thread().read(cx);
        let composer_text = thread.input.text().to_string();
        let composer_cursor = thread.input.cursor();
        let replace_label = contract.replacement.preview_label();

        let Some(staged_state) = crate::ai::agent_chat::ui::portal_contract::next_portal_state(
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState::Idle,
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionEvent::Stage,
        ) else {
            tracing::error!(
                target: "script_kit::agent_chat",
                event = "agent_chat_portal_stage_state_missing",
                "idle portal session failed to stage"
            );
            return;
        };

        self.pending_portal_session = Some(AgentChatPendingPortalSession {
            contract: contract.clone(),
            composer_text,
            composer_cursor,
            state: staged_state,
        });
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::PortalStaged, cx);
        self.history_menu = None;
        self.attach_menu_open = false;

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_contract_staged",
            kind = ?contract.portal_kind,
            query = %contract.query,
            replace_label = %replace_label,
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn attach_portal_part(
        &mut self,
        part: crate::ai::message_parts::AiContextPart,
        cx: &mut Context<Self>,
    ) {
        use crate::ai::context_mentions::part_to_inline_token;

        let inline_token =
            part_to_inline_token(&part).unwrap_or_else(|| format!("@{}", part.label()));
        let should_claim_inline_ownership = self.should_claim_inline_mention_ownership(&part, cx);
        let current_text = self.live_thread().read(cx).input.text().to_string();
        let replacement = format!("{inline_token} ");

        let pending_portal_session = self.pending_portal_session.take();
        let (next_text, next_cursor, exact_match) =
            if let Some(session) = pending_portal_session.as_ref() {
                debug_assert_eq!(
                    session.state,
                    crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState::Active
                );
                crate::ai::agent_chat::ui::portal_contract::apply_portal_replacement(
                    &current_text,
                    &session.contract.replacement,
                    &replacement,
                )
            } else {
                let separator = if current_text.is_empty() || current_text.ends_with(' ') {
                    ""
                } else {
                    " "
                };
                let next_text = format!("{current_text}{separator}{inline_token} ");
                let next_cursor = next_text.chars().count();
                (next_text, next_cursor, false)
            };

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_reentry_applied",
            exact_match,
            new_token = %inline_token,
            portal_kind = ?pending_portal_session
                .as_ref()
                .map(|session| session.contract.portal_kind),
        );

        self.live_thread().update(cx, |thread, cx| {
            thread.input.set_text(next_text);
            thread.input.set_cursor(next_cursor);
            thread.add_context_part(part.clone(), cx);
            cx.notify();
        });

        self.register_typed_alias(inline_token.clone(), part);
        if should_claim_inline_ownership {
            self.register_inline_owned_token(inline_token);
        }
        self.sync_inline_mentions(cx);
        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
    }

    pub(crate) fn cancel_pending_portal_session(
        &mut self,
        portal_kind: crate::ai::context_selector::types::ContextPortalKind,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(session) = self.pending_portal_session.take() else {
            return false;
        };
        if session.contract.portal_kind != portal_kind {
            self.pending_portal_session = Some(session);
            return false;
        }

        let Some(state) = crate::ai::agent_chat::ui::portal_contract::next_portal_state(
            session.state,
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionEvent::Cancel,
        ) else {
            self.pending_portal_session = Some(session);
            return false;
        };
        let restore_text = session.composer_text.clone();
        let restore_cursor = session.composer_cursor;
        let cleared_state =
            crate::ai::agent_chat::ui::portal_contract::clear_terminal_portal_state(state);
        debug_assert_eq!(
            cleared_state,
            crate::ai::agent_chat::ui::portal_contract::AgentChatPortalSessionState::Idle
        );

        self.live_thread().update(cx, |thread, cx| {
            let cursor = restore_cursor.min(restore_text.chars().count());
            thread.input.set_text(restore_text.clone());
            thread.input.set_cursor(cursor);
            cx.notify();
        });

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_session_cancelled",
            kind = ?portal_kind,
            restored_cursor = restore_cursor,
        );

        self.sync_agent_chat_popup_windows_from_cached_parent(cx);
        cx.notify();
        true
    }

    pub(super) fn open_portal_contract(
        &mut self,
        contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
        cx: &mut Context<Self>,
    ) -> bool {
        matches!(
            self.open_portal_contract_result(contract, cx),
            PortalOpenResult::Opened
        )
    }

    fn open_portal_contract_result(
        &mut self,
        contract: crate::ai::agent_chat::ui::portal_contract::AgentChatPortalLaunchContract,
        cx: &mut Context<Self>,
    ) -> PortalOpenResult {
        use crate::ai::agent_chat::ui::portal_contract::{
            decide_portal_open, next_portal_state, AgentChatPortalOpenRefusal,
            AgentChatPortalSessionEvent, AgentChatPortalSessionState,
        };

        let portal_kind = contract.portal_kind;
        let query = contract.query.clone();
        let is_allowed = self.is_portal_kind_allowed(portal_kind);
        let has_host_callback = self.on_open_portal.is_some();

        tracing::info!(
            target: "script_kit::agent_chat",
            event = "agent_chat_portal_open_decision",
            kind = ?portal_kind,
            allowed = is_allowed,
            has_host_callback,
        );

        match decide_portal_open(is_allowed, has_host_callback) {
            Ok(()) => {}
            Err(AgentChatPortalOpenRefusal::UnsupportedByHost) => {
                tracing::info!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_portal_blocked_by_host_capability",
                    kind = ?portal_kind,
                );
                return PortalOpenResult::Refused(PortalRefusal::UnsupportedByHost);
            }
            Err(AgentChatPortalOpenRefusal::MissingHostCallback) => {
                tracing::warn!(
                    target: "script_kit::agent_chat",
                    event = "agent_chat_portal_open_blocked_missing_host_callback",
                    kind = ?portal_kind,
                );
                return PortalOpenResult::Refused(PortalRefusal::NoHost);
            }
        }

        let Some(callback) = self.on_open_portal.clone() else {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "agent_chat_portal_open_blocked_missing_host_callback",
                kind = ?portal_kind,
            );
            return PortalOpenResult::Refused(PortalRefusal::NoHost);
        };
        self.stage_pending_portal_session(contract, cx);
        if let Some(session) = self.pending_portal_session.as_mut() {
            session.state = next_portal_state(session.state, AgentChatPortalSessionEvent::Activate)
                .unwrap_or(AgentChatPortalSessionState::Active);
        }
        if portal_kind == crate::ai::context_selector::types::ContextPortalKind::AgentChatHistory {
            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_history_portal_query_staged",
                query = %query,
            );
        }
        cx.defer(move |cx| {
            callback(portal_kind, cx);
        });
        cx.notify();
        PortalOpenResult::Opened
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext};

    use super::*;
    use crate::ai::agent_chat::ui::portal_contract::{
        AgentChatPortalLaunchContract, AgentChatPortalReplacementTarget,
    };
    use crate::ai::agent_chat::ui::preflight::AgentChatLaunchRequirements;
    use crate::ai::agent_chat::ui::setup_state::{AgentChatInlineSetupState, AgentChatSetupAction};
    use crate::ai::context_selector::types::ContextPortalKind;

    fn setup_state() -> AgentChatInlineSetupState {
        AgentChatInlineSetupState {
            reason_code: "noAgentsAvailable",
            title: "No agents".into(),
            body: "test".into(),
            primary_action: AgentChatSetupAction::OpenCatalog,
            secondary_action: None,
            selected_agent: None,
            catalog_entries: Vec::new(),
            launch_requirements: AgentChatLaunchRequirements::default(),
        }
    }

    fn portal_contract(portal_kind: ContextPortalKind) -> AgentChatPortalLaunchContract {
        AgentChatPortalLaunchContract {
            portal_kind,
            query: String::new(),
            replacement: AgentChatPortalReplacementTarget::AppendAtCursor { cursor: 0 },
        }
    }

    #[gpui::test]
    fn setup_mode_refuses_missing_and_disallowed_portals_without_staging(cx: &mut TestAppContext) {
        let view = cx.new(|cx| AgentChatView::new_setup(setup_state(), cx));

        view.update(cx, |view, cx| {
            let missing_callback = view
                .open_portal_contract_result(portal_contract(ContextPortalKind::FileSearch), cx);
            assert_eq!(
                missing_callback,
                PortalOpenResult::Refused(PortalRefusal::NoHost)
            );
            assert!(view.pending_portal_session.is_none());

            view.set_on_open_portal(|_, _| {});
            view.set_allowed_portal_kinds(vec![ContextPortalKind::AgentChatHistory]);
            let disallowed_kind = view
                .open_portal_contract_result(portal_contract(ContextPortalKind::FileSearch), cx);
            assert_eq!(
                disallowed_kind,
                PortalOpenResult::Refused(PortalRefusal::UnsupportedByHost)
            );
            assert!(view.pending_portal_session.is_none());
        });
    }
}
