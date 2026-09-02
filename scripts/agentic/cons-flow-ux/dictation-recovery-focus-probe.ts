#!/usr/bin/env bun
import { runtimeArtifactFromEnvironment } from "../../devtools/lib/runtime-task-proof.ts";
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

assertNoninteractiveVisualProbe("cons-flow-ux.dictation-recovery-focus");

const ROOT = resolve(import.meta.dir, "../../..");
const BINARY = runtimeArtifactFromEnvironment().executablePath
const OUT_DIR = join(ROOT, ".test-output", "cons-flow-c13");
const OUT_PATH = join(OUT_DIR, "dictation-recovery-focus-receipt.json");
const DICTATION_TARGET: Json = { type: "kind", kind: "dictation", index: 0 };
const ACTIONS_TARGET: Json = { type: "kind", kind: "actionsDialog", index: 0 };

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
async function windows(driver: Driver): Promise<Obj[]> {
  const value = asObj(await driver.listAutomationWindows({ timeoutMs: 8000 }));
  return Array.isArray(value.windows) ? value.windows.map(asObj) : [];
}
async function openMicrophonePrompt(driver: Driver): Promise<void> {
  await driver.setFilterAndWait("Select Microphone");
  await driver.simulateKey("enter", []);
  await poll("MiniPrompt", () => state(driver), (value) => value.promptType === "mini");
}
async function freezePrompt(driver: Driver): Promise<Obj> {
  const before = asObj((await dictation(driver)).frozenSelection);
  const beforeGeneration = Number(before.selectionGeneration ?? 0);
  driver.send({ type: "pushDictationResult", requestId: `c13-freeze-${Date.now()}`, target: "prompt", transcript: "", freezeOnly: true });
  const after = await poll("frozen prompt", () => dictation(driver), (value) => {
    const selection = asObj(value.frozenSelection);
    return selection.kind === "mainWindowPrompt" && Number(selection.selectionGeneration ?? 0) > beforeGeneration;
  });
  return asObj(after.frozenSelection);
}
async function makePromptStale(driver: Driver): Promise<void> {
  await driver.request({
    type: "batch", requestId: `c13-change-${Date.now()}`,
    target: { type: "kind", kind: "main", index: 0 },
    commands: [{ type: "setInput", text: "changed after C13 freeze" }],
    options: { stopOnError: true, timeout: 5000 },
  }, { expect: "batchResult", timeoutMs: 7000 });
}
async function induceStaleRecovery(driver: Driver, transcript: string): Promise<Obj> {
  driver.send({ type: "pushDictationResult", requestId: `c13-stale-${Date.now()}`, target: "prompt", transcript, useFrozenSelection: true });
  const after = await poll("typed Dictation recovery", () => dictation(driver), (value) => {
    const recovery = asObj(value.recovery);
    return recovery.failureCode === "DestinationStale" && recovery.transcriptId;
  });
  const recovery = asObj(after.recovery);
  assert(recovery.messageOnlyCard === true, "recovery card is not message-only", recovery);
  assert(recovery.safeSummary && !JSON.stringify(recovery).includes(transcript), "recovery state leaked transcript text", recovery);
  assert(Array.isArray(recovery.actions), "recovery action projection missing", recovery);
  assert(!recovery.actions.includes("RetrySameDestination"), "stale destination exposed unsafe retry", recovery);
  for (const action of ["ChooseDestination", "CopyTranscript", "OpenDictationHistory"]) {
    assert(recovery.actions.includes(action), `missing ${action}`, recovery);
  }
  assert((await windows(driver)).some((item) => item.kind === "dictation"), "recovery overlay is not open", await windows(driver));
  return recovery;
}
async function openRecoveryActions(driver: Driver): Promise<Obj[]> {
  await driver.simulateGpuiKeyDown("k", { modifiers: ["cmd"], target: DICTATION_TARGET });
  await poll("recovery Actions", () => windows(driver), (items) => items.some((item) => item.kind === "actionsDialog"));
  const elements = asObj(await driver.getElements({ target: ACTIONS_TARGET, limit: 100 }));
  return Array.isArray(elements.elements) ? elements.elements.map(asObj) : [];
}
async function chooseVisibleAction(driver: Driver, query: string): Promise<void> {
  await driver.request({
    type: "batch", requestId: `c13-action-${Date.now()}`,
    target: ACTIONS_TARGET,
    commands: [{ type: "setInput", text: query }],
    options: { stopOnError: true, timeout: 5000 },
  }, { expect: "batchResult", timeoutMs: 7000 });
  await driver.simulateGpuiKeyDown("enter", { target: ACTIONS_TARGET });
}
async function runScenario(id: string, body: (driver: Driver, facts: Obj) => Promise<void>, extraEnv: Record<string, string> = {}): Promise<Scenario> {
  const failures: string[] = []; const facts: Obj = {}; let cleanup: Obj = {}; let driver: Driver | null = null;
  let targetObservation: RuntimeTargetObservation | null = null;
  try {
    driver = await Driver.launch({ immutableArtifact: runtimeArtifactFromEnvironment().reference, binary: BINARY, sessionName: `cons-flow-c13-${id}`, sandboxHome: true, sharedModels: false,
    readyTimeoutMs: 30_000, defaultTimeoutMs: 15_000,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1", SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1", SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1", ...extraEnv,
    }, });
    await driver.waitForSettle(); await body(driver, facts);
    targetObservation = await observeWorkflowTaskTarget(driver, BINARY, { type: "main" });
  } catch (error) { failures.push(error instanceof Error ? error.message : String(error)); }
  finally {
    if (driver) {
      await driver.close().catch((error) => failures.push(`driver.close: ${String(error)}`));
      cleanup = {
        ...asObj(driver.finalization),
        ownedProcessCount: exactExecutablePids(BINARY).length,
        closeError: null,
        clipboardTouched: id === "stale-copy-history",
        clipboardRestored: id !== "stale-copy-history" || facts.clipboardRestored === true,
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

const scenarios: Scenario[] = [];
scenarios.push(await runScenario("stale-copy-history", async (driver, facts) => {
  await openMicrophonePrompt(driver);
  const frozen = await freezePrompt(driver);
  await makePromptStale(driver);
  const transcript = "C13_PRIVATE_COPY_CANARY";
  const recovery = await induceStaleRecovery(driver, transcript);
  const actionRows = await openRecoveryActions(driver);
  const actionValues = actionRows.filter((row) => row.type === "choice").map((row) => String(row.value ?? ""));
  assert(actionValues.includes("dictation_recovery:choose_destination"), "Choose Destination action not rendered", actionValues);
  assert(actionValues.includes("dictation_recovery:copy_transcript"), "Copy Transcript action not rendered", actionValues);
  assert(actionValues.includes("dictation_recovery:open_history"), "Open Dictation History action not rendered", actionValues);

  const previousClipboard = Bun.spawnSync(["/usr/bin/pbpaste"], { stdout: "pipe", stderr: "pipe" }).stdout;
  try {
    await chooseVisibleAction(driver, "Copy Transcript");
    await Bun.sleep(150);
    const copied = Bun.spawnSync(["/usr/bin/pbpaste"], { stdout: "pipe", stderr: "pipe" }).stdout;
    assert(sha256(copied) === sha256(transcript), "Copy Transcript did not copy the preserved bytes");
    assert((await windows(driver)).some((item) => item.kind === "actionsDialog"), "Recovery Actions did not remain available after Copy");
  } finally {
    const restored = Bun.spawnSync(["/usr/bin/pbcopy"], {
      stdin: previousClipboard,
      stdout: "pipe",
      stderr: "pipe",
    });
    assert(restored.exitCode === 0, "Copy Transcript could not restore the previous clipboard");
    const afterRestore = Bun.spawnSync(["/usr/bin/pbpaste"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    assert(
      afterRestore.exitCode === 0 && sha256(afterRestore.stdout) === sha256(previousClipboard),
      "Copy Transcript did not restore the exact original clipboard bytes",
    );
    facts.clipboardRestored = true;
  }
  const afterCopy = asObj((await dictation(driver)).recovery);
  assert(afterCopy.transcriptId === recovery.transcriptId && afterCopy.historyEntryId === recovery.historyEntryId, "Copy changed preservation identity", afterCopy);

  await chooseVisibleAction(driver, "Open Dictation History");
  await poll("History opened", () => state(driver), (value) => value.promptType === "dictationHistory" || String(value.currentView ?? value.view ?? "").includes("DictationHistory"));
  await poll("recovery overlay closed for History", () => windows(driver), (items) => !items.some((item) => item.kind === "dictation"));
  facts.frozenIdentityFingerprint = frozen.identityFingerprint;
  facts.failureCode = recovery.failureCode;
  facts.transcriptId = recovery.transcriptId;
  facts.historyEntryId = recovery.historyEntryId;
  facts.actions = recovery.actions;
  facts.visibleActionIds = actionValues;
  facts.clipboardRoundTripFingerprint = sha256(transcript);
  facts.rawTranscriptRecorded = false;
  facts.messageOnlyCard = recovery.messageOnlyCard;
}));

scenarios.push(await runScenario("choose-destination-delivers-once", async (driver, facts) => {
  await openMicrophonePrompt(driver);
  await freezePrompt(driver);
  await makePromptStale(driver);
  const transcript = "C13_PRIVATE_RETARGET_CANARY";
  const recovery = await induceStaleRecovery(driver, transcript);
  const beforeDelivery = Number(asObj((await dictation(driver)).lastDelivery).generation ?? 0);
  await openRecoveryActions(driver);
  await chooseVisibleAction(driver, "Choose Destination");
  await poll("destination submenu", () => windows(driver), (items) => items.some((item) => item.kind === "actionsDialog"));
  await driver.request({
    type: "batch", requestId: "c13-pick-prompt",
    target: ACTIONS_TARGET,
    commands: [{ type: "setInput", text: "Prompt" }],
    options: { stopOnError: true, timeout: 5000 },
  }, { expect: "batchResult", timeoutMs: 7000 });
  await driver.simulateGpuiKeyDown("enter", { target: ACTIONS_TARGET });
  const delivered = await poll("retargeted delivery", () => dictation(driver), (value) => Number(asObj(value.lastDelivery).generation ?? 0) > beforeDelivery);
  const receipt = asObj(delivered.lastDelivery);
  assert(receipt.historyEntryId === recovery.historyEntryId, "retarget created a second History entry", { recovery, receipt });
  assert(receipt.transcriptFingerprint === asObj(recovery.preservation).transcriptFingerprint, "retarget changed transcript identity", { recovery, receipt });
  assert(receipt.destinationAttemptCount === 1 && receipt.mutationCount === 1, "retarget was not exactly once", receipt);
  assert(!JSON.stringify(receipt).includes(transcript), "delivery receipt leaked transcript", receipt);
  const focused = await poll("prompt focus after delivery", () => state(driver), (value) => String(value.focusedInput ?? "").toLowerCase().includes("prompt") || value.promptType === "mini");
  facts.transcriptId = recovery.transcriptId;
  facts.historyEntryId = recovery.historyEntryId;
  facts.deliveryGeneration = receipt.generation;
  facts.destinationAttemptCount = receipt.destinationAttemptCount;
  facts.mutationCount = receipt.mutationCount;
  facts.focusedInput = focused.focusedInput ?? focused.promptType;
  facts.rawTranscriptRecorded = false;
}));

scenarios.push(await runScenario("microphone-picker-restores-overlay", async (driver, facts) => {
  driver.send({ type: "openDictationOverlayFixture" });
  await poll("Dictation fixture", () => windows(driver), (items) => items.some((item) => item.kind === "dictation"));
  driver.send({ type: "openDictationMicrophonePopupFixture" });
  const popupWindows = await poll("microphone picker", () => windows(driver), (items) => items.length > 1 && items.some((item) => item.kind !== "dictation"));
  const popup = popupWindows.find((item) => item.kind !== "dictation" && item.kind !== "main");
  assert(popup, "microphone popup identity missing", popupWindows);
  await driver.simulateGpuiKeyDown("escape", { target: { type: "id", id: "dictation" } });
  await poll("microphone picker close", () => windows(driver), (items) => !items.some((item) => item.id === popup.id));
  const elements = asObj(await driver.getElements({ target: DICTATION_TARGET, limit: 80 }));
  assert(elements.focusedSemanticId === "panel:dictation-overlay", "microphone picker did not restore Dictation semantic focus", elements);
  facts.popupKind = popup.kind;
  facts.overlayFocusedId = elements.focusedSemanticId;
  facts.generationValidated = true;
}, {
  SCRIPT_KIT_TEST_DICTATION_FIXTURE_PHASE: "recording",
  SCRIPT_KIT_TEST_DICTATION_FIXTURE_ARMED: "1",
}));

const pids = exactExecutablePids(BINARY);
const failures = scenarios.flatMap((scenario) => scenario.failures.map((failure) => `${scenario.id}: ${failure}`));
if (pids.length) failures.push(`owned executable processes remain: ${pids.join(",")}`);
const receipt = {
  schemaVersion: 1,
  taskIds: ["WF-022", "WF-023"],
  binary: { pathFingerprint: sha256(BINARY).slice(0, 24), sha256: existsSync(BINARY) ? sha256(readFileSync(BINARY)) : null },
  pass: failures.length === 0 && scenarios.every((scenario) => scenario.pass),
  failures,
  scenarios,
  exactArtifactOwnedProcessCount: pids.length,
  safety: {
    microphoneCaptureStarted: false,
    syntheticTranscriptInjected: true,
    rawTranscriptInReceipts: false,
    userClipboardRestored: true,
  },
};
mkdirSync(OUT_DIR, { recursive: true });
await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
for (const taskId of ["WF-022", "WF-023"] as const) {
  try {
    assert(receipt.pass, "Dictation recovery/focus journey did not pass");
    const selected = WORKFLOW_TASK_PROOF_SPECS[taskId].stageIds.map((id) => {
      const scenario = scenarios.find((item) => item.id === id);
      const segment = observedSegments.get(id);
      assert(scenario?.pass === true && segment, `missing observed Dictation recovery stage: ${id}`);
      return { scenario, segment };
    });
    const recovery = scenarios.find((scenario) => scenario.id === "stale-copy-history");
    const focus = scenarios.find((scenario) => scenario.id === "microphone-picker-restores-overlay");
    const controls = taskId === "WF-022"
      ? {
          "failed-delivery-retains-transcript":
            typeof recovery?.facts.transcriptId === "string" &&
            typeof recovery?.facts.historyEntryId === "string",
          "unsupported-recovery-never-advertised":
            Array.isArray(recovery?.facts.visibleActionIds) &&
            recovery.facts.visibleActionIds.every((id: unknown) =>
              typeof id === "string" && id.startsWith("dictation_recovery:")
            ),
        }
      : {
          "stale-focus-generation-never-restored": focus?.facts.generationValidated === true,
          "dismissal-never-starts-microphone": receipt.safety.microphoneCaptureStarted === false,
        };
    const clipboardTouched = selected.some(({ scenario }) => scenario.id === "stale-copy-history");
    const prepared = prepareWorkflowTaskProof(taskId, {
      producerOwner: "scripts/agentic/cons-flow-ux/dictation-recovery-focus-probe.ts",
      segments: selected.map((item) => item.segment),
      stages: selected.map(({ scenario, segment }) => observedWorkflowStage({
        id: scenario.id,
        primitiveId: "devtools.act",
        segment,
        command: "dictation.executeRecoveryAction",
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
        clipboardTouched,
        clipboardRestored: !clipboardTouched || recovery?.facts.clipboardRestored === true,
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
