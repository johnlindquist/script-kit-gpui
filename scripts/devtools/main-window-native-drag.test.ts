import { describe, expect, test } from "bun:test";
import {
  analyzeIntegratedFilmstrip,
  analyzeStationaryFidelity,
  analyzeTrace,
  evaluateGutterTransparency,
  selectTerminalAttempt,
  type DragAnalysis,
  type DragSample,
  type NativeTrace,
} from "./main-window-native-drag.ts";

type TraceOptions = {
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
  alignmentUncertaintyPx?: number;
  packetCompletionStepMs?: number;
  staleTopology?: boolean;
  untaggedInput?: boolean;
  crossesMouseUp?: boolean;
  sparseSettling?: boolean;
};

export function trace(options: TraceOptions = {}): NativeTrace {
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
    alignmentUncertaintyPx = 0.05,
    packetCompletionStepMs = 8,
    staleTopology = false,
    untaggedInput = false,
    crossesMouseUp = false,
    sparseSettling = false,
  } = options;
  const mainWindowNumber = 100;
  const footerWindowNumber = twoWindows ? 101 : null;
  const scale = 2;
  const samples: DragSample[] = [];
  let completionNs = 1_000_000_000;
  let coordinateNs = 1_000_000_000;
  let displayIntervalIndex = 0;

  const push = (
    phase: string,
    index: number,
    mainX: number,
    driftPx: number,
  ) => {
    const midpointNs = coordinateNs;
    const driftPt = driftPx / scale;
    const controlMainX = staleControls ? 100 : mainX;
    const measurementMainFrame = { x: mainX, y: 100, width: 750, height: 501 };
    const owner = nullOwnership ? null : twoWindows ? 101 : mainWindowNumber;
    const makeControl = (id: string, x: number, width: number) => {
      const frame = { x, y: 510, width, height: 28 };
      return {
        id,
        framePt: frame,
        mainFramePtAtMeasurement: measurementMainFrame,
        axWindowNumber: owner,
        measurementSource: projectedMeasurements
          ? "cached-ax-local+cgwindow"
          : "live-ax+bracketed-main-v2",
        frameRead: {
          startNs: midpointNs - 100_000,
          endNs: midpointNs + 100_000,
          midpointNs,
          value: frame,
          error: null,
        },
        ownerRead: {
          startNs: midpointNs + 100_001,
          endNs: midpointNs + 200_000,
          midpointNs: midpointNs + 150_000,
          value: owner,
          error: null,
        },
        alignmentUncertaintyPx,
        topologyFresh: !staleTopology,
        displayIntervalIndex,
        crossesEventBoundary:
          crossesMouseUp && phase === "settling" && index === 0,
      };
    };
    const controls = [
      makeControl(
        wrongTargetIds
          ? "wrong-left"
          : "script-kit-footer-left-info-hit-target",
        controlMainX + 12 + driftPt,
        100,
      ),
      ...(index === missingRightAt
        ? []
        : [
            makeControl(
              wrongTargetIds ? "wrong-right" : "script-kit-footer-button-ai",
              controlMainX + (closeControls ? 60 : 630) + driftPt,
              108,
            ),
          ]),
    ];
    const appearingWindow =
      phase === "dragged" && index >= windowAppearsAt && windowAppearsAt >= 0;
    const relevantWindowNumbers = twoWindows
      ? [100, 101]
      : appearingWindow
        ? [100, 102]
        : [100];
    samples.push({
      tNs: completionNs,
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
      packetStartNs: midpointNs - 500_000,
      packetEndNs: midpointNs + 500_000,
      displayTickNs: midpointNs - 4_000_000,
      displayIntervalIndex,
      topologyStartNs: midpointNs - 300_000,
      topologyEndNs: midpointNs + 300_000,
      topologyFresh: !staleTopology,
      topologyComplete: true,
    });
    completionNs += packetCompletionStepMs * 1_000_000;
    coordinateNs +=
      sparseSettling && phase === "settling" ? 40_000_000 : 8_000_000;
    displayIntervalIndex += 1;
  };

  for (let index = 0; index < 16; index += 1) push("pre", index, 100, 0);
  const mouseDownEventNs = coordinateNs;
  push("mouseDown", 16, 100, 0);
  for (let index = 0; index < inMotionCount; index += 1) {
    push("dragged", index, 100 + index * displacementStep, driftAt(index));
  }
  const mouseUpEventNs = coordinateNs;
  if (!missingMouseUp)
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
    schemaVersion: 2,
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
    mouseDownEventNs,
    mouseUpEventNs: missingMouseUp ? null : mouseUpEventNs,
    events: missingMouseUp
      ? []
      : [
          {
            kind: "mouseDown",
            sequence: 1,
            tag: 0x534b,
            intendedNs: mouseDownEventNs,
            actualEventNs: mouseDownEventNs,
            postStartNs: mouseDownEventNs - 10,
            postEndNs: mouseDownEventNs + 10,
            observedByEventTap: true,
          },
          {
            kind: "mouseUp",
            sequence: 2,
            tag: 0x534b,
            intendedNs: mouseUpEventNs,
            actualEventNs: mouseUpEventNs,
            postStartNs: mouseUpEventNs - 10,
            postEndNs: mouseUpEventNs + 10,
            observedByEventTap: true,
          },
        ],
    interference: {
      untaggedInputCount: untaggedInput ? 1 : 0,
      frontmostAppChanged: false,
      pointerDeviationPx: 0,
      targetMovedExternally: false,
    },
    observerHealth: {
      scheduledPackets: samples.length,
      completedPackets: samples.length,
      missedPackets: 0,
      axTimeoutCount: 0,
      topologyStaleCount: staleTopology ? 1 : 0,
      displayTickIntervalsMs: Array.from(
        { length: samples.length },
        () => 16.6667,
      ),
    },
    samples,
    filmstripFrames: [],
    errors: [],
  };
}

function withFilmstrip(fixture: NativeTrace, staleActualTime = false) {
  const dragged = fixture.samples.filter(
    (sample) => sample.phase === "dragged",
  );
  const paths = [
    import.meta.path,
    new URL("./main-window-native-drag.ts", import.meta.url).pathname,
    new URL("../agentic/macos-native-drag-sampler.swift", import.meta.url)
      .pathname,
  ];
  fixture.filmstripFrames = [0.25, 0.5, 0.75].map((fraction, index) => {
    const sample = dragged[Math.floor((dragged.length - 1) * fraction)]!;
    return {
      fraction,
      tNs: sample.tNs,
      actualFrameNs: staleActualTime
        ? fixture.mouseDownEventNs! - 1
        : sample.controls[0].frameRead!.midpointNs,
      markerEventNs: sample.controls[0].frameRead!.midpointNs,
      encodingCompletedNs: sample.tNs + 50_000_000,
      mainFramePt: { x: 100 + index * 120, y: 100, width: 750, height: 501 },
      path: paths[index],
      captureSucceeded: true,
      error: null,
    };
  });
  return fixture;
}

function analysis(
  disposition: DragAnalysis["attemptDisposition"],
): DragAnalysis {
  return { attemptDisposition: disposition } as DragAnalysis;
}

describe("native main-window drag analyzer v2", () => {
  test("accepts a dense one-window zero-drift contemporaneous trace", () => {
    const result = analyzeTrace(trace());
    expect(result.attemptDisposition).toBe("EVALUABLE_PASS");
    expect(result.motionVerdict).toBe("PASS");
    expect(result.topologyVerdict).toBe("PASS");
  });

  test("slow packet completion with timely coordinate midpoints can pass", () => {
    const result = analyzeTrace(trace({ packetCompletionStepMs: 40 }));
    expect(result.cadence.medianMs).toBe(40);
    expect(result.attemptDisposition).toBe("EVALUABLE_PASS");
  });

  test("fast completion with excessive alignment uncertainty is invalid", () => {
    const result = analyzeTrace(trace({ alignmentUncertaintyPx: 20 }));
    expect(result.attemptDisposition).toBe("INVALID_OBSERVER");
    expect(result.motionVerdict).toBe("NOT_EVALUATED");
    expect(result.controls[0].maxDriftPx).toBeNull();
  });

  test("fails one refresh with two physical pixels of relative lag", () => {
    const result = analyzeTrace(
      trace({ driftAt: (index) => (index === 25 ? 2 : 0) }),
    );
    expect(result.attemptDisposition).toBe("EVALUABLE_FAIL");
    expect(result.controls[0].maxDriftPx).toBe(2);
  });

  test("fails sustained subpixel drift through P99 and RMS", () => {
    const result = analyzeTrace(
      trace({ driftAt: (index) => (index >= 10 ? 0.6 : 0) }),
    );
    expect(result.attemptDisposition).toBe("EVALUABLE_FAIL");
    expect(result.controls[0].rmsDriftPx).toBeGreaterThan(0.35);
  });

  test("trustworthy two-window zero-drift topology is an evaluable product fail", () => {
    const result = analyzeTrace(trace({ twoWindows: true }));
    expect(result.evidenceValidity).toBe("VALID");
    expect(result.topologyVerdict).toBe("FAIL");
    expect(result.attemptDisposition).toBe("EVALUABLE_FAIL");
  });

  test("trustworthy topology count change is an evaluable product fail", () => {
    const result = analyzeTrace(trace({ windowAppearsAt: 20 }));
    expect(result.evidenceValidity).toBe("VALID");
    expect(result.topologyVerdict).toBe("FAIL");
  });

  test("missing or stale topology is invalid rather than a product fail", () => {
    const result = analyzeTrace(trace({ staleTopology: true }));
    expect(result.attemptDisposition).toBe("INVALID_OBSERVER");
    expect(result.topologyVerdict).toBe("UNKNOWN");
  });

  test("missing owner is invalid and exposes no drift verdict", () => {
    const result = analyzeTrace(trace({ nullOwnership: true }));
    expect(result.attemptDisposition).toBe("INVALID_OBSERVER");
    expect(
      result.controls.every((control) => control.maxDriftPx === null),
    ).toBe(true);
  });

  test("invalid apparent fifty-pixel drift is diagnostic only", () => {
    const result = analyzeTrace(
      trace({ alignmentUncertaintyPx: 5, driftAt: () => 50 }),
    );
    expect(result.motionVerdict).toBe("NOT_EVALUATED");
    expect(result.controls[0].maxDriftPx).toBeNull();
    expect(result.diagnosticOnly.apparentMaxDriftPx).toBe(50);
  });

  test("packet crossing explicit mouse-up is invalid", () => {
    const result = analyzeTrace(trace({ crossesMouseUp: true }));
    expect(result.attemptDisposition).toBe("INVALID_OBSERVER");
  });

  test("sparse settling without display-interval coverage is an evaluable fail", () => {
    const result = analyzeTrace(trace({ sparseSettling: true }));
    expect(result.attemptDisposition).toBe("EVALUABLE_FAIL");
    expect(result.controls[0].stableAfterSettling).toBe(false);
  });

  test("untagged input produces INVALID_INTERFERENCE", () => {
    expect(
      analyzeTrace(trace({ untaggedInput: true })).attemptDisposition,
    ).toBe("INVALID_INTERFERENCE");
  });

  test("rejects projected measurements, missing controls, and missing event tags", () => {
    expect(
      analyzeTrace(trace({ projectedMeasurements: true })).attemptDisposition,
    ).toBe("INVALID_OBSERVER");
    expect(analyzeTrace(trace({ missingRightAt: 20 })).attemptDisposition).toBe(
      "INVALID_OBSERVER",
    );
    expect(
      analyzeTrace(trace({ missingMouseUp: true })).attemptDisposition,
    ).toBe("INVALID_OBSERVER");
  });

  test("same-drag filmstrip rejects stale actual ScreenCaptureKit frame time", () => {
    expect(analyzeIntegratedFilmstrip(withFilmstrip(trace())).pass).toBe(true);
    expect(analyzeIntegratedFilmstrip(withFilmstrip(trace(), true)).pass).toBe(
      false,
    );
  });

  test("first evaluable failure is terminal and all-invalid selects nothing", () => {
    const firstFail = {
      analysis: analysis("EVALUABLE_FAIL"),
      filmstrip: { pass: true },
      id: 2,
    };
    const selected = selectTerminalAttempt([
      {
        analysis: analysis("INVALID_OBSERVER"),
        filmstrip: { pass: false },
        id: 1,
      },
      firstFail,
      {
        analysis: analysis("EVALUABLE_PASS"),
        filmstrip: { pass: true },
        id: 3,
      },
    ]);
    expect(selected?.id).toBe(2);
    expect(
      selectTerminalAttempt([
        { analysis: analysis("INVALID_OBSERVER"), id: 1 },
        { analysis: analysis("INVALID_INTERFERENCE"), id: 2 },
      ]),
    ).toBeNull();
  });

  test("a later valid pass replaces earlier invalid attempts", () => {
    const selected = selectTerminalAttempt([
      { analysis: analysis("INVALID_OBSERVER"), id: 1 },
      { analysis: analysis("INVALID_INTERFERENCE"), id: 2 },
      {
        analysis: analysis("EVALUABLE_PASS"),
        filmstrip: { pass: true },
        id: 3,
      },
    ]);
    expect(selected?.id).toBe(3);
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
  nodes.push({
    id: "script-kit-footer-left-profile-icon",
    parentId: "script-kit-footer-left-info-capsule-content",
    image: { width: 13, height: 13 },
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

  test("fails when the left capsule icon view has no rendered image", () => {
    const fixture = stationaryFixture();
    const icon = fixture.layout.fidelity.appKit.nodes.find(
      (node: any) => node.id === "script-kit-footer-left-profile-icon",
    );
    icon.image = { width: 0, height: 0 };
    const result = analyzeStationaryFidelity(fixture.layout, fixture.automationWindow);
    expect(result.pass).toBe(false);
    expect(
      result.errors.some((error) => error.includes("left footer icon has no rendered image")),
    ).toBe(true);
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
