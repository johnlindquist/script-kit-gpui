//! Resolved Agent Chat layout model (WP6).
//!
//! `AgentChatView::render` historically duplicated the shell / composer /
//! queue / callout / footer decisions across a "composer in the header" branch
//! and a "composer docked at the bottom" branch, and `automation_layout_info`
//! derived composer placement from the host *window kind* rather than from the
//! same decision the renderer used. That let the measured layout and the
//! painted layout disagree, and it meant every transient lane and the footer
//! owner were decided twice.
//!
//! This module resolves the whole layout ONCE from the presentation
//! [`AgentChatUiVariant`] into a [`ResolvedAgentChatLayout`], and resolves the
//! single footer owner ONCE into an [`AgentChatFooterPresentation`]. Both the
//! renderer and the automation-layout reporter consume the same resolved
//! model, so "render once" is not a comment — it is the only data path.
//!
//! Everything here is pure (no `Context`, no `Window`), so the full
//! variant × footer matrix is covered by ordinary unit tests instead of a
//! source audit.

use super::ui_variant::{AgentChatChromeDensity, AgentChatComposerPlacement, AgentChatUiVariant};

/// Where the composer input is placed in the resolved shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatComposerSlot {
    /// Docked in the shared main-view header, above the transcript.
    Header,
    /// Docked at the bottom of the shell, below the transcript.
    Bottom,
}

/// Where the transcript's content anchors when it does not fill the viewport.
///
/// This is correlated with the composer slot: a header composer reads
/// top-down (short transcripts sit just under the composer → `Top`); a
/// bottom-docked composer reads bottom-up (short transcripts sit just above
/// the composer → `Bottom`). Auto-follow of the streaming tail is orthogonal
/// and stays enabled in both anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatTranscriptAnchor {
    Top,
    Bottom,
}

/// The layout of an Agent Chat surface, resolved ONCE from the presentation
/// variant. The renderer and `automation_layout_info` both consume this so the
/// painted and measured layouts can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedAgentChatLayout {
    pub(crate) composer_slot: AgentChatComposerSlot,
    pub(crate) transcript_anchor: AgentChatTranscriptAnchor,
    pub(crate) density: AgentChatChromeDensity,
    pub(crate) show_sidecar: bool,
    pub(crate) show_variant_badge: bool,
}

impl ResolvedAgentChatLayout {
    /// Resolve the layout from the presentation variant. Total over every
    /// variant, including `FocusedTextMini` (which renders through its own
    /// early-return path but still resolves to a well-defined model so the
    /// function is exhaustive and testable).
    pub(crate) fn resolve(variant: AgentChatUiVariant) -> Self {
        let config = variant.config();
        let composer_slot = match config.composer {
            AgentChatComposerPlacement::Default => AgentChatComposerSlot::Header,
            AgentChatComposerPlacement::BottomDock => AgentChatComposerSlot::Bottom,
            // FocusedTextMini retains its compact outer bottom slot because the window's
            // variation-card sizing contract is 44px-row based. Its nested instruction
            // and scope composers are canonical MainViewInput shells.
            AgentChatComposerPlacement::FocusedTextSingleLine => AgentChatComposerSlot::Bottom,
        };
        // Anchor follows the composer slot: header composer → top-anchored
        // transcript, bottom composer → bottom-anchored transcript. This makes
        // Standard/Quick AI top-anchored and BottomDock/DenseLog/Sidecar
        // bottom-anchored, matching the WP6 alignment contract.
        let transcript_anchor = match composer_slot {
            AgentChatComposerSlot::Header => AgentChatTranscriptAnchor::Top,
            AgentChatComposerSlot::Bottom => AgentChatTranscriptAnchor::Bottom,
        };
        Self {
            composer_slot,
            transcript_anchor,
            density: config.chrome,
            show_sidecar: config.show_sidecar,
            show_variant_badge: config.show_variant_badge,
        }
    }

    pub(crate) fn composer_in_header(self) -> bool {
        matches!(self.composer_slot, AgentChatComposerSlot::Header)
    }

    pub(crate) fn composer_at_bottom(self) -> bool {
        matches!(self.composer_slot, AgentChatComposerSlot::Bottom)
    }
}

/// Which body an Agent Chat surface renders. This is decided ONCE, at the top
/// of `AgentChatView::render`, and drives the single body dispatch — the
/// renderer never re-derives "am I in setup?" per branch. The three non-chat
/// bodies short-circuit the conversation shell; `Conversation` composes the
/// resolved shell / composer / footer from [`ResolvedAgentChatLayout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatBodyKind {
    /// Session-level setup card (`AgentChatSession::Setup`): no live thread.
    InitialSetup,
    /// A live thread raised `SetupRequired`; the setup card replaces the
    /// errored transcript.
    RuntimeSetup,
    /// The compact focused-text editing surface (its own early-return path).
    FocusedTextMini,
    /// The normal conversation shell (header- or bottom-docked composer).
    Conversation,
}

impl AgentChatBodyKind {
    /// The stable automation string repr — consumed by the layout probe so the
    /// projected body kind is asserted against the rendered one.
    pub(crate) fn automation_repr(self) -> &'static str {
        match self {
            Self::InitialSetup => "initial-setup",
            Self::RuntimeSetup => "runtime-setup",
            Self::FocusedTextMini => "focused-text-mini",
            Self::Conversation => "conversation",
        }
    }

    /// Whether this body renders the full conversation shell (transcript +
    /// composer + resolved footer). The three setup/mini bodies do not.
    pub(crate) fn renders_conversation_shell(self) -> bool {
        matches!(self, Self::Conversation)
    }
}

/// The whole render decision, resolved ONCE per frame from the presentation
/// variant and the four live-state facts. The renderer, `automation_layout_info`
/// and the footer owner all consume this single plan, so the painted layout,
/// the measured layout and the footer band can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedAgentChatRenderPlan {
    pub(crate) body: AgentChatBodyKind,
    pub(crate) layout: ResolvedAgentChatLayout,
    pub(crate) footer: AgentChatFooterPresentation,
}

impl ResolvedAgentChatRenderPlan {
    /// Resolve the whole render plan. `is_setup_mode` (session-level setup)
    /// wins over `runtime_setup_active` (a live thread that raised
    /// `SetupRequired`), which wins over `focused_text_active`; the default is
    /// the conversation shell. The body decision is independent of the shell
    /// layout, which is always resolved from the variant so the setup/mini
    /// bodies still carry a well-defined (unused) layout.
    pub(crate) fn resolve(
        variant: AgentChatUiVariant,
        is_setup_mode: bool,
        runtime_setup_active: bool,
        focused_text_active: bool,
        footer_inputs: AgentChatFooterInputs,
    ) -> Self {
        let body = if is_setup_mode {
            AgentChatBodyKind::InitialSetup
        } else if runtime_setup_active {
            AgentChatBodyKind::RuntimeSetup
        } else if focused_text_active {
            AgentChatBodyKind::FocusedTextMini
        } else {
            AgentChatBodyKind::Conversation
        };
        Self {
            body,
            layout: ResolvedAgentChatLayout::resolve(variant),
            footer: resolve_footer_presentation(footer_inputs),
        }
    }

    /// How many in-shell footer bands this plan reserves. A setup card or the
    /// compact focused-text body never reserves an inline/native footer band of
    /// its own (their footers, when present, are native and owned by the host
    /// window), so only the conversation shell reports the resolved footer's
    /// band count.
    pub(crate) fn reserved_footer_band_count(self) -> usize {
        if self.body.renders_conversation_shell() {
            self.footer.reserved_band_count()
        } else {
            0
        }
    }
}

/// The single footer band an Agent Chat surface reserves. Exactly one owner is
/// resolved per surface so the shell never reserves two footer bands (an
/// inline rail *and* a native spacer) or silently reserves none when a footer
/// is expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentChatFooterPresentation {
    /// An external host (portal / prompt shell) owns the footer; this surface
    /// reserves no local band.
    ExternalHost,
    /// The native footer popup owns the pixels; the shell reserves a native
    /// spacer band so the transcript does not paint under it.
    NativeSpacer,
    /// Agent Chat renders its own in-flow footer rail (the config/hint rail).
    InlineConfigRail,
}

impl AgentChatFooterPresentation {
    /// How many in-shell footer bands this presentation reserves. Always 0 or
    /// 1 — never two. `ExternalHost` reserves none (the band lives in the
    /// host); the other two reserve exactly one.
    pub(crate) fn reserved_band_count(self) -> usize {
        match self {
            Self::ExternalHost => 0,
            Self::NativeSpacer | Self::InlineConfigRail => 1,
        }
    }

    /// Whether this presentation renders an inline (in-flow GPUI) footer rail.
    pub(crate) fn renders_inline_rail(self) -> bool {
        matches!(self, Self::InlineConfigRail)
    }

    /// Whether this presentation drives the native footer popup / spacer.
    pub(crate) fn uses_native_spacer(self) -> bool {
        matches!(self, Self::NativeSpacer)
    }
}

/// Inputs to the single footer-owner decision. Every field is a plain fact the
/// caller already knows, so the decision is a pure function of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentChatFooterInputs {
    /// An external footer host (portal / prompt shell) owns the footer.
    pub(crate) uses_external_footer_host: bool,
    /// This surface is rendering inside the embedded main window.
    pub(crate) is_main_window: bool,
    /// A detached window can float a Tahoe liquid-glass in-window footer rail
    /// (macOS glass available AND vibrancy enabled). False elsewhere.
    pub(crate) glass_in_window_footer: bool,
    /// The platform reserves a native footer popup for detached windows
    /// (macOS). On other platforms detached windows render the inline rail.
    pub(crate) platform_native_detached_footer: bool,
    /// The active main-window native footer surface is Agent Chat, so the main
    /// window reserves a native spacer instead of an inline rail.
    pub(crate) main_active_surface_is_agent_chat: bool,
}

/// Resolve the single footer owner. This reproduces the exact branch structure
/// the renderer used to inline twice (once per composer branch) and once more
/// in `automation_layout_info`, now in one pure place.
pub(crate) fn resolve_footer_presentation(
    inputs: AgentChatFooterInputs,
) -> AgentChatFooterPresentation {
    if inputs.uses_external_footer_host {
        return AgentChatFooterPresentation::ExternalHost;
    }
    if !inputs.is_main_window {
        // Detached window.
        if inputs.glass_in_window_footer {
            return AgentChatFooterPresentation::InlineConfigRail;
        }
        if inputs.platform_native_detached_footer {
            return AgentChatFooterPresentation::NativeSpacer;
        }
        return AgentChatFooterPresentation::InlineConfigRail;
    }
    // Embedded main window.
    if inputs.main_active_surface_is_agent_chat {
        AgentChatFooterPresentation::NativeSpacer
    } else {
        AgentChatFooterPresentation::InlineConfigRail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Every variant resolves to exactly ONE composer slot and ONE transcript
    /// anchor, and the two are correlated (header ⇒ top, bottom ⇒ bottom).
    /// Because the model is an enum, "one shell / one composer" is a type-level
    /// guarantee; this pins the actual mapping per variant.
    #[test]
    fn agent_chat_variants_render_one_shell_and_one_composer() {
        for variant in ALL_VARIANTS {
            let resolved = ResolvedAgentChatLayout::resolve(variant);

            // Exactly one composer slot: `composer_in_header` and
            // `composer_at_bottom` are mutually exclusive and exhaustive.
            assert_ne!(
                resolved.composer_in_header(),
                resolved.composer_at_bottom(),
                "{variant:?}: composer must be in exactly one slot",
            );

            // Anchor is derived from the slot, never independently.
            let expected_anchor = match resolved.composer_slot {
                AgentChatComposerSlot::Header => AgentChatTranscriptAnchor::Top,
                AgentChatComposerSlot::Bottom => AgentChatTranscriptAnchor::Bottom,
            };
            assert_eq!(
                resolved.transcript_anchor, expected_anchor,
                "{variant:?}: transcript anchor must follow the composer slot",
            );
        }

        // Spec alignment contract: header composers → top-anchored transcript.
        for variant in [
            AgentChatUiVariant::Standard,
            AgentChatUiVariant::QuickAi,
            AgentChatUiVariant::UserBold,
            AgentChatUiVariant::RoleSplit,
        ] {
            let resolved = ResolvedAgentChatLayout::resolve(variant);
            assert_eq!(resolved.composer_slot, AgentChatComposerSlot::Header);
            assert_eq!(resolved.transcript_anchor, AgentChatTranscriptAnchor::Top);
        }

        // Bottom-docked composers → bottom-anchored transcript.
        for variant in [
            AgentChatUiVariant::BottomDock,
            AgentChatUiVariant::DenseLog,
            AgentChatUiVariant::Sidecar,
            AgentChatUiVariant::FocusedTextMini,
        ] {
            let resolved = ResolvedAgentChatLayout::resolve(variant);
            assert_eq!(resolved.composer_slot, AgentChatComposerSlot::Bottom);
            assert_eq!(
                resolved.transcript_anchor,
                AgentChatTranscriptAnchor::Bottom
            );
        }
    }

    /// Over the entire footer-input matrix, exactly one owner is resolved and
    /// it never reserves more than one band. The `NativeSpacer` and
    /// `InlineConfigRail` owners are mutually exclusive — the shell can never
    /// drive the native popup AND paint an inline rail at once.
    #[test]
    fn footer_presentation_matrix_has_one_owner() {
        let bools = [false, true];
        for &uses_external_footer_host in &bools {
            for &is_main_window in &bools {
                for &glass_in_window_footer in &bools {
                    for &platform_native_detached_footer in &bools {
                        for &main_active_surface_is_agent_chat in &bools {
                            let inputs = AgentChatFooterInputs {
                                uses_external_footer_host,
                                is_main_window,
                                glass_in_window_footer,
                                platform_native_detached_footer,
                                main_active_surface_is_agent_chat,
                            };
                            let presentation = resolve_footer_presentation(inputs);

                            // Native spacer and inline rail are mutually
                            // exclusive: never both owners at once.
                            assert!(
                                !(presentation.uses_native_spacer()
                                    && presentation.renders_inline_rail()),
                                "two footer owners resolved for {inputs:?}",
                            );

                            // An external host always defers; nothing else does.
                            assert_eq!(
                                presentation == AgentChatFooterPresentation::ExternalHost,
                                uses_external_footer_host,
                                "external host ownership must match its input for {inputs:?}",
                            );
                        }
                    }
                }
            }
        }
    }

    /// Every resolved footer presentation reserves 0 or 1 in-shell bands —
    /// never two. When a footer is expected (no external host) it reserves
    /// exactly one band.
    #[test]
    fn agent_chat_footer_has_exactly_one_reserved_band() {
        let bools = [false, true];
        for &is_main_window in &bools {
            for &glass_in_window_footer in &bools {
                for &platform_native_detached_footer in &bools {
                    for &main_active_surface_is_agent_chat in &bools {
                        // External host → zero local bands.
                        let external = resolve_footer_presentation(AgentChatFooterInputs {
                            uses_external_footer_host: true,
                            is_main_window,
                            glass_in_window_footer,
                            platform_native_detached_footer,
                            main_active_surface_is_agent_chat,
                        });
                        assert_eq!(external.reserved_band_count(), 0);

                        // Locally-owned footer → exactly one band.
                        let local = resolve_footer_presentation(AgentChatFooterInputs {
                            uses_external_footer_host: false,
                            is_main_window,
                            glass_in_window_footer,
                            platform_native_detached_footer,
                            main_active_surface_is_agent_chat,
                        });
                        assert_eq!(
                            local.reserved_band_count(),
                            1,
                            "a locally-owned footer must reserve exactly one band",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn footer_matrix_reproduces_legacy_branch_outcomes() {
        // Detached macOS, glass on → inline glass rail.
        assert_eq!(
            resolve_footer_presentation(AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: false,
                glass_in_window_footer: true,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: false,
            }),
            AgentChatFooterPresentation::InlineConfigRail,
        );
        // Detached macOS, glass off → native spacer.
        assert_eq!(
            resolve_footer_presentation(AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: false,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: false,
            }),
            AgentChatFooterPresentation::NativeSpacer,
        );
        // Detached non-macOS → inline rail.
        assert_eq!(
            resolve_footer_presentation(AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: false,
                glass_in_window_footer: false,
                platform_native_detached_footer: false,
                main_active_surface_is_agent_chat: false,
            }),
            AgentChatFooterPresentation::InlineConfigRail,
        );
        // Main window, Agent Chat owns native surface → native spacer.
        assert_eq!(
            resolve_footer_presentation(AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: true,
            }),
            AgentChatFooterPresentation::NativeSpacer,
        );
        // Main window, Agent Chat not the native surface → inline rail.
        assert_eq!(
            resolve_footer_presentation(AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: false,
            }),
            AgentChatFooterPresentation::InlineConfigRail,
        );
    }

    const ALL_BODY_KINDS: [AgentChatBodyKind; 4] = [
        AgentChatBodyKind::InitialSetup,
        AgentChatBodyKind::RuntimeSetup,
        AgentChatBodyKind::FocusedTextMini,
        AgentChatBodyKind::Conversation,
    ];

    /// The four live-state facts that select the body kind, and the body kind
    /// each combination must resolve to. `is_setup_mode` wins over
    /// `runtime_setup_active`, which wins over `focused_text_active`.
    fn expected_body(
        is_setup_mode: bool,
        runtime_setup_active: bool,
        focused_text_active: bool,
    ) -> AgentChatBodyKind {
        if is_setup_mode {
            AgentChatBodyKind::InitialSetup
        } else if runtime_setup_active {
            AgentChatBodyKind::RuntimeSetup
        } else if focused_text_active {
            AgentChatBodyKind::FocusedTextMini
        } else {
            AgentChatBodyKind::Conversation
        }
    }

    fn any_footer_inputs() -> AgentChatFooterInputs {
        AgentChatFooterInputs {
            uses_external_footer_host: false,
            is_main_window: true,
            glass_in_window_footer: false,
            platform_native_detached_footer: true,
            main_active_surface_is_agent_chat: true,
        }
    }

    /// Over every variant × every combination of the four body-state facts, the
    /// plan resolves EXACTLY one body kind with the documented precedence, and
    /// the shell layout is always the variant's resolved layout regardless of
    /// which body renders.
    #[test]
    fn agent_chat_resolved_render_plan_selects_one_body_per_state() {
        let bools = [false, true];
        for variant in ALL_VARIANTS {
            let variant_layout = ResolvedAgentChatLayout::resolve(variant);
            for &is_setup_mode in &bools {
                for &runtime_setup_active in &bools {
                    for &focused_text_active in &bools {
                        let plan = ResolvedAgentChatRenderPlan::resolve(
                            variant,
                            is_setup_mode,
                            runtime_setup_active,
                            focused_text_active,
                            any_footer_inputs(),
                        );
                        assert_eq!(
                            plan.body,
                            expected_body(is_setup_mode, runtime_setup_active, focused_text_active),
                            "{variant:?} @ setup={is_setup_mode} runtime={runtime_setup_active} \
                             focused={focused_text_active}: body precedence",
                        );
                        // The shell layout never changes with the body state —
                        // it is a pure function of the variant.
                        assert_eq!(plan.layout, variant_layout, "{variant:?}: layout stable");
                    }
                }
            }
        }
    }

    /// Every variant × body-kind combination resolves to exactly ONE composer
    /// slot, ONE transcript anchor, and ONE density. Because the layout is an
    /// enum triple, "one of each" is a type-level guarantee; this pins that the
    /// plan surfaces exactly the variant's resolved layout for all four bodies.
    #[test]
    fn agent_chat_resolved_render_plan_has_one_slot_anchor_density_per_combo() {
        for variant in ALL_VARIANTS {
            let layout = ResolvedAgentChatLayout::resolve(variant);
            for body in ALL_BODY_KINDS {
                // Drive the plan to the specific body via the state facts.
                let plan = match body {
                    AgentChatBodyKind::InitialSetup => ResolvedAgentChatRenderPlan::resolve(
                        variant,
                        true,
                        false,
                        false,
                        any_footer_inputs(),
                    ),
                    AgentChatBodyKind::RuntimeSetup => ResolvedAgentChatRenderPlan::resolve(
                        variant,
                        false,
                        true,
                        false,
                        any_footer_inputs(),
                    ),
                    AgentChatBodyKind::FocusedTextMini => ResolvedAgentChatRenderPlan::resolve(
                        variant,
                        false,
                        false,
                        true,
                        any_footer_inputs(),
                    ),
                    AgentChatBodyKind::Conversation => ResolvedAgentChatRenderPlan::resolve(
                        variant,
                        false,
                        false,
                        false,
                        any_footer_inputs(),
                    ),
                };
                assert_eq!(plan.body, body, "{variant:?}: body drive");

                // Exactly one composer slot.
                assert_ne!(
                    plan.layout.composer_in_header(),
                    plan.layout.composer_at_bottom(),
                    "{variant:?}/{body:?}: exactly one composer slot",
                );
                // Anchor follows the slot (one anchor).
                let expected_anchor = match plan.layout.composer_slot {
                    AgentChatComposerSlot::Header => AgentChatTranscriptAnchor::Top,
                    AgentChatComposerSlot::Bottom => AgentChatTranscriptAnchor::Bottom,
                };
                assert_eq!(
                    plan.layout.transcript_anchor, expected_anchor,
                    "{variant:?}/{body:?}: one transcript anchor",
                );
                // Exactly one density, and it matches the variant's config.
                assert_eq!(
                    plan.layout.density, layout.density,
                    "{variant:?}/{body:?}: one density",
                );
            }
        }
    }

    /// Only the conversation shell reserves an in-shell footer band; the setup
    /// and focused-text bodies reserve zero (their footers are native and owned
    /// by the host window). The conversation shell reserves exactly the
    /// resolved footer presentation's band count.
    #[test]
    fn agent_chat_resolved_render_plan_reserves_footer_band_only_for_conversation() {
        for variant in ALL_VARIANTS {
            // Conversation body, locally-owned footer → exactly one band.
            let conversation = ResolvedAgentChatRenderPlan::resolve(
                variant,
                false,
                false,
                false,
                AgentChatFooterInputs {
                    uses_external_footer_host: false,
                    is_main_window: true,
                    glass_in_window_footer: false,
                    platform_native_detached_footer: true,
                    main_active_surface_is_agent_chat: true,
                },
            );
            assert_eq!(conversation.reserved_footer_band_count(), 1, "{variant:?}");

            // Conversation body, external host → zero bands.
            let external = ResolvedAgentChatRenderPlan::resolve(
                variant,
                false,
                false,
                false,
                AgentChatFooterInputs {
                    uses_external_footer_host: true,
                    is_main_window: false,
                    glass_in_window_footer: false,
                    platform_native_detached_footer: true,
                    main_active_surface_is_agent_chat: false,
                },
            );
            assert_eq!(external.reserved_footer_band_count(), 0, "{variant:?}");

            // Setup / runtime-setup / focused-text bodies reserve zero even
            // when the resolved footer presentation would reserve one.
            for (setup, runtime, focused) in [
                (true, false, false),
                (false, true, false),
                (false, false, true),
            ] {
                let non_shell = ResolvedAgentChatRenderPlan::resolve(
                    variant,
                    setup,
                    runtime,
                    focused,
                    AgentChatFooterInputs {
                        uses_external_footer_host: false,
                        is_main_window: true,
                        glass_in_window_footer: false,
                        platform_native_detached_footer: true,
                        main_active_surface_is_agent_chat: true,
                    },
                );
                assert!(!non_shell.body.renders_conversation_shell());
                assert_eq!(
                    non_shell.reserved_footer_band_count(),
                    0,
                    "{variant:?}: non-shell body reserves no band",
                );
            }
        }
    }
}
