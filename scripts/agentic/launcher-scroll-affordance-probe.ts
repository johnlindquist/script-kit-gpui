#!/usr/bin/env bun
/**
 * Runtime proof for the launcher's top-fade and phased boundary affordance.
 *
 * Build an immutable reference, then supply it explicitly:
 *   bun scripts/devtools/devtools.ts build-ops act app-build --artifact-out .test-output/launcher-scroll.reference.json
 *   SCRIPT_KIT_ARTIFACT_REFERENCE=.test-output/launcher-scroll.reference.json bun scripts/agentic/launcher-scroll-affordance-probe.ts
 * PROBE_BINARY/--binary, when supplied, must match that exact reference.
 */
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, relative, resolve } from "node:path";
import { Driver, type Json } from "../devtools/driver.ts";
import { runtimeArtifactFromEnvironment } from "../devtools/lib/runtime-task-proof.ts";
import { inspectMainListScrollAffordance } from "../devtools/scroll.ts";

const repoRoot = resolve(import.meta.dir, "../..");
const outputDir = join(repoRoot, ".test-output", "launcher-scroll-affordance");
const receiptPath = join(outputDir, "receipt.json");
const screenshotPath = join(outputDir, "middle-top-fade.png");
const sessionDir = join(
  "/tmp",
  `sk-launcher-scroll-affordance-${process.pid}-${Date.now().toString(36)}`,
);
const query = "launcher-scroll-affordance";
// A trailing sentinel lets the debounced root-search frame observe the full
// query even though launcher input handoff clears the visible field afterward.
const filterInput = `${query} `;
const target = { type: "main" };
const pollMs = 20;
const artifact = runtimeArtifactFromEnvironment();

function argValue(name: string, fallback: string): string {
  const index = process.argv.indexOf(name);
  return index >= 0 && process.argv[index + 1]
    ? process.argv[index + 1]
    : fallback;
}

const binary = resolve(
  repoRoot,
  argValue(
    "--binary",
    process.env.PROBE_BINARY
      ?? process.env.SCRIPT_KIT_GPUI_BINARY
      ?? artifact.executablePath,
  ),
);
const timeoutMs = Number(argValue("--timeout", "10000"));

type Check = {
  name: string;
  pass: boolean;
  observed?: Json | string | number | boolean | null;
};

const checks: Check[] = [];
const failures: string[] = [];
const scenarios: Json = {};
let driver: Driver | null = null;
let screenshotResult: Json = { classification: "not-attempted" };

function check(
  name: string,
  pass: boolean,
  observed?: Json | string | number | boolean | null,
): boolean {
  checks.push(observed === undefined ? { name, pass } : { name, pass, observed });
  if (!pass) failures.push(name);
  return pass;
}

function finite(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function affordanceOf(state: Json): Json {
  const scroll = state.mainListScroll;
  if (!scroll || typeof scroll !== "object") {
    throw new Error("getState omitted mainListScroll on ScriptList");
  }
  const inspection = inspectMainListScrollAffordance(scroll, true);
  if (!inspection.complete || inspection.affordance == null) {
    throw new Error(
      `mainListScroll.affordance incomplete: ${inspection.missingFields.join(", ")}`,
    );
  }
  return inspection.affordance as Json;
}

function traceSamplesOf(state: Json): Json[] {
  const samples = affordanceOf(state).traceSamples;
  if (!Array.isArray(samples)) {
    throw new Error("elastic traceSamples missing; diagnostic gate is not active");
  }
  return samples as Json[];
}

function compactState(state: Json): Json {
  const scroll = state.mainListScroll ?? {};
  const affordance = affordanceOf(state);
  return {
    inputValue: state.inputValue ?? null,
    surfaceKind: state.surfaceContract?.surfaceKind ?? null,
    visibleChoiceCount: state.visibleChoiceCount ?? null,
    rootFileQuery: state.rootFileSearch?.query ?? null,
    rootFileLoading: state.rootFileSearch?.loading ?? null,
    selectedIndex: scroll.selectedIndex ?? state.selectedIndex ?? null,
    itemCount: scroll.itemCount ?? state.visibleChoiceCount ?? null,
    scrollTop: scroll.scrollTop ?? null,
    scrollTopItem: scroll.scrollTopItem ?? null,
    scrollTopOffset: scroll.scrollTopOffset ?? null,
    maxScrollTop: scroll.maxScrollTop ?? null,
    atTop: affordance.atTop,
    atBottom: affordance.atBottom,
    topFadeActive: affordance.topFadeActive,
    topFadeProgress: affordance.topFadeProgress,
    topFadeAlpha: affordance.topFadeAlpha,
    overscrollOffsetPx: affordance.overscrollOffsetPx,
    overscrollMaxOffsetPx: affordance.overscrollMaxOffsetPx,
    overscrollEdge: affordance.overscrollEdge,
    overscrollPhase: affordance.overscrollPhase,
    generation: affordance.generation,
    lastTouchPhase: affordance.lastTouchPhase,
    lastSettleReason: affordance.lastSettleReason,
    directPhase: affordance.directPhase,
    momentumPhase: affordance.momentumPhase,
    nativeTimestampSeconds: affordance.nativeTimestampSeconds,
    momentumSuppressed: affordance.momentumSuppressed,
    rawPullPx: affordance.rawPullPx,
    visualVelocityPxPerSecond: affordance.visualVelocityPxPerSecond,
    reboundInitialOffsetPx: affordance.reboundInitialOffsetPx,
    reboundInitialVelocityPxPerSecond: affordance.reboundInitialVelocityPxPerSecond,
    reboundElapsedMs: affordance.reboundElapsedMs,
    reboundOmegaPerSecond: affordance.reboundOmegaPerSecond,
    frameGeneration: affordance.frameGeneration,
    traceSampleCount: Array.isArray(affordance.traceSamples)
      ? affordance.traceSamples.length
      : null,
    reducedMotion: affordance.reducedMotion,
  };
}

function safeCompactState(state: Json): Json {
  try {
    return compactState(state);
  } catch {
    return {
      promptType: state.promptType ?? null,
      surfaceKind: state.surfaceContract?.surfaceKind ?? null,
      inputValue: state.inputValue ?? null,
      hasMainListScroll: state.mainListScroll != null,
      hasAffordance: state.mainListScroll?.affordance != null,
    };
  }
}

function sameLogicalTuple(before: Json, after: Json): boolean {
  const a = before.mainListScroll ?? {};
  const b = after.mainListScroll ?? {};
  return a.selectedIndex === b.selectedIndex
    && a.scrollTopItem === b.scrollTopItem
    && a.scrollTopOffset === b.scrollTopOffset
    && a.scrollTop === b.scrollTop;
}

async function pollState(
  label: string,
  predicate: (state: Json) => boolean,
  deadlineMs = timeoutMs,
): Promise<Json> {
  const deadline = performance.now() + deadlineMs;
  let last: Json = {};
  while (performance.now() < deadline) {
    last = await driver!.getState({ timeoutMs: Math.min(deadlineMs, 5_000) });
    if (predicate(last)) return last;
    await Bun.sleep(pollMs);
  }
  throw new Error(`${label}: timed out; last=${JSON.stringify(safeCompactState(last))}`);
}

async function waitForQuietPopulation(label: string): Promise<Json> {
  const startedAt = performance.now();
  const deadline = startedAt + Math.max(timeoutMs, 8_000);
  let lastSignature = "";
  let unchangedSince = startedAt;
  let last: Json = {};
  while (performance.now() < deadline) {
    last = await driver!.getState({ timeoutMs: 5_000 });
    const scroll = last.mainListScroll ?? {};
    const signature = JSON.stringify({
      itemCount: scroll.itemCount,
      contentHeight: scroll.contentHeight,
      maxScrollTop: scroll.maxScrollTop,
      visibleChoiceCount: last.visibleChoiceCount,
      rootFileQuery: last.rootFileSearch?.query,
      rootFileLoading: last.rootFileSearch?.loading,
    });
    if (signature !== lastSignature) {
      lastSignature = signature;
      unchangedSince = performance.now();
    }
    if (performance.now() - startedAt >= 3_000
      && performance.now() - unchangedSince >= 800
      && last.rootFileSearch?.query === query
      && last.rootFileSearch?.loading === false
      && Number(scroll.maxScrollTop) > 0) {
      return last;
    }
    await Bun.sleep(50);
  }
  throw new Error(`${label}: population did not settle; last=${JSON.stringify(safeCompactState(last))}`);
}

async function gpuiKey(key: string): Promise<Json> {
  const response = await driver!.simulateGpuiEvent(
    { type: "keyDown", key, modifiers: [] },
    { target, timeoutMs: 5_000 },
  );
  if (response.success !== true) {
    throw new Error(`${key} dispatch failed: ${JSON.stringify(response)}`);
  }
  return response;
}

async function wheel(
  point: { x: number; y: number },
  phase: "started" | "moved" | "ended",
  deltaY: number,
  lifecycle: {
    directPhase?: "none" | "mayBegin" | "began" | "changed" | "stationary" | "ended" | "cancelled";
    momentumPhase?: "none" | "mayBegin" | "began" | "changed" | "stationary" | "ended" | "cancelled";
    timestampSeconds?: number;
  } = {},
): Promise<Json> {
  const response = await driver!.simulateGpuiScrollWheel(
    { x: point.x, y: point.y, deltaX: 0, deltaY, phase, ...lifecycle },
    { target, timeoutMs: 5_000 },
  );
  if (response.success !== true) {
    throw new Error(
      `scrollWheel ${phase} dispatch failed: ${JSON.stringify(response)}`,
    );
  }
  return response;
}

async function selectableEndpoint(): Promise<Json> {
  const result = await driver!.getElements(
    { target, limit: 2_000, includeHeaders: true },
    { timeoutMs: 5_000 },
  );
  const rows = (Array.isArray(result.elements) ? result.elements : [])
    .filter((element: Json) => element.role === "row" && element.selectable === true)
    .sort((a: Json, b: Json) => Number(a.index) - Number(b.index));
  const selected = rows.find((row: Json) => row.selected === true) ?? null;
  return {
    count: rows.length,
    firstIndex: rows[0]?.index ?? null,
    lastIndex: rows.at(-1)?.index ?? null,
    selectedIndex: selected?.index ?? null,
    selectedSemanticId: selected?.semanticId ?? null,
  };
}

async function establishTop(): Promise<{ state: Json; endpoint: Json }> {
  await gpuiKey("home");
  const atTop = (candidate: Json) => {
    try {
      const affordance = affordanceOf(candidate);
      const scroll = candidate.mainListScroll ?? {};
      return Number(scroll.itemCount) >= 12
        && Number(scroll.maxScrollTop) > 0
        && affordance.atTop === true
        && affordance.overscrollPhase === "idle"
        && Number(affordance.overscrollOffsetPx) === 0;
    } catch {
      return false;
    }
  };
  let state: Json;
  try {
    state = await pollState("top endpoint", atTop, 1_200);
  } catch {
    // Home selects the first row but, after a deep list, selection reveal may
    // leave a leading section header just above the viewport. Replacing the
    // deterministic filter is the public reset seam that restores true zero.
    await driver!.setFilterAndWait("", { timeoutMs });
    await driver!.setFilterAndWait(filterInput, { timeoutMs });
    state = await pollState("top endpoint after filter reset", atTop);
  }
  await gpuiKey("up");
  const confirmed = await pollState("top endpoint confirmation", (candidate) => {
    try {
      return affordanceOf(candidate).atTop === true;
    } catch {
      return false;
    }
  });
  const endpoint = await selectableEndpoint();
  check("top-selection-is-first-selectable", endpoint.selectedIndex === endpoint.firstIndex, endpoint);
  check("top-endpoint-confirmation-does-not-move", sameLogicalTuple(state, confirmed), {
    before: compactState(state),
    after: compactState(confirmed),
  });
  return { state: confirmed, endpoint };
}

async function establishBottom(): Promise<{ state: Json; endpoint: Json }> {
  let state: Json = {};
  let confirmed: Json = {};
  let endpoint: Json = {};
  let quietEndpoint = false;
  for (let attempt = 0; attempt < 32; attempt += 1) {
    await gpuiKey("end");
    state = await pollState("bottom endpoint", (candidate) => {
      try {
        const affordance = affordanceOf(candidate);
        return affordance.atBottom === true
          && affordance.overscrollPhase === "idle";
      } catch {
        return false;
      }
    });
    await gpuiKey("down");
    confirmed = await pollState("bottom endpoint confirmation", (candidate) => {
      try {
        return affordanceOf(candidate).atBottom === true;
      } catch {
        return false;
      }
    });

    // End can deliberately trigger lazy source hydration/pagination. Require
    // a quiet endpoint before testing rebound so a legitimate list replacement
    // cannot masquerade as a boundary-state failure.
    quietEndpoint = true;
    let quietState = confirmed;
    for (let sample = 0; sample < 8; sample += 1) {
      await Bun.sleep(100);
      const candidate = await driver!.getState();
      const a = quietState.mainListScroll ?? {};
      const b = candidate.mainListScroll ?? {};
      let atBottom = false;
      try {
        atBottom = affordanceOf(candidate).atBottom === true
          && affordanceOf(candidate).overscrollPhase === "idle";
      } catch {
        atBottom = false;
      }
      if (!atBottom
        || a.itemCount !== b.itemCount
        || a.maxScrollTop !== b.maxScrollTop
        || !sameLogicalTuple(quietState, candidate)) {
        quietEndpoint = false;
        break;
      }
      quietState = candidate;
    }
    confirmed = quietState;
    endpoint = await selectableEndpoint();
    if (quietEndpoint
      && endpoint.selectedIndex === endpoint.lastIndex
      && Number(confirmed.mainListScroll?.selectedIndex)
        === Number(confirmed.mainListScroll?.itemCount) - 1) {
      break;
    }
  }
  check("bottom-selection-is-last-selectable", quietEndpoint
    && endpoint.selectedIndex === endpoint.lastIndex
    && Number(confirmed.mainListScroll?.selectedIndex)
      === Number(confirmed.mainListScroll?.itemCount) - 1, {
    endpoint,
    state: safeCompactState(confirmed),
  });
  check("bottom-endpoint-confirmation-does-not-move", sameLogicalTuple(state, confirmed), {
    before: compactState(state),
    after: compactState(confirmed),
  });
  return { state: confirmed, endpoint };
}

async function settleAfterEnded(
  point: { x: number; y: number },
  timestampSeconds?: number,
): Promise<Json> {
  await wheel(
    point,
    timestampSeconds == null ? "ended" : "moved",
    0,
    timestampSeconds == null
      ? {}
      : { directPhase: "ended", momentumPhase: "none", timestampSeconds },
  );
  const released = await driver!.getState();
  const releasedAffordance = affordanceOf(released);
  check(
    "direct-terminal-phase-begins-rebound-in-same-dispatch",
    releasedAffordance.reducedMotion === true
      ? releasedAffordance.overscrollPhase === "idle"
      : releasedAffordance.overscrollPhase === "settling",
    compactState(released),
  );

  if (timestampSeconds != null) {
    await wheel(point, "moved", 8, {
      directPhase: "none",
      momentumPhase: "changed",
      timestampSeconds: timestampSeconds + 0.008,
    });
    const afterMomentum = await driver!.getState();
    check(
      "momentum-cannot-reenter-direct-tracking-after-release",
      affordanceOf(afterMomentum).overscrollPhase !== "tracking",
      compactState(afterMomentum),
    );
  }
  return pollState("ended settle", (state) => {
    try {
      const affordance = affordanceOf(state);
      const expectedReason = affordance.reducedMotion === true ? "reducedMotion" : "ended";
      return affordance.overscrollPhase === "idle"
        && affordance.overscrollEdge == null
        && Number(affordance.overscrollOffsetPx) === 0
        && affordance.lastSettleReason === expectedReason;
    } catch {
      return false;
    }
  }, 3_000);
}

async function exerciseTerminalLifecycleScenarios(
  label: "top" | "bottom",
  point: { x: number; y: number },
  establish: () => Promise<{ state: Json; endpoint: Json }>,
  outwardDeltaY: number,
): Promise<void> {
  const baseTimestamp = label === "top" ? 300 : 400;

  const cancelledBaseline = await establish();
  await wheel(point, "moved", 0, {
    directPhase: "began", momentumPhase: "none", timestampSeconds: baseTimestamp,
  });
  await wheel(point, "moved", outwardDeltaY, {
    directPhase: "changed", momentumPhase: "none", timestampSeconds: baseTimestamp + 0.008,
  });
  await wheel(point, "moved", 0, {
    directPhase: "cancelled", momentumPhase: "none", timestampSeconds: baseTimestamp + 0.012,
  });
  const cancelled = await driver!.getState();
  const cancelledAffordance = affordanceOf(cancelled);
  check(`${label}-cancelled-starts-zero-velocity-rebound-immediately`,
    cancelledAffordance.overscrollPhase === "settling"
      && cancelledAffordance.lastSettleReason === "cancelled"
      && Number(cancelledAffordance.reboundInitialVelocityPxPerSecond) === 0,
    compactState(cancelled));
  const cancelledSettled = await pollState(`${label} cancelled settle`, (state) =>
    affordanceOf(state).overscrollPhase === "idle"
      && Number(affordanceOf(state).overscrollOffsetPx) === 0);
  check(`${label}-cancelled-preserves-logical-scroll-and-selection`,
    sameLogicalTuple(cancelledBaseline.state, cancelledSettled), {
      before: compactState(cancelledBaseline.state),
      after: compactState(cancelledSettled),
    });

  const implicitBaseline = await establish();
  await wheel(point, "moved", 0, {
    directPhase: "began", momentumPhase: "none", timestampSeconds: baseTimestamp + 1,
  });
  await wheel(point, "moved", outwardDeltaY, {
    directPhase: "changed", momentumPhase: "none", timestampSeconds: baseTimestamp + 1.008,
  });
  await wheel(point, "moved", 0, {
    directPhase: "none", momentumPhase: "began", timestampSeconds: baseTimestamp + 1.012,
  });
  const implicit = await driver!.getState();
  const implicitAffordance = affordanceOf(implicit);
  check(`${label}-momentum-began-provides-immediate-implicit-release`,
    implicitAffordance.overscrollPhase === "settling"
      && implicitAffordance.lastSettleReason === "momentumBeganImplicitRelease"
      && implicitAffordance.momentumSuppressed === true,
    compactState(implicit));
  await wheel(point, "moved", 0, {
    directPhase: "none", momentumPhase: "ended", timestampSeconds: baseTimestamp + 1.020,
  });
  const implicitSettled = await pollState(`${label} implicit release settle`, (state) => {
    const affordance = affordanceOf(state);
    return affordance.overscrollPhase === "idle"
      && affordance.momentumSuppressed === false
      && Number(affordance.overscrollOffsetPx) === 0;
  });
  check(`${label}-momentum-terminal-clears-suppression`, true, compactState(implicitSettled));
  check(`${label}-implicit-release-preserves-logical-scroll-and-selection`,
    sameLogicalTuple(implicitBaseline.state, implicitSettled), {
      before: compactState(implicitBaseline.state),
      after: compactState(implicitSettled),
    });

  await establish();
  await wheel(point, "moved", 0, {
    directPhase: "began", momentumPhase: "none", timestampSeconds: baseTimestamp + 2,
  });
  await wheel(point, "moved", outwardDeltaY, {
    directPhase: "changed", momentumPhase: "none", timestampSeconds: baseTimestamp + 2.008,
  });
  await wheel(point, "moved", 0, {
    directPhase: "ended", momentumPhase: "none", timestampSeconds: baseTimestamp + 2.012,
  });
  const released = await driver!.getState();
  const beforeInterrupt = await driver!.getState();
  await wheel(point, "moved", 0, {
    directPhase: "mayBegin", momentumPhase: "none", timestampSeconds: baseTimestamp + 2.016,
  });
  const interrupted = await driver!.getState();
  const interruptTrace = Array.isArray(affordanceOf(interrupted).traceSamples)
    ? affordanceOf(interrupted).traceSamples as Json[]
    : [];
  const mayBeginIndex = interruptTrace.findLastIndex((sample: Json) =>
    sample.kind === "input" && sample.directPhase === "mayBegin");
  const preMayBeginSample = mayBeginIndex > 0 ? interruptTrace[mayBeginIndex - 1] : null;
  const mayBeginSample = mayBeginIndex >= 0 ? interruptTrace[mayBeginIndex] : null;
  const interruptionJumpPx = preMayBeginSample && mayBeginSample
    ? Math.abs(Number(mayBeginSample.offsetPx) - Number(preMayBeginSample.offsetPx))
    : Number.POSITIVE_INFINITY;
  check(`${label}-new-direct-may-begin-interrupts-rebound-without-jump`,
    affordanceOf(interrupted).overscrollPhase === "tracking"
      && interruptionJumpPx <= 1,
    {
      released: compactState(released),
      beforeInterrupt: compactState(beforeInterrupt),
      interrupted: compactState(interrupted),
      preMayBeginSample,
      mayBeginSample,
      interruptionJumpPx,
    });
  await wheel(point, "moved", 0, {
    directPhase: "cancelled", momentumPhase: "none", timestampSeconds: baseTimestamp + 2.020,
  });
  await pollState(`${label} interrupted cleanup`, (state) =>
    affordanceOf(state).overscrollPhase === "idle");
}

function evaluateReboundTrace(
  label: string,
  settledState: Json,
  releaseGeneration: number,
  maxOffsetPx: number,
): Json {
  const samples = traceSamplesOf(settledState);
  const momentumRegrabs = samples.filter((sample) =>
    sample.kind === "input"
    && sample.momentumPhase !== "none"
    && sample.boundaryPhase === "tracking");
  check(`${label}-edge-rebound-records-zero-momentum-regrabs`,
    momentumRegrabs.length === 0, momentumRegrabs);
  const release = samples.find((sample) =>
    sample.kind === "input"
    && Number(sample.generation) === releaseGeneration
    && sample.boundaryPhase === "settling"
  );
  const frames = samples.filter((sample) =>
    sample.kind === "reboundFrame"
    && Number(sample.generation) === releaseGeneration
  );
  if (!release || frames.length === 0) {
    check(`${label}-trace-has-release-and-frame-samples`, false, {
      release: release ?? null,
      frameCount: frames.length,
    });
    return { release: release ?? null, frames };
  }

  const releaseAt = Number(release.arrivalElapsedMs);
  const releaseOffset = Math.abs(Number(release.offsetPx));
  const elapsed = (sample: Json) => Number(sample.arrivalElapsedMs) - releaseAt;
  const firstFrameMs = elapsed(frames[0]);
  const half = frames.find((sample) => Math.abs(Number(sample.offsetPx)) <= releaseOffset * 0.5);
  const tenPercent = frames.find((sample) =>
    Math.abs(Number(sample.offsetPx)) <= releaseOffset * 0.1
  );
  const exact = frames.find((sample) => Number(sample.offsetPx) === 0);
  const peak = Math.max(releaseOffset, ...frames.map((sample) => Math.abs(Number(sample.offsetPx))));
  const wrongSign = Math.max(0, ...frames.map((sample) =>
    Math.sign(Number(sample.offsetPx)) === -Math.sign(Number(release.offsetPx))
      ? Math.abs(Number(sample.offsetPx))
      : 0
  ));
  const plateau = frames.slice(2).some((sample, index) => {
    const previous = frames[index + 1];
    const beforePrevious = frames[index];
    return Math.abs(Number(sample.offsetPx)) > 2
      && Math.abs(Number(sample.offsetPx) - Number(previous.offsetPx)) < 0.25
      && Math.abs(Number(previous.offsetPx) - Number(beforePrevious.offsetPx)) < 0.25;
  });

  check(`${label}-first-rebound-change-is-next-frame`, firstFrameMs >= 0 && firstFrameMs <= 34, {
    firstFrameMs,
  });
  check(`${label}-trajectory-stays-within-cap`, peak <= maxOffsetPx + 0.25, { peak, maxOffsetPx });
  check(`${label}-outward-release-peak-is-bounded`, peak <= releaseOffset * 1.22 + 0.25, {
    peak,
    releaseOffset,
  });
  check(`${label}-wrong-sign-overshoot-is-bounded`, wrongSign <= 0.5, { wrongSign });
  check(`${label}-rebound-half-point-is-timely`, half != null
    && elapsed(half) >= 45 && elapsed(half) <= 100, half ? { elapsedMs: elapsed(half) } : null);
  check(`${label}-rebound-ten-percent-point-is-timely`, tenPercent != null
    && elapsed(tenPercent) <= 200, tenPercent ? { elapsedMs: elapsed(tenPercent) } : null);
  check(`${label}-rebound-finishes-exactly-by-deadline`, exact != null
    && elapsed(exact) <= 320, exact ? { elapsedMs: elapsed(exact) } : null);
  check(`${label}-rebound-has-no-visible-plateau`, !plateau, { plateau });

  return {
    release,
    frames,
    metrics: {
      firstFrameMs,
      halfMs: half ? elapsed(half) : null,
      tenPercentMs: tenPercent ? elapsed(tenPercent) : null,
      exactMs: exact ? elapsed(exact) : null,
      peak,
      wrongSign,
      plateau,
    },
  };
}

function screenshotBlocker(error: string): boolean {
  return /screen recording|screen & system audio|permission (?:is )?not granted|blank\/black image/i.test(error);
}

async function captureScreenshot(): Promise<Json> {
  rmSync(screenshotPath, { force: true });
  let response: Json = {};
  try {
    response = await driver!.captureScreenshot({
      target,
      hiDpi: true,
      savePath: screenshotPath,
      timeoutMs: 10_000,
    });
  } catch (error) {
    response = { error: error instanceof Error ? error.message : String(error) };
  }
  const error = typeof response.error === "string" ? response.error : "";
  const bytes = existsSync(screenshotPath) ? statSync(screenshotPath).size : 0;
  const captured = typeof response.data === "string"
    && response.data.length > 0
    && error.length === 0
    && bytes > 0;
  if (captured) {
    return {
      classification: "captured-unvalidated",
      path: relative(repoRoot, screenshotPath),
      bytes,
      width: response.width ?? null,
      height: response.height ?? null,
    };
  }
  const classification = screenshotBlocker(error)
    ? "blocked-by-screen-recording-permission"
    : "error";
  if (classification === "error") {
    check("screenshot-attempt-completed-or-honestly-permission-blocked", false, {
      error: error || "capture returned no PNG data",
    });
  }
  return {
    classification,
    path: null,
    bytes,
    error: error || "capture returned no PNG data",
  };
}

async function runProbe(): Promise<Json> {
  if (!existsSync(binary)) {
    throw new Error(
      `Published probe executable is missing: ${binary}. Rebuild and supply SCRIPT_KIT_ARTIFACT_REFERENCE.`,
    );
  }

  const provider = {
    query,
    delayMs: 0,
    results: Array.from({ length: 80 }, (_, index) => {
      const id = String(index + 1).padStart(3, "0");
      return {
        path: `/tmp/${query}-${id}.txt`,
        name: `Affordance fixture ${id}.txt`,
        fileType: "document",
        size: 2_048 + index,
        modified: 1_700_000_000 - index,
      };
    }),
  };

  driver = await Driver.launch({
    binary,
    immutableArtifact: artifact.reference,
    sessionName: "launcher-scroll-affordance",
    sessionDir,
    sandboxHome: true,
    sharedModels: false,
    readyTimeoutMs: 30_000,
    defaultTimeoutMs: 8_000,
    env: {
      RUST_BACKTRACE: "1",
      SCRIPT_KIT_MAIN_LIST_ELASTIC_TRACE: "1",
      SCRIPT_KIT_PANEL_INVARIANTS_ALLOW_MISMATCH: "1",
      SCRIPT_KIT_ROOT_FILE_SEARCH_TEST_PROVIDER: JSON.stringify(provider),
    },
  });

  driver.send({ type: "show" });
  await driver.waitForState({ windowVisible: true, windowFocused: true }, { timeoutMs });
  await driver.setFilterAndWait(filterInput, { timeoutMs });
  const ready = await pollState("deterministic long ScriptList", (state) => {
    try {
      const scroll = state.mainListScroll ?? {};
      const rootSearch = state.rootFileSearch ?? {};
      const inspection = inspectMainListScrollAffordance(scroll, true);
      return state.surfaceContract?.surfaceKind === "ScriptList"
        && rootSearch.query === query
        && rootSearch.loading === false
        // Headers/status rows count toward the scrollable list but not
        // visibleChoiceCount. Readiness is proven by the actual list geometry.
        && Number(scroll.itemCount) >= 12
        && Number(state.visibleChoiceCount) > 0
        && Number(scroll.maxScrollTop) > 0
        && inspection.complete;
    } catch {
      return false;
    }
  });
  const settledReady = await waitForQuietPopulation("deterministic long ScriptList");
  check("deterministic-long-script-list-ready", true, {
    initialVisibleChoiceCount: ready.visibleChoiceCount,
    settledVisibleChoiceCount: settledReady.visibleChoiceCount,
    itemCount: settledReady.mainListScroll?.itemCount,
    maxScrollTop: settledReady.mainListScroll?.maxScrollTop,
    rootFileQuery: settledReady.rootFileSearch?.query,
  });

  const layout = await driver.getLayoutInfo({ target }, { timeoutMs: 8_000 });
  const scriptList = (Array.isArray(layout.components) ? layout.components : [])
    .find((component: Json) => component.name === "ScriptList");
  const bounds = scriptList?.bounds;
  if (!bounds
    || finite(bounds.x) == null
    || finite(bounds.y) == null
    || finite(bounds.width) == null
    || finite(bounds.height) == null
    || Number(bounds.width) <= 0
    || Number(bounds.height) <= 0) {
    throw new Error(`getLayoutInfo omitted measured ScriptList bounds: ${JSON.stringify(scriptList)}`);
  }
  const point = {
    x: Number(bounds.x) + Number(bounds.width) / 2,
    y: Number(bounds.y) + Number(bounds.height) / 2,
  };
  check("wheel-point-derived-from-script-list-layout", true, { bounds, point });

  // Missing Ended runs first, before unrelated launcher sources can hydrate,
  // so the watchdog is measured against a stable logical list generation.
  const timeoutTop = await establishTop();
  await wheel(point, "started", 0);
  await wheel(point, "moved", 36);
  const timeoutTracking = await driver.getState();
  const timeoutGeneration = Number(affordanceOf(timeoutTracking).generation);
  const timeoutSettled = await pollState("missing-Ended idle watchdog", (state) => {
    try {
      const affordance = affordanceOf(state);
      const expectedReason = affordance.reducedMotion === true
        ? "reducedMotion"
        : "missingTerminalWatchdog";
      return affordance.overscrollPhase === "idle"
        && affordance.overscrollEdge == null
        && Number(affordance.overscrollOffsetPx) === 0
        && Number(affordance.generation) > timeoutGeneration
        && affordance.lastSettleReason === expectedReason;
    } catch {
      return false;
    }
  }, 3_000);
  check("missing-ended-watchdog-settles-to-exact-idle", true, {
    tracking: compactState(timeoutTracking),
    settled: compactState(timeoutSettled),
  });
  check("missing-ended-pull-preserves-logical-scroll-and-selection",
    sameLogicalTuple(timeoutTop.state, timeoutTracking), {
      baseline: compactState(timeoutTop.state),
      tracking: compactState(timeoutTracking),
    });
  check("missing-ended-rebound-preserves-logical-scroll-and-selection",
    sameLogicalTuple(timeoutTop.state, timeoutSettled), {
      baseline: compactState(timeoutTop.state),
      settled: compactState(timeoutSettled),
    });
  scenarios.missingEnded = {
    tracking: compactState(timeoutTracking),
    settled: compactState(timeoutSettled),
  };

  // Top: endpoint and logical tuple are fixed while the rubber band tracks.
  const top = await establishTop();
  const topAffordance = affordanceOf(top.state);
  check("top-fade-is-exactly-inactive", topAffordance.topFadeActive === false
    && Number(topAffordance.topFadeProgress) === 0
    && Number(topAffordance.topFadeAlpha) === 0, compactState(top.state));
  const topGestureTimestamp = 100.0;
  await wheel(point, "moved", 0, {
    directPhase: "began",
    momentumPhase: "none",
    timestampSeconds: topGestureTimestamp,
  });
  await wheel(point, "moved", 36, {
    directPhase: "changed",
    momentumPhase: "none",
    timestampSeconds: topGestureTimestamp + 0.016,
  });
  const topTracking = await driver.getState();
  const topTrackingAffordance = affordanceOf(topTracking);
  check("top-outward-pull-tracks-positive-bounded-offset",
    topTrackingAffordance.overscrollPhase === "tracking"
      && topTrackingAffordance.overscrollEdge === "top"
      && Number(topTrackingAffordance.overscrollOffsetPx) > 0
      && Number(topTrackingAffordance.overscrollOffsetPx)
        < Number(topTrackingAffordance.overscrollMaxOffsetPx),
    compactState(topTracking));
  check("top-pull-preserves-logical-scroll-and-selection", sameLogicalTuple(top.state, topTracking), {
    before: compactState(top.state),
    tracking: compactState(topTracking),
  });
  const topSettled = await settleAfterEnded(point, topGestureTimestamp + 0.020);
  const topSettleAffordance = affordanceOf(topSettled);
  const topTrajectory = evaluateReboundTrace(
    "top",
    topSettled,
    Number(topSettleAffordance.generation),
    Number(topSettleAffordance.overscrollMaxOffsetPx),
  );
  check("top-ended-gesture-settles-to-exact-zero",
    Number(affordanceOf(topSettled).overscrollOffsetPx) === 0,
    compactState(topSettled));
  check("top-rebound-preserves-logical-scroll-and-selection",
    sameLogicalTuple(top.state, topSettled), {
      before: compactState(top.state),
      settled: compactState(topSettled),
    });
  scenarios.top = {
    endpoint: top.endpoint,
    baseline: compactState(top.state),
    tracking: compactState(topTracking),
    settled: compactState(topSettled),
    trajectory: topTrajectory,
  };
  await exerciseTerminalLifecycleScenarios("top", point, establishTop, 36);

  // Reversal: inward travel first unwinds the rubber band without moving the list.
  const reversalTop = await establishTop();
  await wheel(point, "started", 0);
  await wheel(point, "moved", 36);
  const reversalOutward = await driver.getState();
  await wheel(point, "moved", -18);
  const reversalInward = await driver.getState();
  const outwardAffordance = affordanceOf(reversalOutward);
  const inwardAffordance = affordanceOf(reversalInward);
  check("reversal-reduces-overscroll-before-list-motion",
    Number(inwardAffordance.overscrollOffsetPx) > 0
      && Number(inwardAffordance.overscrollOffsetPx)
        < Number(outwardAffordance.overscrollOffsetPx)
      && sameLogicalTuple(reversalTop.state, reversalInward), {
      outward: compactState(reversalOutward),
      inward: compactState(reversalInward),
    });
  const reversalSettled = await settleAfterEnded(point);

  // Excess inward travel in the same gesture becomes normal selection-owned
  // movement only after the visual pull has been fully unwound.
  const excessTop = await establishTop();
  await wheel(point, "started", 0);
  await wheel(point, "moved", 36);
  const excessOutward = await driver.getState();
  await wheel(point, "moved", -144);
  const excessResidual = await pollState("excess reversal residual", (state) => {
    try {
      const affordance = affordanceOf(state);
      return affordance.overscrollPhase === "idle"
        && Number(affordance.overscrollOffsetPx) === 0
        && Number(state.mainListScroll?.selectedIndex)
          > Number(excessTop.state.mainListScroll?.selectedIndex);
    } catch {
      return false;
    }
  });
  check("excess-reversal-resumes-selection-owned-movement-after-unwind",
    !sameLogicalTuple(excessTop.state, excessResidual), {
      baseline: compactState(excessTop.state),
      outward: compactState(excessOutward),
      residual: compactState(excessResidual),
    });
  await wheel(point, "ended", 0);
  const excessEnded = await pollState("excess reversal ended", (state) => {
    try {
      const affordance = affordanceOf(state);
      return affordance.overscrollPhase === "idle"
        && Number(affordance.overscrollOffsetPx) === 0
        && sameLogicalTuple(excessResidual, state);
    } catch {
      return false;
    }
  });
  scenarios.reversal = {
    partial: {
      outward: compactState(reversalOutward),
      inward: compactState(reversalInward),
      settled: compactState(reversalSettled),
    },
    excess: {
      baseline: compactState(excessTop.state),
      outward: compactState(excessOutward),
      residual: compactState(excessResidual),
      ended: compactState(excessEnded),
    },
  };

  // Middle: top occlusion is driven by logical scroll, not translated offset.
  await establishTop();
  let middle = await driver.getState();
  for (let attempt = 0; attempt < 8; attempt += 1) {
    await wheel(point, "started", 0);
    await wheel(point, "moved", -44);
    await wheel(point, "ended", 0);
    middle = await pollState("middle logical scroll", (state) => {
      try {
        return affordanceOf(state).overscrollPhase === "idle";
      } catch {
        return false;
      }
    });
    const affordance = affordanceOf(middle);
    if (affordance.atTop === false && affordance.atBottom === false) break;
  }
  const middleAffordance = affordanceOf(middle);
  check("middle-is-between-logical-boundaries",
    middleAffordance.atTop === false && middleAffordance.atBottom === false,
    compactState(middle));
  check("middle-top-fade-is-active",
    middleAffordance.topFadeActive === true
      && Number(middleAffordance.topFadeProgress) > 0
      && Number(middleAffordance.topFadeAlpha) > 0,
    compactState(middle));
  scenarios.middle = compactState(middle);

  // A native throw is direct contact followed by a distinct momentum stream.
  // Keep both rich lifecycle fields explicit so legacy touch-phase fallback
  // cannot accidentally turn these momentum samples into direct movement.
  const middleSelected = Number(middle.mainListScroll?.selectedIndex);
  const middleItemCount = Number(middle.mainListScroll?.itemCount);
  const throwDeltaSign = middleSelected >= (middleItemCount - 1) / 2 ? 1 : -1;
  const selectionDirection = -throwDeltaSign;
  const throwTimestamp = 500.0;
  await wheel(point, "moved", 0, {
    directPhase: "began", momentumPhase: "none", timestampSeconds: throwTimestamp,
  });
  await wheel(point, "moved", 24 * throwDeltaSign, {
    directPhase: "changed", momentumPhase: "none", timestampSeconds: throwTimestamp + 0.008,
  });
  await wheel(point, "moved", 24 * throwDeltaSign, {
    directPhase: "changed", momentumPhase: "none", timestampSeconds: throwTimestamp + 0.016,
  });
  await wheel(point, "moved", 0, {
    directPhase: "ended", momentumPhase: "none", timestampSeconds: throwTimestamp + 0.020,
  });
  const interiorReleased = await driver.getState();
  const interiorReleasedAffordance = affordanceOf(interiorReleased);
  check("interior-direct-release-leaves-momentum-unowned",
    interiorReleasedAffordance.atTop === false
      && interiorReleasedAffordance.atBottom === false
      && interiorReleasedAffordance.overscrollPhase === "idle"
      && Number(interiorReleasedAffordance.overscrollOffsetPx) === 0
      && interiorReleasedAffordance.momentumSuppressed === false,
    compactState(interiorReleased));

  const momentumInputs = [
    { phase: "began" as const, deltaY: 52 * throwDeltaSign, dt: 0.024 },
    { phase: "changed" as const, deltaY: 48 * throwDeltaSign, dt: 0.032 },
    { phase: "changed" as const, deltaY: 44 * throwDeltaSign, dt: 0.040 },
    { phase: "changed" as const, deltaY: 40 * throwDeltaSign, dt: 0.048 },
    { phase: "changed" as const, deltaY: 36 * throwDeltaSign, dt: 0.056 },
    { phase: "changed" as const, deltaY: 32 * throwDeltaSign, dt: 0.064 },
  ];
  const momentumStates: Json[] = [];
  for (const input of momentumInputs) {
    await wheel(point, "moved", input.deltaY, {
      directPhase: "none",
      momentumPhase: input.phase,
      timestampSeconds: throwTimestamp + input.dt,
    });
    momentumStates.push(await driver.getState());
  }
  await wheel(point, "moved", 0, {
    directPhase: "none", momentumPhase: "ended", timestampSeconds: throwTimestamp + 0.072,
  });
  const momentumTerminal = await driver.getState();
  const releaseSelected = Number(interiorReleased.mainListScroll?.selectedIndex);
  const selectedPath = momentumStates.map((state) =>
    Number(state.mainListScroll?.selectedIndex));
  const advanceCount = selectedPath.reduce((count, value, index) => {
    const previous = index === 0 ? releaseSelected : selectedPath[index - 1];
    return count + ((value - previous) * selectionDirection > 0 ? 1 : 0);
  }, 0);
  const finalMomentumState = momentumStates.at(-1)!;
  check("interior-momentum-advances-selection-over-multiple-events",
    advanceCount >= 2
      && (Number(finalMomentumState.mainListScroll?.selectedIndex) - releaseSelected)
        * selectionDirection > 0,
    { releaseSelected, selectedPath, advanceCount, selectionDirection });
  check("interior-momentum-advances-logical-list",
    !sameLogicalTuple(interiorReleased, finalMomentumState), {
      release: compactState(interiorReleased),
      final: compactState(finalMomentumState),
    });
  check("interior-momentum-never-enters-boundary-rebound",
    momentumStates.every((state) => {
      const affordance = affordanceOf(state);
      return affordance.overscrollPhase === "idle"
        && Number(affordance.overscrollOffsetPx) === 0
        && affordance.momentumSuppressed === false;
    }), momentumStates.map(compactState));
  check("interior-momentum-terminal-leaves-suppression-clear",
    affordanceOf(momentumTerminal).momentumSuppressed === false,
    compactState(momentumTerminal));
  scenarios.interiorMomentum = {
    released: compactState(interiorReleased),
    momentum: momentumStates.map(compactState),
    terminal: compactState(momentumTerminal),
    selectedPath,
    advanceCount,
  };
  screenshotResult = await captureScreenshot();

  // Bottom: endpoint and logical tuple stay fixed under an outward pull.
  const bottom = await establishBottom();
  const bottomAffordance = affordanceOf(bottom.state);
  check("bottom-top-fade-remains-active", bottomAffordance.topFadeActive === true,
    compactState(bottom.state));
  await wheel(point, "started", 0);
  await wheel(point, "moved", -36);
  const bottomTracking = await driver.getState();
  const bottomTrackingAffordance = affordanceOf(bottomTracking);
  check("bottom-outward-pull-tracks-negative-bounded-offset",
    bottomTrackingAffordance.overscrollPhase === "tracking"
      && bottomTrackingAffordance.overscrollEdge === "bottom"
      && Number(bottomTrackingAffordance.overscrollOffsetPx) < 0
      && Math.abs(Number(bottomTrackingAffordance.overscrollOffsetPx))
        < Number(bottomTrackingAffordance.overscrollMaxOffsetPx),
    compactState(bottomTracking));
  check("bottom-pull-preserves-logical-scroll-and-selection",
    sameLogicalTuple(bottom.state, bottomTracking), {
      before: compactState(bottom.state),
      tracking: compactState(bottomTracking),
    });
  const bottomSettled = await settleAfterEnded(point);
  const bottomSettleAffordance = affordanceOf(bottomSettled);
  const bottomTrajectory = evaluateReboundTrace(
    "bottom",
    bottomSettled,
    Number(bottomSettleAffordance.generation),
    Number(bottomSettleAffordance.overscrollMaxOffsetPx),
  );
  const topHalfMs = Number(topTrajectory.metrics?.halfMs);
  const bottomHalfMs = Number(bottomTrajectory.metrics?.halfMs);
  const symmetryDeltaMs = Math.abs(topHalfMs - bottomHalfMs);
  check("top-bottom-rebound-timing-is-symmetric",
    Number.isFinite(topHalfMs) && Number.isFinite(bottomHalfMs)
      && symmetryDeltaMs <= Math.max(2, Math.max(topHalfMs, bottomHalfMs) * 0.08), {
      topHalfMs,
      bottomHalfMs,
      symmetryDeltaMs,
    });
  check("bottom-ended-gesture-settles-to-exact-zero",
    Number(affordanceOf(bottomSettled).overscrollOffsetPx) === 0,
    compactState(bottomSettled));
  check("bottom-rebound-preserves-logical-scroll-and-selection",
    sameLogicalTuple(bottom.state, bottomSettled), {
      before: compactState(bottom.state),
      settled: compactState(bottomSettled),
    });
  scenarios.bottom = {
    endpoint: bottom.endpoint,
    baseline: compactState(bottom.state),
    tracking: compactState(bottomTracking),
    settled: compactState(bottomSettled),
    trajectory: bottomTrajectory,
  };
  await exerciseTerminalLifecycleScenarios("bottom", point, establishBottom, -36);

  return {
    eventPoint: { bounds, point },
    screenshot: screenshotResult,
  };
}

rmSync(outputDir, { recursive: true, force: true });
mkdirSync(outputDir, { recursive: true });

let runResult: Json = {};
let runtimeError: string | null = null;
let driverClosed = false;
let sandboxRemoved = false;
try {
  runResult = await runProbe();
} catch (error) {
  runtimeError = error instanceof Error ? error.stack ?? error.message : String(error);
  failures.push("runtime-error");
} finally {
  if (driver) {
    try {
      await driver.close();
      driverClosed = true;
    } catch (error) {
      failures.push("driver-cleanup");
      runtimeError ??= error instanceof Error ? error.message : String(error);
    }
  }
  if (runtimeError) {
    const appLog = join(sessionDir, "app.log");
    if (existsSync(appLog)) {
      writeFileSync(join(outputDir, "failed-app.log"), readFileSync(appLog));
    }
  }
  try {
    rmSync(sessionDir, { recursive: true, force: true });
    sandboxRemoved = !existsSync(sessionDir);
  } catch (error) {
    failures.push("sandbox-cleanup");
    runtimeError ??= error instanceof Error ? error.message : String(error);
  }
}

const binaryStat = existsSync(binary) ? statSync(binary) : null;
const receipt = {
  schemaVersion: 1,
  probe: "launcher-scroll-affordance",
  status: failures.length === 0 ? "pass" : "fail",
  artifact: artifact.reference,
  binary: {
    path: relative(repoRoot, binary),
    override: process.env.PROBE_BINARY != null || process.argv.includes("--binary"),
    bytes: binaryStat?.size ?? null,
    modifiedAt: binaryStat ? new Date(binaryStat.mtimeMs).toISOString() : null,
  },
  fixture: { source: "root-file-search-test-provider", query, resultCount: 80 },
  checks,
  scenarios,
  eventPoint: runResult.eventPoint ?? null,
  screenshot: runResult.screenshot ?? screenshotResult,
  cleanup: { driverClosed, sandboxRemoved },
  failures,
  error: runtimeError,
};
writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
if (failures.length > 0) process.exitCode = 1;
