#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { Driver } from "./driver.ts";
import { announceTestStatus } from "./test-status.ts";

export type Rect = { x: number; y: number; width: number; height: number };
export type TimedRead<T> = {
  startNs: number;
  endNs: number;
  midpointNs: number;
  value: T | null;
  error: string | null;
};
export type ControlFrame = {
  id: string;
  framePt: Rect | null;
  mainFramePtAtMeasurement?: Rect | null;
  axWindowNumber: number | null;
  measurementSource?: string;
  error?: string | null;
  frameRead?: TimedRead<Rect>;
  ownerRead?: TimedRead<number>;
  alignmentUncertaintyPx?: number;
  topologyFresh?: boolean;
  displayIntervalIndex?: number;
  crossesEventBoundary?: boolean;
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
  packetStartNs?: number;
  packetEndNs?: number;
  displayTickNs?: number;
  displayIntervalIndex?: number;
  topologyStartNs?: number;
  topologyEndNs?: number;
  topologyFresh?: boolean;
  topologyComplete?: boolean;
};
export type FilmstripFrame = {
  fraction: number;
  tNs: number;
  actualFrameNs?: number;
  markerEventNs?: number;
  encodingCompletedNs?: number;
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
  mouseDownEventNs?: number | null;
  mouseUpEventNs?: number | null;
  events?: Array<{
    kind: "mouseDown" | "mouseDragged" | "mouseUp" | string;
    sequence: number;
    tag: number;
    intendedNs: number;
    actualEventNs: number;
    postStartNs: number;
    postEndNs: number;
    observedByEventTap: boolean;
  }>;
  interference?: {
    untaggedInputCount: number;
    frontmostAppChanged: boolean;
    pointerDeviationPx: number;
    targetMovedExternally: boolean;
  };
  observerHealth?: {
    scheduledPackets: number;
    completedPackets: number;
    missedPackets: number;
    axTimeoutCount: number;
    topologyStaleCount: number;
    displayTickIntervalsMs: number[];
  };
  samples: DragSample[];
  filmstripFrames?: FilmstripFrame[];
  errors: string[];
};

export type ControlMetrics = {
  id: string;
  sampleCount: number;
  maxDriftPx: number | null;
  p99DriftPx: number | null;
  rmsDriftPx: number | null;
  consecutiveOverHalfPixel: number | null;
  settlingMs: number | null;
  stableAfterSettling: boolean;
  owningWindowNumbers: number[];
  thresholdsPass: boolean;
};

export type AttemptDisposition =
  | "EVALUABLE_PASS"
  | "EVALUABLE_FAIL"
  | "INVALID_OBSERVER"
  | "INVALID_INTERFERENCE"
  | "INVALID_SETUP"
  | "BLOCKED_ENVIRONMENT";
export type MotionVerdict = "PASS" | "FAIL" | "NOT_EVALUATED";
export type TopologyVerdict = "PASS" | "FAIL" | "UNKNOWN";

export type DragAnalysis = {
  trajectory: string;
  valid: boolean;
  verdict: "PASS" | "FAIL" | "INVALID";
  attemptDisposition: AttemptDisposition;
  motionVerdict: MotionVerdict;
  topologyVerdict: TopologyVerdict;
  evidenceValidity: "VALID" | "INVALID";
  observerHealth: "PASS" | "FAIL";
  interferenceClassification: "NONE" | "USER_OR_ENVIRONMENT";
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
  diagnosticOnly: {
    apparentMaxDriftPx: number | null;
  };
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
const LIVE_MEASUREMENT_SOURCES = new Set([
  "live-ax+interpolated-main",
  "live-ax+bracketed-main-v2",
]);
const MAX_NATIVE_DRAG_ATTEMPTS = 10;
// AX + WindowServer sampling is driven by an NSEventTracking/common-mode timer.
// Allow one millisecond of scheduler quantization around the nominal refresh
// boundary while keeping the separate hard two-refresh maximum-gap guard.
const DISPLAY_GAP_TOLERANCE_MS = 1;

function quantile(values: number[], fraction: number): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(sorted.length * fraction) - 1),
  );
  return sorted[index];
}

function round(value: number, digits = 4): number {
  return Number(value.toFixed(digits));
}

function distance(
  a: { x: number; y: number },
  b: { x: number; y: number },
): number {
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

function controlMidpointNs(sample: DragSample, control: ControlFrame): number {
  return control.frameRead?.midpointNs ?? sample.tNs;
}

function median(values: number[]): number | null {
  return quantile(values, 0.5);
}

function componentMedian(vectors: Array<{ x: number; y: number }>) {
  const x = median(vectors.map((entry) => entry.x));
  const y = median(vectors.map((entry) => entry.y));
  return x == null || y == null ? null : { x, y };
}

function driftEntries(
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
    return [
      {
        sample,
        control,
        midpointNs: controlMidpointNs(sample, control),
        driftPx: distance(relative, baseline) * scale,
      },
    ];
  });
}

function stableBaseline(
  samples: DragSample[],
  controlID: string,
  scale: number,
): { vector: { x: number; y: number } | null; error: string | null } {
  const entries = samples.flatMap((sample) => {
    const control = sample.controls.find((entry) => entry.id === controlID);
    const relative = control ? relativeVector(sample, control) : null;
    return relative
      ? [{ relative, tNs: controlMidpointNs(sample, control!) }]
      : [];
  });
  if (entries.length < 12)
    return {
      vector: null,
      error: `control ${controlID} has only ${entries.length}/12 baseline observations`,
    };
  const spanNs = entries.at(-1)!.tNs - entries[0].tNs;
  if (spanNs < 100_000_000)
    return {
      vector: null,
      error: `control ${controlID} baseline spans only ${round(spanNs / 1_000_000)}ms`,
    };
  const vector = componentMedian(entries.map((entry) => entry.relative));
  if (!vector)
    return {
      vector: null,
      error: `control ${controlID} baseline median is unavailable`,
    };
  const spreadPx = Math.max(
    ...entries.map((entry) => distance(entry.relative, vector) * scale),
  );
  if (spreadPx > 0.25)
    return {
      vector: null,
      error: `control ${controlID} baseline spread ${round(spreadPx)}px exceeds 0.25px`,
    };
  return { vector, error: null };
}

function settlingForControlV2(
  samples: DragSample[],
  controlID: string,
  baseline: { x: number; y: number },
  scale: number,
  mouseUpEventNs: number,
  displayPeriodMs: number,
) {
  const entries = driftEntries(samples, controlID, baseline, scale)
    .filter(
      (entry) =>
        entry.midpointNs > mouseUpEventNs &&
        !entry.control.crossesEventBoundary,
    )
    .sort((a, b) => a.midpointNs - b.midpointNs);
  for (let index = 0; index < entries.length; index += 1) {
    const candidate = entries[index];
    if (candidate.driftPx > 0.5) continue;
    const tail = entries.filter(
      (entry) =>
        entry.midpointNs >= candidate.midpointNs &&
        entry.midpointNs <=
          candidate.midpointNs + 100_000_000 + displayPeriodMs * 1_000_000,
    );
    const spans100ms =
      (tail.at(-1)?.midpointNs ?? 0) - candidate.midpointNs >= 100_000_000;
    const gaps = tail
      .slice(1)
      .map(
        (entry, tailIndex) =>
          (entry.midpointNs - tail[tailIndex].midpointNs) / 1_000_000,
      );
    const coverage =
      tail.length >= 7 &&
      gaps.every((gap) => gap <= displayPeriodMs + DISPLAY_GAP_TOLERANCE_MS);
    if (spans100ms && coverage && tail.every((entry) => entry.driftPx <= 0.5)) {
      return {
        settlingMs: (candidate.midpointNs - mouseUpEventNs) / 1_000_000,
        stable: true,
      };
    }
  }
  return { settlingMs: null, stable: false };
}

function interferenceDetected(trace: NativeTrace) {
  const interference = trace.interference;
  return Boolean(
    interference &&
    (interference.untaggedInputCount > 0 ||
      interference.frontmostAppChanged ||
      interference.pointerDeviationPx > 1 ||
      interference.targetMovedExternally),
  );
}

export function analyzeTrace(trace: NativeTrace): DragAnalysis {
  const evidenceErrors = [...(trace.errors ?? [])];
  const samples = trace.samples ?? [];
  const inMotion = samples.filter((sample) => sample.phase === "dragged");
  const pre = samples.filter(
    (sample) => sample.phase === "pre" || sample.phase === "mouseDown",
  );
  const settlingSamples = samples.filter(
    (sample) => sample.phase === "settling",
  );
  const scale = trace.display?.backingScale ?? 1;
  const refreshPeriodMs = 1000 / Math.max(1, trace.display?.refreshHz ?? 60);
  const controlIDs = [
    ...new Set(
      samples.flatMap((sample) => sample.controls.map((control) => control.id)),
    ),
  ];
  const rightControlID =
    RIGHT_CONTROL_IDS.find((id) => controlIDs.includes(id)) ?? null;
  const requiredIDs = rightControlID
    ? [LEFT_CONTROL_ID, rightControlID]
    : [LEFT_CONTROL_ID];
  const interference = interferenceDetected(trace);

  const mainPositions = inMotion.flatMap((sample) =>
    sample.mainFramePt ? [sample.mainFramePt] : [],
  );
  const distinctMainPositions = new Set(
    mainPositions.map((frame) => `${round(frame.x, 2)},${round(frame.y, 2)}`),
  ).size;
  const displacementPt =
    mainPositions.length >= 2
      ? distance(mainPositions[0], mainPositions.at(-1)!)
      : 0;
  const packetIntervalsMs = inMotion
    .slice(1)
    .map((sample, index) => (sample.tNs - inMotion[index].tNs) / 1_000_000)
    .filter((value) => Number.isFinite(value) && value >= 0);
  const cadence = {
    medianMs: quantile(packetIntervalsMs, 0.5),
    p95Ms: quantile(packetIntervalsMs, 0.95),
    maxMs: packetIntervalsMs.length ? Math.max(...packetIntervalsMs) : null,
    refreshPeriodMs,
  };

  if (trace.schemaVersion < 2)
    evidenceErrors.push(
      `schema ${trace.schemaVersion} lacks contemporaneous evidence`,
    );
  if (trace.status !== "ok")
    evidenceErrors.push(`sampler status is ${trace.status}`);
  if (!trace.accessibilityTrusted)
    evidenceErrors.push("accessibility is not trusted");
  if (!trace.display) evidenceErrors.push("display timeline is missing");
  if (!controlIDs.includes(LEFT_CONTROL_ID))
    evidenceErrors.push(`missing exact left control ${LEFT_CONTROL_ID}`);
  if (!rightControlID)
    evidenceErrors.push(
      `missing exact right control (${RIGHT_CONTROL_IDS.join(",")})`,
    );
  if (controlIDs.length !== 2)
    evidenceErrors.push(
      `expected exactly two controls, sampled ${controlIDs.length}`,
    );
  if (trace.mouseDownEventNs == null || trace.mouseUpEventNs == null) {
    evidenceErrors.push(
      "explicit mouse-down/mouse-up event timestamps are missing",
    );
  }
  const down = trace.events?.find((event) => event.kind === "mouseDown");
  const up = trace.events?.find((event) => event.kind === "mouseUp");
  if (
    !down?.observedByEventTap ||
    !up?.observedByEventTap ||
    down.tag === 0 ||
    up.tag === 0
  ) {
    evidenceErrors.push(
      "tagged mouse-down/mouse-up were not confirmed by the event tap",
    );
  }
  if (inMotion.length < 36)
    evidenceErrors.push(`only ${inMotion.length} in-motion packets`);
  if (distinctMainPositions < 30)
    evidenceErrors.push(
      `only ${distinctMainPositions} distinct main positions`,
    );
  if (displacementPt < 200)
    evidenceErrors.push(
      `main displacement ${round(displacementPt)}pt is below 200pt`,
    );

  const observerHealth = trace.observerHealth;
  if (!observerHealth)
    evidenceErrors.push("observer health telemetry is missing");
  else {
    if (observerHealth.axTimeoutCount > 0)
      evidenceErrors.push(
        `${observerHealth.axTimeoutCount} AX reads timed out`,
      );
    if (observerHealth.topologyStaleCount > 0)
      evidenceErrors.push(
        `${observerHealth.topologyStaleCount} topology snapshots were stale`,
      );
    if (observerHealth.displayTickIntervalsMs.length < 2)
      evidenceErrors.push("display tick timeline is incomplete");
  }

  const enumerationMissing = samples.some(
    (sample) =>
      sample.relevantWindowNumbers == null ||
      sample.relevantWindowNumbers.length !== sample.relevantWindowCount ||
      sample.topologyFresh !== true ||
      sample.topologyComplete !== true ||
      sample.topologyStartNs == null ||
      sample.topologyEndNs == null,
  );
  if (enumerationMissing)
    evidenceErrors.push(
      "fresh complete per-packet native-window topology is missing",
    );

  const baselineByID = new Map<string, { x: number; y: number }>();
  for (const id of requiredIDs) {
    const baseline = stableBaseline(pre, id, scale);
    if (baseline.error) evidenceErrors.push(baseline.error);
    if (baseline.vector) baselineByID.set(id, baseline.vector);
  }
  const baselineLeft = baselineByID.get(LEFT_CONTROL_ID);
  const baselineRight = rightControlID
    ? baselineByID.get(rightControlID)
    : null;
  if (baselineLeft && baselineRight) {
    const separation = baselineRight.x - baselineLeft.x;
    if (separation < 100)
      evidenceErrors.push(
        `left/right controls are not far apart (${round(separation)}pt)`,
      );
  }

  for (const sample of [...pre, ...inMotion, ...settlingSamples]) {
    for (const id of requiredIDs) {
      const control = sample.controls.find((entry) => entry.id === id);
      if (!control) {
        evidenceErrors.push(
          `control ${id} is missing from packet ${sample.tNs}`,
        );
        continue;
      }
      if (!LIVE_MEASUREMENT_SOURCES.has(control.measurementSource ?? "")) {
        evidenceErrors.push(
          `control ${id} is not a live bracketed AX measurement`,
        );
      }
      if (!control.frameRead || !control.ownerRead)
        evidenceErrors.push(`control ${id} read timing is missing`);
      if (control.frameRead?.error || control.ownerRead?.error || control.error)
        evidenceErrors.push(`control ${id} AX read failed`);
      if (control.framePt == null || control.mainFramePtAtMeasurement == null)
        evidenceErrors.push(`control ${id} geometry is missing`);
      if (control.axWindowNumber == null || control.ownerRead?.value == null)
        evidenceErrors.push(`control ${id} ownership is missing`);
      if (
        control.alignmentUncertaintyPx == null ||
        control.alignmentUncertaintyPx > 0.25
      ) {
        evidenceErrors.push(
          `control ${id} alignment uncertainty exceeds 0.25px or is missing`,
        );
      }
      if (control.topologyFresh !== true)
        evidenceErrors.push(`control ${id} topology is stale`);
      if (control.crossesEventBoundary)
        evidenceErrors.push(`control ${id} read crosses an event boundary`);
    }
  }

  const motionIntervals = new Set(
    inMotion
      .map((sample) => sample.displayIntervalIndex)
      .filter((value): value is number => value != null),
  );
  for (const interval of motionIntervals) {
    const packets = inMotion.filter(
      (sample) => sample.displayIntervalIndex === interval,
    );
    for (const id of requiredIDs) {
      if (
        !packets.some((sample) =>
          sample.controls.some((control) => control.id === id),
        )
      ) {
        evidenceErrors.push(
          `display interval ${interval} has no ${id} observation`,
        );
      }
    }
  }
  for (const id of requiredIDs) {
    const midpoints = inMotion
      .flatMap((sample) => {
        const control = sample.controls.find((entry) => entry.id === id);
        return control ? [controlMidpointNs(sample, control)] : [];
      })
      .sort((a, b) => a - b);
    if (midpoints.length < 36)
      evidenceErrors.push(
        `control ${id} has only ${midpoints.length}/36 valid motion observations`,
      );
    const maxGapMs =
      midpoints.length > 1
        ? Math.max(
            ...midpoints
              .slice(1)
              .map(
                (midpoint, index) => (midpoint - midpoints[index]) / 1_000_000,
              ),
          )
        : Number.POSITIVE_INFINITY;
    if (maxGapMs > refreshPeriodMs + DISPLAY_GAP_TOLERANCE_MS) {
      evidenceErrors.push(
        `control ${id} observation gap ${round(maxGapMs)}ms exceeds one display interval plus 1ms`,
      );
    }
  }

  const topologyCounts = new Set(
    samples.map((sample) => sample.relevantWindowCount),
  );
  const topologyNumbers = new Set(
    samples.flatMap((sample) => sample.relevantWindowNumbers ?? []),
  );
  const ownerMismatch = samples.some((sample) =>
    sample.controls.some(
      (control) =>
        control.axWindowNumber != null &&
        sample.mainWindowNumber != null &&
        control.axWindowNumber !== sample.mainWindowNumber,
    ),
  );
  const topologyFail =
    !enumerationMissing &&
    (topologyCounts.size !== 1 ||
      [...topologyCounts][0] !== 1 ||
      topologyNumbers.size !== 1 ||
      samples.some((sample) => sample.footerWindowNumber != null) ||
      ownerMismatch);
  const topologyVerdict: TopologyVerdict = enumerationMissing
    ? "UNKNOWN"
    : topologyFail
      ? "FAIL"
      : "PASS";
  const oneWindowInvariant = topologyVerdict === "PASS";
  const topology =
    topologyVerdict === "PASS"
      ? "one-window"
      : topologyVerdict === "FAIL"
        ? "two-window"
        : "unknown";

  const apparentValues: number[] = [];
  for (const id of requiredIDs) {
    const baseline = baselineByID.get(id);
    if (baseline)
      apparentValues.push(
        ...driftEntries(inMotion, id, baseline, scale).map(
          (entry) => entry.driftPx,
        ),
      );
  }

  const uniqueErrors = [...new Set(evidenceErrors)];
  const evidenceValid = uniqueErrors.length === 0 && !interference;
  const controls: ControlMetrics[] = requiredIDs.map((id) => {
    const baseline = baselineByID.get(id);
    const rawEntries = baseline
      ? driftEntries(inMotion, id, baseline, scale)
      : [];
    const owners = [
      ...new Set(
        rawEntries.flatMap((entry) =>
          entry.control.axWindowNumber == null
            ? []
            : [entry.control.axWindowNumber],
        ),
      ),
    ];
    if (!evidenceValid || !baseline) {
      return {
        id,
        sampleCount: rawEntries.length,
        maxDriftPx: null,
        p99DriftPx: null,
        rmsDriftPx: null,
        consecutiveOverHalfPixel: null,
        settlingMs: null,
        stableAfterSettling: false,
        owningWindowNumbers: owners,
        thresholdsPass: false,
      };
    }
    const values = rawEntries.map((entry) => entry.driftPx);
    let consecutiveOverHalfPixel = 0;
    for (let index = 1; index < values.length; index += 1) {
      if (values[index - 1] > 0.5 && values[index] > 0.5)
        consecutiveOverHalfPixel += 1;
    }
    const settling = settlingForControlV2(
      settlingSamples,
      id,
      baseline,
      scale,
      trace.mouseUpEventNs!,
      refreshPeriodMs,
    );
    const maxDriftPx = Math.max(...values);
    const p99DriftPx = quantile(values, 0.99)!;
    const rmsDriftPx = Math.sqrt(
      values.reduce((sum, value) => sum + value * value, 0) / values.length,
    );
    const thresholdsPass =
      maxDriftPx <= THRESHOLDS.maxDriftPx &&
      p99DriftPx <= THRESHOLDS.p99DriftPx &&
      rmsDriftPx <= THRESHOLDS.rmsDriftPx &&
      consecutiveOverHalfPixel === 0 &&
      settling.stable &&
      settling.settlingMs != null &&
      settling.settlingMs <= refreshPeriodMs + 4;
    return {
      id,
      sampleCount: values.length,
      maxDriftPx: round(maxDriftPx),
      p99DriftPx: round(p99DriftPx),
      rmsDriftPx: round(rmsDriftPx),
      consecutiveOverHalfPixel,
      settlingMs:
        settling.settlingMs == null ? null : round(settling.settlingMs),
      stableAfterSettling: settling.stable,
      owningWindowNumbers: owners,
      thresholdsPass,
    };
  });

  const motionThresholdsPass =
    evidenceValid &&
    controls.length === 2 &&
    controls.every((control) => control.thresholdsPass);
  const productFail =
    evidenceValid && (topologyVerdict === "FAIL" || !motionThresholdsPass);
  const motionVerdict: MotionVerdict = !evidenceValid
    ? "NOT_EVALUATED"
    : productFail
      ? "FAIL"
      : "PASS";
  const attemptDisposition: AttemptDisposition = interference
    ? "INVALID_INTERFERENCE"
    : !evidenceValid
      ? "INVALID_OBSERVER"
      : productFail
        ? "EVALUABLE_FAIL"
        : "EVALUABLE_PASS";
  const overallPass =
    attemptDisposition === "EVALUABLE_PASS" && topologyVerdict === "PASS";
  return {
    trajectory: trace.trajectory,
    valid: evidenceValid,
    verdict: !evidenceValid ? "INVALID" : overallPass ? "PASS" : "FAIL",
    attemptDisposition,
    motionVerdict,
    topologyVerdict,
    evidenceValidity: evidenceValid ? "VALID" : "INVALID",
    observerHealth: uniqueErrors.length === 0 ? "PASS" : "FAIL",
    interferenceClassification: interference ? "USER_OR_ENVIRONMENT" : "NONE",
    errors: interference
      ? [...uniqueErrors, "positive user/environment interference observed"]
      : uniqueErrors,
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
    diagnosticOnly: {
      apparentMaxDriftPx: apparentValues.length
        ? round(Math.max(...apparentValues))
        : null,
    },
  };
}

export function selectTerminalAttempt<
  T extends { analysis: DragAnalysis; filmstrip?: { pass: boolean } },
>(attempts: T[]): T | null {
  for (const attempt of attempts) {
    if (attempt.analysis.attemptDisposition === "EVALUABLE_FAIL")
      return attempt;
    if (
      attempt.analysis.attemptDisposition === "EVALUABLE_PASS" &&
      attempt.filmstrip?.pass !== false
    )
      return attempt;
  }
  return null;
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
  const frames = [...(trace.filmstripFrames ?? [])].sort(
    (a, b) => a.fraction - b.fraction,
  );
  if (frames.length !== 3)
    errors.push(`expected 3 same-run filmstrip frames, found ${frames.length}`);
  const expectedFractions = [0.25, 0.5, 0.75];
  frames.forEach((frame, index) => {
    if (
      Math.abs(frame.fraction - (expectedFractions[index] ?? frame.fraction)) >
      0.001
    ) {
      errors.push(`unexpected filmstrip fraction ${frame.fraction}`);
    }
    if (!frame.captureSucceeded || !existsSync(frame.path)) {
      errors.push(
        `filmstrip capture ${index + 1} failed: ${frame.error ?? "file missing"}`,
      );
    }
    if (frame.actualFrameNs == null || frame.markerEventNs == null) {
      errors.push(
        `filmstrip frame ${index + 1} is not tied to ScreenCaptureKit and event host time`,
      );
    }
  });
  const downNs = trace.mouseDownEventNs ?? null;
  const upNs = trace.mouseUpEventNs ?? null;
  const refreshPeriodNs =
    1_000_000_000 / Math.max(1, trace.display?.refreshHz ?? 60);
  if (
    downNs == null ||
    upNs == null ||
    frames.some(
      (frame) =>
        frame.actualFrameNs == null ||
        frame.actualFrameNs <= downNs ||
        frame.actualFrameNs >= upNs,
    )
  ) {
    errors.push(
      "filmstrip actual frame times are outside the tagged drag interval",
    );
  }
  if (
    frames.some(
      (frame) =>
        frame.actualFrameNs == null ||
        frame.markerEventNs == null ||
        Math.abs(frame.actualFrameNs - frame.markerEventNs) > refreshPeriodNs,
    )
  ) {
    errors.push("filmstrip frame/event skew exceeds one display interval");
  }
  const positions = frames.flatMap((frame) =>
    frame.mainFramePt ? [frame.mainFramePt] : [],
  );
  const distinctPositions = new Set(
    positions.map((frame) => `${round(frame.x, 2)},${round(frame.y, 2)}`),
  );
  const displacementPt =
    positions.length >= 2 ? distance(positions[0], positions.at(-1)!) : 0;
  if (distinctPositions.size !== 3)
    errors.push(
      "filmstrip does not contain three distinct main-window positions",
    );
  if (displacementPt < 100)
    errors.push(
      `filmstrip main-window displacement ${round(displacementPt)}pt is below 100pt`,
    );
  const enrichedFrames = frames.map((frame) => ({
    ...frame,
    exists: existsSync(frame.path),
    sha256: existsSync(frame.path) ? sha256(frame.path) : null,
  }));
  const hashes = new Set(
    enrichedFrames.flatMap((frame) => (frame.sha256 ? [frame.sha256] : [])),
  );
  if (hashes.size !== 3)
    errors.push("filmstrip captures are not three distinct images");
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
  layer?: {
    contentsScale?: number;
    borderWidth?: number;
    cornerRadius?: number;
    shadowOpacity?: number;
    shadowRadius?: number;
    shadowOffsetX?: number;
    shadowOffsetY?: number;
    hasShadowPath?: boolean;
  };
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
  const components = (layout?.components ?? []) as Array<Record<string, any>>;
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
  const mainStage = components.find((component) => component.name === "main-content-stage");
  const dialogBoundary = components.find((component) =>
    component.name === "main-window-dialog-layer-boundary"
  );
  const expectedMainStage = {
    x: 0,
    y: 0,
    width: Number(hostBounds?.width ?? 0),
    height: Number(mainBackdropFrame?.height ?? 0),
  };
  const isContainedByMainStage = (measured: any) =>
    measured != null
    && measured.x >= expectedMainStage.x
    && measured.y >= expectedMainStage.y
    && measured.x + measured.width <= expectedMainStage.x + expectedMainStage.width
    && measured.y + measured.height <= expectedMainStage.y + expectedMainStage.height;
  for (const [name, component, requiredFields] of [
    ["main-content-stage", mainStage, ["bounds", "visibleBounds"]],
    [
      "main-window-dialog-layer-boundary",
      dialogBoundary,
      ["bounds", "visibleBounds", "clipBounds"],
    ],
  ] as const) {
    if (!component) {
      errors.push(`${name} paint-time bounds are missing`);
      continue;
    }
    for (const field of requiredFields) {
      const measured = component[field];
      if (!isContainedByMainStage(measured)) {
        errors.push(
          `${name} ${field} is not bounded to the main-content stage: ${JSON.stringify(measured)}`,
        );
      }
    }
    if (
      name === "main-content-stage"
      && (
        component.bounds?.x !== expectedMainStage.x
        || component.bounds?.y !== expectedMainStage.y
        || component.bounds?.width !== expectedMainStage.width
        || component.bounds?.height !== expectedMainStage.height
      )
    ) {
      errors.push(`${name} does not fill the bounded main-content stage`);
    }
    if (
      component.measurementProvenance !== "paint-time"
      || component.coordinateSpace !== "window"
    ) {
      errors.push(`${name} lacks paint-time window-coordinate provenance`);
    }
  }
  const backdropLayer = appKit?.mainBackdropLayer ?? null;
  if (!backdropLayer) {
    errors.push("main backdrop layer telemetry is missing");
  } else if (
    Number(backdropLayer.shadowOpacity ?? 1) !== 0
    || Number(backdropLayer.shadowRadius ?? 1) !== 0
    || backdropLayer.hasShadowPath === true
  ) {
    errors.push("main backdrop still carries a shadow into the detached gutter");
  }

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
  const leftIcon = byId.get("script-kit-footer-left-profile-icon")
    ?? byId.get("script-kit-footer-cwd-chip-icon");
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
  if (
    !leftIcon
    || Number(leftIcon.image?.width ?? 0) <= 0
    || Number(leftIcon.image?.height ?? 0) <= 0
  ) {
    errors.push("left footer icon has no rendered image");
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
    mainStage: mainStage ?? null,
    dialogBoundary: dialogBoundary ?? null,
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
  const contentLuminanceResult = await run([
    "magick",
    path,
    "-gravity",
    "north",
    "-crop",
    "x880+0+0",
    "+repage",
    "-alpha",
    "off",
    "-colorspace",
    "Gray",
    "-format",
    "%[fx:mean]",
    "info:",
  ]);
  const contentMeanLuminance = Number(contentLuminanceResult.stdout.trim());
  const contentDetailEdgeResult = await run([
    "magick",
    path,
    "-crop",
    "1500x700+0+180",
    "+repage",
    "-alpha",
    "off",
    "-colorspace",
    "Gray",
    "-morphology",
    "Edge",
    "Diamond",
    "-format",
    "%[fx:mean]",
    "info:",
  ]);
  const contentDetailEdgeEnergy = Number(contentDetailEdgeResult.stdout.trim());
  return {
    name,
    nativeWindow,
    path,
    sha256: sha256(path),
    footerCropPath,
    footerCropSha256: sha256(footerCropPath),
    edgeEnergy: Number.isFinite(edgeEnergy) ? round(edgeEnergy, 6) : null,
    contentMeanLuminance: Number.isFinite(contentMeanLuminance)
      ? round(contentMeanLuminance, 6)
      : null,
    contentDetailEdgeEnergy: Number.isFinite(contentDetailEdgeEnergy)
      ? round(contentDetailEdgeEnergy, 6)
      : null,
  };
}

export type GutterAlphaMetrics = {
  pixelWidth: number;
  pixelHeight: number;
  gapY: number;
  gapHeight: number;
  fullAlphaMin: number;
  fullAlphaMax: number;
  fullAlphaMean: number;
  centerAlphaMin: number;
  centerAlphaMax: number;
  centerAlphaMean: number;
};

export function evaluateGutterTransparency(metrics: GutterAlphaMetrics) {
  const errors: string[] = [];
  if (metrics.gapHeight < 4) errors.push(`gutter is only ${metrics.gapHeight} physical pixels high`);
  const alphaTolerance = 1 / 255;
  if (metrics.fullAlphaMax > alphaTolerance) {
    errors.push(`full gutter alpha max ${metrics.fullAlphaMax.toFixed(6)} exceeds ${alphaTolerance.toFixed(6)}`);
  }
  if (metrics.centerAlphaMax > alphaTolerance) {
    errors.push(`central gutter alpha max ${metrics.centerAlphaMax.toFixed(6)} exceeds ${alphaTolerance.toFixed(6)}`);
  }
  return { pass: errors.length === 0, errors, ...metrics };
}

async function analyzeNativeWindowGutterAlpha(
  path: string,
  appKit: any,
) {
  const identify = await run(["magick", path, "-format", "%w,%h", "info:"]);
  if (identify.exitCode !== 0) throw new Error(`gutter image identify failed: ${identify.stderr}`);
  const [pixelWidth, pixelHeight] = identify.stdout.trim().split(",").map(Number);
  const windowBounds = appKit?.windowBounds;
  const backdrop = appKit?.mainBackdropFrame;
  const gapPoints = Number(appKit?.transparentGapPoints ?? 0);
  if (!windowBounds || !backdrop || !pixelWidth || !pixelHeight || gapPoints <= 0) {
    throw new Error("gutter image analysis is missing window/backdrop geometry");
  }
  const scaleY = pixelHeight / Number(windowBounds.height);
  const gapY = Math.round((Number(windowBounds.height) - Number(backdrop.y)) * scaleY);
  const gapHeight = Math.round(gapPoints * scaleY);
  const centerX = Math.round(pixelWidth * 0.25);
  const centerWidth = Math.max(1, Math.round(pixelWidth * 0.5));
  const alphaFormat = "%[fx:minima.a],%[fx:maxima.a],%[fx:mean.a]";
  const measure = async (crop: string) => {
    const result = await run(["magick", path, "-crop", crop, "+repage", "-format", alphaFormat, "info:"]);
    if (result.exitCode !== 0) throw new Error(`gutter alpha measurement failed: ${result.stderr}`);
    return result.stdout.trim().split(",").map(Number);
  };
  const [fullAlphaMin, fullAlphaMax, fullAlphaMean] = await measure(
    `${pixelWidth}x${gapHeight}+0+${gapY}`,
  );
  const [centerAlphaMin, centerAlphaMax, centerAlphaMean] = await measure(
    `${centerWidth}x${gapHeight}+${centerX}+${gapY}`,
  );
  return evaluateGutterTransparency({
    pixelWidth,
    pixelHeight,
    gapY,
    gapHeight,
    fullAlphaMin,
    fullAlphaMax,
    fullAlphaMean,
    centerAlphaMin,
    centerAlphaMax,
    centerAlphaMean,
  });
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
  const visualMatrix = args.includes("--visual-matrix");
  const stationaryOnly = args.includes("--stationary-only") || visualMatrix;
  const baseline = value("--baseline");
  return {
    binary,
    outDir,
    trials: stationaryOnly ? [] : trials,
    expectFallback,
    stationaryOnly,
    visualMatrix,
    baseline,
  };
}

async function cli() {
  const {
    binary,
    outDir,
    trials,
    expectFallback,
    stationaryOnly,
    visualMatrix,
    baseline,
  } = parseCLI();
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

    if (stationaryOnly) {
      await announceTestStatus(
        "Canonical main launcher",
        "Resetting persisted surface state before visual measurements",
      );
      driver.send({
        type: "triggerBuiltin",
        builtinId: "builtin/main-window",
        requestId: "mwnd-stationary-main-window",
      });
      await driver.waitForSettle({ timeoutMs: 10_000 });
      await ensureMainWindowVisible("mwnd-stationary-main-window-show");
      await driver.setFilterAndWait("", { timeoutMs: 15_000 });
      await driver.waitForSettle({ timeoutMs: 10_000 });
    }

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
        stationaryOnly ? { expectedHostSize: { width: 750, height: 480 } } : {},
      );
      const captures: Array<Record<string, unknown>> = [];
      const defaultCapture = await captureNativeWindow(Number(main.pid), outDir, "stationary-default-2x");
      (defaultCapture as any).gutterTransparency = await analyzeNativeWindowGutterAlpha(
        (defaultCapture as any).path,
        (receipt.layout as any)?.fidelity?.appKit,
      );
      if (!(defaultCapture as any).gutterTransparency.pass) {
        structural.errors.push(
          ...((defaultCapture as any).gutterTransparency.errors as string[]).map(
            (error) => `transparent gutter: ${error}`,
          ),
        );
      }
      captures.push(defaultCapture);

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
      const matrixStates: Array<Record<string, any>> = [];
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
      if (visualMatrix) {
        const captureMatrixState = async (
          name: string,
          expectedMode: "mini" | "full",
          expectedAppearance: "system" | "light" | "dark",
        ) => {
          await driver.waitForSettle({ timeoutMs: 10_000 });
          const [state, layout] = await Promise.all([
            driver.getState({ timeoutMs: 15_000 }),
            driver.getLayoutInfo(
              { target: { type: "id", id: "main" } },
              { timeoutMs: 15_000 },
            ),
          ]);
          const capture: any = await captureNativeWindow(Number(main.pid), outDir, name);
          capture.gutterTransparency = await analyzeNativeWindowGutterAlpha(
            capture.path,
            (layout as any)?.fidelity?.appKit,
          );
          captures.push(capture);
          const actualMode = (state as any)?.miniAi?.mainWindowMode ?? null;
          const contentMeanLuminance = Number(capture.contentMeanLuminance);
          const contentDetailEdgeEnergy = Number(capture.contentDetailEdgeEnergy);
          const appearanceCheck = expectedAppearance === "system"
            ? {
              pass: true,
              rule: "system appearance is not luminance-constrained",
              contentMeanLuminance,
              contentDetailEdgeEnergy,
            }
            : expectedAppearance === "light"
            ? {
              pass: contentMeanLuminance >= 0.45 && contentDetailEdgeEnergy >= 0.012,
              rule:
                "light fixture luminance must be >= 0.45 and content-detail edge energy >= 0.012",
              contentMeanLuminance,
              contentDetailEdgeEnergy,
            }
            : {
              pass: contentMeanLuminance <= 0.35 && contentDetailEdgeEnergy >= 0.012,
              rule:
                "dark fixture luminance must be <= 0.35 and content-detail edge energy >= 0.012",
              contentMeanLuminance,
              contentDetailEdgeEnergy,
            };
          const entry = {
            name,
            expectedMode,
            actualMode,
            expectedAppearance,
            promptType: (state as any)?.promptType ?? null,
            activeFooter: (state as any)?.activeFooter ?? null,
            mainStage: (layout as any)?.components?.find(
              (component: any) => component.name === "main-content-stage",
            ) ?? null,
            dialogBoundary: (layout as any)?.components?.find(
              (component: any) => component.name === "main-window-dialog-layer-boundary",
            ) ?? null,
            capture,
            appearanceCheck,
            pass: actualMode === expectedMode
              && capture.gutterTransparency.pass
              && appearanceCheck.pass,
          };
          matrixStates.push(entry);
          return entry;
        };

        await announceTestStatus(
          "Visual matrix · Full",
          "Capturing the expanded theme surface and detached footer",
        );
        driver.send({
          type: "triggerBuiltin",
          builtinId: "builtin/choose-theme",
          requestId: "mwnd-matrix-full",
        });
        await captureMatrixState("matrix-full-expanded-2x", "full", "system");

        await announceTestStatus(
          "Visual matrix · Disabled",
          "Opening a safe confirmation so every footer action is visibly disabled",
        );
        driver.send({
          type: "triggerBuiltin",
          builtinId: "builtin/main-window",
          requestId: "mwnd-matrix-disabled-main",
        });
        await driver.setFilterAndWait("Clear Suggested", { timeoutMs: 15_000 });
        await driver.simulateGpuiKeyDown("enter", {
          target: { type: "id", id: "main" },
          timeoutMs: 15_000,
        });
        for (let attempt = 0; attempt < 20; attempt += 1) {
          const popupWindows = await driver.listAutomationWindows({ timeoutMs: 15_000 });
          if (((popupWindows as any)?.windows ?? []).some(
            (candidate: any) => candidate.id === "confirm-popup",
          )) {
            break;
          }
          await Bun.sleep(50);
        }
        const disabledState = await captureMatrixState(
          "matrix-disabled-confirm-2x",
          "mini",
          "system",
        );
        const disabledButtons = disabledState.activeFooter?.buttons ?? [];
        disabledState.disabledButtonCount = disabledButtons.filter(
          (button: any) => button.enabled === false,
        ).length;
        disabledState.pass = disabledState.pass
          && disabledButtons.length > 0
          && disabledState.disabledButtonCount === disabledButtons.length;
        if (!disabledState.pass) {
          structural.errors.push("disabled footer matrix state did not disable every action");
        }
        await driver.simulateGpuiKeyDown("escape", {
          target: { type: "id", id: "confirm-popup" },
          timeoutMs: 15_000,
        }).catch(() => null);
        await Bun.sleep(250);

        const themePath = join(driver.sessionDir, "home", ".scriptkit", "theme.json");
        const applyThemeFixture = async (
          label: string,
          expectedAppearance: "Light" | "Dark",
          theme: Record<string, unknown>,
        ) => {
          await announceTestStatus(
            `Visual matrix · ${label}`,
            "Applying a sandbox-only theme and waiting for native glass refresh",
          );
          const logOffset = Bun.file(driver.logPath).size;
          writeFileSync(themePath, `${JSON.stringify(theme, null, 2)}\n`);
          const loadMarker = `Theme load completed source=theme_json appearance=${expectedAppearance}`;
          let loadObserved = false;
          for (let attempt = 0; attempt < 150; attempt += 1) {
            const appendedLog = await Bun.file(driver.logPath).slice(logOffset).text();
            if (appendedLog.includes(loadMarker)) {
              loadObserved = true;
              break;
            }
            await Bun.sleep(100);
          }
          if (!loadObserved) {
            throw new Error(`theme reload marker not observed: ${loadMarker}`);
          }
          // The watcher log proves the cache changed. Give AppKit's native
          // glass tint and GPUI's theme projection one morph interval to
          // finish before measuring pixels.
          await Bun.sleep(1_200);
          await driver.waitForSettle({ timeoutMs: 10_000 });
        };
        driver.send({
          type: "triggerBuiltin",
          builtinId: "builtin/main-window",
          requestId: "mwnd-matrix-bright-main",
        });
        await driver.setFilterAndWait("Clear Suggested", { timeoutMs: 15_000 });
        await applyThemeFixture("Bright / light", "Light", {
          appearance: "light",
          colors: {
            background: {
              main: "#FFF7FF",
              title_bar: "#FFE5FF",
              search_box: "#FFFFFF",
              log_panel: "#F3E8FF",
            },
            text: {
              primary: "#111118",
              secondary: "#252536",
              tertiary: "#41415A",
              muted: "#5A5A72",
              dimmed: "#77778A",
              on_accent: "#FFFFFF",
            },
            accent: { selected: "#C026D3", selected_subtle: "#F0ABFC" },
            ui: {
              border: "#7E22CE",
              success: "#15803D",
              error: "#B91C1C",
              warning: "#B45309",
              info: "#1D4ED8",
            },
          },
          background_gradient: {
            enabled: true,
            from: "#FF00CC",
            to: "#00D4FF",
            angle: 130,
            opacity: 0.82,
            layers: [{
              enabled: true,
              from: "#FFD600",
              to: "#7C3AED",
              angle: 35,
              opacity: 0.55,
            }],
          },
          vibrancy: { enabled: true, material: "menu", backdrop_saturation: 2.6 },
          opacity: {
            main: 1.0,
            title_bar: 1.0,
            search_box: 1.0,
            log_panel: 1.0,
            vibrancy_background: 1.0,
            glass_veil_opacity: 0.9,
            glass_tint_opacity: 0.75,
          },
        });
        await captureMatrixState("matrix-bright-light-2x", "mini", "light");

        await driver.setFilterAndWait("Turn Off Background Effect", { timeoutMs: 15_000 });
        await driver.simulateGpuiKeyDown("enter", {
          target: { type: "id", id: "main" },
          timeoutMs: 15_000,
        });
        await ensureMainWindowVisible("mwnd-matrix-dark-main-show");
        driver.send({
          type: "triggerBuiltin",
          builtinId: "builtin/main-window",
          requestId: "mwnd-matrix-dark-main",
        });
        await driver.setFilterAndWait("Clear Suggested", { timeoutMs: 15_000 });
        await applyThemeFixture("Dark / plain", "Dark", {
          appearance: "dark",
          colors: {
            background: {
              main: "#0A0A0D",
              title_bar: "#111116",
              search_box: "#181820",
              log_panel: "#060608",
            },
            text: {
              primary: "#FFFFFF",
              secondary: "#E4E4E7",
              tertiary: "#A1A1AA",
              muted: "#71717A",
              dimmed: "#52525B",
              on_accent: "#0A0A0D",
            },
            accent: { selected: "#FBBF24", selected_subtle: "#3F3F46" },
            ui: {
              border: "#3F3F46",
              success: "#22C55E",
              error: "#EF4444",
              warning: "#F59E0B",
              info: "#3B82F6",
            },
          },
          background_gradient: null,
          vibrancy: { enabled: true, material: "menu", backdrop_saturation: 2.6 },
          opacity: {
            main: 1.0,
            title_bar: 1.0,
            search_box: 1.0,
            log_panel: 1.0,
            vibrancy_background: 1.0,
            glass_veil_opacity: 0.9,
            glass_tint_opacity: 0.75,
          },
        });
        await captureMatrixState("matrix-dark-plain-2x", "mini", "dark");

        const matrixHashes = new Set(
          matrixStates.map((state) => state.capture?.footerCropSha256).filter(Boolean),
        );
        const requiredNames = new Set([
          "matrix-full-expanded-2x",
          "matrix-disabled-confirm-2x",
          "matrix-bright-light-2x",
          "matrix-dark-plain-2x",
        ]);
        const requiredStatesPresent = [...requiredNames].every((name) =>
          matrixStates.some((state) => state.name === name)
        );
        receipt.visualMatrix = {
          pass: requiredStatesPresent
            && matrixStates.every((state) => state.pass)
            && matrixHashes.size === matrixStates.length,
          requiredNames: [...requiredNames],
          requiredStatesPresent,
          distinctFooterHashCount: matrixHashes.size,
          states: matrixStates,
          sandboxThemePath: themePath,
        };
        if (!(receipt.visualMatrix as any).pass) {
          structural.errors.push("stationary appearance/state/background matrix failed");
        }
      }
      for (const capture of captures as any[]) {
        capture.gutterTransparency ??= await analyzeNativeWindowGutterAlpha(
          capture.path,
          (receipt.layout as any)?.fidelity?.appKit,
        );
        if (!capture.gutterTransparency.pass) {
          structural.errors.push(
            ...((capture.gutterTransparency.errors as string[]).map(
              (error) => `${capture.name} transparent gutter: ${error}`,
            )),
          );
        }
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
      const capture: any = await captureNativeWindow(
        Number(main.pid),
        outDir,
        "fallback-main-window-2x",
      );
      receipt.stationary = {
        pass: capture.nativeWindow?.onscreen === true
          && Number(capture.nativeWindow?.alpha ?? 0) >= 0.99
          && Number(capture.edgeEnergy ?? 0) > 0,
        structural: "fallback intentionally has no native glass capsule hierarchy",
        captures: [capture],
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
      && (!visualMatrix || (receipt.visualMatrix as any)?.pass === true)
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
