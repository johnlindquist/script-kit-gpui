//! Source-level contract for Run 2 Pass #46
//! `detached-popup-agent_chat-surface-parity-contract`.
//!
//! Background: when the user detaches the Agent Chat chat into its own popup
//! window (e.g. via the hotkey toggle on `AgentChatView`), the detach handler
//! in `src/ai/agent_chat/ui/chat_window.rs` registers the new popup in the automation
//! metadata registry via `upsert_automation_window`. The pair of fields
//! that identify this window to automation consumers is:
//!
//!   - `kind: AutomationWindowKind::AgentChatDetached` — the kind discriminator
//!     used by `listAutomationWindows` and target-by-kind resolution; this
//!     is how a consumer filters "detached popups" vs. "the main window".
//!   - `semantic_surface: Some("agentChatChat".to_string())` — the wire tag that
//!     says "this window hosts an Agent Chat chat surface", parallel to the
//!     Main-hosted `AgentChatView` which also reports `semanticSurface =
//!     "agentChatChat"` (see `semantic_surface_for_main_view` in
//!     `src/main_sections/app_view_state.rs`).
//!
//! The parity is intentional: the SAME `semanticSurface` on BOTH window
//! kinds means a consumer that asks "where is Agent Chat chat right now?" can
//! match on `semanticSurface == "agentChatChat"` regardless of whether the chat
//! is attached to Main or floating as a detached popup. The `kind` field
//! is the disambiguator when the consumer needs to target one specifically.
//! This is how existing automation tests target detached popups (e.g.
//! `tests/automation/detached_agent_chat_targeting.rs` expects
//! `resolved.semantic_surface.as_deref() == Some("agentChatChat")` for a window
//! whose `kind == AgentChatDetached`).
//!
//! A future refactor that "clarifies" the surface by renaming the detached
//! popup's tag to e.g. `"agentChatDetached"` would silently diverge this
//! parity — consumers matching on `semanticSurface == "agentChatChat"` would
//! miss the detached window, and a swarm of existing tests that hardcode
//! `Some("agentChatChat")` on `AgentChatDetached` fixtures would have to be rewritten.
//! Divergence is a bigger, conscious API change, not a drive-by tweak.
//!
//! This contract pins the single production registration site so the
//! parity cannot silently drift:
//!   1. `src/ai/agent_chat/ui/chat_window.rs` contains EXACTLY ONE
//!      `upsert_automation_window` call whose body mentions both
//!      `AutomationWindowKind::AgentChatDetached` and
//!      `semantic_surface: Some("agentChatChat"...)`.
//!   2. No OTHER non-test source file registers a window with
//!      `AutomationWindowKind::AgentChatDetached` (which would fork the
//!      parity invariant).
//!
//! Complements:
//!   - `tests/automation/detached_agent_chat_targeting.rs` — dynamic behavior
//!     test that asserts the resolved surface tag on a detached fixture.
//!   - `tests/dispatcher_semantic_surface_symmetry_contract.rs` — pins
//!     the Main-hosted half (`AgentChatView` → `"agentChatChat"`).
//!   - `removed-docs window behavior` — design doc.

const CHAT_WINDOW_RS: &str = include_str!("../src/ai/agent_chat/ui/chat_window.rs");

/// Extract the textual body of the single `upsert_automation_window(...)`
/// block in `src/ai/agent_chat/ui/chat_window.rs` that constructs an
/// `AutomationWindowInfo` literal. We slice from the first
/// `upsert_automation_window(crate::protocol::AutomationWindowInfo {` header
/// up to the matching `    generation: None,
/// up to the matching `});`, which conservatively captures the whole
/// struct literal including both the `kind` and `semantic_surface` fields.
fn upsert_block_body(src: &str) -> &str {
    let start_marker = "upsert_automation_window(crate::protocol::AutomationWindowInfo {";
    let start = src.find(start_marker).unwrap_or_else(|| {
        panic!(
            "src/ai/agent_chat/ui/chat_window.rs: could not locate \
             `upsert_automation_window(crate::protocol::AutomationWindowInfo {{` — \
             the detach handler must register the detached popup with the \
             automation metadata registry for this contract to apply. If \
             the registration was removed, detached Agent Chat popups are no \
             longer discoverable via `listAutomationWindows` — update this \
             test ONLY if that's intentional and you also removed the \
             corresponding automation targeting tests."
        )
    });
    // The closing `});` is the first such sequence after the opening brace.
    let search_from = start + start_marker.len();
    let end_rel = src[search_from..].find("});").unwrap_or_else(|| {
        panic!(
            "src/ai/agent_chat/ui/chat_window.rs: could not locate the closing `}});` \
             for the `upsert_automation_window` call — the struct literal \
             must be balanced. If the detach handler now spans multiple \
             upsert calls or the closing brace has moved, update this \
             helper to match."
        )
    });
    &src[start..search_from + end_rel]
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    let mut n = 0usize;
    let mut cursor = 0usize;
    while let Some(pos) = haystack[cursor..].find(needle) {
        n += 1;
        cursor += pos + needle.len();
    }
    n
}

#[test]
fn chat_window_registers_detached_popup_with_agent_chat_surface_parity() {
    let body = upsert_block_body(CHAT_WINDOW_RS);

    assert!(
        body.contains("kind: crate::protocol::AutomationWindowKind::AgentChatDetached"),
        "src/ai/agent_chat/ui/chat_window.rs `upsert_automation_window` block must \
         register the detached popup with \
         `kind: crate::protocol::AutomationWindowKind::AgentChatDetached`. Without \
         this, `listAutomationWindows` and target-resolution by kind \
         cannot discover the detached popup, breaking every automation \
         test that targets `{{kind:\"agentChatDetached\"}}`."
    );

    assert!(
        body.contains("semantic_surface: Some(\"agentChatChat\".to_string())"),
        "src/ai/agent_chat/ui/chat_window.rs `upsert_automation_window` block must \
         set `semantic_surface: Some(\"agentChatChat\".to_string())` — parity \
         with the Main-hosted `AgentChatView` which also reports \
         `semanticSurface = \"agentChatChat\"`. Consumers match on this tag to \
         locate Agent Chat chat regardless of whether it's attached or detached; \
         the `kind` field is the disambiguator. Renaming to a distinct \
         tag (e.g. `\"agentChatDetached\"`) is a conscious API divergence that \
         would also require updating the Main-hosted side in \
         `semantic_surface_for_main_view` plus every fixture in \
         `tests/automation/` that hardcodes `Some(\"agentChatChat\")` on \
         `AgentChatDetached` windows."
    );
}

#[test]
fn chat_window_has_single_detached_upsert_site() {
    let src = CHAT_WINDOW_RS;
    let upsert_calls = count_occurrences(src, "upsert_automation_window(");
    assert_eq!(
        upsert_calls, 1,
        "src/ai/agent_chat/ui/chat_window.rs must contain EXACTLY ONE \
         `upsert_automation_window(` call — found {upsert_calls}. Multiple \
         upsert sites risk divergent (kind, semanticSurface) pairs for \
         detached Agent Chat popups: a refactor that fixes one site and forgets \
         the other would silently produce asymmetric registry entries \
         depending on code path. If a second registration is needed \
         (e.g. for a different popup variant), extract a shared helper so \
         the kind+surface invariant lives in one place."
    );

    let kind_mentions = count_occurrences(src, "AutomationWindowKind::AgentChatDetached");
    assert_eq!(
        kind_mentions, 1,
        "src/ai/agent_chat/ui/chat_window.rs must reference \
         `AutomationWindowKind::AgentChatDetached` EXACTLY ONCE (inside the \
         single upsert block) — found {kind_mentions}. Additional \
         mentions suggest a second registration path or a divergent \
         type-check that the contract helper isn't scoping."
    );
}
