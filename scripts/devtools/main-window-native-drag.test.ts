import { describe, expect, test } from "bun:test";
import {
  analyzeStationaryFidelity,
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
  settlingDriftAt?: (index: number) => number;
  staleControls?: boolean;
} = {}): NativeTrace {
  const {
    driftAt = () => 0,
    twoWindows = false,
    inMotionCount = 50,
    displacementStep = 5,
    missingRightAt = -1,
    settlingDrift = 0,
    settlingDriftAt = () => settlingDrift,
    staleControls = false,
  } = options;
  const mainWindowNumber = 100;
  const footerWindowNumber = twoWindows ? 101 : null;
  const scale = 2;
  const samples: DragSample[] = [];
  let tNs = 1_000_000_000;

  const push = (phase: string, index: number, mainX: number, driftPx: number) => {
    const driftPt = driftPx / scale;
    const controlMainX = staleControls ? 100 : mainX;
    const controls = [
      {
        id: "script-kit-footer-left-info-hit-target",
        framePt: { x: controlMainX + 12 + driftPt, y: 510, width: 100, height: 28 },
        axWindowNumber: twoWindows ? 101 : mainWindowNumber,
      },
      ...(index === missingRightAt ? [] : [{
        id: "script-kit-footer-button-ai",
        framePt: { x: controlMainX + 630 + driftPt, y: 510, width: 108, height: 28 },
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
  push(
    "mouseUp",
    0,
    100 + Math.max(0, inMotionCount - 1) * displacementStep,
    settlingDriftAt(0),
  );
  for (let index = 0; index < 18; index += 1) {
    push(
      "settling",
      index,
      100 + Math.max(0, inMotionCount - 1) * displacementStep,
      settlingDriftAt(index),
    );
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

  test("invalidates stale Accessibility control coordinates", () => {
    const result = analyzeTrace(trace({ staleControls: true }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("AX positions are stale"))).toBe(true);
  });

  test("fails when settling remains late", () => {
    const result = analyzeTrace(trace({ settlingDrift: 0.6 }));
    expect(result.valid).toBe(true);
    expect(result.motionThresholdsPass).toBe(false);
    expect(result.controls[0].stableAfterSettling).toBe(false);
  });

  test("fails when settling takes longer than two display refresh periods", () => {
    const result = analyzeTrace(trace({
      settlingDriftAt: (index) => index < 4 ? 0.6 : 0,
    }));
    expect(result.valid).toBe(true);
    expect(result.controls[0].settlingMs).toBe(40);
    expect(result.controls[0].thresholdsPass).toBe(false);
  });
});

function stationaryFixture(hostHeight = 501) {
  const capsules = [
    { suffix: "left-info", id: "script-kit-footer-left-info-capsule", x: 12, width: 116 },
    { suffix: "run", id: "script-kit-footer-capsule-run", x: 500, width: 100 },
    { suffix: "actions", id: "script-kit-footer-capsule-actions", x: 606, width: 80 },
    { suffix: "ai", id: "script-kit-footer-capsule-ai", x: 692, width: 46 },
  ];
  const nodes: any[] = [];
  for (const capsule of capsules) {
    const contentId = capsule.suffix === "left-info"
      ? "script-kit-footer-left-info-capsule-content"
      : `script-kit-footer-capsule-content-${capsule.suffix}`;
    nodes.push({
      id: capsule.id,
      className: "NSGlassEffectView",
      frame: { x: capsule.x, y: 2, width: capsule.width, height: 28 },
      windowFrame: { x: capsule.x, y: 2, width: capsule.width, height: 28 },
      layer: { contentsScale: 2, cornerRadius: 6 },
    });
    nodes.push({ id: contentId, parentId: capsule.id });
    if (capsule.suffix !== "left-info") {
      nodes.push({
        id: `script-kit-footer-state-layer-${capsule.suffix}`,
        parentId: contentId,
      });
    }
  }
  nodes.push({
    id: "script-kit-footer-label-actions",
    parentId: "script-kit-footer-capsule-content-actions",
    text: { value: "Actions", color: { alpha: 1 } },
    layer: { contentsScale: 2 },
  });
  return {
    layout: {
      fidelity: {
        appKit: {
          nodes,
          mainBackdropFrame: { x: 0, y: 40, width: 750, height: hostHeight - 40 },
          footerContainerFrame: { x: 0, y: 0, width: 750, height: 32 },
          transparentGapPoints: 8,
          backdropFooterIntersectionArea: 0,
          outerWindowHasShadow: false,
        },
      },
    },
    automationWindow: { bounds: { x: 381, y: 166, width: 750, height: hostHeight } },
  };
}

describe("stationary native footer analyzer", () => {
  test("accepts the exact fresh-launch fixture", () => {
    const fixture = stationaryFixture();
    const result = analyzeStationaryFidelity(
      fixture.layout,
      fixture.automationWindow,
      { expectedHostSize: { width: 750, height: 501 } },
    );
    expect(result.pass).toBe(true);
  });

  test("accepts lifecycle-settled height when the material partition remains exact", () => {
    const fixture = stationaryFixture(480);
    expect(analyzeStationaryFidelity(fixture.layout, fixture.automationWindow).pass).toBe(true);
  });

  test("fails a default-fixture size mismatch", () => {
    const fixture = stationaryFixture(480);
    const result = analyzeStationaryFidelity(
      fixture.layout,
      fixture.automationWindow,
      { expectedHostSize: { width: 750, height: 501 } },
    );
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("expected 750x501"))).toBe(true);
  });

  test("fails a material partition that bridges the detached gutter", () => {
    const fixture = stationaryFixture();
    fixture.layout.fidelity.appKit.mainBackdropFrame.y = 39;
    const result = analyzeStationaryFidelity(fixture.layout, fixture.automationWindow);
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("not exactly partitioned"))).toBe(true);
  });
});
