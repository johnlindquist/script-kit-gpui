//! Per-surface capability policy for Agent Chat variants.
//!
//! WP3 (2026-07-21 Oracle panel P0): Quick AI is a "zero-context, ask a quick
//! question" surface. Its clean-launch policy was enforced only at entry, so
//! the FULL Agent Chat affordances (history popup, local file/image
//! attachments, `@`/`>` context portals, the empty-Tab working-directory
//! picker, retained threads) stayed reachable for the lifetime of the view —
//! contradicting the zero-context label and leaking ambient context into a
//! surface that promised none.
//!
//! `AgentChatCapabilities` makes the allowed affordances an explicit,
//! lifetime-long contract resolved from the [`AgentChatUiVariant`]. Every
//! gate reads the same struct, so a Quick AI view cannot drift back into
//! full-surface behavior through any single un-guarded call site.

use super::ui_variant::AgentChatUiVariant;
use crate::ai::message_parts::AiContextPart;

/// Canonical backend identifier of the ONLY tool a web-search-only session is
/// permitted to run. Tool admission is matched against this id ONLY — never
/// against a human-readable display title, which the model (or a compromised
/// turn) can spoof. Mirrors `profiles::QUICK_AI_PI_TOOLS` and the Codex
/// `allowedTools` list so all three enforcement layers name the same tool.
pub(crate) const WEB_SEARCH_TOOL_ID: &str = "web_search";

/// The tool-admission policy for a live turn. Carried on every
/// `AgentChatTurnRequest` so the backend adapter and the event reducer share
/// one authority instead of each re-deriving "is this Quick AI".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatToolPolicy {
    /// Every tool the backend offers is allowed.
    Full,
    /// Only the canonical web-search tool ([`WEB_SEARCH_TOOL_ID`]) may run.
    WebSearchOnly,
}

impl AgentChatToolPolicy {
    /// Whether a tool with the given *canonical backend id* may run. Callers
    /// MUST pass the protocol tool id (e.g. `tool_name` from a tool-call
    /// event), not a rendered title.
    pub(crate) fn allows_tool(self, tool_id: &str) -> bool {
        match self {
            Self::Full => true,
            Self::WebSearchOnly => tool_id == WEB_SEARCH_TOOL_ID,
        }
    }

    /// Whether an interactive permission approval may ever be presented. A
    /// web-search-only session's single tool never requires approval, so any
    /// approval request under this policy is, by definition, for a tool the
    /// policy forbids — reject it before showing UI rather than trusting the
    /// request's display fields.
    pub(crate) const fn allows_permission_prompts(self) -> bool {
        matches!(self, Self::Full)
    }
}

/// The provenance class of a piece of would-be context, used as the single
/// axis the session policy adjudicates against.
///
/// A real [`AiContextPart`] is always context-bearing, so [`classify_context_part`]
/// never returns [`UserAuthoredText`](Self::UserAuthoredText): the user's typed
/// composer text is not a context part and reaches the model through the
/// user-text channel, which is always admitted. `UserAuthoredText` names that
/// channel so the adjudication table can state it explicitly. `Image` is
/// produced for screenshot-bearing parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatContextClass {
    /// Text the user themselves typed into the composer (the always-admitted
    /// user-text channel; never produced from an `AiContextPart`).
    UserAuthoredText,
    /// Desktop / Ask-Anything ambient capture.
    Ambient,
    /// A local file attachment.
    LocalFile,
    /// A slash-mode or menu-selected skill definition.
    Skill,
    /// Recalled conversation / note / stashed-text content.
    History,
    /// A screenshot or other image payload.
    Image,
    /// A resolved focused-application target (selection, script, clipboard).
    FocusedApplication,
    /// An MCP / protocol resource URI.
    ProtocolResource,
}

/// Why a context part was refused admission to a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextAdmissionError {
    /// The thread-owned session policy forbids this class of context.
    DeniedBySessionPolicy,
}

impl ContextAdmissionError {
    /// Stable, user-safe reason string. Never leaks the denied content.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DeniedBySessionPolicy => "denied_by_session_policy",
        }
    }
}

fn is_image_path(path: &str) -> bool {
    let lowered = path.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".heic", ".bmp", ".tiff",
    ]
    .iter()
    .any(|ext| lowered.ends_with(ext))
}

/// Classify a typed context part into its admission class. Always returns a
/// context-bearing class (never `UserAuthoredText`) — see
/// [`AgentChatContextClass`].
pub(crate) fn classify_context_part(part: &AiContextPart) -> AgentChatContextClass {
    match part {
        // Ambient chips (Ask Anything, promoted desktop capture) resolve to
        // the ambient desktop resource URI; every other resource URI is a
        // protocol/MCP resource.
        AiContextPart::ResourceUri { .. } if part.ambient_chip_label().is_some() => {
            AgentChatContextClass::Ambient
        }
        AiContextPart::ResourceUri { .. } => AgentChatContextClass::ProtocolResource,
        AiContextPart::FilePath { path, .. } if is_image_path(path) => AgentChatContextClass::Image,
        AiContextPart::FilePath { .. } => AgentChatContextClass::LocalFile,
        AiContextPart::SkillFile { .. } => AgentChatContextClass::Skill,
        AiContextPart::FocusedTarget { .. } if part.source().contains("screenshot=1") => {
            AgentChatContextClass::Image
        }
        AiContextPart::FocusedTarget { .. } => AgentChatContextClass::FocusedApplication,
        AiContextPart::AmbientContext { .. } => AgentChatContextClass::Ambient,
        AiContextPart::TextBlock { .. } => AgentChatContextClass::History,
    }
}

/// Immutable, lifetime-long policy of an Agent Chat surface (Oracle
/// 2026-07-21, WP3-A). Captured ONCE at view construction from the launch
/// variant and never changed afterwards — `ui_variant` is mutable
/// presentation state (`set_ui_variant` restyles reused/rehosted views), so
/// deriving capabilities from it would let a cached Quick AI thread be
/// "mode-laundered" into a full surface by a later `set_ui_variant(Standard)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentChatSessionPolicy {
    /// Full Agent Chat: every capability.
    Full,
    /// Quick AI: zero-context — text paste and web search only.
    QuickAi,
}

impl AgentChatSessionPolicy {
    /// Resolve the immutable policy from the LAUNCH variant.
    pub(crate) fn for_launch_variant(variant: AgentChatUiVariant) -> Self {
        match variant {
            AgentChatUiVariant::QuickAi => Self::QuickAi,
            _ => Self::Full,
        }
    }

    pub(crate) fn capabilities(self) -> AgentChatCapabilities {
        match self {
            Self::Full => AgentChatCapabilities::FULL,
            Self::QuickAi => AgentChatCapabilities::QUICK_AI,
        }
    }

    /// Whether completed turns are automatically retained. Gates every
    /// automatic on-disk / in-memory egress from a finished turn:
    /// prompt-recall history, conversation + history-index + auto-title
    /// writes, Brain ingest + day-trace append, saved-message load, and the
    /// content-bearing "response ready / turn failed" OS notifications.
    /// Quick AI is zero-retention (WP-B1, Oracle phase-b audit): a launch that
    /// promises no ambient context must not quietly turn every quick question
    /// into recallable retained state.
    pub(crate) const fn allows_automatic_transcript_retention(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Whether a closed view's in-memory thread (its transcript + composer
    /// draft) may be reused by a later launch. Quick AI NEVER reuses a
    /// retained thread — even a QuickAi→QuickAi reopen must start fresh so a
    /// prior quick question can never be resurrected (WP-B1).
    pub(crate) const fn allows_retained_thread_reuse(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Whether fork/rewind checkpoint state is maintained after a finished
    /// turn. Quick AI has no editable/persisted history, so it never refreshes
    /// fork points (WP-B1).
    pub(crate) const fn allows_fork_state(self) -> bool {
        matches!(self, Self::Full)
    }

    /// The tool-admission policy this session grants a live turn (WP-B2). Full
    /// surfaces run every tool; Quick AI is web-search-only.
    pub(crate) const fn tool_policy(self) -> AgentChatToolPolicy {
        match self {
            Self::Full => AgentChatToolPolicy::Full,
            Self::QuickAi => AgentChatToolPolicy::WebSearchOnly,
        }
    }

    /// The SINGLE adjudication point for whether a class of context may enter
    /// this session (WP-B2, Oracle seat 2). Every context ingress in the
    /// thread routes its part through [`classify_context_part`] and then here,
    /// so a surface can never admit context the policy forbids through any one
    /// un-guarded call site. The pair match is exhaustive with no wildcard on
    /// `(policy, class)` so a new policy or class fails to compile until its
    /// admission decision is made explicit.
    ///
    /// Meaning for Quick AI: only user-authored composer text is admitted.
    /// Skills, resolved flow definitions, `@file`/screenshot/history/note/
    /// selection/browser-state/cwd, and ambient desktop capture are all denied
    /// (an earlier pass allowed slash-skill staging; Oracle seat 2 overrules
    /// that — skills are denied absent an explicit promotion to Full, which
    /// does not exist yet).
    pub(crate) fn admit_context(
        self,
        class: AgentChatContextClass,
    ) -> Result<(), ContextAdmissionError> {
        match (self, class) {
            (Self::Full, _) => Ok(()),
            (Self::QuickAi, AgentChatContextClass::UserAuthoredText) => Ok(()),
            (Self::QuickAi, _) => Err(ContextAdmissionError::DeniedBySessionPolicy),
        }
    }
}

/// The affordances a given Agent Chat surface may use, for its whole lifetime.
///
/// Ordinary text paste and web search are always available and are therefore
/// not represented here — they are not context-leaking capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AgentChatCapabilities {
    /// `@`/`>` context portals (files, projects, working directory, etc.).
    pub(crate) context_portals: bool,
    /// Pasting/attaching local files and images from the clipboard.
    pub(crate) local_attachments: bool,
    /// The empty-composer Tab affordance that opens the `>` cwd picker.
    pub(crate) cwd_picker: bool,
    /// The conversation history popup.
    pub(crate) history: bool,
    /// Creating or switching to retained/persistent threads.
    pub(crate) retained_threads: bool,
    /// Switching the provider/model profile in place.
    pub(crate) profile_switch: bool,
}

impl AgentChatCapabilities {
    /// The full Agent Chat surface: everything is allowed.
    pub(crate) const FULL: Self = Self {
        context_portals: true,
        local_attachments: true,
        cwd_picker: true,
        history: true,
        retained_threads: true,
        profile_switch: true,
    };

    /// Quick AI: text paste and web search only. Every context-bearing
    /// affordance is denied. `profile_switch` is DISABLED until real
    /// relaunch/promotion ships (Oracle 2026-07-21 WP3-E: the live-chat
    /// profile path only swaps the LABEL via `set_profile_display` without
    /// replacing the connection/thread, so allowing it would let the shown
    /// profile diverge from the active runtime — worse than no switching).
    pub(crate) const QUICK_AI: Self = Self {
        context_portals: false,
        local_attachments: false,
        cwd_picker: false,
        history: false,
        retained_threads: false,
        profile_switch: false,
    };

    /// Resolve the capability set for a UI variant.
    pub(crate) fn for_variant(variant: AgentChatUiVariant) -> Self {
        match variant {
            AgentChatUiVariant::QuickAi => Self::QUICK_AI,
            _ => Self::FULL,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_ai_denies_context_affordances() {
        let caps = AgentChatCapabilities::for_variant(AgentChatUiVariant::QuickAi);
        assert!(!caps.context_portals);
        assert!(!caps.local_attachments);
        assert!(!caps.cwd_picker);
        assert!(!caps.history);
        assert!(!caps.retained_threads);
        // Disabled until real relaunch/promotion ships — label-only profile
        // switching would let the shown profile diverge from the runtime.
        assert!(!caps.profile_switch);
    }

    /// WP3-A: the session policy is resolved from the LAUNCH variant and is
    /// the capability authority; a later presentation change cannot elevate.
    #[test]
    fn session_policy_is_capability_authority() {
        let quick = AgentChatSessionPolicy::for_launch_variant(AgentChatUiVariant::QuickAi);
        assert_eq!(quick, AgentChatSessionPolicy::QuickAi);
        assert_eq!(quick.capabilities(), AgentChatCapabilities::QUICK_AI);
        let full = AgentChatSessionPolicy::for_launch_variant(AgentChatUiVariant::Standard);
        assert_eq!(full.capabilities(), AgentChatCapabilities::FULL);
    }

    #[test]
    fn standard_surface_allows_everything() {
        let caps = AgentChatCapabilities::for_variant(AgentChatUiVariant::Standard);
        assert_eq!(caps, AgentChatCapabilities::FULL);
    }

    /// WP-B1: retention follows the thread-owned policy — Quick AI threads
    /// must never write conversation/index/prompt history, ingest Brain
    /// memory, reuse a retained thread, or maintain fork state; every other
    /// variant keeps normal persistence. Replaces the removed
    /// `retention_for_launch_variant` free function; the three policy-derived
    /// helpers are now the sole authority.
    #[test]
    fn retention_is_denied_only_for_quick_ai_launches() {
        let quick = AgentChatSessionPolicy::QuickAi;
        assert!(!quick.allows_automatic_transcript_retention());
        assert!(!quick.allows_retained_thread_reuse());
        assert!(!quick.allows_fork_state());

        let full = AgentChatSessionPolicy::Full;
        assert!(full.allows_automatic_transcript_retention());
        assert!(full.allows_retained_thread_reuse());
        assert!(full.allows_fork_state());

        // Every non-Quick launch variant resolves to Full and retains.
        assert!(
            AgentChatSessionPolicy::for_launch_variant(AgentChatUiVariant::Standard)
                .allows_automatic_transcript_retention()
        );
        assert!(
            AgentChatSessionPolicy::for_launch_variant(AgentChatUiVariant::FocusedTextMini)
                .allows_automatic_transcript_retention()
        );
        assert_eq!(
            AgentChatSessionPolicy::for_launch_variant(AgentChatUiVariant::QuickAi),
            AgentChatSessionPolicy::QuickAi
        );
    }

    fn sample_parts() -> Vec<(AiContextPart, AgentChatContextClass)> {
        use crate::ai::message_parts::{ASK_ANYTHING_LABEL, ASK_ANYTHING_RESOURCE_URI};
        vec![
            (
                AiContextPart::ResourceUri {
                    uri: ASK_ANYTHING_RESOURCE_URI.to_string(),
                    label: ASK_ANYTHING_LABEL.to_string(),
                },
                AgentChatContextClass::Ambient,
            ),
            (
                AiContextPart::ResourceUri {
                    uri: "kit://resource?doc=1".to_string(),
                    label: "Doc".to_string(),
                },
                AgentChatContextClass::ProtocolResource,
            ),
            (
                AiContextPart::FilePath {
                    path: "/tmp/notes.txt".to_string(),
                    label: "notes.txt".to_string(),
                },
                AgentChatContextClass::LocalFile,
            ),
            (
                AiContextPart::FilePath {
                    path: "/tmp/shot.png".to_string(),
                    label: "shot.png".to_string(),
                },
                AgentChatContextClass::Image,
            ),
            (
                AiContextPart::SkillFile {
                    path: "/tmp/skill.md".to_string(),
                    label: "Skill".to_string(),
                    skill_name: "skill".to_string(),
                    owner_label: "owner".to_string(),
                    slash_name: "skill".to_string(),
                },
                AgentChatContextClass::Skill,
            ),
            (
                AiContextPart::AmbientContext {
                    label: "Full Screen".to_string(),
                },
                AgentChatContextClass::Ambient,
            ),
            (
                AiContextPart::TextBlock {
                    label: "Snippet".to_string(),
                    source: "clipboard".to_string(),
                    text: "hello".to_string(),
                    mime_type: None,
                },
                AgentChatContextClass::History,
            ),
        ]
    }

    /// WP-B2 (Oracle seat 2): the exhaustive context-admission matrix. Under a
    /// Full policy every context part is admitted; under Quick AI every
    /// context-bearing part is denied, while the user-text channel stays open.
    #[test]
    fn quick_ai_context_admission_matrix() {
        for (part, expected_class) in sample_parts() {
            assert_eq!(
                classify_context_part(&part),
                expected_class,
                "{part:?} should classify as {expected_class:?}",
            );
            assert!(
                AgentChatSessionPolicy::Full
                    .admit_context(classify_context_part(&part))
                    .is_ok(),
                "Full admits every part: {part:?}",
            );
            assert_eq!(
                AgentChatSessionPolicy::QuickAi.admit_context(classify_context_part(&part)),
                Err(ContextAdmissionError::DeniedBySessionPolicy),
                "Quick AI denies every context-bearing part: {part:?}",
            );
        }

        // The user-text channel is the sole class Quick AI admits.
        assert!(AgentChatSessionPolicy::QuickAi
            .admit_context(AgentChatContextClass::UserAuthoredText)
            .is_ok());
    }

    /// WP-B2: tool admission is by canonical id only and Quick AI never shows
    /// a permission prompt (its one tool needs no approval).
    #[test]
    fn quick_ai_tool_policy_is_web_search_only() {
        let quick = AgentChatSessionPolicy::QuickAi.tool_policy();
        assert_eq!(quick, AgentChatToolPolicy::WebSearchOnly);
        assert!(quick.allows_tool(WEB_SEARCH_TOOL_ID));
        assert!(!quick.allows_tool("bash"));
        assert!(!quick.allows_tool("read"));
        assert!(!quick.allows_permission_prompts());

        let full = AgentChatSessionPolicy::Full.tool_policy();
        assert_eq!(full, AgentChatToolPolicy::Full);
        assert!(full.allows_tool("bash"));
        assert!(full.allows_permission_prompts());
    }

    #[test]
    fn only_quick_ai_is_restricted() {
        for variant in [
            AgentChatUiVariant::Standard,
            AgentChatUiVariant::FocusedTextMini,
        ] {
            assert_eq!(
                AgentChatCapabilities::for_variant(variant),
                AgentChatCapabilities::FULL,
                "{variant:?} should keep full capabilities",
            );
        }
    }
}
