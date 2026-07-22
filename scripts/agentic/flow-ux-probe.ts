#!/usr/bin/env bun
/**
 * Red/green receipts for the Flow Desk + Threadline sessions (2026-07-09).
 * Every flow is an agent identity; Enter means CONVERSE on Script Kit's own
 * ChatPrompt surface — no engine TUI is ever wrapped.
 *
 * Runs against deterministic fixtures, zero tokens:
 *  - fixtures/flow-ux-project: project flows discovered via `md roster`
 *    (real mdflow + fake engines on PATH).
 *  - fixtures/flow-desk-package: fake @johnlindquist/flows package with a
 *    codex-engine flow and a fasteng-engine flow.
 *  - fixtures/flow-desk-package/bin/fake-codex: deterministic
 *    `codex app-server` (SCRIPT_KIT_CODEX_BIN seam) that echoes each turn's
 *    prompt back as "FAKE-CODEX-REPLY: …".
 *
 * Receipt matrix:
 *  1. deskOpens — single "Flows" built-in opens the desk, corpus ready.
 *  2. packageProvenance — package flow row shows purpose + origin.
 *  3. enterConverses — Enter opens a Threadline session (chat surface, no
 *     auto-message from a name lookup, codexThread transport, honest idle).
 *  4. firstTurnRoundTrip — submitted message streams back; the reply echo
 *     proves the prompt carried the flow's MISSION + the user text.
 *  5. secondTurnRawMessage — second turn commits (thread holds context).
 *  6. escapeBackgrounds — Esc returns to the desk; session + turns survive.
 *  7. reentrySameSession — re-entering restores the SAME session id and
 *     transcript (no respawn).
 *  8. mdflowTransport — non-codex engine converses via --_task/--events
 *     turns on the same chat surface.
 *  9. cmdKActions — ⌘K in a session shows session verbs.
 * 10. runOnceBackground — ⇧↵ registry run, no session created.
 * 11. cleanupHidden — app left hidden.
 */
import { join } from "node:path";
import { Driver } from "../devtools/driver.ts";

const FIXTURE = join(import.meta.dir, "fixtures/flow-ux-project");
const PACKAGE_FIXTURE = join(import.meta.dir, "fixtures/flow-desk-package");
const SHOTS = join(import.meta.dir, "../../.test-screenshots");

const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/flow-ux/script-kit-gpui";

const driver = await Driver.launch({
  binary,
  sessionName: `flow-desk-probe-${process.pid}`,
  sandboxHome: true,
  env: {
    SCRIPT_KIT_FLOW_UX_CWD: FIXTURE,
    SCRIPT_KIT_FLOWS_PACKAGE_DIR: PACKAGE_FIXTURE,
    SCRIPT_KIT_FLOWS_BIN_DIR: join(PACKAGE_FIXTURE, "bin"),
    SCRIPT_KIT_CODEX_BIN: join(PACKAGE_FIXTURE, "bin/fake-codex"),
    // WP5/C-R8: arm the hot-path counters so the additive quiet-idle phase
    // (check 12) can read the Flow tick / invalidation / surface-render
    // snapshot. Zero-cost when unset, so this never perturbs the existing 11
    // receipts.
    SCRIPT_KIT_CHAT_HOT_COUNTERS: "1",
    PATH: `${join(FIXTURE, "bin")}:${join(PACKAGE_FIXTURE, "bin")}:${process.env.PATH ?? ""}`,
  },
});

// -- WP5 hot-counter reader (additive) -------------------------------------
// The app emits one consolidated `chat_hot_counters` line (target
// script_kit::chat_hot) each Flow tick (throttled) and at turn settle. Parse
// the LATEST line's `key=value` pairs = cumulative process totals.
// WP-B3 semantic counter set + per-frame draw timing.
const COUNTER_KEYS = [
  "flow_tick_wakes",
  "flow_render_requests",
  "flow_desk_render_calls",
  "flow_session_render_calls",
  "flow_events_received",
  "flow_events_effective",
  "flow_child_commits",
  "flow_child_bytes_committed",
  "flow_sessions_scanned",
  "flow_stdout_bytes_copied",
  "list_all_row_passes",
  "list_visible_row_passes",
  "text_full_parses",
  "text_append_parses",
  "text_full_parse_bytes",
  "text_append_parse_bytes",
  "frame_count",
  "frame_draw_busy_us_total",
  "frame_max_us",
  "frame_p95_us",
  "frames_over_33ms",
];
const readFlowCounters = async (): Promise<Json | null> => {
  const result = (await driver.getLogs(
    { target: "script_kit::chat_hot", limit: 50 },
    { timeoutMs: 8_000 },
  )) as Json;
  const entries = (result.entries as Json[]) ?? [];
  for (let i = entries.length - 1; i >= 0; i--) {
    const message = String(entries[i]?.message ?? "");
    if (!message.includes("chat_hot_counters")) continue;
    const counters: Json = {};
    let matched = 0;
    for (const key of COUNTER_KEYS) {
      const m = message.match(new RegExp(`\\b${key}=(\\d+)\\b`));
      if (m) {
        counters[key] = Number.parseInt(m[1], 10);
        matched += 1;
      }
    }
    if (matched > 0) return counters;
  }
  return null;
};
const quietIdleSeconds = (() => {
  const idx = process.argv.indexOf("--quiet-idle-seconds");
  const raw = idx >= 0 ? process.argv[idx + 1] : undefined;
  const parsed = raw ? Number.parseInt(raw, 10) : NaN;
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 2;
})();

type Json = Record<string, any>;
const receipt: Json = { binary, fixture: FIXTURE, packageFixture: PACKAGE_FIXTURE, checks: {} };
const checks = receipt.checks as Json;
let shotIndex = 0;

const flowUx = (state: Json): Json | null => (state?.flowUx as Json) ?? null;
const lastSession = (state: Json): Json | undefined =>
  ((flowUx(state)?.sessions as Json[]) ?? []).at(-1);

const pollState = async (
  pred: (state: Json) => boolean,
  timeoutMs = 8_000,
): Promise<Json> => {
  const deadline = Date.now() + timeoutMs;
  let state: Json = {};
  while (Date.now() < deadline) {
    state = (await driver.getState()) as Json;
    if (pred(state)) return state;
    await Bun.sleep(100);
  }
  return state;
};

const shot = async (name: string) => {
  shotIndex += 1;
  await driver.captureScreenshot({
    savePath: join(SHOTS, `flow-desk-${String(shotIndex).padStart(2, "0")}-${name}.png`),
  });
};

// Real-dispatch key press through the GPUI window (the only path that hits
// element-level on_key_down handlers).
const pressMain = (key: string, modifiers: string[] = []) =>
  driver
    .simulateGpuiEvent(
      { type: "keyDown", key, modifiers },
      { target: { type: "main" }, timeoutMs: 5_000 },
    )
    .catch((error) => ({ error: String(error) }));

// Escape until the app rests on the ScriptList root with an empty filter.
// NEVER escapes when already at root — a root escape hides the window.
const returnToRoot = async () => {
  for (let i = 0; i < 10; i++) {
    const st = (await driver.getState()) as Json;
    if (st.windowVisible === false) {
      driver.send({ type: "show" });
      await Bun.sleep(300);
      continue;
    }
    if (st.promptType === "none" && !st.inputValue) return;
    await pressMain("escape");
    await Bun.sleep(250);
  }
};

const visibleTexts = async (): Promise<string> => {
  const result = (await driver.getElements({ limit: 200 })) as Json;
  return ((result.elements as Json[]) ?? [])
    .map((e) => `${e.text ?? ""}|${e.value ?? ""}|${e.label ?? ""}`)
    .join("\n");
};

// Send one chat message in the open flow session: seed the composer via
// protocol setInput (routed to ChatPrompt), then real-dispatch Enter.
const sendChatMessage = async (text: string) => {
  driver.send({
    type: "batch",
    requestId: `flow-chat-${Date.now()}`,
    commands: [{ type: "setInput", text }],
  });
  await Bun.sleep(250);
  await pressMain("enter");
};

const openDesk = async () => {
  await returnToRoot();
  await driver.setFilterAndWait("Flows");
  await pressMain("enter");
  await Bun.sleep(400);
};

const deskFilter = async (text: string) => {
  await driver.setFilterAndWait(text);
  await Bun.sleep(300);
};

try {
  driver.send({ type: "show" });
  await driver.waitForSettle();

  // -- 1. Desk entry + combined corpus ------------------------------------
  await driver.setFilterAndWait("Flows");
  await Bun.sleep(300);
  const flowsEntryVisible = (await visibleTexts())
    .split("\n")
    .some((line) => line.startsWith("Flows|"));
  await openDesk();
  let state = await pollState(
    (s) => flowUx(s)?.activeVariant === "flash" && flowUx(s)?.roster?.status === "ready",
  );
  let fx = flowUx(state);
  checks.deskOpens = {
    flowsEntryVisible,
    activeVariant: fx?.activeVariant,
    rosterStatus: fx?.roster?.status,
    rosterCount: fx?.roster?.count,
    ok:
      flowsEntryVisible &&
      fx?.activeVariant === "flash" &&
      fx?.roster?.status === "ready" &&
      (fx?.roster?.count ?? 0) >= 6,
  };
  await shot("desk-corpus");

  // -- 2. Package provenance ----------------------------------------------
  await deskFilter("hello-codex");
  state = (await driver.getState()) as Json;
  fx = flowUx(state);
  const deskTexts = await visibleTexts();
  checks.packageProvenance = {
    selectedFlowId: fx?.selectedFlowId,
    rowShowsFriendlyName: deskTexts.includes("Hello Codex"),
    rowShowsOrigin: deskTexts.includes("@johnlindquist/flows"),
    ok:
      fx?.selectedFlowId === "package:flow-hello-codex" &&
      deskTexts.includes("Hello Codex") &&
      deskTexts.includes("@johnlindquist/flows"),
  };
  await shot("package-provenance");

  // -- 3. Enter = converse: Threadline session, honest idle ---------------
  await pressMain("enter");
  state = await pollState(
    (s) => s.promptType === "flowSession" && Boolean(lastSession(s)),
  );
  fx = flowUx(state);
  const session = lastSession(state);
  const sessionElements = await visibleTexts();
  checks.enterConverses = {
    promptType: state.promptType,
    flowId: session?.flowId,
    transport: session?.transport,
    state: session?.state,
    turns: session?.turns,
    turnInFlight: session?.turnInFlight,
    hasChatComposer: sessionElements.includes("chat-input") || sessionElements.includes("Message"),
    ok:
      state.promptType === "flowSession" &&
      session?.flowId === "package:flow-hello-codex" &&
      session?.transport === "codexThread" &&
      session?.state === "needs you" &&
      session?.turns === 0 &&
      session?.turnInFlight === false,
  };
  await shot("threadline-open");

  // -- 4. First turn: mission + message round trip -------------------------
  await sendChatMessage("what is the answer");
  state = await pollState((s) => lastSession(s)?.turns === 1, 10_000);
  const afterFirst = lastSession(state);
  const transcript = await visibleTexts();
  checks.firstTurnRoundTrip = {
    turns: afterFirst?.turns,
    state: afterFirst?.state,
    replyEchoed: transcript.includes("FAKE-CODEX-REPLY"),
    missionInPrompt: transcript.includes("You are Hello Codex"),
    taskInPrompt: transcript.includes("what is the answer"),
    ok:
      afterFirst?.turns === 1 &&
      afterFirst?.state === "needs you" &&
      transcript.includes("FAKE-CODEX-REPLY") &&
      transcript.includes("You are Hello Codex") &&
      transcript.includes("what is the answer"),
  };
  await shot("first-turn-reply");

  // -- 5. Second turn: raw message (thread holds context) ------------------
  await sendChatMessage("and a follow up");
  state = await pollState((s) => lastSession(s)?.turns === 2, 10_000);
  const afterSecond = lastSession(state);
  checks.secondTurnRawMessage = {
    turns: afterSecond?.turns,
    state: afterSecond?.state,
    ok: afterSecond?.turns === 2 && afterSecond?.state === "needs you",
  };
  await shot("second-turn");

  // -- 6. Esc backgrounds: desk returns, session + transcript survive ------
  await pressMain("escape");
  state = await pollState((s) => flowUx(s)?.activeVariant === "flash", 5_000);
  fx = flowUx(state);
  const bgSession = lastSession(state);
  checks.escapeBackgrounds = {
    activeVariant: fx?.activeVariant,
    live: bgSession?.live,
    turns: bgSession?.turns,
    ok: fx?.activeVariant === "flash" && bgSession?.live === true && bgSession?.turns === 2,
  };
  await shot("backgrounded-desk");

  // -- 7. Active row re-entry: SAME session, transcript intact -------------
  const sessionId = bgSession?.sessionId;
  await deskFilter("");
  await pressMain("enter"); // sessions sort first: selection 0 is the live row
  state = await pollState((s) => s.promptType === "flowSession");
  const reSession = lastSession(state);
  checks.reentrySameSession = {
    sessionId,
    reenteredId: reSession?.sessionId,
    sessionCount: ((flowUx(state)?.sessions as Json[]) ?? []).length,
    turns: reSession?.turns,
    ok:
      state.promptType === "flowSession" &&
      typeof sessionId === "number" &&
      reSession?.sessionId === sessionId &&
      reSession?.turns === 2 &&
      ((flowUx(state)?.sessions as Json[]) ?? []).length === 1,
  };
  await shot("reentered-session");

  // -- 8. mdflow transport: non-codex engine converses too ------------------
  await pressMain("escape");
  await pollState((s) => flowUx(s)?.activeVariant === "flash", 5_000);
  await deskFilter("hello-agent");
  await pressMain("enter");
  state = await pollState(
    (s) => s.promptType === "flowSession" && lastSession(s)?.flowId === "package:flow-hello-agent",
  );
  await sendChatMessage("hello task words");
  state = await pollState(
    (s) => lastSession(s)?.flowId === "package:flow-hello-agent" && lastSession(s)?.turns === 1,
    20_000,
  );
  const mdSession = lastSession(state);
  const mdTranscript = await visibleTexts();
  checks.mdflowTransport = {
    transport: mdSession?.transport,
    turns: mdSession?.turns,
    state: mdSession?.state,
    engineReplyVisible: mdTranscript.includes("FASTENG_OK"),
    taskReached: mdTranscript.includes("hello task words"),
    ok:
      mdSession?.transport === "mdflowTurns" &&
      mdSession?.turns === 1 &&
      mdSession?.state === "needs you" &&
      mdTranscript.includes("FASTENG_OK") &&
      mdTranscript.includes("hello task words"),
  };
  await shot("mdflow-transport");

  // -- 9. ⌘K in a session: session verbs -----------------------------------
  await pressMain("k", ["cmd"]);
  state = await pollState((s) => Boolean(s.actionsDialog), 5_000);
  const actionIds = ((state.actionsDialog?.visibleActions as Json[]) ?? [])
    .map((a) => a.id ?? a.title ?? "")
    .join(", ");
  checks.cmdKActions = {
    actionsOpened: Boolean(state.actionsDialog),
    actionIds,
    ok:
      Boolean(state.actionsDialog) &&
      actionIds.includes("flow_desk_session_copy_transcript") &&
      actionIds.includes("flow_desk_session_stop"),
  };
  await shot("cmdk-actions");
  // Close by toggling ⌘K again — the detached actions window owns real
  // Escape presses; a main-window-targeted Escape would background the
  // session underneath instead of closing the dialog.
  if (state.actionsDialog) {
    await pressMain("k", ["cmd"]);
    await pollState((s) => !s.actionsDialog, 5_000);
  }

  // -- 10. Run once (⇧↵) on a project flow: registry, not a session --------
  // ⌘⇧D backgrounds via the app-level interceptor — robust to whatever
  // focus state the detached actions window left behind.
  await pressMain("d", ["cmd", "shift"]);
  state = await pollState((s) => flowUx(s)?.activeVariant === "flash", 5_000);
  if (flowUx(state)?.activeVariant !== "flash") {
    await pressMain("escape");
    state = await pollState((s) => flowUx(s)?.activeVariant === "flash", 5_000);
  }
  checks.cmdKActions.deskReturned = flowUx(state)?.activeVariant === "flash";
  await deskFilter("fast-success");
  const sessionsBefore = ((flowUx((await driver.getState()) as Json)?.sessions as Json[]) ?? []).length;
  await pressMain("enter", ["shift"]);
  state = await pollState((s) =>
    ((flowUx(s)?.runs as Json[]) ?? []).some(
      (r) => r.flowId === "project:fast-success.fasteng" && r.phase === "Succeeded",
    ),
  );
  fx = flowUx(state);
  const runOnce = ((fx?.runs as Json[]) ?? []).findLast(
    (r) => r.flowId === "project:fast-success.fasteng",
  );
  checks.runOnceBackground = {
    phase: runOnce?.phase,
    sessionsBefore,
    sessionsAfter: ((fx?.sessions as Json[]) ?? []).length,
    ok:
      runOnce?.phase === "Succeeded" &&
      ((fx?.sessions as Json[]) ?? []).length === sessionsBefore,
  };
  await shot("run-once-succeeded");

  // -- 12. WP-B3 quiet-idle counters (ADDITIVE) ----------------------------
  // Re-enter the live session, then sit idle. Baseline the Flow hot counters,
  // wait `quietIdleSeconds`, and report BOTH the render REQUESTS (tick
  // `cx.notify()` invalidations, which a session forces every wake) and the
  // ACTUAL render calls (desk + session), split per WP-B3. An idle session
  // should keep requesting (tick forces dirty) but NOT paint more than a bound
  // of actual frames. This is the reading WP9 must later drive to ~0 requests
  // and a parked tick; here it proves the split is real and readable, with an
  // explicit expected bound on actual renders/frames.
  await deskFilter("");
  await pressMain("enter"); // live session sorts first
  await pollState((s) => s.promptType === "flowSession", 5_000);
  const idleBefore = await readFlowCounters();
  const idleWallStart = Bun.nanoseconds();
  await Bun.sleep(quietIdleSeconds * 1000);
  const idleAfter = await readFlowCounters();
  const idleWallMs = (Bun.nanoseconds() - idleWallStart) / 1e6;
  const delta = (key: string): number | null =>
    idleBefore && idleAfter
      ? (idleAfter[key] ?? 0) - (idleBefore[key] ?? 0)
      : null;
  const actualRenderDelta =
    (delta("flow_desk_render_calls") ?? 0) + (delta("flow_session_render_calls") ?? 0);
  const frameDelta = delta("frame_count") ?? 0;
  const drawBusyMs = (delta("frame_draw_busy_us_total") ?? 0) / 1000;
  // Bound: an idle Flow session may repaint on the ~120ms tick, so allow up to
  // ~1.5x the tick rate over the window plus slack; renders must not exceed this.
  const expectedRenderBound = Math.ceil((quietIdleSeconds * 1000) / 120) * 2 + 4;
  checks.quietIdleCounters = {
    quietIdleSeconds,
    countersReadable: idleBefore !== null && idleAfter !== null,
    before: idleBefore,
    after: idleAfter,
    tickWakesDelta: delta("flow_tick_wakes"),
    renderRequestsDelta: delta("flow_render_requests"),
    actualRenderCallsDelta: actualRenderDelta,
    frameCountDelta: frameDelta,
    expectedRenderBound,
    performance: {
      draw_busy_ms: Number(drawBusyMs.toFixed(3)),
      observation_wall_ms: Number(idleWallMs.toFixed(1)),
      draw_share: Number((idleWallMs > 0 ? drawBusyMs / idleWallMs : 0).toFixed(4)),
      frame_p95_ms: Number((Number(idleAfter?.frame_p95_us ?? 0) / 1000).toFixed(3)),
      frame_max_ms: Number((Number(idleAfter?.frame_max_us ?? 0) / 1000).toFixed(3)),
      frames_over_33ms: delta("frames_over_33ms"),
    },
    // Readable, split reported, AND actual renders within the explicit bound.
    ok:
      idleBefore !== null &&
      idleAfter !== null &&
      actualRenderDelta <= expectedRenderBound,
  };
  await shot("quiet-idle-counters");

  // -- 11. Cleanup: leave the app hidden ------------------------------------
  await returnToRoot();
  await pressMain("escape"); // root escape hides
  await Bun.sleep(400);
  state = (await driver.getState()) as Json;
  checks.cleanupHidden = {
    windowVisible: state.windowVisible,
    ok: state.windowVisible === false,
  };
} finally {
  const names = Object.keys(checks);
  const passed = names.filter((n) => checks[n]?.ok === true);
  receipt.summary = { passed: passed.length, total: names.length };
  console.log(JSON.stringify(receipt, null, 2));
  await driver.close();
}
