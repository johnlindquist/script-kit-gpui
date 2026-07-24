#!/usr/bin/env bun
/**
 * Visible, fail-closed stress proof for animation-heavy window toggles.
 *
 * The probe exercises the real GPUI dispatch path for Cmd+K, the Notes
 * toggle command, and the Dictation builtin. Each phase posts input as fast
 * as the app can acknowledge it, watches for duplicate automation windows,
 * proves a deliberate recovery action, and rejects fresh ERROR/crash logs.
 */
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { Driver, type Json } from "./driver.ts";
import {
  identityFromEnvironment,
  newRunId,
} from "./glass-evidence-contract.ts";
import { announceTestStatus } from "./test-status.ts";
import {
  classifyNativeInventory,
  deriveUniqueOwnerDelta,
} from "./glass-topology-contract.ts";
import {
  finishInterferenceMonitor,
  startInterferenceMonitor,
  waitForInterferenceReady,
} from "./glass-interference.ts";

type WindowSnapshot = {
  id?: string;
  kind?: string;
  focused?: boolean;
  visible?: boolean;
};

function arg(name: string, fallback?: string) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function percentile(values: number[], fraction: number) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return Number(sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)]!.toFixed(2));
}

function countKind(snapshot: Json, kind: string) {
  return ((snapshot?.windows ?? []) as WindowSnapshot[]).filter(
    (window) => window.kind === kind && window.visible !== false,
  ).length;
}

async function waitForKindCount(
  driver: Driver,
  kind: string,
  expected: number,
  timeoutMs = 1_500,
) {
  const started = performance.now();
  let last: Json = null;
  while (performance.now() - started < timeoutMs) {
    last = await driver.listAutomationWindows({ timeoutMs: 5_000 });
    if (countKind(last, kind) === expected) {
      return {
        pass: true,
        elapsedMs: Number((performance.now() - started).toFixed(2)),
        count: expected,
        snapshot: last,
      };
    }
    await Bun.sleep(20);
  }
  return {
    pass: false,
    elapsedMs: Number((performance.now() - started).toFixed(2)),
    count: countKind(last, kind),
    snapshot: last,
  };
}

function boundedErrors(logs: Json, baselineCount: number) {
  return ((logs?.entries ?? []) as Json[])
    .slice(baselineCount)
    .map((entry) => ({
      timestamp: entry?.timestamp ?? null,
      target: entry?.target ?? null,
      message: String(entry?.message ?? "").slice(0, 240),
    }));
}

async function activatePid(pid: number) {
  const script = [
    'tell application "System Events"',
    `set processMatches to application processes whose unix id is ${pid}`,
    "if (count of processMatches) is 1 then set frontmost of item 1 of processMatches to true",
    "end tell",
  ].join("\n");
  const child = Bun.spawn(["osascript", "-e", script], {
    stdout: "ignore",
    stderr: "pipe",
  });
  const [stderr, exitCode] = await Promise.all([
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { exitCode, stderr: stderr.trim().slice(0, 400) };
}

async function nativeWindowInventory(
  pid: number,
  title?: string,
  expectedMainWindowId = 0,
) {
  const child = Bun.spawn([
    "swift",
    resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
    "--pid",
    String(pid),
  ], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  if (exitCode !== 0) {
    return {
      ids: [],
      windows: [],
      topology: null,
      mainWindowId: expectedMainWindowId,
      error: stderr.trim().slice(0, 400),
    };
  }
  const parsed = JSON.parse(stdout);
  const windows = (parsed.windows ?? []) as Json[];
  const mainWindowId = expectedMainWindowId || Number(
    windows
      .filter((window: Json) =>
        String(window?.title ?? "") === ""
        && Number(window?.layer) === 101
      )
      .sort((left: Json, right: Json) =>
        Number(right?.bounds?.width ?? 0) * Number(right?.bounds?.height ?? 0)
        - Number(left?.bounds?.width ?? 0) * Number(left?.bounds?.height ?? 0)
      )[0]?.windowId ?? 0,
  );
  return {
    ids: windows
      .filter((window: Json) =>
        (title == null || window?.title === title)
        && Number(window?.windowId ?? 0) > 0
      )
      .map((window: Json) => Number(window.windowId)),
    windows,
    topology: classifyNativeInventory(windows as any[], pid, mainWindowId),
    mainWindowId,
    error: null,
  };
}

async function notesLifecycleState(driver: Driver) {
  try {
    const state = await driver.getTargetState(
      { type: "id", id: "notes" },
      { timeoutMs: 5_000 },
    );
    const notes = state?.notes ?? state ?? null;
    return notes
      ? {
        entryReveal: notes.entryReveal ?? null,
        windowLifecycle: notes.windowLifecycle ?? null,
        editor: notes.editor ?? null,
      }
      : null;
  } catch {
    return null;
  }
}

const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!);
const outPath = resolve(
  arg(
    "--out",
    ".artifacts/main-window-native-drag/mwnd15-rapid-toggle/receipt.json",
  )!,
);
if (!binary || !existsSync(binary)) {
  throw new Error(`binary missing: ${binary || "<unset>"}`);
}
mkdirSync(dirname(outPath), { recursive: true });
const interferenceHelper = join(
  dirname(outPath),
  "macos-glass-interference-monitor",
);
const interferenceCompile = Bun.spawnSync([
  "xcrun",
  "swiftc",
  "-O",
  resolve(import.meta.dir, "../agentic/macos-glass-interference-monitor.swift"),
  "-o",
  interferenceHelper,
]);
if (interferenceCompile.exitCode !== 0) {
  throw new Error(
    `interference helper compile failed: ${interferenceCompile.stderr.toString()}`,
  );
}
let interferenceMonitor: ReturnType<typeof startInterferenceMonitor> | null = null;

const receipt: Json = {
  schemaVersion: 2,
  startedAt: new Date().toISOString(),
  ...identityFromEnvironment({
    runId: newRunId(),
    gitCommit: Bun.spawnSync(["git", "rev-parse", "HEAD"]).stdout.toString().trim(),
    binary,
    binarySha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
  }),
  scenario: process.env.SCRIPT_KIT_GLASS_SCENARIO ?? "rapid-toggle",
  phases: {},
  pass: false,
};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: "mwnd15-rapid-toggle",
  defaultTimeoutMs: 8_000,
});

try {
  receipt.pid = driver.pid ?? null;
  receipt.sessionDir = driver.sessionDir;
  driver.send({ type: "show" });
  const mainVisible = await waitForKindCount(driver, "main", 1, 5_000);
  const initialNativeInventory = await nativeWindowInventory(Number(driver.pid));
  const mainWindowId = initialNativeInventory.mainWindowId;
  if (!mainWindowId || initialNativeInventory.topology?.pass !== true) {
    throw new Error(
      `initial complete same-PID topology invalid: ${
        JSON.stringify(initialNativeInventory.topology?.errors ?? [])
      }`,
    );
  }
  receipt.initialNativeInventory = initialNativeInventory;
  const activation = driver.pid ? await activatePid(driver.pid) : null;
  await driver.waitForSettle({ timeoutMs: 5_000 });
  interferenceMonitor = startInterferenceMonitor(
    interferenceHelper,
    dirname(outPath),
  );
  receipt.interferenceReady = await waitForInterferenceReady(interferenceMonitor);
  const errorBaseline = await driver.getLogs({ limit: 500, level: "error" });
  const baselineErrorCount = ((errorBaseline?.entries ?? []) as Json[]).length;

  await announceTestStatus(
    "MWND-15A · Cmd+K hammer",
    "20 real GPUI key dispatches, then launcher input recovery",
  );
  const actionLatencies: number[] = [];
  const actionDispatches: Json[] = [];
  let maxActionWindows = 0;
  let maxNativeActionWindows = 0;
  for (let index = 0; index < 20; index += 1) {
    const started = performance.now();
    const dispatch = await driver.simulateGpuiKeyDown("k", {
      modifiers: ["cmd"],
      target: { type: "id", id: "main" },
      timeoutMs: 5_000,
    });
    actionLatencies.push(performance.now() - started);
    actionDispatches.push({
      index,
      success: dispatch?.success === true,
      dispatchPath: dispatch?.dispatchPath ?? null,
      resolvedWindowId: dispatch?.resolvedWindowId ?? null,
    });
    const windows = await driver.listAutomationWindows({ timeoutMs: 5_000 });
    maxActionWindows = Math.max(maxActionWindows, countKind(windows, "actionsDialog"));
    const native = await nativeWindowInventory(
      Number(driver.pid),
      undefined,
      mainWindowId,
    );
    const actionOwners = native.topology?.rows?.filter(
      (window: Json) => window?.classification === "Actions",
    ) ?? [];
    maxNativeActionWindows = Math.max(maxNativeActionWindows, actionOwners.length);
    actionDispatches.at(-1)!.nativeTopology = native.topology;
  }
  await Bun.sleep(350);
  await driver.waitForSettle({ timeoutMs: 5_000 });

  const actionStateAfterBurst = await driver.getState({ timeoutMs: 5_000 });
  const actionsOpenAfterBurst = actionStateAfterBurst?.actionsDialog?.open === true;
  if (actionsOpenAfterBurst) {
    await driver.simulateGpuiKeyDown("escape", {
      target: { type: "id", id: "actions-dialog" },
      timeoutMs: 5_000,
    });
    await waitForKindCount(driver, "actionsDialog", 0);
  }
  const inputRecoveryStarted = performance.now();
  await driver.setFilterAndWait("mwnd15-recovery", { timeoutMs: 5_000 });
  const inputRecoveryMs = performance.now() - inputRecoveryStarted;
  await driver.setFilterAndWait("", { timeoutMs: 5_000 });

  await Bun.sleep(350);
  const deliberateOpenStarted = performance.now();
  await driver.simulateGpuiKeyDown("k", {
    modifiers: ["cmd"],
    target: { type: "id", id: "main" },
    timeoutMs: 5_000,
  });
  const deliberateOpen = await waitForKindCount(driver, "actionsDialog", 1);
  const deliberateOpenMs = performance.now() - deliberateOpenStarted;
  const deliberateCloseStarted = performance.now();
  if (deliberateOpen.pass) {
    await driver.simulateGpuiKeyDown("escape", {
      target: { type: "id", id: "actions-dialog" },
      timeoutMs: 5_000,
    });
  }
  const deliberateClose = await waitForKindCount(driver, "actionsDialog", 0);
  const deliberateCloseMs = performance.now() - deliberateCloseStarted;
  const actionsErrors = boundedErrors(
    await driver.getLogs({ limit: 500, level: "error" }),
    baselineErrorCount,
  );
  const actionsPass = mainVisible.pass
    && actionDispatches.every((dispatch) => dispatch.success)
    && maxActionWindows <= 1
    && maxNativeActionWindows <= 1
    && actionDispatches.every((dispatch) => dispatch.nativeTopology?.pass === true)
    && inputRecoveryMs <= 300
    && deliberateOpen.pass
    && deliberateClose.pass
    && deliberateOpenMs <= 750
    && deliberateCloseMs <= 750
    && actionsErrors.length === 0;
  receipt.phases.actions = {
    pass: actionsPass,
    mainVisible,
    activation,
    pulses: actionDispatches.length,
    dispatches: actionDispatches,
    latencyMs: {
      p50: percentile(actionLatencies, 0.5),
      p95: percentile(actionLatencies, 0.95),
      max: actionLatencies.length ? Number(Math.max(...actionLatencies).toFixed(2)) : null,
    },
    maxActionWindows,
    maxNativeActionWindows,
    actionsOpenAfterBurst,
    inputRecoveryMs: Number(inputRecoveryMs.toFixed(2)),
    deliberateOpenMs: Number(deliberateOpenMs.toFixed(2)),
    deliberateCloseMs: Number(deliberateCloseMs.toFixed(2)),
    errors: actionsErrors,
  };

  await announceTestStatus(
    "MWND-15B · Notes hammer",
    "16 immediate Notes toggles, duplicate-window watch, then reopen recovery",
  );
  const notesErrorBaseline = ((await driver.getLogs({
    limit: 500,
    level: "error",
  }))?.entries ?? []).length;
  let maxNotesWindows = 0;
  const notesSamples: Json[] = [];
  for (let index = 0; index < 16; index += 1) {
    driver.send({
      type: "openNotes",
      requestId: `mwnd15-notes-${index}`,
    });
    await Bun.sleep(20);
    const windows = await driver.listAutomationWindows({ timeoutMs: 5_000 });
    const count = countKind(windows, "notes");
    maxNotesWindows = Math.max(maxNotesWindows, count);
    const native = driver.pid
      ? await nativeWindowInventory(driver.pid, "Notes", mainWindowId)
      : { ids: [], error: "driver PID unavailable" };
    const notesState = count > 0 ? await notesLifecycleState(driver) : null;
    notesSamples.push({
      index,
      automationWindowCount: count,
      nativeWindowIds: native.ids,
      nativeWindowCount: native.ids.length,
      nativeWindowError: native.error,
      nativeTopology: native.topology,
      notesState,
    });
  }
  await Bun.sleep(350);
  const notesAfterBurst = await driver.listAutomationWindows({ timeoutMs: 5_000 });
  if (countKind(notesAfterBurst, "notes") > 0) {
    driver.send({ type: "openNotes", requestId: "mwnd15-notes-normalize-close" });
    await waitForKindCount(driver, "notes", 0, 2_000);
  }
  const notesOpenStarted = performance.now();
  driver.send({ type: "openNotes", requestId: "mwnd15-notes-recovery-open" });
  const notesOpen = await waitForKindCount(driver, "notes", 1, 2_000);
  const notesOpenMs = performance.now() - notesOpenStarted;
  const hiddenInputBefore = await notesLifecycleState(driver);
  const hiddenInputText = "mwnd15 hidden editor input";
  const hiddenInputReceipt = await driver.request({
    type: "batch",
    target: { type: "kind", kind: "notes", index: 0 },
    commands: [{ type: "setInput", text: hiddenInputText }],
    options: { stopOnError: true, rollbackOnError: false, timeout: 5_000 },
    trace: "on",
  }, { expect: "batchResult", timeoutMs: 6_000 });
  const hiddenInputAfter = await notesLifecycleState(driver);
  const hiddenInputAccepted = hiddenInputBefore?.entryReveal?.bodyVisible === false
    && hiddenInputReceipt?.success !== false
    && hiddenInputBefore?.editor?.textFingerprint
      !== hiddenInputAfter?.editor?.textFingerprint
    && Number(hiddenInputAfter?.editor?.textLength ?? 0) === hiddenInputText.length;
  const beforeTail = driver.pid
    ? await nativeWindowInventory(driver.pid, "Notes", mainWindowId)
    : { ids: [], error: "driver PID unavailable" };
  driver.send({ type: "openNotes", requestId: "mwnd15-notes-close-before-tail" });
  await Bun.sleep(40);
  const duringNotesExit = await notesLifecycleState(driver);
  driver.send({ type: "openNotes", requestId: "mwnd15-notes-reopen-before-tail" });
  const notesReopened = await waitForKindCount(driver, "notes", 1, 2_000);
  const afterTail = driver.pid
    ? await nativeWindowInventory(driver.pid, "Notes", mainWindowId)
    : { ids: [], error: "driver PID unavailable" };
  const reusedNativeWindow = beforeTail.ids.length === 1
    && afterTail.ids.length === 1
    && beforeTail.ids[0] === afterTail.ids[0];
  const notesOwnerDelta = deriveUniqueOwnerDelta(
    initialNativeInventory.windows as any[],
    beforeTail.windows as any[],
    "Notes",
    Number(driver.pid),
    mainWindowId,
  );
  let revealAfterReopen = await notesLifecycleState(driver);
  const revealDeadline = performance.now() + 2_000;
  while (
    revealAfterReopen?.entryReveal?.bodyVisible !== true
    && performance.now() < revealDeadline
  ) {
    await Bun.sleep(20);
    revealAfterReopen = await notesLifecycleState(driver);
  }
  const notesRevealGenerations = [
    ...new Set(
      [
        ...notesSamples.map(
          (sample) => sample?.notesState?.entryReveal?.generation,
        ),
        duringNotesExit?.entryReveal?.generation,
        revealAfterReopen?.entryReveal?.generation,
      ]
        .map(Number)
        .filter(Number.isFinite),
    ),
  ];
  const notesCloseStarted = performance.now();
  driver.send({ type: "openNotes", requestId: "mwnd15-notes-recovery-close" });
  const notesClose = await waitForKindCount(driver, "notes", 0, 2_000);
  const notesCloseMs = performance.now() - notesCloseStarted;
  const notesErrors = boundedErrors(
    await driver.getLogs({ limit: 500, level: "error" }),
    notesErrorBaseline,
  );
  const notesPass = maxNotesWindows <= 1
    && notesSamples.every((sample) => Number(sample?.nativeWindowCount ?? 0) <= 1)
    && notesSamples.every((sample) => sample?.nativeTopology?.pass === true)
    && notesOpen.pass
    && hiddenInputAccepted
    && notesRevealGenerations.length >= 2
    && duringNotesExit?.windowLifecycle?.phase === "Exiting"
    && duringNotesExit?.windowLifecycle?.hasExitTicket === true
    && typeof duringNotesExit?.windowLifecycle?.exitGeneration === "number"
    && notesReopened.pass
    && reusedNativeWindow
    && notesOwnerDelta.pass
    && beforeTail.topology?.pass === true
    && afterTail.topology?.pass === true
    && revealAfterReopen?.entryReveal?.bodyVisible === true
    && notesClose.pass
    && notesOpenMs <= 750
    && notesCloseMs <= 750
    && notesErrors.length === 0;
  receipt.phases.notes = {
    pass: notesPass,
    pulses: notesSamples.length,
    samples: notesSamples,
    maxNotesWindows,
    notesOpenMs: Number(notesOpenMs.toFixed(2)),
    notesCloseMs: Number(notesCloseMs.toFixed(2)),
    hiddenInput: {
      accepted: hiddenInputAccepted,
      bodyVisibleBefore: hiddenInputBefore?.entryReveal?.bodyVisible ?? null,
      bodyVisibleAfter: hiddenInputAfter?.entryReveal?.bodyVisible ?? null,
      beforeFingerprint: hiddenInputBefore?.editor?.textFingerprint ?? null,
      afterFingerprint: hiddenInputAfter?.editor?.textFingerprint ?? null,
      textLength: hiddenInputAfter?.editor?.textLength ?? null,
      receiptSuccess: hiddenInputReceipt?.success ?? null,
    },
    closeBeforeTailReopen: {
      beforeNativeWindowIds: beforeTail.ids,
      afterNativeWindowIds: afterTail.ids,
      reusedNativeWindow,
      ownerDelta: notesOwnerDelta,
      beforeTopology: beforeTail.topology,
      afterTopology: afterTail.topology,
      reopened: notesReopened,
      duringExit: duringNotesExit,
      entryReveal: revealAfterReopen,
    },
    revealGenerations: notesRevealGenerations,
    errors: notesErrors,
  };

  await announceTestStatus(
    "MWND-15C · Dictation hammer",
    "12 real start/stop requests, then four exit-ticket cancellation cycles",
  );
  const dictationBefore = await driver.getState({ timeoutMs: 5_000 });
  const dictationBaseline = dictationBefore?.dictation
    ?? dictationBefore?.dictationState
    ?? dictationBefore?.dictation_state
    ?? null;
  const baselineGeneration = Number(dictationBaseline?.generation ?? 0);
  const baselineRecordingStateGeneration = Number(
    dictationBaseline?.recordingStateGeneration ?? 0,
  );
  const dictationErrorBaseline = ((await driver.getLogs({
    limit: 500,
    level: "error",
  }))?.entries ?? []).length;
  let maxDictationWindows = 0;
  let maxNativeDictationWindows = 0;
  const dictationSamples: Json[] = [];
  for (let index = 0; index < 12; index += 1) {
    driver.send({
      type: "triggerBuiltin",
      builtinId: "builtin/dictation",
      requestId: `mwnd15-dictation-${index}`,
    });
    await Bun.sleep(35);
    const windows = await driver.listAutomationWindows({ timeoutMs: 5_000 });
    const count = countKind(windows, "dictation");
    maxDictationWindows = Math.max(maxDictationWindows, count);
    const native = driver.pid
      ? await nativeWindowInventory(driver.pid, "Script Kit Dictation", mainWindowId)
      : { ids: [], error: "driver PID unavailable" };
    maxNativeDictationWindows = Math.max(maxNativeDictationWindows, native.ids.length);
    const state = await driver.getState({ timeoutMs: 5_000 });
    const dictation = state?.dictation ?? state?.dictationState ?? state?.dictation_state ?? null;
    dictationSamples.push({
      index,
      count,
      nativeWindowIds: native.ids,
      nativeWindowCount: native.ids.length,
      nativeWindowError: native.error,
      nativeTopology: native.topology,
      generation: dictation?.generation ?? null,
      recordingStateGeneration: dictation?.recordingStateGeneration ?? null,
      isRecording: dictation?.isRecording ?? null,
      phase: dictation?.phase ?? null,
      captureActive: dictation?.cleanup?.captureActive ?? null,
      captureStopInProgress: dictation?.cleanup?.captureStopInProgress ?? null,
      windowLifecycle: dictation?.windowLifecycle ?? null,
    });
  }

  let dictationState: Json = null;
  const dictationSettleStarted = performance.now();
  while (performance.now() - dictationSettleStarted < 8_000) {
    const state = await driver.getState({ timeoutMs: 5_000 });
    dictationState = state?.dictation ?? state?.dictationState ?? state?.dictation_state ?? null;
    const captureActive = dictationState?.cleanup?.captureActive === true;
    const stopInProgress = dictationState?.cleanup?.captureStopInProgress === true;
    if (!captureActive && !stopInProgress && dictationState?.isRecording !== true) break;
    await Bun.sleep(50);
  }
  const dictationSettleMs = performance.now() - dictationSettleStarted;
  const dictationCloseStarted = performance.now();
  const dictationClosed = await waitForKindCount(driver, "dictation", 0, 2_000);
  const dictationCloseMs = performance.now() - dictationCloseStarted;
  const dictationErrors = boundedErrors(
    await driver.getLogs({ limit: 500, level: "error" }),
    dictationErrorBaseline,
  );
  const dictationCleanup = {
    isRecording: dictationState?.isRecording ?? null,
    phase: dictationState?.phase ?? null,
    generation: dictationState?.generation ?? null,
    recordingStateGeneration: dictationState?.recordingStateGeneration ?? null,
    captureActive: dictationState?.cleanup?.captureActive ?? null,
    captureStopInProgress: dictationState?.cleanup?.captureStopInProgress ?? null,
    safety: dictationState?.safety ?? null,
  };
  const exercisedRealToggle = dictationSamples.some((sample) =>
    sample.count > 0
    || sample.isRecording === true
    || sample.captureActive === true
    || Number(sample.generation ?? 0) > baselineGeneration
    || Number(sample.recordingStateGeneration ?? 0) > baselineRecordingStateGeneration
  );
  const realToggleNativeDictationWindowIds = [
    ...new Set(
      dictationSamples.flatMap((sample) =>
        (sample?.nativeWindowIds ?? []) as number[]
      ),
    ),
  ];

  driver.send({
    type: "openDictationOverlayFixture",
    requestId: "mwnd15-dictation-fixture-open",
  });
  const fixtureOpened = await waitForKindCount(driver, "dictation", 1, 2_000);
  const fixtureInitialNative = driver.pid
    ? await nativeWindowInventory(driver.pid, "Script Kit Dictation", mainWindowId)
    : { ids: [], error: "driver PID unavailable" };
  const fixturePinnedWindowId = fixtureInitialNative.ids.length === 1
    ? fixtureInitialNative.ids[0]
    : null;
  const fixtureCycles: Json[] = [];
  for (let index = 0; index < 4; index += 1) {
    const confirmDispatch = await driver.simulateGpuiKeyDown("escape", {
      target: { type: "id", id: "dictation" },
      timeoutMs: 5_000,
    });
    await Bun.sleep(15);
    const discardDispatch = await driver.simulateGpuiKeyDown("backspace", {
      target: { type: "id", id: "dictation" },
      timeoutMs: 5_000,
    });
    await Bun.sleep(15);
    const duringExitState = await driver.getState({ timeoutMs: 5_000 });
    const duringExit = duringExitState?.dictation?.windowLifecycle ?? null;
    driver.send({
      type: "openDictationOverlayFixture",
      requestId: `mwnd15-dictation-fixture-reopen-${index}`,
    });
    const reopened = await waitForKindCount(driver, "dictation", 1, 2_000);
    await Bun.sleep(30);
    const afterReopenState = await driver.getState({ timeoutMs: 5_000 });
    const afterReopen = afterReopenState?.dictation?.windowLifecycle ?? null;
    const native = driver.pid
      ? await nativeWindowInventory(driver.pid, "Script Kit Dictation", mainWindowId)
      : { ids: [], error: "driver PID unavailable" };
    fixtureCycles.push({
      index,
      confirmDispatch,
      discardDispatch,
      duringExit,
      reopened,
      afterReopen,
      nativeWindowIds: native.ids,
      nativeWindowError: native.error,
      nativeTopology: native.topology,
    });
  }
  const fixturePass = fixtureOpened.pass
    && fixturePinnedWindowId != null
    && fixtureCycles.every((cycle) =>
      cycle?.confirmDispatch?.success === true
      && cycle?.discardDispatch?.success === true
      && cycle?.duringExit?.phase === "Exiting"
      && cycle?.duringExit?.handleRegistered === true
      && cycle?.duringExit?.automationRegistered === true
      && cycle?.duringExit?.hasExitTicket === true
      && typeof cycle?.duringExit?.exitGeneration === "number"
      && cycle?.reopened?.pass === true
      && cycle?.afterReopen?.phase === "Open"
      && cycle?.afterReopen?.hasExitTicket === false
      && Array.isArray(cycle?.nativeWindowIds)
      && cycle.nativeWindowIds.length === 1
      && cycle.nativeWindowIds[0] === fixturePinnedWindowId
      && cycle?.nativeTopology?.pass === true
    );
  await driver.simulateGpuiKeyDown("escape", {
    target: { type: "id", id: "dictation" },
    timeoutMs: 5_000,
  });
  await Bun.sleep(15);
  await driver.simulateGpuiKeyDown("backspace", {
    target: { type: "id", id: "dictation" },
    timeoutMs: 5_000,
  });
  const fixtureClosed = await waitForKindCount(driver, "dictation", 0, 2_000);

  const dictationPass = maxDictationWindows <= 1
    && maxNativeDictationWindows <= 1
    && dictationSamples.every((sample) => sample?.nativeTopology?.pass === true)
    && exercisedRealToggle
    && dictationCleanup.isRecording === false
    && dictationCleanup.captureActive === false
    && dictationCleanup.captureStopInProgress === false
    && dictationSettleMs <= 8_000
    && dictationClosed.pass
    && dictationCloseMs <= 2_000
    && fixturePass
    && fixtureClosed.pass
    && dictationErrors.length === 0;
  receipt.phases.dictation = {
    pass: dictationPass,
    pulses: dictationSamples.length,
    samples: dictationSamples,
    baselineGeneration,
    baselineRecordingStateGeneration,
    exercisedRealToggle,
    maxDictationWindows,
    maxNativeDictationWindows,
    realToggleNativeWindowIds: realToggleNativeDictationWindowIds,
    finalWindowCount: dictationClosed.count,
    settleMs: Number(dictationSettleMs.toFixed(2)),
    closeMs: Number(dictationCloseMs.toFixed(2)),
    cleanup: dictationCleanup,
    transcriptContentCaptured: false,
    fixtureExitCancellation: {
      pass: fixturePass,
      opened: fixtureOpened,
      pinnedNativeWindowId: fixturePinnedWindowId,
      cycles: fixtureCycles,
      closed: fixtureClosed,
      noMicrophoneCapture: true,
    },
    errors: dictationErrors,
  };

  const finalInputStarted = performance.now();
  driver.send({ type: "show" });
  await waitForKindCount(driver, "main", 1, 2_000);
  await driver.setFilterAndWait("mwnd15-final", { timeoutMs: 5_000 });
  const finalInputMs = performance.now() - finalInputStarted;
  await driver.setFilterAndWait("", { timeoutMs: 5_000 });
  receipt.finalRecovery = {
    pass: finalInputMs <= 500,
    inputEchoMs: Number(finalInputMs.toFixed(2)),
  };

  const logText = await Bun.file(driver.logPath).text();
  const crashMarkers = logText
    .split("\n")
    .filter((line) => /panicked at|fatal runtime error|abort trap|segmentation fault/i.test(line))
    .slice(0, 20);
  receipt.crashScan = { pass: crashMarkers.length === 0, markers: crashMarkers };
  receipt.driverStats = driver.stats;
  receipt.pass = actionsPass
    && notesPass
    && dictationPass
    && receipt.finalRecovery.pass
    && receipt.crashScan.pass;
} catch (error) {
  receipt.error = String(error);
  receipt.pass = false;
} finally {
  if (interferenceMonitor) {
    receipt.interference = await finishInterferenceMonitor(interferenceMonitor);
    receipt.pass = receipt.pass === true && receipt.interference.pass === true;
  }
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();
  receipt.pass = receipt.pass === true && receipt.cleanedUp === true;
  receipt.disposition = receipt.interference?.disposition === "INVALID_INTERFERENCE"
    ? "INVALID_INTERFERENCE"
    : receipt.pass === true
    ? "EVALUABLE_PASS"
    : receipt.error
    ? "INVALID_OBSERVER"
    : "EVALUABLE_FAIL";
  writeFileSync(outPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receiptPath: outPath, pass: receipt.pass }, null, 2));
}

process.exit(receipt.pass ? 0 : 2);
