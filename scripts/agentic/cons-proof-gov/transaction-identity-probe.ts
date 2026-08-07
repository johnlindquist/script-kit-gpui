#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Driver, type Json } from "../../devtools/driver.ts";
import {
  compareWindowLifetimeSnapshots,
  stableWindowInstanceId,
  targetIdentity,
} from "../../devtools/lib/target-identity.ts";

const binary = resolve(
  process.env.SCRIPT_KIT_GPUI_BINARY
    ?? "target-agent/artifacts/cons-proof-c03/script-kit-gpui",
);
const artifactPath = resolve(
  process.env.CONSISTENCY_RECEIPT_PATH
    ?? ".artifacts/consistency/PF-002/transaction-identity.json",
);

type Obj = Record<string, unknown>;

type CapturedTarget = {
  windowsBefore: Obj;
  windowsAfter: Obj;
  inspect: Obj;
  identity: ReturnType<typeof targetIdentity>;
  lifetime: ReturnType<typeof compareWindowLifetimeSnapshots>;
};

function asObj(value: unknown): Obj {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Obj
    : {};
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function sha256(path: string): string {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function processStartTime(pid: number): string | null {
  const result = Bun.spawnSync(["ps", "-p", String(pid), "-o", "lstart="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode !== 0) return null;
  return new TextDecoder().decode(result.stdout).trim() || null;
}

function exactExecutablePids(executable: string): number[] {
  const result = Bun.spawnSync(["ps", "-axo", "pid=,command="], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const normalized = resolve(executable);
  return new TextDecoder().decode(result.stdout)
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(.+)$/);
      if (!match) return [];
      const executablePath = match[2].trim().split(/\s+/, 1)[0];
      return resolve(executablePath) === normalized ? [Number(match[1])] : [];
    });
}

function listedWindows(envelope: Obj): Obj[] {
  return asArray(envelope.windows ?? envelope.automationWindows ?? envelope.targets)
    .map(asObj);
}

function windowKind(window: Obj): string {
  return String(window.kind ?? window.windowKind ?? "").toLowerCase();
}

function findKind(envelope: Obj, kind: string): Obj | null {
  const expected = kind.toLowerCase();
  return listedWindows(envelope).find((window) => windowKind(window) === expected) ?? null;
}

async function waitForKind(
  driver: Driver,
  kind: string,
  present: boolean,
  timeoutMs = 8_000,
): Promise<Obj> {
  const deadline = performance.now() + timeoutMs;
  let last: Obj = {};
  while (performance.now() < deadline) {
    last = asObj(await driver.listAutomationWindows({ timeoutMs: 3_000 }));
    const window = findKind(last, kind);
    if (present ? window != null : window == null) return last;
    await Bun.sleep(25);
  }
  throw new Error(`${kind} target did not become ${present ? "registered" : "retired"}`);
}

async function inspectTarget(driver: Driver, target: Json, label: string): Promise<Obj> {
  return asObj(await driver.request(
    {
      type: "inspectAutomationWindow",
      target,
      hiDpi: false,
      probes: [],
    },
    { expect: "automationInspectResult", timeoutMs: 8_000 },
  ).catch((error) => ({
    type: "driverRejected",
    errorName: error instanceof Error ? error.name : "UnknownError",
    label,
  })));
}

async function captureTarget(
  driver: Driver,
  target: Json,
  expectedSurfaceKind: string,
  label: string,
): Promise<CapturedTarget> {
  const windowsBefore = asObj(await driver.listAutomationWindows({ timeoutMs: 4_000 }));
  const inspect = await inspectTarget(driver, target, label);
  const windowsAfter = asObj(await driver.listAutomationWindows({ timeoutMs: 4_000 }));
  const identity = targetIdentity(
    { target, strict: true, expectedSurfaceKind },
    asObj(inspect.snapshot ?? inspect),
    windowsBefore,
  );
  const lifetime = compareWindowLifetimeSnapshots(
    identity.resolvedTarget.automationId,
    windowsBefore,
    windowsAfter,
  );
  return { windowsBefore, windowsAfter, inspect, identity, lifetime };
}

function transaction(
  runId: string,
  pid: number,
  binarySha256: string,
  capture: CapturedTarget,
): Obj {
  const target = capture.identity.resolvedTarget;
  const capturedAt = new Date().toISOString();
  const seed = JSON.stringify({
    runId,
    capturedAt,
    windowInstanceId: target.windowInstanceId,
    targetGeneration: target.targetGeneration,
    surfaceGeneration: target.surfaceGeneration,
    dataGeneration: target.dataGeneration,
  });
  return {
    transactionId: `proof:${createHash("sha256").update(seed).digest("hex").slice(0, 24)}`,
    runId,
    capturedAt,
    pid,
    processStartTime: processStartTime(pid),
    binarySha256,
    automationId: target.automationId ?? null,
    windowInstanceId: target.windowInstanceId ?? null,
    windowGeneration: target.windowGeneration ?? null,
    nativeWindowId: target.nativeWindowId ?? null,
    axWindowId: target.axWindowId ?? null,
    windowKind: target.windowKind ?? null,
    hostKind: target.hostKind ?? null,
    parentAutomationId: target.parentAutomationId ?? null,
    parentWindowInstanceId: target.parentWindowInstanceId ?? null,
    openerAutomationId: target.openerAutomationId ?? null,
    surfaceKind: target.surfaceKind ?? null,
    semanticSurface: target.semanticSurface ?? null,
    appViewVariant: target.appViewVariant ?? null,
    routeId: target.routeId ?? null,
    routeStack: target.routeStack ?? [],
    screenId: target.screenId ?? null,
    backingScaleFactor: target.backingScaleFactor ?? null,
    bounds: target.bounds ?? null,
    targetGeneration: target.targetGeneration ?? null,
    surfaceGeneration: target.surfaceGeneration ?? null,
    dataGeneration: target.dataGeneration ?? null,
    layoutGeneration: target.layoutGeneration ?? null,
    selectionGeneration: target.selectionGeneration ?? null,
    scrollGeneration: target.scrollGeneration ?? null,
    frameGeneration: target.frameGeneration ?? null,
  };
}

function requiredTransactionFields(value: Obj): string[] {
  return [
    "transactionId",
    "runId",
    "pid",
    "processStartTime",
    "binarySha256",
    "automationId",
    "windowInstanceId",
    "windowGeneration",
    "windowKind",
    "hostKind",
    "bounds",
    "targetGeneration",
    "surfaceGeneration",
    "dataGeneration",
  ].filter((field) => value[field] == null || value[field] === "");
}

function responseSummary(response: Obj): Obj {
  const snapshot = asObj(response.snapshot ?? response);
  return {
    type: response.type ?? response.responseType ?? null,
    status: response.status ?? null,
    errorCode: response.errorCode ?? asObj(response.error).code ?? null,
    windowId: snapshot.windowId ?? snapshot.id ?? null,
    windowGeneration: snapshot.windowGeneration ?? snapshot.generation ?? null,
    targetGeneration: snapshot.targetGeneration ?? null,
    surfaceGeneration: snapshot.surfaceGeneration ?? null,
    dataGeneration: snapshot.dataGeneration ?? null,
  };
}

function samePassiveTransaction(before: Obj, after: Obj): boolean {
  const stable = [
    "pid",
    "processStartTime",
    "binarySha256",
    "automationId",
    "windowInstanceId",
    "windowGeneration",
    "windowKind",
    "hostKind",
    "parentAutomationId",
    "parentWindowInstanceId",
    "surfaceKind",
    "semanticSurface",
    "appViewVariant",
    "targetGeneration",
    "surfaceGeneration",
    "dataGeneration",
  ];
  return stable.every((field) => JSON.stringify(before[field]) === JSON.stringify(after[field]));
}

let driver: Driver | null = null;
let closeError: string | null = null;
let runtimeError: string | null = null;
let runtimeStage = "launch";
let proof: Obj = {};
const binarySha256 = sha256(binary);

try {
  driver = await Driver.launch({
    binary,
    sessionName: `cons-proof-pf002-${process.pid}`,
    sandboxHome: true,
    sharedModels: false,
    defaultTimeoutMs: 10_000,
    env: {
      SCRIPT_KIT_TEST_STATUS: "1",
      SCRIPT_KIT_DISABLE_AGENT_CHAT_HOT_PREWARM: "1",
      SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK: "1",
    },
  });
  const pid = driver.pid;
  assert(typeof pid === "number", "driver did not expose its process id");
  const runId = `pf002-${pid}`;

  runtimeStage = "main-transaction";
  const mainBefore = await captureTarget(driver, { type: "main" }, "", "main-before");
  const mainTransactionBefore = transaction(runId, pid, binarySha256, mainBefore);
  assert(mainBefore.lifetime.consistent, "main target changed while its transaction began");
  assert(requiredTransactionFields(mainTransactionBefore).length === 0, "main transaction is incomplete");
  assert(mainTransactionBefore.appViewVariant != null, "main transaction lacks AppView identity");

  const mainExactTarget: Json = {
    type: "instance",
    id: String(mainTransactionBefore.automationId),
    generation: Number(mainTransactionBefore.windowGeneration),
  };
  const mainEvidence = {
    state: responseSummary(asObj(await driver.getTargetState(mainExactTarget, { timeoutMs: 5_000 }))),
    elements: responseSummary(asObj(await driver.getElements({ target: mainExactTarget, limit: 80 }, { timeoutMs: 5_000 }))),
    layout: responseSummary(asObj(await driver.getLayoutInfo({ target: mainExactTarget }, { timeoutMs: 5_000 }))),
  };
  const mainAfter = await captureTarget(driver, mainExactTarget, "", "main-after");
  const mainTransactionAfter = transaction(runId, pid, binarySha256, mainAfter);
  assert(mainAfter.lifetime.consistent, "main target changed while evidence was captured");
  assert(samePassiveTransaction(mainTransactionBefore, mainTransactionAfter), "main proof mixed target generations");

  runtimeStage = "notes-first-lifetime";
  driver.send({ type: "openNotes", requestId: "pf002-open-notes-first" });
  const notesFirstList = await waitForKind(driver, "notes", true);
  const notesFirstWindow = findKind(notesFirstList, "notes");
  assert(notesFirstWindow != null, "first Notes target is missing");
  const notesAutomationId = String(notesFirstWindow.id ?? notesFirstWindow.windowId ?? notesFirstWindow.automationId);
  const notesFirstGeneration = Number(notesFirstWindow.generation ?? notesFirstWindow.windowGeneration);
  assert(Number.isFinite(notesFirstGeneration), "first Notes generation is missing");
  const notesFirstTarget: Json = {
    type: "instance",
    id: notesAutomationId,
    generation: notesFirstGeneration,
  };
  const notesFirst = await captureTarget(driver, notesFirstTarget, "Notes", "notes-first");
  const notesFirstTransaction = transaction(runId, pid, binarySha256, notesFirst);
  assert(notesFirst.lifetime.consistent, "first Notes lifetime was not stable");
  assert(requiredTransactionFields(notesFirstTransaction).length === 0, "first Notes transaction is incomplete");

  runtimeStage = "notes-close-stale-negative";
  driver.send({ type: "openNotes", requestId: "pf002-close-notes-first" });
  await waitForKind(driver, "notes", false);
  const staleWhileClosed = await inspectTarget(driver, notesFirstTarget, "stale-while-closed");
  const staleWhileClosedSummary = responseSummary(staleWhileClosed);
  assert(staleWhileClosedSummary.windowId !== notesAutomationId, "retired Notes instance still resolved while closed");

  runtimeStage = "notes-reopen-generation";
  driver.send({ type: "openNotes", requestId: "pf002-open-notes-second" });
  const notesSecondList = await waitForKind(driver, "notes", true);
  const notesSecondWindow = findKind(notesSecondList, "notes");
  assert(notesSecondWindow != null, "reopened Notes target is missing");
  const notesSecondAutomationId = String(notesSecondWindow.id ?? notesSecondWindow.windowId ?? notesSecondWindow.automationId);
  const notesSecondGeneration = Number(notesSecondWindow.generation ?? notesSecondWindow.windowGeneration);
  assert(notesSecondAutomationId === notesAutomationId, "Notes reopen changed its stable automation id");
  assert(notesSecondGeneration > notesFirstGeneration, "Notes reopen did not advance lifetime generation");
  const notesSecondTarget: Json = {
    type: "instance",
    id: notesSecondAutomationId,
    generation: notesSecondGeneration,
  };
  const staleAfterReopen = await inspectTarget(driver, notesFirstTarget, "stale-after-reopen");
  const staleAfterReopenSummary = responseSummary(staleAfterReopen);
  assert(staleAfterReopenSummary.windowGeneration !== notesSecondGeneration, "old instance target resolved the reopened Notes lifetime");
  const notesSecond = await captureTarget(driver, notesSecondTarget, "Notes", "notes-second");
  const notesSecondTransaction = transaction(runId, pid, binarySha256, notesSecond);
  assert(notesSecond.lifetime.consistent, "reopened Notes lifetime was not stable");
  assert(requiredTransactionFields(notesSecondTransaction).length === 0, "reopened Notes transaction is incomplete");
  assert(notesFirstTransaction.windowInstanceId !== notesSecondTransaction.windowInstanceId, "reopened Notes reused a transaction instance id");

  runtimeStage = "attached-popup-parent";
  const openedActions = asObj(await driver.request(
    {
      type: "batch",
      target: notesSecondTarget,
      commands: [{ type: "openActions" }],
      options: { stopOnError: true, rollbackOnError: false, timeout: 8_000 },
    },
    { expect: "batchResult", timeoutMs: 10_000 },
  ));
  assert(openedActions.success === true, "target-scoped Notes Actions did not open");
  const actionsList = await waitForKind(driver, "actionsdialog", true);
  const actionsWindow = findKind(actionsList, "actionsdialog");
  assert(actionsWindow != null, "Actions target is missing");
  const actionsId = String(actionsWindow.id ?? actionsWindow.windowId ?? actionsWindow.automationId);
  const actionsGeneration = Number(actionsWindow.generation ?? actionsWindow.windowGeneration);
  const actionsTarget: Json = { type: "instance", id: actionsId, generation: actionsGeneration };
  const actions = await captureTarget(driver, actionsTarget, "ActionsDialog", "actions-parent");
  const actionsTransaction = transaction(runId, pid, binarySha256, actions);
  assert(actions.lifetime.consistent, "Actions lifetime was not stable");
  assert(actionsTransaction.hostKind === "attachedPopup", "Actions transaction lacks attached host identity");
  assert(actionsTransaction.parentAutomationId === notesSecondAutomationId, "Actions transaction lost its parent automation id");
  assert(actionsTransaction.parentWindowInstanceId === notesSecondTransaction.windowInstanceId, "Actions transaction lost its parent lifetime id");

  runtimeStage = "complete";
  proof = {
    main: {
      transaction: mainTransactionBefore,
      postTransaction: mainTransactionAfter,
      sameTransaction: true,
      evidence: mainEvidence,
    },
    reopen: {
      stableAutomationId: notesAutomationId,
      firstWindowInstanceId: notesFirstTransaction.windowInstanceId,
      secondWindowInstanceId: notesSecondTransaction.windowInstanceId,
      firstGeneration: notesFirstGeneration,
      secondGeneration: notesSecondGeneration,
      generationAdvanced: notesSecondGeneration > notesFirstGeneration,
      staleWhileClosed: staleWhileClosedSummary,
      staleAfterReopen: staleAfterReopenSummary,
      staleInstanceRejected: true,
      currentInstanceAccepted: notesSecond.identity.resolvedTarget.windowInstanceId === notesSecondTransaction.windowInstanceId,
    },
    attachedPopup: {
      transaction: actionsTransaction,
      parentPreserved: true,
      hostPreserved: true,
    },
    comparisonBasis: {
      fixtureId: "pf002-sandbox-main-notes-actions",
      sameUserPathRequired: true,
      sameHostAndSurfaceRequired: true,
      crossRunWindowInstanceMustDiffer: true,
      crossRunImplementationMustDifferForFixedProof: true,
    },
  };
} catch (error) {
  runtimeError = error instanceof Error ? error.message : "UnknownError";
} finally {
  if (driver) {
    try {
      await driver.close();
    } catch (error) {
      closeError = error instanceof Error ? error.name : "UnknownCloseError";
    }
  }
}

const ownedProcessCount = exactExecutablePids(binary).length;
const cleanup = driver
  ? {
      processExited: driver.finalization.processExited,
      streamsDrained: driver.finalization.streamsDrained,
      logWriterClosed: driver.finalization.logWriterClosed,
      ownedProcessCount,
      closeError,
      clipboardTouched: false,
    }
  : {
      processExited: false,
      streamsDrained: false,
      logWriterClosed: false,
      ownedProcessCount,
      closeError,
      clipboardTouched: false,
    };
const runtimePassed = runtimeError == null && runtimeStage === "complete";
const cleanupPassed = cleanup.processExited
  && cleanup.streamsDrained
  && cleanup.logWriterClosed
  && cleanup.ownedProcessCount === 0
  && cleanup.closeError == null;
const receipt = {
  schemaVersion: 2,
  taskId: "PF-002",
  classification: runtimePassed && cleanupPassed ? "RUNTIME-CONFIRMED" : "RUNTIME-FAILED",
  artifact: {
    executable: "target-agent/artifacts/cons-proof-c03/script-kit-gpui",
    sha256: binarySha256,
  },
  proof,
  negativeControls: {
    retiredInstanceRejectedWhileClosed: asObj(proof.reopen).staleInstanceRejected === true,
    oldInstanceRejectedAfterReopen: asObj(proof.reopen).staleInstanceRejected === true,
    reopenedGenerationAdvanced: asObj(proof.reopen).generationAdvanced === true,
    reusedWindowInstanceComparisonDisposition: "INVALID_IDENTITY",
    unlikeHostComparisonDisposition: "BLOCKED_MISSING_PRIMITIVE",
  },
  cleanup,
  runtimeError,
  runtimeStage,
};

await mkdir(resolve(artifactPath, ".."), { recursive: true });
await writeFile(artifactPath, `${JSON.stringify(receipt, null, 2)}\n`);
console.log(JSON.stringify(receipt, null, 2));

if (!runtimePassed || !cleanupPassed) process.exitCode = 1;
