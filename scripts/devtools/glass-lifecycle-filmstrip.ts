#!/usr/bin/env bun

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { Driver, type Json } from "./driver.ts";
import {
  identityFromEnvironment,
  newRunId,
} from "./glass-evidence-contract.ts";
import { announceTestStatus } from "./test-status.ts";

function arg(name: string, fallback?: string) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

async function run(command: string[]) {
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { stdout, stderr, exitCode };
}

async function waitForFile(path: string, timeoutMs = 3_000) {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    if (existsSync(path)) return JSON.parse(readFileSync(path, "utf8"));
    await Bun.sleep(10);
  }
  throw new Error(`timed out waiting for ${path}`);
}

async function waitForWindowCount(
  driver: Driver,
  kind: string,
  expected: number,
  timeoutMs = 3_000,
) {
  const started = performance.now();
  let last: Json = null;
  while (performance.now() - started < timeoutMs) {
    last = await driver.listAutomationWindows({ timeoutMs: 5_000 });
    const count = ((last?.windows ?? []) as Json[]).filter(
      (window) => window?.kind === kind && window?.visible !== false,
    ).length;
    if (count === expected) return { pass: true, count, snapshot: last };
    await Bun.sleep(15);
  }
  const count = ((last?.windows ?? []) as Json[]).filter(
    (window) => window?.kind === kind && window?.visible !== false,
  ).length;
  return { pass: false, count, snapshot: last };
}

async function nativeWindowIds(pid: number, title?: string) {
  const query = await run([
    "swift",
    resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
    "--pid",
    String(pid),
  ]);
  if (query.exitCode !== 0) {
    return {
      ids: [] as number[],
      windows: [] as Json[],
      completeWindows: [] as Json[],
      completeWindowIds: [] as number[],
      includesHiddenAndAlphaZero: true,
      error: query.stderr.trim(),
    };
  }
  const parsed = JSON.parse(query.stdout);
  const allWindows = (parsed.windows ?? [])
    .filter((window: Json) =>
      Number(window?.windowId ?? 0) > 0
      && Number(window?.bounds?.width ?? 0) > 1
      && Number(window?.bounds?.height ?? 0) > 1
    );
  const matching = allWindows
      .filter((window: Json) =>
        window?.onscreen === true
        && Number(window?.alpha ?? 0) > 0
        && (title == null || window?.title === title)
      )
      .sort((left: Json, right: Json) =>
        Number(right?.bounds?.width ?? 0) * Number(right?.bounds?.height ?? 0)
        - Number(left?.bounds?.width ?? 0) * Number(left?.bounds?.height ?? 0)
      );
  return {
    ids: matching.map((window: Json) => Number(window.windowId)),
    windows: matching,
    completeWindows: allWindows,
    completeWindowIds: allWindows.map((window: Json) => Number(window.windowId)),
    includesHiddenAndAlphaZero: true,
    error: null,
  };
}

function analyzeFilmstrip(directory: string, expectedWindowID: number) {
  const receiptPath = join(directory, "receipt.json");
  const receipt = existsSync(receiptPath)
    ? JSON.parse(readFileSync(receiptPath, "utf8"))
    : null;
  const frames = (receipt?.frames ?? []) as Json[];
  const hashes = new Set(
    frames.map((frame) => String(frame?.sha256 ?? "")).filter(Boolean),
  );
  const errors = [
    ...(receipt?.errors ?? []),
    ...(receipt?.windowID === expectedWindowID
      ? []
      : [
        `pinned CGWindowID changed: expected ${expectedWindowID}, got ${
          receipt?.windowID ?? "missing"
        }`,
      ]),
    ...(frames.length >= 4 ? [] : [`only ${frames.length}/4 complete frames`]),
    ...(hashes.size >= 2 ? [] : ["filmstrip contains fewer than two visual states"]),
  ];
  return {
    receiptPath,
    receipt,
    frameCount: frames.length,
    distinctFrameHashes: hashes.size,
    pinnedWindowID: expectedWindowID,
    errors,
    pass: errors.length === 0,
  };
}

const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!);
const outDir = resolve(
  arg("--out", ".artifacts/glass-motion-contrast/lifecycle-filmstrips")!,
);
if (!binary || !existsSync(binary)) {
  throw new Error(`binary missing: ${binary || "<unset>"}`);
}
mkdirSync(outDir, { recursive: true });
const helper = join(outDir, "macos-native-window-filmstrip");
const compile = await run([
  "xcrun",
  "swiftc",
  "-parse-as-library",
  "-O",
  resolve(import.meta.dir, "../agentic/macos-native-window-filmstrip.swift"),
  "-o",
  helper,
]);
if (compile.exitCode !== 0) {
  throw new Error(`filmstrip helper compile failed: ${compile.stderr}`);
}

const receipt: Json = {
  schemaVersion: 2,
  startedAt: new Date().toISOString(),
  ...identityFromEnvironment({
    runId: newRunId(),
    gitCommit: (await run(["git", "rev-parse", "HEAD"])).stdout.trim(),
    binary,
    binarySha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
  }),
  scenario: process.env.SCRIPT_KIT_GLASS_SCENARIO ?? "lifecycle",
  helperSha256: createHash("sha256").update(readFileSync(helper)).digest("hex"),
  scenarios: [],
  pass: false,
};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  sessionName: `glass-lifecycle-filmstrip-${process.pid}`,
  defaultTimeoutMs: 8_000,
});

function startFilmstrip(
  name: string,
  selector: { windowID?: number; pid?: number; title?: string },
  durationMs: number,
) {
  const directory = join(outDir, name);
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
  const readyPath = join(directory, "ready.json");
  const command = [
    helper,
    ...(selector.windowID != null
      ? ["--window-id", String(selector.windowID)]
      : [
        "--pid",
        String(selector.pid),
        ...(selector.title ? ["--title", selector.title] : []),
      ]),
    "--out",
    directory,
    "--ready",
    readyPath,
    "--duration-ms",
    String(durationMs),
    "--fps",
    "120",
  ];
  const process = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  return {
    name,
    directory,
    readyPath,
    process,
    command,
    processStartedAt: new Date().toISOString(),
  };
}

async function finishFilmstrip(
  started: ReturnType<typeof startFilmstrip>,
  expectedWindowID: number,
) {
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(started.process.stdout).text(),
    new Response(started.process.stderr).text(),
    started.process.exited,
  ]);
  const metricsPath = join(started.directory, "metrics.json");
  const metricsResult = await run([
    "python3",
    resolve(import.meta.dir, "../agentic/glass-lifecycle-metrics.py"),
    "--receipt",
    join(started.directory, "receipt.json"),
    "--scenario",
    started.name,
    "--out",
    metricsPath,
  ]);
  const metrics = existsSync(metricsPath)
    ? JSON.parse(readFileSync(metricsPath, "utf8"))
    : null;
  return {
    command: started.command,
    exitCode,
    stderr: stderr.trim().slice(-1_000),
    stdout: stdout.trim().slice(-1_000),
    metricsPath,
    metricsExitCode: metricsResult.exitCode,
    metrics,
    ...analyzeFilmstrip(started.directory, expectedWindowID),
    pass: analyzeFilmstrip(started.directory, expectedWindowID).pass
      && metricsResult.exitCode === 0
      && metrics?.pass === true,
  };
}

async function notesState() {
  try {
    const state = await driver.getTargetState(
      { type: "id", id: "notes" },
      { timeoutMs: 5_000 },
    );
    return state?.notes ?? state ?? null;
  } catch {
    return null;
  }
}

try {
  receipt.pid = driver.pid ?? null;
  receipt.sessionDir = driver.sessionDir;
  driver.send({ type: "show" });
  await waitForWindowCount(driver, "main", 1, 5_000);
  await driver.waitForSettle({ timeoutMs: 5_000 });
  const pid = Number(driver.pid);
  const mainNative = await nativeWindowIds(pid);
  const mainWindowID = mainNative.ids[0];
  if (!mainWindowID) throw new Error(`main native window missing: ${mainNative.error}`);
  receipt.initialCompleteNativeInventory = mainNative;

  await announceTestStatus(
    "Lifecycle filmstrip · Main exit",
    "Exact CGWindowID capture while the main surface and detached capsules fade together",
  );
  const mainExit = startFilmstrip("main-exit", { windowID: mainWindowID }, 650);
  await waitForFile(mainExit.readyPath);
  await Bun.sleep(80);
  driver.send({ type: "hide", requestId: "glass-life-main-hide" });
  await driver.waitForState({ windowVisible: false }, { timeoutMs: 3_000 });
  const mainExitFilmstrip = await finishFilmstrip(mainExit, mainWindowID);
  (receipt.scenarios as Json[]).push({
    name: "main-exit",
    exactWindowID: mainWindowID,
    filmstrip: mainExitFilmstrip,
    pass: mainExitFilmstrip.pass,
  });
  await announceTestStatus(
    "Lifecycle filmstrip · Main entry",
    "Observer starts while hidden; the same CGWindowID and detached gutter emerge together",
  );
  const mainEntry = startFilmstrip("main-entry", { windowID: mainWindowID }, 700);
  const showRequestedAt = new Date().toISOString();
  driver.send({ type: "show", requestId: "glass-life-main-show" });
  const mainEntryReady = await waitForFile(mainEntry.readyPath);
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 3_000 });
  const mainEntryFilmstrip = await finishFilmstrip(mainEntry, mainWindowID);
  (receipt.scenarios as Json[]).push({
    name: "main-entry",
    exactWindowID: mainWindowID,
    observerStartedAt: mainEntry.processStartedAt,
    showRequestedAt,
    streamReady: mainEntryReady,
    filmstrip: mainEntryFilmstrip,
    pass: mainEntryFilmstrip.pass,
  });

  await announceTestStatus(
    "Lifecycle filmstrip · Notes entry",
    "Body remains hidden while the exact native Notes window material settles",
  );
  const notesEntry = startFilmstrip(
    "notes-entry",
    { pid, title: "Notes" },
    800,
  );
  driver.send({ type: "openNotes", requestId: "glass-life-notes-entry-open" });
  const notesEntryReady = await waitForFile(notesEntry.readyPath);
  const notesEntryID = Number(notesEntryReady.windowID);
  const entryStates: Json[] = [];
  for (const elapsedMs of [0, 30, 80, 180, 320]) {
    if (elapsedMs > 0) await Bun.sleep(elapsedMs - Number(entryStates.at(-1)?.elapsedMs ?? 0));
    entryStates.push({ elapsedMs, state: await notesState() });
  }
  const notesEntryFilmstrip = await finishFilmstrip(notesEntry, notesEntryID);
  const configuredState = entryStates.find(
    (sample) => sample?.state?.entryReveal?.nativeConfigured === true,
  )?.state?.entryReveal;
  const hiddenBeforeVisible = entryStates.some(
    (sample) =>
      sample?.state?.entryReveal?.nativeConfigured === true
      && sample?.state?.entryReveal?.bodyVisible === false,
  );
  const visibleAfterSettle = entryStates.at(-1)?.state?.entryReveal?.bodyVisible === true;
  const finalReveal = entryStates.at(-1)?.state?.entryReveal;
  const framesBeforeVisible = (
    notesEntryFilmstrip.receipt?.frames ?? []
  ).filter((frame: Json) =>
    typeof frame?.displayTimeNs === "number"
    && typeof finalReveal?.visibleAtMonotonicNs === "number"
    && frame.displayTimeNs <= finalReveal.visibleAtMonotonicNs
  ).length;
  const notesEntryPass = notesEntryFilmstrip.pass
    && configuredState?.nativeWindowNumber === notesEntryID
    && configuredState?.backdropFoundOrCreated === true
    && configuredState?.nativeSelectorsSupported === true
    && configuredState?.styleApplied === true
    && configuredState?.fallbackUsed === false
    && typeof configuredState?.configuredAtMonotonicNs === "number"
    && typeof configuredState?.settleDurationMs === "number"
    && hiddenBeforeVisible
    && visibleAfterSettle
    && Number(finalReveal?.completedFrameCount ?? 0) >= 2
    && notesEntryFilmstrip.metrics?.bodyPixelTransition === true;
  (receipt.scenarios as Json[]).push({
    name: "notes-entry",
    exactWindowID: notesEntryID,
    states: entryStates,
    bodyOnlyReveal: {
      hiddenBeforeVisible,
      visibleAfterSettle,
      framesBeforeVisible,
      completedFrameCount: finalReveal?.completedFrameCount ?? null,
      bodyPixelTransition: notesEntryFilmstrip.metrics?.bodyPixelTransition ?? false,
    },
    nativeConfiguration: configuredState ?? null,
    filmstrip: notesEntryFilmstrip,
    pass: notesEntryPass,
  });
  driver.send({ type: "openNotes", requestId: "glass-life-notes-entry-close" });
  await waitForWindowCount(driver, "notes", 0, 3_000);

  await announceTestStatus(
    "Lifecycle filmstrip · Notes cancel/reopen",
    "Close before settle, reopen during the fade, and reuse the same CGWindowID",
  );
  const notesReopen = startFilmstrip(
    "notes-close-before-settle-reopen",
    { pid, title: "Notes" },
    950,
  );
  driver.send({ type: "openNotes", requestId: "glass-life-notes-reopen-open" });
  const notesReopenReady = await waitForFile(notesReopen.readyPath);
  const notesReopenID = Number(notesReopenReady.windowID);
  await Bun.sleep(25);
  const beforeClose = await notesState();
  driver.send({ type: "openNotes", requestId: "glass-life-notes-reopen-close" });
  await Bun.sleep(25);
  const duringExit = await notesState();
  driver.send({ type: "openNotes", requestId: "glass-life-notes-reopen-cancel-exit" });
  await Bun.sleep(360);
  const afterReopen = await notesState();
  const notesAfterNative = await nativeWindowIds(pid, "Notes");
  const notesCompleteAfter = await nativeWindowIds(pid);
  const notesReopenFilmstrip = await finishFilmstrip(notesReopen, notesReopenID);
  const notesReopenPass = notesReopenFilmstrip.pass
    && duringExit?.windowLifecycle?.phase === "Exiting"
    && duringExit?.windowLifecycle?.hasExitTicket === true
    && typeof duringExit?.windowLifecycle?.exitGeneration === "number"
    && afterReopen?.windowLifecycle?.phase === "Open"
    && afterReopen?.windowLifecycle?.hasExitTicket === false
    && afterReopen?.entryReveal?.bodyVisible === true
    && notesAfterNative.ids.length === 1
    && notesAfterNative.ids[0] === notesReopenID
    && notesCompleteAfter.completeWindowIds.includes(mainWindowID)
    && notesCompleteAfter.completeWindowIds.includes(notesReopenID);
  (receipt.scenarios as Json[]).push({
    name: "notes-close-before-settle-reopen",
    exactWindowID: notesReopenID,
    beforeClose,
    duringExit,
    afterReopen,
    nativeWindowIdsAfterReopen: notesAfterNative.ids,
    completeNativeInventoryAfterReopen: notesCompleteAfter,
    filmstrip: notesReopenFilmstrip,
    pass: notesReopenPass,
  });
  driver.send({ type: "openNotes", requestId: "glass-life-notes-reopen-final-close" });
  await waitForWindowCount(driver, "notes", 0, 3_000);

  await announceTestStatus(
    "Lifecycle filmstrip · Dictation cancel/reopen",
    "Fixture-only overlay; Escape starts fade, reopen cancels its ticket without microphone capture",
  );
  const dictation = startFilmstrip(
    "dictation-exit-reopen",
    { pid, title: "Script Kit Dictation" },
    900,
  );
  driver.send({
    type: "openDictationOverlayFixture",
    requestId: "glass-life-dictation-open",
  });
  const dictationReady = await waitForFile(dictation.readyPath);
  const dictationID = Number(dictationReady.windowID);
  await Bun.sleep(120);
  const dictationBefore = (await driver.getState({ timeoutMs: 5_000 }))?.dictation;
  const dictationConfirmDispatch = await driver.simulateGpuiKeyDown("escape", {
    target: { type: "id", id: "dictation" },
    timeoutMs: 5_000,
  });
  await Bun.sleep(25);
  const dictationDiscardDispatch = await driver.simulateGpuiKeyDown("backspace", {
    target: { type: "id", id: "dictation" },
    timeoutMs: 5_000,
  });
  await Bun.sleep(25);
  const dictationDuringExit = (await driver.getState({ timeoutMs: 5_000 }))?.dictation;
  driver.send({
    type: "openDictationOverlayFixture",
    requestId: "glass-life-dictation-reopen",
  });
  await Bun.sleep(300);
  const dictationAfter = (await driver.getState({ timeoutMs: 5_000 }))?.dictation;
  const dictationNativeAfter = await nativeWindowIds(pid, "Script Kit Dictation");
  const dictationCompleteAfter = await nativeWindowIds(pid);
  const dictationFilmstrip = await finishFilmstrip(dictation, dictationID);
  const dictationPass = dictationFilmstrip.pass
    && dictationDuringExit?.windowLifecycle?.phase === "Exiting"
    && dictationDuringExit?.windowLifecycle?.handleRegistered === true
    && dictationDuringExit?.windowLifecycle?.automationRegistered === true
    && typeof dictationDuringExit?.windowLifecycle?.exitGeneration === "number"
    && dictationAfter?.windowLifecycle?.phase === "Open"
    && dictationAfter?.windowLifecycle?.hasExitTicket === false
    && dictationNativeAfter.ids.length === 1
    && dictationNativeAfter.ids[0] === dictationID
    && dictationCompleteAfter.completeWindowIds.includes(mainWindowID)
    && dictationCompleteAfter.completeWindowIds.includes(dictationID);
  (receipt.scenarios as Json[]).push({
    name: "dictation-exit-reopen",
    exactWindowID: dictationID,
    before: dictationBefore,
    confirmDispatch: dictationConfirmDispatch,
    discardDispatch: dictationDiscardDispatch,
    duringExit: dictationDuringExit,
    afterReopen: dictationAfter,
    nativeWindowIdsAfterReopen: dictationNativeAfter.ids,
    completeNativeInventoryAfterReopen: dictationCompleteAfter,
    noMicrophoneCapture: true,
    filmstrip: dictationFilmstrip,
    pass: dictationPass,
  });
  await driver.simulateGpuiKeyDown("escape", {
    target: { type: "id", id: "dictation" },
    timeoutMs: 5_000,
  }).catch(() => null);
  await Bun.sleep(25);
  await driver.simulateGpuiKeyDown("backspace", {
    target: { type: "id", id: "dictation" },
    timeoutMs: 5_000,
  }).catch(() => null);
  await waitForWindowCount(driver, "dictation", 0, 3_000);

  receipt.requiredScenarioNames = [
    "main-exit",
    "main-entry",
    "notes-entry",
    "notes-close-before-settle-reopen",
    "dictation-exit-reopen",
  ];
  const scenarios = receipt.scenarios as Json[];
  receipt.pass = scenarios.length === 5
    && receipt.requiredScenarioNames.every((name: string) =>
      scenarios.some((scenario) => scenario?.name === name)
    )
    && scenarios.every((scenario) => scenario?.pass === true);
} catch (error) {
  receipt.error = String(error);
  receipt.pass = false;
} finally {
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();
  receipt.pass = receipt.pass === true && receipt.cleanedUp === true;
  receipt.disposition = receipt.pass === true
    ? "EVALUABLE_PASS"
    : receipt.error
    ? "INVALID_OBSERVER"
    : "EVALUABLE_FAIL";
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receiptPath, pass: receipt.pass }, null, 2));
}

process.exit(receipt.pass ? 0 : 2);
