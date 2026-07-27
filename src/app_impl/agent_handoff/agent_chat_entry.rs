use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatEntryOrigin {
    MainLauncher,
    FileSearch,
    ActionsDialog,
    PluginSkill { skill_id: String },
    Notes,
    Dictation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatThreadTarget {
    ExistingDetachedOrEmbedded,
    CurrentHostEmbedded,
    FreshEmbedded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatSeedPolicy {
    ComposerOnly,
    AutoSubmitFirstTurn,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentChatEntryRequest {
    pub(crate) origin: AgentChatEntryOrigin,
    pub(crate) target: AgentChatThreadTarget,
    pub(crate) seed_text: Option<String>,
    pub(crate) ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
    pub(crate) seed_policy: AgentChatSeedPolicy,
    pub(crate) context_policy: AgentChatContextPolicy,
    pub(crate) return_origin: Option<AppView>,
}

impl AgentChatEntryRequest {
    /// Shared launcher constructor. Private on purpose: callers pick a policy
    /// through the named constructors below so a variant/policy mismatch can
    /// never be expressed as a constructor argument.
    fn main_launcher_internal(
        seed_text: Option<String>,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
        context_policy: AgentChatContextPolicy,
    ) -> Self {
        Self {
            origin: AgentChatEntryOrigin::MainLauncher,
            target: AgentChatThreadTarget::ExistingDetachedOrEmbedded,
            seed_policy: if seed_text
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty())
            {
                AgentChatSeedPolicy::AutoSubmitFirstTurn
            } else {
                AgentChatSeedPolicy::ComposerOnly
            },
            seed_text,
            ui_variant,
            context_policy,
            return_origin: None,
        }
    }

    /// Standard launcher entry that MAY inherit the currently selected launcher
    /// row as ambient/focused context.
    pub(crate) fn main_launcher(seed_text: Option<String>) -> Self {
        Self::main_launcher_internal(
            seed_text,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            AgentChatContextPolicy::AmbientOrFocused,
        )
    }

    /// Standard launcher entry that must NOT inherit the selected launcher row.
    pub(crate) fn clean_main_launcher(seed_text: Option<String>) -> Self {
        Self::main_launcher_internal(
            seed_text,
            crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            AgentChatContextPolicy::SuppressFocused,
        )
    }

    /// Launcher entry whose context policy is derived exhaustively from the UI
    /// variant (Standard inherits, every nonstandard variant suppresses).
    pub(crate) fn main_launcher_with_variant(
        seed_text: Option<String>,
        ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
    ) -> Self {
        let context_policy = AgentChatContextPolicy::for_main_launcher_variant(ui_variant);
        Self::main_launcher_internal(seed_text, ui_variant, context_policy)
    }

    /// Quick-question entry: double-tap of the main hotkey (and any future
    /// "just open a clean chat" affordance).
    ///
    /// CONTRACT: this request must open Agent Chat with an EMPTY composer and
    /// NO context chips. In particular it must never inherit the launcher's
    /// auto-selected default row — the user double-tapped from anywhere, they
    /// did not pick that row. Regression reference (2026-07-10): double-tap
    /// routed through the chip-staging entry, so whatever happened to sit at
    /// `first_selectable_index` (a Brain Inbox capture, a flow row) was staged
    /// as an `@cmd:` chip and pre-filled into the composer. The unit tests
    /// below lock the suppression policy; do not weaken them.
    pub(crate) fn quick_question() -> Self {
        Self::clean_main_launcher(None)
    }

    /// Notes→main handoff: open (or reuse) the MAIN window's Agent Chat with
    /// the selected note staged as an explicit `@note` reference, leaving the
    /// Notes window open.
    ///
    /// CONTRACT: origin is `Notes`; target is main-host-only
    /// (`CurrentHostEmbedded` — never the detached chat window); the UI
    /// variant is Standard; nothing auto-submits; no ambient or implicit
    /// focused context is inherited; no seed text may displace the canonical
    /// `@note` prefill.
    pub(crate) fn notes(
        target: crate::ai::TabAiTargetContext,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
    ) -> Self {
        Self {
            origin: AgentChatEntryOrigin::Notes,
            target: AgentChatThreadTarget::CurrentHostEmbedded,
            seed_text: None,
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            seed_policy: AgentChatSeedPolicy::ComposerOnly,
            context_policy: AgentChatContextPolicy::NotesHandoff {
                target,
                supplemental_parts,
                source,
            },
            return_origin: None,
        }
    }

    /// Promote a bounded Quick AI result into a fresh full Agent Chat without
    /// submitting it or inheriting ambient launcher context.
    pub(crate) fn quick_ai_handoff(seed_text: String) -> Self {
        Self {
            origin: AgentChatEntryOrigin::MainLauncher,
            target: AgentChatThreadTarget::FreshEmbedded,
            seed_text: Some(seed_text),
            ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard,
            seed_policy: AgentChatSeedPolicy::ComposerOnly,
            context_policy: AgentChatContextPolicy::SuppressFocused,
            return_origin: None,
        }
    }
}

impl ScriptListApp {
    pub(crate) fn open_agent_chat_from_entry_request(
        &mut self,
        req: AgentChatEntryRequest,
        cx: &mut Context<Self>,
    ) {
        let _ = self.dispatch_agent_chat_entry_request(req, cx);
    }

    /// Open the MAIN window's Agent Chat from the Notes window with the
    /// selected note staged as the primary explicit context part and the
    /// persisted note-cart parts staged as supplemental chips.
    ///
    /// Returns `true` only when a live, non-setup main-window Agent Chat
    /// received the primary note context (supplemental staging failures are
    /// logged but do not retroactively fail the handoff).
    pub(crate) fn open_agent_chat_from_notes(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        self.dispatch_agent_chat_entry_request(
            AgentChatEntryRequest::notes(target, supplemental_parts, source),
            cx,
        )
    }

    /// Dispatch an entry request, reporting whether the intended destination
    /// received its initial state/context.
    ///
    /// Legacy (non-Notes) policies keep their historical fire-and-forget
    /// semantics and report `true` unconditionally; only the `NotesHandoff`
    /// branch computes a real staging result, which `open_agent_chat_from_notes`
    /// surfaces to the Notes window for cart-consumption decisions.
    fn dispatch_agent_chat_entry_request(
        &mut self,
        req: AgentChatEntryRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.agent_chat_surface_state.blocks_launcher_ai_entry() {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_entry_request_blocked_by_portal",
                origin = ?req.origin,
            );
            return false;
        }

        let source_view = req
            .return_origin
            .clone()
            .unwrap_or_else(|| self.current_view.clone());
        self.seed_agent_chat_return_origin_for_view(&source_view);

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_entry_request_open",
            origin = ?req.origin,
            target = ?req.target,
            agent_chat_ui_variant = req.ui_variant.state_id(),
            seed_policy = ?req.seed_policy,
            context_policy = ?req.context_policy,
            source_view = ?source_view,
        );

        let seed_policy = req.seed_policy.clone();
        // Exhaustive on purpose: `CurrentHostEmbedded` means "the main
        // ScriptListApp's own Agent Chat, never the detached chat window" —
        // it is not an alias for `ExistingDetachedOrEmbedded`. Its only
        // producer today is the NotesHandoff policy, whose handler below
        // never consults `chat_window` reuse paths.
        let force_fresh = match req.target {
            AgentChatThreadTarget::ExistingDetachedOrEmbedded => false,
            AgentChatThreadTarget::CurrentHostEmbedded => false,
            AgentChatThreadTarget::FreshEmbedded => true,
        };
        match req.context_policy {
            AgentChatContextPolicy::AmbientOrFocused => {
                self.open_tab_ai_agent_chat_with_options(
                    req.seed_text,
                    AgentChatContextPolicy::AmbientOrFocused,
                    req.ui_variant,
                    seed_policy,
                    force_fresh,
                    cx,
                );
                true
            }
            AgentChatContextPolicy::SuppressFocused => {
                self.open_tab_ai_agent_chat_with_options(
                    req.seed_text,
                    AgentChatContextPolicy::SuppressFocused,
                    req.ui_variant,
                    seed_policy,
                    force_fresh,
                    cx,
                );
                true
            }
            AgentChatContextPolicy::Parts { parts, source } => {
                // Fail closed: the supported explicit-parts contract is exactly
                // one part. Never fall back to ambient context for zero/multi.
                if parts.len() != 1 {
                    tracing::error!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_entry_parts_cardinality_unsupported",
                        part_count = parts.len(),
                    );
                    return false;
                }
                // Cardinality is exactly one (checked above); the `else` is
                // unreachable but kept fail-closed to honor the crate's
                // `clippy::expect_used` deny.
                let Some(part) = parts.into_iter().next() else {
                    return false;
                };
                self.open_tab_ai_agent_chat_with_context_part(part, source, cx);
                true
            }
            AgentChatContextPolicy::ActionsPayload { target } => {
                self.open_tab_ai_agent_chat_with_explicit_target(target, cx);
                true
            }
            AgentChatContextPolicy::NotesHandoff {
                target,
                supplemental_parts,
                source,
            } => self.open_tab_ai_agent_chat_for_notes_handoff(
                target,
                supplemental_parts,
                source,
                cx,
            ),
        }
    }

    /// Main-window handler for the Notes handoff policy.
    ///
    /// Decision order (locked by the Notes handoff contract):
    /// 1. Attachment portal or save-offer overlay active → fail closed.
    /// 2. Current view is a usable full-session Standard Agent Chat (not
    ///    setup, not focused-text mini) → stage into it, preserving messages
    ///    and any composer draft.
    /// 3. Otherwise open a Standard Agent Chat through the main-only
    ///    context-part seam and stage supplemental parts after the view
    ///    exists.
    ///
    /// Never consults the detached chat window; never auto-submits.
    fn open_tab_ai_agent_chat_for_notes_handoff(
        &mut self,
        target: crate::ai::TabAiTargetContext,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tab_ai_save_offer_state.is_some() {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_handoff_blocked_by_save_offer",
                source,
            );
            return false;
        }

        let label = crate::ai::format_explicit_target_chip_label(&target);
        let semantic_id = target.semantic_id.clone();
        let part = crate::ai::message_parts::AiContextPart::FocusedTarget { target, label };

        // Reuse the live main-window Agent Chat when it is a usable full
        // session: preserve its messages and composer draft.
        if let AppView::AgentChatView { entity, .. } = &self.current_view {
            let entity = entity.clone();
            let reusable = {
                let view = entity.read(cx);
                let is_setup = view.is_setup_mode();
                let is_mini = view.is_focused_text_mini();
                let variant = view.current_ui_variant();
                let policy = view.session_policy();
                let reusable = !is_setup
                    && !is_mini
                    && variant
                        == crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant::Standard
                    && policy
                        == crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::Full;
                tracing::info!(
                    target: "script_kit::tab_ai",
                    event = "notes_handoff_reuse_gate",
                    reusable,
                    is_setup,
                    is_mini,
                    variant = variant.state_id(),
                    policy = ?policy,
                );
                reusable
            };
            if reusable {
                let staged = entity.update(cx, |view, cx| {
                    view.stage_primary_context_part_from_host_preserving_composer(
                        part.clone(),
                        source,
                        cx,
                    )
                });
                return match staged {
                    Ok(()) => {
                        self.stage_notes_supplemental_parts(
                            &entity,
                            supplemental_parts,
                            source,
                            cx,
                        );
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "notes_handoff_main_staged",
                            source,
                            semantic_id = %semantic_id,
                            reused_existing_chat = true,
                        );
                        true
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "script_kit::tab_ai",
                            event = "notes_handoff_reuse_stage_failed",
                            source,
                            error = %error,
                        );
                        false
                    }
                };
            }
        }

        // Replacing an unsuitable Agent Chat view must not make Agent Chat its
        // own return target: derive a non-chat return origin first.
        let derived_return_origin = match &self.current_view {
            AppView::AgentChatView { .. } => self
                .tab_ai_harness_return_view
                .clone()
                .filter(|view| !matches!(view, AppView::AgentChatView { .. }))
                .unwrap_or(AppView::ScriptList),
            other => other.clone(),
        };

        self.open_tab_ai_agent_chat_with_context_part(part, source, cx);

        let staged_entity = match &self.current_view {
            AppView::AgentChatView { entity, .. } if !entity.read(cx).is_setup_mode() => {
                Some(entity.clone())
            }
            _ => None,
        };
        let Some(entity) = staged_entity else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_handoff_main_stage_failed",
                source,
                semantic_id = %semantic_id,
                reason = "no_live_agent_chat_view_after_open",
            );
            return false;
        };

        self.tab_ai_harness_return_view = Some(derived_return_origin);
        self.stage_notes_supplemental_parts(&entity, supplemental_parts, source, cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "notes_handoff_main_staged",
            source,
            semantic_id = %semantic_id,
            reused_existing_chat = false,
        );
        true
    }

    /// Stage persisted note-cart parts as supplemental context chips without
    /// touching the composer. A supplemental failure is logged, never fatal —
    /// the primary note context already staged successfully.
    fn stage_notes_supplemental_parts(
        &mut self,
        entity: &gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
        supplemental_parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
        cx: &mut Context<Self>,
    ) {
        if supplemental_parts.is_empty() {
            return;
        }
        let result = entity.update(cx, |view, cx| {
            view.stage_supplemental_context_parts_from_host(supplemental_parts, source, cx)
        });
        if let Err(error) = result {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "notes_handoff_supplemental_stage_failed",
                source,
                error = %error,
            );
        }
    }
}

#[cfg(test)]
mod quick_question_contract {
    // Deliberately no `use super::*`: the parent glob-imports gpui, whose
    // `test` macro would shadow the builtin `#[test]` attribute.
    use super::{AgentChatContextPolicy, AgentChatEntryRequest, AgentChatSeedPolicy};

    /// Double-tap of the main hotkey means "fastest path to a clean chat for
    /// a quick question". It must never carry the launcher's auto-selected
    /// row (or any other implicit context) into Agent Chat.
    #[test]
    fn quick_question_entry_suppresses_all_implicit_context() {
        let req = AgentChatEntryRequest::quick_question();
        assert_eq!(
            req.context_policy,
            AgentChatContextPolicy::SuppressFocused,
            "quick-question entry must suppress the focused launcher row",
        );
        assert!(
            req.seed_text.is_none(),
            "quick-question entry must open with an empty composer",
        );
        assert_eq!(
            req.seed_policy,
            AgentChatSeedPolicy::ComposerOnly,
            "quick-question entry must never auto-submit",
        );
    }

    #[test]
    fn quick_ai_handoff_is_fresh_composer_only_and_context_free() {
        let req = AgentChatEntryRequest::quick_ai_handoff("bounded result".to_string());
        assert_eq!(req.target, super::AgentChatThreadTarget::FreshEmbedded,);
        assert_eq!(req.seed_policy, AgentChatSeedPolicy::ComposerOnly);
        assert_eq!(req.context_policy, AgentChatContextPolicy::SuppressFocused,);
        assert_eq!(req.seed_text.as_deref(), Some("bounded result"));
    }

    fn notes_target_fixture() -> crate::ai::TabAiTargetContext {
        crate::ai::TabAiTargetContext {
            source: "Notes".to_string(),
            kind: "note".to_string(),
            semantic_id: "note:test-fixture".to_string(),
            label: "Test Note".to_string(),
            metadata: None,
        }
    }

    /// The Notes handoff is main-host-only, composer-only, and fully
    /// explicit: origin Notes, target CurrentHostEmbedded (never the
    /// detached chat window), Standard variant, no seed text, no
    /// auto-submit, and a NotesHandoff context policy.
    #[test]
    fn notes_entry_is_main_host_composer_only_and_explicit() {
        let req = AgentChatEntryRequest::notes(notes_target_fixture(), Vec::new(), "test");
        assert_eq!(
            req.target,
            super::AgentChatThreadTarget::CurrentHostEmbedded,
            "notes entry must target the main host's own Agent Chat",
        );
        assert_ne!(
            req.target,
            super::AgentChatThreadTarget::ExistingDetachedOrEmbedded,
            "notes entry must never target the detached chat window",
        );
        assert!(req.seed_text.is_none(), "notes entry carries no seed text");
        assert_eq!(
            req.seed_policy,
            AgentChatSeedPolicy::ComposerOnly,
            "notes entry must never auto-submit",
        );
        assert!(
            matches!(
                req.context_policy,
                AgentChatContextPolicy::NotesHandoff { .. }
            ),
            "notes entry must carry the NotesHandoff context policy",
        );
    }

    /// The launcher's implicitly selected row must never leak into a
    /// Notes-origin request.
    #[test]
    fn notes_entry_never_admits_implicit_focused_context() {
        let req = AgentChatEntryRequest::notes(notes_target_fixture(), Vec::new(), "test");
        assert!(
            !req.context_policy.admits_implicit_focused_part(),
            "NotesHandoff must suppress the implicit focused row",
        );
    }

    /// Behavior replacement for the former source-string audit that lived in
    /// `src/ai/agent_chat/ui/tests.rs`: Standard launcher entry may inherit the
    /// selected launcher row, every nonstandard variant must suppress it. Kept
    /// under the same test name so existing CI filters continue to work.
    #[test]
    fn agent_chat_ui_variant_launch_suppresses_selected_launcher_row_context_contract() {
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
        let standard =
            AgentChatEntryRequest::main_launcher_with_variant(None, AgentChatUiVariant::Standard);
        assert_eq!(
            standard.context_policy,
            AgentChatContextPolicy::AmbientOrFocused,
            "Standard launcher entry may inherit the selected launcher row",
        );
        for variant in [
            AgentChatUiVariant::UserBold,
            AgentChatUiVariant::RoleSplit,
            AgentChatUiVariant::BottomDock,
            AgentChatUiVariant::DenseLog,
            AgentChatUiVariant::Sidecar,
            AgentChatUiVariant::FocusedTextMini,
            AgentChatUiVariant::QuickAi,
        ] {
            let req = AgentChatEntryRequest::main_launcher_with_variant(None, variant);
            assert_eq!(
                req.context_policy,
                AgentChatContextPolicy::SuppressFocused,
                "{variant:?} launcher entry must not inherit the selected launcher row",
            );
        }
    }
}
