//! Env-gated, proof-grade hot-path counters for the chat/transcript/flow render
//! surfaces (WP-B3 rebuild of the original WP5 instrumentation).
//!
//! Every later performance package (WPs 8–18) must be *provable* by these
//! counters rather than by eyeballing a screen recording. The whole subsystem is
//! a no-op unless `SCRIPT_KIT_CHAT_HOT_COUNTERS` is set in the environment,
//! checked exactly once via [`enabled`]'s `OnceLock`.
//!
//! ## Semantic split (WP-B3)
//!
//! The original counters conflated *scanned* vs *changed* work, *received* vs
//! *applied* events, and *requested* vs *actual* renders — so a green number
//! could hide the real amplification. This module replaces every ambiguous field
//! with a semantic pair: how much was *inspected* and how much actually
//! *changed*; how many events *arrived* and how many were *effective*; how many
//! renders were *requested* and how many actually *painted*.
//!
//! ## Scoping
//!
//! [`ChatHotScope`] tags a metered surface (Agent Chat, Quick AI, Flow chat).
//! The vendored engines (gpui `List` layout, gpui-component `TextViewState`
//! markdown parse) default to **unscoped ⇒ do not count**: only a state that a
//! chat surface has explicitly opted in via `set_hot_metered(true)` contributes,
//! so the main-menu list and unrelated text views never pollute the reading.
//!
//! ## Zero-cost-when-disabled contract
//!
//! Every `record_*` helper starts with `if !enabled() { return; }`. When the
//! gate is unset that is a single already-initialized `OnceLock` load plus a
//! branch — no atomics touched, no formatting, no allocation. Only when the gate
//! is on do we touch the `Relaxed` atomics (counters, never used for
//! synchronization) and, at snapshot time, format a single log line.
//!
//! ## How the numbers get out
//!
//! Counters are cumulative process-wide atomics. [`log_snapshot`] emits one
//! `tracing::info!(target: "script_kit::chat_hot", …)` line carrying every
//! counter — app-owned *and* the two vendored engines, read through their public
//! snapshot getters. The line lands in the protocol log ring, so a devtools probe
//! reads it back with `getLogs({ target: "script_kit::chat_hot" })`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// tracing target for every counter snapshot line. Probes filter on this.
pub const CHAT_HOT_TARGET: &str = "script_kit::chat_hot";

/// Env var that arms the counters. Unset ⇒ the entire subsystem is inert.
const ENABLE_ENV: &str = "SCRIPT_KIT_CHAT_HOT_COUNTERS";

/// Minimum spacing between throttled snapshot lines so a per-delta or per-frame
/// hot site cannot flood the log ring while still producing a steady stream of
/// readings a probe can sample.
const SNAPSHOT_THROTTLE: Duration = Duration::from_millis(200);

/// Which metered chat surface a counting site (or a vendored `List`/`TextView`
/// state) belongs to. Vendored engines only count when a state has been tagged
/// with one of these; an untagged state is "unscoped" and never counts, so
/// unrelated app lists/text views stay out of the chat reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChatHotScope {
    /// Full Agent Chat transcript surface.
    AgentChat,
    /// Quick AI (mini) transcript surface.
    QuickAi,
    /// Flow chat / codex app-server child session surface.
    FlowChat,
}

/// Checked exactly once; every hot site loads this already-initialized bool.
fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var(ENABLE_ENV)
            .map(|v| !v.is_empty() && v != "0" && v != "false")
            .unwrap_or(false)
    })
}

/// Public gate probe/tests can read without re-parsing the env themselves.
pub fn counters_enabled() -> bool {
    enabled()
}

// --- App-owned atomics -------------------------------------------------------
// Grouped by the file that owns the hot path they measure. `Relaxed` is correct:
// these are pure monotonic counters, never used to establish happens-before.

// src/ai/agent_chat/ui/thread.rs — backend ingress, foreground apply, commits.
static AGENT_EVENTS_RECEIVED: AtomicU64 = AtomicU64::new(0);
static AGENT_FOREGROUND_BATCHES: AtomicU64 = AtomicU64::new(0);
static AGENT_EVENTS_APPLIED: AtomicU64 = AtomicU64::new(0);
static AGENT_ASSISTANT_ROW_COMMITS: AtomicU64 = AtomicU64::new(0);
static AGENT_ASSISTANT_BYTES_COMMITTED: AtomicU64 = AtomicU64::new(0);

// src/ai/agent_chat/ui/components/transcript.rs — reconcile scan + change + render.
static TRANSCRIPT_RECONCILE_PASSES: AtomicU64 = AtomicU64::new(0);
static TRANSCRIPT_ROWS_SCANNED: AtomicU64 = AtomicU64::new(0);
static TRANSCRIPT_BYTES_SCANNED: AtomicU64 = AtomicU64::new(0);
static TRANSCRIPT_ROWS_CHANGED: AtomicU64 = AtomicU64::new(0);
static TRANSCRIPT_BYTES_CHANGED: AtomicU64 = AtomicU64::new(0);
static TRANSCRIPT_RENDER_CALLS: AtomicU64 = AtomicU64::new(0);

// src/prompts/chat/state.rs — turn-cache rebuilds + flushes.
static CHAT_TURN_CACHE_REBUILDS: AtomicU64 = AtomicU64::new(0);
static CHAT_TURN_CACHE_INPUT_MESSAGES: AtomicU64 = AtomicU64::new(0);
static CHAT_TURN_CACHE_ROWS: AtomicU64 = AtomicU64::new(0);
static CHAT_TURN_CACHE_BYTES_SCANNED: AtomicU64 = AtomicU64::new(0);
static CHAT_SCHEDULED_FLUSHES: AtomicU64 = AtomicU64::new(0);
static CHAT_TERMINAL_FLUSHES: AtomicU64 = AtomicU64::new(0);

// src/render_builtins/flow_ux.rs — tick wakes, render requests vs actual renders,
// event ingress vs effective, child commits, session scans, stdout copy.
static FLOW_TICK_WAKES: AtomicU64 = AtomicU64::new(0);
static FLOW_RENDER_REQUESTS: AtomicU64 = AtomicU64::new(0);
static FLOW_DESK_RENDER_CALLS: AtomicU64 = AtomicU64::new(0);
static FLOW_SESSION_RENDER_CALLS: AtomicU64 = AtomicU64::new(0);
static FLOW_EVENTS_RECEIVED: AtomicU64 = AtomicU64::new(0);
static FLOW_EVENTS_EFFECTIVE: AtomicU64 = AtomicU64::new(0);
static FLOW_CHILD_COMMITS: AtomicU64 = AtomicU64::new(0);
static FLOW_CHILD_BYTES_COMMITTED: AtomicU64 = AtomicU64::new(0);
static FLOW_SESSIONS_SCANNED: AtomicU64 = AtomicU64::new(0);
static FLOW_STDOUT_BYTES_COPIED: AtomicU64 = AtomicU64::new(0);

#[inline]
fn bump(counter: &AtomicU64, by: u64) {
    if !enabled() {
        return;
    }
    counter.fetch_add(by, Ordering::Relaxed);
}

// --- Agent Chat thread (src/ai/agent_chat/ui/thread.rs) ----------------------

/// One event pulled off a backend `AgentChatEventRx` (main stream, fork, model
/// refresh — all ingress). The raw arrival rate WP10's batching must shrink.
#[inline]
pub fn record_agent_event_received() {
    bump(&AGENT_EVENTS_RECEIVED, 1);
}

/// One foreground apply *batch* — a single `cx.update` that drains one or more
/// received events. `_size` is the number of events collected before entering
/// the update (currently 1 per update; kept in the signature so a future
/// batch-drain at the ingress can report a real batch size without a churned
/// call site).
#[inline]
pub fn record_agent_foreground_batch(_size: u64) {
    bump(&AGENT_FOREGROUND_BATCHES, 1);
}

/// One event that survived the generation/session-validity guard and was
/// actually reduced into thread state. Distinct from *received*: a stale-
/// generation event is received but never applied.
#[inline]
pub fn record_agent_event_applied() {
    bump(&AGENT_EVENTS_APPLIED, 1);
}

/// A streaming text delta that actually mutated a visible assistant row (drain
/// or flush returned `changed`), plus the committed byte count.
#[inline]
pub fn record_agent_assistant_commit(bytes: usize) {
    if !enabled() {
        return;
    }
    AGENT_ASSISTANT_ROW_COMMITS.fetch_add(1, Ordering::Relaxed);
    AGENT_ASSISTANT_BYTES_COMMITTED.fetch_add(bytes as u64, Ordering::Relaxed);
}

// --- Transcript (src/ai/agent_chat/ui/components/transcript.rs) --------------

/// One `set_messages` reconcile pass that passed the identity guard. Counted
/// once *before* the per-row loop so scanned/changed row counts attribute to it.
#[inline]
pub fn record_transcript_reconcile_pass() {
    bump(&TRANSCRIPT_RECONCILE_PASSES, 1);
}

/// One message row *inspected* during reconcile (whether or not it changed),
/// plus the bytes examined. This is the scan cost WP8 must bound to the tail.
#[inline]
pub fn record_transcript_row_scanned(bytes: usize) {
    if !enabled() {
        return;
    }
    TRANSCRIPT_ROWS_SCANNED.fetch_add(1, Ordering::Relaxed);
    TRANSCRIPT_BYTES_SCANNED.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// One message row whose `TextViewState` was actually (re)built/re-parsed, plus
/// the changed byte length handed to the parser.
#[inline]
pub fn record_transcript_row_changed(bytes: usize) {
    if !enabled() {
        return;
    }
    TRANSCRIPT_ROWS_CHANGED.fetch_add(1, Ordering::Relaxed);
    TRANSCRIPT_BYTES_CHANGED.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// One `Render::render` pass of the transcript element.
#[inline]
pub fn record_transcript_render() {
    bump(&TRANSCRIPT_RENDER_CALLS, 1);
}

// --- ChatPrompt state (src/prompts/chat/state.rs) ----------------------------

/// A turn-cache rebuild in `ensure_conversation_turns_cache`, tagged with the
/// number of input messages consumed, rows produced, and bytes scanned. Frequent
/// large rebuilds during a stream are exactly the amplification WP8/WP9 target.
#[inline]
pub fn record_chat_turn_cache_rebuild(input_messages: usize, rows: usize, bytes_scanned: usize) {
    if !enabled() {
        return;
    }
    CHAT_TURN_CACHE_REBUILDS.fetch_add(1, Ordering::Relaxed);
    CHAT_TURN_CACHE_INPUT_MESSAGES.fetch_add(input_messages as u64, Ordering::Relaxed);
    CHAT_TURN_CACHE_ROWS.fetch_add(rows as u64, Ordering::Relaxed);
    CHAT_TURN_CACHE_BYTES_SCANNED.fetch_add(bytes_scanned as u64, Ordering::Relaxed);
}

/// One throttled/scheduled visible stream flush (`flush_stream_updates`) — the
/// 33 ms publish clock that marks the turn cache dirty and notifies the view.
#[inline]
pub fn record_chat_scheduled_flush() {
    bump(&CHAT_SCHEDULED_FLUSHES, 1);
}

/// One terminal flush — `complete_streaming` / `stop_streaming` — that forces a
/// final turn-cache rebuild independent of the scheduled clock.
#[inline]
pub fn record_chat_terminal_flush() {
    bump(&CHAT_TERMINAL_FLUSHES, 1);
}

// --- Flow UX (src/render_builtins/flow_ux.rs) --------------------------------

/// One 120 ms flow-tick wake. WP9 wants this to stop entirely once a session
/// settles; counting wakes proves whether the idle timer actually parked.
#[inline]
pub fn record_flow_tick_wake() {
    bump(&FLOW_TICK_WAKES, 1);
}

/// A flow tick that decided the root view is dirty and called `cx.notify()`.
/// This counts render *requests*, not renders: a flow-session tick forces
/// `dirty = true` every wake, so this rises even when nothing repaints. The
/// actual repaint is counted by the split desk/session render counters at the
/// top of the real Flow render functions (so an idle backgrounded session does
/// not look like it is re-rendering).
#[inline]
pub fn record_flow_render_request() {
    bump(&FLOW_RENDER_REQUESTS, 1);
}

/// One actual render pass of the Flow *desk* surface (`render_flow_ux`).
#[inline]
pub fn record_flow_desk_render() {
    bump(&FLOW_DESK_RENDER_CALLS, 1);
}

/// One actual render pass of the Flow *session* surface (`render_flow_session`).
#[inline]
pub fn record_flow_session_render() {
    bump(&FLOW_SESSION_RENDER_CALLS, 1);
}

/// One codex app-server `FlowThreadEvent` pulled off the stream (ingress).
#[inline]
pub fn record_flow_event_received() {
    bump(&FLOW_EVENTS_RECEIVED, 1);
}

/// A flow event that was *effective* — it actually mutated session state (a
/// non-empty child delta, a real status/turn transition). Empty deltas are
/// received but never effective.
#[inline]
pub fn record_flow_event_effective() {
    bump(&FLOW_EVENTS_EFFECTIVE, 1);
}

/// A non-empty text delta forwarded into the child `ChatPrompt` (`append_chunk`),
/// plus committed bytes. Routed through the same visible-commit helper as final
/// display suffixes so both count identically.
#[inline]
pub fn record_flow_child_commit(bytes: usize) {
    if !enabled() {
        return;
    }
    FLOW_CHILD_COMMITS.fetch_add(1, Ordering::Relaxed);
    FLOW_CHILD_BYTES_COMMITTED.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// One flow session record scanned during a render/lookup pass (the O(sessions)
/// cost WP9 must keep bounded).
#[inline]
pub fn record_flow_session_scanned() {
    bump(&FLOW_SESSIONS_SCANNED, 1);
}

/// Bytes copied out of a running mdflow child's stdout buffer.
#[inline]
pub fn record_flow_stdout_bytes_copied(bytes: usize) {
    bump(&FLOW_STDOUT_BYTES_COPIED, bytes as u64);
}

/// A full, consistent-enough reading of every counter. Cheap to build; only
/// constructed when the gate is on.
#[derive(Clone, Copy, Debug, Default)]
pub struct ChatHotSnapshot {
    pub agent_events_received: u64,
    pub agent_foreground_batches: u64,
    pub agent_events_applied: u64,
    pub agent_assistant_row_commits: u64,
    pub agent_assistant_bytes_committed: u64,
    pub transcript_reconcile_passes: u64,
    pub transcript_rows_scanned: u64,
    pub transcript_bytes_scanned: u64,
    pub transcript_rows_changed: u64,
    pub transcript_bytes_changed: u64,
    pub transcript_render_calls: u64,
    pub chat_turn_cache_rebuilds: u64,
    pub chat_turn_cache_input_messages: u64,
    pub chat_turn_cache_rows: u64,
    pub chat_turn_cache_bytes_scanned: u64,
    pub chat_scheduled_flushes: u64,
    pub chat_terminal_flushes: u64,
    pub flow_tick_wakes: u64,
    pub flow_render_requests: u64,
    pub flow_desk_render_calls: u64,
    pub flow_session_render_calls: u64,
    pub flow_events_received: u64,
    pub flow_events_effective: u64,
    pub flow_child_commits: u64,
    pub flow_child_bytes_committed: u64,
    pub flow_sessions_scanned: u64,
    pub flow_stdout_bytes_copied: u64,
    // Vendored gpui `List` layout engine (vendor/gpui/src/elements/list.rs).
    pub list_all_row_passes: u64,
    pub list_all_row_items_touched: u64,
    pub list_visible_row_passes: u64,
    pub list_visible_row_items_touched: u64,
    // Vendored gpui-component markdown parser (…/ui/src/text/state.rs).
    pub text_full_parses: u64,
    pub text_full_parse_bytes: u64,
    pub text_append_parses: u64,
    pub text_append_parse_bytes: u64,
    pub text_source_rebuild_bytes: u64,
    pub text_selection_rebuild_bytes: u64,
    // Vendored gpui per-frame draw timing (vendor/gpui/src/window.rs). Times the
    // full `Window::draw` CPU transaction; a probe derives draw_share + p95.
    pub frame_count: u64,
    pub frame_draw_busy_us_total: u64,
    pub frame_max_us: u64,
    pub frame_p95_us: u64,
    pub frames_over_33ms: u64,
}

/// Read every app-owned atomic plus both vendored engines' snapshot getters.
pub fn snapshot() -> ChatHotSnapshot {
    let list = gpui::list_hot_counter_snapshot();
    let text = gpui_component::text::text_state_hot_counter_snapshot();
    let frame = gpui::window_frame_hot_counter_snapshot();
    ChatHotSnapshot {
        agent_events_received: AGENT_EVENTS_RECEIVED.load(Ordering::Relaxed),
        agent_foreground_batches: AGENT_FOREGROUND_BATCHES.load(Ordering::Relaxed),
        agent_events_applied: AGENT_EVENTS_APPLIED.load(Ordering::Relaxed),
        agent_assistant_row_commits: AGENT_ASSISTANT_ROW_COMMITS.load(Ordering::Relaxed),
        agent_assistant_bytes_committed: AGENT_ASSISTANT_BYTES_COMMITTED.load(Ordering::Relaxed),
        transcript_reconcile_passes: TRANSCRIPT_RECONCILE_PASSES.load(Ordering::Relaxed),
        transcript_rows_scanned: TRANSCRIPT_ROWS_SCANNED.load(Ordering::Relaxed),
        transcript_bytes_scanned: TRANSCRIPT_BYTES_SCANNED.load(Ordering::Relaxed),
        transcript_rows_changed: TRANSCRIPT_ROWS_CHANGED.load(Ordering::Relaxed),
        transcript_bytes_changed: TRANSCRIPT_BYTES_CHANGED.load(Ordering::Relaxed),
        transcript_render_calls: TRANSCRIPT_RENDER_CALLS.load(Ordering::Relaxed),
        chat_turn_cache_rebuilds: CHAT_TURN_CACHE_REBUILDS.load(Ordering::Relaxed),
        chat_turn_cache_input_messages: CHAT_TURN_CACHE_INPUT_MESSAGES.load(Ordering::Relaxed),
        chat_turn_cache_rows: CHAT_TURN_CACHE_ROWS.load(Ordering::Relaxed),
        chat_turn_cache_bytes_scanned: CHAT_TURN_CACHE_BYTES_SCANNED.load(Ordering::Relaxed),
        chat_scheduled_flushes: CHAT_SCHEDULED_FLUSHES.load(Ordering::Relaxed),
        chat_terminal_flushes: CHAT_TERMINAL_FLUSHES.load(Ordering::Relaxed),
        flow_tick_wakes: FLOW_TICK_WAKES.load(Ordering::Relaxed),
        flow_render_requests: FLOW_RENDER_REQUESTS.load(Ordering::Relaxed),
        flow_desk_render_calls: FLOW_DESK_RENDER_CALLS.load(Ordering::Relaxed),
        flow_session_render_calls: FLOW_SESSION_RENDER_CALLS.load(Ordering::Relaxed),
        flow_events_received: FLOW_EVENTS_RECEIVED.load(Ordering::Relaxed),
        flow_events_effective: FLOW_EVENTS_EFFECTIVE.load(Ordering::Relaxed),
        flow_child_commits: FLOW_CHILD_COMMITS.load(Ordering::Relaxed),
        flow_child_bytes_committed: FLOW_CHILD_BYTES_COMMITTED.load(Ordering::Relaxed),
        flow_sessions_scanned: FLOW_SESSIONS_SCANNED.load(Ordering::Relaxed),
        flow_stdout_bytes_copied: FLOW_STDOUT_BYTES_COPIED.load(Ordering::Relaxed),
        list_all_row_passes: list.all_row_passes,
        list_all_row_items_touched: list.all_row_items_touched,
        list_visible_row_passes: list.visible_row_passes,
        list_visible_row_items_touched: list.visible_row_items_touched,
        text_full_parses: text.full_parses,
        text_full_parse_bytes: text.full_parse_bytes,
        text_append_parses: text.append_parses,
        text_append_parse_bytes: text.append_parse_bytes,
        text_source_rebuild_bytes: text.source_rebuild_bytes,
        text_selection_rebuild_bytes: text.selection_rebuild_bytes,
        frame_count: frame.frame_count,
        frame_draw_busy_us_total: frame.frame_draw_busy_us_total,
        frame_max_us: frame.frame_max_us,
        frame_p95_us: frame.frame_p95_us,
        frames_over_33ms: frame.frames_over_33ms,
    }
}

/// Emit one snapshot line unconditionally (still gated on the env var). Call at
/// settle boundaries so a fresh reading always exists when a stream finishes.
pub fn log_snapshot(reason: &'static str) {
    if !enabled() {
        return;
    }
    let s = snapshot();
    tracing::info!(
        target: "script_kit::chat_hot",
        event = "chat_hot_counters",
        reason,
        agent_events_received = s.agent_events_received,
        agent_foreground_batches = s.agent_foreground_batches,
        agent_events_applied = s.agent_events_applied,
        agent_assistant_row_commits = s.agent_assistant_row_commits,
        agent_assistant_bytes_committed = s.agent_assistant_bytes_committed,
        transcript_reconcile_passes = s.transcript_reconcile_passes,
        transcript_rows_scanned = s.transcript_rows_scanned,
        transcript_bytes_scanned = s.transcript_bytes_scanned,
        transcript_rows_changed = s.transcript_rows_changed,
        transcript_bytes_changed = s.transcript_bytes_changed,
        transcript_render_calls = s.transcript_render_calls,
        chat_turn_cache_rebuilds = s.chat_turn_cache_rebuilds,
        chat_turn_cache_input_messages = s.chat_turn_cache_input_messages,
        chat_turn_cache_rows = s.chat_turn_cache_rows,
        chat_turn_cache_bytes_scanned = s.chat_turn_cache_bytes_scanned,
        chat_scheduled_flushes = s.chat_scheduled_flushes,
        chat_terminal_flushes = s.chat_terminal_flushes,
        flow_tick_wakes = s.flow_tick_wakes,
        flow_render_requests = s.flow_render_requests,
        flow_desk_render_calls = s.flow_desk_render_calls,
        flow_session_render_calls = s.flow_session_render_calls,
        flow_events_received = s.flow_events_received,
        flow_events_effective = s.flow_events_effective,
        flow_child_commits = s.flow_child_commits,
        flow_child_bytes_committed = s.flow_child_bytes_committed,
        flow_sessions_scanned = s.flow_sessions_scanned,
        flow_stdout_bytes_copied = s.flow_stdout_bytes_copied,
        list_all_row_passes = s.list_all_row_passes,
        list_all_row_items_touched = s.list_all_row_items_touched,
        list_visible_row_passes = s.list_visible_row_passes,
        list_visible_row_items_touched = s.list_visible_row_items_touched,
        text_full_parses = s.text_full_parses,
        text_full_parse_bytes = s.text_full_parse_bytes,
        text_append_parses = s.text_append_parses,
        text_append_parse_bytes = s.text_append_parse_bytes,
        text_source_rebuild_bytes = s.text_source_rebuild_bytes,
        text_selection_rebuild_bytes = s.text_selection_rebuild_bytes,
        frame_count = s.frame_count,
        frame_draw_busy_us_total = s.frame_draw_busy_us_total,
        frame_max_us = s.frame_max_us,
        frame_p95_us = s.frame_p95_us,
        frames_over_33ms = s.frames_over_33ms,
        "chat_hot_counters",
    );
}

/// Throttled snapshot for per-event / per-frame hot sites: at most one line per
/// [`SNAPSHOT_THROTTLE`]. Keeps steady readings flowing without flooding.
pub fn maybe_log_snapshot(reason: &'static str) {
    if !enabled() {
        return;
    }
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let cell = LAST.get_or_init(|| Mutex::new(None));
    let now = Instant::now();
    {
        let Ok(mut last) = cell.lock() else {
            return;
        };
        match *last {
            Some(prev) if now.duration_since(prev) < SNAPSHOT_THROTTLE => return,
            _ => *last = Some(now),
        }
    }
    log_snapshot(reason);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_and_log_do_not_panic() {
        // Regardless of gate state, these must be side-effect-safe to call.
        let _ = counters_enabled();
        record_agent_event_received();
        record_agent_foreground_batch(3);
        record_agent_event_applied();
        record_agent_assistant_commit(64);
        record_transcript_reconcile_pass();
        record_transcript_row_scanned(128);
        record_transcript_row_changed(64);
        record_transcript_render();
        record_chat_turn_cache_rebuild(4, 3, 512);
        record_chat_scheduled_flush();
        record_chat_terminal_flush();
        record_flow_tick_wake();
        record_flow_render_request();
        record_flow_desk_render();
        record_flow_session_render();
        record_flow_event_received();
        record_flow_event_effective();
        record_flow_child_commit(32);
        record_flow_session_scanned();
        record_flow_stdout_bytes_copied(256);
        let _ = snapshot();
        log_snapshot("test");
        maybe_log_snapshot("test");
    }

    #[test]
    fn scope_variants_are_distinct() {
        assert_ne!(ChatHotScope::AgentChat, ChatHotScope::QuickAi);
        assert_ne!(ChatHotScope::QuickAi, ChatHotScope::FlowChat);
    }
}
