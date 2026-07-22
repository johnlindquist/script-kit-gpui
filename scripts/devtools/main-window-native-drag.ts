#!/usr/bin/env bun

import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { Driver } from "./driver.ts";

export type Rect = { x: number; y: number; width: number; height: number };
export type ControlFrame = {
  id: string;
  framePt: Rect | null;
  axWindowNumber: number | null;
  error?: string | null;
};
export type DragSample = {
  tNs: number;
  phase: "pre" | "mouseDown" | "dragged" | "mouseUp" | "settling" | string;
  mainWindowNumber: number | null;
  mainFramePt: Rect | null;
  footerWindowNumber: number | null;
  footerFramePt: Rect | null;
  relevantWindowCount: number;
  controls: ControlFrame[];
};
export type NativeTrace = {
  schemaVersion: number;
  status: string;
  pid: number;
  trajectory: string;
  durationMs: number;
  requestedDeltaPt: { x: number; y: number };
  accessibilityTrusted: boolean;
  display: {
    displayID: number;
    refreshHz: number;
    backingScale: number;
    boundsPt: Rect;
  } | null;
  sampleTargetHz: number;
  samples: DragSample[];
  errors: string[];
};

export type ControlMetrics = {
  id: string;
  sampleCount: number;
  maxDriftPx: number;
  p99DriftPx: number;
  rmsDriftPx: number;
  consecutiveOverHalfPixel: number;
  settlingMs: number | null;
  stableAfterSettling: boolean;
  owningWindowNumbers: number[];
  thresholdsPass: boolean;
};

export type DragAnalysis = {
  trajectory: string;
  valid: boolean;
  verdict: "PASS" | "FAIL" | "INVALID";
  errors: string[];
  topology: "one-window" | "two-window" | "unknown";
  oneWindowInvariant: boolean;
  requiredControlCount: number;
  inMotionSampleCount: number;
  distinctMainPositions: number;
  displacementPt: number;
  cadence: {
    medianMs: number | null;
    p95Ms: number | null;
    maxMs: number | null;
    refreshPeriodMs: number;
  };
  controls: ControlMetrics[];
  motionThresholdsPass: boolean;
  overallPass: boolean;
};

const THRESHOLDS = {
  maxDriftPx: 1.0,
  p99DriftPx: 0.75,
  rmsDriftPx: 0.35,
  consecutiveOverHalfPixel: 0,
};

function quantile(values: number[], fraction: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil(sorted.length * fraction) - 1));
  return sorted[index];
}

function round(value: number, digits = 4): number {
  return Number(value.toFixed(digits));
}

function distance(a: { x: number; y: number }, b: { x: number; y: number }): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function relativeVector(sample: DragSample, control: ControlFrame) {
  if (!sample.mainFramePt || !control.framePt) return null;
  return {
    x: control.framePt.x - sample.mainFramePt.x,
    y: control.framePt.y - sample.mainFramePt.y,
  };
}

function driftForControl(
  samples: DragSample[],
  controlID: string,
  baseline: { x: number; y: number },
  scale: number,
) {
  return samples.flatMap((sample) => {
    const control = sample.controls.find((entry) => entry.id === controlID);
    if (!control) return [];
    const relative = relativeVector(sample, control);
    if (!relative) return [];
    return [{ sample, control, driftPx: distance(relative, baseline) * scale }];
  });
}

function settlingForControl(
  samples: DragSample[],
  controlID: string,
  baseline: { x: number; y: number },
  scale: number,
) {
  const settling = driftForControl(
    samples.filter((sample) => sample.phase === "settling"),
    controlID,
    baseline,
    scale,
  );
  const mouseUpNs = samples.find((sample) => sample.phase === "mouseUp")?.tNs
    ?? settling[0]?.sample.tNs
    ?? null;
  if (mouseUpNs == null) return { settlingMs: null, stable: false };
  for (let index = 0; index < settling.length; index += 1) {
    const candidate = settling[index];
    if (candidate.driftPx > 0.5) continue;
    const stableUntil = candidate.sample.tNs + 100_000_000;
    const tail = settling.filter(
      (entry) => entry.sample.tNs >= candidate.sample.tNs && entry.sample.tNs <= stableUntil,
    );
    const spansWindow = tail.at(-1)?.sample.tNs != null
      && Number(tail.at(-1)!.sample.tNs) - Number(candidate.sample.tNs) >= 90_000_000;
    if (spansWindow && tail.every((entry) => entry.driftPx <= 0.5)) {
      return {
        settlingMs: (Number(candidate.sample.tNs) - Number(mouseUpNs)) / 1_000_000,
        stable: true,
      };
    }
  }
  return { settlingMs: null, stable: false };
}

export function analyzeTrace(trace: NativeTrace): DragAnalysis {
  const errors = [...(trace.errors ?? [])];
  const samples = trace.samples ?? [];
  const inMotion = samples.filter((sample) => sample.phase === "dragged");
  const pre = samples.filter((sample) => sample.phase === "pre" || sample.phase === "mouseDown");
  const scale = trace.display?.backingScale ?? 1;
  const refreshPeriodMs = 1000 / Math.max(1, trace.display?.refreshHz ?? 60);
  const controlIDs = [...new Set(samples.flatMap((sample) => sample.controls.map((control) => control.id)))];

  const mainPositions = inMotion.flatMap((sample) => sample.mainFramePt ? [sample.mainFramePt] : []);
  const distinctMainPositions = new Set(
    mainPositions.map((frame) => `${round(frame.x, 2)},${round(frame.y, 2)}`),
  ).size;
  const displacementPt = mainPositions.length >= 2
    ? distance(mainPositions[0], mainPositions.at(-1)!)
    : 0;
  // Cadence is a motion-validity constraint. Startup AX resolution and the
  // post-drag settling tail are retained in the raw trace but must not dilute
  // or invalidate the sampling frequency achieved while the window moves.
  const intervalsMs = inMotion.slice(1).map(
    (sample, index) => (Number(sample.tNs) - Number(inMotion[index].tNs)) / 1_000_000,
  ).filter((value) => Number.isFinite(value) && value >= 0);
  const cadence = {
    medianMs: quantile(intervalsMs, 0.5),
    p95Ms: quantile(intervalsMs, 0.95),
    maxMs: intervalsMs.length ? Math.max(...intervalsMs) : null,
    refreshPeriodMs,
  };

  if (trace.status !== "ok") errors.push(`sampler status is ${trace.status}`);
  if (!trace.accessibilityTrusted) errors.push("accessibility is not trusted");
  if (controlIDs.length < 2) errors.push("fewer than two controls were sampled");
  if (inMotion.length < 36) errors.push(`only ${inMotion.length} in-motion samples`);
  if (distinctMainPositions < 30) errors.push(`only ${distinctMainPositions} distinct main positions`);
  if (displacementPt < 200) errors.push(`main displacement ${round(displacementPt)}pt is below 200pt`);
  if (cadence.medianMs == null || cadence.medianMs > 10) {
    errors.push(`median cadence ${cadence.medianMs ?? "missing"}ms exceeds 10ms`);
  }
  if (cadence.p95Ms == null || cadence.p95Ms > refreshPeriodMs) {
    errors.push(`p95 cadence ${cadence.p95Ms ?? "missing"}ms exceeds one refresh period`);
  }
  if (cadence.maxMs == null || cadence.maxMs > refreshPeriodMs * 2) {
    errors.push(`max cadence ${cadence.maxMs ?? "missing"}ms exceeds two refresh periods`);
  }

  const nativeWindowNumbers = new Set(
    samples.flatMap((sample) => [sample.mainWindowNumber, sample.footerWindowNumber])
      .filter((value): value is number => value != null),
  );
  const oneWindowInvariant = samples.length > 0
    && samples.every((sample) => sample.mainWindowNumber != null && sample.footerWindowNumber == null)
    && samples.every((sample) => sample.controls.every(
      (control) => control.axWindowNumber == null || control.axWindowNumber === sample.mainWindowNumber,
    ));
  const topology = oneWindowInvariant
    ? "one-window"
    : nativeWindowNumbers.size >= 2 || samples.some((sample) => sample.footerWindowNumber != null)
      ? "two-window"
      : "unknown";

  const controls: ControlMetrics[] = controlIDs.map((id) => {
    const baselineEntry = [...pre].reverse().flatMap((sample) => {
      const control = sample.controls.find((entry) => entry.id === id);
      if (!control) return [];
      const relative = relativeVector(sample, control);
      return relative ? [{ sample, control, relative }] : [];
    })[0];
    if (!baselineEntry) {
      errors.push(`control ${id} has no pre-drag baseline`);
      return {
        id,
        sampleCount: 0,
        maxDriftPx: Number.POSITIVE_INFINITY,
        p99DriftPx: Number.POSITIVE_INFINITY,
        rmsDriftPx: Number.POSITIVE_INFINITY,
        consecutiveOverHalfPixel: Number.POSITIVE_INFINITY,
        settlingMs: null,
        stableAfterSettling: false,
        owningWindowNumbers: [],
        thresholdsPass: false,
      };
    }
    const entries = driftForControl(inMotion, id, baselineEntry.relative, scale);
    if (entries.length !== inMotion.length) {
      errors.push(`control ${id} resolved in ${entries.length}/${inMotion.length} in-motion samples`);
    }
    const values = entries.map((entry) => entry.driftPx);
    let consecutiveOverHalfPixel = 0;
    for (let index = 1; index < values.length; index += 1) {
      if (values[index - 1] > 0.5 && values[index] > 0.5) consecutiveOverHalfPixel += 1;
    }
    const settling = settlingForControl(samples, id, baselineEntry.relative, scale);
    const settlingLimitMs = refreshPeriodMs + 4;
    const maxDriftPx = values.length ? Math.max(...values) : Number.POSITIVE_INFINITY;
    const p99DriftPx = quantile(values, 0.99) ?? Number.POSITIVE_INFINITY;
    const rmsDriftPx = values.length
      ? Math.sqrt(values.reduce((sum, value) => sum + value * value, 0) / values.length)
      : Number.POSITIVE_INFINITY;
    const owningWindowNumbers = [...new Set(entries.map((entry) => entry.control.axWindowNumber).filter(
      (value): value is number => value != null,
    ))];
    const thresholdsPass = maxDriftPx <= THRESHOLDS.maxDriftPx
      && p99DriftPx <= THRESHOLDS.p99DriftPx
      && rmsDriftPx <= THRESHOLDS.rmsDriftPx
      && consecutiveOverHalfPixel === THRESHOLDS.consecutiveOverHalfPixel
      && settling.stable
      && settling.settlingMs != null
      && settling.settlingMs <= settlingLimitMs;
    return {
      id,
      sampleCount: entries.length,
      maxDriftPx: round(maxDriftPx),
      p99DriftPx: round(p99DriftPx),
      rmsDriftPx: round(rmsDriftPx),
      consecutiveOverHalfPixel,
      settlingMs: settling.settlingMs == null ? null : round(settling.settlingMs),
      stableAfterSettling: settling.stable,
      owningWindowNumbers,
      thresholdsPass,
    };
  });

  const valid = errors.length === 0;
  const motionThresholdsPass = controls.length >= 2 && controls.every((control) => control.thresholdsPass);
  const overallPass = valid && oneWindowInvariant && motionThresholdsPass;
  return {
    trajectory: trace.trajectory,
    valid,
    verdict: valid ? overallPass ? "PASS" : "FAIL" : "INVALID",
    errors,
    topology,
    oneWindowInvariant,
    requiredControlCount: controlIDs.length,
    inMotionSampleCount: inMotion.length,
    distinctMainPositions,
    displacementPt: round(displacementPt),
    cadence: {
      medianMs: cadence.medianMs == null ? null : round(cadence.medianMs),
      p95Ms: cadence.p95Ms == null ? null : round(cadence.p95Ms),
      maxMs: cadence.maxMs == null ? null : round(cadence.maxMs),
      refreshPeriodMs: round(refreshPeriodMs),
    },
    controls,
    motionThresholdsPass,
    overallPass,
  };
}

async function run(command: string[], options: { stdout?: "pipe" | "ignore" } = {}) {
  const child = Bun.spawn(command, {
    stdout: options.stdout ?? "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    options.stdout === "ignore" ? Promise.resolve("") : new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { stdout, stderr, exitCode };
}

async function runFilmstrip(
  helper: string,
  pid: number,
  trajectory: string,
  outDir: string,
) {
  const durationSeconds = trajectory === "slow-horizontal" ? 0.9
    : trajectory === "diagonal" ? 0.7
    : 0.3;
  const rawPath = join(outDir, `${trajectory}-filmstrip-raw.json`);
  const child = Bun.spawn([
    helper,
    "--pid",
    String(pid),
    "--trajectory",
    trajectory,
    "--output",
    rawPath,
  ], { stdout: "ignore", stderr: "pipe" });
  const captures = [0.25, 0.5, 0.75].map(async (fraction, index) => {
    await Bun.sleep((0.145 + durationSeconds * fraction) * 1000);
    const path = join(outDir, `${trajectory}-filmstrip-${index + 1}.png`);
    const capture = await run(["screencapture", "-x", path]);
    return {
      fraction,
      path,
      exists: existsSync(path),
      sha256: existsSync(path) ? sha256(path) : null,
      exitCode: capture.exitCode,
      stderr: capture.stderr,
    };
  });
  const [stderr, exitCode, frames] = await Promise.all([
    new Response(child.stderr).text(),
    child.exited,
    Promise.all(captures),
  ]);
  return {
    trajectory,
    rawPath,
    rawSha256: existsSync(rawPath) ? sha256(rawPath) : null,
    exitCode,
    stderr,
    frames,
  };
}

function sha256(path: string) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function parseCLI() {
  const args = process.argv.slice(2);
  const value = (name: string, fallback?: string) => {
    const index = args.indexOf(name);
    return index >= 0 && args[index + 1] ? args[index + 1] : fallback;
  };
  const binary = value("--binary") ?? process.env.SCRIPT_KIT_GPUI_BINARY;
  const outDir = resolve(value("--out", ".artifacts/main-window-native-drag/run")!);
  const trials = value("--trials", "slow-horizontal,fast-horizontal,diagonal")!
    .split(",")
    .filter(Boolean);
  const expectFallback = args.includes("--expect-fallback");
  return { binary, outDir, trials, expectFallback };
}

async function cli() {
  const { binary, outDir, trials, expectFallback } = parseCLI();
  if (!binary || !existsSync(binary)) throw new Error(`binary missing: ${binary ?? "<unset>"}`);
  mkdirSync(outDir, { recursive: true });
  const helper = join(outDir, "macos-native-drag-sampler");
  const compile = await run([
    "swiftc",
    resolve(import.meta.dir, "../agentic/macos-native-drag-sampler.swift"),
    "-o",
    helper,
  ]);
  if (compile.exitCode !== 0) throw new Error(`Swift helper compile failed: ${compile.stderr}`);

  const receipt: Record<string, unknown> = {
    schemaVersion: 1,
    startedAt: new Date().toISOString(),
    gitCommit: (await run(["git", "rev-parse", "HEAD"])).stdout.trim(),
    binary: resolve(binary),
    binarySha256: sha256(binary),
    helperSha256: sha256(helper),
    macOS: (await run(["sw_vers", "-productVersion"])).stdout.trim(),
    trials: [],
  };

  const driver = await Driver.launch({
    binary: resolve(binary),
    sessionName: `main-window-native-drag-${process.pid}`,
    sandboxHome: true,
    defaultTimeoutMs: 15_000,
    env: {
      SCRIPT_KIT_FIDELITY_CAPTURE: "agent-chat",
      ...(expectFallback ? { SCRIPT_KIT_DEBUG_NO_GLASS: "1" } : {}),
    },
  });
  receipt.sessionDir = driver.sessionDir;
  try {
    driver.send({ type: "show" });
    await driver.waitForState({ windowVisible: true }, { timeoutMs: 15_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    const windows = await driver.listAutomationWindows({ timeoutMs: 15_000 });
    const main = (windows.windows ?? []).find((window: any) => window.id === "main");
    if (!main?.pid) throw new Error("main automation window PID missing");
    receipt.pid = main.pid;
    receipt.initialAutomationWindows = windows;

    const compositionSnapshot = async () => {
      const [state, layout, automationWindows] = await Promise.all([
        driver.getState({ timeoutMs: 15_000 }),
        driver.getLayoutInfo(
          { target: { type: "id", id: "main" } },
          { timeoutMs: 15_000 },
        ),
        driver.listAutomationWindows({ timeoutMs: 15_000 }),
      ]);
      const appKit = (layout as any)?.fidelity?.appKit ?? null;
      const processWindows = ((automationWindows as any)?.windows ?? [])
        .filter((candidate: any) => candidate.pid === main.pid);
      return {
        windowVisible: (state as any)?.windowVisible ?? null,
        windowFocused: (state as any)?.windowFocused ?? null,
        promptType: (state as any)?.promptType ?? null,
        mainBackdropFrame: appKit?.mainBackdropFrame ?? null,
        footerContainerFrame: appKit?.footerContainerFrame ?? null,
        transparentGapPoints: appKit?.transparentGapPoints ?? null,
        backdropFooterIntersectionArea: appKit?.backdropFooterIntersectionArea ?? null,
        outerWindowHasShadow: appKit?.outerWindowHasShadow ?? null,
        processWindowIds: processWindows.map((candidate: any) => candidate.id),
      };
    };

    const showHideCycles: Array<Record<string, unknown>> = [];
    for (let cycle = 1; cycle <= 10; cycle += 1) {
      driver.send({ type: "hide", requestId: `mwnd-hide-${cycle}` });
      await driver.waitForState({ windowVisible: false }, { timeoutMs: 15_000 });
      const hidden = await driver.getState({ timeoutMs: 15_000 });
      driver.send({ type: "show", requestId: `mwnd-show-${cycle}` });
      await driver.waitForState({ windowVisible: true }, { timeoutMs: 15_000 });
      await driver.waitForSettle({ timeoutMs: 10_000 });
      const shownAttempts = [await compositionSnapshot()];
      if (shownAttempts[0]?.windowVisible !== true) {
        // A human click/hotkey can conceal the panel between the visibility
        // acknowledgement and the snapshot. Re-run only that sample and keep
        // both attempts in the receipt so test interference is explicit.
        driver.send({ type: "show", requestId: `mwnd-show-${cycle}-retry` });
        await driver.waitForState({ windowVisible: true }, { timeoutMs: 15_000 });
        await driver.waitForSettle({ timeoutMs: 10_000 });
        shownAttempts.push(await compositionSnapshot());
      }
      showHideCycles.push({
        cycle,
        hiddenVisible: (hidden as any)?.windowVisible ?? null,
        shownAttempts,
        shown: shownAttempts.at(-1),
      });
    }

    const modeTransitions: Array<Record<string, unknown>> = [];
    for (let transition = 1; transition <= 20; transition += 1) {
      const builtinId = transition % 2 === 1
        ? "builtin/choose-theme"
        : "builtin/main-window";
      driver.send({
        type: "triggerBuiltin",
        builtinId,
        requestId: `mwnd-mode-${transition}`,
      });
      await driver.waitForSettle({ timeoutMs: 10_000 });
      modeTransitions.push({
        transition,
        builtinId,
        snapshot: await compositionSnapshot(),
      });
    }
    receipt.lifecycle = { showHideCycles, modeTransitions };

    const results: Array<Record<string, unknown>> = [];
    for (const trajectory of trials) {
      const attempts: Array<Record<string, unknown>> = [];
      let selected: Record<string, any> | null = null;
      for (let attempt = 1; attempt <= 3; attempt += 1) {
        await driver.waitForSettle({ timeoutMs: 5_000 });
        const rawPath = join(outDir, `${trajectory}-attempt-${attempt}-raw.json`);
        const helperRun = await run([
          helper,
          "--pid",
          String(main.pid),
          "--trajectory",
          trajectory,
          "--output",
          rawPath,
        ], { stdout: "ignore" });
        const trace = JSON.parse(readFileSync(rawPath, "utf8")) as NativeTrace;
        const analysis = analyzeTrace(trace);
        const entry = {
          attempt,
          rawPath,
          rawSha256: sha256(rawPath),
          helperExitCode: helperRun.exitCode,
          helperStderr: helperRun.stderr,
          analysis,
        };
        attempts.push(entry);
        selected = entry;
        if (analysis.valid) break;
      }
      const filmstrip = selected?.analysis?.valid
        ? await runFilmstrip(helper, Number(main.pid), trajectory, outDir)
        : null;
      results.push({
        trajectory,
        attempts,
        filmstrip,
        selectedAttempt: selected?.attempt ?? null,
        rawPath: selected?.rawPath ?? null,
        rawSha256: selected?.rawSha256 ?? null,
        helperExitCode: selected?.helperExitCode ?? null,
        helperStderr: selected?.helperStderr ?? null,
        analysis: selected?.analysis ?? null,
      });
    }
    receipt.trials = results;
    receipt.state = await driver.getState({ timeoutMs: 15_000 });
    receipt.layout = await driver.getLayoutInfo(
      { target: { type: "id", id: "main" } },
      { timeoutMs: 15_000 },
    );
    receipt.logs = await driver.getLogs({ limit: 500 });
    const compositionIsValid = (snapshot: any) => {
      if (expectFallback) {
        return snapshot?.windowVisible === true
          && snapshot?.mainBackdropFrame == null
          && snapshot?.footerContainerFrame == null
          && snapshot?.transparentGapPoints == null
          && snapshot?.backdropFooterIntersectionArea == null
          && snapshot?.outerWindowHasShadow === true
          && snapshot?.processWindowIds?.includes("main");
      }
      return snapshot?.windowVisible === true
        && snapshot?.transparentGapPoints === 8
        && snapshot?.backdropFooterIntersectionArea === 0
        && snapshot?.outerWindowHasShadow === false
        && snapshot?.processWindowIds?.length === 1
        && snapshot?.processWindowIds?.[0] === "main";
    };
    const lifecyclePass = showHideCycles.every((cycle: any) =>
      cycle.hiddenVisible === false && compositionIsValid(cycle.shown)
    ) && modeTransitions.every((transition: any) =>
      compositionIsValid(transition.snapshot)
    );
    receipt.lifecyclePass = lifecyclePass;
    receipt.expectFallback = expectFallback;
    receipt.valid = results.every((result: any) => result.analysis.valid);
    receipt.pass = lifecyclePass
      && results.every((result: any) => result.analysis.overallPass);
  } finally {
    try {
      driver.send({ type: "hide" });
      await driver.waitForState({ windowVisible: false }, { timeoutMs: 5_000 });
    } catch {}
    await driver.close();
    receipt.driverStats = driver.stats;
    receipt.cleanedUp = true;
    receipt.finishedAt = new Date().toISOString();
  }
  const receiptPath = join(outDir, "receipt.json");
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(JSON.stringify({
    receiptPath,
    valid: receipt.valid,
    pass: receipt.pass,
    trials: (receipt.trials as any[]).map((trial) => ({
      trajectory: trial.trajectory,
      verdict: trial.analysis.verdict,
      topology: trial.analysis.topology,
      displacementPt: trial.analysis.displacementPt,
      maxDriftPx: trial.analysis.controls.map((control: any) => control.maxDriftPx),
      errors: trial.analysis.errors,
    })),
  }, null, 2));
  if (receipt.valid !== true) process.exitCode = 2;
  else if (receipt.pass !== true) process.exitCode = 1;
}

if (import.meta.main) {
  await cli();
}
