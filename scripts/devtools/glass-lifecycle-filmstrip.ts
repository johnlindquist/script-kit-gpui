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
import {
  computeLifecycleDisposition,
  expandCaptureBounds,
  filmstripVerdict,
  type LifecycleScenarioName,
  parseAnalysisMode,
  resolveScenarioNames,
  SCENARIO_CAPTURE_DURATIONS_MS,
  type ScenarioProfile,
  type ScenarioTimingInterval,
  validateDetachedExitLifecycle,
  validateFilmstripCapture,
  validateScenarioTimingIntervals,
} from "./glass-lifecycle-filmstrip-contract.ts";
import { analyzeEntryMotionEnvelope } from "./glass-entry-motion-contract.ts";
import {
  finishInterferenceMonitor,
  startInterferenceMonitor,
  waitForInterferenceReady,
} from "./glass-interference.ts";
import { classifyNativeInventory } from "./glass-topology-contract.ts";

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
    ...validateFilmstripCapture(receipt, {
      runId: String(receiptRoot.runId),
      gitCommit: String(receiptRoot.gitCommit),
      binarySha256: String(receiptRoot.binarySha256),
      pid: Number(driver.pid),
      windowId: expectedWindowID,
    }),
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
const themeFixture = arg("--theme-fixture");
const outDir = resolve(
  arg("--out", ".artifacts/glass-motion-contrast/lifecycle-filmstrips")!,
);
const scenarioProfile = (arg("--profile", "full") ?? "full") as ScenarioProfile;
const resolvedScenarioNames = resolveScenarioNames(scenarioProfile);
const analysisMode = parseAnalysisMode(arg("--analysis-mode", "inline"));
if (!binary || !existsSync(binary)) {
  throw new Error(`binary missing: ${binary || "<unset>"}`);
}
mkdirSync(outDir, { recursive: true });
const scriptStartedMs = performance.now();
const timingMs: Record<string, number> = {
  helperPreparation: 0,
  driverLaunchToMainVisible: 0,
  normalization: 0,
  pointerPreparation: 0,
  captureTotal: 0,
  inlineAnalysisTotal: 0,
  cleanup: 0,
  total: 0,
};
const helperPreparationStartedMs = performance.now();
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
const interferenceHelper = join(outDir, "macos-glass-interference-monitor");
const interferenceCompile = await run([
  "xcrun",
  "swiftc",
  "-O",
  resolve(import.meta.dir, "../agentic/macos-glass-interference-monitor.swift"),
  "-o",
  interferenceHelper,
]);
if (interferenceCompile.exitCode !== 0) {
  throw new Error(
    `interference helper compile failed: ${interferenceCompile.stderr}`,
  );
}
timingMs.helperPreparation = performance.now() - helperPreparationStartedMs;
let interferenceMonitor: ReturnType<typeof startInterferenceMonitor> | null = null;

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
  scenarioProfile,
  analysisMode,
  requestedScenarioNames: resolvedScenarioNames,
  themeFixture: themeFixture
    ? {
      path: resolve(themeFixture),
      sha256: createHash("sha256")
        .update(readFileSync(resolve(themeFixture)))
        .digest("hex"),
    }
    : null,
  helperSha256: createHash("sha256").update(readFileSync(helper)).digest("hex"),
  scenarios: [],
  pass: false,
};
const receiptRoot = receipt;

const driverLaunchStartedMs = performance.now();
const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  themeFixturePath: themeFixture,
  sessionName: `glass-lifecycle-filmstrip-${process.pid}`,
  defaultTimeoutMs: 8_000,
  env: {
    SCRIPT_KIT_FIDELITY_CAPTURE: "agent-chat",
  },
});

function startFilmstrip(
  name: LifecycleScenarioName,
  selector: {
    windowID?: number;
    pid?: number;
    title?: string;
    displayStream?: boolean;
    bounds?: { x: number; y: number; width: number; height: number };
  },
) {
  const durationMs = SCENARIO_CAPTURE_DURATIONS_MS[name];
  const directory = join(outDir, name);
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(directory, { recursive: true });
  const readyPath = join(directory, "ready.json");
  const command = [
    helper,
    ...(selector.windowID != null
      ? ["--window-id", String(selector.windowID), "--pid", String(driver.pid)]
      : [
        "--pid",
        String(selector.pid),
        ...(selector.title ? ["--title", selector.title] : []),
      ]),
    ...(selector.displayStream !== false ? ["--display-stream"] : []),
    ...(selector.bounds
      ? [
        "--bounds",
        String(selector.bounds.x),
        String(selector.bounds.y),
        String(selector.bounds.width),
        String(selector.bounds.height),
      ]
      : []),
    "--out",
    directory,
    "--ready",
    readyPath,
    "--duration-ms",
    String(durationMs),
    "--fps",
    "120",
    "--run-id",
    String(receipt.runId),
    "--git-commit",
    String(receipt.gitCommit),
    "--binary-sha256",
    String(receipt.binarySha256),
  ];
  const process = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  return {
    name,
    directory,
    readyPath,
    process,
    command,
    processStartedAt: new Date().toISOString(),
    requestedCaptureDurationMs: durationMs,
    observerStartedMs: performance.now(),
    readyAtMs: null as number | null,
  };
}

/** waitForFile on the observer's ready receipt, stamping observer-ready time. */
async function awaitObserverReady(started: ReturnType<typeof startFilmstrip>) {
  const ready = await waitForFile(started.readyPath);
  started.readyAtMs = performance.now();
  return ready;
}

async function finishFilmstrip(
  started: ReturnType<typeof startFilmstrip>,
  expectedWindowID: number,
  metricsContext?: {
    bodyBounds?: { x: number; y: number; width: number; height: number };
    hiddenHostTimeNs?: number;
    visibleHostTimeNs?: number;
    expectedExitFrame?: [number, number, number, number];
    captureBounds?: { x: number; y: number; width: number; height: number };
    referenceImagePath?: string;
  },
) {
  const finishCalledMs = performance.now();
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(started.process.stdout).text(),
    new Response(started.process.stderr).text(),
    started.process.exited,
  ]);
  const observerExitedMs = performance.now();
  const metricsPath = join(started.directory, "metrics.json");
  const metricsCommand = [
    "python3",
    resolve(import.meta.dir, "../agentic/glass-lifecycle-metrics.py"),
    "--receipt",
    join(started.directory, "receipt.json"),
    "--scenario",
    started.name,
    "--out",
    metricsPath,
    ...(metricsContext?.bodyBounds
      ? [
        "--body-bounds",
        String(metricsContext.bodyBounds.x),
        String(metricsContext.bodyBounds.y),
        String(metricsContext.bodyBounds.width),
        String(metricsContext.bodyBounds.height),
      ]
      : []),
    ...(metricsContext?.visibleHostTimeNs != null
      ? ["--visible-host-time-ns", String(metricsContext.visibleHostTimeNs)]
      : []),
    ...(metricsContext?.hiddenHostTimeNs != null
      ? ["--hidden-host-time-ns", String(metricsContext.hiddenHostTimeNs)]
      : []),
    ...(metricsContext?.expectedExitFrame != null
      ? [
        "--expected-exit-frame",
        ...metricsContext.expectedExitFrame.map(String),
      ]
      : []),
    ...(metricsContext?.captureBounds != null
      ? [
        "--capture-bounds",
        String(metricsContext.captureBounds.x),
        String(metricsContext.captureBounds.y),
        String(metricsContext.captureBounds.width),
        String(metricsContext.captureBounds.height),
      ]
      : []),
    ...(metricsContext?.referenceImagePath != null
      ? ["--reference-image", metricsContext.referenceImagePath]
      : []),
  ];
  // Deferred mode never invokes Python while a display scenario could still
  // be active: the exact grader command is preserved so the offline finalizer
  // reruns it verbatim against the same hash-bound artifacts.
  let metricsExitCode: number | null = null;
  let metrics: Json | null = null;
  let analysisMs = 0;
  if (analysisMode === "inline") {
    const analysisStartedMs = performance.now();
    const metricsResult = await run(metricsCommand);
    analysisMs = performance.now() - analysisStartedMs;
    timingMs.inlineAnalysisTotal += analysisMs;
    metricsExitCode = metricsResult.exitCode;
    metrics = existsSync(metricsPath)
      ? JSON.parse(readFileSync(metricsPath, "utf8"))
      : null;
  }
  const capture = analyzeFilmstrip(started.directory, expectedWindowID);
  const verdict = filmstripVerdict({
    captureErrorCount: capture.errors.length,
    analysisMode,
    metricsExitCode,
    metricsPass: metrics?.pass === true,
  });
  const finishedMs = performance.now();
  return {
    command: started.command,
    exitCode,
    stderr: stderr.trim().slice(-1_000),
    stdout: stdout.trim().slice(-1_000),
    metricsPath,
    metricsCommand,
    metricsExitCode,
    metrics,
    analysisState: verdict.analysisState,
    ...capture,
    capturePass: verdict.capturePass,
    timing: {
      observerStartToReadyMs: started.readyAtMs == null
        ? null
        : started.readyAtMs - started.observerStartedMs,
      driveMs: started.readyAtMs == null
        ? null
        : finishCalledMs - started.readyAtMs,
      requestedCaptureDurationMs: started.requestedCaptureDurationMs,
      observerFinishMs: observerExitedMs - finishCalledMs,
      analysisMs,
      totalMs: finishedMs - started.observerStartedMs,
    },
    pass: verdict.pass,
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
  timingMs.driverLaunchToMainVisible = performance.now() - driverLaunchStartedMs;
  const normalizationStartedMs = performance.now();
  await announceTestStatus(
    "Lifecycle setup · Normalize main owner",
    "Warm one hide/show lifecycle before pinning the exact native window and bounds",
  );
  driver.send({ type: "hide", requestId: "glass-life-warm-hide" });
  await driver.waitForState({ windowVisible: false }, { timeoutMs: 3_000 });
  await Bun.sleep(220);
  driver.send({ type: "show", requestId: "glass-life-warm-show" });
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 3_000 });
  await driver.waitForSettle({ timeoutMs: 3_000 });
  await Bun.sleep(350);
  const pid = Number(driver.pid);
  const mainNative = await nativeWindowIds(pid);
  const mainWindowID = mainNative.ids[0];
  if (!mainWindowID) throw new Error(`main native window missing: ${mainNative.error}`);
  const mainBounds = mainNative.windows.find(
    (window: Json) => Number(window.windowId) === mainWindowID,
  )?.bounds;
  if (
    !mainBounds
    || !["x", "y", "width", "height"].every(
      (key) => Number.isFinite(Number(mainBounds[key])),
    )
  ) {
    throw new Error(`main native window ${mainWindowID} bounds are missing`);
  }
  receipt.initialCompleteNativeInventory = mainNative;
  const mainCaptureBounds = expandCaptureBounds({
    x: Number(mainBounds.x),
    y: Number(mainBounds.y),
    width: Number(mainBounds.width),
    height: Number(mainBounds.height),
  });
  const initialTopology = classifyNativeInventory(
    mainNative.completeWindows as any[],
    pid,
    mainWindowID,
  );
  receipt.initialNativeTopology = initialTopology;
  if (!initialTopology.pass) {
    throw new Error(
      `initial complete same-PID topology invalid: ${JSON.stringify(initialTopology.errors)}`,
    );
  }
  timingMs.normalization = performance.now() - normalizationStartedMs;
  const pointerPreparationStartedMs = performance.now();
  const pointerPreparation = await run(["cliclick", "m:2,2"]);
  if (pointerPreparation.exitCode !== 0) {
    throw new Error(
      `failed to park pointer away from entry capsules: ${pointerPreparation.stderr}`,
    );
  }
  timingMs.pointerPreparation = performance.now() - pointerPreparationStartedMs;
  interferenceMonitor = startInterferenceMonitor(interferenceHelper, outDir);
  receipt.interferenceReady = await waitForInterferenceReady(interferenceMonitor);

  async function runMainExitScenario() {
  await announceTestStatus(
    "Lifecycle filmstrip · Main exit",
    "Exact CGWindowID capture while the main surface and detached capsules fade together",
  );
  const mainExit = startFilmstrip(
    "main-exit",
    { windowID: mainWindowID, bounds: mainCaptureBounds },
  );
  await awaitObserverReady(mainExit);
  await Bun.sleep(40);
  driver.send({ type: "hide", requestId: "glass-life-main-hide" });
  await driver.waitForState({ windowVisible: false }, { timeoutMs: 3_000 });
  await Bun.sleep(220);
  const mainExitReferencePath = join(mainExit.directory, "hidden-reference.png");
  const mainExitReferenceCapture = await run([
    "screencapture",
    "-x",
    `-R${mainCaptureBounds.x},${mainCaptureBounds.y},${mainCaptureBounds.width},${mainCaptureBounds.height}`,
    mainExitReferencePath,
  ]);
  const mainAfterExit = await nativeWindowIds(pid);
  const exitedOwner = mainAfterExit.completeWindows.find(
    (window: Json) => Number(window?.windowId) === mainWindowID,
  );
  const mainExitReference = {
    path: mainExitReferencePath,
    captureSource: "explicit-post-exit-display-screenshot",
    capturedAt: new Date().toISOString(),
    captureExitCode: mainExitReferenceCapture.exitCode,
    captureStderr: mainExitReferenceCapture.stderr.trim(),
    sha256: mainExitReferenceCapture.exitCode === 0
        && existsSync(mainExitReferencePath)
      ? createHash("sha256")
        .update(readFileSync(mainExitReferencePath))
        .digest("hex")
      : null,
    expectedWindowID: mainWindowID,
    ownerOnscreenAfterExit: exitedOwner?.onscreen ?? false,
    ownerAlphaAfterExit: exitedOwner?.alpha ?? 0,
  };
  const mainExitReferencePass = mainExitReference.captureExitCode === 0
    && /^[a-f0-9]{64}$/.test(String(mainExitReference.sha256 ?? ""))
    && mainExitReference.ownerOnscreenAfterExit === false
    && Number(mainExitReference.ownerAlphaAfterExit) <= 0;
  const mainExitFilmstrip = await finishFilmstrip(
    mainExit,
    mainWindowID,
    {
      captureBounds: mainCaptureBounds,
      referenceImagePath: mainExitReferencePath,
    },
  );
  (receipt.scenarios as Json[]).push({
    name: "main-exit",
    exactWindowID: mainWindowID,
    hiddenReference: mainExitReference,
    hiddenReferencePass: mainExitReferencePass,
    filmstrip: mainExitFilmstrip,
    structuralPass: mainExitFilmstrip.capturePass && mainExitReferencePass,
    pass: mainExitFilmstrip.pass && mainExitReferencePass,
  });
  }

  async function runMainEntryScenario() {
  await announceTestStatus(
    "Lifecycle filmstrip · Main entry",
    "Observer starts while hidden; the same CGWindowID and detached gutter emerge together",
  );
  const mainEntry = startFilmstrip(
    "main-entry",
    { windowID: mainWindowID, displayStream: true, bounds: mainCaptureBounds },
  );
  const mainEntryReady = await awaitObserverReady(mainEntry);
  const mainEntryCaptureBoundsMatch = ["x", "y", "width", "height"].every(
    (key) =>
      Math.abs(
        Number(mainEntryReady?.captureBounds?.[key])
        - Number(mainCaptureBounds[key as keyof typeof mainCaptureBounds]),
      ) <= 0.01,
  );
  await Bun.sleep(40);
  const showRequestedAt = new Date().toISOString();
  driver.send({ type: "show", requestId: "glass-life-main-show" });
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 3_000 });
  await driver.waitForSettle({ timeoutMs: 3_000 });
  const mainEntrySettledLayout = await driver.getLayoutInfo(
    { target: { type: "id", id: "main" } },
    { timeoutMs: 5_000 },
  );
  const settledCaptures: Json[] = [];
  for (let index = 0; index < 3; index += 1) {
    const path = join(
      mainEntry.directory,
      `settled-${String(index).padStart(4, "0")}.png`,
    );
    const capture = await run([
      "screencapture",
      "-x",
      `-R${mainCaptureBounds.x},${mainCaptureBounds.y},${mainCaptureBounds.width},${mainCaptureBounds.height}`,
      path,
    ]);
    const native = await nativeWindowIds(pid);
    const owner = native.completeWindows.find(
      (window: Json) => Number(window?.windowId) === mainWindowID,
    );
    settledCaptures.push({
      sequence: `settled-${index}`,
      captureSource: "explicit-post-settle-display-screenshot",
      path,
      sha256: capture.exitCode === 0 && existsSync(path)
        ? createHash("sha256").update(readFileSync(path)).digest("hex")
        : null,
      capturedAt: new Date().toISOString(),
      captureExitCode: capture.exitCode,
      captureStderr: capture.stderr.trim(),
      windowBounds: owner
        ? [
          [Number(owner.bounds.x), Number(owner.bounds.y)],
          [Number(owner.bounds.width), Number(owner.bounds.height)],
        ]
        : null,
      windowAlpha: owner?.alpha ?? null,
      windowOnscreen: owner?.onscreen ?? null,
      actualWindowID: owner?.windowId ?? null,
      expectedWindowID: mainWindowID,
    });
    await Bun.sleep(17);
  }
  const settledCapturesPass = settledCaptures.length === 3
    && settledCaptures.every((capture) =>
      capture.captureExitCode === 0
      && /^[a-f0-9]{64}$/.test(String(capture.sha256 ?? ""))
      && capture.actualWindowID === mainWindowID
      && Number(capture.windowAlpha) >= 0.999
      && capture.windowOnscreen === true
    );
  const mainEntryFilmstrip = await finishFilmstrip(
    mainEntry,
    mainWindowID,
    { captureBounds: mainCaptureBounds },
  );
  const mainEntryMotionEnvelope = analyzeEntryMotionEnvelope(
    mainEntryFilmstrip.receipt?.frames ?? [],
    settledCaptures[0]?.windowBounds,
    1.06,
  );
  (receipt.scenarios as Json[]).push({
    name: "main-entry",
    exactWindowID: mainWindowID,
    observerStartedAt: mainEntry.processStartedAt,
    showRequestedAt,
    streamReady: mainEntryReady,
    captureBounds: mainCaptureBounds,
    captureBoundsMatch: mainEntryCaptureBoundsMatch,
    settledCaptures,
    settledCapturesPass,
    motionEnvelope: mainEntryMotionEnvelope,
    pointerPreparation: {
      command: ["cliclick", "m:2,2"],
      exitCode: pointerPreparation.exitCode,
      target: { x: 2, y: 2 },
      purpose: "prevent an inherited hover state from obscuring base material",
    },
    settledLayout: mainEntrySettledLayout,
    filmstrip: mainEntryFilmstrip,
    structuralPass: mainEntryFilmstrip.capturePass
      && mainEntryCaptureBoundsMatch
      && settledCapturesPass
      && mainEntryMotionEnvelope.pass,
    pass: mainEntryFilmstrip.pass
      && mainEntryCaptureBoundsMatch
      && settledCapturesPass
      && mainEntryMotionEnvelope.pass,
  });
  }

  async function runNotesEntryScenario() {
  await announceTestStatus(
    "Lifecycle filmstrip · Notes entry",
    "Body remains hidden while the exact native Notes window material settles",
  );
  const notesEntry = startFilmstrip(
    "notes-entry",
    { pid, title: "Notes" },
  );
  driver.send({ type: "openNotes", requestId: "glass-life-notes-entry-open" });
  const notesEntryReady = await awaitObserverReady(notesEntry);
  const notesEntryID = Number(notesEntryReady.windowID);
  const notesReadyState = await notesState();
  let notesEntryPrimedByCancelReopen = false;
  if (notesReadyState?.entryReveal?.bodyVisible === true) {
    // Window discovery can occasionally outlast the 280ms settle interval.
    // Keep the already-pinned exact owner alive, begin its native exit, then
    // cancel/reopen it after the observer is ready. The product restarts the
    // same entry morph and body-reveal state on that exact CGWindowID.
    driver.send({ type: "openNotes", requestId: "glass-life-notes-entry-prime-close" });
    await Bun.sleep(25);
    driver.send({ type: "openNotes", requestId: "glass-life-notes-entry-prime-reopen" });
    notesEntryPrimedByCancelReopen = true;
  }
  const entryStates: Json[] = [];
  for (const elapsedMs of [0, 30, 80, 180, 320]) {
    if (elapsedMs > 0) await Bun.sleep(elapsedMs - Number(entryStates.at(-1)?.elapsedMs ?? 0));
    entryStates.push({ elapsedMs, state: await notesState() });
  }
  const notesLayout = await driver.getLayoutInfo(
    { target: { type: "id", id: "notes" } },
    { timeoutMs: 5_000 },
  );
  const notesBodyBounds = (notesLayout?.components ?? []).find(
    (component: Json) =>
      ["NotesEditor", "NotesPreview", "NotesEmbeddedAgentChat"].includes(
        String(component?.name ?? ""),
      ),
  )?.bounds;
  const preliminaryFinalReveal = entryStates.at(-1)?.state?.entryReveal;
  const notesEntryFilmstrip = await finishFilmstrip(notesEntry, notesEntryID, {
    bodyBounds: notesBodyBounds,
    hiddenHostTimeNs: preliminaryFinalReveal?.firstFrameAtMonotonicNs,
    visibleHostTimeNs: preliminaryFinalReveal?.visibleAtMonotonicNs,
  });
  const configuredState = entryStates.find(
    (sample) => sample?.state?.entryReveal?.nativeConfigured === true,
  )?.state?.entryReveal;
  const hiddenBeforeVisible = entryStates.some(
    (sample) =>
      sample?.state?.entryReveal?.nativeConfigured === true
      && sample?.state?.entryReveal?.bodyVisible === false,
  );
  const visibleAfterAnchor = entryStates.at(-1)?.state?.entryReveal?.bodyVisible === true;
  const finalReveal = entryStates.at(-1)?.state?.entryReveal;
  const notesTimes = {
    configured: Number(configuredState?.configuredAtMonotonicNs),
    firstFrame: Number(finalReveal?.firstFrameAtMonotonicNs),
    revealAnchor: Number(finalReveal?.revealAnchorAtMonotonicNs),
    revealRequested: Number(finalReveal?.revealRequestedAtMonotonicNs),
    visible: Number(finalReveal?.visibleAtMonotonicNs),
  };
  const notesTimesOrdered = Object.values(notesTimes).every(Number.isFinite)
    && notesTimes.configured <= notesTimes.firstFrame
    && notesTimes.firstFrame <= notesTimes.revealAnchor
    && notesTimes.revealAnchor <= notesTimes.revealRequested
    && notesTimes.revealRequested <= notesTimes.visible;
  const notesDisplayPeriodNs = 1_000_000_000
    / Number(notesEntryFilmstrip.receipt?.refreshRateHz ?? 60);
  const expectedVisibleLowerNs = notesTimes.configured
    + Number(configuredState?.revealDelayMs ?? 0) * 1_000_000
    - 2_000_000;
  const expectedVisibleUpperNs = notesTimes.configured
    + Number(configuredState?.revealDelayMs ?? 0) * 1_000_000
    + notesDisplayPeriodNs * 4
    + 21_000_000;
  const notesVisibleWithinBounds = Number.isFinite(notesTimes.visible)
    && notesTimes.visible >= expectedVisibleLowerNs
    && notesTimes.visible <= expectedVisibleUpperNs;
  // If exact-window discovery finished after first entry, the cancel/reopen
  // generation has no new native morph. Preserve the original generation's
  // runtime clocks so the receipt still proves the first-open reveal crossed
  // final size during phase one and began before rebound.
  const morphReveal = finalReveal?.morphStarted === true
    ? finalReveal
    : notesReadyState?.entryReveal;
  const morphConfiguredNs = Number(morphReveal?.configuredAtMonotonicNs);
  const morphVisibleNs = Number(morphReveal?.visibleAtMonotonicNs);
  const morphSettleDurationMs = Number(morphReveal?.settleDurationMs);
  const morphRevealDelayMs = Number(morphReveal?.revealDelayMs);
  const phaseOneEndNs = morphConfiguredNs + morphSettleDurationMs * 500_000;
  const revealBeganBeforeRebound = morphReveal?.morphStarted === true
    && morphRevealDelayMs > 0
    && morphRevealDelayMs < morphSettleDurationMs / 2
    && Number.isFinite(morphVisibleNs)
    && morphVisibleNs < phaseOneEndNs;
  const framesBeforeVisible = (
    notesEntryFilmstrip.receipt?.frames ?? []
  ).filter((frame: Json) =>
    typeof frame?.displayTimeNs === "number"
    && typeof finalReveal?.visibleAtMonotonicNs === "number"
    && frame.displayTimeNs <= finalReveal.visibleAtMonotonicNs
  ).length;
  const notesEntryStructuralPass = configuredState?.nativeWindowNumber === notesEntryID
    && configuredState?.backdropFoundOrCreated === true
    && configuredState?.nativeSelectorsSupported === true
    && configuredState?.styleApplied === true
    && configuredState?.fallbackUsed === false
    && typeof configuredState?.configuredAtMonotonicNs === "number"
    && typeof configuredState?.settleDurationMs === "number"
    && typeof configuredState?.revealDelayMs === "number"
    && hiddenBeforeVisible
    && visibleAfterAnchor
    && revealBeganBeforeRebound
    && Number(finalReveal?.completedFrameCount ?? 0) >= 2
    && notesTimesOrdered
    && notesVisibleWithinBounds;
  const notesEntryPass = notesEntryFilmstrip.pass
    && notesEntryStructuralPass
    && notesEntryFilmstrip.metrics?.bodyMaskPass === true;
  (receipt.scenarios as Json[]).push({
    name: "notes-entry",
    exactWindowID: notesEntryID,
    states: entryStates,
    bodyOnlyReveal: {
      hiddenBeforeVisible,
      visibleAfterAnchor,
      revealBeganBeforeRebound,
      framesBeforeVisible,
      completedFrameCount: finalReveal?.completedFrameCount ?? null,
      bodyPixelTransition: notesEntryFilmstrip.metrics?.bodyPixelTransition ?? false,
      bodyMaskPass: notesEntryFilmstrip.metrics?.bodyMaskPass ?? false,
      bodyBounds: notesBodyBounds ?? null,
      hostClockTiming: {
        times: notesTimes,
        ordered: notesTimesOrdered,
        displayPeriodNs: notesDisplayPeriodNs,
        expectedVisibleLowerNs,
        expectedVisibleUpperNs,
        visibleWithinBounds: notesVisibleWithinBounds,
        morph: {
          configuredAtMonotonicNs: morphConfiguredNs,
          visibleAtMonotonicNs: morphVisibleNs,
          settleDurationMs: morphSettleDurationMs,
          settledCrossingDelayMs: morphRevealDelayMs,
          phaseOneEndNs,
          beganBeforeRebound: revealBeganBeforeRebound,
        },
      },
    },
    notesLayout,
    nativeConfiguration: configuredState ?? null,
    observerPrimedByExactOwnerCancelReopen: notesEntryPrimedByCancelReopen,
    filmstrip: notesEntryFilmstrip,
    structuralPass: notesEntryFilmstrip.capturePass && notesEntryStructuralPass,
    pass: notesEntryPass,
  });
  driver.send({ type: "openNotes", requestId: "glass-life-notes-entry-close" });
  await waitForWindowCount(driver, "notes", 0, 3_000);
  }

  async function runNotesCloseBeforeSettleReopenScenario() {
  await announceTestStatus(
    "Lifecycle filmstrip · Notes cancel/reopen",
    "Close before settle, reopen during the fade, and reuse the same CGWindowID",
  );
  const notesReopen = startFilmstrip(
    "notes-close-before-settle-reopen",
    { pid, title: "Notes" },
  );
  driver.send({ type: "openNotes", requestId: "glass-life-notes-reopen-open" });
  const notesReopenReady = await awaitObserverReady(notesReopen);
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
  const notesReopenFilmstrip = await finishFilmstrip(
    notesReopen,
    notesReopenID,
    {
      expectedExitFrame:
        duringExit?.windowLifecycle?.nativeExit?.originalFrame,
    },
  );
  const notesActiveExitErrors = validateDetachedExitLifecycle(
    duringExit?.windowLifecycle?.nativeExit,
    notesReopenID,
    "exiting",
  );
  const notesCancelledExitErrors = validateDetachedExitLifecycle(
    afterReopen?.windowLifecycle?.nativeExit,
    notesReopenID,
    "cancelled",
  );
  const notesTopology = classifyNativeInventory(
    notesCompleteAfter.completeWindows as any[],
    pid,
    mainWindowID,
  );
  const notesReopenStructural = duringExit?.windowLifecycle?.phase === "Exiting"
    && duringExit?.windowLifecycle?.hasExitTicket === true
    && typeof duringExit?.windowLifecycle?.exitGeneration === "number"
    && afterReopen?.windowLifecycle?.phase === "Open"
    && afterReopen?.windowLifecycle?.hasExitTicket === false
    && afterReopen?.entryReveal?.bodyVisible === true
    && notesAfterNative.ids.length === 1
    && notesAfterNative.ids[0] === notesReopenID
    && notesCompleteAfter.completeWindowIds.includes(mainWindowID)
    && notesCompleteAfter.completeWindowIds.includes(notesReopenID)
    && notesTopology.pass
    && notesActiveExitErrors.length === 0
    && notesCancelledExitErrors.length === 0;
  const notesReopenPass = notesReopenFilmstrip.pass && notesReopenStructural;
  (receipt.scenarios as Json[]).push({
    name: "notes-close-before-settle-reopen",
    exactWindowID: notesReopenID,
    beforeClose,
    duringExit,
    afterReopen,
    nativeWindowIdsAfterReopen: notesAfterNative.ids,
    completeNativeInventoryAfterReopen: notesCompleteAfter,
    completeNativeTopologyAfterReopen: notesTopology,
    nativeExitValidation: {
      activeErrors: notesActiveExitErrors,
      cancelledErrors: notesCancelledExitErrors,
    },
    filmstrip: notesReopenFilmstrip,
    structuralPass: notesReopenFilmstrip.capturePass && notesReopenStructural,
    pass: notesReopenPass,
  });
  driver.send({ type: "openNotes", requestId: "glass-life-notes-reopen-final-close" });
  await waitForWindowCount(driver, "notes", 0, 3_000);
  }

  async function runDictationExitReopenScenario() {
  await announceTestStatus(
    "Lifecycle filmstrip · Dictation cancel/reopen",
    "Fixture-only overlay; Escape starts fade, reopen cancels its ticket without microphone capture",
  );
  const dictation = startFilmstrip(
    "dictation-exit-reopen",
    { pid, title: "Script Kit Dictation" },
  );
  driver.send({
    type: "openDictationOverlayFixture",
    requestId: "glass-life-dictation-open",
  });
  const dictationReady = await awaitObserverReady(dictation);
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
  const dictationFilmstrip = await finishFilmstrip(
    dictation,
    dictationID,
    {
      expectedExitFrame:
        dictationDuringExit?.windowLifecycle?.nativeExit?.originalFrame,
    },
  );
  const dictationActiveExitErrors = validateDetachedExitLifecycle(
    dictationDuringExit?.windowLifecycle?.nativeExit,
    dictationID,
    "exiting",
  );
  const dictationCancelledExitErrors = validateDetachedExitLifecycle(
    dictationAfter?.windowLifecycle?.nativeExit,
    dictationID,
    "cancelled",
  );
  const dictationTopology = classifyNativeInventory(
    dictationCompleteAfter.completeWindows as any[],
    pid,
    mainWindowID,
  );
  const dictationStructural = dictationDuringExit?.windowLifecycle?.phase === "Exiting"
    && dictationDuringExit?.windowLifecycle?.handleRegistered === true
    && dictationDuringExit?.windowLifecycle?.automationRegistered === true
    && typeof dictationDuringExit?.windowLifecycle?.exitGeneration === "number"
    && dictationAfter?.windowLifecycle?.phase === "Open"
    && dictationAfter?.windowLifecycle?.hasExitTicket === false
    && dictationNativeAfter.ids.length === 1
    && dictationNativeAfter.ids[0] === dictationID
    && dictationCompleteAfter.completeWindowIds.includes(mainWindowID)
    && dictationCompleteAfter.completeWindowIds.includes(dictationID)
    && dictationTopology.pass
    && dictationActiveExitErrors.length === 0
    && dictationCancelledExitErrors.length === 0;
  const dictationPass = dictationFilmstrip.pass && dictationStructural;
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
    completeNativeTopologyAfterReopen: dictationTopology,
    nativeExitValidation: {
      activeErrors: dictationActiveExitErrors,
      cancelledErrors: dictationCancelledExitErrors,
    },
    noMicrophoneCapture: true,
    filmstrip: dictationFilmstrip,
    structuralPass: dictationFilmstrip.capturePass && dictationStructural,
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
  }

  // Dispatch the resolved profile in declared order. Timing intervals are
  // recorded per scenario and validated as monotone/non-overlapping so a
  // refactor can never silently interleave capture work.
  const scenarioRunners: Record<LifecycleScenarioName, () => Promise<void>> = {
    "main-exit": runMainExitScenario,
    "main-entry": runMainEntryScenario,
    "notes-entry": runNotesEntryScenario,
    "notes-close-before-settle-reopen": runNotesCloseBeforeSettleReopenScenario,
    "dictation-exit-reopen": runDictationExitReopenScenario,
  };
  const captureStartedMs = performance.now();
  const scenarioIntervals: ScenarioTimingInterval[] = [];
  for (const name of resolvedScenarioNames) {
    const startedAtMs = performance.now();
    await scenarioRunners[name]();
    scenarioIntervals.push({
      name,
      startedAtMs,
      finishedAtMs: performance.now(),
    });
  }
  timingMs.captureTotal = performance.now() - captureStartedMs;
  receipt.scenarioIntervalsMs = scenarioIntervals;
  const intervalErrors = validateScenarioTimingIntervals(scenarioIntervals);
  if (intervalErrors.length > 0) {
    throw new Error(`scenario timing intervals invalid: ${intervalErrors}`);
  }

  receipt.requiredScenarioNames = resolvedScenarioNames;
  const scenarios = receipt.scenarios as Json[];
  receipt.pass = scenarios.length === resolvedScenarioNames.length
    && receipt.requiredScenarioNames.every((name: string) =>
      scenarios.some((scenario) => scenario?.name === name)
    )
    && scenarios.every((scenario) => scenario?.pass === true);
} catch (error) {
  receipt.error = String(error);
  receipt.pass = false;
} finally {
  const cleanupStartedMs = performance.now();
  if (interferenceMonitor) {
    receipt.interference = await finishInterferenceMonitor(interferenceMonitor);
    receipt.pass = receipt.pass === true && receipt.interference.pass === true;
  }
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();
  receipt.pass = receipt.pass === true && receipt.cleanedUp === true;
  if (analysisMode === "deferred") {
    // A deferred capture is NEVER a normal passing lifecycle receipt: the
    // Python graders have not run. The offline finalizer consumes
    // capture-receipt.json and writes the standard receipt.json later.
    receipt.analysisState = "pending";
    receipt.capturePass = (receipt.scenarios as Json[]).length > 0
      && (receipt.scenarios as Json[]).every(
        (scenario) => scenario?.filmstrip?.capturePass === true,
      )
      && receipt.error == null
      && receipt.cleanedUp === true
      && receipt.interference?.pass === true;
    receipt.pass = false;
  } else {
    receipt.analysisState = "inline";
  }
  receipt.disposition = computeLifecycleDisposition({
    interferenceDisposition: receipt.interference?.disposition ?? null,
    analysisState: receipt.analysisState,
    pass: receipt.pass === true,
    hasObserverError: receipt.error != null,
  });
  timingMs.cleanup = performance.now() - cleanupStartedMs;
  timingMs.total = performance.now() - scriptStartedMs;
  receipt.timingMs = timingMs;
  const receiptPath = join(
    outDir,
    analysisMode === "deferred" ? "capture-receipt.json" : "receipt.json",
  );
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(
    JSON.stringify(
      {
        receiptPath,
        pass: receipt.pass,
        disposition: receipt.disposition,
        analysisMode,
      },
      null,
      2,
    ),
  );
}

process.exit(receipt.pass ? 0 : 2);
