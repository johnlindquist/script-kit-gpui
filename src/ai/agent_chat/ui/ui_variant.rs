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
            },
            Self::UserBold => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::UserBold,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: false,
                show_variant_badge: true,
            },
            Self::RoleSplit => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::RoleSplit,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: false,
                show_variant_badge: true,
            },
            Self::BottomDock => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::Standard,
                composer: AgentChatComposerPlacement::BottomDock,
                chrome: AgentChatChromeDensity::Compact,
                show_sidecar: false,
                show_variant_badge: true,
            },
            Self::DenseLog => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::DenseLog,
                composer: AgentChatComposerPlacement::BottomDock,
                chrome: AgentChatChromeDensity::Compact,
                show_sidecar: false,
                show_variant_badge: true,
            },
            Self::Sidecar => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::RoleSplit,
                composer: AgentChatComposerPlacement::BottomDock,
                chrome: AgentChatChromeDensity::Default,
                show_sidecar: true,
                show_variant_badge: true,
            },
            Self::FocusedTextMini => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::FocusedTextPreview,
                composer: AgentChatComposerPlacement::FocusedTextSingleLine,
                chrome: AgentChatChromeDensity::Mini,
                show_sidecar: false,
                show_variant_badge: false,
            },
            Self::QuickAi => AgentChatUiConfig {
                transcript: AgentChatTranscriptPresentation::Standard,
                composer: AgentChatComposerPlacement::Default,
                chrome: AgentChatChromeDensity::Compact,
                show_sidecar: false,
                show_variant_badge: false,
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
