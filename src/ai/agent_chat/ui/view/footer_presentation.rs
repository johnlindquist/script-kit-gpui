//! Pure footer ownership, native lifecycle, and automation projection planning.

use super::AgentChatAutomationProjection;

pub(super) fn combined_agent_model_header_label(profile: &str, model: &str) -> String {
    let profile = profile.trim();
    let model = model.trim();
    match (profile.is_empty(), model.is_empty(), profile == model) {
        (false, false, true) => profile.to_string(),
        (false, false, false) => format!("{profile} · {model}"),
        (false, true, _) => profile.to_string(),
        (true, false, _) => model.to_string(),
        (true, true, _) => String::new(),
    }
}

/// The single footer owner an Agent Chat surface reconciles to per frame (C-R5).
///
/// This is the imperative counterpart to
/// [`crate::ai::agent_chat::ui::layout::AgentChatFooterPresentation`]: the
/// presentation says WHAT band is reserved; the owner says WHO drives the
/// native host and owns the transition side-effects (install / clear). Routing
/// every footer branch (normal, setup, runtime-setup, FocusedTextMini,
/// bottom-dock) through the one reconcile step guarantees a single owner is
/// live at a time — a detached window can never leave an orphan native footer
/// host behind after switching to an inline rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AgentChatFooterOwner {
    /// An external host (portal / prompt shell) owns the footer.
    External,
    /// The native footer popup owns the pixels; the shell reserves a spacer.
    Native,
    /// Agent Chat renders its own in-flow config rail.
    Inline,
}

impl AgentChatFooterOwner {
    pub(super) fn from_presentation(
        presentation: crate::ai::agent_chat::ui::layout::AgentChatFooterPresentation,
    ) -> Self {
        use crate::ai::agent_chat::ui::layout::AgentChatFooterPresentation;
        match presentation {
            AgentChatFooterPresentation::ExternalHost => Self::External,
            AgentChatFooterPresentation::NativeSpacer => Self::Native,
            AgentChatFooterPresentation::InlineConfigRail => Self::Inline,
        }
    }

    /// The automation string repr, consumed by the layout probe.
    pub(super) fn automation_repr(self) -> &'static str {
        match self {
            Self::External => "external",
            Self::Native => "native",
            Self::Inline => "inline",
        }
    }

    /// How many in-shell footer bands this owner reserves — 0 for External, 1
    /// for Native/Inline. Mirrors `AgentChatFooterPresentation::reserved_band_count`.
    pub(super) fn reserved_band_count(self) -> usize {
        match self {
            Self::External => 0,
            Self::Native | Self::Inline => 1,
        }
    }
}

/// The pure outcome of transitioning the footer owner from `previous` to
/// `desired`. Kept side-effect-free so the whole transition matrix is covered
/// by unit tests: exactly one owner survives, the native host is explicitly
/// cleared on any Native→non-Native move, and the reserved band count is 0 only
/// for External.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AgentChatFooterOwnerTransition {
    pub(super) owner: AgentChatFooterOwner,
    /// The native footer host must be explicitly torn down this frame.
    pub(super) clears_native_host: bool,
    pub(super) reserved_bands: usize,
}

/// The memoized native-footer presentation state (BC-2, Oracle seat 3). Captures
/// everything a footer lifecycle side-effect depends on: the resolved owner, the
/// host-window class (native side-effects apply only to detached windows), and
/// the synced native config (`Some` only while a detached window owns the native
/// footer). `transition_footer_owner` compares the next state against this so it
/// installs / tears down / re-syncs the native host ONLY on an actual change,
/// instead of re-driving those side-effects every render frame.
#[derive(Clone, PartialEq)]
pub(super) struct AgentChatFooterPresentationState {
    pub(super) owner: AgentChatFooterOwner,
    pub(super) is_main_window: bool,
    pub(super) native_config: Option<crate::footer_popup::MainWindowFooterConfig>,
    pub(super) theme_revision: u64,
}

/// The pure native-footer lifecycle decision for a presentation transition
/// (BC-2, Oracle seat 3). Kept side-effect-free so the memoization matrix is
/// unit-tested: an unchanged presentation does nothing; leaving a detached
/// native footer tears the previous host down; entering (or re-configuring) a
/// detached native footer syncs it. Only DETACHED windows carry native
/// side-effects — the embedded main window's native footer is owned elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NativeFooterLifecycle {
    /// The presentation is byte-for-byte identical to the last applied one — no
    /// side-effects run this frame (this is what stops the per-frame re-sync).
    pub(super) unchanged: bool,
    /// Tear down the detached native footer host installed by the previous
    /// presentation (and drop its action listener).
    pub(super) tear_down_previous_native: bool,
    /// Ensure the action listener and sync the native popup for the next
    /// presentation.
    pub(super) sync_next_native: bool,
}

pub(super) fn plan_native_footer_lifecycle(
    previous: Option<&AgentChatFooterPresentationState>,
    next: &AgentChatFooterPresentationState,
) -> NativeFooterLifecycle {
    if previous == Some(next) {
        return NativeFooterLifecycle {
            unchanged: true,
            tear_down_previous_native: false,
            sync_next_native: false,
        };
    }
    let previous_installed_detached_native = previous
        .is_some_and(|prev| prev.owner == AgentChatFooterOwner::Native && !prev.is_main_window);
    let next_owns_detached_native =
        next.owner == AgentChatFooterOwner::Native && !next.is_main_window;
    NativeFooterLifecycle {
        unchanged: false,
        tear_down_previous_native: previous_installed_detached_native && !next_owns_detached_native,
        sync_next_native: next_owns_detached_native,
    }
}

pub(super) fn plan_footer_owner_transition(
    previous: Option<AgentChatFooterOwner>,
    desired: AgentChatFooterOwner,
) -> AgentChatFooterOwnerTransition {
    // Leaving Native for any non-native owner requires an explicit host
    // teardown; entering or staying Native re-syncs the host instead.
    let clears_native_host =
        previous == Some(AgentChatFooterOwner::Native) && desired != AgentChatFooterOwner::Native;
    AgentChatFooterOwnerTransition {
        owner: desired,
        clears_native_host,
        reserved_bands: desired.reserved_band_count(),
    }
}

/// The footer owner a render plan reconciles to. The conversation shell maps
/// its resolved footer presentation to an owner; every other body (setup,
/// runtime-setup, focused-text mini) reserves no in-shell band and reconciles
/// to `External`, which tears down any orphan native footer host on a detached
/// window while leaving the host window's own native footer surface untouched.
pub(super) fn desired_footer_owner_for_plan(
    plan: crate::ai::agent_chat::ui::layout::ResolvedAgentChatRenderPlan,
) -> AgentChatFooterOwner {
    if plan.body.renders_conversation_shell() {
        AgentChatFooterOwner::from_presentation(plan.footer)
    } else {
        AgentChatFooterOwner::External
    }
}

impl AgentChatAutomationProjection {
    pub(super) fn from_plan(
        plan: crate::ai::agent_chat::ui::layout::ResolvedAgentChatRenderPlan,
    ) -> Self {
        use crate::ai::agent_chat::ui::layout::{AgentChatComposerSlot, AgentChatTranscriptAnchor};
        use crate::ai::agent_chat::ui::ui_variant::AgentChatChromeDensity;
        let owner = desired_footer_owner_for_plan(plan);
        Self {
            body_kind: plan.body.automation_repr(),
            composer_slot: match plan.layout.composer_slot {
                AgentChatComposerSlot::Header => "header",
                AgentChatComposerSlot::Bottom => "bottom",
            },
            transcript_anchor: match plan.layout.transcript_anchor {
                AgentChatTranscriptAnchor::Top => "top",
                AgentChatTranscriptAnchor::Bottom => "bottom",
            },
            density: match plan.layout.density {
                AgentChatChromeDensity::Default => "default",
                AgentChatChromeDensity::Compact => "compact",
                AgentChatChromeDensity::Mini => "mini",
            },
            footer_owner: owner.automation_repr(),
            reserved_footer_bands: plan.reserved_footer_band_count(),
            show_sidecar: plan.layout.show_sidecar,
            show_variant_badge: plan.layout.show_variant_badge,
        }
    }
}
