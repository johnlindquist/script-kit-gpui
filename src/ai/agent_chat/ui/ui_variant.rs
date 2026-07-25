#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AgentChatUiVariant {
    #[default]
    Standard,
    UserBold,
    RoleSplit,
    BottomDock,
    DenseLog,
    Sidecar,
    FocusedTextMini,
    /// Zero-context instant answers: launcher Tab-with-text. Pinned to the
    /// Quick AI profile (spark model, web_search only, no skills/context). Not listed in
    /// EXPERIMENTS — it is a launch mode, not a pickable chat design.
    QuickAi,
}

impl AgentChatUiVariant {
    pub(crate) const EXPERIMENTS: [Self; 6] = [
        Self::UserBold,
        Self::RoleSplit,
        Self::BottomDock,
        Self::DenseLog,
        Self::Sidecar,
        Self::FocusedTextMini,
    ];

    pub(crate) fn state_id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::UserBold => "user-bold",
            Self::RoleSplit => "role-split",
            Self::BottomDock => "bottom-dock",
            Self::DenseLog => "dense-log",
            Self::Sidecar => "sidecar",
            Self::FocusedTextMini => "focused-text-mini",
            Self::QuickAi => "quick-ai",
        }
    }

    pub(crate) fn menu_id(self) -> &'static str {
        match self {
            Self::Standard => "builtin/ai-chat",
            Self::UserBold => "builtin/ai-chat/user-bold",
            Self::RoleSplit => "builtin/ai-chat/role-split",
            Self::BottomDock => "builtin/ai-chat/bottom-dock",
            Self::DenseLog => "builtin/ai-chat/dense-log",
            Self::Sidecar => "builtin/ai-chat/sidecar",
            Self::FocusedTextMini => "builtin/ai-chat/focused-text-mini",
            Self::QuickAi => "builtin/ai-chat/quick-ai",
        }
    }

    pub(crate) fn menu_name(self) -> &'static str {
        match self {
            Self::Standard => "Agent Chat",
            Self::UserBold => "Agent Chat: Bold User Messages",
            Self::RoleSplit => "Agent Chat: Split Roles",
            Self::BottomDock => "Agent Chat: Bottom Composer",
            Self::DenseLog => "Agent Chat: Compact Transcript",
            Self::Sidecar => "Agent Chat: State Sidecar",
            Self::FocusedTextMini => "Agent Chat: Focused Text Editor",
            Self::QuickAi => "Quick AI",
        }
    }

    pub(crate) fn menu_icon(self) -> &'static str {
        match self {
            Self::Standard => "bot",
            Self::UserBold => "text-cursor-input",
            Self::RoleSplit => "layout-grid",
            Self::BottomDock => "monitor-down",
            Self::DenseLog => "scroll-text",
            Self::Sidecar => "app-window",
            Self::FocusedTextMini => "text-cursor-input",
            Self::QuickAi => "bot",
        }
    }

    pub(crate) fn menu_description(self) -> &'static str {
        match self {
            Self::Standard => "Open Agent Chat with fresh context",
            Self::UserBold => "Open Agent Chat with emphasized user messages",
            Self::RoleSplit => "Open Agent Chat with assistant left and user right",
            Self::BottomDock => "Open Agent Chat with the input docked at the bottom",
            Self::DenseLog => "Open Agent Chat in a compact transcript layout",
            Self::Sidecar => "Open Agent Chat with a live state sidecar",
            Self::FocusedTextMini => "Open Agent Chat as a compact focused-text editing surface",
            Self::QuickAi => {
                "Ask the zero-context Quick AI (web search only — no files, skills, or memories)"
            }
        }
    }

    pub(crate) fn footer_label(self) -> &'static str {
        match self {
            Self::Standard => "Agent",
            Self::UserBold => "Bold",
            Self::RoleSplit => "Split",
            Self::BottomDock => "Bottom",
            Self::DenseLog => "Log",
            Self::Sidecar => "Sidecar",
            Self::FocusedTextMini => "Text",
            Self::QuickAi => "Quick AI",
        }
    }

    pub(crate) fn keywords(self) -> Vec<&'static str> {
        let mut keywords = vec![
            "ai",
            "agent",
            "chat",
            "assistant",
            "agent_chat",
            "ui",
            "variant",
            "design",
        ];
        match self {
            Self::Standard => keywords.extend(["harness", "gpt", "llm", "tab"]),
            Self::UserBold => keywords.extend(["bold", "user", "message", "emphasis"]),
            Self::RoleSplit => keywords.extend(["left", "right", "assistant", "user", "bubbles"]),
            Self::BottomDock => keywords.extend(["bottom", "input", "composer", "dock"]),
            Self::DenseLog => keywords.extend(["dense", "compact", "log", "transcript"]),
            Self::Sidecar => keywords.extend(["sidecar", "rail", "state", "status", "metadata"]),
            Self::FocusedTextMini => {
                keywords.extend(["text", "focused", "inline", "edit", "replace", "append"])
            }
            Self::QuickAi => keywords.extend(["quick", "fast", "instant", "spark", "tab"]),
        }
        keywords
    }

    pub(crate) fn config(self) -> AgentChatUiConfig {
        match self {
            Self::Standard => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::Standard,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: false,
                show_variant_badge: false,
                show_turn_copy: true,
                show_jump_to_latest: true,
            },
            Self::UserBold => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::UserBold,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: false,
                show_variant_badge: true,
                show_turn_copy: true,
                show_jump_to_latest: true,
            },
            Self::RoleSplit => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::RoleSplit,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: false,
                show_variant_badge: true,
                show_turn_copy: true,
                show_jump_to_latest: true,
            },
            Self::BottomDock => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::Standard,
                composer: AgentChatComposerPlacement::BottomDock,
                chrome: AgentChatChromeDensity::Compact,
                show_sidecar: false,
                show_variant_badge: true,
                show_turn_copy: true,
                show_jump_to_latest: true,
            },
            Self::DenseLog => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::DenseLog,
                composer: AgentChatComposerPlacement::BottomDock,
                chrome: AgentChatChromeDensity::Compact,
                show_sidecar: false,
                show_variant_badge: true,
                show_turn_copy: true,
                show_jump_to_latest: true,
            },
            Self::Sidecar => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::RoleSplit,
                composer: AgentChatComposerPlacement::BottomDock,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: true,
                show_variant_badge: true,
                show_turn_copy: true,
                show_jump_to_latest: true,
            },
            Self::FocusedTextMini => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::FocusedTextPreview,
                composer: AgentChatComposerPlacement::FocusedTextSingleLine,
                chrome: AgentChatChromeDensity::Mini,
                show_sidecar: false,
                show_variant_badge: false,
                show_turn_copy: false,
                show_jump_to_latest: false,
            },
            Self::QuickAi => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::Standard,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Compact,
                show_sidecar: false,
                show_variant_badge: false,
                show_turn_copy: false,
                show_jump_to_latest: false,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentChatUiConfig {
    pub(crate) transcript: AgentChatTranscriptPresentation,
    pub(crate) composer: AgentChatComposerPlacement,
    pub(crate) chrome: AgentChatChromeDensity,
    pub(crate) show_sidecar: bool,
    pub(crate) show_variant_badge: bool,
    /// Per-turn response copy control on assistant rows.
    ///
    /// Gated here rather than by `if variant == QuickAi` checks inside the
    /// renderer so the answer for a NEW variant is a compile-time decision in
    /// one table instead of a grep across render functions.
    pub(crate) show_turn_copy: bool,
    /// "Jump to latest" affordance when the transcript is scrolled off tail.
    pub(crate) show_jump_to_latest: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatTranscriptPresentation {
    Standard,
    UserBold,
    RoleSplit,
    DenseLog,
    FocusedTextPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatComposerPlacement {
    Default,
    BottomDock,
    FocusedTextSingleLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatChromeDensity {
    Default,
    Compact,
    Mini,
}

impl AgentChatChromeDensity {
    /// In-flow content gap for the conversation middle area (transcript column
    /// and the transcript↔sidecar row), resolved to an EXISTING chrome spacing
    /// token — never a new one-off value. `Default` keeps the roomy panel
    /// rhythm; `Compact`/`Mini` tighten to the dense gap so the log-style and
    /// compact variants pack more rows. Because the default header-composer
    /// variants render the middle column with a single child, this gap is
    /// visually inert for them (no CLS) and only affects the multi-child
    /// sidecar row and compact bottom-dock variants.
    pub(crate) fn content_gap_px(self) -> f32 {
        use crate::ui::chrome as chrome_tokens;
        match self {
            Self::Default => chrome_tokens::LIQUID_GLASS_PANEL_PADDING_PX,
            Self::Compact | Self::Mini => chrome_tokens::LIQUID_GLASS_DENSE_GAP_PX,
        }
    }
}

#[cfg(test)]
mod agent_chat_ui_variant_affordance_tests {
    use super::*;

    /// Every variant, so a newly added one cannot quietly default into or out
    /// of the full-Agent affordances. `EXPERIMENTS` is only six of the eight —
    /// it omits `Standard` and `QuickAi` — so it is the wrong list to iterate
    /// for a capability check.
    const ALL_VARIANTS: [AgentChatUiVariant; 8] = [
        AgentChatUiVariant::Standard,
        AgentChatUiVariant::UserBold,
        AgentChatUiVariant::RoleSplit,
        AgentChatUiVariant::BottomDock,
        AgentChatUiVariant::DenseLog,
        AgentChatUiVariant::Sidecar,
        AgentChatUiVariant::FocusedTextMini,
        AgentChatUiVariant::QuickAi,
    ];

    /// Quick AI is a zero-context instant-answer surface, deliberately out of
    /// scope for the per-turn affordances in this batch. Locked here, at the
    /// one config table, rather than by `if variant == QuickAi` scattered
    /// through render functions where a new call site could miss it.
    #[test]
    fn quick_ai_does_not_gain_full_agent_turn_affordances() {
        let config = AgentChatUiVariant::QuickAi.config();
        assert!(
            !config.show_turn_copy,
            "Quick AI must not gain the per-turn copy control in this batch"
        );
        assert!(
            !config.show_jump_to_latest,
            "Quick AI must not gain the jump-to-latest affordance in this batch"
        );
    }

    /// The focused-text mini surface is a compact editing affordance, not a
    /// scrollable conversation, so neither affordance applies.
    #[test]
    fn focused_text_mini_does_not_gain_full_agent_turn_affordances() {
        let config = AgentChatUiVariant::FocusedTextMini.config();
        assert!(!config.show_turn_copy);
        assert!(!config.show_jump_to_latest);
    }

    #[test]
    fn every_full_agent_chat_variant_has_both_affordances() {
        for variant in [
            AgentChatUiVariant::Standard,
            AgentChatUiVariant::UserBold,
            AgentChatUiVariant::RoleSplit,
            AgentChatUiVariant::BottomDock,
            AgentChatUiVariant::DenseLog,
            AgentChatUiVariant::Sidecar,
        ] {
            let config = variant.config();
            assert!(
                config.show_turn_copy,
                "{} must expose per-turn copy",
                variant.state_id()
            );
            assert!(
                config.show_jump_to_latest,
                "{} must expose jump-to-latest",
                variant.state_id()
            );
        }
    }

    /// Exactly six of eight variants carry the affordances. A count guards
    /// against a new variant being added to `ALL_VARIANTS` without a
    /// deliberate decision about these two flags.
    #[test]
    fn exactly_the_six_full_agent_variants_carry_the_affordances() {
        let with_copy = ALL_VARIANTS
            .iter()
            .filter(|v| v.config().show_turn_copy)
            .count();
        let with_jump = ALL_VARIANTS
            .iter()
            .filter(|v| v.config().show_jump_to_latest)
            .count();
        assert_eq!(
            with_copy, 6,
            "six full Agent Chat variants expose turn copy"
        );
        assert_eq!(
            with_jump, 6,
            "six full Agent Chat variants expose jump-to-latest"
        );
    }
}
