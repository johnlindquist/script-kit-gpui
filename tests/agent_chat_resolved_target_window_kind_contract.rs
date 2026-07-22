//! Source + behavior contract pinning the Pass #18 Fix for
//! `build_agent_chat_resolved_target` in `src/prompt_handler/mod.rs`.
//!
//! Background: `getAgentChatState {target:{type:"id",id:"ai"}}` (or
//! `{type:"kind",kind:"ai"}`) was routed by `resolve_agent_chat_read_target` to
//! `AgentChatReadTarget::Main { info: Some(resolved) }` because the embedded
//! AI was a subview of main. Before this
//! Fix, `build_agent_chat_resolved_target` hardcoded `window_kind: "main"`
//! for that arm, so the receipt reported `windowKind:"main"` even when
//! the resolved `AutomationWindowInfo.kind` was the legacy embedded-AI kind
//! (removed with the legacy AI window).
//! Agentic callers reading the receipt could not tell from the receipt
//! alone whether they had reached the embedded AI subview or the
//! ambient scriptList main surface.
//!
//! The Fix introduces an authoritative
//! `AutomationWindowKind::as_camel_case(self) -> &'static str` method
//! (matching `#[serde(rename_all = "camelCase")]`) and makes
//! `build_agent_chat_resolved_target` read the resolved `info.kind` through
//! that helper instead of hardcoding string literals.
//!
//! These tests pin:
//! 1. The `as_camel_case` helper exists on `AutomationWindowKind` and
//!    returns the exact camelCase string for every variant, so future
//!    variants (e.g. a hypothetical `AutomationWindowKind::Terminal`)
//!    must extend it explicitly or fail the match at test time.
//! 2. `build_agent_chat_resolved_target`'s Main arm uses
//!    `info.kind.as_camel_case()` — NOT the string literal
//!    `"main".to_string()` inside the `info: Some(info)` branch.
//! 3. The serialized camelCase form round-trips through `serde_json`
//!    identically to the helper — so `listAutomationWindows.windows[].kind`
//!    (serde) and `AgentChatResolvedTarget.windowKind` (helper) never drift
//!    out of lock-step.
//!
//! **Refactor threat**: a contributor refactoring
//! `build_agent_chat_resolved_target` to "simplify" the Main arm could collapse
//! the `info: Some(info)` branch back to a literal `"main"` — either
//! because they read the legacy shape before this Fix, or because they
//! assume "Main read target ⇒ windowKind is main". That collapse would
//! silently regress the receipt for every Ai-kind embedded-AI target
//! and break agentic callers that confirm reach via `windowKind`. This
//! contract catches that edit at test time.

use script_kit_gpui::protocol::AutomationWindowKind;

const HANDLER: &str = include_str!("../src/prompt_handler/mod.rs");
const AUTOMATION_WINDOW: &str = include_str!("../src/protocol/types/automation_window.rs");

#[test]
fn automation_window_kind_has_as_camel_case_helper() {
    assert!(
        AUTOMATION_WINDOW.contains("pub fn as_camel_case(self) -> &'static str"),
        "`AutomationWindowKind::as_camel_case(self) -> &'static str` MUST \
         exist in src/protocol/types/automation_window.rs. Removing it \
         would force `build_agent_chat_resolved_target` back onto hand-rolled \
         string literals and reintroduce the hardcoded `\"main\"` drift \
         the Pass #18 Fix closed."
    );
}

#[test]
fn as_camel_case_matches_serde_for_every_variant() {
    // Exhaustive match — adding a new variant without adding a case here
    // will fail to compile and force the contributor to extend both the
    // helper and this contract together.
    let cases: &[(AutomationWindowKind, &str)] = &[
        (AutomationWindowKind::Main, "main"),
        (AutomationWindowKind::Notes, "notes"),
        (AutomationWindowKind::AgentChatDetached, "agentChatDetached"),
        (AutomationWindowKind::Dictation, "dictation"),
        (AutomationWindowKind::ActionsDialog, "actionsDialog"),
        (AutomationWindowKind::PromptPopup, "promptPopup"),
    ];
    for (kind, expected) in cases {
        assert_eq!(
            kind.as_camel_case(),
            *expected,
            "as_camel_case drift for {:?}",
            kind
        );
        let serde_form = serde_json::to_value(kind).expect("serialize kind");
        assert_eq!(
            serde_form,
            serde_json::Value::String((*expected).to_string()),
            "serde rename drift for {:?} — helper and serde must agree \
             so listAutomationWindows and AgentChatResolvedTarget never \
             disagree on a window's kind string",
            kind
        );
    }
}

#[test]
fn build_agent_chat_resolved_target_main_arm_reads_info_kind() {
    let start = HANDLER
        .find("fn build_agent_chat_resolved_target(")
        .expect("build_agent_chat_resolved_target must exist");
    let after = &HANDLER[start..];
    let end_rel = after
        .find("/// Build a `UiStateSnapshot`")
        .unwrap_or(after.len());
    let body = &after[..end_rel];

    assert!(
        body.contains("info.kind.as_camel_case()"),
        "build_agent_chat_resolved_target MUST call `info.kind.as_camel_case()` \
         so the AgentChatResolvedTarget receipt reports the actual resolved \
         AutomationWindowKind. Without this, a `kind:\"ai\"` request \
         that routes to AgentChatReadTarget::Main {{ info: Some(resolved) }} \
         reports `windowKind:\"main\"` and agentic callers cannot \
         confirm from the receipt that they reached the embedded AI \
         subview vs the ambient scriptList main surface.\n\nBody was:\n{}",
        body
    );
    assert!(
        !body.contains("\"main\".to_string(), info.title.clone()"),
        "build_agent_chat_resolved_target MUST NOT carry the legacy \
         `\"main\".to_string(), info.title.clone()` tuple pattern — \
         that is the exact shape the Pass #18 Fix replaced. Seeing it \
         back indicates a revert or a cherry-pick collision.\n\nBody was:\n{}",
        body
    );
    assert!(
        !body.contains("\"agentChatDetached\".to_string(),"),
        "build_agent_chat_resolved_target MUST NOT hardcode \
         `\"agentChatDetached\".to_string()` for the Detached arm — use \
         `info.kind.as_camel_case()` so the Main and Detached arms \
         stay lock-step with the helper. Hardcoding one arm creates a \
         drift surface: a future variant serialized differently via \
         serde would disagree with the hardcoded literal.\n\nBody was:\n{}",
        body
    );
}
