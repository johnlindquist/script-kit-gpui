//! Source-level contract test for the `portal-dictation-roundtrip` user story.
//!
//! The story wants a live end-to-end proof: start the dictation portal from
//! inside the Agent Chat composer, feed a synthetic transcript via `pushDictationResult`,
//! close the portal, and watch the text land at the cursor without auto-submit
//! while `getAgentChatState.dictationStatus` returns to `idle`. The stdin hook now
//! covers transcript injection, but a full portal live run still needs stable
//! `dictationStatus` exposure. Until then, this test pins the production
//! delivery invariants that the story's acceptance criteria depend on.
//!
//! Invariants pinned (all in `handle_dictation_transcript` in
//! `src/app_execute/builtin_execution.rs`):
//!
//! 1. `DictationTarget::AiChatComposer` delivers via
//!    `ai::set_ai_input(&mut **cx, &transcript, false)` — the `false` is the
//!    no-auto-submit flag that encodes the story's "nothing else changed"
//!    half: transcript appears in composer, user still presses Enter.
//!
//! 2. `DictationTarget::TabAiHarness` (the Agent Chat-hosted path) opens Agent Chat Chat
//!    with the transcript as the entry intent while suppressing focused
//!    launcher context, so the dictation becomes the initial prompt and is
//!    auto-submitted by the Agent Chat launch path.
//!
//! 3. `record_dictation_history(&transcript, audio_duration, target)` runs
//!    BEFORE the delivery `match target` block, so a delivery failure never
//!    silently drops the captured audio.
//!
//! 4. The `TabAiHarness` arm dispatches `WindowEvent::FinishDictation` inside
//!    itself, and the post-match cleanup block guards against a second
//!    dispatch with `if !matches!(target, ...::TabAiHarness)` — a regression
//!    removing the guard would double-fire orchestrator events.
//!
//! 5. `DictationSessionPhase::Idle` remains a valid terminal/inactive state
//!    the overlay can return to — approximates the story's "dictationStatus
//!    returns to idle" assertion at the type level.

const DELIVERY_SOURCE: &str = include_str!("../src/app_execute/builtin_execution.rs");
const DICTATION_TYPES_SOURCE: &str = include_str!("../src/dictation/types.rs");

fn handler_slice() -> &'static str {
    let start = DELIVERY_SOURCE
        .find("fn handle_dictation_transcript")
        .expect("handle_dictation_transcript must exist in builtin_execution.rs");
    let tail = &DELIVERY_SOURCE[start..];
    let end = tail.find("\n    fn ").unwrap_or(tail.len().min(6000));
    &tail[..end]
}

#[test]
fn ai_chat_composer_delivery_stages_agent_chat_composer_without_auto_submit() {
    let handler = handler_slice();
    assert!(
        handler.contains("DictationTarget::AiChatComposer =>"),
        "handle_dictation_transcript must have an explicit AiChatComposer \
         arm so Agent Chat-adjacent dictation has a routed delivery path"
    );
    assert!(
        handler.contains("open_agent_chat_with_composer_seed("),
        "AiChatComposer delivery must stage the transcript through the \
         composer-seed entry — that path uses AgentChatSeedPolicy::ComposerOnly, \
         the 'nothing else changed' invariant from the story. An auto-submit \
         entry would send the transcript without the user reviewing it."
    );
    let arm_start = handler
        .find("DictationTarget::AiChatComposer =>")
        .expect("AiChatComposer arm must exist");
    let arm = &handler[arm_start..arm_start + 1200.min(handler.len() - arm_start)];
    assert!(
        arm.contains("AgentChatUiVariant::Standard"),
        "AiChatComposer delivery must open the standard Agent Chat surface"
    );
}

#[test]
fn tab_ai_harness_delivery_opens_agent_chat_with_transcript_entry_intent() {
    let handler = handler_slice();
    assert!(
        handler.contains("DictationTarget::TabAiHarness =>"),
        "TabAiHarness arm (the Agent Chat-hosted tab-ai path) must remain in the \
         dispatch block — losing it would drop the Agent Chat composer target entirely"
    );
    assert!(
        handler.contains("open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part")
            && handler.contains("Some(transcript.clone())"),
        "TabAiHarness delivery must open Agent Chat Chat with the transcript as \
         the entry intent so Agent Chat auto-submits the dictated prompt without \
         inheriting the selected launcher row as context"
    );
}

#[test]
fn day_page_today_delivery_appends_capture_without_opening_ui() {
    let handler = handler_slice();
    assert!(
        handler.contains("DictationTarget::DayPageToday =>"),
        "DayPageToday arm must remain in the delivery match — it is the \
         'Today' overlay chip's destination"
    );
    assert!(
        handler.contains("append_to_day") && handler.contains("DayEntry::Capture"),
        "DayPageToday delivery must append a timestamped capture through the \
         brain substrate (lock-safe, self-indexing) rather than opening any UI"
    );
}

#[test]
fn quick_ai_delivery_submit_flag_comes_from_config() {
    let handler = handler_slice();
    assert!(
        handler.contains("DictationTarget::QuickAiQuestion =>"),
        "QuickAiQuestion arm must remain in the delivery match — it is the \
         'Ask' overlay chip's destination"
    );
    assert!(
        handler.contains("quick_ai_answers()"),
        "QuickAiQuestion delivery must derive its submit flag from \
         dictation.quickAi via quick_ai_answers() — hardcoding submit would \
         break the 'composer' config mode"
    );
    assert!(
        handler.contains("AgentChatUiVariant::QuickAi"),
        "fire-and-show must route the question to a Quick AI Agent Chat session"
    );
}

#[test]
fn history_is_recorded_before_delivery_routing() {
    let handler = handler_slice();
    // Match the call site by name — rustfmt reflows the argument list, and
    // the invariant is the ordering, not the call's line layout.
    let history_pos = handler
        .find("record_dictation_history(")
        .expect("record_dictation_history call must be present in the handler");
    let dispatch_pos = handler
        .find("let delivered_internally = match target {")
        .expect("delivery match must be present in the handler");
    assert!(
        history_pos < dispatch_pos,
        "record_dictation_history must run BEFORE the delivery match so a \
         delivery failure never silently drops the captured audio — found \
         history at {history_pos} and dispatch at {dispatch_pos}"
    );
}

#[test]
fn finish_dictation_dispatch_is_guarded_against_double_fire_for_tab_ai() {
    let handler = handler_slice();
    let tab_ai_arm_pos = handler
        .find("DictationTarget::TabAiHarness =>")
        .expect("TabAiHarness arm must exist");
    let tab_ai_arm_tail =
        &handler[tab_ai_arm_pos..tab_ai_arm_pos + 1200.min(handler.len() - tab_ai_arm_pos)];
    assert!(
        tab_ai_arm_tail.contains("crate::window_orchestrator::WindowEvent::FinishDictation"),
        "TabAiHarness arm must dispatch FinishDictation INSIDE the arm so the \
         orchestrator fires RevealMain immediately (before the post-match \
         cleanup runs)"
    );
    assert!(
        handler.contains("if !matches!(target, crate::dictation::DictationTarget::TabAiHarness) {"),
        "post-match cleanup must guard the SECOND FinishDictation dispatch \
         with `if !matches!(target, ...::TabAiHarness)` so tab-ai dictation \
         does not double-fire the orchestrator event (once in-arm, once \
         post-match would cause duplicate RevealMain)"
    );
}

#[test]
fn dictation_session_phase_idle_is_a_valid_inactive_state() {
    assert!(
        DICTATION_TYPES_SOURCE.contains("pub enum DictationSessionPhase {")
            && DICTATION_TYPES_SOURCE.contains("    Idle,"),
        "DictationSessionPhase::Idle must remain a variant of the session \
         phase enum — that is the type-level anchor for the story's \
         'dictationStatus returns to idle' assertion. If the phase enum \
         ever loses Idle, the story's acceptance becomes structurally \
         unverifiable."
    );
    assert!(
        DICTATION_TYPES_SOURCE.contains("Finished,"),
        "DictationSessionPhase::Finished must remain — the overlay \
         transitions from Recording → Transcribing → Finished, then the \
         cleanup schedule returns the runtime to Idle. Losing Finished \
         would break the overlay's terminal display."
    );
}
