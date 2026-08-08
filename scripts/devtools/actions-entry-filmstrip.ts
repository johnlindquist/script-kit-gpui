#!/usr/bin/env bun
/**
 * Actions entry rendered observer (PF-011, C08 repair).
 *
 * The pre-C08 probe carried an explicit false-win: it gated on LOGGED
 * geometry while letting rendered evidence pass when it was under-resolved
 * (`renderedGeometry.underResolved === true || renderedGeometry.pass`), and
 * it classified every clean non-pass as EVALUABLE_FAIL. C08 splits the truth
 * layers:
 *
 *   renderedEnvelope          — the ONLY gate (locked evaluator, unchanged)
 *   loggedGeometryDiagnostic  — receipt-only source diagnostics, never gating
 *
 * Ownership is derived from the complete same-PID before/after inventory
 * delta (`deriveUniqueOwnerDelta`): more than one candidate is INVALID —
 * never sorted, never candidates[0]. Every rendered frame, the filmstrip
 * ready.windowID, and the settled native sample must bind to that ONE owner.
 */

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
import {
  ACTIONS_GLASS_ENTRY_EXPECTATION,
  analyzeEntryMotionEnvelope,
  analyzeEntrySurfaceFields,
  analyzeLoggedEntryGeometry,
  analyzeOnsetReceipt,
  type NativeWindowBounds,
} from "./glass-entry-motion-contract.ts";
import { validateFilmstripCapture } from "./glass-lifecycle-filmstrip-contract.ts";
import {
  finishInterferenceMonitor,
  startInterferenceMonitor,
  waitForInterferenceReady,
} from "./glass-interference.ts";
import {
  deriveUniqueOwnerDelta,
  type NativeWindowRow,
} from "./glass-topology-contract.ts";
import { announceTestStatus } from "./test-status.ts";
import { requireValidatedHelper } from "./glass-native-helper-cache.ts";
import {
  classifyGlassObservation,
  exitCodeForDisposition,
  type GlassObservationInput,
  validateOwnedRenderedFrames,
} from "./glass-observers.ts";

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

async function waitForFile(path: string, timeoutMs = 5_000) {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    if (existsSync(path)) return JSON.parse(readFileSync(path, "utf8"));
    await Bun.sleep(10);
  }
  throw new Error(`timed out waiting for ${path}`);
}

async function waitForActions(driver: Driver, expected: number, timeoutMs = 3_000) {
  const started = performance.now();
  let last: Json = null;
  while (performance.now() - started < timeoutMs) {
    last = await driver.listAutomationWindows({ timeoutMs: 5_000 });
    const count = (last?.windows ?? []).filter(
      (window: Json) =>
        window?.kind === "actionsDialog" && window?.visible !== false,
    ).length;
    if (count === expected) return { pass: true, count, snapshot: last };
    await Bun.sleep(15);
  }
  const count = ((last?.windows ?? []) as Json[]).filter(
    (window: Json) =>
      window?.kind === "actionsDialog" && window?.visible !== false,
  ).length;
  return { pass: false, count, snapshot: last };
}

const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!);
const themeFixture = resolve(
  arg(
    "--theme-fixture",
    "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
  )!,
);
const expectedFixtureSha = arg("--theme-fixture-sha256");
const outDir = resolve(
  arg(
    "--out",
    ".artifacts/glass-motion/actions-entry-filmstrip",
  )!,
);
if (!existsSync(binary)) throw new Error(`binary missing: ${binary}`);
if (!existsSync(themeFixture)) {
  throw new Error(`theme fixture missing: ${themeFixture}`);
}
rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const fixtureSha256 = createHash("sha256")
  .update(readFileSync(themeFixture))
  .digest("hex");

const receipt: Json = {
  schemaVersion: 2,
  startedAt: new Date().toISOString(),
  ...identityFromEnvironment({
    runId: newRunId(),
    gitCommit: (await run(["git", "rev-parse", "HEAD"])).stdout.trim(),
    binary,
    binarySha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
  }),
  scenario: process.env.SCRIPT_KIT_GLASS_SCENARIO ?? "actions-entry",
  themeFixture: { path: themeFixture, sha256: fixtureSha256 },
  pass: false,
};

function finalizeAndExit(): never {
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(
    JSON.stringify(
      { receiptPath, pass: receipt.pass, disposition: receipt.disposition },
      null,
      2,
    ),
  );
  process.exit(exitCodeForDisposition(String(receipt.disposition)));
}

// Fixture-exactness gate: when the caller pins the expected fixture SHA, a
// mismatch invalidates the observation before the app launches.
const fixtureErrors: string[] = [];
if (expectedFixtureSha && expectedFixtureSha !== fixtureSha256) {
  fixtureErrors.push(
    `theme fixture sha256 mismatch: expected ${expectedFixtureSha}, on disk ${fixtureSha256}`,
  );
}

// WP4 (glass-smoke-harness-max-info) + C08: accept pre-compiled
// hash-validated helpers; compile per-run only when absent. A malformed
// SUPPLIED helper keeps its original INVALID_SETUP diagnostic on stderr but
// now also writes an INVALID_OBSERVER receipt (nonzero exit) instead of an
// unclassified crash.
const suppliedFilmstripHelper = arg("--filmstrip-helper");
const suppliedInterferenceHelper = arg("--interference-helper");
const suppliedWindowQueryHelper = arg("--window-query-helper");
let helper: string;
let interferenceHelper: string;
let windowQueryHelper: string | null = null;
try {
  if (suppliedFilmstripHelper) {
    helper = requireValidatedHelper(suppliedFilmstripHelper, "filmstrip")
      .binaryPath;
  } else {
    helper = join(outDir, "macos-native-window-filmstrip");
    const helperCompile = await run([
      "xcrun",
      "swiftc",
      "-parse-as-library",
      "-O",
      resolve(import.meta.dir, "../agentic/macos-native-window-filmstrip.swift"),
      "-o",
      helper,
    ]);
    if (helperCompile.exitCode !== 0) {
      throw new Error(`filmstrip helper compile failed: ${helperCompile.stderr}`);
    }
  }
  if (suppliedInterferenceHelper) {
    interferenceHelper = requireValidatedHelper(
      suppliedInterferenceHelper,
      "interference",
    ).binaryPath;
  } else {
    interferenceHelper = join(outDir, "macos-glass-interference-monitor");
    const interferenceCompile = await run([
      "xcrun",
      "swiftc",
      "-O",
      resolve(import.meta.dir, "../agentic/macos-glass-interference-monitor.swift"),
      "-o",
      interferenceHelper,
    ]);
    if (interferenceCompile.exitCode !== 0) {
      throw new Error(`interference helper compile failed: ${interferenceCompile.stderr}`);
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
  receipt.helperValidation = { error: message };
  receipt.observation = {
    disposition: "INVALID_OBSERVER",
    pass: false,
    observerErrors: [message],
    productErrors: [],
  };
  receipt.disposition = "INVALID_OBSERVER";
  receipt.finishedAt = new Date().toISOString();
  finalizeAndExit();
}

/** Complete same-PID native inventory (hidden + alpha-zero included). */
async function completeNativeInventory(pid: number): Promise<{
  windows: NativeWindowRow[];
  error: string | null;
}> {
  const command = windowQueryHelper
    ? [windowQueryHelper, "--pid", String(pid)]
    : [
      "swift",
      resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
      "--pid",
      String(pid),
    ];
  const query = await run(command);
  if (query.exitCode !== 0) {
    return { windows: [], error: query.stderr.trim().slice(0, 400) };
  }
  const parsed = JSON.parse(query.stdout);
  return {
    windows: ((parsed.windows ?? []) as NativeWindowRow[]).filter(
      (window) => Number(window?.windowId ?? 0) > 0,
    ),
    error: null,
  };
}

function expandBounds(bounds: { x?: number; y?: number; width?: number; height?: number }, padding = 80) {
  return {
    x: Number(bounds.x) - padding,
    y: Number(bounds.y) - padding,
    width: Number(bounds.width) + padding * 2,
    height: Number(bounds.height) + padding * 2,
  };
}

function pinMainWindowId(windows: NativeWindowRow[]): number {
  return Number(
    windows
      .filter(
        (window) =>
          String(window?.title ?? "") === "" && Number(window?.layer) === 101,
      )
      .sort(
        (left, right) =>
          Number(right?.bounds?.width ?? 0) * Number(right?.bounds?.height ?? 0)
          - Number(left?.bounds?.width ?? 0) * Number(left?.bounds?.height ?? 0),
      )[0]?.windowId ?? 0,
  );
}

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  themeFixturePath: themeFixture,
  sessionName: `actions-entry-filmstrip-${process.pid}`,
  defaultTimeoutMs: 8_000,
});
let interferenceMonitor: ReturnType<typeof startInterferenceMonitor> | null = null;

// Classification inputs assembled across the run; judged in `finally`.
const observerErrors: string[] = [];
const ownerErrors: string[] = [];
const identityErrors: string[] = [];
let captureHealthPass = false;
let rendered: GlassObservationInput["rendered"] = {
  present: false,
  underResolved: false,
  pass: false,
  errors: [],
};

try {
  receipt.pid = driver.pid;
  receipt.sessionDir = driver.sessionDir;
  driver.send({ type: "show", requestId: "actions-entry-show-main" });
  await driver.waitForState({ windowVisible: true }, { timeoutMs: 5_000 });
  await driver.waitForSettle({ timeoutMs: 5_000 });
  await run(["cliclick", "m:2,2"]);
  interferenceMonitor = startInterferenceMonitor(interferenceHelper, outDir);
  receipt.interferenceReady = await waitForInterferenceReady(interferenceMonitor);

  await announceTestStatus(
    "Animation lock · Actions entry",
    "120 Hz exact-window filmstrip; 98.8% -> 101.3% -> 100% grow-in must remain inside the golden envelope",
  );

  // Arm over the already-known main region. Actions is constrained inside that
  // region, so no warm popup is needed and no prior transient owner can leak
  // pixels or identity into the evidence run.
  const beforeInventory = await completeNativeInventory(Number(driver.pid));
  if (beforeInventory.error) {
    observerErrors.push(`pre-open native inventory failed: ${beforeInventory.error}`);
  }
  const mainWindowId = pinMainWindowId(beforeInventory.windows);
  const mainWindow = beforeInventory.windows.find(
    (window) => Number(window.windowId) === mainWindowId,
  );
  if (!mainWindowId || !mainWindow?.bounds) {
    throw new Error("main native window and bounds could not be pinned before open");
  }
  const captureBounds = expandBounds(mainWindow.bounds, 0);
  receipt.preOpenInventory = beforeInventory;
  receipt.mainWindowId = mainWindowId;
  receipt.captureBounds = captureBounds;

  const framesDir = join(outDir, "frames");
  mkdirSync(framesDir, { recursive: true });
  const readyPath = join(framesDir, "ready.json");
  const command = [
    helper,
    "--pid",
    String(driver.pid),
    "--display-stream",
    "--region-armed",
    "--owner-class",
    "Actions",
    "--bounds",
    String(captureBounds.x),
    String(captureBounds.y),
    String(captureBounds.width),
    String(captureBounds.height),
    ...beforeInventory.windows.flatMap((window) => [
      "--exclude-window-id",
      String(window.windowId),
    ]),
    "--out",
    framesDir,
    "--ready",
    readyPath,
    "--duration-ms",
    "700",
    "--fps",
    "120",
    "--run-id",
    String(receipt.runId),
    "--git-commit",
    String(receipt.gitCommit),
    "--binary-sha256",
    String(receipt.binarySha256),
  ];
  const capture = Bun.spawn(command, { stdout: "pipe", stderr: "pipe" });
  const ready = await waitForFile(readyPath);
  const dispatch = await driver.simulateGpuiKeyDown("k", {
    modifiers: ["cmd"],
    target: { type: "id", id: "main" },
    timeoutMs: 5_000,
  });
  if (dispatch?.success !== true) {
    observerErrors.push("Cmd+K dispatch was not acknowledged by the app");
  }
  const opened = await waitForActions(driver, 1, 3_000);
  if (opened.pass !== true || opened.count !== 1) {
    ownerErrors.push(
      `expected exactly one automation actionsDialog target, observed ${opened.count ?? "none"}`,
    );
  }
  await driver.waitForSettle({ timeoutMs: 3_000 });

  // Complete post-open inventory → ONE fresh Actions owner via the delta.
  const afterInventory = await completeNativeInventory(Number(driver.pid));
  if (afterInventory.error) {
    observerErrors.push(`post-open native inventory failed: ${afterInventory.error}`);
  }
  receipt.postOpenInventory = afterInventory;
  const ownerDelta = deriveUniqueOwnerDelta(
    beforeInventory.windows,
    afterInventory.windows,
    "Actions",
    Number(driver.pid),
    mainWindowId,
  );
  if (!ownerDelta.pass) {
    ownerErrors.push(
      `expected exactly one new native Actions owner, observed ${ownerDelta.candidateIds.length} (${
        ownerDelta.candidateIds.join(", ") || "none"
      })`,
    );
  }
  const ownerId = ownerDelta.pass ? ownerDelta.candidateIds[0]! : null;
  if (Number(ready?.windowID) !== 0 || ready?.captureMode !== "display-region-armed-before-owner") {
    ownerErrors.push("filmstrip was not ready in pre-owner region-armed mode");
  }
  // Settled native sample must bind to the same owner.
  const settledInventory = await completeNativeInventory(Number(driver.pid));
  const settledOwner = ownerId != null
    ? settledInventory.windows.find(
      (window) => Number(window.windowId) === ownerId,
    ) ?? null
    : null;
  if (ownerId != null && !settledOwner) {
    ownerErrors.push(
      `settled native sample missing for the unique Actions owner ${ownerId}`,
    );
  }
  receipt.owner = {
    windowId: ownerId,
    delta: ownerDelta,
    readyWindowId: ready?.windowID ?? null,
    settledSample: settledOwner,
    errors: [...ownerErrors],
  };

  const [captureStdout, captureStderr, captureExitCode] = await Promise.all([
    new Response(capture.stdout).text(),
    new Response(capture.stderr).text(),
    capture.exited,
  ]);
  const filmstripPath = join(framesDir, "receipt.json");
  const filmstrip = existsSync(filmstripPath)
    ? JSON.parse(readFileSync(filmstripPath, "utf8"))
    : null;
  const presentationPath = join(framesDir, "presentation-geometry.json");
  const presentationRun = ownerId != null && filmstrip
    ? await run([
      "python3",
      resolve(import.meta.dir, "../agentic/rendered-capsule-geometry.py"),
      "--receipt",
      filmstripPath,
      "--expected-owner",
      String(ownerId),
      "--anchor-bounds",
      String(settledOwner?.bounds?.x),
      String(settledOwner?.bounds?.y),
      String(settledOwner?.bounds?.width),
      String(settledOwner?.bounds?.height),
      "--out",
      presentationPath,
    ])
    : { exitCode: 1, stdout: "", stderr: "owner unresolved" };
  const presentation = existsSync(presentationPath)
    ? JSON.parse(readFileSync(presentationPath, "utf8"))
    : null;
  const filmstripIdentityErrors = filmstrip && ownerId != null
    ? validateFilmstripCapture(filmstrip, {
      runId: String(receipt.runId),
      gitCommit: String(receipt.gitCommit),
      binarySha256: String(receipt.binarySha256),
      pid: Number(driver.pid),
      windowId: ownerId,
    })
    : ["filmstrip receipt missing or owner unresolved"];
  identityErrors.push(...filmstripIdentityErrors);
  captureHealthPass = captureExitCode === 0
    && filmstrip?.captureHealthPass === true
    && presentationRun.exitCode === 0
    && presentation?.pass === true;
  if (ownerId != null) {
    ownerErrors.push(
      ...validateOwnedRenderedFrames(
        (filmstrip?.frames ?? []).filter((frame: Json) => frame?.actualWindowID != null),
        ownerId,
      ),
    );
  }
  if (presentation?.pass !== true) {
    observerErrors.push(...(presentation?.errors ?? ["presentation geometry analysis missing"]));
  }

  const appLog = await Bun.file(driver.logPath).text();
  const motionLog = appLog
    .split("\n")
    .filter((line) =>
      line.includes("event=glass_morph")
      && line.includes("Actions popup")
      && line.includes("phase=enter")
    )
    .slice(-3);
  // SOURCE DIAGNOSTIC ONLY (C08): the logged geometry is retained for
  // diagnosis and cross-surface comparison. It is NEVER part of the gate —
  // rendered evidence is the only truth layer that can pass this probe.
  const loggedGeometryDiagnostic = analyzeLoggedEntryGeometry(
    motionLog,
    ACTIONS_GLASS_ENTRY_EXPECTATION,
  );
  const presentationFrames = presentation?.frames ?? [];
  const settledBounds: NativeWindowBounds = presentationFrames.at(-1)?.windowBounds ?? null;
  // LOCKED evaluator, unchanged: composited-pixel geometry is the rendered gate.
  const renderedEnvelope = analyzeEntryMotionEnvelope(
    presentationFrames,
    settledBounds,
    ACTIONS_GLASS_ENTRY_EXPECTATION,
  );
  rendered = {
    present: Number(renderedEnvelope.measuredFrameCount ?? 0) > 0,
    underResolved: renderedEnvelope.underResolved === true,
    pass: renderedEnvelope.pass === true,
    errors: renderedEnvelope.errors ?? [],
  };
  const surfaceFields = analyzeEntrySurfaceFields(motionLog);
  const onsetLog = appLog
    .split("\n")
    .filter((line) => line.includes("event=native_glass_entry_onset"))
    .slice(-3);
  const onset = analyzeOnsetReceipt(onsetLog);
  receipt.capture = {
    command,
    exitCode: captureExitCode,
    stderr: captureStderr.trim().slice(-1_000),
    stdout: captureStdout.trim().slice(-1_000),
    ready,
    receiptPath: filmstripPath,
    receipt: filmstrip,
    identityErrors: filmstripIdentityErrors,
    presentationGeometry: {
      commandExitCode: presentationRun.exitCode,
      stderr: presentationRun.stderr.trim().slice(-1_000),
      receiptPath: presentationPath,
      receipt: presentation,
    },
  };
  receipt.dispatch = dispatch;
  receipt.opened = opened;
  receipt.motion = {
    loggedGeometryDiagnostic,
    renderedEnvelope,
    surfaceFields,
    onset,
  };
  receipt.motionLog = motionLog;
  receipt.onsetLog = onsetLog;
  if (motionLog.length < 1) {
    // A missing source log is a WARNING against the source layer, not a
    // rendered-observer failure (plan §3.5: rendered truth stays
    // authoritative; the discrepancy is preserved for integration).
    receipt.sourceLogWarnings = ["runtime entry geometry log is missing"];
  }

  const closeDispatch = await driver.simulateGpuiKeyDown("escape", {
    target: { type: "id", id: "actions-dialog" },
    timeoutMs: 5_000,
  });
  const closed = await waitForActions(driver, 0, 3_000);
  receipt.close = { dispatch: closeDispatch, closed };
  if (closeDispatch?.success !== true || closed.pass !== true) {
    observerErrors.push("Actions dialog did not close during observer teardown");
  }
} catch (error) {
  receipt.error = String(error);
  observerErrors.push(String(error));
} finally {
  if (interferenceMonitor) {
    receipt.interference = await finishInterferenceMonitor(interferenceMonitor);
  }
  await driver.close();
  receipt.cleanedUp = !driver.alive;
  receipt.finishedAt = new Date().toISOString();

  const input: GlassObservationInput = {
    captureHealthPass,
    helperErrors: [],
    fixtureErrors,
    identityErrors,
    ownerErrors,
    requiredPhaseErrors: [],
    cleanupErrors: [
      ...(receipt.cleanedUp === true ? [] : ["app process survived driver close"]),
      ...observerErrors,
    ],
    interference: {
      validated: receipt.interference?.receipt != null
        && receipt.interference?.exitCode === 0,
      disposition: receipt.interference?.disposition ?? null,
      errors: receipt.interference?.errors ?? [],
    },
    rendered,
    sourceDiagnostics: receipt.motion ?? null,
  };
  const observation = classifyGlassObservation(input);
  receipt.observation = observation;
  receipt.disposition = observation.disposition;
  receipt.pass = observation.pass;
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(
    JSON.stringify(
      { receiptPath, pass: receipt.pass, disposition: receipt.disposition },
      null,
      2,
    ),
  );
}

process.exit(exitCodeForDisposition(String(receipt.disposition)));
