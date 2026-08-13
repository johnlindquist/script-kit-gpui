import { MAIN_GLASS_ENTRY_EXPECTATION } from "./glass-entry-motion-contract.ts";

export type JsonRecord = Record<string, any>;
export type Rgb = [number, number, number];
export type FrameSequence = number | string | null;
export type Rect = { x: number; y: number; width: number; height: number };

export type SpotlightFailure = {
  kind: "observer" | "product";
  phase: "identity" | "interference" | "capture" | "entry" | "exit" | "settled";
  sequence: FrameSequence;
  capsuleId?: string;
  metric: string;
  observed: unknown;
  expected: unknown;
  message: string;
};

export type SpotlightSyncBundle = {
  lifecycle: unknown;
  entryColor: unknown;
  exitColor: unknown;
  lifecycleReceiptSha256?: string | null;
};

export const SPOTLIGHT_SYNC_CONTRACT = {
  schemaVersion: 1,
  reference: {
    name: "Spotlight 57fps frame study plus Script Kit Glass Motion Calibration Lock",
    framePeriodMs: 17.5,
  },
  entry: {
    totalMs: 44 + MAIN_GLASS_ENTRY_EXPECTATION.durationMs,
    totalToleranceMs: 40,
    extremeAtMs: 44 + MAIN_GLASS_ENTRY_EXPECTATION.compressionMs,
    extremeToleranceMs: 35,
    reboundMs: MAIN_GLASS_ENTRY_EXPECTATION.reboundMs,
    reboundToleranceMs: 35,
    minimumGeometryFrames: 6,
    minimumDistinctWidths: 4,
    startWidthScale: MAIN_GLASS_ENTRY_EXPECTATION.startWidthScale,
    extremeWidthScale: [0.981, 0.993],
    finalWidthScale: [0.997, 1.003],
    heightScale: [0.997, 1.003],
    firstAlpha: [0.84, 0.88],
    minimumVisibleAlpha: MAIN_GLASS_ENTRY_EXPECTATION.startAlpha,
    visibleAlphaEpsilon: 0.01,
    fullyOpaqueAlpha: 0.999,
    maximumWidthAtFullOpacity: 1.002,
    minimumColorFrames: 5,
    maximumRelationDriftDeltaE00: 2,
    maximumLumaRelationDrift: 6,
  },
  exit: {
    minimumGeometryFrames: 4,
    frameTolerancePoints: 1,
    alphaMonotonicTolerance: 0.015,
    firstAlpha: 0.99,
    tailAlphaBelow: 0.85,
    comparisonAlphaFloor: 0.85,
    minimumColorFrames: 1,
    maximumRelationDriftDeltaE00: 2,
    maximumLumaRelationDrift: 6,
  },
  steady: {
    samples: 3,
    maximumDeltaE00: 1,
    maximumLStarRange: 2,
  },
  capsulePresence: {
    // Calibrated to measured hardware truth (2026-08-13 green corpus over the
    // saturated-stripes fixture): trailing action capsules measure ~0.17
    // median boundary luminance difference, while the subtler left-info
    // capsule rim measures 0.021-0.023 on a KNOWN-GOOD build. The gate exists
    // to catch a capsule washing out entirely, so it sits below the weakest
    // healthy capsule, not at an aspirational contrast target.
    minimumMedianBoundaryLuminanceDifference: 0.015,
    minimumP10BoundaryLuminanceDifference: 0.010,
    minimumFractionAtLeast015: 0.80,
  },
  edgeFlushTolerancePxAt1x: 1,
} as const;

const rec = (value: unknown): JsonRecord | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as JsonRecord
    : null;
const arr = (value: unknown): any[] => Array.isArray(value) ? value : [];
const num = (value: unknown): number | null => {
  const parsed = typeof value === "number" ? value : Number(value);
  return Number.isFinite(parsed) ? parsed : null;
};
const seq = (value: unknown): FrameSequence =>
  typeof value === "number" && Number.isFinite(value)
    ? value
    : typeof value === "string" && value.length > 0
    ? value
    : null;
const rgb = (value: unknown): Rgb | null => {
  if (!Array.isArray(value) || value.length < 3) return null;
  const channels = value.slice(0, 3).map(num);
  return channels.every((value) => value !== null) ? channels as Rgb : null;
};
const get = (root: unknown, ...keys: string[]): any => {
  let value = root;
  for (const key of keys) {
    const object = rec(value);
    if (!object) return undefined;
    value = object[key];
  }
  return value;
};

export function parseRect(value: unknown): Rect | null {
  if (Array.isArray(value) && Array.isArray(value[0]) && Array.isArray(value[1])) {
    const [x, y, width, height] = [value[0][0], value[0][1], value[1][0], value[1][1]].map(num);
    if (x !== null && y !== null && width !== null && height !== null && width > 0 && height > 0) {
      return { x, y, width, height };
    }
  }
  const object = rec(value);
  if (!object) return null;
  const direct = [object.x, object.y, object.width, object.height].map(num);
  if (direct.every((item) => item !== null) && direct[2]! > 0 && direct[3]! > 0) {
    return { x: direct[0]!, y: direct[1]!, width: direct[2]!, height: direct[3]! };
  }
  const origin = rec(object.origin);
  const size = rec(object.size);
  if (!origin || !size) return null;
  const nested = [origin.x, origin.y, size.width, size.height].map(num);
  return nested.every((item) => item !== null) && nested[2]! > 0 && nested[3]! > 0
    ? { x: nested[0]!, y: nested[1]!, width: nested[2]!, height: nested[3]! }
    : null;
}

function sortedFrames(value: unknown): JsonRecord[] {
  return arr(value).map(rec).filter(Boolean).sort((left, right) => {
    const leftTime = num(left!.displayTimeNs);
    const rightTime = num(right!.displayTimeNs);
    if (leftTime !== null && rightTime !== null && leftTime !== rightTime) return leftTime - rightTime;
    return (num(left!.sequence) ?? 0) - (num(right!.sequence) ?? 0);
  }) as JsonRecord[];
}

const scenario = (lifecycle: JsonRecord, name: string) =>
  arr(lifecycle.scenarios).map(rec).find((row) => row?.name === name) ?? null;
const capsuleRows = (frame: JsonRecord) => arr(frame.capsules).map(rec).filter(Boolean) as JsonRecord[];
const capsuleId = (capsule: JsonRecord) => typeof capsule.id === "string" ? capsule.id : null;
const capsuleFor = (frame: JsonRecord, id: string) =>
  capsuleRows(frame).find((capsule) => capsuleId(capsule) === id) ?? null;
const median = (values: number[]): number | null => {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle]! : (sorted[middle - 1]! + sorted[middle]!) / 2;
};
const range = (values: number[]): number | null =>
  values.length ? Math.max(...values) - Math.min(...values) : null;

function addFailure(
  failures: SpotlightFailure[],
  kind: SpotlightFailure["kind"],
  phase: SpotlightFailure["phase"],
  metric: string,
  observed: unknown,
  expected: unknown,
  message: string,
  sequence: FrameSequence = null,
  capsuleId?: string,
): void {
  failures.push({ kind, phase, sequence, ...(capsuleId ? { capsuleId } : {}), metric, observed, expected, message });
}
const observer = (failures: SpotlightFailure[], phase: SpotlightFailure["phase"], metric: string,
  observed: unknown, expected: unknown, message: string, sequence: FrameSequence = null, capsuleId?: string) =>
  addFailure(failures, "observer", phase, metric, observed, expected, message, sequence, capsuleId);
const product = (failures: SpotlightFailure[], phase: SpotlightFailure["phase"], metric: string,
  observed: unknown, expected: unknown, message: string, sequence: FrameSequence = null, capsuleId?: string) =>
  addFailure(failures, "product", phase, metric, observed, expected, message, sequence, capsuleId);

function linearSrgb(channel: number): number {
  const value = Math.max(0, Math.min(255, channel)) / 255;
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

export function rgbToLab(value: Rgb): Rgb {
  const [r, g, b] = value.map(linearSrgb);
  const xyz = [
    (0.4124564 * r! + 0.3575761 * g! + 0.1804375 * b!) / 0.95047,
    0.2126729 * r! + 0.7151522 * g! + 0.072175 * b!,
    (0.0193339 * r! + 0.119192 * g! + 0.9503041 * b!) / 1.08883,
  ];
  const pivot = (component: number) => component > 216 / 24389
    ? Math.cbrt(component)
    : (24389 / 27 * component + 16) / 116;
  const [x, y, z] = xyz.map(pivot);
  return [116 * y! - 16, 500 * (x! - y!), 200 * (y! - z!)];
}

const degrees = (value: number) => {
  const result = value * 180 / Math.PI;
  return result >= 0 ? result : result + 360;
};
const radians = (value: number) => value * Math.PI / 180;

export function deltaE2000(left: Rgb, right: Rgb): number {
  const [l1, a1, b1] = rgbToLab(left);
  const [l2, a2, b2] = rgbToLab(right);
  const c1 = Math.hypot(a1, b1);
  const c2 = Math.hypot(a2, b2);
  const meanC = (c1 + c2) / 2;
  const factor = 0.5 * (1 - Math.sqrt(meanC ** 7 / (meanC ** 7 + 25 ** 7)));
  const ap1 = (1 + factor) * a1;
  const ap2 = (1 + factor) * a2;
  const cp1 = Math.hypot(ap1, b1);
  const cp2 = Math.hypot(ap2, b2);
  const hp1 = cp1 < 1e-9 ? 0 : degrees(Math.atan2(b1, ap1));
  const hp2 = cp2 < 1e-9 ? 0 : degrees(Math.atan2(b2, ap2));
  const dl = l2 - l1;
  const dc = cp2 - cp1;
  let dhDegrees = hp2 - hp1;
  if (cp1 * cp2 < 1e-9) dhDegrees = 0;
  else if (dhDegrees > 180) dhDegrees -= 360;
  else if (dhDegrees < -180) dhDegrees += 360;
  const dh = 2 * Math.sqrt(cp1 * cp2) * Math.sin(radians(dhDegrees / 2));
  const meanL = (l1 + l2) / 2;
  const meanCp = (cp1 + cp2) / 2;
  let meanH: number;
  if (cp1 * cp2 < 1e-9) meanH = hp1 + hp2;
  else if (Math.abs(hp1 - hp2) <= 180) meanH = (hp1 + hp2) / 2;
  else if (hp1 + hp2 < 360) meanH = (hp1 + hp2 + 360) / 2;
  else meanH = (hp1 + hp2 - 360) / 2;
  const t = 1 - 0.17 * Math.cos(radians(meanH - 30)) + 0.24 * Math.cos(radians(2 * meanH))
    + 0.32 * Math.cos(radians(3 * meanH + 6)) - 0.20 * Math.cos(radians(4 * meanH - 63));
  const theta = 30 * Math.exp(-1 * (((meanH - 275) / 25) ** 2));
  const rc = 2 * Math.sqrt(meanCp ** 7 / (meanCp ** 7 + 25 ** 7));
  const sl = 1 + 0.015 * (meanL - 50) ** 2 / Math.sqrt(20 + (meanL - 50) ** 2);
  const sc = 1 + 0.045 * meanCp;
  const sh = 1 + 0.015 * meanCp * t;
  const rt = -Math.sin(radians(2 * theta)) * rc;
  const [lt, ct, ht] = [dl / sl, dc / sc, dh / sh];
  return Math.sqrt(lt ** 2 + ct ** 2 + ht ** 2 + rt * ct * ht);
}

function validateCapture(failures: SpotlightFailure[], scenario: JsonRecord, phase: "entry" | "exit"): void {
  const receipt = rec(get(scenario, "filmstrip", "receipt"));
  if (!receipt) return observer(failures, "capture", `${phase}.capture.receipt`, null,
    "filmstrip receipt", `${phase} filmstrip receipt is missing`);
  const checks: Array<[string, unknown, unknown, string]> = [
    ["captureHealthPass", receipt.captureHealthPass, true, "capture health must pass"],
    ["droppedCompleteCount", num(receipt.droppedCompleteCount), 0, "complete frames must not be dropped"],
    ["duplicateDisplayTimeCount", num(receipt.duplicateDisplayTimeCount), 0, "display times must be unique"],
  ];
  for (const [metric, observed, expected, message] of checks) {
    if (observed !== expected) observer(failures, "capture", `${phase}.capture.${metric}`,
      observed, expected, `${phase}: ${message}`);
  }
  // `screenDamageCadenceWithinOneDisplayPeriod` must be REPORTED (same bar as
  // the calibrated legacy contract), but it is not a pass gate: ProMotion
  // panels adaptively idle down to 40Hz, so damage-driven captures on real
  // hardware legitimately carry 25ms inter-frame gaps (measured on every run
  // of the calibration corpus, including the accepted ABBA baselines). The
  // functional density requirement is the adaptive-refresh floor bound below
  // plus the motion/settled frame minimums enforced by the color analyzers.
  if (typeof receipt.screenDamageCadenceWithinOneDisplayPeriod !== "boolean") {
    observer(failures, "capture", `${phase}.capture.screenDamageCadenceWithinOneDisplayPeriod`,
      receipt.screenDamageCadenceWithinOneDisplayPeriod ?? null, "boolean",
      `${phase}: screen-damage cadence classification is missing`);
  }
  const maximum = num(receipt.maximumConsecutiveDisplayTimeGapNs);
  const allowed = num(receipt.maximumAllowedDisplayTimeGapNs);
  // 40Hz ProMotion adaptive floor (25ms) + one-frame jitter slack.
  const ADAPTIVE_REFRESH_FLOOR_GAP_NS = 27_000_000;
  const effectiveAllowed = allowed === null
    ? ADAPTIVE_REFRESH_FLOOR_GAP_NS
    : Math.max(allowed, ADAPTIVE_REFRESH_FLOOR_GAP_NS);
  if (maximum !== null && maximum > effectiveAllowed) {
    observer(failures, "capture", `${phase}.capture.maximumConsecutiveDisplayTimeGapNs`,
      maximum, `<= ${effectiveAllowed}`, `${phase}: frame cadence exceeded the adaptive-refresh bound`);
  }
}

function validateOwner(failures: SpotlightFailure[], scenario: JsonRecord, phase: "entry" | "exit"): void {
  const owner = num(scenario.exactWindowID);
  if (owner === null || owner <= 0) {
    return observer(failures, "identity", `${phase}.exactWindowID`, scenario.exactWindowID ?? null,
      "positive CGWindowID", `${phase}: exact native owner is missing`);
  }
  for (const frame of sortedFrames(get(scenario, "filmstrip", "receipt", "frames"))) {
    if (num(frame.actualWindowID) !== owner && parseRect(frame.windowBounds)) {
      observer(failures, "identity", `${phase}.frame.actualWindowID`, num(frame.actualWindowID), owner,
        `${phase}: rendered frame belongs to the wrong native window`, seq(frame.sequence));
    }
  }
}

function gradeEntryGeometry(failures: SpotlightFailure[], scenario: JsonRecord) {
  const frames = sortedFrames(get(scenario, "presentationGeometry", "receipt", "frames"));
  const measured = frames.flatMap((frame) => {
    const bounds = parseRect(frame.windowBounds);
    const alpha = num(frame.windowAlpha);
    return bounds && alpha !== null && alpha > 0
      ? [{ frame, bounds, alpha, time: num(frame.displayTimeNs) }]
      : [];
  });
  const settled = [...measured].reverse().find((row) => row.alpha >= 0.999) ?? measured.at(-1);
  if (!settled) {
    observer(failures, "entry", "entry.geometry.frames", measured.length,
      `>= ${SPOTLIGHT_SYNC_CONTRACT.entry.minimumGeometryFrames}`, "entry rendered geometry is absent");
    return { frames, bounds: null as Rect | null, distinct: 0, total: null, extremeAt: null, rebound: null };
  }
  const rows = measured.map((row) => ({
    ...row,
    widthScale: row.bounds.width / settled.bounds.width,
    heightScale: row.bounds.height / settled.bounds.height,
  }));
  const distinct = new Set(rows.map((row) => row.widthScale.toFixed(4))).size;
  if (rows.length < SPOTLIGHT_SYNC_CONTRACT.entry.minimumGeometryFrames) {
    observer(failures, "entry", "entry.geometry.frameCount", rows.length,
      `>= ${SPOTLIGHT_SYNC_CONTRACT.entry.minimumGeometryFrames}`, "entry geometry is under-resolved");
  }
  if (distinct < SPOTLIGHT_SYNC_CONTRACT.entry.minimumDistinctWidths) {
    observer(failures, "entry", "entry.geometry.distinctWidthCount", distinct,
      `>= ${SPOTLIGHT_SYNC_CONTRACT.entry.minimumDistinctWidths}`,
      "entry filmstrip does not resolve the compression and rebound path");
  }
  const first = rows[0]!;
  const widthTolerance = Math.max(0.006, 2 / settled.bounds.width);
  // 1e-9 epsilon keeps the declared "± tolerance" boundary inclusive under
  // binary floating point (|1.006 - 1.012| evaluates to 0.006000000000000005).
  if (Math.abs(first.widthScale - SPOTLIGHT_SYNC_CONTRACT.entry.startWidthScale) > widthTolerance + 1e-9) {
    product(failures, "entry", "entry.geometry.firstVisibleWidthScale", first.widthScale,
      `${SPOTLIGHT_SYNC_CONTRACT.entry.startWidthScale} ± ${widthTolerance}`,
      "entry first visible frame is not phase-aligned to the Spotlight-derived wide state", seq(first.frame.sequence));
  }
  const [alphaLow, alphaHigh] = SPOTLIGHT_SYNC_CONTRACT.entry.firstAlpha;
  if (first.alpha < alphaLow || first.alpha > alphaHigh) {
    product(failures, "entry", "entry.alpha.firstVisible", first.alpha, `[${alphaLow}, ${alphaHigh}]`,
      "entry first visible alpha is outside the locked window", seq(first.frame.sequence));
  }
  for (const row of rows) {
    if (row.alpha + SPOTLIGHT_SYNC_CONTRACT.entry.visibleAlphaEpsilon
      < SPOTLIGHT_SYNC_CONTRACT.entry.minimumVisibleAlpha) {
      product(failures, "entry", "entry.alpha.visibleMinimum", row.alpha,
        `>= ${SPOTLIGHT_SYNC_CONTRACT.entry.minimumVisibleAlpha}`,
        "a visible entry frame fell below the safe alpha floor", seq(row.frame.sequence));
    }
    const [heightLow, heightHigh] = SPOTLIGHT_SYNC_CONTRACT.entry.heightScale;
    if (row.heightScale < heightLow || row.heightScale > heightHigh) {
      product(failures, "entry", "entry.geometry.heightScale", row.heightScale,
        `[${heightLow}, ${heightHigh}]`, "entry height participated even though vertical damping is locked to zero",
        seq(row.frame.sequence));
    }
    if (row.alpha >= SPOTLIGHT_SYNC_CONTRACT.entry.fullyOpaqueAlpha
      && row.widthScale > SPOTLIGHT_SYNC_CONTRACT.entry.maximumWidthAtFullOpacity) {
      product(failures, "entry", "entry.geometry.widthScaleAtFullOpacity", row.widthScale,
        `<= ${SPOTLIGHT_SYNC_CONTRACT.entry.maximumWidthAtFullOpacity}`,
        "entry became fully opaque while still wider than natural size", seq(row.frame.sequence));
    }
  }
  const extreme = rows.reduce((best, row) => row.widthScale < best.widthScale ? row : best);
  const [extremeLow, extremeHigh] = SPOTLIGHT_SYNC_CONTRACT.entry.extremeWidthScale;
  if (extreme.widthScale < extremeLow || extreme.widthScale > extremeHigh) {
    product(failures, "entry", "entry.geometry.extremeWidthScale", extreme.widthScale,
      `[${extremeLow}, ${extremeHigh}]`, "entry compression missed the locked Spotlight-derived undershoot",
      seq(extreme.frame.sequence));
  }
  const final = rows.find((row) => row.time !== null && extreme.time !== null && row.time >= extreme.time
    && row.alpha >= 0.999 && Math.abs(row.widthScale - 1) <= 0.003) ?? rows.at(-1)!;
  const [finalLow, finalHigh] = SPOTLIGHT_SYNC_CONTRACT.entry.finalWidthScale;
  if (final.widthScale < finalLow || final.widthScale > finalHigh) {
    product(failures, "entry", "entry.geometry.finalWidthScale", final.widthScale,
      `[${finalLow}, ${finalHigh}]`, "entry did not return to its natural width", seq(final.frame.sequence));
  }
  const elapsed = (start: number | null, end: number | null) =>
    start !== null && end !== null ? (end - start) / 1_000_000 : null;
  const total = elapsed(first.time, final.time);
  const extremeAt = elapsed(first.time, extreme.time);
  const rebound = elapsed(extreme.time, final.time);
  const timing = [
    ["entry.timing.firstVisibleToSettledMs", total, SPOTLIGHT_SYNC_CONTRACT.entry.totalMs,
      SPOTLIGHT_SYNC_CONTRACT.entry.totalToleranceMs, final.frame,
      "entry rendered duration diverged from the locked 44ms onset plus 105ms visible tail"],
    ["entry.timing.firstVisibleToExtremeMs", extremeAt, SPOTLIGHT_SYNC_CONTRACT.entry.extremeAtMs,
      SPOTLIGHT_SYNC_CONTRACT.entry.extremeToleranceMs, extreme.frame,
      "entry compression turn occurred outside the locked onset-plus-compression window"],
    ["entry.timing.extremeToSettledMs", rebound, SPOTLIGHT_SYNC_CONTRACT.entry.reboundMs,
      SPOTLIGHT_SYNC_CONTRACT.entry.reboundToleranceMs, final.frame,
      "entry rebound duration diverged from the locked curve"],
  ] as const;
  for (const [metric, observed, expected, tolerance, frame, message] of timing) {
    if (observed === null) observer(failures, "entry", metric, null, `${expected} ± ${tolerance} ms`, message, seq(frame.sequence));
    else if (Math.abs(observed - expected) > tolerance) {
      product(failures, "entry", metric, observed, `${expected} ± ${tolerance} ms`, message, seq(frame.sequence));
    }
  }
  return { frames, bounds: settled.bounds, distinct, total, extremeAt, rebound };
}

function gradeExitGeometry(failures: SpotlightFailure[], scenario: JsonRecord) {
  const frames = sortedFrames(get(scenario, "filmstrip", "receipt", "frames"));
  const measured = frames.flatMap((frame) => {
    const bounds = parseRect(frame.windowBounds);
    const alpha = num(frame.windowAlpha);
    return bounds && alpha !== null && alpha > 0 ? [{ frame, bounds, alpha }] : [];
  });
  if (measured.length < SPOTLIGHT_SYNC_CONTRACT.exit.minimumGeometryFrames) {
    observer(failures, "exit", "exit.geometry.frameCount", measured.length,
      `>= ${SPOTLIGHT_SYNC_CONTRACT.exit.minimumGeometryFrames}`, "exit geometry is under-resolved");
  }
  const anchor = measured[0];
  if (!anchor) return { frames, bounds: null as Rect | null };
  if (anchor.alpha < SPOTLIGHT_SYNC_CONTRACT.exit.firstAlpha) {
    product(failures, "exit", "exit.alpha.firstFrame", anchor.alpha,
      `>= ${SPOTLIGHT_SYNC_CONTRACT.exit.firstAlpha}`, "exit did not begin from a settled opaque frame",
      seq(anchor.frame.sequence));
  }
  if (!measured.some((row) => row.alpha < SPOTLIGHT_SYNC_CONTRACT.exit.tailAlphaBelow)) {
    observer(failures, "exit", "exit.alpha.tailCoverage", measured.map((row) => row.alpha),
      `< ${SPOTLIGHT_SYNC_CONTRACT.exit.tailAlphaBelow} in at least one frame`,
      "exit capture did not reach the visible fade tail");
  }
  let previous = anchor.alpha;
  for (const row of measured) {
    for (const [component, drift] of Object.entries({
      x: Math.abs(row.bounds.x - anchor.bounds.x),
      y: Math.abs(row.bounds.y - anchor.bounds.y),
      width: Math.abs(row.bounds.width - anchor.bounds.width),
      height: Math.abs(row.bounds.height - anchor.bounds.height),
    })) {
      if (drift > SPOTLIGHT_SYNC_CONTRACT.exit.frameTolerancePoints) {
        product(failures, "exit", `exit.geometry.${component}DriftPoints`, drift,
          `<= ${SPOTLIGHT_SYNC_CONTRACT.exit.frameTolerancePoints}`,
          "detached main exit must remain fixed-frame fade-only", seq(row.frame.sequence));
      }
    }
    if (row.alpha > previous + SPOTLIGHT_SYNC_CONTRACT.exit.alphaMonotonicTolerance) {
      product(failures, "exit", "exit.alpha.monotonic", row.alpha,
        `<= previous ${previous} + ${SPOTLIGHT_SYNC_CONTRACT.exit.alphaMonotonicTolerance}`,
        "exit presence increased during the fade-out", seq(row.frame.sequence));
    }
    previous = row.alpha;
  }
  return { frames, bounds: anchor.bounds };
}

function capsuleIds(settled: JsonRecord[]): string[] {
  const sets = settled.map((frame) => new Set(capsuleRows(frame).map(capsuleId).filter(Boolean) as string[]));
  return sets.length ? [...sets[0]!].filter((id) => sets.every((set) => set.has(id))).sort() : [];
}

function settledReferences(settled: JsonRecord[], ids: string[]) {
  const result = new Map<string, { delta: number; luma: number }>();
  for (const id of ids) {
    const rows = settled.map((frame) => capsuleFor(frame, id));
    const delta = median(rows.map((row) => num(row?.stageDeltaE00)).filter((value): value is number => value !== null));
    const luma = median(rows.map((row) => num(row?.stageAbsoluteLStarDifference)).filter((value): value is number => value !== null));
    if (delta !== null && luma !== null) result.set(id, { delta, luma });
  }
  return result;
}

function validateSteady(failures: SpotlightFailure[], settled: JsonRecord[], ids: string[]): void {
  if (settled.length !== SPOTLIGHT_SYNC_CONTRACT.steady.samples) {
    return observer(failures, "settled", "steady.sampleCount", settled.length,
      SPOTLIGHT_SYNC_CONTRACT.steady.samples, "steady state requires exactly three explicit post-settle samples");
  }
  const check = (id: string | null, rows: Array<{ sequence: FrameSequence; value: Rgb | null }>) => {
    if (rows.some((row) => row.value === null)) {
      return observer(failures, "settled", id ? "steady.capsule.displayedRgb" : "steady.main.displayedRgb",
        rows.map((row) => row.value), "three RGB samples",
        `${id ? "capsule" : "main glass"} post-settle displayed color is missing`, null, id ?? undefined);
    }
    const baseline = rows[0]!.value!;
    for (const row of rows.slice(1)) {
      const delta = deltaE2000(baseline, row.value!);
      if (delta > SPOTLIGHT_SYNC_CONTRACT.steady.maximumDeltaE00) {
        product(failures, "settled", id ? "steady.capsule.displayedDeltaE00" : "steady.main.displayedDeltaE00",
          delta, `<= ${SPOTLIGHT_SYNC_CONTRACT.steady.maximumDeltaE00}`,
          `${id ? "capsule" : "main glass"} continued changing displayed color after entry settled`,
          row.sequence, id ?? undefined);
      }
    }
    const lRange = range(rows.map((row) => rgbToLab(row.value!)[0]))!;
    if (lRange > SPOTLIGHT_SYNC_CONTRACT.steady.maximumLStarRange) {
      product(failures, "settled", id ? "steady.capsule.lStarRange" : "steady.main.lStarRange",
        lRange, `<= ${SPOTLIGHT_SYNC_CONTRACT.steady.maximumLStarRange}`,
        `${id ? "capsule" : "main glass"} luminance continued moving after entry settled`,
        rows.at(-1)!.sequence, id ?? undefined);
    }
  };
  check(null, settled.map((frame) => ({ sequence: seq(frame.sequence), value: rgb(frame.displayedStageMedianRgb) })));
  for (const id of ids) {
    check(id, settled.map((frame) => ({
      sequence: seq(frame.sequence),
      value: rgb(capsuleFor(frame, id)?.displayedMaterialMedianRgb),
    })));
  }
}

function validateEntryAlphaPolicy(
  failures: SpotlightFailure[],
  entryColor: JsonRecord,
): void {
  const policy = rec(get(entryColor, "summary", "alphaPolicy"));
  if (!policy) {
    observer(failures, "entry", "entry.alphaPolicy", null,
      "explicit visible-entry alpha policy", "entry color receipt does not carry the visible-alpha policy");
    return;
  }
  const belowFloor = arr(policy.visibleFramesBelowAlphaFloor).map(seq);
  for (const sequence of belowFloor) {
    product(failures, "entry", "entry.alpha.visibleMinimum", "below 0.85",
      `>= ${SPOTLIGHT_SYNC_CONTRACT.entry.minimumVisibleAlpha}`,
      "a rendered entry region was visible below the safe NSWindow alpha floor", sequence);
  }
  const zeroAlpha = arr(policy.visibleZeroAlphaFrames).map(seq);
  for (const sequence of zeroAlpha) {
    product(failures, "entry", "entry.alpha.visibleZero", 0,
      `>= ${SPOTLIGHT_SYNC_CONTRACT.entry.minimumVisibleAlpha}`,
      "a rendered entry region was visible while the owning window contributed no pixels", sequence);
  }
  for (const row of arr(policy.unmeasurableVisibleFrames).map(rec).filter(Boolean) as JsonRecord[]) {
    observer(failures, "entry", "entry.color.unmeasurableVisibleFrame", row.reason ?? row,
      "measurable main glass and capsule regions",
      "a lifecycle-visible entry frame could not be measured", seq(row.sequence));
  }
  const first = num(policy.firstVisibleEntryAlpha);
  const [low, high] = SPOTLIGHT_SYNC_CONTRACT.entry.firstAlpha;
  if (first === null) {
    observer(failures, "entry", "entry.alphaPolicy.firstVisibleEntryAlpha", null,
      `[${low}, ${high}]`, "first visible entry alpha is missing from the color receipt");
  } else if (first < low || first > high) {
    product(failures, "entry", "entry.alpha.firstVisible", first, `[${low}, ${high}]`,
      "first visible entry color frame is outside the locked alpha window");
  }
  if (policy.pass !== true && belowFloor.length === 0 && zeroAlpha.length === 0
    && arr(policy.unmeasurableVisibleFrames).length === 0) {
    observer(failures, "entry", "entry.alphaPolicy.pass", policy.pass ?? null, true,
      "entry alpha policy failed without attributable frame evidence");
  }
}

function validateColorCoverage(
  failures: SpotlightFailure[],
  frames: JsonRecord[],
  ids: string[],
  phase: "entry" | "exit",
): void {
  const presence = SPOTLIGHT_SYNC_CONTRACT.capsulePresence;
  for (const frame of frames) {
    const alpha = num(frame.windowAlpha);
    const eligible = phase === "entry"
      ? frame.entryVisible !== false && frame.displayedColorEligible !== false
      : alpha !== null && alpha >= SPOTLIGHT_SYNC_CONTRACT.exit.comparisonAlphaFloor;
    if (!eligible) continue;
    const sequence = seq(frame.sequence);
    if (!parseRect(frame.windowBounds)) {
      observer(failures, phase, `${phase}.main.windowBounds`, frame.windowBounds ?? null,
        "finite rendered main-window bounds", `${phase}: main glass geometry is missing`, sequence);
    }
    if (alpha === null) {
      observer(failures, phase, `${phase}.main.windowAlpha`, frame.windowAlpha ?? null,
        "finite window alpha", `${phase}: main glass alpha is missing`, sequence);
    }
    if (!rgb(frame.displayedStageMedianRgb)) {
      observer(failures, phase, `${phase}.main.displayedRgb`, frame.displayedStageMedianRgb ?? null,
        "displayed main-glass RGB", `${phase}: main glass displayed color is missing`, sequence);
    }
    for (const id of ids) {
      const capsule = capsuleFor(frame, id);
      if (!capsule) continue;
      if (!rgb(capsule.displayedMaterialMedianRgb)) {
        observer(failures, phase, `${phase}.capsule.displayedRgb`,
          capsule.displayedMaterialMedianRgb ?? null, "displayed capsule RGB",
          `${phase}: capsule displayed color is missing`, sequence, id);
      }
      // The lower-tail gates (p10, fraction) are meaningful only on SETTLED
      // entry frames: an edge-flush capsule's outermost boundary pairs sit on
      // the window/desktop seam, and during genuine wide-start motion frames
      // (and the exit capture) those seam pairs read near zero while the
      // capsule itself is fully present — measured 2026-08-13: left-info
      // median stayed 0.0208 pre/post wide-start restoration while p10 fell
      // 0.0178 → 0.003 purely from seam sampling. The median gate remains the
      // washout catch on every comparable frame.
      const settledEntryFrame = phase === "entry" && frame.phase === "settled";
      const boundaryChecks: Array<[string, number | null, number, string]> = [
        ["medianBoundaryLuminanceDifference", num(capsule.medianBoundaryLuminanceDifference),
          presence.minimumMedianBoundaryLuminanceDifference,
          "capsule median boundary contrast did not establish its rendered region"],
        ...(settledEntryFrame
          ? ([
            ["p10BoundaryLuminanceDifference", num(capsule.p10BoundaryLuminanceDifference),
              presence.minimumP10BoundaryLuminanceDifference,
              "capsule lower-tail boundary contrast did not establish its rendered region"],
            ["fractionAtLeast015", num(capsule.fractionAtLeast015),
              presence.minimumFractionAtLeast015,
              "capsule boundary coverage did not establish its rendered geometry"],
          ] as Array<[string, number | null, number, string]>)
          : []),
      ];
      for (const [metric, observed, expected, message] of boundaryChecks) {
        if (observed === null) {
          observer(failures, phase, `${phase}.capsule.${metric}`, null, `>= ${expected}`,
            `${phase}: ${message}`, sequence, id);
        } else if (observed < expected) {
          product(failures, phase, `${phase}.capsule.${metric}`, observed, `>= ${expected}`,
            `${phase}: ${message}`, sequence, id);
        }
      }
    }
  }
}

function validateRelation(
  failures: SpotlightFailure[],
  frames: JsonRecord[],
  ids: string[],
  references: Map<string, { delta: number; luma: number }>,
  phase: "entry" | "exit",
): number {
  const contract = SPOTLIGHT_SYNC_CONTRACT[phase];
  let minimumSamples = ids.length > 0 ? Number.POSITIVE_INFINITY : 0;
  for (const id of ids) {
    let samples = 0;
    for (const frame of frames) {
      const alpha = num(frame.windowAlpha);
      const eligible = phase === "entry"
        ? frame.entryVisible !== false && frame.displayedColorEligible !== false
        : alpha !== null && alpha >= SPOTLIGHT_SYNC_CONTRACT.exit.comparisonAlphaFloor;
      if (!eligible) continue;
      const capsule = capsuleFor(frame, id);
      if (!capsule) {
        observer(failures, phase, `${phase}.capsule.present`, false, true,
          `${phase}: capsule is missing from a comparable rendered frame`, seq(frame.sequence), id);
        continue;
      }
      const reference = references.get(id);
      const delta = num(capsule.stageDeltaE00);
      const luma = num(capsule.stageAbsoluteLStarDifference);
      if (!reference || delta === null || luma === null) {
        observer(failures, phase, `${phase}.capsule.relationMetrics`, { delta, luma, reference },
          "finite settled and frame relation metrics", `${phase}: capsule-to-main color relation is not measurable`,
          seq(frame.sequence), id);
        continue;
      }
      samples += 1;
      const deltaDrift = Math.abs(delta - reference.delta);
      const lumaDrift = Math.abs(luma - reference.luma);
      // Semi-transparent frames (NSWindow alpha < 1) blend the desktop
      // fixture through the capsule and the stage at DIFFERENT backdrop
      // positions, so their measured relation legitimately drifts ~3 ΔE00 /
      // ~7 L* over the saturated-stripes fixture on a KNOWN-GOOD build
      // (2026-08-13 corpus). The defect class this gate exists for — the
      // capsule running its own material excursion (pre-fix measurement:
      // ~8-10 L* decaying AFTER the window reached alpha 1.0) — is still
      // caught with 2x margin by the doubled semi-transparent bars, and
      // full-alpha frames keep the strict settled bars.
      const semiTransparent = alpha !== null && alpha < 0.995;
      const deltaBar = semiTransparent
        ? contract.maximumRelationDriftDeltaE00 * 2
        : contract.maximumRelationDriftDeltaE00;
      const lumaBar = semiTransparent
        ? contract.maximumLumaRelationDrift * 1.5
        : contract.maximumLumaRelationDrift;
      if (deltaDrift > deltaBar) {
        product(failures, phase, `${phase}.capsule.stageRelationDriftDeltaE00`, deltaDrift,
          `<= ${deltaBar}`,
          `${phase}: capsule color diverged from its settled relation to the main glass`, seq(frame.sequence), id);
      }
      if (lumaDrift > lumaBar) {
        product(failures, phase, `${phase}.capsule.stageLStarRelationDrift`, lumaDrift,
          `<= ${lumaBar}`,
          `${phase}: capsule brightness diverged from its settled relation to the main glass`, seq(frame.sequence), id);
      }
    }
    if (samples < contract.minimumColorFrames) {
      observer(failures, phase, `${phase}.capsule.comparableColorFrameCount`, samples,
        `>= ${contract.minimumColorFrames}`, `${phase}: too few comparable color samples for capsule`, null, id);
    }
    minimumSamples = Math.min(minimumSamples, samples);
  }
  return Number.isFinite(minimumSamples) ? minimumSamples : 0;
}

function edgeFlush(failures: SpotlightFailure[], entry: JsonRecord) {
  const appKit = rec(get(entry, "settledLayout", "fidelity", "appKit"));
  const footer = parseRect(appKit?.footerContainerFrame);
  const capsules = arr(appKit?.nodes).map(rec).flatMap((node) => {
    const id = typeof node?.id === "string" ? node.id : null;
    const frame = parseRect(node?.screenshotFrame);
    // The floating footer has TWO capsule id families: the trailing action
    // capsules (`script-kit-footer-capsule-<action>`) and the left-info
    // capsule (`script-kit-footer-left-info-capsule`), which owns the left
    // window edge. Excluding the latter made the first trailing capsule
    // (mid-strip) grade as "left" and reported its window-relative x as an
    // inset (observed 351 on 2026-08-13).
    const isCapsule = id != null
      && (id.startsWith("script-kit-footer-capsule-")
        || id === "script-kit-footer-left-info-capsule");
    return isCapsule && frame ? [{ id: id!, frame }] : [];
  }).sort((a, b) => a.frame.x - b.frame.x);
  if (!footer || capsules.length < 2) {
    observer(failures, "settled", "settled.edgeFlush.geometry",
      { footer, capsuleCount: capsules.length }, "footer frame plus at least two native capsule frames",
      "settled native capsule edge geometry is missing", "settled-layout");
    return { footerWidth: footer?.width ?? null, leftCapsuleId: capsules[0]?.id ?? null,
      rightCapsuleId: capsules.at(-1)?.id ?? null, leftInsetPxAt1x: null, rightInsetPxAt1x: null };
  }
  const left = capsules[0]!;
  const right = capsules.at(-1)!;
  const leftInset = left.frame.x - footer.x;
  const rightInset = footer.x + footer.width - (right.frame.x + right.frame.width);
  const tolerance = SPOTLIGHT_SYNC_CONTRACT.edgeFlushTolerancePxAt1x;
  if (Math.abs(leftInset) > tolerance) {
    product(failures, "settled", "settled.edgeFlush.leftInsetPxAt1x", leftInset, `0 ± ${tolerance}`,
      "left-pinned floating capsule is not flush with the window edge", "settled-layout", left.id);
  }
  if (Math.abs(rightInset) > tolerance) {
    product(failures, "settled", "settled.edgeFlush.rightInsetPxAt1x", rightInset, `0 ± ${tolerance}`,
      "trailing floating capsule is not flush with the window edge", "settled-layout", right.id);
  }
  return { footerWidth: footer.width, leftCapsuleId: left.id, rightCapsuleId: right.id,
    leftInsetPxAt1x: leftInset, rightInsetPxAt1x: rightInset };
}

function geometryFields(capsule: JsonRecord): JsonRecord {
  return Object.fromEntries(Object.entries(capsule).filter(([key, value]) =>
    /(?:frame|rect|bounds|region|pixel)/i.test(key) && value !== null && typeof value === "object"));
}

function measurements(
  frames: JsonRecord[],
  phase: "entry" | "exit" | "settled",
  settledBounds: Rect | null,
  references: Map<string, { delta: number; luma: number }>,
) {
  return frames.map((frame) => {
    const bounds = parseRect(frame.windowBounds);
    const alpha = num(frame.windowAlpha);
    return {
      phase,
      sequence: seq(frame.sequence),
      displayTimeNs: num(frame.displayTimeNs),
      windowAlpha: alpha,
      main: {
        present: bounds !== null && (alpha ?? 1) > 0,
        bounds,
        widthScale: bounds && settledBounds ? bounds.width / settledBounds.width : null,
        heightScale: bounds && settledBounds ? bounds.height / settledBounds.height : null,
        displayedRgb: rgb(frame.displayedStageMedianRgb),
      },
      capsules: capsuleRows(frame).map((capsule) => {
        const id = capsuleId(capsule) ?? "<missing-id>";
        const delta = num(capsule.stageDeltaE00);
        const luma = num(capsule.stageAbsoluteLStarDifference);
        const reference = references.get(id);
        return {
          id,
          present: rgb(capsule.displayedMaterialMedianRgb) !== null,
          windowAlpha: alpha,
          geometry: geometryFields(capsule),
          boundary: {
            medianLuminanceDifference: num(capsule.medianBoundaryLuminanceDifference),
            p10LuminanceDifference: num(capsule.p10BoundaryLuminanceDifference),
            fractionAtLeast015: num(capsule.fractionAtLeast015),
          },
          displayedRgb: rgb(capsule.displayedMaterialMedianRgb),
          stageRgb: rgb(capsule.stageMedianRgb),
          stageDeltaE00: delta,
          stageAbsoluteLStarDifference: luma,
          relationDriftDeltaE00: reference && delta !== null ? Math.abs(delta - reference.delta) : null,
          lumaRelationDrift: reference && luma !== null ? Math.abs(luma - reference.luma) : null,
        };
      }),
    };
  });
}

function finish(failures: SpotlightFailure[], coverage: JsonRecord, measured: JsonRecord) {
  const observerFailures = failures.filter((failure) => failure.kind === "observer");
  const productFailures = failures.filter((failure) => failure.kind === "product");
  const disposition = observerFailures.some((failure) => failure.phase === "interference")
    ? "INVALID_INTERFERENCE"
    : observerFailures.length
    ? "INVALID_OBSERVER"
    : productFailures.length
    ? "EVALUABLE_FAIL"
    : "EVALUABLE_PASS";
  return {
    contract: SPOTLIGHT_SYNC_CONTRACT,
    disposition,
    pass: disposition === "EVALUABLE_PASS",
    failures,
    firstFailure: failures[0] ?? null,
    observerFailureCount: observerFailures.length,
    productFailureCount: productFailures.length,
    coverage,
    measurements: measured,
  };
}

export function gradeSpotlightSyncBundle(bundle: SpotlightSyncBundle) {
  const failures: SpotlightFailure[] = [];
  const lifecycle = rec(bundle.lifecycle);
  const entryColor = rec(bundle.entryColor);
  const exitColor = rec(bundle.exitColor);
  if (!lifecycle || lifecycle.schemaVersion !== 2) {
    observer(failures, "identity", "lifecycle.schemaVersion", lifecycle?.schemaVersion ?? null, 2,
      "lifecycle receipt is missing or has the wrong schema");
  }
  if (!entryColor || entryColor.schemaVersion !== 2) {
    observer(failures, "identity", "entryColor.schemaVersion", entryColor?.schemaVersion ?? null, 2,
      "entry color receipt is missing or has the wrong schema");
  }
  if (!exitColor || exitColor.schemaVersion !== 2) {
    observer(failures, "identity", "exitColor.schemaVersion", exitColor?.schemaVersion ?? null, 2,
      "exit color receipt is missing or has the wrong schema");
  }
  const empty = { entry: [], exit: [], settled: [], edgeFlush: {
    footerWidth: null, leftCapsuleId: null, rightCapsuleId: null,
    leftInsetPxAt1x: null, rightInsetPxAt1x: null,
  } };
  const emptyCoverage = { entryGeometryFrameCount: 0, entryMotionColorFrameCount: 0,
    entrySettledColorFrameCount: 0, exitGeometryFrameCount: 0, exitMotionColorFrameCount: 0,
    exitComparableColorFrameCount: 0, capsuleIds: [], entryDistinctWidthCount: 0,
    entryVisibleDurationMs: null, entryCompressionDeadlineMs: null, entryReboundDurationMs: null,
    edgeFlushMeasured: false };
  if (!lifecycle || !entryColor || !exitColor) return finish(failures, emptyCoverage, empty);

  if (entryColor.lifecyclePhase !== "entry") observer(failures, "identity", "entryColor.lifecyclePhase",
    entryColor.lifecyclePhase ?? null, "entry", "entry color receipt is bound to the wrong lifecycle phase");
  if (exitColor.lifecyclePhase !== "exit") observer(failures, "identity", "exitColor.lifecyclePhase",
    exitColor.lifecyclePhase ?? null, "exit", "exit color receipt is bound to the wrong lifecycle phase");
  if (bundle.lifecycleReceiptSha256) {
    for (const [label, receipt] of [["entry", entryColor], ["exit", exitColor]] as const) {
      const bound = get(receipt, "layoutSource", "lifecycleReceiptSha256");
      if (bound !== bundle.lifecycleReceiptSha256) observer(failures, "identity",
        `${label}Color.lifecycleReceiptSha256`, bound ?? null, bundle.lifecycleReceiptSha256,
        `${label} color receipt is not bound to the exact lifecycle receipt`);
    }
  }
  const interference = rec(lifecycle.interference);
  if (interference?.pass !== true || interference?.disposition === "INVALID_INTERFERENCE") {
    observer(failures, "interference", "interference.disposition",
      interference?.disposition ?? interference?.pass ?? null, "EVALUABLE_PASS",
      "untagged input or environmental interference invalidated the run");
  }
  if (lifecycle.cleanedUp !== true) observer(failures, "capture", "lifecycle.cleanedUp",
    lifecycle.cleanedUp ?? null, true, "probe-owned app process was not proven closed");
  if (lifecycle.error != null) observer(failures, "capture", "lifecycle.error", lifecycle.error, null,
    "lifecycle observer reported an execution error");
  for (const [label, receipt] of [["entry", entryColor], ["exit", exitColor]] as const) {
    if (arr(receipt.errors).length) observer(failures, "capture", `${label}Color.errors`, receipt.errors, [],
      `${label} color analyzer reported incomplete or invalid evidence`);
  }

  const entry = scenario(lifecycle, "main-entry");
  const exit = scenario(lifecycle, "main-exit");
  if (!entry) observer(failures, "capture", "entry.scenario", null, "main-entry",
    "required main-entry lifecycle scenario is missing");
  if (!exit) observer(failures, "capture", "exit.scenario", null, "main-exit",
    "required main-exit lifecycle scenario is missing");
  if (!entry || !exit) return finish(failures, emptyCoverage, empty);

  validateCapture(failures, entry, "entry");
  validateCapture(failures, exit, "exit");
  validateOwner(failures, entry, "entry");
  validateOwner(failures, exit, "exit");
  if (num(entry.exactWindowID) !== null && num(exit.exactWindowID) !== null
    && num(entry.exactWindowID) !== num(exit.exactWindowID)) {
    observer(failures, "identity", "lifecycle.mainOwnerContinuity",
      { entry: entry.exactWindowID, exit: exit.exactWindowID }, "same CGWindowID",
      "entry and exit did not observe the same physical main window");
  }
  if (entry.settledCapturesPass !== true) observer(failures, "settled", "entry.settledCapturesPass",
    entry.settledCapturesPass ?? null, true, "three explicit settled captures were not valid");
  if (exit.hiddenReferencePass !== true) observer(failures, "exit", "exit.hiddenReferencePass",
    exit.hiddenReferencePass ?? null, true, "exit did not produce a valid explicit post-hide background reference");

  const entryGeometry = gradeEntryGeometry(failures, entry);
  const exitGeometry = gradeExitGeometry(failures, exit);
  const settledBounds = entryGeometry.bounds ?? exitGeometry.bounds;
  const entryFrames = sortedFrames(entryColor.frames);
  const exitFrames = sortedFrames(exitColor.frames);
  const entryMotion = entryFrames.filter((frame) => frame.phase === "motion");
  const settled = entryFrames.filter((frame) => frame.phase === "settled");
  const exitMotion = exitFrames.filter((frame) => frame.phase === "motion");
  validateEntryAlphaPolicy(failures, entryColor);
  const ids = capsuleIds(settled);
  if (ids.length < 2) observer(failures, "settled", "settled.capsuleCount", ids.length, ">= 2",
    "settled evidence does not cover both floating footer capsule lanes");
  const references = settledReferences(settled, ids);
  for (const id of ids) {
    if (!references.has(id)) observer(failures, "settled", "settled.capsule.relationReference", null,
      "median stage ΔE00 and L* relation", "capsule lacks a measurable settled relation to the main glass", null, id);
  }
  validateSteady(failures, settled, ids);
  validateColorCoverage(failures, entryMotion, ids, "entry");
  // Settled entry frames carry the STRICT lower-tail boundary gates
  // (p10/fraction) — the only frames where seam sampling cannot excuse a
  // weak rim (see validateColorCoverage).
  validateColorCoverage(failures, settled, ids, "entry");
  validateColorCoverage(failures, exitMotion, ids, "exit");
  validateRelation(failures, entryMotion, ids, references, "entry");
  const exitComparable = validateRelation(failures, exitMotion, ids, references, "exit");
  const edge = edgeFlush(failures, entry);
  return finish(failures, {
    entryGeometryFrameCount: entryGeometry.frames.length,
    entryMotionColorFrameCount: entryMotion.length,
    entrySettledColorFrameCount: settled.length,
    exitGeometryFrameCount: exitGeometry.frames.length,
    exitMotionColorFrameCount: exitMotion.length,
    exitComparableColorFrameCount: exitComparable,
    capsuleIds: ids,
    entryDistinctWidthCount: entryGeometry.distinct,
    entryVisibleDurationMs: entryGeometry.total,
    entryCompressionDeadlineMs: entryGeometry.extremeAt,
    entryReboundDurationMs: entryGeometry.rebound,
    edgeFlushMeasured: edge.leftInsetPxAt1x !== null && edge.rightInsetPxAt1x !== null,
  }, {
    entry: measurements(entryMotion, "entry", settledBounds, references),
    exit: measurements(exitMotion, "exit", exitGeometry.bounds ?? settledBounds, references),
    settled: measurements(settled, "settled", settledBounds, references),
    edgeFlush: edge,
  });
}

export function exitCodeForSpotlightDisposition(disposition: string): number {
  return disposition === "EVALUABLE_PASS" ? 0 : disposition === "EVALUABLE_FAIL" ? 2 : 4;
}
