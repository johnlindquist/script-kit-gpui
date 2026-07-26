// ---------------------------------------------------------------------------
// Scenario plan, timing, and analysis-mode contracts
// (Oracle plan glass-smoke-harness-max-info, work package 3.)
// ---------------------------------------------------------------------------

/** The exact legacy capture order. The default profile MUST preserve it. */
export const LEGACY_FULL_SCENARIO_ORDER = [
  "main-exit",
  "main-entry",
  "notes-entry",
  "notes-close-before-settle-reopen",
  "dictation-exit-reopen",
] as const;

export type LifecycleScenarioName = (typeof LEGACY_FULL_SCENARIO_ORDER)[number];

export type ScenarioProfile = "full" | "entry-color" | "extended";

/**
 * Locked legacy capture durations (ms) per scenario. These are observation
 * windows, not animation values, but they are still part of the calibrated
 * capture contract: changing one changes what the filmstrips can see.
 */
export const SCENARIO_CAPTURE_DURATIONS_MS: Record<
  LifecycleScenarioName,
  number
> = {
  "main-exit": 200,
  "main-entry": 700,
  "notes-entry": 800,
  "notes-close-before-settle-reopen": 950,
  "dictation-exit-reopen": 900,
};

/**
 * Resolve a capture profile to its ordered scenario list.
 *
 * - `full` (default) is the exact legacy five in the exact legacy order.
 * - `entry-color` is the minimal set the displayed-color metric needs:
 *   main-entry ALWAYS pulls in main-exit because the metric requires the
 *   explicit post-exit hidden background reference.
 * - `extended` is the legacy five (an optional embedded Actions scenario may
 *   append later; it never reorders or drops the legacy prefix).
 */
export function resolveScenarioNames(
  profile: ScenarioProfile,
): LifecycleScenarioName[] {
  switch (profile) {
    case "full":
    case "extended":
      return [...LEGACY_FULL_SCENARIO_ORDER];
    case "entry-color":
      return ["main-exit", "main-entry"];
    default: {
      const exhaustive: never = profile;
      throw new Error(`unknown scenario profile: ${String(exhaustive)}`);
    }
  }
}

export type AnalysisMode = "inline" | "deferred";

export function parseAnalysisMode(value: string | undefined): AnalysisMode {
  if (value == null || value === "inline") return "inline";
  if (value === "deferred") return "deferred";
  throw new Error(`unknown analysis mode: ${value}`);
}

export type FilmstripVerdictInput = {
  captureErrorCount: number;
  analysisMode: AnalysisMode;
  metricsExitCode: number | null;
  metricsPass: boolean | null;
};

export type FilmstripVerdict = {
  capturePass: boolean;
  analysisState: "inline" | "pending";
  pass: boolean;
};

/**
 * A filmstrip's verdict separates CAPTURE validity from METRIC analysis.
 * In deferred mode the Python graders never run while the display scenario
 * is active, so no filmstrip (and therefore no receipt) can pass inline —
 * analysisState stays "pending" and pass is unconditionally false. In inline
 * mode a missing or failed metric analysis remains red exactly as before.
 */
export function filmstripVerdict(
  input: FilmstripVerdictInput,
): FilmstripVerdict {
  const capturePass = input.captureErrorCount === 0;
  if (input.analysisMode === "deferred") {
    return { capturePass, analysisState: "pending", pass: false };
  }
  return {
    capturePass,
    analysisState: "inline",
    pass: capturePass
      && input.metricsExitCode === 0
      && input.metricsPass === true,
  };
}

export type LifecycleDispositionInput = {
  interferenceDisposition: string | null;
  analysisState: "inline" | "pending";
  pass: boolean;
  hasObserverError: boolean;
};

/**
 * Terminal disposition for a lifecycle run receipt. ANALYSIS_PENDING can
 * never be green: it is a capture-only receipt awaiting the offline grader,
 * which writes the final standard receipt later. Interference invalidity
 * always dominates — an interfered capture is not evidence in any mode.
 */
export function computeLifecycleDisposition(
  input: LifecycleDispositionInput,
): string {
  if (input.interferenceDisposition === "INVALID_INTERFERENCE") {
    return "INVALID_INTERFERENCE";
  }
  if (input.analysisState === "pending") return "ANALYSIS_PENDING";
  if (input.pass === true) return "EVALUABLE_PASS";
  if (input.hasObserverError) return "INVALID_OBSERVER";
  return "EVALUABLE_FAIL";
}

export type ScenarioTimingInterval = {
  name: string;
  startedAtMs: number;
  finishedAtMs: number;
};

/**
 * Scenario timing intervals must be monotone and non-overlapping: scenario
 * N+1 may not begin before scenario N finished, and no interval may run
 * backwards. A refactor that reorders or interleaves scenario work would
 * surface here before it silently changed what the captures measure.
 */
export function validateScenarioTimingIntervals(
  intervals: ScenarioTimingInterval[],
): string[] {
  const errors: string[] = [];
  for (const [index, interval] of intervals.entries()) {
    if (!(interval.finishedAtMs >= interval.startedAtMs)) {
      errors.push(`${interval.name}: interval runs backwards`);
    }
    const previous = intervals[index - 1];
    if (previous && interval.startedAtMs < previous.finishedAtMs) {
      errors.push(
        `${interval.name}: started before ${previous.name} finished`,
      );
    }
  }
  return errors;
}

export type FilmstripIdentity = {
  runId: string;
  gitCommit: string;
  binarySha256: string;
  pid: number;
  windowId: number;
};

export type CaptureBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export function expandCaptureBounds(
  bounds: CaptureBounds,
  scale = 1.08,
): CaptureBounds {
  if (
    !Number.isFinite(scale)
    || scale < 1
    || ![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite)
    || bounds.width <= 0
    || bounds.height <= 0
  ) {
    throw new Error("capture bounds and expansion scale must be finite and positive");
  }
  const width = Math.ceil(bounds.width * scale);
  const height = Math.ceil(bounds.height * scale);
  return {
    x: Math.floor(bounds.x - (width - bounds.width) / 2),
    y: Math.floor(bounds.y - (height - bounds.height) / 2),
    width,
    height,
  };
}

export function validateFilmstripCapture(
  receipt: any,
  expected: FilmstripIdentity,
): string[] {
  const errors: string[] = [];
  if (!receipt || typeof receipt !== "object") return ["filmstrip receipt missing"];
  if (receipt.schemaVersion !== 2) errors.push("filmstrip schemaVersion must be 2");
  if (receipt.status !== "ok") errors.push("filmstrip status must be ok");
  if (receipt.captureHealthPass !== true) {
    errors.push("filmstrip captureHealthPass must be true");
  }
  if (receipt.runID !== expected.runId) errors.push("filmstrip runId mismatch");
  if (receipt.gitCommit !== expected.gitCommit) errors.push("filmstrip gitCommit mismatch");
  if (receipt.binarySHA256 !== expected.binarySha256) {
    errors.push("filmstrip binary SHA-256 mismatch");
  }
  if (Number(receipt.pid) !== expected.pid) errors.push("filmstrip PID mismatch");
  if (Number(receipt.windowID) !== expected.windowId) {
    errors.push("filmstrip expected CGWindowID mismatch");
  }
  if (!Number.isFinite(Number(receipt.displayID))) errors.push("display ID missing");
  if (!(Number(receipt.refreshRateHz) > 0)) errors.push("refresh rate missing");
  if (!(Number(receipt.captureScale) > 0)) errors.push("capture scale missing");
  if (receipt.pixelFormat !== "BGRA") errors.push("pixel format must be BGRA");

  const received = Number(receipt.receivedSampleCount);
  const accounted = Number(receipt.accountedSampleCount);
  const complete = Number(receipt.completeSampleCount);
  const copied = Number(receipt.copiedCompleteCount);
  const encoded = Number(receipt.encodedCompleteCount);
  const incomplete = Number(receipt.incompleteSampleCount);
  const incompleteRenderable = Number(receipt.incompleteRenderableSampleCount);
  const missingDisplayTime = Number(receipt.missingDisplayTimeCount);
  const dropped = Number(receipt.droppedCompleteCount);
  const duplicates = Number(receipt.duplicateDisplayTimeCount);
  const late = Number(receipt.lateFrameCount);
  if (![received, accounted, complete, copied, encoded, incomplete,
    incompleteRenderable,
    missingDisplayTime, dropped, duplicates, late]
    .every(Number.isFinite)) {
    errors.push("capture accounting fields missing");
  } else {
    if (received !== accounted) errors.push("received sample accounting mismatch");
    if (accounted !== complete + incomplete) {
      errors.push("complete plus incomplete sample accounting mismatch");
    }
    if (missingDisplayTime !== 0) {
      errors.push("one or more samples lack display time");
    }
    if (incompleteRenderable !== 0) {
      errors.push("one or more renderable non-complete samples were not encoded");
    }
    if (complete !== copied) errors.push("copied complete count mismatch");
    if (copied !== encoded) errors.push("encoded complete count mismatch");
    if (dropped !== 0) errors.push("dropped complete count must be zero");
    if (duplicates !== 0) errors.push("duplicate display time observed");
    if (typeof receipt.screenDamageCadenceWithinOneDisplayPeriod !== "boolean") {
      errors.push("screen-damage cadence classification missing");
    }
  }
  if (
    !Number.isFinite(Number(receipt.maximumConsecutiveDisplayTimeGapNs))
    || !Number.isFinite(Number(receipt.maximumAllowedDisplayTimeGapNs))
  ) {
    errors.push("screen-damage cadence measurements missing");
  }
  const frames = Array.isArray(receipt.frames) ? receipt.frames : [];
  if (frames.length !== encoded) errors.push("encoded frame inventory mismatch");
  const hasOwnedFrame = frames.some(
    (frame: any) => Number(frame?.actualWindowID) === expected.windowId,
  );
  for (const [index, frame] of frames.entries()) {
    if (Number(frame?.expectedWindowID) !== expected.windowId) {
      errors.push(`frame ${index} expected CGWindowID mismatch`);
    }
    const absentPinnedWindow = hasOwnedFrame
      && frame?.actualWindowID == null
      && frame?.windowBounds == null;
    if (
      Number(frame?.actualWindowID) !== expected.windowId
      && !absentPinnedWindow
    ) {
      errors.push(`frame ${index} actual CGWindowID mismatch`);
    }
    if (!(Number(frame?.displayTimeNs) > 0)) {
      errors.push(`frame ${index} host display time missing`);
    }
    if (!/^[a-f0-9]{64}$/.test(String(frame?.sha256 ?? ""))) {
      errors.push(`frame ${index} SHA-256 missing`);
    }
  }
  return errors;
}

export function validateDetachedExitLifecycle(
  receipt: any,
  expectedWindowId: number,
  expectedState: "exiting" | "cancelled",
): string[] {
  const errors: string[] = [];
  if (receipt?.schemaVersion !== 2) errors.push("native exit schemaVersion must be 2");
  if (Number(receipt?.nativeWindowNumber) !== expectedWindowId) {
    errors.push("native exit CGWindowID mismatch");
  }
  if (receipt?.exitMode !== "DetachedRegionsFadeOnly") {
    errors.push("native exit mode must be DetachedRegionsFadeOnly");
  }
  if (expectedState === "exiting") {
    const original = receipt?.originalFrame;
    const current = receipt?.currentFrame;
    if (
      !Array.isArray(original)
      || !Array.isArray(current)
      || original.length !== 4
      || current.length !== 4
      || original.some((value: number, index: number) =>
        Math.abs(value - Number(current[index])) > 0.25
      )
    ) {
      errors.push("native exit frame moved by more than 0.5 device pixel");
    }
  }
  if (Number(receipt?.commonContentViewFilterCount) !== 0) {
    errors.push("common content-view filter must remain absent");
  }
  if (receipt?.glassHostAttached !== true) {
    errors.push("native glass host detached before current exit resolved");
  }
  const request = Number(receipt?.requestHostTimeNs);
  const deadline = Number(receipt?.expectedRemovalDeadlineNs);
  if (!Number.isFinite(request) || deadline - request !== 135_000_000) {
    errors.push("native exit removal deadline is not exactly 135ms");
  }
  const events = Array.isArray(receipt?.history) ? receipt.history : [];
  if (!events.some((event: any) => event?.event === "ticketBegin")) {
    errors.push("native exit ticket-begin event missing");
  }
  if (expectedState === "exiting") {
    if (receipt?.cancelledAtHostTimeNs != null) {
      errors.push("active native exit was already cancelled");
    }
    if (receipt?.committedAtHostTimeNs != null) {
      errors.push("active native exit committed before deadline");
    }
  } else {
    if (!Number.isFinite(Number(receipt?.cancelledAtHostTimeNs))) {
      errors.push("reopened native exit lacks cancellation time");
    }
    if (!events.some((event: any) => event?.event === "ticketCancel")) {
      errors.push("native exit ticket-cancel event missing");
    }
    if (Number(receipt?.currentAlpha) < 0.999) {
      errors.push("cancelled native exit did not restore alpha");
    }
    if (receipt?.committedAtHostTimeNs != null) {
      errors.push("cancelled native exit was incorrectly committed");
    }
  }
  return errors;
}
