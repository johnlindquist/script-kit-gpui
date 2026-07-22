import { describe, expect, test } from "bun:test";
import {
  analyzeTrace,
  type DragSample,
  type NativeTrace,
} from "./main-window-native-drag.ts";

function trace(options: {
  driftAt?: (index: number) => number;
  twoWindows?: boolean;
  inMotionCount?: number;
  displacementStep?: number;
  missingRightAt?: number;
  settlingDrift?: number;
} = {}): NativeTrace {
  const {
    driftAt = () => 0,
    twoWindows = false,
    inMotionCount = 50,
    displacementStep = 5,
    missingRightAt = -1,
    settlingDrift = 0,
  } = options;
  const mainWindowNumber = 100;
  const footerWindowNumber = twoWindows ? 101 : null;
  const scale = 2;
  const samples: DragSample[] = [];
  let tNs = 1_000_000_000;

  const push = (phase: string, index: number, mainX: number, driftPx: number) => {
    const driftPt = driftPx / scale;
    const controls = [
      {
        id: "script-kit-footer-left-info-hit-target",
        framePt: { x: mainX + 12 + driftPt, y: 510, width: 100, height: 28 },
        axWindowNumber: twoWindows ? 101 : mainWindowNumber,
      },
      ...(index === missingRightAt ? [] : [{
        id: "script-kit-footer-button-ai",
        framePt: { x: mainX + 630 + driftPt, y: 510, width: 108, height: 28 },
        axWindowNumber: twoWindows ? 101 : mainWindowNumber,
      }]),
    ];
    samples.push({
      tNs,
      phase,
      mainWindowNumber,
      mainFramePt: { x: mainX, y: 100, width: 750, height: 501 },
      footerWindowNumber,
      footerFramePt: twoWindows
        ? { x: mainX + driftPt, y: 569, width: 750, height: 32 }
        : null,
      relevantWindowCount: twoWindows ? 2 : 1,
      controls,
    });
    tNs += 8_000_000;
  };

  for (let index = 0; index < 16; index += 1) push("pre", index, 100, 0);
  push("mouseDown", 16, 100, 0);
  for (let index = 0; index < inMotionCount; index += 1) {
    push("dragged", index, 100 + index * displacementStep, driftAt(index));
  }
  push("mouseUp", 0, 100 + Math.max(0, inMotionCount - 1) * displacementStep, settlingDrift);
  for (let index = 0; index < 18; index += 1) {
    push("settling", index, 100 + Math.max(0, inMotionCount - 1) * displacementStep, settlingDrift);
  }

  return {
    schemaVersion: 1,
    status: "ok",
    pid: 42,
    trajectory: "synthetic",
    durationMs: 400,
    requestedDeltaPt: { x: 245, y: 0 },
    accessibilityTrusted: true,
    display: {
      displayID: 1,
      refreshHz: 60,
      backingScale: scale,
      boundsPt: { x: 0, y: 0, width: 1512, height: 982 },
    },
    sampleTargetHz: 120,
    samples,
    errors: [],
  };
}

describe("native main-window drag analyzer", () => {
  test("accepts a dense one-window zero-drift trace", () => {
    const result = analyzeTrace(trace());
    expect(result.valid).toBe(true);
    expect(result.topology).toBe("one-window");
    expect(result.motionThresholdsPass).toBe(true);
    expect(result.overallPass).toBe(true);
  });

  test("fails one frame with two physical pixels of lag", () => {
    const result = analyzeTrace(trace({ driftAt: (index) => index === 25 ? 2 : 0 }));
    expect(result.valid).toBe(true);
    expect(result.motionThresholdsPass).toBe(false);
    expect(result.controls[0].maxDriftPx).toBe(2);
  });

  test("fails accumulated subpixel drift through P99 and RMS", () => {
    const result = analyzeTrace(trace({ driftAt: (index) => index >= 10 ? 0.6 : 0 }));
    expect(result.valid).toBe(true);
    expect(result.motionThresholdsPass).toBe(false);
    expect(result.controls[0].rmsDriftPx).toBeGreaterThan(0.35);
  });

  test("rejects before-and-after-only traces", () => {
    const result = analyzeTrace(trace({ inMotionCount: 0 }));
    expect(result.valid).toBe(false);
    expect(result.verdict).toBe("INVALID");
  });

  test("rejects traces without meaningful movement", () => {
    const result = analyzeTrace(trace({ displacementStep: 0 }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("displacement"))).toBe(true);
  });

  test("rejects a missing far control sample", () => {
    const result = analyzeTrace(trace({ missingRightAt: 20 }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("resolved in"))).toBe(true);
  });

  test("fails two-window topology even with zero measured drift", () => {
    const result = analyzeTrace(trace({ twoWindows: true }));
    expect(result.valid).toBe(true);
    expect(result.motionThresholdsPass).toBe(true);
    expect(result.topology).toBe("two-window");
    expect(result.overallPass).toBe(false);
  });

  test("fails when settling remains late", () => {
    const result = analyzeTrace(trace({ settlingDrift: 0.6 }));
    expect(result.valid).toBe(true);
    expect(result.motionThresholdsPass).toBe(false);
    expect(result.controls[0].stableAfterSettling).toBe(false);
  });
});
