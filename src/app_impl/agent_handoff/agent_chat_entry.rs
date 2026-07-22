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
}

impl ScriptListApp {
    pub(crate) fn open_agent_chat_from_entry_request(
        &mut self,
        req: AgentChatEntryRequest,
        cx: &mut Context<Self>,
    ) {
        if self.agent_chat_surface_state.blocks_launcher_ai_entry() {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "agent_chat_entry_request_blocked_by_portal",
                origin = ?req.origin,
            );
            return;
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

        match req.context_policy {
            AgentChatContextPolicy::AmbientOrFocused => {
                self.open_tab_ai_agent_chat_with_options(
                    req.seed_text,
                    AgentChatContextPolicy::AmbientOrFocused,
                    req.ui_variant,
                    cx,
                );
            }
            AgentChatContextPolicy::SuppressFocused => {
                self.open_tab_ai_agent_chat_with_options(
                    req.seed_text,
                    AgentChatContextPolicy::SuppressFocused,
                    req.ui_variant,
                    cx,
                );
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
                    return;
                }
                // Cardinality is exactly one (checked above); the `else` is
                // unreachable but kept fail-closed to honor the crate's
                // `clippy::expect_used` deny.
                let Some(part) = parts.into_iter().next() else {
                    return;
                };
                self.open_tab_ai_agent_chat_with_context_part(part, source, cx);
            }
            AgentChatContextPolicy::ActionsPayload { target } => {
                self.open_tab_ai_agent_chat_with_explicit_target(target, cx);
            }
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

    /// Behavior replacement for the former source-string audit that lived in
    /// `src/ai/agent_chat/ui/tests.rs`: Standard launcher entry may inherit the
    /// selected launcher row, every nonstandard variant must suppress it. Kept
    /// under the same test name so existing CI filters continue to work.
    #[test]
    fn agent_chat_ui_variant_launch_suppresses_selected_launcher_row_context_contract() {
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
        let standard = AgentChatEntryRequest::main_launcher_with_variant(
            None,
            AgentChatUiVariant::Standard,
        );
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
