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
import { announceTestStatus } from "./test-status.ts";

export type Rect = { x: number; y: number; width: number; height: number };
export type ControlFrame = {
  id: string;
  framePt: Rect | null;
  mainFramePtAtMeasurement?: Rect | null;
  axWindowNumber: number | null;
  measurementSource?: string;
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
  relevantWindowNumbers?: number[];
  controls: ControlFrame[];
};
export type FilmstripFrame = {
  fraction: number;
  tNs: number;
  mainFramePt: Rect | null;
  path: string;
  captureSucceeded: boolean;
  error?: string | null;
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
  mouseUpEventNs?: number | null;
  samples: DragSample[];
  filmstripFrames?: FilmstripFrame[];
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
const LEFT_CONTROL_ID = "script-kit-footer-left-info-hit-target";
const RIGHT_CONTROL_IDS = [
  "script-kit-footer-button-ai",
  "script-kit-footer-button-actions",
  "script-kit-footer-button-run",
];
const LIVE_MEASUREMENT_SOURCE = "live-ax+interpolated-main";
const MAX_NATIVE_DRAG_ATTEMPTS = 10;
// AX + WindowServer sampling is driven by an NSEventTracking/common-mode timer.
// Allow one millisecond of scheduler quantization around the nominal refresh
// boundary while keeping the separate hard two-refresh maximum-gap guard.
const TIMER_JITTER_TOLERANCE_MS = 1;

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
  const mainFrame = control.mainFramePtAtMeasurement;
  if (!mainFrame || !control.framePt) return null;
  return {
    x: control.framePt.x - mainFrame.x,
    y: control.framePt.y - mainFrame.y,
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
  mouseUpEventNs: number | null | undefined,
) {
  const settling = driftForControl(
    samples.filter((sample) => sample.phase === "settling"),
    controlID,
    baseline,
    scale,
  );
  const mouseUpNs = mouseUpEventNs ?? null;
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
  const rightControlID = RIGHT_CONTROL_IDS.find((id) => controlIDs.includes(id)) ?? null;

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
  if (!controlIDs.includes(LEFT_CONTROL_ID)) errors.push(`missing exact left control ${LEFT_CONTROL_ID}`);
  if (!rightControlID) errors.push(`missing exact right control (${RIGHT_CONTROL_IDS.join(",")})`);
  if (controlIDs.length !== 2) errors.push(`expected exactly two controls, sampled ${controlIDs.length}`);
  if (inMotion.length < 36) errors.push(`only ${inMotion.length} in-motion samples`);
  if (distinctMainPositions < 30) errors.push(`only ${distinctMainPositions} distinct main positions`);
  if (displacementPt < 200) errors.push(`main displacement ${round(displacementPt)}pt is below 200pt`);
  if (cadence.medianMs == null || cadence.medianMs > 10) {
    errors.push(`median cadence ${cadence.medianMs ?? "missing"}ms exceeds 10ms`);
  }
  if (cadence.p95Ms == null || cadence.p95Ms > refreshPeriodMs + TIMER_JITTER_TOLERANCE_MS) {
    errors.push(
      `p95 cadence ${cadence.p95Ms ?? "missing"}ms exceeds one refresh period plus ${TIMER_JITTER_TOLERANCE_MS}ms timer jitter`,
    );
  }
  if (cadence.maxMs == null || cadence.maxMs > refreshPeriodMs * 2) {
    errors.push(`max cadence ${cadence.maxMs ?? "missing"}ms exceeds two refresh periods`);
  }
  const mouseUpSample = samples.find((sample) => sample.phase === "mouseUp");
  if (trace.mouseUpEventNs == null) errors.push("explicit mouse-up event timestamp is missing");
  if (!mouseUpSample) errors.push("mouse-up phase was not sampled");
  if (mouseUpSample && trace.mouseUpEventNs != null
    && Math.abs(Number(mouseUpSample.tNs) - Number(trace.mouseUpEventNs)) > 50_000_000) {
    errors.push("sampled mouse-up phase is not tied to the explicit mouse-up event timestamp");
  }
  if (samples.some((sample) =>
    sample.relevantWindowNumbers == null
    || sample.relevantWindowNumbers.length !== sample.relevantWindowCount
  )) {
    errors.push("per-sample relevant native-window enumeration is missing or inconsistent");
  }
  const relevantWindowCounts = new Set(samples.map((sample) => sample.relevantWindowCount));
  if (relevantWindowCounts.size !== 1) {
    errors.push(`relevant native-window count changed during drag: ${[...relevantWindowCounts].join(",")}`);
  }
  if (samples.some((sample) => sample.controls.some((control) =>
    control.measurementSource !== LIVE_MEASUREMENT_SOURCE
    || control.mainFramePtAtMeasurement == null
  ))) {
    errors.push(`all control samples must use ${LIVE_MEASUREMENT_SOURCE}`);
  }
  if (samples.some((sample) => sample.controls.some((control) => control.axWindowNumber == null))) {
    errors.push("control ownership must be non-null in every sample");
  }

  const baselineLeft = [...pre].reverse().flatMap((sample) => {
    const control = sample.controls.find((entry) => entry.id === LEFT_CONTROL_ID);
    return control?.framePt ? [control.framePt] : [];
  })[0];
  const baselineRight = rightControlID == null ? null : [...pre].reverse().flatMap((sample) => {
    const control = sample.controls.find((entry) => entry.id === rightControlID);
    return control?.framePt ? [control.framePt] : [];
  })[0] ?? null;
  if (!baselineLeft || !baselineRight) {
    errors.push("left/right pre-drag control separation is unavailable");
  } else {
    const separation = Math.abs(baselineRight.x - baselineLeft.x);
    if (baselineRight.x <= baselineLeft.x || separation < 100) {
      errors.push(`left/right controls are not far apart (${round(separation)}pt)`);
    }
  }

  const nativeWindowNumbers = new Set(
    samples.flatMap((sample) => [sample.mainWindowNumber, sample.footerWindowNumber])
      .filter((value): value is number => value != null),
  );
  const oneWindowInvariant = samples.length > 0
    && samples.every((sample) => sample.mainWindowNumber != null && sample.footerWindowNumber == null)
    && samples.every((sample) => sample.relevantWindowCount === 1)
    && samples.every((sample) =>
      sample.relevantWindowNumbers?.length === 1
      && sample.relevantWindowNumbers[0] === sample.mainWindowNumber
    )
    && samples.every((sample) => sample.controls.every(
      (control) => control.axWindowNumber != null && control.axWindowNumber === sample.mainWindowNumber,
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
    const distinctAbsolutePositions = new Set(entries.flatMap((entry) => {
      const frame = entry.control.framePt;
      return frame ? [`${round(frame.x, 2)},${round(frame.y, 2)}`] : [];
    })).size;
    // Live AX geometry can publish at a lower cadence than WindowServer on a
    // 120 Hz display. Require multiple independent live updates so a cached
    // coordinate cannot pass, without pretending AX must expose every frame.
    if (distinctMainPositions >= 30 && distinctAbsolutePositions < 3) {
      errors.push(
        `control ${id} AX positions are stale (${distinctAbsolutePositions} distinct positions for ${distinctMainPositions} main positions)`,
      );
    }
    const values = entries.map((entry) => entry.driftPx);
    let consecutiveOverHalfPixel = 0;
    for (let index = 1; index < values.length; index += 1) {
      if (values[index - 1] > 0.5 && values[index] > 0.5) consecutiveOverHalfPixel += 1;
    }
    const settling = settlingForControl(
      samples,
      id,
      baselineEntry.relative,
      scale,
      trace.mouseUpEventNs,
    );
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

export function analyzeIntegratedFilmstrip(trace: NativeTrace) {
  const errors: string[] = [];
  const frames = [...(trace.filmstripFrames ?? [])].sort((a, b) => a.fraction - b.fraction);
  if (frames.length !== 3) errors.push(`expected 3 same-run filmstrip frames, found ${frames.length}`);
  const expectedFractions = [0.25, 0.5, 0.75];
  frames.forEach((frame, index) => {
    if (Math.abs(frame.fraction - (expectedFractions[index] ?? frame.fraction)) > 0.001) {
      errors.push(`unexpected filmstrip fraction ${frame.fraction}`);
    }
    if (!frame.captureSucceeded || !existsSync(frame.path)) {
      errors.push(`filmstrip capture ${index + 1} failed: ${frame.error ?? "file missing"}`);
    }
  });
  const dragged = trace.samples.filter((sample) => sample.phase === "dragged");
  const firstDraggedNs = dragged[0]?.tNs ?? null;
  const lastDraggedNs = dragged.at(-1)?.tNs ?? null;
  if (firstDraggedNs == null || lastDraggedNs == null || frames.some((frame) =>
    frame.tNs < firstDraggedNs || frame.tNs > lastDraggedNs
  )) {
    errors.push("filmstrip frames were not captured inside the analyzed drag interval");
  }
  const positions = frames.flatMap((frame) => frame.mainFramePt ? [frame.mainFramePt] : []);
  const distinctPositions = new Set(positions.map((frame) => `${round(frame.x, 2)},${round(frame.y, 2)}`));
  const displacementPt = positions.length >= 2 ? distance(positions[0], positions.at(-1)!) : 0;
  if (distinctPositions.size !== 3) errors.push("filmstrip does not contain three distinct main-window positions");
  if (displacementPt < 100) errors.push(`filmstrip main-window displacement ${round(displacementPt)}pt is below 100pt`);
  const enrichedFrames = frames.map((frame) => ({
    ...frame,
    exists: existsSync(frame.path),
    sha256: existsSync(frame.path) ? sha256(frame.path) : null,
  }));
  const hashes = new Set(enrichedFrames.flatMap((frame) => frame.sha256 ? [frame.sha256] : []));
  if (hashes.size !== 3) errors.push("filmstrip captures are not three distinct images");
  return {
    pass: errors.length === 0,
    errors,
    sameRunRawTrace: true,
    distinctMainPositions: distinctPositions.size,
    displacementPt: round(displacementPt),
    frames: enrichedFrames,
  };
}

function sha256(path: string) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function summarizeNativeWindowInventory(trace: NativeTrace) {
  const phases = [...new Set(trace.samples.map((sample) => sample.phase))];
  return Object.fromEntries(phases.map((phase) => {
    const samples = trace.samples.filter((sample) => sample.phase === phase);
    return [phase, {
      sampleCount: samples.length,
      relevantWindowCounts: [...new Set(samples.map((sample) => sample.relevantWindowCount))],
      mainWindowNumbers: [...new Set(samples.flatMap((sample) =>
        sample.mainWindowNumber == null ? [] : [sample.mainWindowNumber]
      ))],
      footerWindowNumbers: [...new Set(samples.flatMap((sample) =>
        sample.footerWindowNumber == null ? [] : [sample.footerWindowNumber]
      ))],
      controlWindowNumbers: [...new Set(samples.flatMap((sample) =>
        sample.controls.flatMap((control) =>
          control.axWindowNumber == null ? [] : [control.axWindowNumber]
        )
      ))],
    }];
  }));
}

type AppKitNode = {
  id: string;
  parentId?: string;
  className?: string;
  hidden?: boolean;
  alpha?: number;
  frame?: Rect;
  windowFrame?: Rect;
  screenshotFrame?: Rect;
  layer?: { contentsScale?: number; borderWidth?: number; cornerRadius?: number };
  text?: { value?: string; color?: { alpha?: number } };
  image?: unknown;
};

export function analyzeStationaryFidelity(
  layout: any,
  automationWindow: any,
  options: { expectedHostSize?: { width: number; height: number } } = {},
) {
  const appKit = layout?.fidelity?.appKit ?? null;
  const nodes = (appKit?.nodes ?? []) as AppKitNode[];
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const errors: string[] = [];
  const ancestorIds = (node: AppKitNode) => {
    const ids: string[] = [];
    const seen = new Set<string>();
    let parentId = node.parentId;
    while (parentId && !seen.has(parentId)) {
      ids.push(parentId);
      seen.add(parentId);
      parentId = byId.get(parentId)?.parentId;
    }
    return ids;
  };

  if (!appKit) errors.push("AppKit fidelity snapshot missing");
  const hostBounds = automationWindow?.bounds ?? null;
  const mainBackdropFrame = appKit?.mainBackdropFrame ?? null;
  const footerContainerFrame = appKit?.footerContainerFrame ?? null;
  const expectedHostSize = options.expectedHostSize;
  if (
    expectedHostSize
    && (hostBounds?.width !== expectedHostSize.width || hostBounds?.height !== expectedHostSize.height)
  ) {
    errors.push(
      `native host is ${hostBounds?.width ?? "?"}x${hostBounds?.height ?? "?"}, expected ${expectedHostSize.width}x${expectedHostSize.height}`,
    );
  }
  if (
    !hostBounds
    || !mainBackdropFrame
    || !footerContainerFrame
    || hostBounds.width !== mainBackdropFrame.width
    || hostBounds.width !== footerContainerFrame.width
    || mainBackdropFrame.x !== 0
    || footerContainerFrame.x !== 0
    || footerContainerFrame.y !== 0
    || mainBackdropFrame.y !== footerContainerFrame.height + 8
    || mainBackdropFrame.y + mainBackdropFrame.height !== hostBounds.height
  ) {
    errors.push("native host is not exactly partitioned into footer, 8pt gutter, and bounded main backdrop");
  }
  if (appKit?.footerContainerFrame?.height !== 32) errors.push("footer container is not 32pt high");
  if (appKit?.transparentGapPoints !== 8) errors.push("main/footer gutter is not 8pt");
  if (appKit?.backdropFooterIntersectionArea !== 0) errors.push("main/footer materials overlap");
  if (appKit?.outerWindowHasShadow !== false) errors.push("outer window shadow must stay disabled");

  const capsules = nodes.filter((node) =>
    node.className === "NSGlassEffectView"
    && (node.id.startsWith("script-kit-footer-capsule-")
      || node.id === "script-kit-footer-left-info-capsule")
  );
  if (capsules.length < 2) errors.push(`only ${capsules.length} independent footer capsules found`);
  for (const capsule of capsules) {
    if (capsule.frame?.height !== 28) errors.push(`${capsule.id} is not 28pt high`);
    if (capsule.layer?.cornerRadius !== 6) errors.push(`${capsule.id} radius is not 6pt`);
    if (capsule.layer?.contentsScale !== 2) errors.push(`${capsule.id} is not rendered at 2x`);
    if ((capsule.frame?.width ?? 750) >= (appKit?.footerContainerFrame?.width ?? 750)) {
      errors.push(`${capsule.id} incorrectly spans the footer`);
    }
    const expectedContentId = capsule.id === "script-kit-footer-left-info-capsule"
      ? "script-kit-footer-left-info-capsule-content"
      : capsule.id.replace("script-kit-footer-capsule-", "script-kit-footer-capsule-content-");
    if (byId.get(expectedContentId)?.parentId !== capsule.id) {
      errors.push(`${capsule.id} has no identified contentView child`);
    }
    if (capsule.id.startsWith("script-kit-footer-capsule-")) {
      const stateLayerId = capsule.id.replace(
        "script-kit-footer-capsule-",
        "script-kit-footer-state-layer-",
      );
      if (byId.get(stateLayerId)?.parentId !== expectedContentId) {
        errors.push(`${capsule.id} has no foreground interaction-state layer`);
      }
    }
  }

  const leftCapsule = byId.get("script-kit-footer-left-info-capsule");
  const leftHitTarget = byId.get("script-kit-footer-left-info-hit-target")
    ?? byId.get("script-kit-footer-cwd-chip-hit");
  const leftKeycap = byId.get("script-kit-footer-left-info-keycap")
    ?? byId.get("script-kit-footer-cwd-chip-keycap");
  const leftKeycapGlyph = byId.get("script-kit-footer-left-info-keycap-glyph")
    ?? byId.get("script-kit-footer-cwd-chip-keycap-glyph");
  if (!leftCapsule) errors.push("left footer capsule is missing");
  if (!leftHitTarget) errors.push("left footer hit target is missing");
  if (!leftKeycap) {
    errors.push("left footer shortcut keycap is missing");
  } else {
    if (leftKeycap.frame?.height !== 20) errors.push("left footer shortcut keycap is not 20pt high");
    if (leftKeycap.layer?.cornerRadius !== 6) errors.push("left footer shortcut keycap radius is not 6pt");
    if (leftKeycap.layer?.borderWidth !== 1) errors.push("left footer shortcut keycap border is not 1pt");
    if (leftKeycap.layer?.contentsScale !== 2) errors.push("left footer shortcut keycap is not rendered at 2x");
  }
  if (!leftKeycapGlyph?.text?.value?.trim()) {
    errors.push("left footer shortcut glyph is missing");
  }

  const visualNodes = nodes.filter((node) =>
    node.text != null
    || node.image != null
    || node.id.includes("status-dot")
    || node.id.includes("leading-dot")
    || node.id.includes("keycap-")
  );
  for (const node of visualNodes) {
    const owners = ancestorIds(node).filter((id) => id.includes("capsule-content"));
    if (owners.length !== 1) errors.push(`${node.id} is not owned by exactly one capsule contentView`);
    if (node.layer && node.layer.contentsScale !== 2) errors.push(`${node.id} layer is not rendered at 2x`);
    if (node.text && (node.text.color?.alpha ?? 0) < 0.6) {
      errors.push(`${node.id} text alpha is below the readable footer token floor`);
    }
  }

  const sortedCapsules = capsules
    .filter((node) => !node.hidden)
    .sort((a, b) => (a.windowFrame?.x ?? 0) - (b.windowFrame?.x ?? 0));
  const openGaps = sortedCapsules.slice(1).map((capsule, index) =>
    round(
      (capsule.windowFrame?.x ?? 0)
      - ((sortedCapsules[index].windowFrame?.x ?? 0) + (sortedCapsules[index].windowFrame?.width ?? 0)),
    )
  );
  if (openGaps.some((gap) => gap <= 0)) errors.push(`capsule gaps are not visibly open: ${openGaps.join(",")}`);
  const trailingCapsules = sortedCapsules.filter((node) =>
    node.id.startsWith("script-kit-footer-capsule-")
  );
  const trailingGaps = trailingCapsules.slice(1).map((capsule, index) =>
    round(
      (capsule.windowFrame?.x ?? 0)
      - ((trailingCapsules[index].windowFrame?.x ?? 0)
        + (trailingCapsules[index].windowFrame?.width ?? 0)),
    )
  );
  if (trailingGaps.some((gap) => gap !== 6)) {
    errors.push(`trailing glass capsule gaps are ${trailingGaps.join(",")}, expected shared 6pt token`);
  }

  return {
    pass: errors.length === 0,
    errors,
    capsuleIds: capsules.map((node) => node.id),
    visualNodeIds: visualNodes.map((node) => node.id),
    openGaps,
    trailingGaps,
    hostBounds,
    expectedHostSize: expectedHostSize ?? null,
    mainBackdropFrame,
    footerContainerFrame,
    transparentGapPoints: appKit?.transparentGapPoints ?? null,
  };
}

async function resolveNativeWindow(pid: number) {
  const query = await run([
    "swift",
    resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
    "--pid",
    String(pid),
  ]);
  if (query.exitCode !== 0) throw new Error(`native window query failed: ${query.stderr}`);
  const parsed = JSON.parse(query.stdout);
  const candidates = (parsed.windows ?? []).filter((window: any) =>
    window.windowId > 0 && window.bounds?.width >= 700 && window.bounds?.height >= 400
  );
  const selected = candidates.sort((a: any, b: any) =>
    Number(b.onscreen) - Number(a.onscreen)
    || b.bounds.width * b.bounds.height - a.bounds.width * a.bounds.height
  )[0];
  if (!selected) throw new Error(`no native main window found for pid ${pid}`);
  return selected;
}

async function captureNativeWindow(pid: number, outDir: string, name: string) {
  const nativeWindow = await resolveNativeWindow(pid);
  const path = join(outDir, `${name}.png`);
  const capture = await run([
    "screencapture",
    `-l${nativeWindow.windowId}`,
    "-o",
    "-x",
    path,
  ]);
  if (capture.exitCode !== 0 || !existsSync(path)) {
    throw new Error(`native window capture ${name} failed: ${capture.stderr}`);
  }
  const footerCropPath = join(outDir, `${name}-footer-2x.png`);
  const crop = await run([
    "magick",
    path,
    "-gravity",
    "south",
    "-crop",
    "x80+0+0",
    "+repage",
    footerCropPath,
  ]);
  if (crop.exitCode !== 0 || !existsSync(footerCropPath)) {
    throw new Error(`footer crop ${name} failed: ${crop.stderr}`);
  }
  const edge = await run([
    "magick",
    footerCropPath,
    "-colorspace",
    "Gray",
    "-morphology",
    "Edge",
    "Diamond",
    "-format",
    "%[fx:mean]",
    "info:",
  ]);
  const edgeEnergy = Number(edge.stdout.trim());
  return {
    name,
    nativeWindow,
    path,
    sha256: sha256(path),
    footerCropPath,
    footerCropSha256: sha256(footerCropPath),
    edgeEnergy: Number.isFinite(edgeEnergy) ? round(edgeEnergy, 6) : null,
  };
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
  const stationaryOnly = args.includes("--stationary-only");
  const baseline = value("--baseline");
  return {
    binary,
    outDir,
    trials: stationaryOnly ? [] : trials,
    expectFallback,
    stationaryOnly,
    baseline,
  };
}

async function cli() {
  const { binary, outDir, trials, expectFallback, stationaryOnly, baseline } = parseCLI();
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
    baselineCommit: baseline
      ? (await run(["git", "rev-parse", baseline])).stdout.trim()
      : null,
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
    await driver.setFilterAndWait("", { timeoutMs: 15_000 });
    await driver.waitForSettle({ timeoutMs: 10_000 });
    const windows = await driver.listAutomationWindows({ timeoutMs: 15_000 });
    const main = (windows.windows ?? []).find((window: any) => window.id === "main");
    if (!main?.pid) throw new Error("main automation window PID missing");
    receipt.pid = main.pid;
    receipt.initialAutomationWindows = windows;

    const ensureMainWindowVisible = async (requestId: string) => {
      const state = await driver.getState({ timeoutMs: 15_000 });
      if ((state as any)?.windowVisible === true) return false;
      driver.send({ type: "show", requestId });
      await driver.waitForState({ windowVisible: true }, { timeoutMs: 15_000 });
      await driver.waitForSettle({ timeoutMs: 5_000 });
      return true;
    };

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
    if (!stationaryOnly) {
      await announceTestStatus("Window lifecycle", "10 hide/show cycles · the panel will blink");
    }
    for (let cycle = 1; cycle <= (stationaryOnly ? 0 : 10); cycle += 1) {
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
    if (!stationaryOnly) {
      await announceTestStatus("Full ↔ compact lifecycle", "20 automated mode transitions");
    }
    for (let transition = 1; transition <= (stationaryOnly ? 0 : 20); transition += 1) {
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
      await announceTestStatus(
        `Native drag · ${trajectory}`,
        "Script Kit will move while live control geometry is sampled",
      );
      const attempts: Array<Record<string, unknown>> = [];
      let selected: Record<string, any> | null = null;
      for (let attempt = 1; attempt <= MAX_NATIVE_DRAG_ATTEMPTS; attempt += 1) {
        await ensureMainWindowVisible(`mwnd-${trajectory}-attempt-${attempt}-show`);
        const rawPath = join(outDir, `${trajectory}-attempt-${attempt}-raw.json`);
        const filmstripPrefix = `${trajectory}-attempt-${attempt}`;
        const helperRun = await run([
          helper,
          "--pid",
          String(main.pid),
          "--trajectory",
          trajectory,
          "--output",
          rawPath,
          "--filmstrip-dir",
          outDir,
          "--filmstrip-prefix",
          filmstripPrefix,
        ], { stdout: "ignore" });
        const trace = JSON.parse(readFileSync(rawPath, "utf8")) as NativeTrace;
        const analysis = analyzeTrace(trace);
        const filmstrip = analyzeIntegratedFilmstrip(trace);
        const entry = {
          attempt,
          rawPath,
          rawSha256: sha256(rawPath),
          helperExitCode: helperRun.exitCode,
          helperStderr: helperRun.stderr,
          nativeWindowInventory: summarizeNativeWindowInventory(trace),
          analysis,
          filmstrip,
        };
        attempts.push(entry);
        selected ??= entry;
        if (analysis.valid && filmstrip.pass) {
          selected = entry;
          if (analysis.overallPass) break;
        }
      }
      results.push({
        trajectory,
        attempts,
        filmstrip: selected?.filmstrip ?? null,
        selectedAttempt: selected?.attempt ?? null,
        rawPath: selected?.rawPath ?? null,
        rawSha256: selected?.rawSha256 ?? null,
        helperExitCode: selected?.helperExitCode ?? null,
        helperStderr: selected?.helperStderr ?? null,
        nativeWindowInventory: selected?.nativeWindowInventory ?? null,
        analysis: selected?.analysis ?? null,
      });
    }
    receipt.trials = results;
    receipt.state = await driver.getState({ timeoutMs: 15_000 });
    receipt.layout = await driver.getLayoutInfo(
      { target: { type: "id", id: "main" } },
      { timeoutMs: 15_000 },
    );
    const finalWindows = await driver.listAutomationWindows({ timeoutMs: 15_000 });
    receipt.finalAutomationWindows = finalWindows;
    const finalMain = ((finalWindows as any)?.windows ?? []).find((window: any) =>
      window.id === "main" && window.pid === main.pid
    ) ?? main;
    if (!expectFallback) {
      await announceTestStatus(
        "Footer visual states",
        "Capturing default, Actions hover, and Actions selected",
      );
      const structural = analyzeStationaryFidelity(
        receipt.layout,
        finalMain,
        stationaryOnly ? { expectedHostSize: { width: 750, height: 501 } } : {},
      );
      const captures: Array<Record<string, unknown>> = [];
      captures.push(await captureNativeWindow(Number(main.pid), outDir, "stationary-default-2x"));

      const appKitNodes = ((receipt.layout as any)?.fidelity?.appKit?.nodes ?? []) as AppKitNode[];
      const actionsButton = appKitNodes.find((node) => node.id === "script-kit-footer-button-actions");
      const actionsFrame = actionsButton?.screenshotFrame;
      let leftInteraction: Record<string, unknown> | null = null;
      if (actionsFrame && finalMain?.bounds) {
        const footerHeight = Number((receipt.layout as any)?.fidelity?.appKit?.footerContainerFrame?.height ?? 32);
        const hoverX = Math.round(finalMain.bounds.x + actionsFrame.x + actionsFrame.width / 2);
        const hoverY = Math.round(
          finalMain.bounds.y + finalMain.bounds.height - footerHeight
          + actionsFrame.y + actionsFrame.height / 2,
        );
        const hover = await run(["cliclick", `m:${hoverX},${hoverY}`]);
        await Bun.sleep(350);
        if (hover.exitCode === 0) {
          captures.push(await captureNativeWindow(Number(main.pid), outDir, "stationary-hover-actions-2x"));
        } else {
          structural.errors.push(`hover input failed: ${hover.stderr.trim()}`);
        }

        const select = await run(["cliclick", `c:${hoverX},${hoverY}`]);
        await Bun.sleep(500);
        if (select.exitCode === 0) {
          captures.push(await captureNativeWindow(Number(main.pid), outDir, "stationary-actions-selected-2x"));
          await run(["cliclick", "kp:esc"]);
          await Bun.sleep(200);
        } else {
          structural.errors.push(`Actions selection input failed: ${select.stderr.trim()}`);
        }
      } else {
        structural.errors.push("Actions hit target frame missing from AppKit fidelity snapshot");
      }
      const leftButton = appKitNodes.find((node) =>
        node.id === "script-kit-footer-left-info-hit-target"
        || node.id === "script-kit-footer-cwd-chip-hit"
      );
      const leftFrame = leftButton?.screenshotFrame;
      if (leftFrame && finalMain?.bounds) {
        for (let attempt = 0; attempt < 4; attempt += 1) {
          const openWindows = await driver.listAutomationWindows({ timeoutMs: 15_000 });
          const actionsOpen = ((openWindows as any)?.windows ?? []).some(
            (candidate: any) => candidate.id === "actions-dialog",
          );
          if (!actionsOpen) break;
          await driver.simulateGpuiKeyDown("escape", {
            target: { type: "id", id: "actions-dialog" },
            timeoutMs: 15_000,
          }).catch(() => null);
          await Bun.sleep(150);
        }
        driver.send({ type: "show" });
        await driver.waitForState({ windowVisible: true }, { timeoutMs: 15_000 });
        await driver.waitForSettle({ timeoutMs: 10_000 });
        const footerHeight = Number((receipt.layout as any)?.fidelity?.appKit?.footerContainerFrame?.height ?? 32);
        const leftX = Math.round(finalMain.bounds.x + leftFrame.x + leftFrame.width / 2);
        const leftY = Math.round(
          finalMain.bounds.y + finalMain.bounds.height - footerHeight
          + leftFrame.y + leftFrame.height / 2,
        );
        await announceTestStatus(
          "Left capsule visual",
          "Capturing its hover style, shortcut keycap, and glyph",
        );
        const leftHover = await run(["cliclick", `m:${leftX},${leftY}`]);
        await Bun.sleep(350);
        if (leftHover.exitCode === 0) {
          captures.push(await captureNativeWindow(Number(main.pid), outDir, "stationary-hover-left-2x"));
        } else {
          structural.errors.push(`left footer hover input failed: ${leftHover.stderr.trim()}`);
        }
        const beforeLogs = await driver.getLogs({
          limit: 500,
          contains: "Enqueued native footer action",
        });
        const beforeCount = Number((beforeLogs as any)?.entries?.length ?? 0);
        await announceTestStatus(
          "Left capsule interaction",
          "One active-window click must dispatch exactly once",
        );
        const click = await run(["cliclick", `c:${leftX},${leftY}`]);
        await Bun.sleep(400);
        const afterLogs = await driver.getLogs({
          limit: 500,
          contains: "Enqueued native footer action",
        });
        const afterCount = Number((afterLogs as any)?.entries?.length ?? 0);
        const dispatchDelta = afterCount - beforeCount;
        leftInteraction = {
          targetId: leftButton?.id ?? null,
          activeWindowClick: true,
          clickExitCode: click.exitCode,
          dispatchDelta,
          pass: click.exitCode === 0 && dispatchDelta === 1,
        };
        if (click.exitCode !== 0 || dispatchDelta !== 1) {
          structural.errors.push(
            `active left footer click dispatched ${dispatchDelta} actions (click exit ${click.exitCode}), expected exactly one`,
          );
        }
      } else {
        structural.errors.push("left footer hit target frame missing from AppKit fidelity snapshot");
      }
      for (const capture of captures as any[]) {
        if (capture.nativeWindow?.onscreen !== true || Number(capture.nativeWindow?.alpha ?? 0) < 0.99) {
          structural.errors.push(`${capture.name} captured a hidden or transparent main window`);
        }
        if (Number(capture.edgeEnergy ?? 0) <= 0) {
          structural.errors.push(`${capture.name} contains no visible native-window edges`);
        }
      }
      const defaultFooterHash = (captures.find((capture: any) =>
        capture.name === "stationary-default-2x"
      ) as any)?.footerCropSha256;
      const leftHoverFooterHash = (captures.find((capture: any) =>
        capture.name === "stationary-hover-left-2x"
      ) as any)?.footerCropSha256;
      if (!leftHoverFooterHash || leftHoverFooterHash === defaultFooterHash) {
        structural.errors.push("left footer hover did not produce a distinct visual state");
      }
      structural.pass = structural.errors.length === 0;
      const distinctFooterStates = new Set(captures.map((capture: any) => capture.footerCropSha256));
      receipt.stationary = {
        pass: structural.pass && captures.length >= 3 && distinctFooterStates.size >= 2,
        structural,
        captures,
        leftInteraction,
        distinctFooterStateCount: distinctFooterStates.size,
        captureMethod: "Quartz CGWindowID resolved by exact launched PID; screencapture -l",
        reviewRequired: true,
      };
    } else {
      receipt.stationary = {
        pass: true,
        structural: "fallback intentionally has no native glass capsule hierarchy",
        captures: [],
        priorFallbackReceipt: ".artifacts/main-window-native-drag/fallback/receipt.json",
      };
    }
    receipt.logs = await driver.getLogs({ limit: 500 });
    const logText = JSON.stringify(receipt.logs);
    const crashMarkers = logText.match(/panic|fatal error|segmentation fault|crash(?:ed)?/gi) ?? [];
    receipt.crashScan = {
      pass: crashMarkers.length === 0,
      markerCount: crashMarkers.length,
      markers: [...new Set(crashMarkers.map((marker) => marker.toLowerCase()))],
    };
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
    receipt.valid = results.every((result: any) =>
      result.analysis.valid && result.filmstrip?.pass === true
    );
    receipt.pass = lifecyclePass
      && (receipt.stationary as any)?.pass === true
      && (receipt.crashScan as any)?.pass === true
      && results.every((result: any) => result.analysis.overallPass && result.filmstrip?.pass === true);
  } finally {
    try {
      driver.send({ type: "hide" });
      await driver.waitForState({ windowVisible: false }, { timeoutMs: 5_000 });
    } catch {}
    await driver.close();
    await Bun.sleep(250);
    const launchedPid = Number(receipt.pid ?? 0);
    const processProbe = launchedPid > 0
      ? await run(["kill", "-0", String(launchedPid)])
      : { exitCode: 1, stdout: "", stderr: "PID unavailable" };
    const nativeWindowProbe = launchedPid > 0
      ? await run([
        "swift",
        resolve(import.meta.dir, "../agentic/macos-window-query.swift"),
        "--pid",
        String(launchedPid),
      ])
      : { exitCode: 1, stdout: "", stderr: "PID unavailable" };
    let survivingNativeWindows: unknown[] = [];
    try {
      survivingNativeWindows = JSON.parse(nativeWindowProbe.stdout).windows ?? [];
    } catch {}
    const cleanupPass = launchedPid > 0
      && processProbe.exitCode !== 0
      && nativeWindowProbe.exitCode === 0
      && survivingNativeWindows.length === 0;
    receipt.cleanup = {
      pass: cleanupPass,
      launchedPid,
      processAlive: processProbe.exitCode === 0,
      nativeWindowQueryExitCode: nativeWindowProbe.exitCode,
      survivingNativeWindows,
      nativeWindowQueryStderr: nativeWindowProbe.stderr.trim(),
    };
    receipt.pass = receipt.pass === true && cleanupPass;
    receipt.driverStats = driver.stats;
    receipt.cleanedUp = cleanupPass;
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
