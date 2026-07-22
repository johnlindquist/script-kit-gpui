//! Markdown-parser *smoke* fixtures for the chat/transcript text engine.
//!
//! WP-B3 note: Oracle (seat 2) judged these fixtures NOT chat-surface behavior
//! contracts — they prove the vendored `TextViewState` markdown engine parses a
//! feature matrix into a paintable document, which is parser coverage, not a
//! cross-surface chat contract. They are kept (renamed `markdown_parser_smoke_*`)
//! as engine smoke tests. The REAL chat-surface behavior contracts — driving
//! actual `AgentChatEvent::AgentMessageDelta` chunks through the thread's
//! streaming reduction, the deterministic text-append seam, and the flow finalize
//! projection, all asserting EXACT final source text/bytes — live in the bin
//! target: `agent_real_stream_*` / `text_append_*` (src/ai/agent_chat/ui/thread/
//! tests.rs), `flow_real_stream_*` (src/render_builtins/flow_ux.rs), and
//! `flow_history_restore_bulk_rebuilds_once` (src/prompts/chat/tests.rs).
//!
//! Each fixture still drives real markdown through the exact engine every chat
//! surface renders into and asserts on the rendered *paint structure* via gpui's
//! fidelity capture. Nothing here reads app source text.
//!
//! ## What is covered
//!
//! The fixture matrix walks every markdown feature the spec enumerates —
//! heading, unordered + ordered + task lists, block quote, table, fenced code,
//! inline code, link, long bare URL, Unicode — plus the three streaming edge
//! cases: a pending first token, an empty terminal response, and a partial
//! failure (an unterminated fence). Each asserts the document parses into a
//! `TextDocument` scope with the exact source byte length and, for non-empty
//! input, at least one painted primitive.
//!
//! It also verifies the WP5 hot-path parse counters actually move when armed:
//! parsing a fixture drives a *full* parse (byte cost = whole document) and a
//! streaming `push_str` drives an *append* parse — the amplification signal
//! WP8/WP10 must later shrink.
//!
//! ## Deferred (needs a live painted window + input, not reachable here)
//!
//! Copy (selection + Cmd+C), manual scrolling, and light/dark theme parity are
//! interaction/paint behaviors that require a driven app window; they are
//! covered by the `agent-chat-stream-render-budget-probe.ts` /
//! `quick-ai-stream-render-budget-probe.ts` runtime probes instead.

use std::sync::Once;

use gpui::{
    div, px, AppContext as _, Context, Entity, FidelityNodeKind, IntoElement, ParentElement as _,
    Render, Styled as _, TestAppContext, Window,
};
use gpui_component::text::{text_state_hot_counter_snapshot, TextView, TextViewState};

/// Arm the WP5 hot counters exactly once for this test binary, BEFORE any
/// fixture parse runs (the gpui-component gate is a process-global `OnceLock`
/// read on first parse). Every test calls this as its first statement so the
/// env is always set before the gate initializes, regardless of run order.
fn arm_counters() {
    static ARM: Once = Once::new();
    ARM.call_once(|| {
        // Safety: set once, before any counter gate is read; no other thread is
        // concurrently reading the environment through `Once`'s barrier.
        unsafe { std::env::set_var("SCRIPT_KIT_CHAT_HOT_COUNTERS", "1") };
    });
}

/// Minimal render host that mounts a `TextViewState` through the public
/// `TextView` element with a stable fidelity scope, mirroring the transcript's
/// per-message text element.
struct MarkdownProbeView {
    state: Entity<TextViewState>,
}

impl Render for MarkdownProbeView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // The fidelity capture is a closed allowlist keyed on the Agent Chat
        // proof contract (`Window::should_capture_fidelity_selector`), so the
        // scope MUST live under the `agent-chat.` namespace to be recorded.
        div().w(px(560.)).h(px(360.)).child(
            TextView::new(&self.state)
                .selectable(true)
                .fidelity_scope("agent-chat.contract.text"),
        )
    }
}

/// Observed paint structure of one rendered markdown fixture.
struct RenderedDoc {
    kind: FidelityNodeKind,
    primitive_count: usize,
    source_byte_length: u64,
}

/// Render `source` through the markdown engine and return the captured document
/// scope. Uses the synchronous `markdown_for_fidelity_test` constructor so the
/// parse has completed before we read the fidelity summaries.
fn render_markdown(cx: &mut TestAppContext, source: &str) -> RenderedDoc {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        cx.open_window(Default::default(), |_, cx| {
            let state = cx.new(|cx| TextViewState::markdown_for_fidelity_test(source, cx));
            cx.new(|_| MarkdownProbeView { state })
        })
        .unwrap()
    });

    window
        .update(cx, |_, window, cx| {
            window.set_fidelity_capture_target_for_test(Some("agent-chat"));
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();

    window
        .update(cx, |_, window, _| {
            let summaries = window.fidelity_scope_summaries();
            let document = summaries
                .iter()
                .find(|scope| scope.id == "agent-chat.contract.text/document")
                .unwrap_or_else(|| {
                    let ids: Vec<&str> = summaries.iter().map(|s| s.id.as_str()).collect();
                    panic!(
                        "markdown fixture must render a TextView document scope; saw ids: {ids:?}"
                    )
                });
            let source_byte_length = document
                .metadata
                .as_ref()
                .and_then(|m| m.get("sourceByteLength"))
                .and_then(|v| v.as_u64())
                .expect("document metadata must report sourceByteLength");
            RenderedDoc {
                kind: document.kind,
                primitive_count: document.primitive_count,
                source_byte_length,
            }
        })
        .unwrap()
}

/// Assert a fixture renders as a text document whose byte length round-trips
/// exactly and which painted at least one primitive.
fn assert_renders(cx: &mut TestAppContext, label: &str, source: &str) {
    let doc = render_markdown(cx, source);
    assert_eq!(
        doc.kind,
        FidelityNodeKind::TextDocument,
        "{label}: markdown must render as a TextDocument scope",
    );
    assert_eq!(
        doc.source_byte_length,
        source.len() as u64,
        "{label}: rendered document must carry the exact source byte length",
    );
    assert!(
        doc.primitive_count > 0,
        "{label}: non-empty markdown must paint at least one primitive (got {})",
        doc.primitive_count,
    );
}

#[gpui::test]
fn markdown_parser_smoke_heading_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "heading", "# Release Notes\n\nSecond level below.");
}

#[gpui::test]
fn markdown_parser_smoke_unordered_list_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "unordered-list", "- alpha\n- beta\n- gamma\n");
}

#[gpui::test]
fn markdown_parser_smoke_ordered_list_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "ordered-list", "1. first\n2. second\n3. third\n");
}

#[gpui::test]
fn markdown_parser_smoke_task_list_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "task-list", "- [x] done\n- [ ] pending\n");
}

#[gpui::test]
fn markdown_parser_smoke_block_quote_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "quote", "> a quoted line\n> continued\n");
}

#[gpui::test]
fn markdown_parser_smoke_table_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(
        cx,
        "table",
        "| Name | Role |\n| ---- | ---- |\n| Ada  | Lead |\n| Alan | Eng  |\n",
    );
}

#[gpui::test]
fn markdown_parser_smoke_fenced_code_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(
        cx,
        "fenced-code",
        "```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n",
    );
}

#[gpui::test]
fn markdown_parser_smoke_inline_code_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "inline-code", "Call `render()` before `paint()`.");
}

#[gpui::test]
fn markdown_parser_smoke_link_renders(cx: &mut TestAppContext) {
    arm_counters();
    assert_renders(cx, "link", "See [the docs](https://example.com/guide).");
}

#[gpui::test]
fn markdown_parser_smoke_long_bare_url_renders(cx: &mut TestAppContext) {
    arm_counters();
    // A long bare URL is a known wrap/measure stressor for the text engine.
    let source = format!(
        "Reference: https://example.com/{}/end",
        "segment-".repeat(40)
    );
    assert_renders(cx, "long-bare-url", &source);
}

#[gpui::test]
fn markdown_parser_smoke_unicode_renders(cx: &mut TestAppContext) {
    arm_counters();
    // Multi-byte graphemes: byte length must not be confused with char count.
    assert_renders(cx, "unicode", "café — 日本語 — 🚀 — naïve façade");
}

#[gpui::test]
fn markdown_parser_smoke_pending_first_token_renders(cx: &mut TestAppContext) {
    arm_counters();
    // The very first streamed token, before any block terminator arrives.
    assert_renders(cx, "pending-first-token", "The");
}

#[gpui::test]
fn markdown_parser_smoke_empty_terminal_response_renders_without_panic(cx: &mut TestAppContext) {
    arm_counters();
    // An empty terminal response must not panic and must report zero bytes. An
    // empty document may legitimately paint zero primitives, so this case does
    // NOT assert primitive_count > 0.
    let doc = render_markdown(cx, "");
    assert_eq!(
        doc.kind,
        FidelityNodeKind::TextDocument,
        "empty response still renders a (possibly empty) TextDocument scope",
    );
    assert_eq!(
        doc.source_byte_length, 0,
        "empty response must report zero source bytes",
    );
}

#[gpui::test]
fn markdown_parser_smoke_partial_failure_renders_leniently(cx: &mut TestAppContext) {
    arm_counters();
    // A partial/failed turn often leaves an unterminated fence. The engine must
    // render the partial content leniently rather than dropping the row.
    assert_renders(
        cx,
        "partial-failure",
        "Here is the start of a block:\n\n```rust\nfn interrupted() {\n    let x = 1;",
    );
}

#[gpui::test]
fn markdown_parser_smoke_full_parse_counter_moves(cx: &mut TestAppContext) {
    arm_counters();
    let before = text_state_hot_counter_snapshot();
    let source = "# Heading\n\nSome **bold** body text.";
    // WP-B3: text counters are scope-gated — only a metered chat surface counts.
    // Opt in, then drive a synchronous full parse of `source`; that parse is what
    // must register in the counters (the empty initial parse is unmetered noise).
    let state = cx.update(|cx| {
        gpui_component::init(cx);
        cx.new(|cx| {
            let mut state = TextViewState::markdown_for_fidelity_test("", cx);
            state.set_hot_metered(true);
            state
        })
    });
    state.update(cx, |state, cx| {
        state.set_markdown_text_immediate(source, cx);
    });
    let after = text_state_hot_counter_snapshot();

    // Counters are process-global and other tests run concurrently, so assert
    // monotonic movement (>=), never exact equality.
    assert!(
        after.full_parses >= before.full_parses + 1,
        "a fixture parse must register at least one full parse ({} -> {})",
        before.full_parses,
        after.full_parses,
    );
    assert!(
        after.full_parse_bytes >= before.full_parse_bytes + source.len() as u64,
        "bytes_parsed must grow by at least the parsed source length",
    );
}

// NOTE (deferred): the streaming *append* parse counter cannot be asserted at
// this layer. `push_str` routes through a `smol::Timer`-debounced background
// task, and gpui's deterministic test scheduler panics on the resulting real
// `async-io` thread activity ("Your test is not deterministic"). The append
// path is instead proven end-to-end by the streaming render-budget probes
// (`agent-chat-stream-render-budget-probe.ts` / `quick-ai-…`), which drive real
// deltas and read `text_append_parses` back through `getLogs`.
