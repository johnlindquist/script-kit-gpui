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
import { requireValidatedHelper } from "./glass-native-helper-cache.ts";
import {
  classifyNativeInventory,
  deriveUniqueOwnerDelta,
} from "./glass-topology-contract.ts";
import {
  finishInterferenceMonitor,
  startInterferenceMonitor,
  waitForInterferenceReady,
} from "./glass-interference.ts";
import { exitCodeForDisposition } from "./glass-observers.ts";

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
  // C08: prefer the prepared hash-validated window-query helper; standalone
  // full-profile runs keep the legacy source interpretation.
  const child = Bun.spawn(
    windowQueryHelper
      ? [windowQueryHelper, "--pid", String(pid)]
      : [
        "swift",
        resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
        "--pid",
        String(pid),
      ],
    {
      stdout: "pipe",
      stderr: "pipe",
    },
  );
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
const themeFixture = arg("--theme-fixture");
const outPath = resolve(
  arg(
    "--out",
    ".artifacts/main-window-native-drag/mwnd15-rapid-toggle/receipt.json",
  )!,
);
if (!binary || !existsSync(binary)) {
  throw new Error(`binary missing: ${binary || "<unset>"}`);
}
// C08: backward-compatible probe profiles. `full` is the existing
// Actions + Notes + Dictation behavior; `pf011` (the PF-011 aggregator
// profile) runs Actions + Notes only and REQUIRES the named theme fixture
// and prepared helper manifests instead of interpreting Swift source.
const RAPID_TOGGLE_PROFILES: Record<string, string[]> = {
  full: ["actions", "notes", "dictation"],
  pf011: ["actions", "notes"],
};
const profile = arg("--profile", "full") ?? "full";
const requiredPhaseNames = RAPID_TOGGLE_PROFILES[profile];
if (!requiredPhaseNames) {
  console.error(
    `argument error: unknown profile "${profile}" (expected full | pf011)`,
  );
  process.exit(64);
}
if (profile === "pf011" && !themeFixture) {
  console.error("argument error: --profile pf011 requires --theme-fixture");
  process.exit(64);
}
const suppliedWindowQueryHelper = arg("--window-query-helper");
if (profile === "pf011" && !suppliedWindowQueryHelper) {
  console.error(
    "argument error: --profile pf011 requires a validated --window-query-helper",
  );
  process.exit(64);
}
mkdirSync(dirname(outPath), { recursive: true });
// WP4 (glass-smoke-harness-max-info): accept a pre-compiled hash-validated
// interference helper from the study orchestrator; compile only when absent.
// C08: a malformed SUPPLIED helper keeps its original INVALID_SETUP stderr
// diagnostic but writes an INVALID_OBSERVER receipt (exit 4) instead of an
// unclassified crash.
const suppliedInterferenceHelper = arg("--interference-helper");
let interferenceHelper: string;
let windowQueryHelper: string | null = null;
try {
  if (suppliedInterferenceHelper) {
    interferenceHelper = requireValidatedHelper(
      suppliedInterferenceHelper,
      "interference",
    ).binaryPath;
  } else {
    interferenceHelper = join(
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
  }
  if (suppliedWindowQueryHelper) {
    windowQueryHelper = requireValidatedHelper(
      suppliedWindowQueryHelper,
      "window-query",
    ).binaryPath;
  }
} catch (error) {
  const message = String(error instanceof Error ? error.message : error);
  console.error(message);
  writeFileSync(
    outPath,
    `${
      JSON.stringify(
        {
          schemaVersion: 2,
          scenario: process.env.SCRIPT_KIT_GLASS_SCENARIO ?? "rapid-toggle",
          profile,
          helperValidation: { error: message },
          disposition: "INVALID_OBSERVER",
          pass: false,
          finishedAt: new Date().toISOString(),
        },
        null,
        2,
      )
    }\n`,
  );
  process.exit(4);
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
  profile,
  requiredPhaseNames,
  executedPhaseNames: [] as string[],
  helperIdentities: {
    interference: suppliedInterferenceHelper
      ? {
        binaryPath: interferenceHelper,
        binarySha256: createHash("sha256")
          .update(readFileSync(interferenceHelper))
          .digest("hex"),
      }
      : { binaryPath: interferenceHelper, compiledPerRun: true },
    windowQuery: windowQueryHelper
      ? {
        binaryPath: windowQueryHelper,
        binarySha256: createHash("sha256")
          .update(readFileSync(windowQueryHelper))
          .digest("hex"),
      }
      : null,
  },
  themeFixture: themeFixture
    ? {
      path: resolve(themeFixture),
      sha256: createHash("sha256")
        .update(readFileSync(resolve(themeFixture)))
        .digest("hex"),
    }
    : null,
  phases: {},
  pass: false,
};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  themeFixturePath: themeFixture,
  sessionName: "mwnd15-rapid-toggle",
  defaultTimeoutMs: 8_000,
});

try {
  receipt.pid = driver.pid ?? null;
  receipt.sessionDir = driver.sessionDir;
  driver.send({ type: "show" });
  const mainVisible = await waitForKindCount(driver, "main", 1, 5_000);
  const initialNativeInventory = await nativeWindowInventory(Number(driver.pid));
  // Preserve the complete inventory even when validation fails so an observer
  // defect remains diagnosable instead of collapsing to an error string.
  receipt.initialNativeInventory = initialNativeInventory;
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
  receipt.interferenceMonitorPid = interferenceMonitor.process.pid;
  // C08: observer defects (query failures, missing topology/runtime state)
  // are collected separately from product failures so the final disposition
  // can distinguish INVALID_OBSERVER from EVALUABLE_FAIL.
  receipt.observerErrors = [] as string[];
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
  let maxVisibleNativeActionWindows = 0;
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
    let native = await nativeWindowInventory(
      Number(driver.pid),
      undefined,
      mainWindowId,
    );
    // Deferred GPUI dispatch can expose AppKit's just-created 0×0 shell for a
    // few milliseconds before popup configuration assigns its final frame.
    // That transient is not evaluable geometry, so retry only that explicit
    // observer state; all other topology failures remain immediate failures.
    for (let retry = 0; retry < 5 && native.topology?.errors?.some(
      (error: string) => error.includes("unknown or stale same-PID native window"),
    ); retry += 1) {
      await Bun.sleep(10);
      native = await nativeWindowInventory(
        Number(driver.pid),
        undefined,
        mainWindowId,
      );
    }
    const actionOwners = native.topology?.rows?.filter(
      (window: Json) => window?.classification === "Actions",
    ) ?? [];
    maxNativeActionWindows = Math.max(maxNativeActionWindows, actionOwners.length);
    maxVisibleNativeActionWindows = Math.max(
      maxVisibleNativeActionWindows,
      actionOwners.filter((window: Json) => window?.onscreen === true).length,
    );
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
  // Popup exit deliberately retains a hidden native tail for the calibrated
  // 135 ms removal delay. It is not a second visible popup, but it must still
  // disappear after that bounded lifecycle completes.
  await Bun.sleep(250);
  const settledActionNativeInventory = await nativeWindowInventory(
    Number(driver.pid),
    undefined,
    mainWindowId,
  );
  const settledActionOwners = settledActionNativeInventory.topology?.rows?.filter(
    (window: Json) => window?.classification === "Actions",
  ) ?? [];
  const actionsErrors = boundedErrors(
    await driver.getLogs({ limit: 500, level: "error" }),
    baselineErrorCount,
  );
  const actionsPass = mainVisible.pass
    && actionDispatches.every((dispatch) => dispatch.success)
    && maxActionWindows <= 1
    && maxVisibleNativeActionWindows <= 1
    && settledActionOwners.length === 0
    && settledActionNativeInventory.topology?.pass === true
    && actionDispatches.every((dispatch) => {
      const topology = dispatch.nativeTopology;
      if (topology?.pass === true) return true;
      const errors = topology?.errors ?? [];
      const owners = topology?.rows?.filter(
        (window: Json) => window?.classification === "Actions",
      ) ?? [];
      const visibleOwners = owners.filter((window: Json) => window?.onscreen === true);
      const hiddenOwners = owners.filter((window: Json) => window?.onscreen !== true);
      return errors.length === 1
        && errors[0] === "Actions has 2 complete native owners"
        && owners.length === 2
        && visibleOwners.length <= 1
        && hiddenOwners.every((window: Json) => Number(window?.alpha) === 0);
    })
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
    maxVisibleNativeActionWindows,
    settledNativeActionWindows: settledActionOwners.length,
    settledNativeTopology: settledActionNativeInventory.topology,
    actionsOpenAfterBurst,
    inputRecoveryMs: Number(inputRecoveryMs.toFixed(2)),
    deliberateOpenMs: Number(deliberateOpenMs.toFixed(2)),
    deliberateCloseMs: Number(deliberateCloseMs.toFixed(2)),
    errors: actionsErrors,
  };
  (receipt.executedPhaseNames as string[]).push("actions");
  (receipt.observerErrors as string[]).push(
    ...actionDispatches
      .filter((dispatch: Json) => dispatch?.nativeTopology == null)
      .map((dispatch: Json) =>
        `actions pulse ${dispatch?.index}: native topology query missing`
      ),
  );

  await announceTestStatus(
    "MWND-15B · Notes hammer",
    "16 immediate Notes toggles, reopen recovery, then real Cmd+W close",
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
  const noteReveals = [
    ...notesSamples.map((sample) => sample?.notesState?.entryReveal),
    duringNotesExit?.entryReveal,
    revealAfterReopen?.entryReveal,
  ].filter((reveal) => reveal != null);
  const notesRevealGenerations = [
    ...new Set(
      noteReveals
        .map((reveal) => Number(reveal?.generation))
        .filter(Number.isFinite),
    ),
  ];
  // Generation numbers are local to a Notes window instance. Rapid close/open
  // pulses can construct several instances that each begin at generation 1,
  // so a generation-only set collapses real independent reveals into one and
  // produces a false failure. The composite identity proves that at least two
  // distinct reveal lifecycles ran without weakening any timing or topology
  // requirement.
  const notesRevealInstances = [
    ...new Set(
      noteReveals
        .map((reveal) => {
          const instanceId = Number(reveal?.instanceId);
          const generation = Number(reveal?.generation);
          return Number.isFinite(instanceId) && Number.isFinite(generation)
            ? `${instanceId}:${generation}`
            : null;
        })
        .filter((identity): identity is string => identity != null),
    ),
  ];
  const notesCloseStarted = performance.now();
  const notesShortcutDispatch = await driver.simulateGpuiKeyDown("w", {
    modifiers: ["cmd"],
    target: { type: "kind", kind: "notes", index: 0 },
    timeoutMs: 5_000,
  });
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
    && notesRevealInstances.length >= 2
    && duringNotesExit?.windowLifecycle?.phase === "Exiting"
    && duringNotesExit?.windowLifecycle?.hasExitTicket === true
    && typeof duringNotesExit?.windowLifecycle?.exitGeneration === "number"
    && notesReopened.pass
    && reusedNativeWindow
    && notesOwnerDelta.pass
    && beforeTail.topology?.pass === true
    && afterTail.topology?.pass === true
    && revealAfterReopen?.entryReveal?.bodyVisible === true
    && notesShortcutDispatch?.success === true
    && notesShortcutDispatch?.dispatchPath === "exact_handle"
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
    cmdWClose: {
      dispatch: notesShortcutDispatch,
      closed: notesClose,
    },
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
    revealInstances: notesRevealInstances,
    errors: notesErrors,
  };
  (receipt.executedPhaseNames as string[]).push("notes");
  (receipt.observerErrors as string[]).push(
    ...notesSamples
      .filter((sample: Json) => sample?.nativeWindowError != null)
      .map((sample: Json) =>
        `notes pulse ${sample?.index}: native window query failed: ${sample?.nativeWindowError}`
      ),
    ...notesSamples
      .filter((sample: Json) => sample?.nativeTopology == null)
      .map((sample: Json) =>
        `notes pulse ${sample?.index}: native topology query missing`
      ),
  );

  // C08: the pf011 profile supports the PF-011 aggregator with Actions +
  // Notes only; Dictation remains part of the default full profile.
  let dictationPass = true;
  if (profile === "full") {
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

  dictationPass = maxDictationWindows <= 1
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
  (receipt.executedPhaseNames as string[]).push("dictation");
  (receipt.observerErrors as string[]).push(
    ...dictationSamples
      .filter((sample: Json) => sample?.nativeWindowError != null)
      .map((sample: Json) =>
        `dictation pulse ${sample?.index}: native window query failed: ${sample?.nativeWindowError}`
      ),
  );
  }

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
    && receipt.requiredPhaseNames.every((name: string) =>
      (receipt.executedPhaseNames as string[]).includes(name)
    )
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
  // C08 disposition contract: interference (from a VALID monitor) dominates;
  // observer defects (caught error, query/topology/runtime-state gaps,
  // failed cleanup) are INVALID_OBSERVER; only a valid lifecycle observation
  // with a bad product outcome is EVALUABLE_FAIL. Pass derives from the
  // final disposition alone.
  const observerDefects = [
    ...((receipt.observerErrors as string[] | undefined) ?? []),
    ...(receipt.error ? [String(receipt.error)] : []),
    ...(receipt.cleanedUp === true ? [] : ["app process survived driver close"]),
    ...(receipt.interference?.receipt != null
        && receipt.interference?.exitCode === 0
      ? []
      : ["interference monitor did not produce a valid receipt"]),
  ];
  receipt.observerDefects = observerDefects;
  receipt.disposition = receipt.interference?.receipt != null
      && receipt.interference?.disposition === "INVALID_INTERFERENCE"
    ? "INVALID_INTERFERENCE"
    : observerDefects.length > 0
    ? "INVALID_OBSERVER"
    : receipt.pass === true
    ? "EVALUABLE_PASS"
    : "EVALUABLE_FAIL";
  receipt.pass = receipt.disposition === "EVALUABLE_PASS";
  writeFileSync(outPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(
    JSON.stringify(
      { receiptPath: outPath, pass: receipt.pass, disposition: receipt.disposition },
      null,
      2,
    ),
  );
}

process.exit(exitCodeForDisposition(String(receipt.disposition)));
