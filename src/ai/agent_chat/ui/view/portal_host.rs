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

    /// Whether the given portal kind is allowed: an INTERSECTION of the
    /// host allowlist and the surface's immutable capability policy (Oracle
    /// 2026-07-21 WP3-B). A detached-host allowlist must never re-enable a
    /// portal the session policy (e.g. Quick AI zero-context) forbids.
    pub(super) fn is_portal_kind_allowed(
        &self,
        kind: crate::ai::context_selector::types::ContextPortalKind,
    ) -> bool {
        // Fail-closed on the view's captured policy (no `cx` here). It is
        // derived from the thread at construction and is tighten-only, so it
        // is never LESS restrictive than the thread — safe for a restriction
        // gate. The cx-based `capabilities(cx)` at the portal-open dispatch
        // (`open_picker_portal`) is the authoritative second layer.
        self.session_policy.capabilities().context_portals
            && self.allowed_portal_kinds.contains(&kind)
    }

    pub(crate) fn prepare_for_attachment_portal_open(&mut self, cx: &mut Context<Self>) {
        self.attach_menu_open = false;
        self.permission_options_open = false;
        self.clear_composer_picker(AgentChatComposerPickerDismissReason::PortalStaged, cx);
        self.close_history_popup_for_owner_transition("attachment_portal_opened", true, cx);
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
        self.close_history_popup_for_owner_transition("portal_session_staged", true, cx);
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

    /// WP3-C: the detached-window host paths (`open_history_portal_with_entries`,
    /// `open_history_popup_from_host`) must honor the same immutable session
    /// policy as the in-view `toggle_history_popup` — a Quick AI session never
    /// resurfaces conversation history, no matter which host asks.
    #[gpui::test]
    fn quick_ai_policy_refuses_host_driven_history_portal(cx: &mut TestAppContext) {
        use crate::ai::agent_chat::ui::history::{
            AgentChatHistoryEntry, AgentChatHistorySearchField, AgentChatHistorySearchHit,
        };

        let hit = || {
            vec![AgentChatHistorySearchHit {
                entry: AgentChatHistoryEntry::default(),
                score: 0,
                matched_field: AgentChatHistorySearchField::Title,
                evidence: None,
            }]
        };

        // Setup-mode popup sync clears the staged menu, so the gate contract
        // is asserted on the return value: accepted (true) vs refused (false).
        let full = cx.new(|cx| AgentChatView::new_setup(setup_state(), cx));
        full.update(cx, |view, cx| {
            assert!(
                view.open_history_portal_with_entries("q".into(), hit(), cx),
                "a Full-policy view must accept host-driven history portals"
            );
        });

        // BC-1 (Oracle seat 3): the QuickAi policy is now established by the
        // LAUNCH (a policy-changing `set_ui_variant` restyle is refused), so the
        // QuickAi view is constructed with its policy, not laundered into it.
        let quick = cx.new(|cx| {
            AgentChatView::new_setup_with_policy(
                setup_state(),
                crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::QuickAi,
                cx,
            )
        });
        quick.update(cx, |view, cx| {
            assert!(
                !view.open_history_portal_with_entries("q".into(), hit(), cx),
                "a QuickAi-policy view must refuse host-driven history portals"
            );
            assert!(view.history_menu.is_none());

            // Attaching a prior conversation is the same retained-context
            // capability; the policy error fires before any disk access.
            let denied = view
                .attach_history_session(
                    "any-session",
                    crate::ai::agent_chat::ui::history_attachment::AgentChatHistoryAttachMode::Summary,
                    cx,
                )
                .expect_err("QuickAi policy must deny history attachments");
            assert!(
                denied.to_string().contains("session policy"),
                "denial must be the policy error, not a lookup failure: {denied}"
            );
        });
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

    // ── WP-B1 / C-R3: thread-owned policy authority ─────────────────────

    use crate::ai::agent_chat::ui::capabilities::{AgentChatCapabilities, AgentChatSessionPolicy};
    use crate::ai::agent_chat::ui::thread::AgentChatThread;
    use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
    use crate::footer_popup::FooterAction;
    use crate::spine::list::{SpineListAction, SpineListRow, SpineListRowKind, SpineListSection};

    fn quick_ai_thread(cx: &mut TestAppContext) -> gpui::Entity<AgentChatThread> {
        cx.new(|_cx| {
            let mut thread = AgentChatThread::test_new(Vec::new(), None);
            thread.set_session_policy_test(AgentChatSessionPolicy::QuickAi);
            thread
        })
    }

    /// C-R3: the pure footer-dispatch guard denies exactly the policy-forbidden
    /// actions (CWD picker, profile/model switch) and allows everything else.
    #[test]
    fn footer_action_allowed_matrix() {
        let full = AgentChatCapabilities::FULL;
        let quick = AgentChatCapabilities::QUICK_AI;

        // Full allows every footer action.
        for action in [
            FooterAction::Cwd,
            FooterAction::Ai,
            FooterAction::AgentModel,
            FooterAction::Run,
            FooterAction::Actions,
        ] {
            assert!(AgentChatView::footer_action_allowed(full, action));
        }

        // Quick AI denies the context/profile actions, allows the rest.
        assert!(!AgentChatView::footer_action_allowed(
            quick,
            FooterAction::Cwd
        ));
        assert!(!AgentChatView::footer_action_allowed(
            quick,
            FooterAction::Ai
        ));
        assert!(!AgentChatView::footer_action_allowed(
            quick,
            FooterAction::AgentModel
        ));
        assert!(AgentChatView::footer_action_allowed(
            quick,
            FooterAction::Run
        ));
        assert!(AgentChatView::footer_action_allowed(
            quick,
            FooterAction::Actions
        ));
    }

    /// WP-B1: a Quick AI launch FAILURE produces a Quick-AI-policy setup view,
    /// not a default-Full one — otherwise the error card would re-advertise
    /// denied affordances (history, CWD, profile switch).
    #[gpui::test]
    fn quick_ai_setup_failure_preserves_quick_policy(cx: &mut TestAppContext) {
        let quick = cx.new(|cx| {
            AgentChatView::new_setup_with_policy(setup_state(), AgentChatSessionPolicy::QuickAi, cx)
        });
        quick.update(cx, |view, cx| {
            assert_eq!(
                view.effective_session_policy(cx),
                AgentChatSessionPolicy::QuickAi
            );
            assert_eq!(view.capabilities(cx), AgentChatCapabilities::QUICK_AI);
        });

        // Standard setup stays Full.
        let full = cx.new(|cx| AgentChatView::new_setup(setup_state(), cx));
        full.update(cx, |view, cx| {
            assert_eq!(view.capabilities(cx), AgentChatCapabilities::FULL);
        });
    }

    /// WP-B1 / BC-1: with a live thread, the THREAD's immutable policy is the
    /// sole authority. A policy-changing restyle is now REFUSED outright (a
    /// relaunch is required), so the view's capabilities can never diverge from
    /// what the thread enforces — a QuickAi thread stays QuickAi and the
    /// requested Full-policy variant never even takes effect.
    #[gpui::test]
    fn view_and_thread_policy_cannot_diverge(cx: &mut TestAppContext) {
        use crate::ai::agent_chat::ui::view::AgentChatRestyleOutcome;

        let thread = quick_ai_thread(cx);
        let view = cx.new(|cx| AgentChatView::new(thread.clone(), cx));

        view.update(cx, |view, cx| {
            // Constructed policy is derived from the QuickAi thread.
            assert_eq!(view.capabilities(cx), AgentChatCapabilities::QUICK_AI);
            let before_variant = view.debug_ui_variant_id();

            // Restyle to a Full-policy presentation variant. The restyle is
            // refused (it would change the policy), so the variant is unchanged
            // and capabilities stay QuickAi.
            let outcome = view.set_ui_variant(AgentChatUiVariant::UserBold, cx);
            assert_eq!(outcome, AgentChatRestyleOutcome::RefusedRelaunchRequired);
            assert_eq!(
                view.debug_ui_variant_id(),
                before_variant,
                "a refused restyle must not change the active variant",
            );
            assert_eq!(
                view.effective_session_policy(cx),
                AgentChatSessionPolicy::QuickAi,
                "a view restyle must not diverge from the thread policy",
            );
            assert_eq!(view.capabilities(cx), AgentChatCapabilities::QUICK_AI);
        });
    }

    /// BC-1 (Oracle seat 3): a policy-changing restyle in EITHER direction is
    /// refused and requires a relaunch. A same-policy restyle (Standard↔UserBold,
    /// both Full) still applies.
    #[gpui::test]
    fn policy_changing_restyle_requires_relaunch(cx: &mut TestAppContext) {
        use crate::ai::agent_chat::ui::view::AgentChatRestyleOutcome;

        // Full live thread: Full → QuickAi is a policy change → refused.
        let full_thread = cx.new(|_cx| AgentChatThread::test_new(Vec::new(), None));
        let full_view = cx.new(|cx| AgentChatView::new(full_thread.clone(), cx));
        full_view.update(cx, |view, cx| {
            assert_eq!(view.capabilities(cx), AgentChatCapabilities::FULL);
            let before = view.debug_ui_variant_id();
            let outcome = view.set_ui_variant(AgentChatUiVariant::QuickAi, cx);
            assert_eq!(outcome, AgentChatRestyleOutcome::RefusedRelaunchRequired);
            assert_eq!(view.debug_ui_variant_id(), before);
            assert_eq!(
                view.effective_session_policy(cx),
                AgentChatSessionPolicy::Full,
            );

            // Same-policy restyle (Full → Full) applies.
            let applied = view.set_ui_variant(AgentChatUiVariant::UserBold, cx);
            assert_eq!(applied, AgentChatRestyleOutcome::Applied);
            assert_eq!(
                view.debug_ui_variant_id(),
                AgentChatUiVariant::UserBold.state_id()
            );
            assert_eq!(view.capabilities(cx), AgentChatCapabilities::FULL);
        });

        // QuickAi live thread (view presents as the default Standard variant,
        // but the thread policy is QuickAi): restyling to any Full-policy
        // variant is a policy change → refused. Use UserBold so it is not a
        // no-op against the current Standard variant.
        let quick_thread = quick_ai_thread(cx);
        let quick_view = cx.new(|cx| AgentChatView::new(quick_thread.clone(), cx));
        quick_view.update(cx, |view, cx| {
            assert_eq!(
                view.effective_session_policy(cx),
                AgentChatSessionPolicy::QuickAi,
            );
            let before = view.debug_ui_variant_id();
            let outcome = view.set_ui_variant(AgentChatUiVariant::UserBold, cx);
            assert_eq!(outcome, AgentChatRestyleOutcome::RefusedRelaunchRequired);
            assert_eq!(view.debug_ui_variant_id(), before);
            assert_eq!(
                view.effective_session_policy(cx),
                AgentChatSessionPolicy::QuickAi,
            );
        });
    }

    /// BC-1 (Oracle seat 3): the retained-thread machinery is hard-gated at the
    /// method boundary for a Quick AI surface — summaries are empty and
    /// activation is refused — so the switcher is inert by data, not merely
    /// hidden by the UI.
    #[gpui::test]
    fn quick_ai_retained_thread_machinery_is_inert(cx: &mut TestAppContext) {
        let quick_thread = quick_ai_thread(cx);
        let view = cx.new(|cx| AgentChatView::new(quick_thread.clone(), cx));

        // A second thread we attempt (and fail) to activate.
        let other = cx.new(|_cx| AgentChatThread::test_new(Vec::new(), None));

        view.update(cx, |view, cx| {
            assert!(
                view.retained_thread_summaries(cx).is_empty(),
                "QuickAi reports no retained threads",
            );

            let active_before = view.thread().map(|t| t.entity_id());
            view.activate_session_thread(other.clone(), cx);
            assert_eq!(
                view.thread().map(|t| t.entity_id()),
                active_before,
                "QuickAi refuses retained-thread activation — session thread unchanged",
            );
            assert!(view.retained_thread_summaries(cx).is_empty());
        });
    }

    /// BC-2 (Oracle seat 3): the shared session-transition closer clears every
    /// transient overlay (attach / permission / history / portal / picker).
    #[gpui::test]
    fn close_transient_ui_clears_every_overlay(cx: &mut TestAppContext) {
        let thread = cx.new(|_cx| AgentChatThread::test_new(Vec::new(), None));
        let view = cx.new(|cx| AgentChatView::new(thread.clone(), cx));

        view.update(cx, |view, cx| {
            view.attach_menu_open = true;
            view.permission_options_open = true;

            view.close_transient_ui_for_session_transition(cx);

            assert!(!view.attach_menu_open, "attach menu closed");
            assert!(!view.permission_options_open, "permission options closed");
            assert!(view.history_menu.is_none(), "history menu closed");
            assert!(
                view.pending_portal_session.is_none(),
                "pending portal cleared"
            );
            assert!(
                view.composer_picker_session.is_none(),
                "composer picker closed",
            );
        });
    }

    /// BC-2 (Oracle seat 3): the runtime `SetupRequired` transition edge closes
    /// every transient overlay so a menu staged against the errored chat cannot
    /// linger over the setup card.
    #[gpui::test]
    fn setup_required_transition_closes_transient_popups(cx: &mut TestAppContext) {
        let thread = cx.new(|_cx| AgentChatThread::test_new(Vec::new(), None));
        let view = cx.new(|cx| AgentChatView::new(thread.clone(), cx));

        view.update(cx, |view, _cx| {
            view.attach_menu_open = true;
            view.permission_options_open = true;
        });

        // Drive the live thread into runtime setup recovery; the view observer
        // detects the None→Some edge and closes transients.
        thread.update(cx, |t, cx| t.replace_setup_state(setup_state(), cx));
        cx.run_until_parked();

        view.update(cx, |view, _cx| {
            assert!(
                view.runtime_setup_active_seen,
                "the runtime setup edge was observed",
            );
            assert!(
                !view.attach_menu_open,
                "SetupRequired closed the attach menu"
            );
            assert!(
                !view.permission_options_open,
                "SetupRequired closed permission options",
            );
            assert!(view.history_menu.is_none());
            assert!(view.pending_portal_session.is_none());
            assert!(view.composer_picker_session.is_none());
        });
    }

    /// C-R3: typing `>` (the CWD picker) cannot even DISPLAY a working-directory
    /// row in a Quick AI composer — the spine projection filter drops CWD and
    /// profile sections the policy denies.
    #[gpui::test]
    fn quick_ai_cwd_and_profile_spine_projection_denied(cx: &mut TestAppContext) {
        let thread = quick_ai_thread(cx);
        let view = cx.new(|cx| AgentChatView::new(thread.clone(), cx));

        let cwd_section = SpineListSection {
            id: "cwd".into(),
            title: "Working Directory".into(),
            subtitle: None,
            icon: None,
            rows: vec![SpineListRow {
                id: "cwd-row".into(),
                kind: SpineListRowKind::Hint,
                title: "~/dev".into(),
                subtitle: None,
                meta: None,
                icon: None,
                badges: Vec::new(),
                score: 0,
                is_selectable: true,
                action_label: None,
                action: SpineListAction::ResolveSegment {
                    segment_index: 0,
                    segment_byte_range: 0..1,
                    replacement: "".into(),
                    resolution_id: "/Users/dev".into(),
                    resolution_label: "~/dev".into(),
                    resolution_source: "cwd".into(),
                    trailing_space: false,
                },
            }],
        };
        let profile_section = SpineListSection {
            id: "profile".into(),
            title: "Profiles".into(),
            subtitle: None,
            icon: None,
            rows: vec![SpineListRow {
                id: "profile-row".into(),
                kind: SpineListRowKind::Profile {
                    profile_id: "p1".into(),
                },
                title: "Profile One".into(),
                subtitle: None,
                meta: None,
                icon: None,
                badges: Vec::new(),
                score: 0,
                is_selectable: true,
                action_label: None,
                action: SpineListAction::ResolveSegment {
                    segment_index: 0,
                    segment_byte_range: 0..1,
                    replacement: "".into(),
                    resolution_id: "p1".into(),
                    resolution_label: "Profile One".into(),
                    resolution_source: "profile".into(),
                    trailing_space: false,
                },
            }],
        };
        let file_section = SpineListSection {
            id: "files".into(),
            title: "Files".into(),
            subtitle: None,
            icon: None,
            rows: vec![SpineListRow {
                id: "file-row".into(),
                kind: SpineListRowKind::ContextResult {
                    context_type: "file".into(),
                    result_id: "/a".into(),
                },
                title: "a.rs".into(),
                subtitle: None,
                meta: None,
                icon: None,
                badges: Vec::new(),
                score: 0,
                is_selectable: true,
                action_label: None,
                action: SpineListAction::ResolveSegment {
                    segment_index: 0,
                    segment_byte_range: 0..1,
                    replacement: "@file:a.rs".into(),
                    resolution_id: "/a".into(),
                    resolution_label: "a.rs".into(),
                    resolution_source: "file".into(),
                    trailing_space: true,
                },
            }],
        };

        view.update(cx, |view, _cx| {
            let filtered = view.filter_agent_chat_spine_sections_by_policy(vec![
                cwd_section,
                profile_section,
                file_section,
            ]);
            // Only the non-denied file section survives for Quick AI.
            assert_eq!(filtered.len(), 1);
            assert_eq!(filtered[0].id.as_ref(), "files");
        });
    }
}
