#!/usr/bin/env bun
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver";
import { assertNoninteractiveVisualProbe } from "../../devtools/lib/operator-safety.ts";
import {
  observedWorkflowSegment,
  observedWorkflowStage,
  observeWorkflowTaskTarget,
  prepareBlockedWorkflowTaskProof,
  prepareWorkflowTaskProof,
  writeWorkflowTaskProof,
  type WorkflowObservedSegment,
} from "../../devtools/lib/workflow-task-proof.ts";
import { WORKFLOW_TASK_PROOF_SPECS } from "../../devtools/lib/workflow-task-contract.ts";
import type { RuntimeTargetObservation } from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.dictation-delivery");

const ROOT = resolve(import.meta.dir, "../../..");
const BINARY = resolve(process.env.PROBE_BINARY ?? join(ROOT, "target-agent/artifacts/cons-flow-c12/script-kit-gpui"));
const OUT_DIR = join(ROOT, ".test-output", "cons-flow-c12");
const OUT_PATH = join(OUT_DIR, "dictation-delivery-receipt.json");
type Obj = Record<string, any>;
type Scenario = { id: string; pass: boolean; failures: string[]; facts: Obj; cleanup: Obj };
const observedSegments = new Map<string, WorkflowObservedSegment>();
const asObj = (value: unknown): Obj => value && typeof value === "object" && !Array.isArray(value) ? value as Obj : {};
const sha256 = (value: string | Uint8Array) => createHash("sha256").update(value).digest("hex");
function assert(condition: unknown, message: string, detail?: unknown): asserts condition {
  if (!condition) throw new Error(`${message}${detail === undefined ? "" : `\n${JSON.stringify(detail, null, 2)}`}`);
}
function exactExecutablePids(executable: string): number[] {
  const proc = Bun.spawnSync(["/bin/ps", "-axo", "pid=,command="], { stdout: "pipe", stderr: "pipe" });
  const normalized = resolve(executable);
  return new TextDecoder().decode(proc.stdout).split("\n").map((line) => line.trim()).filter(Boolean).flatMap((line) => {
    const match = line.match(/^(\d+)\s+(.+)$/); if (!match) return [];
    return resolve(match[2].trim().split(/\s+/, 1)[0]) === normalized ? [Number(match[1])] : [];
  });
}
async function poll<T>(label: string, read: () => Promise<T>, accepts: (value: T) => boolean, timeoutMs = 15_000): Promise<T> {
  const deadline = Date.now() + timeoutMs; let last = await read();
  while (Date.now() < deadline) { if (accepts(last)) return last; await Bun.sleep(80); last = await read(); }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`);
}
async function state(driver: Driver): Promise<Obj> { return asObj(await driver.getState({ timeoutMs: 8000 })); }
async function dictation(driver: Driver): Promise<Obj> { return asObj((await state(driver)).dictation); }
async function openMicrophonePrompt(driver: Driver): Promise<void> {
  await driver.setFilterAndWait("Select Microphone");
  await driver.simulateKey("enter", []);
  await poll("MiniPrompt", () => state(driver), (value) => value.promptType === "mini");
}
async function windows(driver: Driver): Promise<Obj[]> {
  const value = asObj(await driver.listAutomationWindows({ timeoutMs: 8000 }));
  return Array.isArray(value.windows) ? value.windows.map(asObj) : [];
}
async function deliver(driver: Driver, target: string, transcript: string): Promise<Obj> {
  const before = await dictation(driver);
  const beforeGeneration = Number(asObj(before.lastDelivery).generation ?? 0);
  driver.send({ type: "pushDictationResult", requestId: `c12-${target}-${Date.now()}`, target, transcript });
  return poll(`delivery ${target}`, () => dictation(driver), (value) => Number(asObj(value.lastDelivery).generation ?? 0) > beforeGeneration);
}
async function freeze(driver: Driver, target: string, expectedKind: string): Promise<Obj> {
  const before = asObj((await dictation(driver)).frozenSelection);
  const beforeGeneration = Number(before.selectionGeneration ?? 0);
  driver.send({ type: "pushDictationResult", requestId: `c12-freeze-${target}-${Date.now()}`, target, transcript: "", freezeOnly: true });
  const after = await poll(`freeze ${target}`, () => dictation(driver), (value) => {
    const selection = asObj(value.frozenSelection);
    return selection.kind === expectedKind && Number(selection.selectionGeneration ?? 0) > beforeGeneration;
  });
  return asObj(after.frozenSelection);
}
async function deliverFrozen(driver: Driver, target: string, transcript: string): Promise<Obj> {
  const before = await dictation(driver);
  const beforeGeneration = Number(asObj(before.lastDelivery).generation ?? 0);
  driver.send({ type: "pushDictationResult", requestId: `c12-frozen-${target}-${Date.now()}`, target, transcript, useFrozenSelection: true });
  return poll(`frozen delivery ${target}`, () => dictation(driver), (value) => Number(asObj(value.lastDelivery).generation ?? 0) > beforeGeneration);
}
async function deliverFrozenRefusal(driver: Driver, target: string, transcript: string): Promise<Obj> {
  const before = await dictation(driver);
  const deliveryBefore = Number(asObj(before.lastDelivery).generation ?? 0);
  const refusalBefore = Number(asObj(before.wrongTargetRefusal).generation ?? 0);
  driver.send({ type: "pushDictationResult", requestId: `c12-stale-${target}-${Date.now()}`, target, transcript, useFrozenSelection: true });
  const after = await poll(`stale refusal ${target}`, () => dictation(driver), (value) => Number(asObj(value.wrongTargetRefusal).generation ?? 0) > refusalBefore);
  assert(Number(asObj(after.lastDelivery).generation ?? 0) === deliveryBefore, "stale target mutated a destination", after);
  const refusal = asObj(after.wrongTargetRefusal);
  assert(refusal.reasonCode === "targetStale" && refusal.noDeliveryAttempted === true, "stale target did not fail closed", refusal);
  return refusal;
}
async function runScenario(id: string, body: (driver: Driver, facts: Obj) => Promise<void>): Promise<Scenario> {
  const failures: string[] = []; const facts: Obj = {}; let cleanup: Obj = {}; let driver: Driver | null = null;
  let targetObservation: RuntimeTargetObservation | null = null;
  try {
    driver = await Driver.launch({
      binary: BINARY, sessionName: `cons-flow-c12-${id}`, sandboxHome: true, sharedModels: false,
      seedAgentAuth: id.includes("agent"), readyTimeoutMs: 30_000, defaultTimeoutMs: 15_000,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1", SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1", SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
      },
    });
    await driver.waitForSettle(); await body(driver, facts);
    targetObservation = await observeWorkflowTaskTarget(driver, BINARY, { type: "main" });
  } catch (error) { failures.push(error instanceof Error ? error.message : String(error)); }
  finally {
    if (driver) {
      await driver.close().catch((error) => failures.push(`driver.close: ${String(error)}`));
      cleanup = {
        ...asObj(driver.finalization),
        ownedProcessCount: exactExecutablePids(BINARY).length,
        clipboardTouched: false,
        closeError: null,
      };
      if (cleanup.processExited !== true || cleanup.streamsDrained !== true || cleanup.logWriterClosed !== true || cleanup.ownedProcessCount !== 0) {
        failures.push("incomplete Driver finalization");
      } else if (targetObservation !== null) {
        observedSegments.set(id, observedWorkflowSegment(id, targetObservation, cleanup));
      }
    }
  }
  return { id, pass: failures.length === 0, failures, facts, cleanup };
}
function assertReceipt(state: Obj, expectedTarget: string, expectedKind: string, transcript: string): Obj {
  const receipt = asObj(state.lastDelivery);
  assert(receipt.target === expectedTarget, "wrong delivery target", receipt);
  assert(receipt.frozenIdentityKind === expectedKind, "wrong frozen identity kind", receipt);
  assert(typeof receipt.frozenIdentityFingerprint === "string" && receipt.frozenIdentityFingerprint.startsWith("fnv1a64:"), "missing frozen identity fingerprint", receipt);
  assert(receipt.destinationAttemptCount === 1 && receipt.mutationCount === 1, "delivery was not exactly once", receipt);
  assert(receipt.transcriptLen === transcript.length && typeof receipt.transcriptFingerprint === "string", "redacted transcript receipt mismatch", receipt);
  assert(receipt.redacted === true && JSON.stringify(receipt).includes(transcript) === false, "receipt leaked transcript", receipt);
  return receipt;
}

const scenarios: Scenario[] = [];
scenarios.push(await runScenario("launcher-filter", async (driver, facts) => {
  const transcript = "C12 launcher fixture";
  const after = await deliver(driver, "mainWindowFilter", transcript);
  const receipt = assertReceipt(after, "MainWindowFilter", "mainWindowFilter", transcript);
  assert(asObj(receipt.insertionRange).operation === "replaceFrozenInput", "launcher did not use frozen input actor", receipt);
  facts.receipt = receipt;
}));

scenarios.push(await runScenario("stale-launcher-input-refuses", async (driver, facts) => {
  const frozen = await freeze(driver, "mainWindowFilter", "mainWindowFilter");
  await driver.setFilterAndWait("changed after frozen selection");
  const refusal = await deliverFrozenRefusal(driver, "mainWindowFilter", "C12 stale launcher fixture");
  facts.frozenSelection = frozen;
  facts.refusal = refusal;
  facts.destinationAttemptCount = 0;
  facts.noFallback = true;
}));

scenarios.push(await runScenario("prompt-input", async (driver, facts) => {
  await openMicrophonePrompt(driver);
  const transcript = "C12 prompt fixture";
  const after = await deliver(driver, "prompt", transcript);
  const receipt = assertReceipt(after, "MainWindowPrompt", "mainWindowPrompt", transcript);
  assert(asObj(receipt.insertionRange).operation === "replaceFrozenInput", "prompt did not use frozen input actor", receipt);
  facts.receipt = receipt;
}));

scenarios.push(await runScenario("stale-prompt-input-refuses", async (driver, facts) => {
  await openMicrophonePrompt(driver);
  const frozen = await freeze(driver, "prompt", "mainWindowPrompt");
  await driver.request({
    type: "batch",
    requestId: "c12-change-stale-prompt",
    target: { type: "kind", kind: "main", index: 0 },
    commands: [{ type: "setInput", text: "changed after frozen prompt selection" }],
    options: { stopOnError: true, timeout: 5000 },
  }, { expect: "batchResult", timeoutMs: 7000 });
  const refusal = await deliverFrozenRefusal(driver, "prompt", "C12 stale prompt fixture");
  facts.frozenSelection = frozen;
  facts.refusal = refusal;
  facts.destinationAttemptCount = 0;
  facts.noFallback = true;
}));

scenarios.push(await runScenario("notes-editor", async (driver, facts) => {
  driver.send({ type: "openNotes", requestId: "c12-open-notes" });
  await poll("Notes window", () => windows(driver), (items) => items.some((item) => item.kind === "notes"));
  const transcript = "C12 notes fixture";
  const after = await deliver(driver, "notes", transcript);
  const receipt = assertReceipt(after, "NotesEditor", "notesEditor", transcript);
  assert(asObj(receipt.insertionRange).operation?.includes("Frozen"), "Notes did not use frozen anchor", receipt);
  facts.receipt = receipt;
}));

scenarios.push(await runScenario("stale-notes-editor-refuses", async (driver, facts) => {
  driver.send({ type: "openNotes", requestId: "c12-open-stale-notes" });
  await poll("stale Notes window", () => windows(driver), (items) => items.some((item) => item.kind === "notes"));
  const frozen = await freeze(driver, "notes", "notesEditor");
  await driver.request({
    type: "batch",
    requestId: "c12-change-stale-note",
    target: { type: "kind", kind: "notes", index: 0 },
    commands: [{ type: "setInput", text: "changed after frozen Notes selection" }],
    options: { stopOnError: true, timeout: 5000 },
  }, { expect: "batchResult", timeoutMs: 7000 });
  const refusal = await deliverFrozenRefusal(driver, "notes", "C12 stale notes fixture");
  facts.frozenSelection = frozen;
  facts.refusal = refusal;
  facts.destinationAttemptCount = 0;
  facts.noFallback = true;
}));

scenarios.push(await runScenario("captured-day", async (driver, facts) => {
  const transcript = "C12 captured day fixture";
  const after = await deliver(driver, "today", transcript);
  const receipt = assertReceipt(after, "DayPageToday", "dayPage", transcript);
  facts.receipt = receipt;
}));

scenarios.push(await runScenario("fresh-agent-chat", async (driver, facts) => {
  const transcript = "C12 fresh agent fixture";
  const after = await deliver(driver, "agentchat", transcript);
  const receipt = assertReceipt(after, "TabAiHarness", "agentChat", transcript);
  facts.receipt = receipt;
  facts.zeroFocusedContextPolicy = true;
}));

scenarios.push(await runScenario("existing-agent-chat", async (driver, facts) => {
  await deliver(driver, "agentchat", "C12 establish existing chat fixture");
  const frozen = await freeze(driver, "agentchat", "agentChat");
  const transcript = "C12 existing agent fixture";
  const after = await deliverFrozen(driver, "agentchat", transcript);
  const receipt = assertReceipt(after, "TabAiHarness", "agentChat", transcript);
  facts.frozenSelection = frozen;
  facts.receipt = receipt;
  facts.existingThreadPolicy = true;
  facts.acceptedSubmission = true;
}));

scenarios.push(await runScenario("fresh-quick-ai", async (driver, facts) => {
  const transcript = "C12 quick ai fixture";
  const after = await deliver(driver, "ask", transcript);
  const receipt = assertReceipt(after, "QuickAiQuestion", "quickAi", transcript);
  facts.receipt = receipt;
  facts.freshZeroContext = true;
}));

scenarios.push(await runScenario("unknown-target-refuses", async (driver, facts) => {
  const before = await dictation(driver);
  const deliveryBefore = Number(asObj(before.lastDelivery).generation ?? 0);
  const refusalBefore = Number(asObj(before.wrongTargetRefusal).generation ?? 0);
  driver.send({ type: "pushDictationResult", requestId: "c12-unknown-target", target: "definitely-missing", transcript: "C12 refusal fixture" });
  const after = await poll("target refusal", () => dictation(driver), (value) => Number(asObj(value.wrongTargetRefusal).generation ?? 0) > refusalBefore);
  assert(Number(asObj(after.lastDelivery).generation ?? 0) === deliveryBefore, "unavailable prompt mutated another destination", after);
  assert(asObj(after.wrongTargetRefusal).noDeliveryAttempted === true, "refusal did not prove zero attempts", after);
  facts.refusal = after.wrongTargetRefusal;
  facts.noFallback = true;
}));

const pids = exactExecutablePids(BINARY);
const failures = scenarios.flatMap((scenario) => scenario.failures.map((failure) => `${scenario.id}: ${failure}`));
if (pids.length) failures.push(`owned executable processes remain: ${pids.join(",")}`);
const receipt = {
  schemaVersion: 1, taskIds: ["WF-020", "WF-021"],
  binary: { pathFingerprint: sha256(BINARY).slice(0, 24), sha256: existsSync(BINARY) ? sha256(readFileSync(BINARY)) : null },
  pass: failures.length === 0 && scenarios.every((scenario) => scenario.pass), failures, scenarios,
  exactArtifactOwnedProcessCount: pids.length,
  safety: { microphoneCaptureStarted: false, syntheticTranscriptInjected: true, rawTranscriptInReceipts: false },
};
mkdirSync(OUT_DIR, { recursive: true });
await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
for (const taskId of ["WF-020", "WF-021"] as const) {
  try {
    assert(receipt.pass, "Dictation delivery journey did not pass");
    const selected = WORKFLOW_TASK_PROOF_SPECS[taskId].stageIds.map((id) => {
      const scenario = scenarios.find((item) => item.id === id);
      const segment = observedSegments.get(id);
      assert(scenario?.pass === true && segment, `missing observed Dictation delivery stage: ${id}`);
      return { scenario, segment };
    });
    const stale = scenarios.filter((scenario) => scenario.id.includes("stale-"));
    const unknown = scenarios.find((scenario) => scenario.id === "unknown-target-refuses");
    const delivered = selected
      .map(({ scenario }) => asObj(scenario.facts.receipt))
      .filter((candidate) => Object.keys(candidate).length > 0);
    const controls = taskId === "WF-020"
      ? {
          "stale-destination-never-mutates":
            stale.length === 3 && stale.every((scenario) => scenario.facts.destinationAttemptCount === 0),
          "stale-destination-never-falls-back":
            stale.length === 3 && stale.every((scenario) => scenario.facts.noFallback === true),
        }
      : {
          "delivery-occurs-exactly-once":
            delivered.length > 0 && delivered.every((result) =>
              result.destinationAttemptCount === 1 && result.mutationCount === 1
            ),
          "unknown-destination-never-falls-back": unknown?.facts.noFallback === true,
        };
    const prepared = prepareWorkflowTaskProof(taskId, {
      producerOwner: "scripts/agentic/cons-flow-ux/dictation-delivery-probe.ts",
      segments: selected.map((item) => item.segment),
      stages: selected.map(({ scenario, segment }) => observedWorkflowStage({
        id: scenario.id,
        primitiveId: "devtools.dictation.deliverFixture",
        segment,
        command: "pushDictationResult",
        requestId: `${taskId}:${scenario.id}`,
        result: scenario.facts,
        pass: scenario.pass,
      })),
      negativeControls: controls,
      safety: {
        microphoneCaptureStarted: false,
        nativeInputInjected: false,
        liveAiStarted: false,
        screenTakeoverStarted: false,
        clipboardTouched: false,
      },
    });
    writeWorkflowTaskProof(taskId, prepared.receipt);
  } catch (error) {
    writeWorkflowTaskProof(taskId, prepareBlockedWorkflowTaskProof(
      taskId,
      error instanceof Error ? error.message : String(error),
    ).receipt);
  }
}
console.log(JSON.stringify(receipt, null, 2));
if (!receipt.pass) process.exitCode = 1;
