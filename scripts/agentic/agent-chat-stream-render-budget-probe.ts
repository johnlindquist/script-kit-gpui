#!/usr/bin/env bun
/**
 * WP-B3 render-budget + frame-timing receipt for the Agent Chat transcript.
 *
 * Launches the app with `SCRIPT_KIT_CHAT_HOT_COUNTERS=1` (the env gate for the
 * WP-B3 hot-path counters + per-frame draw timing), drives the deterministic
 * Agent Chat kitchen-sink fixture through structured phases (startup →
 * active-stream → scroll → settle → quiet-idle), reads scoped counter snapshots
 * at each phase boundary from the app's in-process log ring
 * (`getLogs({ target: "script_kit::chat_hot" })`), and derives real performance
 * metrics (frame p95 / max / over-33ms, draw_share, process CPU).
 *
 * It also runs an instrumentation-overhead CONTROL: the identical scenario with
 * the counters gate OFF, so the receipt reports the p95 / draw-share delta the
 * instrumentation itself costs.
 *
 * Zero tokens: no real backend is contacted — the fixture pushes messages
 * directly, exercising set_messages → reconcile → render → list layout →
 * markdown parse → frame draw. A RED baseline is expected and fine; the point is
 * that every number is real and machine-checkable.
 *
 * Usage:
 *   bun scripts/agentic/agent-chat-stream-render-budget-probe.ts \
 *     [--message-count 60] [--receipt /tmp/agent-chat-stream-render-budget.json] \
 *     [--no-overhead-control]
 */
import { Driver } from "../devtools/driver.ts";

type Json = Record<string, any>;

const argOf = (name: string, fallback: string): string => {
  const flag = `--${name}`;
  const idx = process.argv.indexOf(flag);
  return idx >= 0 && process.argv[idx + 1] ? process.argv[idx + 1] : fallback;
};
const hasFlag = (name: string): boolean => process.argv.includes(`--${name}`);

const messageCount = Number.parseInt(argOf("message-count", "60"), 10);
const receiptPath = argOf("receipt", "/tmp/agent-chat-stream-render-budget.json");
const runOverheadControl = !hasFlag("no-overhead-control");
const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/wp5-proof/script-kit-gpui";

// The consolidated counter line carries every counter as `key=value` pairs.
// WP-B3 semantic counter set (parse the LATEST line = cumulative totals).
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

const delta = (after: Json | null, before: Json | null, key: string): number =>
  Number(after?.[key] ?? 0) - Number(before?.[key] ?? 0);

// Process CPU (ms) for a pid via `ps` (utime+stime as reported by the OS).
const processCpuMs = async (pid: number): Promise<number> => {
  try {
    const proc = Bun.spawn(["ps", "-o", "time=", "-p", String(pid)], {
      stdout: "pipe",
    });
    const out = (await new Response(proc.stdout).text()).trim();
    // Format: [[DD-]HH:]MM:SS(.ss) — parse to milliseconds.
    const parts = out.replace("-", ":").split(":").map((p) => Number.parseFloat(p));
    let seconds = 0;
    for (const p of parts) seconds = seconds * 60 + (Number.isFinite(p) ? p : 0);
    return Math.round(seconds * 1000);
  } catch {
    return 0;
  }
};

/** Derive the machine-checkable frame/CPU performance metrics for a window. */
const derivePerf = (
  before: Json | null,
  after: Json | null,
  observationWallMs: number,
  cpuMs: number,
): Json => {
  const frameCount = delta(after, before, "frame_count");
  const drawBusyMs = delta(after, before, "frame_draw_busy_us_total") / 1000;
  const frameMaxMs = Number(after?.frame_max_us ?? 0) / 1000;
  const frameP95Ms = Number(after?.frame_p95_us ?? 0) / 1000;
  const framesOver33 = delta(after, before, "frames_over_33ms");
  const drawShare = observationWallMs > 0 ? drawBusyMs / observationWallMs : 0;
  const processCpuPercent =
    observationWallMs > 0 ? (cpuMs / observationWallMs) * 100 : 0;
  return {
    frame_count: frameCount,
    frame_max_ms: Number(frameMaxMs.toFixed(3)),
    frame_p95_ms: Number(frameP95Ms.toFixed(3)),
    frames_over_33ms: framesOver33,
    draw_busy_ms: Number(drawBusyMs.toFixed(3)),
    observation_wall_ms: Number(observationWallMs.toFixed(1)),
    draw_share: Number(drawShare.toFixed(4)),
    process_cpu_ms: cpuMs,
    process_cpu_percent: Number(processCpuPercent.toFixed(1)),
  };
};

const phaseSequence: Array<{ phase: string; assistantText?: string }> = [
  { phase: "awaitingFirstAssistantText" },
  { phase: "assistantText", assistantText: "The" },
  { phase: "assistantText", assistantText: "The answer is **42**." },
  {
    phase: "assistantText",
    assistantText:
      "The answer is **42**.\n\n## Details\n\n- item one\n- item two\n\n```rust\nfn main() {}\n```\n\nSee [docs](https://example.com).",
  },
  { phase: "idle", assistantText: "Final settled assistant answer." },
];

const FINAL_TRANSCRIPT_ASSISTANT = "Heavy assistant row with enough markdown to parse.";

/**
 * Drive the full scenario against one launched app and return the phase-scoped
 * snapshots + derived metrics. `countersOn` distinguishes the measured run from
 * the overhead control.
 */
const runScenario = async (countersOn: boolean): Promise<Json> => {
  const env: Json = {
    SCRIPT_KIT_AGENT_CHAT_RENDER_TRACE: "1",
    SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
  };
  if (countersOn) env.SCRIPT_KIT_CHAT_HOT_COUNTERS = "1";

  const driver = await Driver.launch({
    binary,
    sandboxHome: true,
    sessionName: `agent-chat-stream-${countersOn ? "on" : "control"}-${process.pid}`,
    defaultTimeoutMs: 10_000,
    env,
  });

  const out: Json = { countersOn, phases: {}, checks: {} };
  try {
    out.target = { pid: driver.pid, sessionDir: driver.sessionDir };
    driver.send({ type: "show" });
    await driver.waitForSettle();

    // --- Phase: startup --------------------------------------------------
    const startupStart = Bun.nanoseconds();
    const opened = await driver.request(
      { type: "openAgentChatKitchenSinkFixture" },
      { expect: "externalCommandResult", timeoutMs: 10_000 },
    );
    out.checks.fixtureOpened = { ok: opened.ok !== false && opened.success !== false };
    await driver.waitForSettle();
    await Bun.sleep(400);
    const afterStartup = await readCounters(driver);
    const startupCpu = await processCpuMs(driver.pid);
    out.phases.startup = {
      snapshot: afterStartup,
      perf: derivePerf(null, afterStartup, (Bun.nanoseconds() - startupStart) / 1e6, startupCpu),
    };

    // --- Phase: active-stream -------------------------------------------
    const streamStart = Bun.nanoseconds();
    const streamCpuBefore = await processCpuMs(driver.pid);
    for (const step of phaseSequence) {
      const res = await driver.request(
        {
          type: "setAgentChatTestFixture",
          phase: step.phase,
          userText: "render budget stream probe",
          assistantText: step.assistantText,
        },
        { expect: "externalCommandResult", timeoutMs: 12_000 },
      );
      if (res.ok === false || res.success === false) {
        out.checks.phaseSequenceError = { step, res };
      }
      await Bun.sleep(250);
    }
    await driver.waitForSettle();
    const afterStream = await readCounters(driver);
    const streamCpuAfter = await processCpuMs(driver.pid);
    out.phases.activeStream = {
      snapshot: afterStream,
      perf: derivePerf(
        afterStartup,
        afterStream,
        (Bun.nanoseconds() - streamStart) / 1e6,
        streamCpuAfter - streamCpuBefore,
      ),
    };

    // --- Phase: heavy transcript + scroll -------------------------------
    const scrollStart = Bun.nanoseconds();
    const scrollCpuBefore = await processCpuMs(driver.pid);
    const heavy = await driver.request(
      {
        type: "setAgentChatTestFixture",
        phase: "idle",
        userText: "render budget heavy transcript",
        assistantText: FINAL_TRANSCRIPT_ASSISTANT,
        messageCount,
      },
      { expect: "externalCommandResult", timeoutMs: 20_000 },
    );
    out.checks.heavyTranscript = { ok: heavy.ok !== false && heavy.success !== false };
    await driver.waitForSettle();
    // Scroll the long transcript to force bounded visible-window layout passes.
    for (let i = 0; i < 8; i++) {
      driver.send({ type: "simulateGpuiEvent", event: { kind: "scroll", deltaY: -400 } });
      await Bun.sleep(80);
    }
    for (let i = 0; i < 8; i++) {
      driver.send({ type: "simulateGpuiEvent", event: { kind: "scroll", deltaY: 400 } });
      await Bun.sleep(80);
    }
    await driver.waitForSettle();
    const afterScroll = await readCounters(driver);
    const scrollCpuAfter = await processCpuMs(driver.pid);
    out.phases.scroll = {
      snapshot: afterScroll,
      perf: derivePerf(
        afterStream,
        afterScroll,
        (Bun.nanoseconds() - scrollStart) / 1e6,
        scrollCpuAfter - scrollCpuBefore,
      ),
    };

    // --- Assert exact final transcript ----------------------------------
    const state = await driver
      .request({ type: "getAgentChatState" }, { expect: "agent_chatStateResult", timeoutMs: 10_000 })
      .catch((error) => ({ error: String(error) }));
    out.checks.agentChatState = {
      messageCount: state.messageCount,
      variant: state.uiVariant ?? state.variant,
      sessionPolicy: state.sessionPolicy ?? state.policy,
      finalAssistantText: state.lastAssistantText ?? state.finalAssistantText,
      ok: state.messageCount === messageCount,
    };

    // --- Phase: settle + quiet-idle -------------------------------------
    const idleStart = Bun.nanoseconds();
    const idleCpuBefore = await processCpuMs(driver.pid);
    const beforeIdle = await readCounters(driver);
    await Bun.sleep(2_000); // quiet window: nothing should churn.
    const afterIdle = await readCounters(driver);
    const idleCpuAfter = await processCpuMs(driver.pid);
    const idleWallMs = (Bun.nanoseconds() - idleStart) / 1e6;
    out.phases.quietIdle = {
      snapshot: afterIdle,
      renderRequestsDelta: delta(afterIdle, beforeIdle, "flow_render_requests"),
      transcriptRendersDelta: delta(afterIdle, beforeIdle, "transcript_render_calls"),
      frameCountDelta: delta(afterIdle, beforeIdle, "frame_count"),
      expectedIdleFrameBound: 4,
      perf: derivePerf(beforeIdle, afterIdle, idleWallMs, idleCpuAfter - idleCpuBefore),
    };
    out.checks.quietIdleBounded = {
      frameCountDelta: delta(afterIdle, beforeIdle, "frame_count"),
      ok: delta(afterIdle, beforeIdle, "frame_count") <= 4,
    };

    out.counters = afterIdle;
    out.checks.countersReadable = { ok: afterIdle !== null };
    out.checks.renderPathExercised = {
      transcript_render_calls: afterIdle?.transcript_render_calls ?? 0,
      transcript_rows_scanned: afterIdle?.transcript_rows_scanned ?? 0,
      list_visible_row_passes: afterIdle?.list_visible_row_passes ?? 0,
      text_full_parses: afterIdle?.text_full_parses ?? 0,
      frame_count: afterIdle?.frame_count ?? 0,
      ok:
        (afterIdle?.transcript_render_calls ?? 0) > 0 &&
        (afterIdle?.transcript_rows_scanned ?? 0) > 0 &&
        (afterIdle?.list_visible_row_passes ?? 0) > 0 &&
        (afterIdle?.text_full_parses ?? 0) > 0 &&
        (afterIdle?.frame_count ?? 0) > 0,
    };
  } finally {
    await driver.close();
  }
  return out;
};

const receipt: Json = {
  probe: "agent-chat-stream-render-budget",
  binary,
  messageCount,
  checks: {},
};

try {
  const measured = await runScenario(true);
  receipt.measured = measured;
  receipt.counters = measured.counters;
  receipt.checks = measured.checks;
  receipt.performance = {
    startup: measured.phases.startup?.perf,
    activeStream: measured.phases.activeStream?.perf,
    scroll: measured.phases.scroll?.perf,
    quietIdle: measured.phases.quietIdle?.perf,
  };

  if (runOverheadControl) {
    // Overhead control: identical scenario, counters OFF. Frame timing itself is
    // gated off, so we compare the app's own wall/CPU cost, not frame internals.
    const control = await runScenario(false);
    const on = measured.phases.scroll?.perf ?? {};
    const off = control.phases.scroll?.perf ?? {};
    receipt.overheadControl = {
      scrollPhase: {
        drawShareOn: on.draw_share,
        cpuPercentOn: on.process_cpu_percent,
        cpuPercentOff: off.process_cpu_percent,
        cpuPercentDelta: Number(
          ((on.process_cpu_percent ?? 0) - (off.process_cpu_percent ?? 0)).toFixed(1),
        ),
      },
      note:
        "Counters-off run has no frame internals (gate off); the CPU% delta is the " +
        "instrumentation overhead on the identical scenario.",
    };
  }
} finally {
  const checks = receipt.checks as Json;
  const names = Object.keys(checks);
  const passed = names.filter((n) => checks[n]?.ok === true);
  receipt.summary = { passed: passed.length, total: names.length };
  await Bun.write(receiptPath, JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify(receipt, null, 2));
}
