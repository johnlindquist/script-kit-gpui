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
 * Every judgement is delegated to `ai-chat-parity-evidence.ts`, which is
 * unit-tested without an app, so a run that drove nothing reports `absent`
 * rather than "no failures".
 *
 * Usage:
 *   bun scripts/devtools/devtools.ts build-ops act app-build --artifact-out .test-output/ai-chat-parity.reference.json
 *   SCRIPT_KIT_ARTIFACT_REFERENCE=.test-output/ai-chat-parity.reference.json bun scripts/agentic/ai-chat-parity-probe.ts
 *
 * Negative control — confirm the probe can actually fail before trusting it:
 *   SCRIPT_KIT_AGENT_CHAT_MARKDOWN_SELECTABLE=0 \
 *     bun scripts/agentic/ai-chat-parity-probe.ts
 *   # => flowSession.answer-text-is-selectable FAILS with `notSelectable`
 *
 * Exits non-zero on any failed assertion. Run on a quiet machine: this drives
 * real windows, and a contended machine produces interference receipts that
 * must NOT be read as product failures.
 *
 * SCOPE: this covers the FLOW half — the half this batch changed. Agent Chat's
 * ported affordances (per-turn copy, jump-to-latest) are NOT driven here
 * because `openAgentChatKitchenSinkFixture`, the entry point every existing
 * Agent Chat probe uses to open that surface, was deleted in 401936c41; those
 * probes are stale. Reviving it is its own task. Claiming coverage without
 * driving it would be worse than naming the gap.
 */
import { join, resolve } from "node:path";

import { Driver, type Json } from "../devtools/driver";
import { runtimeArtifactFromEnvironment } from "../devtools/lib/runtime-task-proof.ts";
import {
  FLOW_ANSWER_SCOPE_PREFIX,
  FLOW_ANSWER_SCOPE_SUFFIX,
  clipboardCopyPasses,
  componentPresent,
  evaluateClipboardCopy,
  evaluateSelectable,
  verdictPasses,
  type FidelityNode,
  type LayoutComponent,
} from "./ai-chat-parity-evidence";

const repoRoot = resolve(import.meta.dir, "../..");
const flowFixture = join(import.meta.dir, "fixtures/flow-ux-project");
const packageFixture = join(import.meta.dir, "fixtures/flow-desk-package");
const artifact = runtimeArtifactFromEnvironment();
const binary = process.env.SCRIPT_KIT_GPUI_BINARY ?? artifact.executablePath;

const receipt: Json = {
  schemaVersion: 1,
  tool: "ai-chat-parity-probe",
  repoRoot,
  binary,
  artifact: artifact.reference,
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
  immutableArtifact: artifact.reference,
  sessionName: `ai-chat-parity-${process.pid}`,
  sandboxHome: true,
  readyTimeoutMs: 30_000,
  defaultTimeoutMs: 12_000,
  env: {
    // Fidelity paint scopes — and therefore the `selectable` annotation this
    // probe exists to read — are only captured when this is set. Without it
    // `getLayoutInfo().fidelity` is `None` and every scope reads `absent`,
    // which looks exactly like a product regression.
    SCRIPT_KIT_FIDELITY_CAPTURE: "agent-chat",
    // Negative control for CI/manual use: setting this to "0" must flip
    // `flowSession.answer-text-is-selectable` to a FAILING `notSelectable`
    // verdict. A probe nobody has watched fail is a probe nobody should trust.
    ...(process.env.SCRIPT_KIT_AGENT_CHAT_MARKDOWN_SELECTABLE
      ? {
          SCRIPT_KIT_AGENT_CHAT_MARKDOWN_SELECTABLE:
            process.env.SCRIPT_KIT_AGENT_CHAT_MARKDOWN_SELECTABLE,
        }
      : {}),
    SCRIPT_KIT_TEST_STATUS: "1",
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

/**
 * Plant a known value on the real system pasteboard.
 *
 * `pbcopy`/`pbpaste` deliberately, not an app-provided read-back: the point of
 * this leg is to leave the app's own reporting and check the boundary the user
 * actually experiences when they hit ⌘V somewhere else.
 */
async function setClipboard(value: string): Promise<void> {
  const proc = Bun.spawn(["pbcopy"], { stdin: "pipe" });
  proc.stdin.write(value);
  await proc.stdin.end();
  await proc.exited;
}

async function readClipboard(): Promise<string> {
  const proc = Bun.spawn(["pbpaste"], { stdout: "pipe" });
  const text = await new Response(proc.stdout).text();
  await proc.exited;
  return text;
}

/**
 * Submit one real turn to the open Flow session and wait for it to settle.
 *
 * Polls the FLOW ANSWER SCOPE rather than a timer: an answer region only
 * appears once the renderer painted one, so this cannot report success against
 * a transcript that never arrived. Returns the layout that carried it, or the
 * last layout seen if it never did — the caller's `evaluateSelectable` then
 * reports `absent`, which is never a pass.
 */
async function submitTurnAndSettle(message: string): Promise<Json> {
  await driver.setFilterAndWait(message);
  await Bun.sleep(120);
  await press("enter");
  let layout = await driver.getLayoutInfo({}, { timeoutMs: 12_000 });
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const answers = fidelityNodes(layout).filter((node) =>
      String(node?.id ?? "").startsWith(FLOW_ANSWER_SCOPE_PREFIX),
    );
    // Wait past the `_Thinking…_` placeholder for real answer text.
    if (answers.length > 0 && !String(JSON.stringify(layout)).includes("Thinking")) break;
    await Bun.sleep(250);
    layout = await driver.getLayoutInfo({}, { timeoutMs: 12_000 });
  }
  return layout;
}

try {
  driver.send({ type: "show" });
  await driver.waitForSettle();

  // ── Flow session, driven by a REAL turn ───────────────────────────
  // Deliberately not seeded through a test fixture. The claim under proof is
  // about what the renderer paints for an ordinary answer, and the
  // `fake-codex` fixture binary is deterministic, so an actual submit is both
  // available and stronger evidence than an injected transcript.
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

  const beforeTurnMessage = "Say something quotable.";
  const flowLayout = await submitTurnAndSettle(beforeTurnMessage);
  const flowSelectable = evaluateSelectable(
    fidelityNodes(flowLayout),
    FLOW_ANSWER_SCOPE_PREFIX,
    FLOW_ANSWER_SCOPE_SUFFIX,
  );
  (receipt.evidence as Json).flowSelectable = flowSelectable as unknown as Json;
  // Every scope the flow surface painted, so an `absent` verdict names what
  // WAS there instead of only what was missing.
  (receipt.evidence as Json).flowScopeIds = fidelityNodes(flowLayout)
    .map((node) => node?.id)
    .filter((id) => String(id ?? "").includes("chat")) as unknown as Json;

  // THE headline claim of the whole batch: a Flow answer is selectable text.
  // Before this work, Flow's answers rendered through an engine with no
  // concept of selection, so this scope would report `absent` (no shared
  // scope was emitted at all) rather than `notSelectable`.
  expect(
    "flowSession.answer-text-is-selectable",
    verdictPasses(flowSelectable),
    { verdict: flowSelectable as unknown as Json },
  );

  // ── ⇧⌘C copies the last response ──────────────────────────────────
  // Runs BEFORE the ⌘L leg on purpose: ⌘L discards the transcript, and a copy
  // chord tested against an empty conversation proves nothing.
  //
  // The clipboard is the real system pasteboard, so this crosses an actual
  // integration boundary rather than reading back app state the app itself
  // reported. A sentinel is planted first — without it, "the clipboard has
  // text" passes for a clipboard nobody touched, which is precisely what an
  // unbound chord leaves behind. That was the defect: the ⌘K menu printed a
  // ⇧⌘C badge and no code answered it.
  const sentinel = `sentinel-${beforeTurnMessage.length}-not-a-response`;
  await setClipboard(sentinel);
  await press("c", ["cmd", "shift"]);
  await Bun.sleep(320);
  const clipboardAfterCopy = await readClipboard();
  const copyVerdict = evaluateClipboardCopy(sentinel, clipboardAfterCopy, [
    // Copying the user's own turn instead of the assistant's would otherwise
    // read as a healthy copy: the clipboard changed, and it holds real text.
    beforeTurnMessage,
  ]);
  (receipt.evidence as Json).copyLastResponse = copyVerdict as unknown as Json;
  expect("flowSession.shift-cmd-c-copies-the-last-response", clipboardCopyPasses(copyVerdict), {
    verdict: copyVerdict as unknown as Json,
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
  // `inputValue`, not `filterText` — that is the key `getState` actually
  // exposes the composer under. Reading a key that does not exist yields
  // `undefined`, which fails against every expected string and reads exactly
  // like a product bug.
  const afterResetState = await driver.getState();
  expect(
    "flowSession.reset-keeps-the-unsent-draft-and-consumes-the-chord",
    afterResetState.inputValue === draft,
    { expected: draft, actual: afterResetState.inputValue ?? null },
  );
  expect(
    "flowSession.still-open-after-reset",
    afterResetState.promptType === "flowSession",
    { promptType: afterResetState.promptType },
  );
  // The transcript viewport must still be mounted: a reset that left the
  // surface without a place to render the next answer would pass every count
  // check above while being unusable.
  //
  // Checked against `FlowSessionContent`, NOT `chat-transcript-viewport`: an
  // empty conversation legitimately paints the starter state instead of a
  // transcript viewport, so asserting the viewport here would fail on correct
  // behavior — which is how a probe teaches you to weaken it.
  expect(
    "flowSession.body-survives-the-reset",
    componentPresent(components(afterResetLayout), "FlowSessionContent"),
    {
      componentNames: components(afterResetLayout).map((component) => component?.name),
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
} catch (error) {
  // A thrown driver error must become a NAMED failure with a receipt, not a
  // stack trace and no output. A probe that dies before printing is
  // indistinguishable from a probe that was never run.
  expect("probe.completed-without-driver-error", false, {
    error: String((error as Error)?.message ?? error),
  });
} finally {
  await driver.close();
  (receipt.cleanup as Json).driverClosed = true;
  (receipt.cleanup as Json).pid = driver.pid;
}

receipt.pass = (receipt.failures as Json[]).length === 0;
console.log(JSON.stringify(receipt, null, 2));
process.exit(receipt.pass ? 0 : 1);
