use super::super::AppView;

/// Single-source policy describing how an Agent Chat launch treats the source
/// surface's implicit focused row and any explicit context.
///
/// This is the one authority for context staging: it replaces the former
/// suppression bool + staging enum pair, which could encode contradictions
/// (a false suppression flag beside a suppressing staging). There is
/// deliberately NO `Default`; every request constructor must choose a policy
/// explicitly.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AgentChatContextPolicy {
    /// The source surface may contribute its focused row. If it has no focused
    /// row, the existing Ask Anything / ambient fallback may run.
    AmbientOrFocused,
    /// Do not inherit the source surface's implicit focused row.
    ///
    /// This does NOT suppress explicit context parts or an explicit
    /// FullScreen/FocusedWindow/etc. capture kind — it is not `NoContext`.
    SuppressFocused,
    /// Explicit host-provided context parts. The supported contract is exactly
    /// one part; the dispatcher fails closed on any other cardinality.
    Parts {
        parts: Vec<crate::ai::message_parts::AiContextPart>,
        source: &'static str,
    },
    /// Explicit actions-payload target (Cmd+Enter from an actions dialog etc.).
    ActionsPayload {
        target: crate::ai::TabAiTargetContext,
    },
}

impl AgentChatContextPolicy {
    /// Derive the launcher policy for a given Agent Chat UI variant.
    ///
    /// Standard launcher entry may inherit the selected launcher row; every
    /// other (nonstandard) variant is a menu preset / launch mode, not a
    /// context source, so it suppresses the implicit focused row. This match is
    /// exhaustive over all eight variants and contains no wildcard arm.
    pub(crate) fn for_main_launcher_variant(
        variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
    ) -> Self {
        use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
        match variant {
            AgentChatUiVariant::Standard => Self::AmbientOrFocused,
            AgentChatUiVariant::UserBold
            | AgentChatUiVariant::RoleSplit
            | AgentChatUiVariant::BottomDock
            | AgentChatUiVariant::DenseLog
            | AgentChatUiVariant::Sidecar
            | AgentChatUiVariant::FocusedTextMini
            | AgentChatUiVariant::QuickAi => Self::SuppressFocused,
        }
    }

    /// Whether this policy permits staging the source surface's implicit
    /// focused row. Only `AmbientOrFocused` does.
    pub(crate) fn admits_implicit_focused_part(&self) -> bool {
        match self {
            Self::AmbientOrFocused => true,
            Self::SuppressFocused | Self::Parts { .. } | Self::ActionsPayload { .. } => false,
        }
    }
}

/// Resolved Tab AI context payload ready for harness submission.
#[derive(Debug, Clone)]
pub(crate) struct TabAiResolvedContext {
    pub(crate) context: crate::ai::TabAiContextBlob,
    pub(crate) invocation_receipt: crate::ai::TabAiInvocationReceipt,
    pub(crate) suggested_intents: Vec<crate::ai::TabAiSuggestedIntentSpec>,
}

/// Pre-switch snapshot of the UI state captured at the Tab interception
/// boundary, before the view flips to `QuickTerminalView`.
///
/// The deferred capture pipeline uses this to assemble context in the
/// background while the harness terminal is already visible.
#[derive(Debug, Clone)]
pub(crate) struct TabAiLaunchRequest {
    /// The `AppView` that was active when Tab was pressed.
    pub(crate) source_view: AppView,
    /// Optional user intent (from Shift+Tab typed query).
    pub(crate) entry_intent: Option<String>,
    /// Whether the initial text stays in the composer or is submitted as the
    /// first turn after context bootstrap.
    pub(crate) seed_policy: super::agent_chat_entry::AgentChatSeedPolicy,
    /// Agent Chat presentation variant. Standard preserves the existing UI.
    pub(crate) ui_variant: crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant,
    /// Single-source context policy: whether this launch inherits the source
    /// surface's implicit focused row, suppresses it, or carries explicit parts.
    pub(crate) context_policy: AgentChatContextPolicy,
    /// Quick-submit plan from the deterministic planner (fallback / dictation).
    pub(crate) quick_submit_plan: Option<crate::ai::TabAiQuickSubmitPlan>,
    /// UI snapshot taken synchronously before the view switch.
    pub(crate) ui_snapshot: crate::ai::TabAiUiSnapshot,
    /// Invocation receipt for logging and downstream consumption.
    pub(crate) invocation_receipt: crate::ai::TabAiInvocationReceipt,
    /// What kind of capture to perform (focused window, full screen, etc.).
    pub(crate) capture_kind: crate::ai::TabAiCaptureKind,
    /// Monotonic generation counter, used to drop stale capture results.
    pub(crate) capture_generation: u64,
}

/// Artifacts produced by the deferred background capture task.
#[derive(Debug, Clone, Default)]
pub(crate) struct TabAiDeferredCaptureArtifacts {
    /// Desktop context snapshot (frontmost app, selected text, browser URL).
    pub(crate) desktop: crate::context_snapshot::AiContextSnapshot,
    /// Absolute path to the focused window screenshot file, if captured.
    pub(crate) screenshot_path: Option<String>,
}

/// Channel receiver for deferred capture results.
pub(crate) type TabAiDeferredCaptureRx =
    async_channel::Receiver<Result<TabAiDeferredCaptureArtifacts, String>>;

/// Maximum visible elements captured per UI snapshot for Tab AI context.
pub(crate) const TAB_AI_VISIBLE_ELEMENT_LIMIT: usize = 24;

/// Maximum visible targets resolved per surface for Tab AI context.
pub(crate) const TAB_AI_VISIBLE_TARGET_LIMIT: usize = 10;

/// Maximum clipboard history entries included in the Tab AI context blob.
pub(crate) const TAB_AI_CLIPBOARD_HISTORY_LIMIT: usize = 8;

/// Maximum character length for hydrated clipboard text entries.
pub(crate) const TAB_AI_CLIPBOARD_TEXT_LIMIT: usize = 1000;
