#!/usr/bin/env bun
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import { resolve, join } from "node:path";
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
import type { RuntimeTargetObservation } from "../../devtools/lib/runtime-task-proof.ts";

assertNoninteractiveVisualProbe("cons-flow-ux.dictation-dismiss-targets");

const ROOT = resolve(import.meta.dir, "../../..");
const BINARY = resolve(process.env.PROBE_BINARY ?? join(ROOT, "target-agent/artifacts/cons-flow-c11/script-kit-gpui"));
const OUT_DIR = join(ROOT, ".test-output", "cons-flow-c11");
const OUT_PATH = join(OUT_DIR, "dictation-dismiss-targets-receipt.json");
const TARGET: Json = { type: "kind", kind: "dictation", index: 0 };
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
async function poll<T>(label: string, read: () => Promise<T>, accepts: (value: T) => boolean, timeoutMs = 10_000): Promise<T> {
  const deadline = Date.now() + timeoutMs; let last = await read();
  while (Date.now() < deadline) { if (accepts(last)) return last; await Bun.sleep(60); last = await read(); }
  throw new Error(`${label} timed out: ${JSON.stringify(last)}`);
}
async function windows(driver: Driver): Promise<Obj[]> {
  const result = asObj(await driver.listAutomationWindows({ timeoutMs: 5000 }));
  return Array.isArray(result.windows) ? result.windows.map(asObj) : [];
}
async function waitOpen(driver: Driver, open: boolean) {
  return poll("Dictation window state", () => windows(driver), (items) => items.some((item) => item.kind === "dictation") === open);
}
async function dictationState(driver: Driver): Promise<Obj> {
  const result = asObj(await driver.getState({ timeoutMs: 8000 }));
  return asObj(result.dictation);
}
async function openFixture(driver: Driver) {
  driver.send({ type: "openDictationOverlayFixture" });
  await waitOpen(driver, true);
  return poll("Dictation fixture state", () => dictationState(driver), (state) => typeof state.phase === "string" && state.phase !== "idle");
}
async function runScenario(id: string, extraEnv: Record<string, string>, body: (driver: Driver, facts: Obj) => Promise<void>): Promise<Scenario> {
  const failures: string[] = []; const facts: Obj = {}; let cleanup: Obj = {}; let driver: Driver | null = null;
  let targetObservation: RuntimeTargetObservation | null = null;
  try {
    driver = await Driver.launch({
      binary: BINARY, sessionName: `cons-flow-c11-${id}`, sandboxHome: true, sharedModels: false,
      readyTimeoutMs: 30_000, defaultTimeoutMs: 12_000,
      env: {
        SCRIPT_KIT_TEST_STATUS: "1", SCRIPT_KIT_STARTUP_PROFILE: "dev-fast",
        SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1", SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
        SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1", ...extraEnv,
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

const scenarios: Scenario[] = [];
scenarios.push(await runScenario("recording-confirm-resume-discard", {
  SCRIPT_KIT_TEST_DICTATION_FIXTURE_PHASE: "recording", SCRIPT_KIT_TEST_DICTATION_FIXTURE_ARMED: "1",
}, async (driver, facts) => {
  const initial = await openFixture(driver);
  const actions = Array.isArray(initial.targetActions) ? initial.targetActions.map(asObj) : [];
  assert(actions.length === 7, "Actions did not expose seven current targets", actions);
  assert(!actions.some((action) => action.stableId === "aichat"), "legacy AI target remained selectable", actions);
  assert(JSON.stringify(initial.quickTargetIds) === JSON.stringify(["frontmost", "today", "ask", "agentchat"]), "quick target subset/order drifted", initial);
  const elements = asObj(await driver.getElements({ target: TARGET, limit: 80 }));
  const chips = Array.isArray(elements.elements) ? elements.elements.map(asObj).filter((element) => String(element.semanticId ?? "").startsWith("dictation-destination:")) : [];
  assert(chips.length >= 5, "destination indicator/chips missing", chips);
  assert(chips.filter((chip) => chip.kind === "selectDestination").length === 4, "quick chips are not four selectable destination actions", chips);

  await driver.simulateGpuiKeyDown("k", { modifiers: ["cmd"], target: TARGET });
  await poll("Dictation Actions open", () => windows(driver), (items) => items.some((item) => item.kind === "actionsDialog"));
  const actionElements = asObj(await driver.getElements({ target: ACTIONS_TARGET, limit: 80 }));
  const destinationActions = Array.isArray(actionElements.elements)
    ? actionElements.elements.map(asObj).filter((element) => element.type === "choice" && String(element.value ?? "").startsWith("dictation_target:"))
    : [];
  const destinationIds = destinationActions.map((action) => String(action.value).replace("dictation_target:", ""));
  assert(destinationActions.length === 7, "Dictation Actions did not render seven descriptor-backed targets", actionElements);
  assert(!destinationIds.includes("aichat"), "legacy AI target appeared in Dictation Actions", destinationIds);
  assert(destinationActions.every((action) => typeof action.text === "string" && action.text.length > 0), "Dictation Actions lost selector labels", destinationActions);
  const beforeActionTargetGeneration = Number(initial.targetGeneration ?? 0);
  const beforeDeliveryGeneration = Number(asObj(initial.lastDelivery).generation ?? 0);
  const beforeStopGeneration = Number(asObj(initial.stop).generation ?? 0);
  const beforeConfigFingerprint = JSON.stringify(asObj(asObj(initial.setup).configFingerprint));
  await driver.simulateGpuiKeyDown("enter", { target: TARGET });
  await poll("Dictation Actions selection", () => dictationState(driver), (state) => state.target === "MainWindowFilter");
  await poll("Dictation Actions close", () => windows(driver), (items) => !items.some((item) => item.kind === "actionsDialog"));
  const afterActionSelection = await dictationState(driver);
  assert(Number(afterActionSelection.targetGeneration) === beforeActionTargetGeneration + 1, "Dictation Actions did not advance target generation exactly once", afterActionSelection);
  assert(Number(asObj(afterActionSelection.lastDelivery).generation ?? 0) === beforeDeliveryGeneration, "Dictation Actions selection delivered a transcript", afterActionSelection);
  assert(Number(asObj(afterActionSelection.stop).generation ?? 0) === beforeStopGeneration, "Dictation Actions selection stopped capture", afterActionSelection);

  const beforeTargetGeneration = Number(afterActionSelection.targetGeneration ?? 0);
  for (const [x, expected] of [[198, "ExternalApp"], [255, "DayPageToday"], [309, "QuickAiQuestion"], [365, "TabAiHarness"]] as const) {
    await driver.simulateGpuiClick(x, 14, { target: TARGET, timeoutMs: 8000 });
    await poll(`target ${expected}`, () => dictationState(driver), (state) => state.target === expected);
  }
  const selected = await dictationState(driver);
  assert(Number(selected.targetGeneration) >= beforeTargetGeneration + 4, "chip clicks did not increment only target generation", selected);
  assert(Number(asObj(selected.lastDelivery).generation ?? 0) === beforeDeliveryGeneration, "chip selection delivered a transcript", selected);
  assert(Number(asObj(selected.stop).generation ?? 0) === beforeStopGeneration, "chip selection stopped capture", selected);
  assert(JSON.stringify(asObj(asObj(selected.setup).configFingerprint)) === beforeConfigFingerprint, "fixture target selection persisted config", { beforeConfigFingerprint, after: asObj(asObj(selected.setup).configFingerprint) });

  await driver.simulateGpuiKeyDown("escape", { target: TARGET });
  const confirming = await poll("confirmation", () => dictationState(driver), (state) => state.phase === "confirming");
  assert((await windows(driver)).some((item) => item.kind === "dictation"), "early Escape closed Dictation instead of confirming");
  await driver.simulateGpuiKeyDown("escape", { target: TARGET });
  await poll("resume", () => dictationState(driver), (state) => state.phase === "recording");
  await driver.simulateGpuiKeyDown("w", { modifiers: ["cmd"], target: TARGET });
  await poll("Command+W confirmation", () => dictationState(driver), (state) => state.phase === "confirming");
  assert((await windows(driver)).some((item) => item.kind === "dictation"), "Command+W bypassed recording confirmation");
  await driver.simulateGpuiKeyDown("escape", { target: TARGET });
  await poll("Command+W resume", () => dictationState(driver), (state) => state.phase === "recording");
  await driver.simulateGpuiKeyDown("escape", { target: TARGET });
  await poll("second confirmation", () => dictationState(driver), (state) => state.phase === "confirming");
  await driver.simulateGpuiKeyDown("backspace", { target: TARGET });
  await waitOpen(driver, false);
  facts.actions = actions.map((action) => ({ stableId: action.stableId, verb: action.deliveryVerb }));
  facts.visibleActionTargetIds = destinationIds;
  facts.actionsRenderedSemanticCount = destinationActions.length;
  facts.actionsSelectionTarget = "MainWindowFilter";
  facts.quickTargetIds = initial.quickTargetIds;
  facts.targetSelections = 5;
  facts.earlyEscapeStatus = confirming.phase;
  facts.confirmationEscapeResumed = true;
  facts.commandWConfirmedWithoutClosing = true;
  facts.explicitDiscardClosed = true;
  facts.deliveryGenerationUnchanged = true;
  facts.stopGenerationUnchanged = true;
  facts.configFingerprintUnchanged = true;
}));

scenarios.push(await runScenario("processing-hide-reopen", {
  SCRIPT_KIT_TEST_DICTATION_FIXTURE_PHASE: "transcribing", SCRIPT_KIT_TEST_DICTATION_FIXTURE_ARMED: "1",
}, async (driver, facts) => {
  const initial = await openFixture(driver);
  const deliveryBefore = Number(asObj(initial.lastDelivery).generation ?? 0);
  const elements = asObj(await driver.getElements({ target: TARGET, limit: 80 }));
  const chips = Array.isArray(elements.elements) ? elements.elements.map(asObj).filter((element) => element.kind === "selectDestination" && element.selectable === true) : [];
  assert(chips.length === 0, "processing chips remained executable", chips);
  const disabled = Array.isArray(elements.elements) ? elements.elements.map(asObj).filter((element) => String(element.semanticId ?? "").startsWith("dictation-destination:") && element.actionDisabled) : [];
  assert(disabled.length === 4, "processing chips did not expose truthful disabled reasons", elements);
  await driver.simulateGpuiKeyDown("escape", { target: TARGET });
  await waitOpen(driver, false);
  const hidden = await dictationState(driver);
  assert(hidden.phase === "transcribing", "hiding processing changed its phase", hidden);
  assert(Number(asObj(hidden.lastDelivery).generation ?? 0) === deliveryBefore, "hiding processing delivered/cancelled", hidden);
  driver.send({ type: "triggerBuiltin", builtinId: "builtin/dictation" });
  await waitOpen(driver, true);
  const reopened = await poll("processing hotkey reopen", () => dictationState(driver), (state) => state.phase === "transcribing");
  facts.processingPhase = reopened.phase;
  facts.disabledChipCount = disabled.length;
  facts.escapeHidWithoutCancellation = true;
  facts.hotkeyReopenedCurrentOverlay = true;
  facts.deliveryGenerationUnchanged = true;
}));

const pids = exactExecutablePids(BINARY);
const failures = scenarios.flatMap((scenario) => scenario.failures.map((failure) => `${scenario.id}: ${failure}`));
if (pids.length) failures.push(`owned executable processes remain: ${pids.join(",")}`);
const receipt = {
  schemaVersion: 1, taskIds: ["SAFE-002", "WF-018", "WF-019"],
  binary: { pathFingerprint: sha256(BINARY).slice(0, 24), sha256: sha256(readFileSync(BINARY)) },
  pass: failures.length === 0 && scenarios.every((scenario) => scenario.pass), failures, scenarios,
  exactArtifactOwnedProcessCount: pids.length,
  safety: { microphoneCaptureStarted: false, transcriptReturned: false, configPersisted: false },
};
mkdirSync(OUT_DIR, { recursive: true });
await Bun.write(OUT_PATH, `${JSON.stringify(receipt, null, 2)}\n`);
for (const taskId of ["SAFE-002", "WF-018", "WF-019"] as const) {
  try {
    assert(receipt.pass, "Dictation dismissal journey did not pass");
    const selected = ["recording-confirm-resume-discard", "processing-hide-reopen"]
      .map((id) => {
        const scenario = scenarios.find((item) => item.id === id);
        const segment = observedSegments.get(id);
        assert(scenario?.pass === true && segment, `missing observed Dictation stage: ${id}`);
        return { scenario, segment };
      });
    const recording = selected[0]!.scenario.facts;
    const processing = selected[1]!.scenario.facts;
    const controls = taskId === "SAFE-002"
      ? {
          "recording-never-discarded-without-confirmation":
            recording.confirmationEscapeResumed === true &&
            recording.commandWConfirmedWithoutClosing === true,
          "processing-never-cancelled-by-escape":
            processing.escapeHidWithoutCancellation === true,
        }
      : taskId === "WF-018"
        ? {
            "legacy-destination-never-selectable":
              Array.isArray(recording.visibleActionTargetIds) &&
              !recording.visibleActionTargetIds.includes("aichat"),
            "disabled-destination-explains-refusal":
              Number(processing.disabledChipCount) === 4,
          }
        : {
            "destination-selection-never-delivers":
              recording.deliveryGenerationUnchanged === true,
            "destination-selection-never-stops-recording":
              recording.stopGenerationUnchanged === true,
          };
    const prepared = prepareWorkflowTaskProof(taskId, {
      producerOwner: "scripts/agentic/cons-flow-ux/dictation-dismiss-targets-probe.ts",
      segments: selected.map((item) => item.segment),
      stages: selected.map(({ scenario, segment }) => observedWorkflowStage({
        id: scenario.id,
        primitiveId: "devtools.act",
        segment,
        command: "simulateGpuiKeyDown",
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
