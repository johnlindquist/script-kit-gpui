#!/usr/bin/env bun
/**
 * Runtime proof for the AI chat parity batch.
 *
 * The premise's headline outcome is "Flow answers can be selected". That is a
 * claim about what the renderer painted, and no Rust test can reach it: the
 * two renderers produce visually similar output, so source reasoning cannot
 * tell selectable text from unselectable text. The vendored `TextView`
 * annotates its paint scope with `{"selectable": bool}`, and that annotation
 * is the only machine-readable channel that distinguishes them.
 *
 * This probe drives both surfaces and reads that annotation back, plus the
 * three ported affordances. Every judgement is delegated to
 * `ai-chat-parity-evidence.ts`, which is unit-tested without an app, so a run
 * that drove nothing reports `absent` rather than "no failures".
 *
 * Usage:
 *   SCRIPT_KIT_AGENT_ARTIFACT_NAME=ai-chat-parity \
 *     ./scripts/agentic/agent-cargo.sh build --bin script-kit-gpui
 *   bun scripts/agentic/ai-chat-parity-probe.ts
 *
 * Exits non-zero on any failed assertion. Run on a quiet machine: this drives
 * real windows, and a contended machine produces interference receipts that
 * must NOT be read as product failures.
 */
import { join, resolve } from "node:path";

import { Driver, type Json } from "../devtools/driver";
import {
  AGENT_CHAT_ANSWER_SCOPE_PREFIX,
  AGENT_CHAT_ANSWER_SCOPE_SUFFIX,
  FLOW_ANSWER_SCOPE_PREFIX,
  FLOW_ANSWER_SCOPE_SUFFIX,
  componentPresent,
  evaluateSelectable,
  jumpPillBehaviour,
  selectionParity,
  type FidelityNode,
  type LayoutComponent,
} from "./ai-chat-parity-evidence";

const repoRoot = resolve(import.meta.dir, "../..");
const flowFixture = join(import.meta.dir, "fixtures/flow-ux-project");
const packageFixture = join(import.meta.dir, "fixtures/flow-desk-package");
const binary =
  process.env.SCRIPT_KIT_GPUI_BINARY ?? "target-agent/artifacts/ai-chat-parity/script-kit-gpui";

const JUMP_PILL_ID = "agent-chat-jump-to-latest";

const receipt: Json = {
  schemaVersion: 1,
  tool: "ai-chat-parity-probe",
  repoRoot,
  binary,
  assertions: [],
  failures: [],
  evidence: {},
  cleanup: {},
};

function expect(name: string, pass: boolean, detail: Json = {}) {
  (receipt.assertions as Json[]).push({ name, pass, ...detail });
  if (!pass) (receipt.failures as Json[]).push({ name, ...detail });
}

function fidelityNodes(layout: Json): FidelityNode[] {
  const info = ((layout.rawLayout as Json)?.info ?? layout) as Json;
  const nodes = ((info.fidelity as Json)?.nodes ?? []) as FidelityNode[];
  return Array.isArray(nodes) ? nodes : [];
}

function components(layout: Json): LayoutComponent[] {
  const info = ((layout.rawLayout as Json)?.info ?? layout) as Json;
  const list = (info.components ?? []) as LayoutComponent[];
  return Array.isArray(list) ? list : [];
}

const driver = await Driver.launch({
  binary,
  sessionName: `ai-chat-parity-${process.pid}`,
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

/**
 * `modifiers` is a LIST of names (`["cmd"]`), not a flags object — see
 * `stdin_commands::KeyModifier`. Passing `{ platform: true }` deserializes
 * into an empty modifier set, so the chord would arrive unmodified and the
 * probe would "pass" a plain `l` keystroke. Always pass the list explicitly.
 */
async function press(key: string, modifiers: string[] = []) {
  await driver.simulateGpuiEvent(
    { type: "keyDown", key, modifiers },
    { target: { type: "main" } },
  );
  await Bun.sleep(140);
}

async function settledTranscript(assistantText: string) {
  await driver.request(
    {
      type: "setAgentChatTestFixture",
      phase: "completed",
      userText: "Explain the difference between the two renderers.",
      assistantText,
    },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(200);
  return driver.getLayoutInfo({}, { timeoutMs: 12_000 });
}

try {
  driver.send({ type: "show" });
  await driver.waitForSettle();

  // ── Agent Chat ────────────────────────────────────────────────────
  await driver.request(
    { type: "openAgentChatKitchenSinkFixture" },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(250);

  const agentChatLayout = await settledTranscript(
    "A settled answer with `code` and a [link](https://example.com).",
  );
  const agentChatSelectable = evaluateSelectable(
    fidelityNodes(agentChatLayout),
    AGENT_CHAT_ANSWER_SCOPE_PREFIX,
    AGENT_CHAT_ANSWER_SCOPE_SUFFIX,
  );
  (receipt.evidence as Json).agentChatSelectable = agentChatSelectable as unknown as Json;

  // Per-turn copy: ported FROM Flow, so a settled assistant row must carry it.
  const copyPresent = components(agentChatLayout).some((component) =>
    String(component?.name ?? "").startsWith("agent-chat-copy-turn-"),
  );
  expect("agentChat.turn-copy-present-on-settled-answer", copyPresent, {
    componentNames: components(agentChatLayout)
      .map((component) => component?.name)
      .filter((name) => String(name ?? "").includes("copy")),
  });

  // Jump to latest: absent at the tail, present after scrolling up. The
  // always-visible failure is the likelier one, so both halves are asserted.
  const followingTail = components(agentChatLayout);
  await driver.request(
    { type: "setAgentChatTranscriptScroll", itemIx: 0, offsetPx: 24 },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(220);
  const scrolledLayout = await driver.getLayoutInfo({}, { timeoutMs: 12_000 });
  const pill = jumpPillBehaviour(followingTail, components(scrolledLayout), JUMP_PILL_ID);
  (receipt.evidence as Json).jumpPill = pill as unknown as Json;
  expect("agentChat.jump-to-latest-follows-tail-state", pill.pass, {
    whileFollowing: pill.whileFollowing,
    afterScroll: pill.afterScroll,
  });

  await driver.request(
    { type: "agentChatEscape" },
    { expect: "externalCommandResult", timeoutMs: 12_000 },
  );
  await Bun.sleep(180);

  // ── Flow session ──────────────────────────────────────────────────
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

  const flowLayout = await settledTranscript(
    "A settled Flow answer with `code` and a [link](https://example.com).",
  );
  const flowSelectable = evaluateSelectable(
    fidelityNodes(flowLayout),
    FLOW_ANSWER_SCOPE_PREFIX,
    FLOW_ANSWER_SCOPE_SUFFIX,
  );
  (receipt.evidence as Json).flowSelectable = flowSelectable as unknown as Json;

  // THE headline claim.
  const parity = selectionParity(flowSelectable, agentChatSelectable);
  expect("parity.both-surfaces-render-selectable-answers", parity.pass, {
    reason: parity.reason,
    flow: flowSelectable as unknown as Json,
    agentChat: agentChatSelectable as unknown as Json,
  });

  // ── ⌘L starts a new conversation ──────────────────────────────────
  const beforeReset = fidelityNodes(flowLayout).filter((node) =>
    String(node?.id ?? "").startsWith(FLOW_ANSWER_SCOPE_PREFIX),
  ).length;
  // Seed a composer draft first. ⌘L must clear the CONVERSATION, not the
  // user's unsent text — that is a deliberate deviation from the plan (it
  // matches Agent Chat), so it needs a check that can actually fail. The
  // draft also makes "did ⌘L type an l into the composer?" answerable by
  // exact comparison rather than a substring guess.
  const draft = "unsent draft kept across the reset";
  await driver.setFilterAndWait(draft);
  await Bun.sleep(140);
  await press("l", ["cmd"]);
  await Bun.sleep(260);
  const afterResetLayout = await driver.getLayoutInfo({}, { timeoutMs: 12_000 });
  const afterReset = fidelityNodes(afterResetLayout).filter((node) =>
    String(node?.id ?? "").startsWith(FLOW_ANSWER_SCOPE_PREFIX),
  ).length;
  (receipt.evidence as Json).newConversation = { beforeReset, afterReset };
  expect(
    "flowSession.cmd-l-clears-the-transcript",
    beforeReset > 0 && afterReset === 0,
    { beforeReset, afterReset },
  );
  // Exact equality covers both halves at once: the draft survived AND ⌘L was
  // consumed rather than typed (an unconsumed ⌘L would append an "l").
  const afterResetState = await driver.getState();
  expect(
    "flowSession.reset-keeps-the-unsent-draft-and-consumes-the-chord",
    afterResetState.filterText === draft,
    { expected: draft, actual: afterResetState.filterText },
  );
  expect(
    "flowSession.still-open-after-reset",
    afterResetState.promptType === "flowSession",
    { promptType: afterResetState.promptType },
  );
  // The transcript viewport must still be mounted: a reset that left the
  // surface without a place to render the next answer would pass every count
  // check above while being unusable.
  expect(
    "flowSession.transcript-viewport-survives-the-reset",
    componentPresent(components(afterResetLayout), "chat-transcript-viewport"),
    {
      componentNames: components(afterResetLayout)
        .map((component) => component?.name)
        .filter((name) => String(name ?? "").startsWith("chat-transcript")),
    },
  );

  for (let attempt = 0; attempt < 4; attempt += 1) {
    const state = await driver.getState();
    if (state.windowVisible === false) break;
    await press("escape");
  }
  const finalState = await driver.getState();
  (receipt.cleanup as Json).finalWindowHidden = finalState.windowVisible === false;
  expect("cleanup.final-window-hidden", finalState.windowVisible === false, {
    windowVisible: finalState.windowVisible,
    promptType: finalState.promptType,
  });
} finally {
  await driver.close();
  (receipt.cleanup as Json).driverClosed = true;
  (receipt.cleanup as Json).pid = driver.pid;
}

receipt.pass = (receipt.failures as Json[]).length === 0;
console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.pass ? 0 : 1);
