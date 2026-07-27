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
import { announceTestStatus } from "./test-status.ts";
import { requireValidatedHelper } from "./glass-native-helper-cache.ts";

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
  return { pass: false, count: null, snapshot: last };
}

async function nativeActionsWindow(pid: number) {
  const query = await run([
    "swift",
    resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
    "--pid",
    String(pid),
  ]);
  if (query.exitCode !== 0) {
    return { window: null, error: query.stderr.trim() };
  }
  const parsed = JSON.parse(query.stdout);
  const candidates = (parsed.windows ?? [])
    .filter((window: Json) =>
      Number(window?.windowId ?? 0) > 0
      && window?.onscreen === true
      && Number(window?.layer ?? 0) === 101
      && Number(window?.bounds?.width ?? 0) > 1
      && Number(window?.bounds?.width ?? 0) <= 500
      && Number(window?.bounds?.height ?? 0) > 1
    )
    .sort((left: Json, right: Json) =>
      Number(right?.alpha ?? 0) - Number(left?.alpha ?? 0)
    );
  return {
    window: candidates[0] ?? null,
    candidates,
    error: candidates.length === 1
      ? null
      : `expected one Actions native owner, found ${candidates.length}`,
  };
}

const binary = resolve(arg("--binary", process.env.SCRIPT_KIT_GPUI_BINARY ?? "")!);
const themeFixture = resolve(
  arg(
    "--theme-fixture",
    "scripts/agentic/fixtures/glass-motion-calibration-theme.json",
  )!,
);
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

// WP4 (glass-smoke-harness-max-info): accept pre-compiled hash-validated
// helpers from the study orchestrator; compile per-run only when absent.
const suppliedFilmstripHelper = arg("--filmstrip-helper");
const suppliedInterferenceHelper = arg("--interference-helper");
let helper: string;
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
let interferenceHelper: string;
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

const receipt: Json = {
  schemaVersion: 1,
  startedAt: new Date().toISOString(),
  ...identityFromEnvironment({
    runId: newRunId(),
    gitCommit: (await run(["git", "rev-parse", "HEAD"])).stdout.trim(),
    binary,
    binarySha256: createHash("sha256").update(readFileSync(binary)).digest("hex"),
  }),
  scenario: process.env.SCRIPT_KIT_GLASS_SCENARIO ?? "actions-entry",
  themeFixture: {
    path: themeFixture,
    sha256: createHash("sha256")
      .update(readFileSync(themeFixture))
      .digest("hex"),
  },
  pass: false,
};

const driver = await Driver.launch({
  binary,
  sandboxHome: true,
  themeFixturePath: themeFixture,
  sessionName: `actions-entry-filmstrip-${process.pid}`,
  defaultTimeoutMs: 8_000,
});
let interferenceMonitor: ReturnType<typeof startInterferenceMonitor> | null = null;

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
  const framesDir = join(outDir, "frames");
  mkdirSync(framesDir, { recursive: true });
  const readyPath = join(framesDir, "ready.json");
  const command = [
    helper,
    "--pid",
    String(driver.pid),
    "--max-width",
    "500",
    "--display-stream",
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
  await Bun.sleep(30);
  const dispatch = await driver.simulateGpuiKeyDown("k", {
    modifiers: ["cmd"],
    target: { type: "id", id: "main" },
    timeoutMs: 5_000,
  });
  const ready = await waitForFile(readyPath);
  const opened = await waitForActions(driver, 1, 3_000);
  await driver.waitForSettle({ timeoutMs: 3_000 });
  const settledNative = await nativeActionsWindow(Number(driver.pid));
  const [captureStdout, captureStderr, captureExitCode] = await Promise.all([
    new Response(capture.stdout).text(),
    new Response(capture.stderr).text(),
    capture.exited,
  ]);
  const filmstripPath = join(framesDir, "receipt.json");
  const filmstrip = existsSync(filmstripPath)
    ? JSON.parse(readFileSync(filmstripPath, "utf8"))
    : null;
  const identityErrors = filmstrip
    ? validateFilmstripCapture(filmstrip, {
      runId: String(receipt.runId),
      gitCommit: String(receipt.gitCommit),
      binarySha256: String(receipt.binarySha256),
      pid: Number(driver.pid),
      windowId: Number(ready.windowID),
    })
    : ["filmstrip receipt missing"];
  const appLog = await Bun.file(driver.logPath).text();
  const motionLog = appLog
    .split("\n")
    .filter((line) =>
      line.includes("event=glass_morph")
      && line.includes("Actions popup")
      && line.includes("phase=enter")
    )
    .slice(-3);
  const motionEnvelope = analyzeLoggedEntryGeometry(
    motionLog,
    ACTIONS_GLASS_ENTRY_EXPECTATION,
  );

  // WP0 (Oracle `glass-entry-feel-options`): until now this probe judged the
  // Actions curve from its LOGGED targets while the main lifecycle probe judged
  // main from CAPTURED frames. The two references were therefore not measured
  // symmetrically, and the user has named the Actions entry as the perceptual
  // target for a main-window retune. Since `open_actions_window` configures the
  // morph before attaching the popup to its parent, AppKit's parent-child
  // machinery may be damping what actually renders — so the logged target is
  // not proof of the reference feel. Measure the rendered bounds too.
  const settledActionsBounds = settledNative.window?.bounds;
  const settledBounds: NativeWindowBounds = settledActionsBounds
    ? [
      [
        Number(settledActionsBounds.x),
        Number(settledActionsBounds.y),
      ],
      [
        Number(settledActionsBounds.width),
        Number(settledActionsBounds.height),
      ],
    ]
    : null;
  const renderedGeometry = analyzeEntryMotionEnvelope(
    filmstrip?.frames ?? [],
    settledBounds,
    ACTIONS_GLASS_ENTRY_EXPECTATION,
  );
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
    identityErrors,
  };
  receipt.dispatch = dispatch;
  receipt.opened = opened;
  receipt.settledNative = settledNative;
  receipt.motionEnvelope = motionEnvelope;
  receipt.motion = {
    loggedGeometry: motionEnvelope,
    renderedGeometry,
    surfaceFields,
    onset,
  };
  receipt.motionLog = motionLog;
  receipt.onsetLog = onsetLog;

  const closeDispatch = await driver.simulateGpuiKeyDown("escape", {
    target: { type: "id", id: "actions-dialog" },
    timeoutMs: 5_000,
  });
  const closed = await waitForActions(driver, 0, 3_000);
  receipt.close = { dispatch: closeDispatch, closed };
  receipt.pass = captureExitCode === 0
    && identityErrors.length === 0
    && dispatch?.success === true
    && opened.pass === true
    && settledNative.error == null
    && motionEnvelope.pass === true
    // Rendered geometry is a SOFT gate: a damage-driven capture that produced
    // too few geometry samples falls back to the exact logged proof rather than
    // failing the run (decision rule 5B, glass-entry-spotlight-retune). It only
    // fails the probe when it resolved enough frames to judge AND disagreed.
    && (renderedGeometry.underResolved === true || renderedGeometry.pass === true)
    && motionLog.length >= 1
    && closeDispatch?.success === true
    && closed.pass === true;
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
    : receipt.pass
    ? "EVALUABLE_PASS"
    : "EVALUABLE_FAIL";
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({ receiptPath, pass: receipt.pass }, null, 2));
}

process.exit(receipt.pass ? 0 : 2);
