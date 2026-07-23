import { describe, expect, test } from "bun:test";
import {
  analyzeIntegratedFilmstrip,
  analyzeStationaryFidelity,
  analyzeTrace,
  evaluateGutterTransparency,
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
  nullOwnership?: boolean;
  projectedMeasurements?: boolean;
  windowAppearsAt?: number;
  missingMouseUp?: boolean;
  wrongTargetIds?: boolean;
  closeControls?: boolean;
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
    nullOwnership = false,
    projectedMeasurements = false,
    windowAppearsAt = -1,
    missingMouseUp = false,
    wrongTargetIds = false,
    closeControls = false,
  } = options;
  const mainWindowNumber = 100;
  const footerWindowNumber = twoWindows ? 101 : null;
  const scale = 2;
  const samples: DragSample[] = [];
  let tNs = 1_000_000_000;

  const push = (phase: string, index: number, mainX: number, driftPx: number) => {
    const driftPt = driftPx / scale;
    const controlMainX = staleControls ? 100 : mainX;
    const measurementMainFrame = { x: mainX, y: 100, width: 750, height: 501 };
    const controls = [
      {
        id: wrongTargetIds ? "wrong-left" : "script-kit-footer-left-info-hit-target",
        framePt: { x: controlMainX + 12 + driftPt, y: 510, width: 100, height: 28 },
        mainFramePtAtMeasurement: measurementMainFrame,
        axWindowNumber: nullOwnership ? null : twoWindows ? 101 : mainWindowNumber,
        measurementSource: projectedMeasurements
          ? "cached-ax-local+cgwindow"
          : "live-ax+interpolated-main",
      },
      ...(index === missingRightAt ? [] : [{
        id: wrongTargetIds ? "wrong-right" : "script-kit-footer-button-ai",
        framePt: {
          x: controlMainX + (closeControls ? 60 : 630) + driftPt,
          y: 510,
          width: 108,
          height: 28,
        },
        mainFramePtAtMeasurement: measurementMainFrame,
        axWindowNumber: nullOwnership ? null : twoWindows ? 101 : mainWindowNumber,
        measurementSource: projectedMeasurements
          ? "cached-ax-local+cgwindow"
          : "live-ax+interpolated-main",
      }]),
    ];
    const appearingWindow = phase === "dragged" && index >= windowAppearsAt && windowAppearsAt >= 0;
    const relevantWindowNumbers = twoWindows ? [100, 101] : appearingWindow ? [100, 102] : [100];
    samples.push({
      tNs,
      phase,
      mainWindowNumber,
      mainFramePt: { x: mainX, y: 100, width: 750, height: 501 },
      footerWindowNumber,
      footerFramePt: twoWindows
        ? { x: mainX + driftPt, y: 569, width: 750, height: 32 }
        : null,
      relevantWindowCount: relevantWindowNumbers.length,
      relevantWindowNumbers,
      controls,
    });
    tNs += 8_000_000;
  };

  for (let index = 0; index < 16; index += 1) push("pre", index, 100, 0);
  push("mouseDown", 16, 100, 0);
  for (let index = 0; index < inMotionCount; index += 1) {
    push("dragged", index, 100 + index * displacementStep, driftAt(index));
  }
  const mouseUpEventNs = tNs;
  if (!missingMouseUp) {
    push(
      "mouseUp",
      0,
      100 + Math.max(0, inMotionCount - 1) * displacementStep,
      settlingDriftAt(0),
    );
  }
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
    mouseUpEventNs: missingMouseUp ? null : mouseUpEventNs,
    samples,
    filmstripFrames: [],
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

  test("rejects projected rather than live Accessibility measurements", () => {
    const result = analyzeTrace(trace({ projectedMeasurements: true }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("live-ax+interpolated-main"))).toBe(true);
  });

  test("rejects null control ownership", () => {
    const result = analyzeTrace(trace({ nullOwnership: true }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("non-null"))).toBe(true);
  });

  test("rejects a native window appearing during the drag", () => {
    const result = analyzeTrace(trace({ windowAppearsAt: 20 }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("count changed"))).toBe(true);
  });

  test("rejects a missing explicit mouse-up", () => {
    const result = analyzeTrace(trace({ missingMouseUp: true }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("mouse-up"))).toBe(true);
  });

  test("rejects wrong target identifiers", () => {
    const result = analyzeTrace(trace({ wrongTargetIds: true }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("exact left"))).toBe(true);
  });

  test("rejects controls that are not far apart", () => {
    const result = analyzeTrace(trace({ closeControls: true }));
    expect(result.valid).toBe(false);
    expect(result.errors.some((error) => error.includes("far apart"))).toBe(true);
  });

  test("rejects missing same-run filmstrip captures", () => {
    const result = analyzeIntegratedFilmstrip(trace());
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("expected 3"))).toBe(true);
  });

  test("rejects an explicitly failed same-run filmstrip capture", () => {
    const fixture = trace();
    const dragged = fixture.samples.filter((sample) => sample.phase === "dragged");
    fixture.filmstripFrames = [0.25, 0.5, 0.75].map((fraction, index) => ({
      fraction,
      tNs: dragged[Math.floor((dragged.length - 1) * fraction)]!.tNs,
      mainFramePt: { x: 100 + index * 120, y: 100, width: 750, height: 501 },
      path: import.meta.path,
      captureSucceeded: index !== 1,
      error: index === 1 ? "injected capture failure" : null,
    }));
    const result = analyzeIntegratedFilmstrip(fixture);
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("capture 2 failed"))).toBe(true);
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

function stationaryFixture(hostHeight = 480) {
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
  nodes.push({
    id: "script-kit-footer-left-info-hit-target",
    parentId: "script-kit-footer-left-info-capsule-content",
  });
  nodes.push({
    id: "script-kit-footer-left-info-keycap",
    parentId: "script-kit-footer-left-info-capsule-content",
    frame: { x: 8, y: 4, width: 41, height: 20 },
    layer: { contentsScale: 2, cornerRadius: 6, borderWidth: 1 },
  });
  nodes.push({
    id: "script-kit-footer-left-info-keycap-glyph",
    parentId: "script-kit-footer-left-info-keycap",
    text: { value: "Space", color: { alpha: 0.8 } },
    layer: { contentsScale: 2 },
  });
  return {
    layout: {
      components: [
        {
          name: "main-content-stage",
          bounds: { x: 0, y: 0, width: 750, height: hostHeight - 40 },
          visibleBounds: { x: 0, y: 0, width: 750, height: hostHeight - 40 },
          clipBounds: { x: 0, y: 0, width: 750, height: hostHeight - 40 },
          measurementProvenance: "paint-time",
          coordinateSpace: "window",
        },
        {
          name: "main-window-dialog-layer-boundary",
          bounds: { x: 1, y: 1, width: 748, height: hostHeight - 42 },
          visibleBounds: { x: 1, y: 1, width: 748, height: hostHeight - 42 },
          clipBounds: { x: 1, y: 1, width: 748, height: hostHeight - 42 },
          measurementProvenance: "paint-time",
          coordinateSpace: "window",
        },
      ],
      fidelity: {
        appKit: {
          nodes,
          mainBackdropFrame: { x: 0, y: 40, width: 750, height: hostHeight - 40 },
          footerContainerFrame: { x: 0, y: 0, width: 750, height: 32 },
          transparentGapPoints: 8,
          backdropFooterIntersectionArea: 0,
          outerWindowHasShadow: false,
          mainBackdropLayer: {
            shadowOpacity: 0,
            shadowRadius: 0,
            shadowOffsetX: 0,
            shadowOffsetY: 0,
            hasShadowPath: false,
          },
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
      { expectedHostSize: { width: 750, height: 480 } },
    );
    expect(result.pass).toBe(true);
  });

  test("accepts an alternate height when the material partition remains exact", () => {
    const fixture = stationaryFixture(501);
    expect(analyzeStationaryFidelity(fixture.layout, fixture.automationWindow).pass).toBe(true);
  });

  test("fails a default-fixture size mismatch", () => {
    const fixture = stationaryFixture(501);
    const result = analyzeStationaryFidelity(
      fixture.layout,
      fixture.automationWindow,
      { expectedHostSize: { width: 750, height: 480 } },
    );
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("expected 750x480"))).toBe(true);
  });

  test("fails a material partition that bridges the detached gutter", () => {
    const fixture = stationaryFixture();
    fixture.layout.fidelity.appKit.mainBackdropFrame.y = 39;
    const result = analyzeStationaryFidelity(fixture.layout, fixture.automationWindow);
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("not exactly partitioned"))).toBe(true);
  });

  test("fails when a GPUI dialog boundary reaches into the detached gutter", () => {
    const fixture = stationaryFixture();
    fixture.layout.components[1].bounds.height = 480;
    fixture.layout.components[1].visibleBounds.height = 480;
    fixture.layout.components[1].clipBounds.height = 480;
    const result = analyzeStationaryFidelity(fixture.layout, fixture.automationWindow);
    expect(result.pass).toBe(false);
    expect(
      result.errors.some((error) =>
        error.includes("main-window-dialog-layer-boundary bounds is not bounded")
      ),
    ).toBe(true);
  });

  test("fails when the left capsule loses its shortcut keycap or glyph", () => {
    const fixture = stationaryFixture();
    fixture.layout.fidelity.appKit.nodes = fixture.layout.fidelity.appKit.nodes.filter(
      (node: any) => !node.id.startsWith("script-kit-footer-left-info-keycap"),
    );
    const result = analyzeStationaryFidelity(fixture.layout, fixture.automationWindow);
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("left footer shortcut keycap is missing"))).toBe(true);
    expect(result.errors.some((error) => error.includes("left footer shortcut glyph is missing"))).toBe(true);
  });
});

describe("detached gutter alpha analyzer", () => {
  const transparent = {
    pixelWidth: 1500,
    pixelHeight: 1002,
    gapY: 922,
    gapHeight: 16,
    fullAlphaMin: 0,
    fullAlphaMax: 0,
    fullAlphaMean: 0,
    centerAlphaMin: 0,
    centerAlphaMax: 0,
    centerAlphaMean: 0,
  };

  test("accepts a physically transparent sixteen-pixel gutter", () => {
    expect(evaluateGutterTransparency(transparent).pass).toBe(true);
  });

  test("rejects the audited full-width shadow veil", () => {
    const result = evaluateGutterTransparency({
      ...transparent,
      fullAlphaMax: 0.333333,
      fullAlphaMean: 0.28488,
      centerAlphaMax: 0.313725,
      centerAlphaMean: 0.288204,
    });
    expect(result.pass).toBe(false);
    expect(result.errors.some((error) => error.includes("full gutter alpha max"))).toBe(true);
    expect(result.errors.some((error) => error.includes("central gutter alpha max"))).toBe(true);
  });
});
