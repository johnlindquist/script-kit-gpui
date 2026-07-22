#!/usr/bin/env bun
/**
 * WP5 render-budget receipt for the Quick AI entry surface.
 *
 * Launches the app with `SCRIPT_KIT_CHAT_HOT_COUNTERS=1`, opens Quick AI via the
 * standard mock-data entry (`openAiWithMockData` → the same shared Agent Chat
 * transcript engine Quick AI now renders into), drives a streaming-shaped
 * fixture sequence, then reads the cumulative WP5 hot-path counter snapshot back
 * from the app's log ring via `getLogs({ target: "script_kit::chat_hot" })`.
 *
 * Zero tokens: no real backend is contacted. This is the Quick-AI-surface
 * companion to `agent-chat-stream-render-budget-probe.ts`; together they set the
 * per-surface render-cost baseline every later WP (8–18) must beat.
 *
 * The ChatPrompt turn-cache counters (`chat_turn_cache_rebuilds`,
 * `chat_stream_flushes`) belong to the Flow/ChatPrompt surface, not this shared
 * Agent Chat transcript; they are exercised by `flow-ux-probe.ts` and reported
 * here (typically 0) for completeness.
 *
 * Usage:
 *   bun scripts/agentic/quick-ai-stream-render-budget-probe.ts \
 *     [--receipt /tmp/quick-ai-stream-render-budget.json]
 */
import { Driver } from "../devtools/driver.ts";

type Json = Record<string, any>;

const argOf = (name: string, fallback: string): string => {
  const flag = `--${name}`;
  const idx = process.argv.indexOf(flag);
  return idx >= 0 && process.argv[idx + 1] ? process.argv[idx + 1] : fallback;
};

const receiptPath = argOf("receipt", "/tmp/quick-ai-stream-render-budget.json");
const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/wp5-counters/script-kit-gpui";

// WP-B3 semantic counter set + per-frame draw timing.
const COUNTER_KEYS = [
  "agent_events_received",
  "agent_foreground_batches",
  "agent_events_applied",
  "agent_assistant_row_commits",
  "agent_assistant_bytes_committed",
  "transcript_reconcile_passes",
  "transcript_rows_scanned",
  "transcript_bytes_scanned",
  "transcript_rows_changed",
  "transcript_bytes_changed",
  "transcript_render_calls",
  "chat_turn_cache_rebuilds",
  "chat_turn_cache_input_messages",
  "chat_turn_cache_rows",
  "chat_turn_cache_bytes_scanned",
  "chat_scheduled_flushes",
  "chat_terminal_flushes",
  "flow_tick_wakes",
  "flow_render_requests",
  "flow_desk_render_calls",
  "flow_session_render_calls",
  "list_all_row_passes",
  "list_all_row_items_touched",
  "list_visible_row_passes",
  "list_visible_row_items_touched",
  "text_full_parses",
  "text_full_parse_bytes",
  "text_append_parses",
  "text_append_parse_bytes",
  "text_source_rebuild_bytes",
  "text_selection_rebuild_bytes",
  "frame_count",
  "frame_draw_busy_us_total",
  "frame_max_us",
  "frame_p95_us",
  "frames_over_33ms",
] as const;

const processCpuMs = async (pid: number): Promise<number> => {
  try {
    const proc = Bun.spawn(["ps", "-o", "time=", "-p", String(pid)], { stdout: "pipe" });
    const out = (await new Response(proc.stdout).text()).trim();
    const parts = out.replace("-", ":").split(":").map((p) => Number.parseFloat(p));
    let seconds = 0;
    for (const p of parts) seconds = seconds * 60 + (Number.isFinite(p) ? p : 0);
    return Math.round(seconds * 1000);
  } catch {
    return 0;
  }
};

const derivePerf = (
  before: Json | null,
  after: Json | null,
  observationWallMs: number,
  cpuMs: number,
): Json => {
  const d = (k: string) => Number(after?.[k] ?? 0) - Number(before?.[k] ?? 0);
  const drawBusyMs = d("frame_draw_busy_us_total") / 1000;
  return {
    frame_count: d("frame_count"),
    frame_max_ms: Number((Number(after?.frame_max_us ?? 0) / 1000).toFixed(3)),
    frame_p95_ms: Number((Number(after?.frame_p95_us ?? 0) / 1000).toFixed(3)),
    frames_over_33ms: d("frames_over_33ms"),
    draw_busy_ms: Number(drawBusyMs.toFixed(3)),
    observation_wall_ms: Number(observationWallMs.toFixed(1)),
    draw_share: Number((observationWallMs > 0 ? drawBusyMs / observationWallMs : 0).toFixed(4)),
    process_cpu_ms: cpuMs,
    process_cpu_percent: Number(
      (observationWallMs > 0 ? (cpuMs / observationWallMs) * 100 : 0).toFixed(1),
    ),
  };
};

const parseCounterLine = (message: string): Json | null => {
  if (!message.includes("chat_hot_counters")) return null;
  const counters: Json = {};
  let matched = 0;
  for (const key of COUNTER_KEYS) {
    const m = message.match(new RegExp(`\\b${key}=(\\d+)\\b`));
    if (m) {
      counters[key] = Number.parseInt(m[1], 10);
      matched += 1;
    }
  }
  return matched > 0 ? counters : null;
};

const readCounters = async (driver: Driver): Promise<Json | null> => {
  const result = (await driver.getLogs(
    { target: "script_kit::chat_hot", limit: 50 },
    { timeoutMs: 8_000 },
  )) as Json;
  const entries = (result.entries as Json[]) ?? [];
  for (let i = entries.length - 1; i >= 0; i--) {
    const parsed = parseCounterLine(String(entries[i]?.message ?? ""));
    if (parsed) return parsed;
  }
  return null;
};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `quick-ai-stream-render-budget-${process.pid}`,
  defaultTimeoutMs: 10_000,
  env: {
    SCRIPT_KIT_CHAT_HOT_COUNTERS: "1",
    SCRIPT_KIT_AGENT_CHAT_RENDER_TRACE: "1",
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
  },
});

const receipt: Json = { probe: "quick-ai-stream-render-budget", binary, checks: {} };
const checks = receipt.checks as Json;

try {
  receipt.target = { pid: driver.pid, sessionDir: driver.sessionDir };

  driver.send({ type: "show" });
  await driver.waitForSettle();

  // 1. Open Quick AI via the standard mock-data entry (shared Agent Chat
  //    transcript engine). Fire-and-forget: the command has no request id.
  driver.send({ type: "openAiWithMockData" });
  await Bun.sleep(900);

  // 2. Drive a streaming-shaped sequence: pending first token → growing
  //    markdown → settle. Each transition re-renders + re-parses.
  const phaseSequence: Array<{ phase: string; assistantText?: string }> = [
    { phase: "awaitingFirstAssistantText" },
    { phase: "assistantText", assistantText: "Q" },
    { phase: "assistantText", assistantText: "Quick answer with `inline code`." },
    {
      phase: "assistantText",
      assistantText:
        "Quick answer with `inline code`.\n\n1. first\n2. second\n\n> a quoted note\n\n[link](https://example.com/quick)",
    },
    { phase: "idle", assistantText: "Quick AI settled response." },
  ];
  for (const step of phaseSequence) {
    const res = await driver.request(
      {
        type: "setAgentChatTestFixture",
        phase: step.phase,
        userText: "quick ai render budget probe",
        assistantText: step.assistantText,
      },
      { expect: "externalCommandResult", timeoutMs: 12_000 },
    );
    if (res.ok === false || res.success === false) {
      checks.phaseSequenceError = { step, res };
    }
    await Bun.sleep(250);
  }
  await driver.waitForSettle();
  const afterStream = await readCounters(driver);
  const streamCpu = await processCpuMs(driver.pid);
  receipt.performance = {
    activeStream: derivePerf(null, afterStream, 2_000, streamCpu),
  };

  const state = await driver
    .request(
      { type: "getAgentChatState" },
      { expect: "agent_chatStateResult", timeoutMs: 10_000 },
    )
    .catch((error) => ({ error: String(error) }));
  checks.agentChatState = {
    messageCount: state.messageCount,
    variant: state.uiVariant ?? state.variant,
    sessionPolicy: state.sessionPolicy ?? state.policy,
    finalAssistantText: state.lastAssistantText ?? state.finalAssistantText,
    ok: typeof state.messageCount === "number" && state.messageCount > 0,
  };

  // Quiet-idle phase: after settle, nothing should churn.
  const idleStart = Bun.nanoseconds();
  const beforeIdle = await readCounters(driver);
  await Bun.sleep(2_000);
  const afterIdle = await readCounters(driver);
  const idleCpu = await processCpuMs(driver.pid);
  const idleWallMs = (Bun.nanoseconds() - idleStart) / 1e6;
  const frameIdleDelta =
    Number(afterIdle?.frame_count ?? 0) - Number(beforeIdle?.frame_count ?? 0);
  receipt.quietIdle = {
    frameCountDelta: frameIdleDelta,
    transcriptRendersDelta:
      Number(afterIdle?.transcript_render_calls ?? 0) -
      Number(beforeIdle?.transcript_render_calls ?? 0),
    expectedIdleFrameBound: 4,
    perf: derivePerf(beforeIdle, afterIdle, idleWallMs, idleCpu),
  };
  checks.quietIdleBounded = { frameCountDelta: frameIdleDelta, ok: frameIdleDelta <= 4 };

  const counters = afterIdle;
  receipt.counters = counters;
  checks.countersReadable = { ok: counters !== null };
  checks.renderPathExercised = {
    transcript_render_calls: counters?.transcript_render_calls ?? 0,
    transcript_rows_scanned: counters?.transcript_rows_scanned ?? 0,
    list_visible_row_passes: counters?.list_visible_row_passes ?? 0,
    text_full_parses: counters?.text_full_parses ?? 0,
    frame_count: counters?.frame_count ?? 0,
    ok:
      (counters?.transcript_render_calls ?? 0) > 0 &&
      (counters?.transcript_rows_scanned ?? 0) > 0 &&
      (counters?.list_visible_row_passes ?? 0) > 0 &&
      (counters?.text_full_parses ?? 0) > 0 &&
      (counters?.frame_count ?? 0) > 0,
  };
} finally {
  const names = Object.keys(checks);
  const passed = names.filter((n) => checks[n]?.ok === true);
  receipt.summary = { passed: passed.length, total: names.length };
  await Bun.write(receiptPath, JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify(receipt, null, 2));
  await driver.close();
}
