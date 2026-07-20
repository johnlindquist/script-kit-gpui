#!/usr/bin/env bun
import { join, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver";

const phases = [
  "waitingNoAssistant",
  "emptyAssistant",
  "firstToken",
  "multiTokenStreaming",
  "completed",
  "terminalEmpty",
  "error",
] as const;

const repoRoot = resolve(import.meta.dir, "../..");
const flowFixture = join(import.meta.dir, "fixtures/flow-ux-project");
const packageFixture = join(import.meta.dir, "fixtures/flow-desk-package");
const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ??
  "target-agent/artifacts/agent-chat-geometry-timeline/script-kit-gpui";
const longUserText = Array.from(
  { length: 90 },
  (_, index) => `Stable user row line ${String(index + 1).padStart(2, "0")}: alpha beta gamma delta epsilon.`,
).join("\n");
const assistantText = "First token plus several deterministic streaming tokens.";

type Surface = "agentChat" | "flowSession";
type Mode = "followTail" | "manualScroll";

const receipt: Json = {
  schemaVersion: 1,
  tool: "agent-chat-flow-session-geometry-timeline-probe",
  binary,
  phases,
  surfaces: {},
  assertions: [],
  failures: [],
  cleanup: {},
};

const driver = await Driver.launch({
  binary,
  sessionName: `agent-chat-geometry-timeline-${process.pid}`,
  sandboxHome: true,
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 12_000,
  env: {
    SCRIPT_KIT_FLOW_UX_CWD: flowFixture,
    SCRIPT_KIT_FLOWS_PACKAGE_DIR: packageFixture,
    SCRIPT_KIT_FLOWS_BIN_DIR: join(packageFixture, "bin"),
    SCRIPT_KIT_CODEX_BIN: join(packageFixture, "bin/fake-codex"),
    PATH: `${join(flowFixture, "bin")}:${join(packageFixture, "bin")}:${process.env.PATH ?? ""}`,
  },
});

function expect(name: string, pass: boolean, detail: Json = {}) {
  receipt.assertions.push({ name, pass, ...detail });
  if (!pass) receipt.failures.push({ name, ...detail });
}

function component(layout: Json, name: string): Json | null {
  const matches = ((layout.components as Json[]) ?? []).filter((entry) => entry.name === name);
  return matches.length === 1 ? matches[0] : null;
}

function boundsOf(layout: Json, names: string[]): Json | null {
  for (const name of names) {
    const bounds = component(layout, name)?.bounds;
    if (bounds && typeof bounds === "object") return bounds as Json;
  }
  return null;
}

function delta(a: Json | null, b: Json | null, edge: "top" | "bottom") {
  if (!a || !b) return Number.POSITIVE_INFINITY;
  const ay = Number(a.y ?? 0) + (edge === "bottom" ? Number(a.height ?? 0) : 0);
  const by = Number(b.y ?? 0) + (edge === "bottom" ? Number(b.height ?? 0) : 0);
  return Math.abs(ay - by);
}

async function setPhase(phase: string) {
  return driver.request(
    {
      type: "setAgentChatTestFixture",
      phase,
      userText: longUserText,
      assistantText: phase === "error" ? "Fixture provider error" : assistantText,
    },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
}

async function geometry(surface: Surface): Promise<Json> {
  if (surface === "agentChat") {
    const result = await driver.request(
      { type: "getAgentChatState" },
      { expect: "agent_chatStateResult", timeoutMs: 12_000 },
    );
    const state = (result.state ?? result) as Json;
    const scroll = (state.transcriptScroll ?? {}) as Json;
    return {
      stableSemanticIdentity: scroll.rowSemanticIds ?? [],
      transcriptViewport: {
        heightPx: scroll.viewportHeightPx,
        bounds: null,
      },
      contentHeightPx: scroll.contentHeightPx,
      scrollAnchor: {
        itemIx: scroll.scrollTopItem,
        offsetPx: scroll.scrollTopOffsetPx,
        scrollTopPx: scroll.scrollTopPx,
      },
      followTail: scroll.followTail,
      manualScroll: scroll.manualScroll,
      selectedTextState: {
        composerHasSelection: state.hasSelection,
        selectionRange: state.selectionRange ?? null,
      },
      pendingIndicatorCount: scroll.pendingIndicatorCount,
      streamingCopyAvailable: scroll.streamingCopyAvailable,
      status: state.status,
    };
  }
  const state = await driver.getState({ timeoutMs: 12_000 });
  return (((state.flowUx ?? {}) as Json).activeTranscript ?? {}) as Json;
}

async function sample(surface: Surface, phase: string, mode: Mode): Promise<Json> {
  const mutation = await setPhase(phase);
  await Bun.sleep(120);
  const layout = await driver.getLayoutInfo({}, { timeoutMs: 12_000 });
  const stateGeometry = await geometry(surface);
  const userNames = surface === "agentChat"
    ? ["agent-chat-transcript-row-user-1"]
    : ["chat-transcript-user-turn-0"];
  const responseNames = surface === "agentChat"
    ? ["agent-chat-transcript-row-assistant-2", "agent-chat-transcript-row-error-2", "agent-chat-transcript-row-assistant-pending"]
    : ["chat-transcript-response-turn-0"];
  const viewportName = surface === "agentChat"
    ? "agent-chat-transcript-viewport"
    : "chat-transcript-viewport";
  const viewportBounds = boundsOf(layout, [viewportName]);
  const scaleFactor = Number(
    ((layout.resolvedTarget as Json | undefined)?.scaleFactor ?? layout.scaleFactor ?? 1),
  );

  return {
    phase,
    mode,
    mutationAccepted: mutation.ok !== false && mutation.success !== false,
    stableSemanticIdentity: stateGeometry.stableSemanticIdentity ?? stateGeometry.rowSemanticIds ?? [],
    userRowBounds: boundsOf(layout, userNames),
    assistantOrPendingBounds: boundsOf(layout, responseNames),
    transcriptViewport: {
      bounds: viewportBounds,
      heightPx: stateGeometry.transcriptViewport?.heightPx ?? stateGeometry.viewportHeightPx,
    },
    contentHeightPx: stateGeometry.contentHeightPx,
    scrollAnchor: stateGeometry.scrollAnchor,
    followTail: stateGeometry.followTail,
    manualScroll: stateGeometry.manualScroll,
    selectedTextState: stateGeometry.selectedTextState ?? stateGeometry.selectedText,
    pendingIndicatorCount: stateGeometry.pendingIndicatorCount,
    streamingCopyAvailable: stateGeometry.streamingCopyAvailable,
    scaleFactor,
    paintFrameGeneration: component(layout, viewportName)?.measurementFrameGeneration ?? null,
  };
}

async function runTimeline(surface: Surface): Promise<Json> {
  const followTail = [];
  for (const phase of phases) followTail.push(await sample(surface, phase, "followTail"));

  await setPhase(phases[0]);
  await Bun.sleep(100);
  await driver.request(
    { type: "setAgentChatTranscriptScroll", itemIx: 0, offsetPx: 24 },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(100);
  const manualScroll = [];
  for (const phase of phases) manualScroll.push(await sample(surface, phase, "manualScroll"));

  const waiting = followTail[0];
  const empty = followTail[1];
  const scale = Math.max(Number(waiting.scaleFactor ?? 1), 1);
  const topDeltaPhysical = delta(waiting.userRowBounds, empty.userRowBounds, "top") * scale;
  const bottomDeltaPhysical = delta(waiting.userRowBounds, empty.userRowBounds, "bottom") * scale;
  expect(`${surface}.user-row-waiting-to-empty-stable`, topDeltaPhysical <= 1 && bottomDeltaPhysical <= 1, {
    topDeltaPhysical,
    bottomDeltaPhysical,
  });
  expect(`${surface}.one-pending-before-first-text`, followTail.every((entry, index) =>
    Number(entry.pendingIndicatorCount) === (index < 2 ? 1 : 0)), {
    counts: followTail.map((entry) => entry.pendingIndicatorCount),
  });
  expect(`${surface}.copy-available-during-visible-streaming`, [2, 3].every((index) =>
    followTail[index].streamingCopyAvailable === true), {
    availability: followTail.map((entry) => entry.streamingCopyAvailable),
  });
  expect(`${surface}.follow-tail-remains-at-tail`, followTail.every((entry) =>
    entry.followTail === true && entry.manualScroll === false), {
    modes: followTail.map((entry) => ({ followTail: entry.followTail, manualScroll: entry.manualScroll })),
  });
  const firstAnchor = manualScroll[0].scrollAnchor as Json;
  const anchorDeltas = manualScroll.map((entry) => {
    const anchor = entry.scrollAnchor as Json;
    return Math.abs(Number(anchor?.offsetPx ?? 0) - Number(firstAnchor?.offsetPx ?? 0)) * scale;
  });
  expect(`${surface}.manual-scroll-anchor-stable`, manualScroll.every((entry, index) =>
    entry.manualScroll === true && entry.followTail === false &&
    Number((entry.scrollAnchor as Json)?.itemIx) === Number(firstAnchor?.itemIx) &&
    anchorDeltas[index] <= 1), { anchorDeltasPhysical: anchorDeltas });

  return { followTail, manualScroll };
}

async function press(key: string) {
  await driver.simulateGpuiEvent({ type: "keyDown", key }, { target: { type: "main" } });
  await Bun.sleep(120);
}

try {
  driver.send({ type: "show" });
  await driver.waitForSettle();
  await driver.request(
    { type: "openAgentChatKitchenSinkFixture" },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(250);
  receipt.surfaces.agentChat = await runTimeline("agentChat");

  await driver.request(
    { type: "agentChatEscape" },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(150);
  driver.send({ type: "show" });
  await driver.waitForSettle();
  await driver.setFilterAndWait("Flows");
  await press("enter");
  await driver.setFilterAndWait("hello-codex");
  await press("enter");
  let flowState = await driver.getState();
  for (let attempt = 0; attempt < 40 && flowState.promptType !== "flowSession"; attempt += 1) {
    await Bun.sleep(100);
    flowState = await driver.getState();
  }
  expect("flowSession.opened", flowState.promptType === "flowSession", {
    promptType: flowState.promptType,
  });
  receipt.surfaces.flowSession = await runTimeline("flowSession");

  for (let attempt = 0; attempt < 4; attempt += 1) {
    const state = await driver.getState();
    if (state.windowVisible === false) break;
    await press("escape");
  }
  const finalState = await driver.getState();
  receipt.cleanup.finalWindowHidden = finalState.windowVisible === false;
  expect("cleanup.final-window-hidden", finalState.windowVisible === false, {
    windowVisible: finalState.windowVisible,
    promptType: finalState.promptType,
  });
} finally {
  await driver.close();
  receipt.cleanup.driverClosed = true;
  receipt.cleanup.sessionClosed = true;
  receipt.cleanup.pid = driver.pid;
}

receipt.pass = receipt.failures.length === 0;
console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.pass ? 0 : 1);
